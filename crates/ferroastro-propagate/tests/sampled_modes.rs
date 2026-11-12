// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP17 run modes for the oracle comparisons (PR tier, header.440 from the pinned cache):
//!
//! * test-particle scenarios with `output.cadence_days` (the C-lite series, CI-009): states on an
//!   exact grid, the same IC hash as the passage-mode run of the same orbit, refusals for
//!   double-double and for perihelion output combined with a cadence;
//! * ephemeris-field mode (stage B, REF-006): what the runner refuses (REF-011) and what the
//!   schema rejects; the run itself is compared with Orekit in `ferroastro-oracle`'s stage B test.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_constants::header440::Header;
use ferroastro_core::ErrorCode;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::{IcSource, Precision, ScenarioSpec};
use std::path::PathBuf;

const SHORT_YEARS: f64 = 1.0; // lint: allow-literal(run length, PR-tier cost)

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A sampled test-particle run: one record per whole cadence interval plus the initial state, on
/// the exact grid `k · cadence`, central body at the origin, the initial record equal to the IC;
/// the IC hash is the passage-mode run's (identical ICs, CI-009).
///
/// Verifies: CI-009, REF-007
#[test]
fn test_particle_cadence_samples_lie_on_the_exact_grid() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut s = spec("oracle-clite-sun-mercury-tp-gr1pn-252yr.toml");
    s.span.years = SHORT_YEARS;
    let out = run(&s, &data).unwrap();
    let cadence = s.output.cadence_days.unwrap();
    let span_days = SHORT_YEARS * ferroastro_scenario::DAYS_PER_JULIAN_YEAR;
    let mut k: u32 = 0;
    for rec in &out.states {
        assert_eq!(
            rec.t[0].to_bits(),
            (f64::from(k) * cadence * SECONDS_PER_DAY).to_bits()
        );
        k += 1;
        assert_eq!(rec.bodies, vec![10, 1]);
        assert!(rec.positions[0].iter().all(|p| p[0] == 0.0 && p[1] == 0.0));
    }
    // Every whole cadence interval of the span, and no more.
    assert!(f64::from(k - 1) * cadence <= span_days);
    assert!(f64::from(k) * cadence > span_days);
    assert!(out.passages.is_empty());

    // Same orbit in passage mode: the same IC hash and the same initial state.
    let mut p = spec("sun-mercury-gr1pn-header440.toml");
    p.span.years = SHORT_YEARS;
    p.span.stop_after_passages = None;
    let q = run(&p, &data).unwrap();
    assert_eq!(out.ic_hash, q.ic_hash);
    assert_eq!(out.states[0].positions[1], q.states[0].positions[0]);
    assert_eq!(out.states[0].velocities[1], q.states[0].velocities[0]);
    println!(
        "{} samples; IC hash {} equals the passage-mode run's",
        out.states.len(),
        out.ic_hash
    );
}

/// Test-particle cadence output is refused in double-double and together with perihelion output.
///
/// Verifies: REF-011
#[test]
fn test_particle_cadence_refusals_name_the_option() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut s = spec("oracle-clite-sun-mercury-tp-newton-252yr.toml");
    s.span.years = SHORT_YEARS;
    s.integrator.precision = Precision::Dd;
    let e = run(&s, &data).unwrap_err();
    assert_eq!(e.code(), ErrorCode::ScenarioUnsupported);
    assert!(e.to_string().contains("double-double"), "{e}");
    s.integrator.precision = Precision::F64;
    s.output.perihelion = vec![ferroastro_scenario::BodyName::Mercury];
    let e = run(&s, &data).unwrap_err();
    assert!(e.to_string().contains("perihelion"), "{e}");
}

/// B4 (PR #13 review): an output grid past `MAX_OUTPUT_SAMPLES` (a huge span or a tiny cadence)
/// is refused by `ScenarioSpec::validate` and by `run` with `FA-E1203`, at once, for every mode
/// that samples; a grid at the cap validates. Neither the refusal nor the check counts the grid.
///
/// Verifies: API-002, CI-009
#[test]
fn output_grid_past_the_cap_is_refused_without_panic_or_hang() {
    use ferroastro_scenario::{DAYS_PER_JULIAN_YEAR, MAX_OUTPUT_SAMPLES};
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let cap = f64::from(MAX_OUTPUT_SAMPLES);
    let t0 = std::time::Instant::now();
    for file in [
        "oracle-clite-sun-mercury-tp-newton-252yr.toml",
        "stage-a-sun-8planets-newton-100yr.toml",
    ] {
        let base = spec(file);
        let cadence = base.output.cadence_days.unwrap();
        // At the cap: (cap - 1) whole intervals, i.e. MAX_OUTPUT_SAMPLES samples, validate.
        let mut ok = base.clone();
        ok.span.years = (cap - 1.0) * cadence / DAYS_PER_JULIAN_YEAR;
        ok.validate().unwrap();
        // Huge spans (2^32 intervals and more) and tiny cadences, forwards and backwards.
        let mut huge = base.clone();
        huge.span.years = cap * cadence / DAYS_PER_JULIAN_YEAR;
        let mut wraps = base.clone();
        wraps.span.years = -f64::from(u32::MAX) * 2.0 * cadence / DAYS_PER_JULIAN_YEAR;
        let mut tiny = base.clone();
        tiny.output.cadence_days = Some(f64::MIN_POSITIVE);
        let mut subnormal = base.clone();
        subnormal.output.cadence_days = Some(f64::MIN_POSITIVE / 4.0);
        let mut far = base.clone();
        far.span.years = f64::MAX;
        for s in [huge, wraps, tiny, subnormal, far] {
            let e = s.validate().unwrap_err();
            assert_eq!(e.code(), ErrorCode::ScenarioValue, "{file}: {e}");
            assert!(e.to_string().contains("MAX_OUTPUT_SAMPLES"), "{e}");
            let e = run(&s, &data).unwrap_err();
            assert_eq!(e.code(), ErrorCode::ScenarioValue, "{file}: {e}");
        }
    }
    assert!(
        t0.elapsed().as_secs_f64() < 5.0, // lint: allow-literal(wall-clock bound: refusals must not count the grid)
        "refusing a huge grid must not count it"
    );
}

/// Ephemeris-field mode: only Newtonian f64 from de440-bsp; J2 and double-double are refused by
/// name; a header IC source is a schema error.
///
/// Verifies: REF-011, REF-006, CI-002
#[test]
fn ephemeris_field_mode_refuses_what_it_does_not_implement() {
    let base = spec("oracle-stage-b-mercury-de440-field-100yr.toml");
    let mut s = base.clone();
    s.forces.sun_j2 = true;
    let e = run(&s, &DataSources::none()).unwrap_err();
    assert_eq!(e.code(), ErrorCode::ScenarioUnsupported);
    assert!(e.to_string().contains("forces.sun_j2"), "{e}");
    let mut s = base.clone();
    s.integrator.precision = Precision::Dd;
    let e = run(&s, &DataSources::none()).unwrap_err();
    assert!(e.to_string().contains("integrator.precision.dd"), "{e}");
    let mut s = base.clone();
    s.initial_conditions.source = IcSource::De440Header;
    s.initial_conditions.epoch_jd_tdb = None;
    let e = s.validate().unwrap_err();
    assert!(e.to_string().contains("initial_conditions.source"), "{e}");
    let e = run(&base, &DataSources::none()).unwrap_err();
    assert!(e.to_string().contains("SPK"), "{e}");
}
