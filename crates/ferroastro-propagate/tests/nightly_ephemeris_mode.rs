// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP25 nightly checks N1–N4: the Subbotin, Kodukov & Pavlov 2023 Table 1 configuration on
//! DE440 data (`ci/scenarios/ephemeris-mixed-epm16-40yr.toml`: 16 EIH Majors of their §2 + 338
//! Minor asteroids, header.440 + REF-003 states, relativistic SSB, ABM13 PECEC h = 1/16 d,
//! 40 yr).
//!
//! Five runs of one scenario, in parallel threads:
//! * full double-double (truth);
//! * the NUM-007 mixed mode;
//! * f64 with plain sums (Table 1's f64 column);
//! * f64 compensated (reported);
//! * the mixed mode with every asteroid Major (all-mutual, N4), in checkpointed 10-yr segments
//!   through a store under `target/wp25-store` (CI-010; an interrupted run resumes).
//!
//! Checks (pre-registered in `tests/vectors/ephemeris-mode.toml`):
//! * N1 `EPH-MIXED-MERCURY-40YR-KM`, N1b `EPH-F64-MERCURY-40YR-BAND-KM`;
//! * N2 `EPH-MIXED-MOON-40YR-KM`;
//! * N3 `EPH-SSB-DRIFT-40YR-KM` (DD and mixed);
//! * N4 `EPH-TIERING-PLANETS-40YR-KM`, `EPH-TIERING-MOON-40YR-KM`, under the pre-registered
//!   resolvability condition.
//!
//! Every number is printed before any assertion. Run: `cargo nextest run -p ferroastro-propagate
//! --cargo-profile release --profile nightly --run-ignored only -E
//! 'test(nightly_wp25_subbotin_table1)'`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; one \
              linear report"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::units::Velocity;
use ferroastro_eih::{BarycentreBody, relativistic_barycentre};
use ferroastro_propagate::{
    Checkpoint, DataSources, RunOutput, StateRecord, initial_conditions, resume_until, run,
    run_until,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_scenario::{MajorAsteroids, Parts, Precision, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

const REFERENCE: &str = "ephemeris-mixed-epm16-40yr.toml";
/// Checkpoint segment of the all-mutual run (Julian years).
const SEGMENT_YEARS: f64 = 10.0;
const MERCURY: i32 = 1;
const MARS: i32 = 4;
const EARTH: i32 = 399;
const MOON: i32 = 301;
/// The planets and Pluto (barycentric slots) of the N4 tiering check.
const PLANETS: [(i32, &str); 9] = [
    (1, "Mercury"),
    (2, "Venus"),
    (399, "Earth"),
    (4, "Mars"),
    (5, "Jupiter"),
    (6, "Saturn"),
    (7, "Uranus"),
    (8, "Neptune"),
    (9, "Pluto"),
];

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
    let f =
        ferroastro_system::SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    println!("REF-003 states: {}", f.provenance());
    f
}

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn dd(x: Parts) -> DoubleDouble {
    DoubleDouble::from_parts(x[0], x[1])
}

fn index(r: &StateRecord, id: i32) -> usize {
    r.bodies.iter().position(|&x| x == id).unwrap()
}

/// |a − b| of body `id`'s barycentric position, or of Moon − Earth for `MOON`.
fn diff(a: &StateRecord, b: &StateRecord, id: i32) -> f64 {
    let p = |r: &StateRecord, k: usize| -> DoubleDouble {
        if id == MOON {
            dd(r.positions[index(r, MOON)][k]) - dd(r.positions[index(r, EARTH)][k])
        } else {
            dd(r.positions[index(r, id)][k])
        }
    };
    (0..3)
        .map(|k| (p(a, k) - p(b, k)).to_f64().powi(2))
        .sum::<f64>()
        .sqrt()
}

fn records(o: &RunOutput) -> impl Iterator<Item = &StateRecord> {
    o.states.iter().chain(o.final_state.iter())
}

/// Max over the samples and the final state of |a − b| for `id`.
fn max_diff(a: &RunOutput, b: &RunOutput, id: i32) -> f64 {
    assert_eq!(a.states.len(), b.states.len());
    records(a)
        .zip(records(b))
        .map(|(x, y)| {
            assert_eq!(x.t.map(f64::to_bits), y.t.map(f64::to_bits));
            diff(x, y, id)
        })
        .fold(0.0, f64::max)
}

/// Max over the samples and the final state of |b(t)|, μ* in double-double over every body.
fn ssb_drift(o: &RunOutput, gm: &[DoubleDouble], c: Velocity<DoubleDouble>) -> f64 {
    records(o)
        .map(|r| {
            assert_eq!(r.bodies.len(), gm.len());
            let bodies: Vec<BarycentreBody<DoubleDouble>> = gm
                .iter()
                .zip(r.positions.iter().zip(&r.velocities))
                .map(|(&g, (p, w))| BarycentreBody {
                    gm: g,
                    position: p.map(dd),
                    velocity: w.map(dd),
                })
                .collect();
            let (b, _) = relativistic_barycentre(&bodies, c).unwrap();
            let [x, y, z] = b.map(DoubleDouble::to_f64);
            x.hypot(y).hypot(z)
        })
        .fold(0.0, f64::max)
}

fn timed(label: &str, s: &ScenarioSpec, data: &DataSources<'_>) -> (RunOutput, f64) {
    let t = Instant::now();
    let out = run(s, data).unwrap();
    let wall = t.elapsed().as_secs_f64();
    println!(
        "  ran {label}: {wall:.1} s, {} steps, {} evaluations",
        out.stats.steps, out.stats.force_evaluations
    );
    (out, wall)
}

/// The run in `SEGMENT_YEARS` segments through checkpoints, stored in `dir` after each segment
/// (checkpoint + output so far), resuming from the store if present.
fn segmented(s: &ScenarioSpec, data: &DataSources<'_>, dir: &Path) -> (RunOutput, f64, u32) {
    std::fs::create_dir_all(dir).unwrap();
    let ck_path = dir.join(format!("{}.checkpoint", s.digest()));
    let out_path = dir.join(format!("{}.output", s.digest()));
    let t = Instant::now();
    let (mut acc, mut ck, mut segment) = match (std::fs::read(&ck_path), std::fs::read(&out_path)) {
        (Ok(c), Ok(o)) => {
            let c = Checkpoint::decode(&c).unwrap();
            let o = RunOutput::decode(&o).unwrap();
            let seg = (o.final_state.as_ref().map_or(0.0, |r| r.t[0] + r.t[1])
                / (SEGMENT_YEARS * ferroastro_scenario::DAYS_PER_JULIAN_YEAR * 86_400.0)) // lint: allow-literal(seconds per day)
                .round();
            println!("  all-mutual: resuming from the store after {seg} segment(s)");
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a few segments"
            )]
            (Some(o), Some(c), seg as u32)
        }
        _ => (None, None, 0),
    };
    let mut segments_run = 0;
    loop {
        segment += 1;
        let stop = SEGMENT_YEARS * f64::from(segment);
        let (out, next) = match &ck {
            None => run_until(s, data, stop).unwrap(),
            Some(c) => resume_until(s, data, c, stop).unwrap(),
        };
        segments_run += 1;
        println!(
            "  all-mutual segment {segment} done at {:.1} s",
            t.elapsed().as_secs_f64()
        );
        match acc.as_mut() {
            Some(a) => a.extend_with(out).unwrap(),
            None => acc = Some(out),
        }
        match next {
            None => {
                let _ = std::fs::remove_file(&ck_path);
                let _ = std::fs::remove_file(&out_path);
                return (acc.unwrap(), t.elapsed().as_secs_f64(), segments_run);
            }
            Some(c) => {
                std::fs::write(&ck_path, c.encode().unwrap()).unwrap();
                std::fs::write(&out_path, acc.as_ref().unwrap().encode().unwrap()).unwrap();
                ck = Some(c);
            }
        }
    }
}

/// N1–N4: the Subbotin et al. 2023 Table 1 configuration in DD, mixed and f64, and the tiering
/// error against the all-mutual run.
///
/// Verifies: NUM-007, NUM-008, NUM-018, NUM-009, DEP-007
#[test]
#[ignore = "nightly: five 40-yr runs with 354 bodies (one all-mutual, ~50 min); run with --profile nightly"]
fn nightly_wp25_subbotin_table1_mixed_mode() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let base = spec(REFERENCE);
    let with = |p: Precision, plain: bool, all: bool| {
        let mut s = base.clone();
        s.integrator.precision = p;
        if plain {
            s.integrator.compensated = Some(false);
        }
        if all {
            s.forces.major_asteroids = MajorAsteroids::All;
            s.name = format!("{}-all-mutual", base.name);
        }
        s
    };
    let store = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/wp25-store");
    let ic = initial_conditions(&base, &data).unwrap();
    println!(
        "WP25 reference: {} ({} bodies, IC hash {})",
        base.description,
        ic.records.len(),
        ic.ic_hash
    );
    let ((truth, w_dd), (mixed, w_mx), (plain, w_fp), (comp, w_fc), (all, w_all, segs)) =
        std::thread::scope(|sc| {
            let a = sc.spawn(|| timed("double-double", &with(Precision::Dd, false, false), &data));
            let b = sc.spawn(|| timed("mixed", &with(Precision::Mixed, false, false), &data));
            let c = sc.spawn(|| timed("f64 plain", &with(Precision::F64, true, false), &data));
            let d = sc.spawn(|| {
                timed(
                    "f64 compensated",
                    &with(Precision::F64, false, false),
                    &data,
                )
            });
            let e = sc.spawn(|| segmented(&with(Precision::Mixed, false, true), &data, &store));
            (
                a.join().unwrap(),
                b.join().unwrap(),
                c.join().unwrap(),
                d.join().unwrap(),
                e.join().unwrap(),
            )
        });
    assert_eq!(mixed.ic_hash, truth.ic_hash);
    println!(
        "  mixed model: {}",
        mixed.model.lines().last().unwrap_or("")
    );
    println!(
        "  all-mutual model: {}",
        all.model.lines().next().unwrap_or("")
    );
    println!("  all-mutual: {segs} segment(s) of {SEGMENT_YEARS} yr in this call");

    // N1, N1b, N2 and Mars (reported).
    let row = |o: &RunOutput| [MERCURY, MARS, MOON].map(|id| max_diff(o, &truth, id));
    let r_mx = row(&mixed);
    let r_fp = row(&plain);
    let r_fc = row(&comp);
    println!("Table 1 (max over 40 yr vs the full double-double run, m; Moon geocentric):");
    println!(
        "  {:<22} {:>12} {:>12} {:>12} {:>10}",
        "", "Mercury", "Mars", "Moon", "wall (s)"
    );
    for (name, r, w) in [
        ("f64 plain", r_fp, w_fp),
        ("f64 compensated", r_fc, w_fc),
        ("mixed (NUM-007)", r_mx, w_mx),
    ] {
        println!(
            "  {name:<22} {:>12.4e} {:>12.4e} {:>12.4e} {w:>10.1}",
            r[0] * 1e3, // lint: allow-literal(km to m)
            r[1] * 1e3, // lint: allow-literal(km to m)
            r[2] * 1e3  // lint: allow-literal(km to m)
        );
    }
    println!(
        "  {:<22} {:>12} {:>12} {:>12} {w_dd:>10.1}",
        "double-double", "truth", "truth", "truth"
    );
    println!(
        "  Subbotin et al. 2023 Table 1: f64 2.47 / 0.7 / 0.28 m (81 s); mixed 8.4 / 0.44 / 1.2 mm \
         (246 s); double-double 32 min (two-way integration, EPM2021 model)"
    );

    // N3: barycentre drift (mu* in DD over every body).
    let de = De440::load().unwrap();
    let c =
        Velocity::from_km_per_s(DoubleDouble::from_f64(de.speed_of_light().km_per_s())).unwrap();
    let gm: Vec<DoubleDouble> = ic.records.iter().map(|r| dd(r.gm)).collect();
    let drift_dd = ssb_drift(&truth, &gm, c);
    let drift_mx = ssb_drift(&mixed, &gm, c);
    let drift_fp = ssb_drift(&plain, &gm, c);
    println!(
        "N3 barycentre drift max |b(t)| over 40 yr: DD {:.4e} m, mixed {:.4e} m, f64 plain {:.4e} m \
         (Subbotin: 3.14 um, 2.6 um, 0.5 mm)",
        drift_dd * 1e3, // lint: allow-literal(km to m)
        drift_mx * 1e3, // lint: allow-literal(km to m)
        drift_fp * 1e3  // lint: allow-literal(km to m)
    );

    // N4: tiering vs all-mutual, both mixed, with the resolvability condition.
    let tp = vs.get("EPH-TIERING-PLANETS-40YR-KM").unwrap();
    let tm = vs.get("EPH-TIERING-MOON-40YR-KM").unwrap();
    let mut failures: Vec<String> = Vec::new();
    println!("N4 tiering (epm2021 vs all-mutual, both mixed), max over 40 yr:");
    let mut tier_row = |id: i32, name: &str, bound: &ferroastro_vectors::Vector| {
        let t = max_diff(&mixed, &all, id);
        let noise = max_diff(&mixed, &truth, id);
        let limit = bound.tolerance.allowed(0.0);
        let resolvable = 2.0_f64.sqrt() * noise <= limit / 3.0;
        println!(
            "  {name:<8} |tiered - all| = {:.4e} m; mixed round-off {:.4e} m; {}",
            t * 1e3,     // lint: allow-literal(km to m)
            noise * 1e3, // lint: allow-literal(km to m)
            if resolvable {
                "resolvable"
            } else {
                "UNRESOLVED"
            }
        );
        if !resolvable {
            failures.push(format!(
                "N4 {name}: UNRESOLVED (sqrt(2) x round-off {noise:.3e} km > bound/3 = {:.3e} km)",
                limit / 3.0
            ));
        } else if let Err(e) = bound.check(t) {
            failures.push(format!("N4 {name}: {e}"));
        }
    };
    for (id, name) in PLANETS {
        tier_row(id, name, tp);
    }
    tier_row(MOON, "Moon", tm);
    println!("  wall time of the all-mutual mixed run (this call): {w_all:.1} s");

    // Assertions, after every number is printed.
    for (id, measured) in [
        ("EPH-MIXED-MERCURY-40YR-KM", r_mx[0]),
        ("EPH-F64-MERCURY-40YR-BAND-KM", r_fp[0]),
        ("EPH-MIXED-MOON-40YR-KM", r_mx[2]),
        ("EPH-SSB-DRIFT-40YR-KM", drift_dd),
        ("EPH-SSB-DRIFT-40YR-KM", drift_mx),
    ] {
        match vs.get(id).unwrap().check(measured) {
            Ok(d) => println!("  PASS {id}: {measured:.4e} km (|deviation| {d:.3e})"),
            Err(e) => failures.push(format!("{id}: {e}")),
        }
    }
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{} check(s) failed", failures.len());
}

/// N4 follow-up (pre-registered in the WP25 plan): the tiering error in full double-double, where
/// round-off does not limit the comparison. The reference (EPM2021 tiers) and the all-mutual run
/// (every asteroid Major), both double-double, 40 yr; the all-mutual run in checkpointed 10-yr
/// segments through the store (≈ 5 h on the reference machine; an interrupted run resumes).
///
/// Verifies: NUM-009
#[test]
#[ignore = "nightly (long): two 40-yr double-double runs with 354 bodies, one all-mutual (~5 h)"]
fn nightly_wp25_tiering_double_double() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let mut tiered = spec(REFERENCE);
    tiered.integrator.precision = Precision::Dd;
    let mut all = tiered.clone();
    all.forces.major_asteroids = MajorAsteroids::All;
    all.name = format!("{}-all-mutual", tiered.name);
    let store = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/wp25-store");
    let ((t_out, w_t, _), (a_out, w_a, segs)) = std::thread::scope(|sc| {
        let a = sc.spawn(|| segmented(&tiered, &data, &store));
        let b = sc.spawn(|| segmented(&all, &data, &store));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(t_out.ic_hash, a_out.ic_hash);
    println!(
        "N4 (double-double): tiered {w_t:.1} s, all-mutual {w_a:.1} s ({segs} segment(s) in this call)"
    );
    let tp = vs.get("EPH-TIERING-PLANETS-40YR-KM").unwrap();
    let tm = vs.get("EPH-TIERING-MOON-40YR-KM").unwrap();
    let mut failures = Vec::new();
    for (id, name, bound) in PLANETS
        .iter()
        .map(|&(id, n)| (id, n, tp))
        .chain(std::iter::once((MOON, "Moon", tm)))
    {
        let d = max_diff(&t_out, &a_out, id);
        println!(
            "  {name:<8} |tiered - all-mutual| = {:.4e} m (bound {:.0e} m)",
            d * 1e3,                            // lint: allow-literal(km to m)
            bound.tolerance.allowed(0.0) * 1e3  // lint: allow-literal(km to m)
        );
        if let Err(e) = bound.check(d) {
            failures.push(format!("N4 {name}: {e}"));
        }
    }
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{} check(s) failed", failures.len());
}
