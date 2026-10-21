// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP29: the Wisdom–Holman integrator (`ferroastro-symplectic`, NUM-004) and IRK16
//! (`ferroastro-integrators`, NUM-003) on the real DE440 initial conditions (PR tier: header.440
//! from the pinned JPL cache, re-verified against its SHA-256).
//!
//! * W1: two bodies (Sun + Mercury): the WH map equals one Kepler drift of the relative orbit.
//! * W2: Sun + 8 planets, 100 yr: the uncorrected energy error scales as h² (h = 4 d vs 2 d).
//! * W3: the order-11 corrector reduces the energy error by at least the registered factor.
//! * G1: Sun + Mercury with the GR effective potential: the perihelion rate is the PHY-005 one.
//! * P1: `WHFast` and IRK16 from scenario files (model string, steps, agreement with IAS15
//!   reported), checkpoint and resume bit for bit (CI-010).
//! * P2: refusals by name (REF-011).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_analytic::drift::kepler_drift;
use ferroastro_constants::header440::Header;
use ferroastro_core::ErrorCode;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::Velocity;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GrPotential, MoonVariables, NBody, NBodyModel, NewtonianMutual,
};
use ferroastro_propagate::{
    Checkpoint, DataSources, RunOutput, StateRecord, initial_conditions, resume, run, run_until,
};
use ferroastro_scenario::{BodyName, IntegratorName, Parts, Precision, ScenarioSpec};
use ferroastro_symplectic::{CorrectorOrder, WhConfig, Whfast};
use ferroastro_system::Tier;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const SECONDS_PER_DAY: f64 = ferroastro_core::time::SECONDS_PER_DAY;
const DAYS_PER_YEAR: f64 = ferroastro_scenario::DAYS_PER_JULIAN_YEAR;
/// W1 step (days) and step count (WP29 plan).
const W1_STEP_DAYS: f64 = 4.0; // lint: allow-literal(test design: WH step, days)
const W1_STEPS: u32 = 1000;
/// W2/W3 steps (days), span (Julian years) and energy sampling interval (steps of 4 d).
const W2_STEPS_DAYS: [f64; 2] = [4.0, 2.0]; // lint: allow-literal(test design: WH steps, days)
const W2_YEARS: f64 = 100.0; // lint: allow-literal(test design: span, Julian years)
const W2_SAMPLE_EVERY_DAYS: f64 = 100.0; // lint: allow-literal(test design: energy sampling, days)

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

fn vs() -> VectorSet {
    VectorSet::load_workspace().unwrap()
}

fn value(v: &VectorSet, id: &str) -> f64 {
    v.get(id).unwrap().value().unwrap()
}

fn p(x: Parts) -> f64 {
    x[0] + x[1]
}

/// A Newtonian (optionally + GR potential) model of a scenario's bodies and its initial state
/// (component-major), as the scenario assembles them (same IC hash).
struct System {
    model: NBodyModel<Icrf>,
    gm: Vec<f64>,
    x0: Vec<f64>,
    v0: Vec<f64>,
    ic_hash: String,
}

fn system(s: &ScenarioSpec, data: &DataSources<'_>, gr_potential: bool) -> System {
    let ic = initial_conditions(s, data).unwrap();
    let n = ic.records.len();
    let bodies: Vec<NBody> = ic
        .records
        .iter()
        .map(|r| NBody {
            id: NaifId(r.naif),
            gm: p(r.gm),
            tier: Tier::Major,
        })
        .collect();
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
    if gr_potential {
        let de = ferroastro_constants::De440::load().unwrap();
        terms.push(Box::new(
            GrPotential::new(Velocity::from_km_per_s(de.speed_of_light().km_per_s()).unwrap())
                .unwrap(),
        ));
    }
    let model = NBodyModel::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
    let flat = |f: &dyn Fn(&ferroastro_scenario::IcRecord) -> [Parts; 3]| -> Vec<f64> {
        (0..3)
            .flat_map(|k| ic.records.iter().map(move |r| p(f(r)[k])))
            .collect()
    };
    System {
        model,
        gm: ic.records.iter().map(|r| p(r.gm)).collect(),
        x0: flat(&|r| r.position),
        v0: flat(&|r| r.velocity),
        ic_hash: format!("{} ({n} bodies)", ic.ic_hash),
    }
}

fn with_bodies(mut s: ScenarioSpec, bodies: &[BodyName]) -> ScenarioSpec {
    s.bodies = bodies.to_vec();
    s
}

fn rel(x: &[f64], n: usize, i: usize, j: usize) -> [f64; 3] {
    [0, 1, 2].map(|k| x[k * n + i] - x[k * n + j])
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

/// Verifies: NUM-004
#[test]
fn w1_two_body_wh_equals_the_kepler_drift() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = with_bodies(
        spec("stage-a-sun-8planets-newton-100yr.toml"),
        &[BodyName::Sun, BodyName::Mercury],
    );
    let sys = system(&s, &data, false);
    let hs = W1_STEP_DAYS * SECONDS_PER_DAY;
    let cfg = WhConfig {
        output_every_step: false,
        ..WhConfig::default()
    };
    let mut wh = Whfast::new(&sys.model, &sys.gm, 0.0, &sys.x0, &sys.v0, hs, cfg).unwrap();
    for _ in 0..W1_STEPS {
        wh.step(&sys.model).unwrap();
    }
    let (mut x, mut v) = (vec![0.0; 6], vec![0.0; 6]);
    wh.physical_state(&sys.model, &mut x, &mut v).unwrap();
    let mu = sys.gm[0] + sys.gm[1];
    let (mut r, mut w) = (rel(&sys.x0, 2, 1, 0), rel(&sys.v0, 2, 1, 0));
    kepler_drift(mu, &mut r, &mut w, f64::from(W1_STEPS) * hs).unwrap();
    let got = rel(&x, 2, 1, 0);
    let d = norm([got[0] - r[0], got[1] - r[1], got[2] - r[2]]) / norm(r);
    let bound = value(&vs(), "WH-TWO-BODY-EXACT-REL");
    println!(
        "W1: Sun + Mercury (IC hash {}), {W1_STEPS} WH steps of {W1_STEP_DAYS} d vs one Kepler \
         drift: |dr|/r = {d:.3e} (bound {bound:e})",
        sys.ic_hash
    );
    assert!(d <= bound);
}

/// Max |ΔE/E₀| over the samples of a WH run: (uncorrected kernel, corrected physical).
fn wh_energy(sys: &System, step_days: f64, corrector: CorrectorOrder, years: f64) -> (f64, f64) {
    let hs = step_days * SECONDS_PER_DAY;
    let cfg = WhConfig {
        corrector,
        output_every_step: false,
    };
    let mut wh = Whfast::new(&sys.model, &sys.gm, 0.0, &sys.x0, &sys.v0, hs, cfg).unwrap();
    let e0 = sys.model.energy_over_g(&sys.x0, &sys.v0).unwrap();
    let steps = (years * DAYS_PER_YEAR / step_days).round();
    let every = (W2_SAMPLE_EVERY_DAYS / step_days).round();
    let m = sys.x0.len();
    let (mut x, mut v) = (vec![0.0; m], vec![0.0; m]);
    let (mut worst_k, mut worst_p) = (0.0_f64, 0.0_f64);
    let mut k = 0.0;
    while k < steps {
        wh.step(&sys.model).unwrap();
        k += 1.0;
        if k % every == 0.0 || k >= steps {
            wh.kernel_state(&mut x, &mut v).unwrap();
            let ek = sys.model.energy_over_g(&x, &v).unwrap();
            worst_k = worst_k.max(((ek - e0) / e0).abs());
            if corrector != CorrectorOrder::None {
                wh.physical_state(&sys.model, &mut x, &mut v).unwrap();
                let ep = sys.model.energy_over_g(&x, &v).unwrap();
                worst_p = worst_p.max(((ep - e0) / e0).abs());
            }
        }
    }
    (worst_k, worst_p)
}

/// Verifies: NUM-004
#[test]
fn w2_w3_energy_error_scales_as_h_squared_and_the_corrector_removes_it() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("stage-a-sun-8planets-newton-100yr.toml");
    let sys = system(&s, &data, false);
    let v = vs();
    let (e4, e4c) = wh_energy(&sys, W2_STEPS_DAYS[0], CorrectorOrder::Eleventh, W2_YEARS);
    let (e2, _) = wh_energy(&sys, W2_STEPS_DAYS[1], CorrectorOrder::None, W2_YEARS);
    let ratio = e4 / e2;
    let gain = e4 / e4c;
    let (want, tol) = (value(&v, "WH-ENERGY-ORDER-RATIO"), {
        let x = v.get("WH-ENERGY-ORDER-RATIO").unwrap();
        x.tolerance.allowed(x.value().unwrap())
    });
    let gain_min = value(&v, "WH-CORRECTOR-GAIN-MIN");
    println!(
        "W2/W3: Sun + 8 planets (IC hash {}), {W2_YEARS} yr, energy every {W2_SAMPLE_EVERY_DAYS} \
         d: max |dE/E| uncorrected h = 4 d {e4:.3e}, h = 2 d {e2:.3e} (ratio {ratio:.3}, \
         expected {want} +/- {tol}); order-11 corrector at h = 4 d {e4c:.3e} (gain {gain:.1}, \
         min {gain_min})",
        sys.ic_hash
    );
    assert!((ratio - want).abs() <= tol, "W2 ratio {ratio}");
    assert!(gain >= gain_min, "W3 gain {gain}");
}

/// Verifies: NUM-004, PHY-005
#[test]
fn g1_gr_potential_rung_gives_the_phy005_rate() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("wh-sun-mercury-grpotential-100yr.toml");
    let sys = system(&s, &data, true);
    let hs = s.integrator.step_days.unwrap() * SECONDS_PER_DAY;
    let cfg = WhConfig {
        output_every_step: false,
        ..WhConfig::default()
    };
    let mut wh = Whfast::new(&sys.model, &sys.gm, 0.0, &sys.x0, &sys.v0, hs, cfg).unwrap();
    let mu = sys.gm[0] + sys.gm[1];
    let ecc = |x: &[f64], v: &[f64]| -> [f64; 3] {
        let (r, w) = (rel(x, 2, 1, 0), rel(v, 2, 1, 0));
        ferroastro_analytic::vector::eccentricity_vector(r, w, mu)
    };
    // In-plane basis of the initial orbit: p = ê₀, q = ĥ₀ × ê₀.
    let e0 = ecc(&sys.x0, &sys.v0);
    let h0 = ferroastro_analytic::vector::cross(rel(&sys.x0, 2, 1, 0), rel(&sys.v0, 2, 1, 0));
    let unit = |a: [f64; 3]| {
        let n = norm(a);
        a.map(|z| z / n)
    };
    let (pv, hv) = (unit(e0), unit(h0));
    let qv = ferroastro_analytic::vector::cross(hv, pv);
    let dot = ferroastro_analytic::vector::dot::<f64>;
    let steps = (s.span.years * DAYS_PER_YEAR / s.integrator.step_days.unwrap()).round();
    let (mut x, mut v) = (vec![0.0; 6], vec![0.0; 6]);
    let (mut ts, mut angles) = (Vec::new(), Vec::new());
    let mut prev = 0.0_f64;
    let mut turns = 0.0_f64;
    let mut k = 0.0;
    while k < steps {
        wh.step(&sys.model).unwrap();
        k += 1.0;
        wh.physical_state(&sys.model, &mut x, &mut v).unwrap();
        let e = ecc(&x, &v);
        let a = dot(e, qv).atan2(dot(e, pv));
        if a - prev > std::f64::consts::PI {
            turns -= 1.0;
        } else if prev - a > std::f64::consts::PI {
            turns += 1.0;
        }
        prev = a;
        ts.push(wh.time());
        angles.push(turns.mul_add(std::f64::consts::TAU, a));
    }
    let fit = ferroastro_precession::fit::fit_line(&ts, &angles).unwrap();
    let per_cy = |r: f64| {
        ferroastro_analytic::advance::rate_arcsec_per_century(
            ferroastro_core::units::AngularRate::from_rad_per_s(r).unwrap(),
        )
    };
    let rate = per_cy(fit.slope);
    let want = value(&vs(), "PHY005-GR-NUM-RATE");
    let tol = {
        let x = vs();
        let vv = x.get("PHY005-GR-NUM-RATE").unwrap();
        vv.tolerance.allowed(vv.value().unwrap())
    };
    println!(
        "G1: Sun + Mercury (IC hash {}), Newton + GR effective potential (Nobili & Roxburgh), \
         WHFast corrector 11, h = 1 d, 100 yr, LRL angle line fit over {} samples: \
         {rate:.6} arcsec/cy +/- {:.2e} (formal), expected {want} +/- {tol}; stats {:?}",
        sys.ic_hash,
        ts.len(),
        per_cy(fit.slope_stderr),
        wh.stats()
    );
    assert!((rate - want).abs() <= tol);
}

fn bits(o: &RunOutput) -> Vec<u64> {
    let mut w = Vec::new();
    let mut st = |r: &StateRecord| {
        for q in r.positions.iter().chain(&r.velocities) {
            w.extend(q.iter().flatten().map(|z| z.to_bits()));
        }
    };
    o.states.iter().for_each(&mut st);
    if let Some(f) = &o.final_state {
        st(f);
    }
    w
}

/// P1 span (Julian years) and checkpoint time (years).
const P1_YEARS: f64 = 10.0; // lint: allow-literal(test design: span, Julian years)
const P1_STOP_YEARS: f64 = 5.0; // lint: allow-literal(test design: checkpoint time, Julian years)

/// Verifies: NUM-003, NUM-004, CI-010, CI-002
#[test]
fn p1_scenarios_run_whfast_and_irk16_and_resume_bit_for_bit() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut ias = spec("stage-a-sun-8planets-newton-100yr.toml");
    ias.span.years = P1_YEARS;
    let reference = run(&ias, &data).unwrap();
    for file in [
        "wh-sun-8planets-newton-100yr.toml",
        "irk16-sun-8planets-newton-100yr.toml",
    ] {
        let mut s = spec(file);
        s.span.years = P1_YEARS;
        let whole = run(&s, &data).unwrap();
        assert!(whole.finished);
        assert_eq!(whole.ic_hash, reference.ic_hash, "same ICs as stage A");
        let label = match s.integrator.name {
            IntegratorName::Whfast => "Wisdom-Holman",
            IntegratorName::Irk16 => "IRK16",
            _ => unreachable!(),
        };
        assert!(whole.model.contains(label), "{}", whole.model);
        // Agreement with IAS15 (reported): every body, max over the yearly samples.
        let mut worst: Vec<(i32, f64)> = Vec::new();
        for (a, b) in whole.states.iter().zip(&reference.states) {
            for (i, &id) in a.bodies.iter().enumerate() {
                let d = norm([0, 1, 2].map(|k| p(a.positions[i][k]) - p(b.positions[i][k])));
                match worst.iter_mut().find(|(j, _)| *j == id) {
                    Some(e) => e.1 = e.1.max(d),
                    None => worst.push((id, d)),
                }
            }
        }
        println!(
            "P1 {file}: {} samples, {} steps, model: {}; max |dr| vs IAS15 over {P1_YEARS} yr \
             (km): {worst:?}",
            whole.states.len(),
            whole.stats.steps,
            whole.model
        );
        // Checkpoint at P1_STOP_YEARS, resume through an encoded checkpoint, compare bits.
        let (first, ck) = run_until(&s, &data, P1_STOP_YEARS).unwrap();
        let ck = Checkpoint::decode(&ck.unwrap().encode().unwrap()).unwrap();
        let mut joined = first;
        joined.extend_with(resume(&s, &data, &ck).unwrap()).unwrap();
        let (a, b) = (bits(&whole), bits(&joined));
        let differing = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        println!(
            "P1 {file}: checkpoint at {P1_STOP_YEARS} yr + resume: {differing} differing words of {}",
            a.len()
        );
        assert_eq!(a.len(), b.len());
        assert_eq!(differing, 0);
    }
}

/// Verifies: REF-011, NUM-004
#[test]
fn p2_whfast_and_irk16_refuse_what_they_cannot_integrate() {
    let base = spec("wh-sun-8planets-newton-100yr.toml");
    let mut cases: Vec<(ScenarioSpec, &str)> = Vec::new();
    let mut a = base.clone();
    a.forces.gr_eih = true;
    cases.push((a, "forces.gr_eih with integrator whfast"));
    let mut b = base.clone();
    b.forces.lense_thirring = true;
    cases.push((b, "forces.lense_thirring with integrator whfast"));
    let mut c = base.clone();
    c.forces.asteroids = 16; // lint: allow-literal(a switch value)
    cases.push((c, "forces.asteroids with integrator whfast"));
    let mut d = base.clone();
    d.bodies = vec![BodyName::Sun, BodyName::Earth, BodyName::Moon];
    cases.push((d, "geocentric Moon variables"));
    let mut e = base.clone();
    e.integrator.precision = Precision::Mixed;
    cases.push((e, "integrator.precision.mixed with integrator whfast"));
    let mut f = spec("irk16-sun-8planets-newton-100yr.toml");
    f.integrator.precision = Precision::Mixed;
    cases.push((f, "integrator.precision.mixed with integrator irk16"));
    let mut g = base.clone();
    g.mode = ferroastro_scenario::Mode::TestParticle;
    g.bodies = vec![BodyName::Sun, BodyName::Mercury];
    cases.push((g, "integrator.whfast"));
    for (s, needle) in cases {
        let err = run(&s, &DataSources::none()).unwrap_err();
        println!("P2: {err}");
        assert_eq!(err.code(), ErrorCode::ScenarioUnsupported);
        assert!(err.to_string().contains(needle), "{err}");
    }
    // Keys of another integrator are refused by the scenario validation.
    let mut k = base;
    k.integrator.order = Some(13); // lint: allow-literal(an ABM order)
    let err = k.validate().unwrap_err();
    println!("P2: {err}");
    assert!(err.to_string().contains("integrator.order"));
}
