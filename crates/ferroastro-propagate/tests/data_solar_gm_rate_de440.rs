// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Issue #42 (OI-04), data tier: the opt-in, **inferred** time-varying solar GM against
//! de440.bsp. The 353-body configuration the issue #42 investigation calls `all`: the Sun, the 8
//! planets and the geocentric Moon from header.440 at JDEPOC (no Pluto), the 343 asteroids of
//! the committed REF-003 states as Minor bodies; all-body EIH, DE440 solar J2 (WGCCRE 2015 pole) and solar
//! Lense–Thirring with reaction; IAS15 default, f64; 10 Julian yr sampled every 10 d. Two runs,
//! in two threads: the default model (constant GM☉) and the same model with
//! [`SolarGmVariation`] under `de440_inferred_solar_gm_rate` (header `GMSDOT` read as a
//! dimensionless rate per Julian year, reference epoch J2000.0 TDB).
//!
//! The convention is inferred from a fit against this same de440.bsp (and from JPL IOM
//! 335N-20-01's wording); no public JPL document states it. The assertions are the
//! pre-registered bounds of `tests/vectors/solar-gm-rate.toml`, committed before the library
//! term existed, and provisional on OI-04.
//!
//! Reads the pinned header.440 and de440.bsp from the data cache (no network); it skips with a
//! notice when one is absent and fails instead under `FA_REQUIRE_DATA=1`:
//! `FA_REQUIRE_DATA=1 cargo nextest run -p ferroastro-propagate --cargo-profile release --profile
//! data --run-ignored only -E 'test(data_inferred_solar_gm_rate_ten_years_vs_de440_bsp)'`.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::suboptimal_flops,
    clippy::too_many_lines,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              reference arithmetic is written plainly; one linear model assembly"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Barycentric, Icrf, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_core::units::{Length, TimeSpan, Velocity};
use ferroastro_eih::{Eih, RhsAcceleration};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, LtReaction, MOON_TAYLOR_TERMS_DEFAULT, MinorFromMajors,
    MinorReaction, MoonVariables, NBody, NBodyModel, NBodyPropagator, NewtonianMutual, Pole,
    SolarGmVariation, SolarJ2WithReaction, SolarLenseThirring,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_propagate::{de440_inferred_solar_gm_rate, de440_solar_spin};
use ferroastro_system::{Catalogue, InitialConditions, SmallBodyStates, Tier, place_small_bodies};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const TEST: &str = "data_inferred_solar_gm_rate_ten_years_vs_de440_bsp";
/// km to m.
const KM_TO_M: f64 = 1e3; // lint: allow-literal(unit conversion km -> m)
/// Sampling interval, days.
const STEP_DAYS: f64 = 10.0;
/// Span, Julian years.
const YEARS: f64 = 10.0;
/// The bodies compared: the Sun (reference), Mercury and Venus.
const SAMPLED: [NaifId; 3] = [
    NaifId::SUN,
    NaifId::MERCURY_BARYCENTER,
    NaifId::VENUS_BARYCENTER,
];

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

/// One run's samples: per 10-d sample the barycentric positions (km) of [`SAMPLED`].
struct Run {
    samples: Vec<[[f64; 3]; 3]>,
    bodies: usize,
    steps: u64,
    seconds: f64,
    description: String,
}

/// The 353-body model from header.440 and the REF-003 asteroid states, advanced over the span;
/// `gm_rate` adds the opt-in inferred GM☉(t) term (last in the term order).
fn run(h: &Header, hprov: &str, gm_rate: bool) -> Run {
    let de = De440::load_cached().unwrap();
    let ic = InitialConditions::<f64>::from_header(h, hprov).unwrap();
    let cat = Catalogue::from_header(h, hprov).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    // Majors: Sun, planets, Earth, Moon (no Pluto), header ICs. Minors: all 343 asteroids, in
    // decreasing GM order (the order of the investigation's `all` configuration).
    let major_ids: Vec<NaifId> = ferroastro_system::DE440_MAJOR
        .iter()
        .map(|&(id, _)| id)
        .filter(|&id| id != NaifId::PLUTO_BARYCENTER)
        .collect();
    let mut asteroids: Vec<(NaifId, f64)> = cat
        .ids_in(Tier::Minor)
        .map(|id| (id, cat.gm(id).unwrap().km3_per_s2()))
        .collect();
    asteroids.sort_by(|a, b| b.1.total_cmp(&a.1));
    let chosen: Vec<NaifId> = asteroids.iter().map(|a| a.0).collect();
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
    let placed = place_small_bodies(&file, &chosen, &sun).unwrap();
    for (id, st) in placed.states().unwrap() {
        states.push(id, &st).unwrap();
        bodies.push(NBody {
            id,
            gm: cat.gm(id).unwrap().km3_per_s2(),
            tier: Tier::Minor,
        });
    }
    let c = de.speed_of_light();
    let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
        .unwrap()
        .unit_vector();
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
        Box::new(NewtonianMutual),
        Box::new(MinorFromMajors::new(MinorReaction::On)),
        Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        Box::new(Eih::new(c, 1.0, 1.0, RhsAcceleration::Newtonian).unwrap()),
        Box::new(
            SolarJ2WithReaction::new(
                de.sun_j2(),
                Length::from_km(de.sun_radius().km()).unwrap(),
                Pole::from_components(k).unwrap(),
            )
            .unwrap(),
        ),
        Box::new(
            SolarLenseThirring::new(
                de440_solar_spin(&de, TEST).unwrap(),
                c,
                1.0,
                Pole::from_components(k).unwrap(),
                LtReaction::On,
            )
            .unwrap(),
        ),
    ];
    if gm_rate {
        // The inferred law (header GMSDOT per Julian year, reference epoch J2000.0 TDB), with the
        // epoch of the initial conditions (JDEPOC) as the model's time origin.
        let law = de440_inferred_solar_gm_rate(&de).unwrap();
        terms.push(Box::new(SolarGmVariation::new(law, ic.epoch())));
    }
    let model = NBodyModel::<Icrf>::new(
        &bodies,
        MoonVariables::Geocentric {
            emrat: de.earth_moon_mass_ratio(),
        },
        terms,
    )
    .unwrap();
    let description = model.describe();
    let mut p = NBodyPropagator::new(
        model,
        &states,
        ic.earth_moon(),
        ic.epoch(),
        // First trial step 0.1 d (as the WP22 diagnostic harness).
        TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap(),
        Ias15Config::default(),
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    let n = sample_count();
    let mut samples = Vec::with_capacity(n as usize);
    for kk in 1..=n {
        let e = ic.epoch()
            + TimeSpan::from_seconds(f64::from(kk) * STEP_DAYS * SECONDS_PER_DAY).unwrap();
        p.advance_to(e).unwrap();
        let s = p.state().unwrap();
        samples.push(SAMPLED.map(|id| pos(&s.state(id).unwrap())));
    }
    Run {
        samples,
        bodies: bodies.len(),
        steps: p.stats().steps,
        seconds: t0.elapsed().as_secs_f64(),
        description,
    }
}

/// Number of 10-d samples in the span (365).
fn sample_count() -> u32 {
    let n = (YEARS * DAYS_PER_JULIAN_YEAR / STEP_DAYS).round();
    assert!(n > 0.0 && n < 1.0e4); // lint: allow-literal(sanity range of the sample count)
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small positive integer, checked above"
    )]
    let n = n as u32;
    n
}

/// Per compared body: maximum heliocentric residual and the along-track residual at the end
/// (FerroAstro minus de440.bsp, projected on DE440's heliocentric velocity), in metres.
fn residuals(run: &Run, spk: &Spk<'_>, h: &Header, hprov: &str) -> [(f64, f64); 2] {
    let ic = InitialConditions::<f64>::from_header(h, hprov).unwrap();
    let mut out = [(0.0_f64, 0.0_f64); 2];
    for (kk, sample) in (1_u32..).zip(&run.samples) {
        let e = ic.epoch()
            + TimeSpan::from_seconds(f64::from(kk) * STEP_DAYS * SECONDS_PER_DAY).unwrap();
        let sun_ref = spk.state::<Barycentric, f64>(NaifId::SUN, e).unwrap();
        for (j, row) in out.iter_mut().enumerate() {
            let body_ref = spk.state::<Barycentric, f64>(SAMPLED[j + 1], e).unwrap();
            let want = sub(pos(&body_ref), pos(&sun_ref));
            let v = sub(vel(&body_ref), vel(&sun_ref));
            let d = sub(sub(sample[j + 1], sample[0]), want);
            row.0 = row.0.max(norm(d) * KM_TO_M);
            row.1 = (d[0] * v[0] + d[1] * v[1] + d[2] * v[2]) / norm(v) * KM_TO_M;
        }
    }
    out
}

/// The pre-registered checks of the opt-in inferred GM☉(t) term (issue #42, OI-04):
///
/// * option **on**: Mercury's maximum heliocentric residual against de440.bsp over 10 yr is
///   within `GMSUN-RATE-DE440-MERCURY-HELIO-MAX-M` (≤ 1.0 m) and Venus's within
///   `GMSUN-RATE-DE440-VENUS-HELIO-MAX-M` (≤ 5.0 m);
/// * option **off** (the default model): Mercury's stays in the two-sided band of
///   `GMSUN-RATE-OFF-DE440-MERCURY-HELIO-MAX-M` around the known 86 m, so the test shows the
///   effect and fails if the default model ever changes.
///
/// The bounds were committed before the library term; a failing bound is reported to the
/// maintainer, never moved. The convention is inferred, so the on-assertions are provisional on
/// OI-04.
///
/// Verifies: PHY-002, CI-007
#[test]
#[ignore = "data: reads the pinned header.440 and de440.bsp from the data cache (no network); two 353-body 10-yr runs; run with --profile data"]
fn data_inferred_solar_gm_rate_ten_years_vs_de440_bsp() {
    if De440::load_cached_or_skip(TEST).unwrap().is_none() {
        return;
    }
    if ferroastro_testdata::skip_if_absent("de440-bsp", TEST).unwrap() {
        return;
    }
    let hf = ferroastro_testdata::locate_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::locate_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    // The two runs are independent and single-threaded: run them side by side so that the test
    // stays inside the data tier's time limit.
    let (off, on) = std::thread::scope(|s| {
        let off = s.spawn(|| run(&h, &hprov, false));
        let on = s.spawn(|| run(&h, &hprov, true));
        (off.join().unwrap(), on.join().unwrap())
    });
    println!("model (option on): {}", on.description);
    assert!(on.description.contains("solar-gm-rate-inferred"));
    assert!(!off.description.contains("solar-gm-rate-inferred"));
    let (r_off, r_on) = (
        residuals(&off, &spk, &h, &hprov),
        residuals(&on, &spk, &h, &hprov),
    );
    for (name, run, r) in [("off", &off, &r_off), ("on ", &on, &r_on)] {
        println!(
            "GM_sun(t) {name}: {} bodies, {} IAS15 steps, {:.1} s; Mercury helio max {:.3} m (along at end {:+.3} m); Venus helio max {:.3} m (along at end {:+.3} m)",
            run.bodies, run.steps, run.seconds, r[0].0, r[0].1, r[1].0, r[1].1
        );
    }
    let vs = VectorSet::load_canonical().unwrap();
    let open_items = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/requirements/OPEN-ITEMS.md"),
    )
    .unwrap();
    let mut failures = Vec::new();
    for (id, measured) in [
        ("GMSUN-RATE-OFF-DE440-MERCURY-HELIO-MAX-M", r_off[0].0),
        ("GMSUN-RATE-DE440-MERCURY-HELIO-MAX-M", r_on[0].0),
        ("GMSUN-RATE-DE440-VENUS-HELIO-MAX-M", r_on[1].0),
    ] {
        let v = vs.get(id).unwrap();
        let label = v
            .provisional_open_item(&open_items)
            .unwrap()
            .map_or_else(String::new, |p| format!(" [{p}]"));
        match v.check(measured) {
            Ok(_) => println!("{id}: measured {measured:.3} m, within the bound{label}"),
            Err(e) => {
                println!("{id}: measured {measured:.3} m, OUTSIDE the bound{label}: {e}");
                failures.push(e.to_string());
            }
        }
    }
    assert!(
        failures.is_empty(),
        "pre-registered bounds missed (report the measured values, do not move the bounds): {failures:#?}"
    );
}
