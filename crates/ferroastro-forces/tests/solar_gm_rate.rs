// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The opt-in, inferred time-varying solar GM (issue #42, OI-04): values of the linear law
//! ([`SolarGmRate`]), the force term ([`SolarGmVariation`]) against an independent evaluation and
//! against the difference of two constant-GM Newtonian models, the two-body secular effect
//! against the analytic adiabatic law (pre-registered bound `GMSUN-RATE-ADIABATIC-SMA-REL`), and
//! the guards (time origin, energy diagnostic). Every rate here is synthetic: the DE440 value is
//! read from header.440 at run time and is exercised by the data-tier test
//! `data_inferred_solar_gm_rate_ten_years_vs_de440_bsp` (ferroastro-propagate).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              reference arithmetic is written plainly"
)]

use ferroastro_core::frames::{Barycentric, Icrf, Position, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{GravParam, Length, TimeSpan, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, MinorFromMajors, MinorReaction, MoonVariables, NBody, NBodyModel, NBodyPropagator,
    NewtonianMutual, RingMasses, SecondOrderSystem, SolarGmRate, SolarGmVariation,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_system::{SystemState, Tier};
use ferroastro_vectors::VectorSet;

type Model = NBodyModel<Icrf>;

/// Synthetic solar GM (km³/s²), the scale of the Sun's.
const GM_SUN: f64 = 1.3e11; // lint: allow-literal(synthetic solar GM of the fixtures and of GMSUN-RATE-ADIABATIC-SMA-REL)
/// Synthetic rate of the law-value checks (per Julian year), the order of a solar mass-loss rate.
const RATE_SMALL: f64 = -1.0e-13; // lint: allow-literal(synthetic rate; the DE440 value is not reproduced, #22)
/// Synthetic exaggerated rate of the adiabatic test (per Julian year).
const RATE_ADIABATIC: f64 = -1.0e-6; // lint: allow-literal(synthetic rate of GMSUN-RATE-ADIABATIC-SMA-REL)
/// Seconds per Julian year.
const YEAR_S: f64 = SECONDS_PER_DAY * DAYS_PER_JULIAN_YEAR;

fn j2000_plus_days(days: f64) -> Epoch<Tdb> {
    let (day, frac) = Epoch::<Tdb>::j2000().jd_parts();
    Epoch::from_jd_parts(day + days, frac).unwrap()
}

/// The law's values: 0 at the reference epoch, the rate itself one Julian year later, linear in
/// between and before (at DE440's JDEPOC, from the committed vector), and `GM (1 + offset)`.
///
/// Verifies: PHY-002
#[test]
fn solar_gm_law_values_follow_the_inferred_linear_form() {
    let law = SolarGmRate::inferred_j2000(RATE_SMALL).unwrap();
    assert_eq!(law.reference_epoch(), Epoch::<Tdb>::j2000());
    assert_eq!(
        law.fraction_per_julian_year().to_bits(),
        RATE_SMALL.to_bits()
    );
    // Exact points: 0 at t0 (a signed zero: k x 0); k after exactly one Julian year; -2k two
    // Julian years before.
    assert_eq!(
        law.fractional_offset_at(Epoch::<Tdb>::j2000())
            .abs()
            .to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        law.fractional_offset_at(j2000_plus_days(DAYS_PER_JULIAN_YEAR))
            .to_bits(),
        RATE_SMALL.to_bits()
    );
    assert_eq!(
        law.fractional_offset_at(j2000_plus_days(-2.0 * DAYS_PER_JULIAN_YEAR))
            .to_bits(),
        (-2.0 * RATE_SMALL).to_bits()
    );
    // At DE440's JDEPOC (before J2000): the Sun's GM was larger by -k x 30.5 yr.
    let jdepoc = VectorSet::load_canonical()
        .unwrap()
        .get("DE440-JDEPOC")
        .unwrap()
        .value()
        .unwrap();
    let (j2000, _) = Epoch::<Tdb>::j2000().jd_parts();
    let years = (jdepoc - j2000) / DAYS_PER_JULIAN_YEAR;
    let at_jdepoc = law.fractional_offset_at(Epoch::<Tdb>::from_jd_parts(jdepoc, 0.0).unwrap());
    let expected = RATE_SMALL * years;
    println!("offset at JDEPOC ({years:.4} yr from J2000): {at_jdepoc:e} (expected {expected:e})");
    assert!(at_jdepoc > 0.0);
    assert!((at_jdepoc - expected).abs() <= 4.0 * f64::EPSILON * expected.abs());
    let gm0 = GravParam::from_km3_per_s2(GM_SUN).unwrap();
    let gm = law
        .gm_at(gm0, Epoch::<Tdb>::from_jd_parts(jdepoc, 0.0).unwrap())
        .unwrap()
        .km3_per_s2();
    assert!((gm - GM_SUN * (1.0 + expected)).abs() <= 2.0 * f64::EPSILON * GM_SUN);
    assert!(gm > GM_SUN);
    assert_eq!(
        law.gm_at(gm0, Epoch::<Tdb>::j2000())
            .unwrap()
            .km3_per_s2()
            .to_bits(),
        GM_SUN.to_bits()
    );
    // The same law with an explicit reference epoch elsewhere: only (t - t0) matters.
    let shifted = SolarGmRate::new(RATE_SMALL, j2000_plus_days(DAYS_PER_JULIAN_YEAR)).unwrap();
    assert_eq!(
        shifted
            .fractional_offset_at(j2000_plus_days(2.0 * DAYS_PER_JULIAN_YEAR))
            .to_bits(),
        RATE_SMALL.to_bits()
    );
    // Refusals carry FA-E0500 and say that the convention is inferred.
    for bad in [f64::NAN, f64::INFINITY] {
        let e = SolarGmRate::inferred_j2000(bad).unwrap_err();
        println!("{e}");
        assert_eq!(e.code().to_string(), "FA-E0500");
        assert!(e.to_string().contains("inferred"), "{e}");
    }
    // A linear law pushed to GM <= 0 is refused, not returned.
    let absurd = SolarGmRate::inferred_j2000(-1.0).unwrap(); // lint: allow-literal(absurd rate, refusal check)
    let e = absurd
        .gm_at(gm0, j2000_plus_days(2.0 * DAYS_PER_JULIAN_YEAR))
        .unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(e.to_string().contains("inferred"), "{e}");
}

/// A structural 5-body fixture: Sun, two Major planets, one Minor body, one Ring mass at rest.
fn fixture() -> (Vec<NBody>, Vec<f64>, Vec<f64>) {
    let b = |id: NaifId, gm: f64, tier: Tier| NBody { id, gm, tier };
    let bodies = vec![
        b(NaifId::SUN, GM_SUN, Tier::Major),
        b(NaifId::MERCURY_BARYCENTER, 2.2e4, Tier::Major), // lint: allow-literal(structural fixture)
        b(NaifId::JUPITER_BARYCENTER, 1.3e8, Tier::Major), // lint: allow-literal(structural fixture)
        b(NaifId(2_000_001), 62.0, Tier::Minor), // lint: allow-literal(structural fixture, an asteroid-like id and GM)
        b(NaifId(8201), 5.0e3, Tier::Ring), // lint: allow-literal(structural fixture, a ring-point id and GM)
    ];
    let p: [[f64; 3]; 5] = [
        [-7.0e5, 2.0e5, 1.0e4],  // lint: allow-literal(structural fixture)
        [5.0e7, -2.0e7, 3.0e6],  // lint: allow-literal(structural fixture)
        [5.0e8, -5.5e8, -2.0e8], // lint: allow-literal(structural fixture)
        [-3.0e8, 2.5e8, 4.0e7],  // lint: allow-literal(structural fixture)
        [6.0e9, 2.0e9, 1.0e8],   // lint: allow-literal(structural fixture)
    ];
    let w: [[f64; 3]; 5] = [
        [0.01, -0.012, 0.0003], // lint: allow-literal(structural fixture)
        [20.0, 40.0, 3.0],      // lint: allow-literal(structural fixture)
        [9.0, 8.0, -3.0],       // lint: allow-literal(structural fixture)
        [-12.0, -14.0, 1.0],    // lint: allow-literal(structural fixture)
        [0.0, 0.0, 0.0],
    ];
    let flat = |a: [[f64; 3]; 5]| -> Vec<f64> { (0..3).flat_map(|k| a.map(|r| r[k])).collect() };
    (bodies, flat(p), flat(w))
}

fn accel(m: &Model, t: f64, x: &[f64], v: &[f64]) -> Vec<f64> {
    let mut a = vec![0.0; x.len()];
    m.acceleration(t, x, v, &mut a).unwrap();
    a
}

fn of(a: &[f64], n: usize, i: usize) -> [f64; 3] {
    [a[i], a[n + i], a[2 * n + i]]
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

/// The term alone equals `−GM☉ k (t − t0)/yr · (rᵢ − r☉)/|rᵢ − r☉|³` on every moving non-Sun
/// body (Major and Minor) and adds nothing to the Sun and the Ring; it vanishes at the reference
/// epoch; and it equals the difference between two constant-GM Newtonian models whose solar GMs
/// differ by `δGM`, in which the Sun's own acceleration is bit for bit the same (which is why the
/// Sun receives nothing).
///
/// Verifies: PHY-002, NUM-009
#[test]
fn solar_gm_variation_is_the_newtonian_pull_of_delta_gm() {
    let (bodies, x, v) = fixture();
    let n = bodies.len();
    // Model epoch 3 Julian years after the reference epoch; evaluate 0.25 yr into the run.
    let law = SolarGmRate::inferred_j2000(RATE_SMALL).unwrap();
    let epoch = j2000_plus_days(3.0 * DAYS_PER_JULIAN_YEAR);
    let term = SolarGmVariation::<f64>::new(law, epoch);
    let t = 0.25 * YEAR_S;
    let offset = RATE_SMALL * (3.0 + 0.25);
    assert!((term.fractional_offset(t) - offset).abs() <= 2.0 * f64::EPSILON * offset.abs());
    let m = Model::new(&bodies, MoonVariables::Barycentric, vec![Box::new(term)]).unwrap();
    println!("{}", m.describe());
    assert!(m.describe().contains("solar-gm-rate-inferred"));
    assert!(m.describe().contains("inferred convention, OI-04"));
    let a = accel(&m, t, &x, &v);
    let sun = of(&x, n, 0);
    let mut worst = 0.0_f64;
    for (i, b) in bodies.iter().enumerate() {
        let ai = of(&a, n, i);
        if i == 0 || b.tier == Tier::Ring {
            assert_eq!(norm(ai).to_bits(), 0.0_f64.to_bits(), "body {i}");
            continue;
        }
        let pi = of(&x, n, i);
        let d = [pi[0] - sun[0], pi[1] - sun[1], pi[2] - sun[2]];
        let r = norm(d);
        let want = d.map(|c| -GM_SUN * offset * c / (r * r * r));
        let diff = norm([ai[0] - want[0], ai[1] - want[1], ai[2] - want[2]]) / norm(want);
        worst = worst.max(diff);
        // The rate is negative and the epoch is after t0: GM is smaller, the pull weaker, so the
        // term points away from the Sun.
        assert!(ai[0] * d[0] + ai[1] * d[1] + ai[2] * d[2] > 0.0, "body {i}");
    }
    println!("term vs independent evaluation: max relative difference {worst:.3e}");
    assert!(worst <= 16.0 * f64::EPSILON, "{worst:e}"); // lint: allow-literal(round-off budget of a dozen operations)
    // At the reference epoch (t = -3 yr from the model epoch) the term vanishes.
    let a0 = accel(&m, -3.0 * YEAR_S, &x, &v);
    assert!(a0.iter().all(|c| c.abs().to_bits() == 0.0_f64.to_bits()));

    // Against the difference of two constant-GM Newtonian models (solar GM x (1 + delta) and
    // solar GM), with an exaggerated delta so that the difference is resolved in binary64.
    let delta = 1.0e-3; // lint: allow-literal(synthetic exaggerated dGM/GM)
    let big = SolarGmRate::new(delta, epoch).unwrap();
    let newton = |gm_sun: f64| -> Model {
        let mut bs = bodies.clone();
        bs[0].gm = gm_sun;
        let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(MinorFromMajors::new(MinorReaction::On)),
            Box::new(RingMasses),
        ];
        Model::new(&bs, MoonVariables::Barycentric, terms).unwrap()
    };
    let (base, scaled) = (
        accel(&newton(GM_SUN), YEAR_S, &x, &v),
        accel(&newton(GM_SUN * (1.0 + delta)), YEAR_S, &x, &v),
    );
    let only = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(SolarGmVariation::<f64>::new(big, epoch))],
    )
    .unwrap();
    let term_acc = accel(&only, YEAR_S, &x, &v);
    for k in 0..3 {
        assert_eq!(
            base[k * n].to_bits(),
            scaled[k * n].to_bits(),
            "the Sun's acceleration does not depend on its own GM"
        );
    }
    let mut worst = 0.0_f64;
    for (i, body) in bodies.iter().enumerate().skip(1) {
        let (b, s, tm) = (of(&base, n, i), of(&scaled, n, i), of(&term_acc, n, i));
        let diff = [
            s[0] - b[0] - tm[0],
            s[1] - b[1] - tm[1],
            s[2] - b[2] - tm[2],
        ];
        if body.tier == Tier::Ring {
            assert_eq!(norm(tm).to_bits(), 0.0_f64.to_bits());
            continue;
        }
        worst = worst.max(norm(diff) / norm(tm));
    }
    println!("term vs difference of two Newtonian models: max relative difference {worst:.3e}");
    // The difference of two accelerations that agree to 1e-3 loses 3 digits: ~1e-13 expected.
    assert!(worst <= 1.0e-11, "{worst:e}"); // lint: allow-literal(cancellation budget: eps / delta x 50)
}

fn state(p: [f64; 3], w: [f64; 3]) -> State<Icrf, Barycentric> {
    let l = |z: f64| Length::from_km(z).unwrap();
    let s = |z: f64| Velocity::from_km_per_s(z).unwrap();
    State::new(
        Position::new(l(p[0]), l(p[1]), l(p[2])),
        [s(w[0]), s(w[1]), s(w[2])],
    )
}

/// Sun + a massless body on a circular orbit of radius `R0`, with or without the term.
fn two_body(term: Option<SolarGmVariation>, epoch: Epoch<Tdb>) -> NBodyPropagator<Icrf> {
    const R0: f64 = 5.8e7; // lint: allow-literal(orbit radius of GMSUN-RATE-ADIABATIC-SMA-REL, km)
    let bodies = [
        NBody {
            id: NaifId::SUN,
            gm: GM_SUN,
            tier: Tier::Major,
        },
        NBody {
            id: NaifId::MERCURY_BARYCENTER,
            gm: 0.0,
            tier: Tier::Major,
        },
    ];
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
    if let Some(t) = term {
        terms.push(Box::new(t));
    }
    let model = Model::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
    let mut states = SystemState::new();
    states
        .push(NaifId::SUN, &state([0.0; 3], [0.0; 3]))
        .unwrap();
    states
        .push(
            NaifId::MERCURY_BARYCENTER,
            &state([R0, 0.0, 0.0], [0.0, (GM_SUN / R0).sqrt(), 0.0]),
        )
        .unwrap();
    NBodyPropagator::new(
        model,
        &states,
        None,
        epoch,
        TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap(),
        Ias15Config::default(),
    )
    .unwrap()
}

/// Osculating semi-major axis (km) of the body about the Sun for central parameter `gm`.
fn semi_major_axis(p: &NBodyPropagator<Icrf>, gm: f64) -> f64 {
    let s = p.state().unwrap();
    let raw = |id: NaifId| {
        let st = s.state(id).unwrap();
        (
            st.position().components().map(Length::km),
            st.velocity().map(Velocity::km_per_s),
        )
    };
    let ((ps, vs), (pb, vb)) = (raw(NaifId::SUN), raw(NaifId::MERCURY_BARYCENTER));
    let r = norm([pb[0] - ps[0], pb[1] - ps[1], pb[2] - ps[2]]);
    let v = norm([vb[0] - vs[0], vb[1] - vs[1], vb[2] - vs[2]]);
    1.0 / (2.0 / r - v * v / gm)
}

/// The two-body secular effect against the analytic adiabatic law: for a slowly varying central
/// GM a circular orbit keeps `a·GM` (da/dt = −a·GMdot/GM). Pre-registered bound
/// `GMSUN-RATE-ADIABATIC-SMA-REL` (derivation and justification in the vector).
///
/// Verifies: PHY-002
#[test]
fn two_body_semi_major_axis_follows_the_adiabatic_law() {
    let epoch = Epoch::<Tdb>::j2000();
    let law = SolarGmRate::new(RATE_ADIABATIC, epoch).unwrap();
    let mut p = two_body(Some(SolarGmVariation::new(law, epoch)), epoch);
    let mut control = two_body(None, epoch);
    let gm0 = GravParam::from_km3_per_s2(GM_SUN).unwrap();
    let a0 = semi_major_axis(&p, GM_SUN);
    let years = 10_u32;
    let (mut worst, mut signal, mut control_drift) = (0.0_f64, 0.0_f64, 0.0_f64);
    for y in 1..=years {
        let e = j2000_plus_days(f64::from(y) * DAYS_PER_JULIAN_YEAR);
        p.advance_to(e).unwrap();
        control.advance_to(e).unwrap();
        let gm = law.gm_at(gm0, e).unwrap().km3_per_s2();
        let a = semi_major_axis(&p, gm);
        let dev = (a * gm / (a0 * GM_SUN) - 1.0).abs();
        signal = a / a0 - 1.0;
        control_drift = (semi_major_axis(&control, GM_SUN) / a0 - 1.0).abs();
        println!(
            "t = {y:>2} yr: a/a0 - 1 = {signal:+.6e} (adiabatic {:+.6e}), |a GM/(a0 GM0) - 1| = {dev:.3e}; without the term |a/a0 - 1| = {control_drift:.3e}",
            GM_SUN / gm - 1.0
        );
        worst = worst.max(dev);
    }
    println!(
        "max |a GM/(a0 GM0) - 1| over {years} yr = {worst:.3e}; steps {}",
        p.stats().steps
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("GMSUN-RATE-ADIABATIC-SMA-REL")
        .unwrap()
        .check(worst)
        .unwrap();
    // The check has something to see: the orbit widened by |k| T = 1e-5, and only with the term.
    let expected = -RATE_ADIABATIC * f64::from(years);
    assert!(signal > 0.5 * expected && signal < 2.0 * expected);
    assert!(control_drift < 1.0e-3 * expected); // lint: allow-literal(the control drift is round-off, far below the signal)
}

/// The term's time origin is enforced, and the constant-mass energy diagnostic is unavailable
/// with it (the model is explicitly time dependent).
///
/// Verifies: PHY-002, API-002
#[test]
fn time_origin_is_enforced_and_the_energy_diagnostic_is_unavailable() {
    let epoch = Epoch::<Tdb>::j2000();
    let law = SolarGmRate::inferred_j2000(RATE_SMALL).unwrap();
    let with = two_body(Some(SolarGmVariation::new(law, epoch)), epoch);
    let without = two_body(None, epoch);
    assert_eq!(with.model().time_origin(), Some(epoch));
    assert_eq!(without.model().time_origin(), None);
    assert!(without.energy_over_g().is_some());
    assert!(with.energy_over_g().is_none());
    assert!(with.momentum_over_g().is_some());

    // A term built for another epoch than the propagation's start is refused.
    let (bodies, _, _) = fixture();
    let later = j2000_plus_days(1.0);
    let model = Model::new(
        &bodies[..2],
        MoonVariables::Barycentric,
        vec![
            Box::new(NewtonianMutual),
            Box::new(SolarGmVariation::<f64>::new(law, later)),
        ],
    )
    .unwrap();
    let mut states = SystemState::new();
    states
        .push(NaifId::SUN, &state([0.0; 3], [0.0; 3]))
        .unwrap();
    states
        .push(
            NaifId::MERCURY_BARYCENTER,
            &state([5.0e7, 0.0, 0.0], [0.0, 50.0, 0.0]), // lint: allow-literal(structural state)
        )
        .unwrap();
    let e = NBodyPropagator::new(
        model,
        &states,
        None,
        epoch,
        TimeSpan::from_seconds(SECONDS_PER_DAY).unwrap(),
        Ias15Config::default(),
    )
    .unwrap_err();
    println!("{e}");
    assert!(e.to_string().contains("FA-E0500"), "{e}");
    assert!(e.to_string().contains("time origin"), "{e}");

    // Two time-dependent terms with different origins cannot share a model.
    let e = Model::new(
        &bodies[..2],
        MoonVariables::Barycentric,
        vec![
            Box::new(SolarGmVariation::<f64>::new(law, epoch)),
            Box::new(SolarGmVariation::<f64>::new(law, later)),
        ],
    )
    .unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(e.to_string().contains("time origins"), "{e}");

    // The term needs a Major Sun.
    let e = Model::new(
        &bodies[1..3],
        MoonVariables::Barycentric,
        vec![Box::new(SolarGmVariation::<f64>::new(law, epoch))],
    )
    .unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(e.to_string().contains("inferred"), "{e}");
}
