// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 review follow-up: the Mercury-regression diagnostic harness (data tier, `#[ignore]`d,
//! never run by CI profiles by default; issue #42). It reproduces the runs tabulated in
//! IMPLEMENTATION-NOTES "WP22 review follow-up": Sun + 8 planets + geocentric Moon from
//! header.440 (Pluto optional), all-body EIH + DE440 J2 + LT by default, optional small bodies,
//! 10 Julian yr every 10 d, against de440.bsp. It asserts nothing; it prints Mercury's and Venus'
//! heliocentric residuals (maximum and along-track at the end), Mars' heliocentric and the Sun's
//! barycentric maximum.
//!
//! Configuration through the `FA_WP22_DIAG` environment variable, comma-separated tokens:
//! `top:N` (the N asteroids with the largest header GMs), `ids:2000001+2000004` (NAIF ids),
//! `all` (all 343), `kbos` (the 3 identified KBOs), `ring` (the 36-point ring), `pluto`,
//! `noeih`, `noj2`, `nolt`, `noreact:1+10` (Minor bodies do not pull these Major NAIF ids),
//! `years:Y`. Example:
//! `FA_WP22_DIAG=all,nolt cargo nextest run -p ferroastro-propagate --cargo-profile release
//! --profile data --run-ignored only -E 'test(data_wp22_mercury_regression_diagnostic)'`.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    reason = "diagnostic test: failures are test failures; indices in range by construction; one \
              linear diagnostic; the sample count is a small positive integer"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Barycentric, Icrf, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};

/// km to m (printing).
const KM_TO_M: f64 = 1e3; // lint: allow-literal(unit conversion km -> m)
/// Sampling interval, days.
const STEP_DAYS: f64 = 10.0;
use ferroastro_core::units::{Length, TimeSpan, Velocity};
use ferroastro_eih::{Eih, RhsAcceleration};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, LtReaction, MOON_TAYLOR_TERMS_DEFAULT, MinorFromMajors,
    MinorReaction, MoonVariables, NBody, NBodyModel, NBodyPropagator, NewtonianMutual, Pole,
    RingMasses, SolarJ2WithReaction, SolarLenseThirring,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_system::{
    Catalogue, InitialConditions, KBO_IDENTIFICATION, KBO_RING, SmallBodyStates, Tier,
    kbo_ring_states, place_small_bodies,
};
use std::path::PathBuf;

fn pos(s: &State<Icrf, Barycentric>) -> [f64; 3] {
    s.position().components().map(Length::km)
}
fn vel(s: &State<Icrf, Barycentric>) -> [f64; 3] {
    s.velocity().map(Velocity::km_per_s)
}
fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[derive(Debug)]
struct MinorExcept {
    skip: Vec<i32>,
}
impl ForceTerm<Icrf, f64> for MinorExcept {
    fn name(&self) -> &'static str {
        "minor-except"
    }
    fn gauge(&self) -> ferroastro_forces::Gauge {
        ferroastro_forces::Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<ferroastro_forces::TermParameter> {
        Vec::new()
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Minor, Tier::Major]
    }
    fn bind(
        &mut self,
        _l: &ferroastro_forces::Layout<f64>,
    ) -> Result<(), ferroastro_forces::ForceError> {
        Ok(())
    }
    fn accumulate(
        &self,
        _t: f64,
        view: &ferroastro_forces::View<'_, f64>,
        acc: &mut ferroastro_forces::Accelerations<'_, f64>,
    ) -> Result<(), ferroastro_forces::ForceError> {
        let l = view.layout();
        for &i in l.indices(Tier::Minor) {
            let pi = view.position(i)?;
            for &j in l.indices(Tier::Major) {
                let d = sub(view.position(j)?, pi);
                let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                let s = 1.0 / (r2 * r2.sqrt());
                acc.add(i, d.map(|x| l.gm(j) * s * x));
                if !self.skip.contains(&l.ids()[j].0) {
                    acc.add(j, d.map(|x| -l.gm(i) * s * x));
                }
            }
        }
        Ok(())
    }
    fn potential_over_g(&self, _v: &ferroastro_forces::View<'_, f64>) -> Option<f64> {
        None
    }
}

/// Verifies: NUM-009, REF-003
#[test]
#[ignore = "data diagnostic (issue #42): reads the pinned header.440 and de440.bsp; configure with FA_WP22_DIAG"]
fn data_wp22_mercury_regression_diagnostic() {
    let diag = std::env::var("FA_WP22_DIAG").unwrap_or_default();
    let has = |t: &str| diag.split(',').any(|x| x == t);
    let val = |t: &str| {
        diag.split(',')
            .find_map(|x| x.strip_prefix(&format!("{t}:")).map(str::to_owned))
    };
    let years: f64 = val("years").map_or(10.0, |s| s.parse().unwrap());
    let hf = ferroastro_testdata::locate_product("de440-header").unwrap();
    let hprov = hf.provenance();
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::locate_product("de440-bsp").unwrap();
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let de = De440::load_cached().unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let mut cat = Catalogue::from_header(&h, &hprov).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    let unid: f64 = cat
        .unidentified_masses()
        .iter()
        .map(|m| m.gm.km3_per_s2())
        .sum();
    println!(
        "unidentified masses: {} of total GM {unid:.4} km^3/s^2 = {:.3e} GMS",
        cat.unidentified_masses().len(),
        unid / de.gm(ferroastro_constants::Body::Sun).km3_per_s2()
    );
    // Majors: Sun, planets, Earth, Moon (no Pluto), header ICs.
    let major_ids: Vec<NaifId> = ferroastro_system::DE440_MAJOR
        .iter()
        .map(|&(id, _)| id)
        .filter(|&id| id != NaifId::PLUTO_BARYCENTER || has("pluto"))
        .collect();
    let mut asteroids: Vec<(NaifId, f64)> = cat
        .ids_in(Tier::Minor)
        .map(|id| (id, cat.gm(id).unwrap().km3_per_s2()))
        .collect();
    asteroids.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut chosen: Vec<NaifId> = Vec::new();
    if let Some(n) = val("top") {
        chosen.extend(asteroids.iter().take(n.parse().unwrap()).map(|a| a.0));
    }
    if let Some(list) = val("ids") {
        chosen.extend(list.split('+').map(|s| NaifId(s.parse().unwrap())));
    }
    if has("all") {
        chosen.extend(asteroids.iter().map(|a| a.0));
    }
    if has("kbos") {
        cat.with_identified_kbos().unwrap();
        chosen.extend(KBO_IDENTIFICATION.iter().map(|k| k.id));
    }
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let mut states = ic.states().clone();
    let mut bodies: Vec<NBody> = major_ids
        .iter()
        .map(|&id| NBody {
            id,
            gm: Catalogue::de440_major(&de).gm(id).unwrap().km3_per_s2(),
            tier: Tier::Major,
        })
        .collect();
    if !chosen.is_empty() {
        let placed = place_small_bodies(&file, &chosen, &sun).unwrap();
        for (id, st) in placed.states().unwrap() {
            states.push(id, &st).unwrap();
            bodies.push(NBody {
                id,
                gm: cat.gm(id).unwrap().km3_per_s2(),
                tier: Tier::Minor,
            });
        }
    }
    if has("ring") {
        cat.with_kbo_ring().unwrap();
        let ring = kbo_ring_states::<f64>(&KBO_RING, h.value("AU").unwrap()).unwrap();
        for (id, st) in ring.states().unwrap() {
            states.push(id, &st).unwrap();
            bodies.push(NBody {
                id,
                gm: cat.gm(id).unwrap().km3_per_s2(),
                tier: Tier::Ring,
            });
        }
    }
    {
        let mut ptot = [0.0; 3];
        let mut mtot = 0.0;
        for b in &bodies {
            if b.tier == Tier::Ring {
                mtot += b.gm;
                continue;
            }
            let st = states.state(b.id).unwrap();
            let w = vel(&st);
            for k in 0..3 {
                ptot[k] += b.gm * w[k];
            }
            mtot += b.gm;
        }
        // geocentric pair: states hold barycentric Earth and Moon already (ic.states())
        let v = ptot.map(|x| x / mtot);
        println!(
            "initial V_cm = {:?} km/s, |V_cm| = {:.4e} km/s, predicted drift over {years} yr = {:.4} km",
            v,
            norm(v),
            norm(v) * years * DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY
        );
    }
    let c = de.speed_of_light();
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
    if bodies.iter().any(|b| b.tier == Tier::Minor) {
        if let Some(list) = val("noreact") {
            terms.push(Box::new(MinorExcept {
                skip: list.split('+').map(|x| x.parse().unwrap()).collect(),
            }));
        } else {
            terms.push(Box::new(MinorFromMajors::new(MinorReaction::On)));
        }
    }
    if bodies.iter().any(|b| b.tier == Tier::Ring) {
        terms.push(Box::new(RingMasses));
    }
    terms.push(Box::new(
        GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap(),
    ));
    if !has("noeih") {
        terms.push(Box::new(
            Eih::new(c, 1.0, 1.0, RhsAcceleration::Newtonian).unwrap(),
        ));
    }
    let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
        .unwrap()
        .unit_vector();
    if !has("noj2") {
        terms.push(Box::new(
            SolarJ2WithReaction::new(
                de.sun_j2(),
                Length::from_km(de.sun_radius().km()).unwrap(),
                Pole::from_components(k).unwrap(),
            )
            .unwrap(),
        ));
    }
    if !has("nolt") {
        terms.push(Box::new(
            SolarLenseThirring::new(
                ferroastro_propagate::de440_solar_spin(&de, "scratch").unwrap(),
                c,
                1.0,
                Pole::from_components(k).unwrap(),
                LtReaction::On,
            )
            .unwrap(),
        ));
    }
    let model = NBodyModel::<Icrf>::new(
        &bodies,
        MoonVariables::Geocentric {
            emrat: de.earth_moon_mass_ratio(),
        },
        terms,
    )
    .unwrap();
    let dt0 = TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap();
    let mut p = NBodyPropagator::new(
        model,
        &states,
        ic.earth_moon(),
        ic.epoch(),
        dt0,
        Ias15Config::default(),
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    let n = (years * DAYS_PER_JULIAN_YEAR / STEP_DAYS).round() as u32;
    let mut worst = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut last = [0.0; 3];
    let mut venus_max = 0.0f64;
    let mut venus_end = 0.0f64;
    let mut series = Vec::new();
    for kk in 1..=n {
        let e = ic.epoch()
            + TimeSpan::from_seconds(f64::from(kk) * STEP_DAYS * SECONDS_PER_DAY).unwrap();
        p.advance_to(e).unwrap();
        let s = p.state().unwrap();
        let want = |id: i32| pos(&spk.state::<Barycentric, f64>(NaifId(id), e).unwrap());
        let got = |id: i32| pos(&s.state(NaifId(id)).unwrap());
        let (gs, ws) = (got(10), want(10));
        let dme = sub(sub(got(1), gs), sub(want(1), ws));
        let dma = sub(sub(got(4), gs), sub(want(4), ws));
        let dve = sub(sub(got(2), gs), sub(want(2), ws));
        let rv = sub(want(2), ws);
        let vv = sub(
            vel(&spk.state::<Barycentric, f64>(NaifId(2), e).unwrap()),
            vel(&spk.state::<Barycentric, f64>(NaifId(10), e).unwrap()),
        );
        let alongv = (dve[0] * vv[0] + dve[1] * vv[1] + dve[2] * vv[2]) / norm(vv);
        let _ = rv;
        venus_max = venus_max.max(norm(dve));
        venus_end = alongv;
        worst.0 = worst.0.max(norm(dme));
        worst.1 = worst.1.max(norm(dma));
        worst.2 = worst.2.max(norm(sub(gs, ws)));
        // along-track component of Mercury residual
        let r = sub(want(1), ws);
        let v = sub(
            vel(&spk.state::<Barycentric, f64>(NaifId(1), e).unwrap()),
            vel(&spk.state::<Barycentric, f64>(NaifId(10), e).unwrap()),
        );
        let vn = norm(v);
        let along = (dme[0] * v[0] + dme[1] * v[1] + dme[2] * v[2]) / vn;
        let rad = (dme[0] * r[0] + dme[1] * r[1] + dme[2] * r[2]) / norm(r);
        worst.3 = worst.3.max(along.abs());
        last = [along, rad, norm(dme)];
        if kk % 73 == 0 {
            series.push(format!(
                "{:.1}y:{:.1}m",
                f64::from(kk) * STEP_DAYS / DAYS_PER_JULIAN_YEAR,
                along * KM_TO_M
            ));
        }
    }
    println!(
        "DIAG {diag:?}: {} bodies; Mercury helio max {:.2} m (max along {:.2} m; end along {:.2} rad {:.2} m); Mars helio max {:.3} km; Sun bary max {:.3} km; {} steps {:.1} s; along series {}",
        bodies.len(),
        worst.0 * KM_TO_M,
        worst.3 * KM_TO_M,
        last[0] * KM_TO_M,
        last[1] * KM_TO_M,
        worst.1,
        worst.2,
        p.stats().steps,
        t0.elapsed().as_secs_f64(),
        series.join(" ")
    );
    println!(
        "VENUS {diag:?}: helio max {:.2} m, end along {:.2} m",
        venus_max * KM_TO_M,
        venus_end * KM_TO_M
    );
}
