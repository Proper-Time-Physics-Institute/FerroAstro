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
    Checkpoint, DataSources, RunOutput, SolarOverrides, initial_conditions, resume, resume_until,
    run, run_batch, run_until, run_with_energy,
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
    // EIH is N-body only (WP19): refused in test-particle mode.
    let mut eih = spec("sun-mercury-gr1pn-header440.toml");
    eih.forces.gr_1pn_central = false;
    eih.forces.gr_eih = true;
    // Small bodies are N-body only (WP15): refused in test-particle mode.
    let mut ast = spec("sun-mercury-gr1pn-header440.toml");
    ast.forces.asteroids = 343; // lint: allow-literal(the DE440 asteroid count, a switch value)
    // Solar Lense-Thirring is implemented in both modes since WP20, so it is not listed here.
    let mut abm = base;
    abm.integrator.name = ferroastro_scenario::IntegratorName::Abm;
    for (s, needle) in [
        (eih, "forces.gr_eih"),
        (ast, "forces.asteroids"),
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

const SMALL_BODY_YEARS: f64 = 1.0; // lint: allow-literal(run length, PR-tier cost)

fn small_body_file() -> ferroastro_system::SmallBodyStates {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    ferroastro_system::SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn bits(p: &[ferroastro_scenario::Parts; 3]) -> Vec<u64> {
    p.iter()
        .flat_map(|x| x.iter().map(|y| y.to_bits()))
        .collect()
}

/// WP15: the `asteroids`, `kbos` and `kbo_ring` switches add the DE440 small bodies (REF-003
/// states + the header Sun; GMs from header.440) without touching the major-body records; the
/// ring stays at rest; sampling can be restricted to named bodies; the run is deterministic.
///
/// Verifies: NUM-009, REF-003, TST-005
#[test]
fn small_body_switches_add_minor_and_ring_bodies_without_touching_the_majors() {
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let mut planets = spec("phy012-newton-full-header440.toml");
    planets.span.years = SMALL_BODY_YEARS;
    planets.output.sample_bodies = Some(vec![BodyName::Sun, BodyName::Mercury]);
    let mut full = planets.clone();
    full.forces.asteroids = 343; // lint: allow-literal(the DE440 asteroid count, a switch value)
    full.forces.kbos = true;
    full.forces.kbo_ring = true;
    let a = initial_conditions(&planets, &data).unwrap();
    let b = initial_conditions(&full, &data).unwrap();
    let cat = {
        let mut c = Catalogue::from_header(&h, &prov).unwrap();
        c.with_identified_kbos().unwrap();
        c.with_kbo_ring().unwrap();
        c
    };
    let minor = cat.ids_in(Tier::Minor).count();
    let ring = cat.ids_in(Tier::Ring).count();
    println!(
        "records: planets {} -> full {} ({minor} Minor, {ring} Ring)",
        a.records.len(),
        b.records.len()
    );
    assert_eq!(b.records.len(), a.records.len() + minor + ring);
    for (x, y) in a.records.iter().zip(&b.records) {
        assert_eq!(x.naif, y.naif);
        assert_eq!(bits(&x.position), bits(&y.position), "NAIF {}", x.naif);
        assert_eq!(bits(&x.velocity), bits(&y.velocity), "NAIF {}", x.naif);
    }
    let out = run(&full, &data).unwrap();
    println!("{}\n{}", out.model, out.ic_source);
    assert!(out.model.contains("minor-from-majors") && out.model.contains("ring-fixed-masses"));
    assert!(
        out.model.contains("reaction on Majors = 1"),
        "the asteroids must pull on the Majors (N-AST): {}",
        out.model
    );
    assert!(
        out.states
            .iter()
            .all(|s| s.bodies == [NaifId::SUN.0, NaifId::MERCURY_BARYCENTER.0])
    );
    // The final state honours `sample_bodies` too, so the ring is checked on an unrestricted run
    // (WP15 review: the check above was vacuous with `sample_bodies = [Sun, Mercury]`).
    let mut unrestricted = full.clone();
    unrestricted.output.sample_bodies = None;
    unrestricted.output.cadence_days = None;
    let all = run(&unrestricted, &data).unwrap();
    let fin = all.final_state.as_ref().unwrap();
    let mut ring_checked = 0;
    for (i, id) in fin.bodies.iter().enumerate() {
        let Some(r) = b.records.iter().find(|r| r.naif == *id) else {
            continue;
        };
        if cat.get(NaifId(*id)).is_ok_and(|e| e.tier() == Tier::Ring) {
            assert_eq!(
                bits(&fin.positions[i]),
                bits(&r.position),
                "ring point {id} moved"
            );
            assert!(fin.velocities[i].iter().flatten().all(|v| *v == 0.0));
            ring_checked += 1;
        }
    }
    assert_eq!(ring_checked, ring, "every ring point is checked");
    let again = run(&full, &data).unwrap();
    assert_eq!(
        format!("{:?}", out.final_state),
        format!("{:?}", again.final_state)
    );
    assert_eq!(out.stats.steps, again.stats.steps);

    // Coded refusals: no sampled file, too many asteroids.
    let none = DataSources::none().with_header(&h, &prov);
    let e = run(&full, &none).unwrap_err();
    assert_eq!(e.code(), ErrorCode::PropagateSetup, "{e}");
    let mut too_many = full.clone();
    too_many.forces.asteroids = u32::try_from(cat.ids_in(Tier::Minor).count()).unwrap();
    let e = run(&too_many, &data).unwrap_err();
    println!("{e}");
    assert!(e.to_string().contains("forces.asteroids"), "{e}");
}

/// WP15 review: the Minor bodies enter the dynamics with their header.440 GMs and pull on the
/// Majors. With `asteroids` and `kbos` (no ring, which is an external field), mutual Newtonian
/// forces plus the Minor reaction conserve the total GM-weighted momentum `Σ GMᵢ vᵢ` over every
/// body, to round-off (the WP12 bound). Zeroed small-body GMs in the model, or no reaction,
/// leave the asteroids' momentum change uncompensated and fail this check. The momentum is taken
/// from the barycentric final states of a run of a few steps and of a 1-yr run (the IC records
/// hold the geocentric Moon).
///
/// Verifies: NUM-009, REF-003, NUM-019
#[test]
fn small_bodies_carry_their_gm_and_conserve_total_momentum() {
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let mut s = spec("phy012-newton-full-header440.toml");
    s.forces.asteroids = 343; // lint: allow-literal(the DE440 asteroid count, a switch value)
    s.forces.kbos = true;
    s.output.cadence_days = None;
    let ic = initial_conditions(&s, &data).unwrap();
    let cat = {
        let mut c = Catalogue::from_header(&h, &prov).unwrap();
        c.with_identified_kbos().unwrap();
        c
    };
    let val = |p: &ferroastro_scenario::Parts| p[0] + p[1];
    // (momentum / G, sum GM|v|, Minor share of that sum, IAS15 steps)
    let momentum = |years: f64| {
        let mut t = s.clone();
        t.span.years = years;
        let out = run(&t, &data).unwrap();
        let fin = out.final_state.unwrap();
        let (mut p, mut scale, mut minor) = ([0.0_f64; 3], 0.0_f64, 0.0_f64);
        for r in &ic.records {
            let gm = val(&r.gm);
            let i = fin.bodies.iter().position(|b| *b == r.naif).unwrap();
            let v = fin.velocities[i].map(|x| val(&x));
            for k in 0..3 {
                p[k] += gm * v[k];
            }
            let w = gm * v[0].hypot(v[1]).hypot(v[2]);
            scale += w;
            if cat
                .get(NaifId(r.naif))
                .is_ok_and(|e| e.tier() == Tier::Minor)
            {
                minor += w;
            }
        }
        (p, scale, minor / scale, out.stats.steps)
    };
    let (p0, scale, minor_share, s0) = momentum(HASH_YEARS);
    let (p1, _, _, s1) = momentum(SMALL_BODY_YEARS);
    let dp = (p1[0] - p0[0]).hypot(p1[1] - p0[1]).hypot(p1[2] - p0[2]) / scale;
    let steps = s0 + s1;
    #[expect(clippy::cast_precision_loss, reason = "step counts are small")]
    let ulps = dp / (f64::EPSILON * (steps as f64).sqrt());
    println!(
        "{} bodies, {steps} IAS15 steps: |P(1 yr) - P({HASH_YEARS} yr)| / sum GM|v| = {dp:.3e} = \
         {ulps:.3} eps sqrt(N_steps); Minor share of the momentum scale {minor_share:.3e}",
        ic.records.len(),
    );
    assert!(minor_share > 0.0, "the Minor bodies carry no GM");
    VectorSet::load_canonical()
        .unwrap()
        .get("NBODY-MOMENTUM-ROUNDOFF-ULPS")
        .unwrap()
        .check(ulps)
        .unwrap();
}

/// Progress review after WP15, F9/F10: every Minor and Ring GM in the propagator's model equals
/// its header.440 `MAnnnn` value bitwise (converted with the header's AU and 86400 s/d, the unit
/// change of `ferroastro-system`), under `asteroids = N` (a prefix and all of them), `kbos` and
/// `kbo_ring`. The expected symbols are read here, independently of the catalogue code: the first
/// N numbered `MAnnnn` (n < `MA_UNIDENTIFIED_FROM`) in header order, NAIF id `2 000 000 + n`
/// (cross-checked against the symbol column of the REF-003 file); the identified KBOs from
/// [`KBO_IDENTIFICATION`]; the ring points from [`KBO_RING`]. The GM records are those of the
/// model (`initial_conditions` hashes the GM the propagator integrates with). Kills a consistent
/// rescaling of the small-body GMs (invisible to the momentum test) and a KBO that takes another
/// header KBO's mass.
///
/// Verifies: NUM-009, REF-003
#[test]
fn small_body_gms_equal_their_header_values_bitwise() {
    use ferroastro_system::{
        ASTEROID_NAIF_BASE, KBO_IDENTIFICATION, KBO_RING, MA_UNIDENTIFIED_FROM,
    };
    use std::collections::BTreeMap;
    let (h, prov) = header();
    let file = small_body_file();
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&file);
    let au = h.value("AU").unwrap();
    let to_km3s2 = |au3d2: f64| au3d2 * (au * au * au / (SECONDS_PER_DAY * SECONDS_PER_DAY));
    // Symbol column of the committed REF-003 file (naif,symbol,...), for the cross-check.
    let csv = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv"),
    )
    .unwrap();
    let csv_symbol: BTreeMap<i32, String> = csv
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("naif"))
        .filter_map(|l| {
            let mut f = l.split(',');
            let naif = f.next()?.trim().parse().ok()?;
            let sym = f.next()?.trim().to_owned();
            (!sym.is_empty()).then_some((naif, sym))
        })
        .collect();
    let numbered: Vec<(i32, String)> = h
        .constants()
        .iter()
        .filter_map(|c| {
            let n: i32 = c.name.strip_prefix("MA")?.parse().ok()?;
            (n < MA_UNIDENTIFIED_FROM).then(|| (ASTEROID_NAIF_BASE + n, c.name.clone()))
        })
        .collect();
    for (naif, sym) in &numbered {
        assert_eq!(
            csv_symbol.get(naif),
            Some(sym),
            "REF-003 symbol of NAIF {naif}"
        );
    }
    let mut base = spec("phy012-newton-full-header440.toml");
    base.output.cadence_days = None;
    let majors = initial_conditions(&base, &data).unwrap().records.len();
    // lint: allow-literal(switch values: a prefix of the 343 DE440 asteroids, and all of them)
    for (asteroids, kbos, ring) in [
        (16_u32, false, false),
        (343, false, false),
        (0, true, false),
        (0, false, true),
        (343, true, true),
    ] {
        let mut s = base.clone();
        s.forces.asteroids = asteroids;
        s.forces.kbos = kbos;
        s.forces.kbo_ring = ring;
        let mut expected: Vec<(i32, String)> =
            numbered.iter().take(asteroids as usize).cloned().collect();
        if kbos {
            expected.extend(
                KBO_IDENTIFICATION
                    .iter()
                    .map(|k| (k.id.0, k.symbol.to_owned())),
            );
        }
        if ring {
            expected.extend((0..KBO_RING.points).map(|k| {
                let (sym, id) = KBO_RING.point(k);
                (id.0, sym)
            }));
        }
        let records = initial_conditions(&s, &data).unwrap().records;
        let small: BTreeMap<i32, ferroastro_scenario::Parts> =
            records[majors..].iter().map(|r| (r.naif, r.gm)).collect();
        assert_eq!(
            small.len(),
            expected.len(),
            "asteroids={asteroids} kbos={kbos} ring={ring}: small-body count"
        );
        for (naif, sym) in &expected {
            let want = to_km3s2(h.value(sym).unwrap());
            let got = small
                .get(naif)
                .unwrap_or_else(|| panic!("NAIF {naif} ({sym}) missing"));
            assert_eq!(
                (got[0].to_bits(), got[1].to_bits()),
                (want.to_bits(), 0),
                "NAIF {naif}: model GM {got:?} != header {sym} = {want:e} km^3/s^2"
            );
        }
        println!(
            "asteroids={asteroids} kbos={kbos} kbo_ring={ring}: {} small-body GMs equal header.440 \
             bitwise",
            expected.len()
        );
    }
}

const ENERGY_YEARS: f64 = 10.0; // lint: allow-literal(run length, PR-tier cost)
// A sum of ~50 terms of mixed sign, each rounded once: a few tens of ulp relative to |E|.
const ENERGY_ULPS: f64 = 64.0; // lint: allow-literal(round-off allowance of two energy sums)

/// WP18: `run_with_energy` returns `run`'s output bit for bit, and its initial model energy equals
/// an independent `Σ ½ μ v² − Σ μᵢ μⱼ / r` over the initial records (stage A system, Newtonian);
/// the test-particle mode has none.
///
/// Verifies: NUM-023, CI-008
#[test]
fn run_with_energy_returns_the_run_and_the_model_energy() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut s = spec("stage-a-sun-8planets-newton-100yr.toml");
    s.span.years = ENERGY_YEARS;
    let plain = run(&s, &data).unwrap();
    let (out, energy) = run_with_energy(&s, &data).unwrap();
    assert_eq!(out.differing_words(&plain), Some(0));
    let e = energy.expect("an N-body Newtonian model has an energy");
    let recs = initial_conditions(&s, &data).unwrap().records;
    let mut kinetic = 0.0;
    let mut potential = 0.0;
    for (i, a) in recs.iter().enumerate() {
        let v2: f64 = a.velocity.iter().map(|w| w[0] * w[0]).sum();
        kinetic += 0.5 * a.gm[0] * v2;
        for b in &recs[i + 1..] {
            let r2: f64 = (0..3)
                .map(|k| (a.position[k][0] - b.position[k][0]).powi(2))
                .sum();
            potential -= a.gm[0] * b.gm[0] / r2.sqrt();
        }
    }
    let independent = kinetic + potential;
    let rel = ((e.initial - independent) / independent).abs();
    println!(
        "E/G initial {:.17e}, independent {independent:.17e} (rel {rel:.2e}); last {:.17e}; |dE/E| over {ENERGY_YEARS} yr = {:.3e}",
        e.initial,
        e.last,
        e.relative_change()
    );
    assert!(rel <= ENERGY_ULPS * f64::EPSILON, "{rel}");
    assert!(e.relative_change().is_finite());

    let mut tp = spec("oracle-clite-sun-mercury-tp-gr1pn-252yr.toml");
    tp.span.years = 1.0; // lint: allow-literal(run length, PR-tier cost)
    let (_, none) = run_with_energy(&tp, &data).unwrap();
    assert!(none.is_none());
}

/// WP20: solar Lense–Thirring runs in N-body mode; solar overrides are recorded in the model
/// string and in the checkpoint, and a checkpoint refuses to resume under other overrides
/// (CI-010). The IC hash does not depend on overrides (they are not initial conditions).
///
/// Verifies: NUM-019, CI-010
#[test]
fn nbody_lense_thirring_and_solar_overrides_are_recorded_and_checked() {
    let (h, prov) = header();
    let plain = DataSources::none().with_header(&h, &prov);
    let ov = SolarOverrides {
        j2: Some(2.25e-7), // lint: allow-literal(Park17 J2, vector PARK17-J2SUN; structural here)
        pole: Some([0.0, 0.0, 1.0]),
        spin_gm: None,
        provenance: "structural test".into(),
    };
    let tagged = plain.with_solar_overrides(&ov);
    let mut s = spec("sun-mercury-newton-header440.toml");
    s.forces.sun_j2 = true;
    s.forces.lense_thirring = true;
    s.span.years = 2.0; // lint: allow-literal(short structural run)
    let a = run(&s, &plain).unwrap();
    let b = run(&s, &tagged).unwrap();
    println!("{}\n{}", a.model, b.model);
    assert!(a.model.contains("solar-lense-thirring-with-reaction"));
    assert!(!a.model.contains("solar overrides"));
    assert!(b.model.contains(&ov.describe()) && b.model.contains("J2 = 2.25e-7"));
    assert_eq!(a.ic_hash, b.ic_hash);
    assert_ne!(a.final_state, b.final_state);
    // Both solar terms use the overridden pole (WP20 mutation probe T7) and J2.
    let pole = "pole x = 0 1; pole y = 0 1; pole z = 1 1;";
    for term in [
        "solar-j2-with-reaction",
        "solar-lense-thirring-with-reaction",
    ] {
        let line = b.model.lines().find(|l| l.contains(term)).unwrap();
        assert!(line.contains(pole), "{line}");
    }
    let j2_line = b.model.lines().find(|l| l.contains("solar-j2")).unwrap();
    assert!(j2_line.contains("J2 = 0.000000225 1;"), "{j2_line}");
    let (_, ck) = run_until(&s, &tagged, 1.0).unwrap();
    let ck = ck.unwrap();
    assert_eq!(ck.solar_overrides, ov.describe());
    let e = resume(&s, &plain, &ck).unwrap_err();
    assert!(e.to_string().contains("solar overrides"), "{e}");
    let resumed = resume(&s, &tagged, &ck).unwrap();
    assert_eq!(resumed.final_state, b.final_state);
}
