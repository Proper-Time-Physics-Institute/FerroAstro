// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP25 PR tier: the NUM-007 ephemeris precision mode (`precision = "mixed"`: double-double
//! state, integrator sums and coefficients, f64 forces), the EPM2021 tiers of Subbotin et al.
//! 2023 (`major_asteroids = "epm2021"`) and the relativistic re-barycentring of their Eq. 5
//! (`initial_conditions.barycentre = "relativistic"`), on real DE440 data: header.440 from the
//! pinned JPL cache (CI-005, SHA-256 re-verified) and the committed REF-003 asteroid states.
//!
//! * C2: refusals, tiers, the shared IC hash, the model string, mixed checkpoint/resume.
//! * C3: the re-barycentring residual in double-double (`EPH-REBARY-DD-RESIDUAL-KM`, `-KMS`).
//! * C4: the mixed mode beats f64 by ≥ 10× over 2 yr (`EPH-MIXED-SHORT-RATIO`).
//! * C5: DEP-007, the N-body forces in double-double vs f64 (`EPH-FORCES-DD-F64-REL`).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::units::Velocity;
use ferroastro_eih::{BarycentreBody, relativistic_barycentre_with_rate};
use ferroastro_propagate::{
    DataSources, RunOutput, StateRecord, initial_accelerations, initial_conditions, resume, run,
    run_until,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_scenario::{IcBarycentre, MajorAsteroids, Mode, Parts, Precision, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const REFERENCE: &str = "ephemeris-mixed-epm16-40yr.toml";
const MAJOR_SYSTEM: &str = "abm-sun-planets-moon-eih-40yr.toml";
/// C4 span (Julian years; WP25 plan).
const C4_YEARS: f64 = 2.0;
const MERCURY: i32 = 1;
/// NAIF ids of (7) Iris and (324) Bamberga.
const IRIS: i32 = 2_000_007; // lint: allow-literal(NAIF id 2000000 + 7)
const BAMBERGA: i32 = 2_000_324; // lint: allow-literal(NAIF id 2000000 + 324)

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

fn small_body_file() -> ferroastro_system::SmallBodyStates {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    ferroastro_system::SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn with(base: &ScenarioSpec, precision: Precision, plain: bool) -> ScenarioSpec {
    let mut s = base.clone();
    s.integrator.precision = precision;
    if plain {
        s.integrator.compensated = Some(false);
    }
    s
}

fn dd(x: Parts) -> DoubleDouble {
    DoubleDouble::from_parts(x[0], x[1])
}

/// |a − b| of body `id`'s position, from exact parts.
fn diff(a: &StateRecord, b: &StateRecord, id: i32) -> f64 {
    let ia = a.bodies.iter().position(|&x| x == id).unwrap();
    let ib = b.bodies.iter().position(|&x| x == id).unwrap();
    (0..3)
        .map(|k| {
            let d = dd(a.positions[ia][k]) - dd(b.positions[ib][k]);
            d.to_f64().powi(2)
        })
        .sum::<f64>()
        .sqrt()
}

/// Max over the samples and the final state of |a − b| for body `id`.
fn max_diff(a: &RunOutput, b: &RunOutput, id: i32) -> f64 {
    let fa = a.final_state.clone().unwrap();
    let fb = b.final_state.clone().unwrap();
    a.states
        .iter()
        .zip(&b.states)
        .chain(std::iter::once((&fa, &fb)))
        .map(|(x, y)| {
            assert_eq!(x.t.map(f64::to_bits), y.t.map(f64::to_bits));
            diff(x, y, id)
        })
        .fold(0.0, f64::max)
}

fn code(e: &ferroastro_propagate::PropagateError) -> &'static str {
    e.code().as_str()
}

/// C2: the mixed mode and the new keys are refused where they do not apply, by name; the
/// EPM2021 tiers give 16 Major + 338 Minor; the mixed, DD and f64 twins assemble one set of
/// initial conditions (one IC hash for mixed and DD, which assemble in double-double).
///
/// Verifies: NUM-007, NUM-009, REF-011, CI-002
#[test]
fn mixed_mode_refusals_tiers_and_shared_initial_conditions() {
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);

    // Test-particle mode refuses the mixed mode by name.
    let mut tp = spec("sun-mercury-gr1pn-header440.toml");
    assert_eq!(tp.mode, Mode::TestParticle);
    tp.integrator.precision = Precision::Mixed;
    let e = run(&tp, &data).unwrap_err();
    assert_eq!(code(&e), "FA-E1201");
    assert!(e.to_string().contains("integrator.precision.mixed"), "{e}");
    // ... and re-barycentring.
    let mut tp = spec("sun-mercury-gr1pn-header440.toml");
    tp.initial_conditions.barycentre = Some(IcBarycentre::Relativistic);
    let e = run(&tp, &data).unwrap_err();
    assert!(
        e.to_string().contains("initial_conditions.barycentre"),
        "{e}"
    );

    // major_asteroids needs asteroids, and every EPM2021 asteroid among them.
    let base = spec(REFERENCE);
    let mut s = base.clone();
    s.forces.asteroids = 0;
    let e = initial_conditions(&s, &data).unwrap_err();
    assert!(e.to_string().contains("forces.major_asteroids"), "{e}");
    let mut s = base.clone();
    s.forces.asteroids = 5;
    let e = initial_conditions(&s, &data).unwrap_err();
    assert!(e.to_string().contains("(7)"), "{e}");

    // Tiers, IC hash and model string, from a one-step run of each twin.
    let mut short = base.clone();
    short.span.years = 1.0 / 365.25 / 16.0; // lint: allow-literal(one ABM step of 1/16 d)
    short.output.cadence_days = None;
    let mixed = run(&short, &data).unwrap();
    println!("mixed model: {}", mixed.model);
    println!("mixed IC source: {}", mixed.ic_source);
    assert!(
        mixed.model.contains("16 Major, 338 Minor, 0 Ring"),
        "{}",
        mixed.model
    );
    assert!(mixed.model.contains("mixed (NUM-007)"), "{}", mixed.model);
    assert!(
        mixed
            .ic_source
            .contains("5 Major: forces.major_asteroids = epm2021")
    );
    let dd_out = run(&with(&short, Precision::Dd, false), &data).unwrap();
    assert_eq!(mixed.ic_hash, dd_out.ic_hash);
    assert!(dd_out.model.contains("double-double"));
    assert!(!dd_out.model.contains("mixed"));
    let ic = initial_conditions(&short, &data).unwrap();
    assert_eq!(ic.ic_hash, mixed.ic_hash);
    assert_eq!(ic.records.len(), 354);
    let f64_out = run(&with(&short, Precision::F64, false), &data).unwrap();
    println!(
        "IC hash: mixed = dd {}, f64 {} (f64 re-barycentres in f64)",
        mixed.ic_hash, f64_out.ic_hash
    );
    // Iris and Bamberga are Major: with major_asteroids = none, the tier counts change.
    let mut none = short.clone();
    none.forces.major_asteroids = MajorAsteroids::None;
    let o = run(&none, &data).unwrap();
    assert!(o.model.contains("11 Major, 343 Minor"), "{}", o.model);
    let ids: Vec<i32> = ic.records.iter().map(|r| r.naif).collect();
    assert!(ids.contains(&IRIS) && ids.contains(&BAMBERGA));
}

/// C2: a mixed run stopped at 1 yr and resumed equals the uninterrupted 2-yr run bitwise; a
/// double-double checkpoint does not resume a mixed run.
///
/// Verifies: NUM-007, CI-010
#[test]
fn mixed_mode_checkpoint_resume_is_bitwise() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut s = with(&spec(MAJOR_SYSTEM), Precision::Mixed, false);
    s.span.years = C4_YEARS;
    let full = run(&s, &data).unwrap();
    let (first, ck) = run_until(&s, &data, C4_YEARS / 2.0).unwrap();
    let ck = ck.unwrap();
    assert_eq!(ck.precision, "mixed");
    let mut joined = first;
    joined.extend_with(resume(&s, &data, &ck).unwrap()).unwrap();
    assert_eq!(joined.states, full.states);
    assert_eq!(joined.final_state, full.final_state);
    let (_, ck_dd) = run_until(&with(&s, Precision::Dd, false), &data, C4_YEARS / 2.0).unwrap();
    let e = resume(&s, &data, &ck_dd.unwrap()).unwrap_err();
    assert_eq!(code(&e), "FA-E1302");
}

/// C3: after three passes of the relativistic re-barycentring in double-double, the barycentre
/// of the full EPM2021-tier body set and its Eq. 5 velocity vanish to double-double round-off.
///
/// Verifies: NUM-018
#[test]
fn relativistic_rebarycentring_residual_in_double_double() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let mut s = with(&spec(REFERENCE), Precision::Dd, false);
    s.span.years = 1.0 / 365.25 / 16.0; // lint: allow-literal(one ABM step of 1/16 d)
    let ic = initial_conditions(&s, &data).unwrap();
    let out = run(&s, &data).unwrap();
    let r0 = &out.states[0];
    assert_eq!(r0.t.map(f64::to_bits), [0, 0]);
    assert_eq!(
        r0.bodies,
        ic.records.iter().map(|r| r.naif).collect::<Vec<_>>()
    );
    let bodies: Vec<BarycentreBody<DoubleDouble>> = ic
        .records
        .iter()
        .zip(r0.positions.iter().zip(&r0.velocities))
        .map(|(rec, (p, w))| BarycentreBody {
            gm: dd(rec.gm),
            position: p.map(dd),
            velocity: w.map(dd),
        })
        .collect();
    let de = De440::load().unwrap();
    let c =
        Velocity::from_km_per_s(DoubleDouble::from_f64(de.speed_of_light().km_per_s())).unwrap();
    let (b, w) = relativistic_barycentre_with_rate(&bodies, c).unwrap();
    let norm = |a: [DoubleDouble; 3]| {
        let [x, y, z] = a.map(DoubleDouble::to_f64);
        x.hypot(y).hypot(z)
    };
    let (rb, rw) = (norm(b), norm(w));
    println!(
        "C3: {} bodies; |b| = {rb:.3e} km, |b'| (Eq. 5) = {rw:.3e} km/s after re-barycentring in DD",
        bodies.len()
    );
    println!("  IC source: {}", out.ic_source);
    // Without re-barycentring the same body set is offset by ~1e2 km (reported; WP11/WP15).
    let mut raw = s.clone();
    raw.initial_conditions.barycentre = None;
    let o = run(&raw, &data).unwrap();
    let bodies_raw: Vec<BarycentreBody<DoubleDouble>> = ic
        .records
        .iter()
        .zip(o.states[0].positions.iter().zip(&o.states[0].velocities))
        .map(|(rec, (p, w))| BarycentreBody {
            gm: dd(rec.gm),
            position: p.map(dd),
            velocity: w.map(dd),
        })
        .collect();
    let (b0, w0) = relativistic_barycentre_with_rate(&bodies_raw, c).unwrap();
    println!(
        "  as given (header.440 + REF-003, not re-barycentred): |b| = {:.6} km, |b'| = {:.3e} km/s",
        norm(b0),
        norm(w0)
    );
    vs.get("EPH-REBARY-DD-RESIDUAL-KM")
        .unwrap()
        .check(rb)
        .unwrap();
    vs.get("EPH-REBARY-DD-RESIDUAL-KMS")
        .unwrap()
        .check(rw)
        .unwrap();
}

/// C4: over 2 yr of the WP24 major-body ABM scenario, the mixed mode's Mercury round-off (vs a
/// full double-double run) is at least 10× smaller than plain f64's.
///
/// Verifies: NUM-007, DEP-007
#[test]
fn mixed_mode_round_off_is_far_below_f64() {
    let vs = VectorSet::load_canonical().unwrap();
    let want = vs.get("EPH-MIXED-SHORT-RATIO").unwrap().value().unwrap();
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut base = spec(MAJOR_SYSTEM);
    base.span.years = C4_YEARS;
    let truth = run(&with(&base, Precision::Dd, false), &data).unwrap();
    let mixed = run(&with(&base, Precision::Mixed, false), &data).unwrap();
    let plain = run(&with(&base, Precision::F64, true), &data).unwrap();
    let comp = run(&with(&base, Precision::F64, false), &data).unwrap();
    assert_eq!(mixed.ic_hash, truth.ic_hash);
    let m = max_diff(&mixed, &truth, MERCURY);
    let f = max_diff(&plain, &truth, MERCURY);
    let fc = max_diff(&comp, &truth, MERCURY);
    let ratio = f / m;
    println!(
        "C4 ({C4_YEARS} yr, Mercury max |dr| vs DD): mixed {m:.3e} km, f64 plain {f:.3e} km, \
         f64 compensated {fc:.3e} km; ratio f64 plain / mixed = {ratio:.1} (bound >= {want})"
    );
    assert!(
        m > 0.0,
        "the mixed run equals the DD run bitwise: forces are not f64"
    );
    assert!(ratio >= want, "ratio {ratio} < {want}");
}

/// C5 (DEP-007): the N-body force model (Newtonian mutual + geocentric Moon + EIH + solar J2 and
/// Lense–Thirring with reaction, Sun … Pluto from header.440) evaluated in double-double and in
/// f64 at the initial conditions agree to f64 round-off; the mixed mode evaluates the f64 model.
///
/// Verifies: DEP-007, NUM-007
#[test]
fn nbody_forces_agree_in_double_double_and_f64() {
    let vs = VectorSet::load_canonical().unwrap();
    let tol = vs.get("EPH-FORCES-DD-F64-REL").unwrap();
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut s = spec(MAJOR_SYSTEM);
    s.bodies.push(ferroastro_scenario::BodyName::Pluto);
    s.forces.sun_j2 = true;
    s.forces.lense_thirring = true;
    let (ids, a_dd) = initial_accelerations(&with(&s, Precision::Dd, false), &data).unwrap();
    let (ids64, a64) = initial_accelerations(&with(&s, Precision::F64, false), &data).unwrap();
    let (_, amix) = initial_accelerations(&with(&s, Precision::Mixed, false), &data).unwrap();
    assert_eq!(ids, ids64);
    let mut worst = 0.0_f64;
    let mut worst_mix = 0.0_f64;
    for (i, id) in ids.iter().enumerate() {
        let d: [DoubleDouble; 3] = a_dd[i].map(dd);
        let n = d.map(|z| z.to_f64().powi(2)).iter().sum::<f64>().sqrt();
        let e = (0..3)
            .map(|k| (d[k] - dd(a64[i][k])).to_f64().powi(2))
            .sum::<f64>()
            .sqrt();
        let em = (0..3)
            .map(|k| (d[k] - dd(amix[i][k])).to_f64().powi(2))
            .sum::<f64>()
            .sqrt();
        // The mixed mode's accelerations are f64 values (no low word).
        assert!(amix[i].iter().all(|z| z[1] == 0.0));
        println!(
            "  {id:>4}: |a| = {n:.6e} km/s^2, |a_dd - a_f64|/|a| = {:.2e}, |a_dd - a_mixed|/|a| = {:.2e}",
            e / n,
            em / n
        );
        worst = worst.max(e / n);
        worst_mix = worst_mix.max(em / n);
    }
    println!(
        "C5: max relative |a_dd - a_f64| = {worst:.3e}, |a_dd - a_mixed| = {worst_mix:.3e} \
         (bound {})",
        tol.tolerance.allowed(0.0)
    );
    tol.check(worst).unwrap();
    tol.check(worst_mix).unwrap();
}
