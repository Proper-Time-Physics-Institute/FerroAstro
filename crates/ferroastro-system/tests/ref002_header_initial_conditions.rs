// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! REF-002 / OI-23 / NUM-009 / NUM-018: initial conditions and the catalogue from the real
//! header.440, checked against the independent de440.bsp (both from the PR-fast JPL cache,
//! CI-005, re-verified against their pinned SHA-256).
//!
//! The header's state block is read under [`HEADER_STATE_INTERPRETATION`]; check 1 of WP11 shows
//! it reproduces de440.bsp at JDEPOC and that the alternative readings (heliocentric; km, km/s)
//! do not. The output is the evidence attached to OI-23 for maintainer closure.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure with a clear message; counts are small"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Center, Icrf, State};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::GravParam;
use ferroastro_ephem::{NaifId, Spk};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_system::{
    Catalogue, DE440_MAJOR, HEADER_STATE_INTERPRETATION, InitialConditions, MassModel, Tier,
    barycentre, rebarycentre,
};
use ferroastro_vectors::VectorSet;

fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

fn cached(name: &str) -> (Vec<u8>, String) {
    let f = ferroastro_testdata::locate_product(name).unwrap_or_else(|e| panic!("{e}"));
    let prov = f.provenance();
    println!("{name}: {prov}");
    (std::fs::read(&f.path).unwrap(), prov)
}

fn header() -> (Header, String) {
    let (bytes, prov) = cached("de440-header");
    (
        Header::parse(&String::from_utf8(bytes).unwrap()).unwrap(),
        prov,
    )
}

#[derive(Default)]
struct Max {
    value: f64,
    at: String,
}

impl Max {
    fn add(&mut self, x: f64, at: impl Fn() -> String) {
        assert!(x.is_finite(), "non-finite difference at {}", at());
        if x > self.value {
            self.value = x;
            self.at = at();
        }
    }
}

fn check(v: &VectorSet, id: &str, m: &Max) {
    let vector = v.get(id).unwrap();
    let allowed = vector.tolerance.allowed(vector.value().unwrap());
    let margin = if allowed > 0.0 {
        format!("{:.3}", m.value / allowed)
    } else if m.value == 0.0 {
        "exact".to_owned()
    } else {
        "inf".to_owned()
    };
    println!(
        "{id:<44} max |diff| {:.3e} {} (margin {margin}; at {})",
        m.value, vector.unit, m.at
    );
    if let Err(e) = vector.check(m.value) {
        panic!("{e}");
    }
}

/// Position (km) and velocity (km/s) components of a typed state, in f64.
fn raw<C: Center, R: Real>(s: &State<Icrf, C, R>) -> ([f64; 3], [f64; 3]) {
    (
        s.position().components().map(|x| x.km().to_f64()),
        s.velocity().map(|x| x.km_per_s().to_f64()),
    )
}

fn max_abs_diff(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

/// Largest component differences between two typed states, computed in `R` before rounding.
fn diff<C: Center, R: Real>(a: &State<Icrf, C, R>, b: &State<Icrf, C, R>) -> (f64, f64) {
    let dp = a
        .position()
        .components()
        .iter()
        .zip(&b.position().components())
        .map(|(x, y)| (x.km() - y.km()).to_f64().abs())
        .fold(0.0, f64::max);
    let dv = a
        .velocity()
        .iter()
        .zip(&b.velocity())
        .map(|(x, y)| (x.km_per_s() - y.km_per_s()).to_f64().abs())
        .fold(0.0, f64::max);
    (dp, dv)
}

fn jdepoc_tdb<R: Real>(h: &Header) -> Epoch<Tdb, R> {
    Epoch::from_jd_parts(R::from_f64(h.value("JDEPOC").unwrap()), R::ZERO).unwrap()
}

fn ids() -> Vec<NaifId> {
    DE440_MAJOR.iter().map(|&(id, _)| id).collect()
}

fn run_check1<R: Real>(label: &str, h: &Header, hprov: &str, spk: &Spk<'_>) -> (Max, Max) {
    let from_header = InitialConditions::<R>::from_header(h, hprov).unwrap();
    let epoch = jdepoc_tdb::<R>(h);
    let from_spk = InitialConditions::<R>::from_ephemeris(spk, epoch, &ids(), "de440.bsp").unwrap();
    let (mut pos, mut vel) = (Max::default(), Max::default());
    println!("check 1 ({label}): header.440 IC minus de440.bsp at JDEPOC, barycentric ICRF");
    for &(id, name) in &DE440_MAJOR {
        let (dp, dv) = diff(
            &from_header.states().state(id).unwrap(),
            &from_spk.states().state(id).unwrap(),
        );
        println!(
            "  {name:<16} NAIF {:>3}: |dr| {dp:.3e} km, |dv| {dv:.3e} km/s",
            id.0
        );
        pos.add(dp, || format!("{label} {name}"));
        vel.add(dv, || format!("{label} {name}"));
    }
    // The DE integration variables: EMB and the geocentric Moon, compared directly.
    let hp = from_header.earth_moon().unwrap();
    let sp = from_spk.earth_moon().unwrap();
    let (dp, dv) = diff(&hp.emb, &sp.emb);
    println!(
        "  {:<16} NAIF   3: |dr| {dp:.3e} km, |dv| {dv:.3e} km/s",
        "Earth-Moon bary"
    );
    pos.add(dp, || format!("{label} EMB"));
    vel.add(dv, || format!("{label} EMB"));
    let (dp, dv) = diff(&hp.moon, &sp.moon);
    println!(
        "  {:<16} 301-399: |dr| {dp:.3e} km, |dv| {dv:.3e} km/s",
        "geocentric Moon"
    );
    pos.add(dp, || format!("{label} geocentric Moon"));
    vel.add(dv, || format!("{label} geocentric Moon"));
    (pos, vel)
}

/// WP11 check 1 (OI-23 evidence): the header.440 initial conditions, read as SSB-centred ICRF
/// states in au and au/day, equal de440.bsp at JDEPOC for every body, in f64 and in
/// double-double; the heliocentric reading and the km/km-s reading do not.
///
/// Verifies: REF-002, NUM-009
#[test]
fn ref002_header_initial_conditions_equal_de440_at_jdepoc() {
    let v = vectors();
    let (h, hprov) = header();
    let (bsp, _) = cached("de440-bsp");
    let spk = Spk::parse(&bsp).unwrap();
    println!("interpretation under test: {HEADER_STATE_INTERPRETATION}");
    println!(
        "header CENTER token {:?}, JDEPOC {}",
        h.token("CENTER").unwrap(),
        h.token("JDEPOC").unwrap()
    );
    let (pos, vel) = run_check1::<f64>("f64", &h, &hprov, &spk);
    check(&v, "SYSTEM-HEADER-VS-DE440-POS-MAX-DIFF", &pos);
    check(&v, "SYSTEM-HEADER-VS-DE440-VEL-MAX-DIFF", &vel);
    let (pos, vel) = run_check1::<DoubleDouble>("double-double", &h, &hprov, &spk);
    check(&v, "SYSTEM-HEADER-VS-DE440-POS-MAX-DIFF", &pos);
    check(&v, "SYSTEM-HEADER-VS-DE440-VEL-MAX-DIFF", &vel);

    // Alternative readings must miss by far more than the tolerance.
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let epoch = jdepoc_tdb::<f64>(&h);
    let spk_ic =
        InitialConditions::<f64>::from_ephemeris(&spk, epoch, &ids(), "de440.bsp").unwrap();
    let helio_spk = spk_ic.states().heliocentric().unwrap();
    let au = h.value("AU").unwrap();
    let floor = v
        .get("SYSTEM-HEADER-ALT-READING-MIN-DIFF")
        .unwrap()
        .value()
        .unwrap();
    let (mut helio_min, mut km_min) = (f64::INFINITY, f64::INFINITY);
    for &(id, name) in &DE440_MAJOR {
        if id == NaifId::SUN {
            continue;
        }
        let (hp, _) = raw(&ic.states().state(id).unwrap());
        // Heliocentric reading: the header vector taken as relative to the Sun.
        let (sp_helio, _) = raw(&helio_spk.state(id).unwrap());
        let dp = max_abs_diff(hp, sp_helio);
        // km reading: the header numbers taken as km (i.e. our km values divided by AU).
        let (sp, _) = raw(&spk_ic.states().state(id).unwrap());
        let km_dp = max_abs_diff(hp.map(|x| x / au), sp);
        println!(
            "  alternative readings, {name:<16}: heliocentric |dr| {dp:.3e} km, km-units |dr| {km_dp:.3e} km"
        );
        helio_min = helio_min.min(dp);
        km_min = km_min.min(km_dp);
    }
    println!(
        "alternative readings: smallest miss heliocentric {helio_min:.3e} km, km units {km_min:.3e} km \
         (each must exceed {floor} km = 1e3 x the position tolerance)"
    );
    assert!(helio_min > floor && km_min > floor);
    println!(
        "OI-23 evidence: CENTER = 0 reading confirmed as SSB-centred ICRF, au and au/day, at JDEPOC; \
         XM = Moon - Earth; XB = EMB. Closure is a maintainer decision."
    );
}

/// WP11 check 2: the Earth and Moon GMs split from GMB with EMRAT recombine to GMB within 2 ulp.
///
/// Verifies: REF-002, PHY-002
#[test]
fn emrat_split_recombines_to_gmb_at_the_ulp_level() {
    let v = vectors();
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let gmb = de.gm(ferroastro_constants::Body::EarthMoon).km3_per_s2();
    let sum =
        cat.gm(NaifId::EARTH).unwrap().km3_per_s2() + cat.gm(NaifId::MOON).unwrap().km3_per_s2();
    let ulp = f64::from_bits(gmb.to_bits() + 1) - gmb;
    let m = Max {
        value: (sum - gmb).abs() / ulp,
        at: format!("GM_E + GM_M = {sum:.17e}, GMB = {gmb:.17e} km^3/s^2"),
    };
    check(&v, "SYSTEM-EMRAT-GMB-RECOMBINE-ULP", &m);
}

/// WP11 check 3 and the asteroid counts: the catalogue built from the header at run time equals
/// the committed table bit for bit on the 11 major bodies; it holds the numbered asteroids as
/// Minor bodies and keeps the unidentified MA entries apart.
///
/// Verifies: REF-002, NUM-009
#[test]
fn header_catalogue_equals_the_committed_table_and_tiers_the_asteroids() {
    let v = vectors();
    let (h, hprov) = header();
    let de = De440::load().unwrap();
    let table = Catalogue::de440_major(&de);
    let cat = Catalogue::from_header(&h, &hprov).unwrap();
    let mut m = Max::default();
    for b in table.bodies() {
        let g: GravParam = cat.gm(b.id()).unwrap();
        m.add((g.km3_per_s2() - b.gm().km3_per_s2()).abs(), || {
            b.name().to_owned()
        });
        assert_eq!(cat.get(b.id()).unwrap().tier(), Tier::Major);
    }
    check(&v, "SYSTEM-HEADER-CATALOGUE-VS-TABLE-GM-DIFF", &m);
    // NAIF numbering of asteroids: 2 000 000 + the asteroid number of the header symbol MAnnnn.
    for b in cat.bodies().iter().filter(|b| b.tier() == Tier::Minor) {
        let number: i32 = b.gm_source().trim_start_matches("MA").parse().unwrap();
        assert_eq!(
            b.id().0,
            ferroastro_system::ASTEROID_NAIF_BASE + number,
            "{}",
            b.gm_source()
        );
        assert_eq!(b.name(), format!("({number})"));
    }
    let ceres = cat
        .get(NaifId(ferroastro_system::ASTEROID_NAIF_BASE + 1))
        .unwrap();
    assert_eq!(ceres.gm_source(), "MA0001");
    let minor = cat.ids_in(Tier::Minor).count();
    let unidentified = cat.unidentified_masses().len();
    let gms = de.gm_sun().km3_per_s2();
    let sum = |it: &mut dyn Iterator<Item = f64>| it.sum::<f64>() / gms;
    println!(
        "catalogue: {} bodies ({} Major, {minor} Minor); {unidentified} unidentified MA entries \
         ({} .. {}); sum GM/GMS: numbered asteroids {:.6e}, unidentified {:.6e}; constants: {}",
        cat.bodies().len(),
        cat.ids_in(Tier::Major).count(),
        cat.unidentified_masses().first().unwrap().symbol,
        cat.unidentified_masses().last().unwrap().symbol,
        sum(&mut cat
            .bodies()
            .iter()
            .filter(|b| b.tier() == Tier::Minor)
            .map(|b| b.gm().km3_per_s2())),
        sum(&mut cat.unidentified_masses().iter().map(|u| u.gm.km3_per_s2())),
        cat.constants()
    );
    let count = |id: &str, n: usize| {
        println!("{id}: {n}");
        v.get(id)
            .unwrap()
            .check(n as f64)
            .unwrap_or_else(|e| panic!("{e}"));
    };
    count("SYSTEM-HEADER-NUMBERED-ASTEROIDS", minor);
    count("SYSTEM-HEADER-UNIDENTIFIED-MA", unidentified);
    count("DE440-MA-COUNT", minor + unidentified);
}

/// WP11 checks 4 and 8 (without asteroids): the barycentre of the header bodies is reported
/// under both mass models (no assertion: the plan defers it to WP15, when Minor states exist);
/// rebarycentring then removes it to round-off under the relativistic model.
///
/// Verifies: NUM-018, REF-002
#[test]
fn header_barycentre_is_reported_and_rebarycentring_removes_it() {
    let v = vectors();
    let (h, hprov) = header();
    let de = De440::load().unwrap();
    let cat = Catalogue::from_header(&h, &hprov).unwrap();
    let c = de.speed_of_light();
    let mut ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    for model in [MassModel::Newtonian, MassModel::RelativisticDe440] {
        let b = barycentre(&cat, ic.states(), model, c).unwrap();
        println!(
            "REPORT (no assertion until WP15) header bodies only ({} bodies, no asteroids), {}: \
             barycentre offset {:.6} km, {:.6e} km/s; position {:?} km",
            b.bodies,
            model.label(),
            b.distance_km(),
            b.speed_km_s(),
            b.position_km
        );
    }
    let w_n = ferroastro_system::mass_weights(&cat, ic.states(), MassModel::Newtonian, c).unwrap();
    let w_r = ferroastro_system::mass_weights(&cat, ic.states(), MassModel::RelativisticDe440, c)
        .unwrap();
    for ((id, wn), wr) in ic.states().ids().iter().zip(&w_n).zip(&w_r) {
        println!("  mu*/mu - 1 for NAIF {:>3}: {:+.6e}", id.0, wr / wn - 1.0);
    }
    let after = rebarycentre(&cat, ic.states_mut(), MassModel::RelativisticDe440, c, 2).unwrap();
    let pos = Max {
        value: after.distance_km(),
        at: "after 2 passes".into(),
    };
    let vel = Max {
        value: after.speed_km_s(),
        at: "after 2 passes".into(),
    };
    check(&v, "SYSTEM-REBARYCENTRE-POS-RESIDUAL", &pos);
    check(&v, "SYSTEM-REBARYCENTRE-VEL-RESIDUAL", &vel);
}

/// The geocentric Moon from the header pair equals Moon − Earth of the split states to round-off
/// of a geocentric difference of barycentric coordinates (a self-consistency check of the split).
///
/// Verifies: NUM-008, NUM-009
#[test]
fn geocentric_moon_round_trips_through_the_split() {
    let (h, hprov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let pair = *ic.earth_moon().unwrap();
    let e = ic.states().state(NaifId::EARTH).unwrap();
    let m = ic.states().state(NaifId::MOON).unwrap();
    let back = ferroastro_system::EarthMoon::from_bodies(&e, &m, h.value("EMRAT").unwrap());
    let (dp, dv) = diff(&back.moon, &pair.moon);
    let r_emb = pair
        .emb
        .position()
        .components()
        .iter()
        .map(|x| x.km().abs())
        .fold(0.0, f64::max);
    println!(
        "geocentric Moon after split + recombine: |dr| {dp:.3e} km, |dv| {dv:.3e} km/s (EMB |r| ~ {r_emb:.3e} km)"
    );
    // Round-off of a difference of two ~1.5e8 km coordinates: a few ulp of 1.5e8 km.
    assert!(dp <= 8.0 * f64::EPSILON * r_emb, "{dp}");
}

/// μ* sign and size (NUM-018): for a Keplerian orbit v² − GM/r = GM(1/r − 1/a), so
/// |v² − U| ≤ e·U < U, and the relativistic mass correction μ*/μ − 1 = (v² − U)/(2c²) must be
/// smaller in magnitude than U/(2c²) for every planet (U = Σⱼ μⱼ/rᵢⱼ, computed here
/// independently). A sign error in the potential term gives (v² + U)/(2c²) > U/(2c²).
///
/// The value itself is also pinned to (vᵢ² − Uᵢ)/(2c²) recomputed from the raw states for every
/// body, so a missing correction or a 1/c² factor fails (the bound alone does not catch them).
///
/// Verifies: NUM-018
#[test]
fn relativistic_mass_correction_is_bounded_by_the_virial_cancellation() {
    let (h, hprov) = header();
    let de = De440::load().unwrap();
    let c = de.speed_of_light().km_per_s();
    let cat = Catalogue::from_header(&h, &hprov).unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let s = ic.states();
    let w_n = ferroastro_system::mass_weights(&cat, s, MassModel::Newtonian, de.speed_of_light())
        .unwrap();
    let w_r =
        ferroastro_system::mass_weights(&cat, s, MassModel::RelativisticDe440, de.speed_of_light())
            .unwrap();
    let raws: Vec<([f64; 3], [f64; 3])> = s
        .ids()
        .iter()
        .map(|&id| raw(&s.state(id).unwrap()))
        .collect();
    for (i, &id) in s.ids().iter().enumerate() {
        let (ri, vi) = raws[i];
        let u: f64 = raws
            .iter()
            .zip(&w_n)
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, ((rj, _), mu))| {
                let d = [rj[0] - ri[0], rj[1] - ri[1], rj[2] - ri[2]];
                mu / d[0].hypot(d[1]).hypot(d[2])
            })
            .sum();
        let corr = w_r[i] / w_n[i] - 1.0;
        let bound = u / (2.0 * c * c);
        // Magnitude pin (review of WP11): the library value must equal (v^2 - U)/(2c^2) recomputed
        // here from the raw states. Without it a relativistic model that silently returned the
        // Newtonian weights, or used 1/c^2, passed this test. The budget is the round-off of
        // w_r/w_n - 1 (a few ulp of 1).
        let v2 = vi[0].mul_add(vi[0], vi[1].mul_add(vi[1], vi[2] * vi[2]));
        let expected = (v2 - u) / (2.0 * c * c);
        println!(
            "NAIF {:>3}: mu*/mu - 1 = {corr:+.6e}, (v^2-U)/(2c^2) = {expected:+.6e}, U/(2c^2) = {bound:.6e}",
            id.0
        );
        assert!(
            (corr - expected).abs() <= 8.0 * f64::EPSILON, // lint: allow-literal(round-off budget of a ratio minus one)
            "NAIF {}: mu*/mu - 1 = {corr} but (v^2 - U)/(2c^2) = {expected}",
            id.0
        );
        assert!(expected != 0.0, "NAIF {}: no relativistic correction", id.0);
        if id == NaifId::SUN || id == NaifId::MOON || id == NaifId::EARTH {
            // The Sun is not on a heliocentric Kepler orbit; Earth and Moon are a close pair.
            continue;
        }
        assert!(corr.abs() < bound, "NAIF {}: |{corr}| >= {bound}", id.0);
    }
}
