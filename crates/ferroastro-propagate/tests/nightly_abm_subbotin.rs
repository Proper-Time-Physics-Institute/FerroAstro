// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP24 check B3 (nightly, reported; the assertion is WP25's): the Subbotin, Kodukov & Pavlov
//! 2023 configuration (40-yr EIH, ABM13 PECEC, h = 1/16 d, DP8 start-up at h/8) from the
//! header.440 initial conditions, in f64 (plain sums and compensated update) against a full
//! double-double run of the same scenario as truth. IAS15 in f64 and in double-double is run
//! alongside, so that the ABM−IAS15 difference of B2 can be attributed:
//!
//! * ABM(DD) − IAS15(DD): the two methods' truncation (and DD round-off);
//! * ABM(f64) − ABM(DD), IAS15(f64) − IAS15(DD): each integrator's f64 round-off.
//!
//! Systems: (a) the major-body scenario `abm-sun-planets-moon-eih-40yr`; (b) the same plus the
//! 343 DE440 Minor bodies (REF-003 states; Newtonian from the Majors, NUM-009), f64 and DD.
//! Subbotin et al. 2023 Table 1 (16 Major + 343 Minor, their hardware): Mercury f64 2.47 m,
//! mixed DD 8.4 mm.
//! Run: `cargo nextest run -p ferroastro-propagate --cargo-profile release --profile nightly
//! --run-ignored only`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, RunOutput, StateRecord, run};
use ferroastro_scenario::{BodyName, IntegratorName, Parts, Precision, ScenarioSpec};
use std::path::PathBuf;
use std::time::Instant;

/// Number of DE440 Minor bodies (header.440 numbered asteroids; Subbotin's 343).
const MINOR: u32 = 343; // lint: allow-literal(count of DE440 Minor bodies, REF-003)
const MERCURY: i32 = 1;
const MARS: i32 = 4;
const EARTH: i32 = 399;
const MOON: i32 = 301;

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
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

fn small_body_file() -> ferroastro_system::SmallBodyStates {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    ferroastro_system::SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn p(x: Parts) -> f64 {
    x[0] + x[1]
}

/// Position difference `a − b` of body `id` in exact parts (so DD records keep their low word).
fn diff(a: &StateRecord, b: &StateRecord, id: i32) -> f64 {
    let ia = a.bodies.iter().position(|&x| x == id).unwrap();
    let ib = b.bodies.iter().position(|&x| x == id).unwrap();
    (0..3)
        .map(|k| {
            let (x, y) = (a.positions[ia][k], b.positions[ib][k]);
            let d = (x[0] - y[0]) + (x[1] - y[1]);
            d * d
        })
        .sum::<f64>()
        .sqrt()
}

fn geo_diff(a: &StateRecord, b: &StateRecord) -> f64 {
    let g = |r: &StateRecord| {
        let i = |id: i32| r.bodies.iter().position(|&x| x == id).unwrap();
        let (m, e) = (i(MOON), i(EARTH));
        (0..3)
            .map(|k| {
                let (mm, ee) = (r.positions[m][k], r.positions[e][k]);
                (mm[0] - ee[0]) + (mm[1] - ee[1])
            })
            .collect::<Vec<f64>>()
    };
    let (x, y) = (g(a), g(b));
    (0..3).map(|k| (x[k] - y[k]).powi(2)).sum::<f64>().sqrt()
}

/// Max over samples and the final state of |a − b| for Mercury, Mars and the geocentric Moon.
fn row(a: &RunOutput, b: &RunOutput) -> [f64; 3] {
    let mut w = [0.0_f64; 3];
    let fa = a.final_state.clone().unwrap();
    let fb = b.final_state.clone().unwrap();
    for (x, y) in a
        .states
        .iter()
        .zip(&b.states)
        .chain(std::iter::once((&fa, &fb)))
    {
        assert_eq!(x.t.map(f64::to_bits), y.t.map(f64::to_bits));
        w[0] = w[0].max(diff(x, y, MERCURY));
        w[1] = w[1].max(diff(x, y, MARS));
        w[2] = w[2].max(geo_diff(x, y));
    }
    w
}

fn timed(s: &ScenarioSpec, data: &DataSources<'_>) -> RunOutput {
    let t = Instant::now();
    let out = run(s, data).unwrap();
    println!(
        "  ran {} [{} {}{}] in {:.1} s: {} steps, {} evaluations, final t = {:.6} yr",
        s.name,
        s.integrator.name.label(),
        s.integrator.precision.label(),
        if s.integrator.compensated == Some(false) {
            ", plain sums"
        } else {
            ""
        },
        t.elapsed().as_secs_f64(),
        out.stats.steps,
        out.stats.force_evaluations,
        p(out.final_state.as_ref().unwrap().t) / s.span_seconds() * s.span.years
    );
    out
}

fn variant(
    base: &ScenarioSpec,
    name: IntegratorName,
    prec: Precision,
    plain: bool,
) -> ScenarioSpec {
    let mut s = base.clone();
    s.integrator.precision = prec;
    if name == IntegratorName::Ias15 {
        let ias = spec("stage-c-sun-planets-moon-eih-100yr.toml").integrator;
        s.integrator = ias;
        s.integrator.precision = prec;
    } else if plain {
        s.integrator.compensated = Some(false);
    }
    s
}

fn report(label: &str, base: &ScenarioSpec, data: &DataSources<'_>, with_ias15: bool) {
    println!("B3 ({label}): {}", base.description);
    let abm_dd = timed(
        &variant(base, IntegratorName::Abm, Precision::Dd, false),
        data,
    );
    let abm_c = timed(
        &variant(base, IntegratorName::Abm, Precision::F64, false),
        data,
    );
    let abm_p = timed(
        &variant(base, IntegratorName::Abm, Precision::F64, true),
        data,
    );
    let mut rows = vec![
        ("ABM f64 compensated - ABM DD", row(&abm_c, &abm_dd)),
        ("ABM f64 plain - ABM DD", row(&abm_p, &abm_dd)),
    ];
    if with_ias15 {
        let ias_dd = timed(
            &variant(base, IntegratorName::Ias15, Precision::Dd, false),
            data,
        );
        let ias_f = timed(
            &variant(base, IntegratorName::Ias15, Precision::F64, false),
            data,
        );
        rows.push(("IAS15 f64 - IAS15 DD", row(&ias_f, &ias_dd)));
        rows.push(("ABM DD - IAS15 DD (method)", row(&abm_dd, &ias_dd)));
        rows.push(("ABM f64 compensated - IAS15 f64 (B2)", row(&abm_c, &ias_f)));
        rows.push(("ABM f64 plain - IAS15 f64", row(&abm_p, &ias_f)));
        rows.push(("IAS15 f64 - ABM DD", row(&ias_f, &abm_dd)));
    }
    println!(
        "  {:<40} {:>14} {:>14} {:>14}",
        "max |dr| over 40 yr (km)", "Mercury", "Mars", "Moon-Earth"
    );
    for (name, r) in rows {
        println!(
            "  {name:<40} {:>14.6e} {:>14.6e} {:>14.6e}",
            r[0], r[1], r[2]
        );
    }
}

/// B3: the Subbotin et al. 2023 configuration runs end to end; f64 round-off against a full
/// double-double run is reported (asserted in WP25).
///
/// Verifies: NUM-002, NUM-007, DEP-007
#[test]
#[ignore = "nightly: 40-yr double-double EIH runs, including 343 Minor bodies; run with --profile nightly"]
fn nightly_abm_subbotin_configuration_roundoff_vs_double_double() {
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let base = spec("abm-sun-planets-moon-eih-40yr.toml");
    report("major bodies", &base, &data, true);
    let mut minor = base.clone();
    minor.name = format!("{}-minor343", base.name);
    minor.description = format!("{} + {MINOR} Minor bodies (REF-003)", base.description);
    minor.forces.asteroids = MINOR;
    minor.output.sample_bodies = Some(base.bodies.clone());
    assert!(
        minor
            .output
            .sample_bodies
            .as_ref()
            .unwrap()
            .contains(&BodyName::Mercury)
    );
    report("+ 343 Minor bodies", &minor, &data, false);
}
