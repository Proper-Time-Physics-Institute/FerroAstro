// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Scenario-driven runs on the real DE440 initial conditions (PR tier: header.440 from the
//! pinned JPL cache, CI-005, re-verified against its SHA-256). WP13 checks:
//!
//! * 2 (in process): checkpoint → encode → decode → resume is bitwise; a checkpoint of another
//!   scenario is refused (FA-E1302);
//! * 3: TST-005 (the same scenario twice) and NUM-022 (a batch on 1 and on N threads);
//! * 4: REF-011 refusals through [`run`];
//! * 6: NUM-010 perihelion events of a Newtonian Sun + Mercury N-body run vs analytic Kepler;
//! * 7: IC hash identity;
//! * the scenario path equals the direct `ferroastro-forces` API (WP12 model) bit for bit.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_analytic::kepler::Elements;
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::ErrorCode;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::{Length, TimeSpan};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MoonVariables, NBody, NBodyModel,
    NBodyPropagator, NewtonianMutual, Pole, SolarJ2WithReaction,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_propagate::{
    Checkpoint, DataSources, RunOutput, initial_conditions, resume, resume_until, run, run_batch,
    run_until,
};
use ferroastro_scenario::{BodyName, Precision, ScenarioSpec};
use ferroastro_system::{Catalogue, InitialConditions, Tier};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

// Run lengths in Julian years: run controls chosen for PR-tier cost, not physical values.
const CHECKPOINT_YEARS_F64: f64 = 30.0; // lint: allow-literal(run length, PR-tier cost)
const CHECKPOINT_YEARS_DD: f64 = 6.0; // lint: allow-literal(run length, PR-tier cost)
const TWICE_YEARS: f64 = 20.0; // lint: allow-literal(run length, PR-tier cost)
const BATCH_STEP_YEARS: f64 = 5.0; // lint: allow-literal(batch run lengths 5..40 yr)
const HASH_YEARS: f64 = 0.01; // lint: allow-literal(a few steps: only the IC hash is used)
const SEGMENT_YEARS: f64 = 4.0; // lint: allow-literal(segment length of the chained checkpoint test)
const SEGMENTS: u32 = 5; // lint: allow-literal(number of segments, PR-tier cost)

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

fn planets(years: f64, precision: Precision) -> ScenarioSpec {
    let mut s = spec("sun-8planets-newton-j2-1000yr.toml");
    s.span.years = years;
    s.integrator.precision = precision;
    s
}

/// 2 (in process): stop, encode, decode, resume; and a mismatched checkpoint is refused.
///
/// Verifies: CI-010
#[test]
fn checkpoint_round_trip_and_mismatch_refusal() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    for precision in [Precision::F64, Precision::Dd] {
        let s = planets(
            if precision == Precision::F64 {
                CHECKPOINT_YEARS_F64
            } else {
                CHECKPOINT_YEARS_DD
            },
            precision,
        );
        let full = run(&s, &data).unwrap();
        let (mut first, ck) = run_until(&s, &data, s.span.years / 3.0).unwrap();
        let bytes = ck.unwrap().encode().unwrap();
        let ck = Checkpoint::decode(&bytes).unwrap();
        first.extend_with(resume(&s, &data, &ck).unwrap()).unwrap();
        let words = full.differing_words(&first);
        println!(
            "{}: {} yr, checkpoint {} bytes at step {}; differing words {words:?}",
            precision.label(),
            s.span.years,
            bytes.len(),
            ck.integrator.stats.steps
        );
        assert_eq!(words, Some(0));
        assert_eq!(full.stats, first.stats);
        // Another scenario (one more year) refuses the checkpoint.
        let mut other = s.clone();
        other.span.years += 1.0;
        let e = resume(&other, &data, &ck).unwrap_err();
        assert_eq!(e.code(), ErrorCode::PropagateCheckpoint, "{e}");
        // A corrupted checkpoint is refused.
        let e = Checkpoint::decode(&bytes[..bytes.len() / 2]).unwrap_err();
        assert_eq!(e.code(), ErrorCode::PropagateCheckpoint, "{e}");
    }
}

/// 3: TST-005 in process, and NUM-022: a batch of 8 scenarios gives the same bits on 1 thread
/// and on 8 threads (and as single runs). The speed-up is reported, not asserted.
///
/// Verifies: TST-005, NUM-022
#[test]
fn same_scenario_twice_and_batch_thread_counts_are_bitwise() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = planets(TWICE_YEARS, Precision::F64);
    let (a, b) = (run(&s, &data).unwrap(), run(&s, &data).unwrap());
    assert_eq!(a.differing_words(&b), Some(0));
    assert_eq!(a.stats, b.stats);
    let batch: Vec<ScenarioSpec> = (1..=8_u32)
        .map(|k| planets(BATCH_STEP_YEARS * f64::from(k), Precision::F64))
        .collect();
    let timed = |threads: usize| {
        let t = std::time::Instant::now();
        let r: Vec<RunOutput> = run_batch(&batch, &data, threads)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect();
        (r, t.elapsed().as_secs_f64())
    };
    let (one, t1) = timed(1);
    let (many, tn) = timed(8);
    let differing: usize = one
        .iter()
        .zip(&many)
        .map(|(x, y)| x.differing_words(y).expect("same shapes"))
        .sum();
    let singles_equal = batch
        .iter()
        .zip(&one)
        .all(|(s, r)| run(s, &data).unwrap().differing_words(r) == Some(0));
    println!(
        "batch of {} runs (5..40 yr): 1 thread {t1:.2} s, 8 threads {tn:.2} s (speed-up {:.2}, \
         rayon `parallel` feature {}); differing words {differing}; equal to single runs: \
         {singles_equal}",
        batch.len(),
        t1 / tn,
        if cfg!(feature = "parallel") {
            "on"
        } else {
            "OFF: sequential"
        }
    );
    assert_eq!(differing, 0);
    assert!(singles_equal);
}

/// 4: switches this runner does not implement are refused by name through `run` (REF-011).
///
/// Verifies: REF-011
#[test]
fn run_refuses_unimplemented_switches_by_name() {
    let base = spec("sun-mercury-newton-header440.toml");
    let mut eih = base.clone();
    eih.forces.gr_eih = true;
    let mut ast = base.clone();
    ast.forces.asteroids = 343;
    let mut pn = base.clone();
    pn.forces.gr_1pn_central = true;
    let mut abm = base;
    abm.integrator.name = ferroastro_scenario::IntegratorName::Abm;
    for (s, needle) in [
        (eih, "forces.gr_eih"),
        (ast, "forces.asteroids"),
        (pn, "forces.gr_1pn_central"),
        (abm, "integrator.abm"),
    ] {
        let e = run(&s, &DataSources::none()).unwrap_err();
        println!("{e}");
        assert_eq!(e.code(), ErrorCode::ScenarioUnsupported);
        assert!(e.to_string().contains(needle), "{e}");
    }
}

/// 6: NUM-010 on the N-body dense output: the perihelion events of a Newtonian Sun + Mercury run
/// (header.440 ICs, 100 yr) fall on the analytic Kepler times of the relative orbit, and each
/// is located to NUM-010's 1e-6 s.
///
/// Verifies: NUM-010
#[test]
fn nbody_perihelion_events_match_analytic_kepler_times() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("sun-mercury-newton-header440.toml");
    let out = run(&s, &data).unwrap();
    let de = De440::load().unwrap();
    let mu = de.gm_sun().km3_per_s2() + de.gm(ferroastro_constants::Body::Mercury).km3_per_s2();
    // Initial relative state from the header ICs (the run's own k = 0 values).
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let raw = |id: NaifId| {
        let st = ic.states().state(id).unwrap();
        (
            st.position().components().map(Length::km),
            st.velocity()
                .map(ferroastro_core::units::Velocity::km_per_s),
        )
    };
    let ((rm, vm), (rs, vs)) = (raw(NaifId::MERCURY_BARYCENTER), raw(NaifId::SUN));
    let r0 = [0, 1, 2].map(|k| rm[k] - rs[k]);
    let v0 = [0, 1, 2].map(|k| vm[k] - vs[k]);
    let el = Elements::from_vectors(r0, v0, mu).unwrap();
    let a = el.a.km();
    let n = (mu / (a * a * a)).sqrt();
    let period = std::f64::consts::TAU / n;
    let m0 = el.mean_anomaly.radians().rem_euclid(std::f64::consts::TAU);
    let t_first = (std::f64::consts::TAU - m0) / n;
    let mut worst_kepler: f64 = 0.0;
    let mut worst_location: f64 = 0.0;
    for (k, p) in out.passages.iter().enumerate() {
        let kf = f64::from(u32::try_from(k).unwrap());
        let expected = kf.mul_add(period, t_first);
        worst_kepler = worst_kepler.max((p.t[0] + p.t[1] - expected).abs());
        let (r, v) = (p.r.map(|z| z[0] + z[1]), p.v.map(|z| z[0] + z[1]));
        let dot = |a: [f64; 3], b: [f64; 3]| a[2].mul_add(b[2], a[0].mul_add(b[0], a[1] * b[1]));
        let rr = dot(r, r).sqrt();
        let g = dot(r, v);
        let dg = dot(v, v) - mu / rr;
        worst_location = worst_location.max((g / dg).abs());
    }
    println!(
        "NUM-010 N-body: {} passages over {} yr (P = {:.6} d, first at {:.3} s); max |t_k - \
         (t_p + kP)| = {worst_kepler:.3e} s; max remaining Newton step {worst_location:.3e} s; \
         {} IAS15 steps",
        out.passages.len(),
        s.span.years,
        period / SECONDS_PER_DAY,
        t_first,
        out.stats.steps
    );
    assert!(out.passages.len() > 400);
    let vs = VectorSet::load_canonical().unwrap();
    vs.get("NUM010-NBODY-PERIHELION-VS-KEPLER")
        .unwrap()
        .check(worst_kepler)
        .unwrap();
    vs.get("NUM010-NBODY-EVENT-LOCATION")
        .unwrap()
        .check(worst_location)
        .unwrap();
}

/// 7: the IC hash is reproducible, independent of span and cadence, and changes with the
/// precision (the double-double header ICs carry low parts) and the bodies.
///
/// Verifies: CI-002, NUM-013
#[test]
fn ic_hash_depends_on_the_initial_conditions_only() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let short = |s: &mut ScenarioSpec| s.span.years = HASH_YEARS;
    let mut a = planets(HASH_YEARS, Precision::F64);
    let mut b = planets(HASH_YEARS, Precision::F64);
    b.output.cadence_days = None;
    short(&mut a);
    let mut dd = planets(HASH_YEARS, Precision::Dd);
    short(&mut dd);
    let mut fewer = planets(HASH_YEARS, Precision::F64);
    fewer.bodies.retain(|x| *x != BodyName::Neptune);
    let hash = |s: &ScenarioSpec| run(s, &data).unwrap().ic_hash;
    let (ha, hb, hd, hf) = (hash(&a), hash(&b), hash(&dd), hash(&fewer));
    println!("IC hashes: f64 {ha}; f64 without cadence {hb}; dd {hd}; without Neptune {hf}");
    assert_eq!(ha, hb);
    assert_eq!(ha, hash(&a));
    assert_ne!(ha, hd);
    assert_ne!(ha, hf);
}

/// The scenario path builds exactly the WP12 direct-API model: Sun + 8 planets + geocentric
/// Moon, Newtonian + solar J2 with reaction, header.440 ICs, 10 yr; the final states agree bit
/// for bit with `NBodyPropagator` driven by hand.
///
/// Verifies: CI-002, TST-005
#[test]
fn scenario_run_equals_the_direct_api_bitwise() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("sun-planets-moon-newton-j2-10yr.toml");
    let out = run(&s, &data).unwrap();
    // Direct API, as crates/ferroastro-forces/tests/common/mod.rs (WP12).
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let bodies: Vec<NBody> = s
        .bodies
        .iter()
        .map(|b| NBody {
            id: NaifId(b.naif()),
            gm: cat.gm(NaifId(b.naif())).unwrap().km3_per_s2(),
            tier: Tier::Major,
        })
        .collect();
    let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
        .unwrap()
        .unit_vector();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
        Box::new(NewtonianMutual),
        Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        Box::new(
            SolarJ2WithReaction::new(
                de.sun_j2(),
                Length::from_km(de.sun_radius().km()).unwrap(),
                Pole::from_components(k).unwrap(),
            )
            .unwrap(),
        ),
    ];
    let model = NBodyModel::new(
        &bodies,
        MoonVariables::Geocentric {
            emrat: de.earth_moon_mass_ratio(),
        },
        terms,
    )
    .unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let mut p = NBodyPropagator::new(
        model,
        ic.states(),
        ic.earth_moon(),
        ic.epoch(),
        TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap(),
        Ias15Config::default(),
    )
    .unwrap();
    let end = ic.epoch() + TimeSpan::from_seconds(s.span_seconds()).unwrap();
    p.advance_to(end).unwrap();
    let direct = p.state().unwrap();
    let fin = out.final_state.as_ref().unwrap();
    let mut differing = 0;
    for (i, id) in fin.bodies.iter().enumerate() {
        let st = direct.state(NaifId(*id)).unwrap();
        let pos = st.position().components().map(Length::km);
        let vel = st
            .velocity()
            .map(ferroastro_core::units::Velocity::km_per_s);
        for k in 0..3 {
            differing += usize::from(pos[k].to_bits() != fin.positions[i][k][0].to_bits());
            differing += usize::from(vel[k].to_bits() != fin.velocities[i][k][0].to_bits());
        }
    }
    println!(
        "scenario vs direct API after {} yr: {} steps vs {} steps; differing words {differing} \
         of {}; {} states sampled, {} passages",
        s.span.years,
        out.stats.steps,
        p.stats().steps,
        6 * fin.bodies.len(),
        out.states.len(),
        out.passages.len()
    );
    assert_eq!(out.stats.steps, p.stats().steps);
    assert_eq!(differing, 0);
}

/// CI-010 for long runs cut into many segments: `run_until` + a chain of `resume_until` calls,
/// forwards and backwards in time, each through an encoded checkpoint, equals the uninterrupted
/// run bit for bit; and [`initial_conditions`] reports the hash the run reports.
///
/// Verifies: CI-010, NUM-013
#[test]
fn chained_segments_equal_the_uninterrupted_run_in_both_directions() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let total = SEGMENT_YEARS * f64::from(SEGMENTS);
    for sign in [1.0, -1.0] {
        let mut s = planets(sign * total, Precision::F64);
        s.output.cadence_days = Some(10.0); // lint: allow-literal(output cadence, days)
        let full = run(&s, &data).unwrap();
        let ic = initial_conditions(&s, &data).unwrap();
        assert_eq!(ic.ic_hash, full.ic_hash);
        assert_eq!(ic.records.len(), s.bodies.len());
        let (mut joined, mut ck) = run_until(&s, &data, sign * SEGMENT_YEARS).unwrap();
        let mut k = 1_u32;
        while let Some(c) = ck {
            k += 1;
            let c = Checkpoint::decode(&c.encode().unwrap()).unwrap();
            let (seg, next) =
                resume_until(&s, &data, &c, sign * SEGMENT_YEARS * f64::from(k)).unwrap();
            joined.extend_with(seg).unwrap();
            ck = next;
        }
        let words = full.differing_words(&joined);
        println!(
            "span {} yr in {k} segments: {} states, {} passages, {} steps; differing words {words:?}",
            s.span.years,
            full.states.len(),
            full.passages.len(),
            full.stats.steps
        );
        assert_eq!(k, SEGMENTS);
        // Samples every cadence step in the run's direction of time (backward runs included).
        let cadence_s = 10.0 * SECONDS_PER_DAY; // lint: allow-literal(the cadence set above)
        assert!(full.states.len() > 700, "{} samples", full.states.len()); // lint: allow-literal(20 yr / 10 d)
        for (i, st) in full.states.iter().enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "small sample index")]
            let expect = (sign * cadence_s).mul_add(i as f64, 0.0); // + 0.0: sample 0 is t = +0
            assert_eq!(st.t[0].to_bits(), expect.to_bits(), "sample {i}");
        }
        assert_eq!(words, Some(0));
        assert_eq!(full.stats, joined.stats);
        assert!(joined.finished);
    }
}
