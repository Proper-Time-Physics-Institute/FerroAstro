// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Unit tests of the N-body terms on synthetic configurations (WP12 plan U2, U3 and structure).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              bitwise identities are asserted on purpose; reference arithmetic is written plainly"
)]

use super::*;
use crate::central::{CentralTerm, Pole};
use crate::{ForceModel, Gauge, SolarJ2};
use ferroastro_core::ErrorCode;
use ferroastro_core::frames::{Barycentric, Geocentric, Icrf, Position, State};
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_integrators::SecondOrderSystem;
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_system::{EarthMoon, SystemState, Tier};
use ferroastro_vectors::VectorSet;

type Model<R = f64> = NBodyModel<Icrf, R>;

fn body<R: Real>(id: NaifId, gm: f64, tier: Tier) -> NBody<R> {
    NBody {
        id,
        gm: R::from_f64(gm),
        tier,
    }
}

/// A structural 6-body fixture (no physics): Sun, two planets, Earth, Moon, one more planet.
/// GMs and positions are arbitrary but solar-system-like in scale. Flat layout component-major.
fn fixture() -> (Vec<NBody>, Vec<f64>, Vec<f64>) {
    let bodies = vec![
        body(NaifId::SUN, 1.3e11, Tier::Major), // lint: allow-literal(structural fixture)
        body(NaifId::MERCURY_BARYCENTER, 2.2e4, Tier::Major), // lint: allow-literal(structural fixture)
        body(NaifId::VENUS_BARYCENTER, 3.2e5, Tier::Major), // lint: allow-literal(structural fixture)
        body(NaifId::EARTH, 4.0e5, Tier::Major), // lint: allow-literal(structural fixture)
        body(NaifId::MOON, 4.9e3, Tier::Major),  // lint: allow-literal(structural fixture)
        body(NaifId::JUPITER_BARYCENTER, 1.3e8, Tier::Major), // lint: allow-literal(structural fixture)
    ];
    // Positions (km) per body, then laid out component-major.
    let p: [[f64; 3]; 6] = [
        [-7.0e5, 2.0e5, 1.0e4],  // lint: allow-literal(structural fixture)
        [5.0e7, -2.0e7, 3.0e6],  // lint: allow-literal(structural fixture)
        [-6.0e7, 8.0e7, -1.0e6], // lint: allow-literal(structural fixture)
        [1.2e8, 8.0e7, 3.0e7],   // lint: allow-literal(structural fixture)
        [1.2e8 + 3.0e5, 8.0e7 - 2.2e5, 3.0e7 + 1.1e5], // lint: allow-literal(structural fixture)
        [5.0e8, -5.5e8, -2.0e8], // lint: allow-literal(structural fixture)
    ];
    let w: [[f64; 3]; 6] = [
        [0.01, -0.012, 0.0003], // lint: allow-literal(structural fixture)
        [20.0, 40.0, 3.0],      // lint: allow-literal(structural fixture)
        [-25.0, -20.0, 1.0],    // lint: allow-literal(structural fixture)
        [-17.0, 24.0, 10.0],    // lint: allow-literal(structural fixture)
        [-17.5, 24.8, 10.3],    // lint: allow-literal(structural fixture)
        [9.0, 8.0, -3.0],       // lint: allow-literal(structural fixture)
    ];
    let flat = |a: [[f64; 3]; 6]| -> Vec<f64> { (0..3).flat_map(|k| a.map(|b| b[k])).collect() };
    (bodies, flat(p), flat(w))
}

fn eps() -> f64 {
    f64::EPSILON
}

fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

fn accel<R: Real>(m: &Model<R>, x: &[R], v: &[R]) -> Vec<R> {
    let mut a = vec![R::ZERO; x.len()];
    m.acceleration(R::ZERO, x, v, &mut a).unwrap();
    a
}

fn pole() -> Pole<Icrf> {
    Pole::from_components([0.12, -0.4, 0.9]).unwrap() // lint: allow-literal(structural pole)
}

fn j2_term() -> SolarJ2WithReaction<Icrf> {
    // lint: allow-literal(synthetic J2 = 1e-3 and radius, WP12 plan U2)
    SolarJ2WithReaction::new(1e-3, Length::from_km(7.0e5).unwrap(), pole()).unwrap()
}

/// U2: the solar J2 term with its reaction leaves Σ μᵢ aᵢ = 0 to round-off, and each body's
/// acceleration equals the single-body `SolarJ2` field at `rᵢ − r_Sun` bit for bit.
///
/// Verifies: NUM-019
#[test]
fn solar_j2_reaction_balances_momentum() {
    let (bodies, x, v) = fixture();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(j2_term())];
    let m = Model::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
    let a = accel(&m, &x, &v);
    let n = bodies.len();
    let mut sum = [0.0_f64; 3];
    let mut scale = 0.0;
    for (i, b) in bodies.iter().enumerate() {
        let ai = [a[i], a[n + i], a[2 * n + i]];
        for k in 0..3 {
            sum[k] += b.gm * ai[k];
        }
        scale += b.gm * ai[0].hypot(ai[1]).hypot(ai[2]);
    }
    let ulps = sum[0].hypot(sum[1]).hypot(sum[2]) / (eps() * scale);
    println!("U2: |sum GM_i a_i| / (eps sum GM_i |a_i|) = {ulps:.3}");
    vectors()
        .get("NBODY-J2-REACTION-MOMENTUM-ULPS")
        .unwrap()
        .check(ulps)
        .unwrap();
    // Body 1 (not the Sun) equals the single-body term evaluated at r_1 - r_Sun.
    let gm_sun = GravParam::from_km3_per_s2(bodies[0].gm).unwrap();
    let single = SolarJ2::new(gm_sun, 1e-3, Length::from_km(7.0e5).unwrap(), pole()).unwrap(); // lint: allow-literal(same synthetic J2 and radius)
    let r = [x[1] - x[0], x[n + 1] - x[n], x[2 * n + 1] - x[2 * n]];
    let mut want = [0.0; 3];
    single.add_acceleration(r, [0.0; 3], &mut want);
    assert_eq!([a[1], a[n + 1], a[2 * n + 1]], want);
    // The reaction mutant (no Sun term) breaks the balance by orders of magnitude.
    assert!(a[0] != 0.0 && a[n] != 0.0);
}

/// `(d − ρ)/|d − ρ|³ − d/d³` directly, in double-double.
fn direct_dd(d: [f64; 3], rho: [f64; 3]) -> [f64; 3] {
    let dd = |z: f64| DoubleDouble::from_f64(z);
    let d = d.map(dd);
    let rho = rho.map(dd);
    let dm: [DoubleDouble; 3] = [d[0] - rho[0], d[1] - rho[1], d[2] - rho[2]];
    let n3 = |z: [DoubleDouble; 3]| {
        let r2 = z[0] * z[0] + z[1] * z[1] + z[2] * z[2];
        r2 * r2.sqrt()
    };
    let (a, b) = (n3(dm), n3(d));
    [0, 1, 2].map(|k| (dm[k] / a - d[k] / b).to_f64())
}

/// U3: the 12-term expansion equals the double-double direct difference to a few ulp of the
/// tidal term for Sun-, Venus- and Jupiter-like geometries; 4 terms and the closed form are
/// reported.
///
/// Verifies: NUM-008
#[test]
fn geocentric_moon_expansion_matches_double_double_direct_difference() {
    let vs = vectors();
    let bound = vs.get("NBODY-MOON-TAYLOR-VS-DD-ULPS").unwrap();
    let t12 = GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap();
    let t4 = GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_MIN).unwrap();
    // Structural geometries (not physics): third body at ~1 au, ~0.27 au and ~5.2 au from the
    // Earth, Moon at ~3.8e5 km in a skew direction.
    let rho = [2.1e5, -2.9e5, 1.3e5]; // lint: allow-literal(structural geometry)
    let cases = [
        ("Sun-like", [-1.1e8, -9.0e7, -3.5e7]), // lint: allow-literal(structural geometry)
        ("Venus-at-closest-like", [3.0e7, 2.4e7, 1.0e7]), // lint: allow-literal(structural geometry)
        ("Jupiter-like", [6.0e8, -4.0e8, -1.6e8]), // lint: allow-literal(structural geometry)
        ("Sun-aligned-with-rho", rho.map(|z| z * 389.0)), // lint: allow-literal(structural geometry: q ~ -5e-3)
    ];
    let mut worst = 0.0_f64;
    for (name, d) in cases {
        let truth = direct_dd(d, rho);
        let tn = truth[0].hypot(truth[1]).hypot(truth[2]);
        let err = |got: [f64; 3]| {
            (0..3)
                .map(|k| (got[k] - truth[k]).abs())
                .fold(0.0, f64::max)
                / (eps() * tn)
        };
        let e12 = err(t12.differential(d, rho));
        let e4 = err(t4.differential(d, rho));
        // Direct f64 difference, for comparison.
        let dm = [d[0] - rho[0], d[1] - rho[1], d[2] - rho[2]];
        let n3 = |z: [f64; 3]| {
            let r2 = z[0] * z[0] + z[1] * z[1] + z[2] * z[2];
            r2 * r2.sqrt()
        };
        let direct = [0, 1, 2].map(|k| dm[k] / n3(dm) - d[k] / n3(d));
        let ed = err(direct);
        let q = (rho.iter().map(|z| z * z).sum::<f64>()
            - 2.0 * rho.iter().zip(&d).map(|(a, b)| a * b).sum::<f64>())
            / d.iter().map(|z| z * z).sum::<f64>();
        let closed = t12.f_of_q(1.0) - (2.0_f64.powf(-1.5) - 1.0); // lint: allow-literal(closed-form branch check at q = 1)
        println!(
            "U3 {name:<22} q = {q:+.3e}: 12 terms {e12:.3} eps, 4 terms {e4:.3e} eps, direct f64 {ed:.3e} eps (of |tidal|); closed-form branch at q=1 off by {closed:.1e}"
        );
        assert!(closed.abs() < 4.0 * eps());
        worst = worst.max(e12);
    }
    bound.check(worst).unwrap();
}

/// Tiers (NUM-009): Minor bodies feel the Majors only and not each other; Ring bodies never
/// move and attract the moving bodies; the reaction switch controls the Minor pull on Majors.
///
/// Verifies: NUM-009
#[test]
fn tiers_follow_num009() {
    let ids = [
        NaifId(10),
        NaifId(2_000_001),
        NaifId(2_000_002),
        NaifId(2_000_003),
    ];
    let mk = |reaction| {
        let bodies = vec![
            body(ids[0], 1.0, Tier::Major),
            body(ids[1], 0.5, Tier::Minor),
            body(ids[2], 0.25, Tier::Minor),
            body(ids[3], 2.0, Tier::Ring),
        ];
        let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(MinorFromMajors::new(reaction)),
            Box::new(RingMasses),
        ];
        Model::new(&bodies, MoonVariables::Barycentric, terms).unwrap()
    };
    // Bodies on the x axis at 0, 1, 3, 7 (exact powers keep the expected values exact).
    let x = [0.0, 1.0, 3.0, 7.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let v = [0.0; 12];
    let a = accel(&mk(MinorReaction::On), &x, &v);
    // Minor 1 at x = 1: pulled to the Sun (−1/1) and to the ring mass (+2/36); not by Minor 2.
    assert!((a[1] - (-1.0 + 2.0 / 36.0)).abs() < 4.0 * eps());
    // Minor 2 at x = 3: −1/9 + 2/16.
    assert!((a[2] - (-1.0 / 9.0 + 2.0 / 16.0)).abs() < 4.0 * eps());
    // Sun: +0.5/1 + 0.25/9 (reaction) + 2/49 (ring).
    assert!((a[0] - (0.5 + 0.25 / 9.0 + 2.0 / 49.0)).abs() < 4.0 * eps());
    // The ring body never moves.
    assert_eq!([a[3], a[7], a[11]], [0.0; 3]);
    let off = accel(&mk(MinorReaction::Off), &x, &v);
    assert!((off[0] - 2.0 / 49.0).abs() < 4.0 * eps());
    assert_eq!(off[1], a[1]);
}

/// The geocentric formulation's accelerations equal those of the barycentric formulation,
/// recombined: EMB = `f_E a_E + f_M a_M` and Moon − Earth = `a_M − a_E` (the latter from a
/// double-double barycentric evaluation, the truth for the cancelling difference).
///
/// Verifies: NUM-008
#[test]
fn geocentric_accelerations_equal_recombined_barycentric_ones() {
    let (bodies, x, v) = fixture();
    let emrat = bodies[3].gm / bodies[4].gm;
    let geo_terms = || -> Vec<Box<dyn ForceTerm<Icrf>>> {
        vec![
            Box::new(NewtonianMutual),
            Box::new(GeocentricMoon::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
            Box::new(j2_term()),
        ]
    };
    let geo = Model::new(&bodies, MoonVariables::Geocentric { emrat }, geo_terms()).unwrap();
    // Barycentric double-double model as the truth.
    let bodies_dd: Vec<NBody<DoubleDouble>> =
        bodies.iter().map(|b| body(b.id, b.gm, b.tier)).collect();
    let j2_dd = SolarJ2WithReaction::new(
        DoubleDouble::from_f64(1e-3), // lint: allow-literal(same synthetic J2)
        Length::from_km(DoubleDouble::from_f64(7.0e5)).unwrap(), // lint: allow-literal(same synthetic radius)
        Pole::from_components(pole().unit().map(DoubleDouble::from_f64)).unwrap(),
    )
    .unwrap();
    let bary_terms: Vec<Box<dyn ForceTerm<Icrf, DoubleDouble>>> =
        vec![Box::new(NewtonianMutual), Box::new(j2_dd)];
    let bary =
        Model::<DoubleDouble>::new(&bodies_dd, MoonVariables::Barycentric, bary_terms).unwrap();
    let n = bodies.len();
    let (xd, vd): (Vec<DoubleDouble>, Vec<DoubleDouble>) = (
        x.iter().copied().map(DoubleDouble::from_f64).collect(),
        v.iter().copied().map(DoubleDouble::from_f64).collect(),
    );
    let ab = accel(&bary, &xd, &vd);
    // Geocentric variables from the same physical state.
    let (e, m) = (3, 4);
    let fe = emrat / (1.0 + emrat);
    let fm = 1.0 / (1.0 + emrat);
    let mut xg = x.clone();
    let mut vg = v.clone();
    for k in 0..3 {
        let (pe, pm) = (x[k * n + e], x[k * n + m]);
        xg[k * n + e] = fe * pe + fm * pm;
        xg[k * n + m] = pm - pe;
        let (we, wm) = (v[k * n + e], v[k * n + m]);
        vg[k * n + e] = fe * we + fm * wm;
        vg[k * n + m] = wm - we;
    }
    let ag = accel(&geo, &xg, &vg);
    let mut worst_rel = 0.0_f64;
    let mut worst_emb = 0.0_f64;
    let mut worst_other = 0.0_f64;
    for k in 0..3 {
        let rel_truth = (ab[k * n + m] - ab[k * n + e]).to_f64();
        let emb_truth = (ab[k * n + e] * DoubleDouble::from_f64(fe)
            + ab[k * n + m] * DoubleDouble::from_f64(fm))
        .to_f64();
        worst_rel = worst_rel.max((ag[k * n + m] - rel_truth).abs() / rel_truth.abs());
        worst_emb = worst_emb.max((ag[k * n + e] - emb_truth).abs() / emb_truth.abs());
        for i in [0, 1, 2, 5] {
            let t = ab[k * n + i].to_f64();
            worst_other = worst_other.max((ag[k * n + i] - t).abs() / t.abs());
        }
    }
    println!(
        "geocentric vs DD barycentric: Moon-Earth rel. {worst_rel:.3e}, EMB {worst_emb:.3e}, others {worst_other:.3e}"
    );
    // The geocentric state was rounded from the barycentric one (|x| ~ 1e8 km), so the
    // reconstructed Earth/Moon positions carry ~eps |x| errors; the bound is that size relative
    // to the Earth-Moon distance (|x|/|rho| ~ 400) times a small factor.
    assert!(worst_rel < 1e4 * eps(), "{worst_rel}"); // lint: allow-literal(round-off budget: |x|/|rho| ~ 4e2 times 25 ulp)
    assert!(worst_emb < 1e3 * eps(), "{worst_emb}"); // lint: allow-literal(round-off budget)
    assert!(worst_other < 1e3 * eps(), "{worst_other}"); // lint: allow-literal(round-off budget)
}

/// Compensated accumulation agrees with plain accumulation to a few ulp; the model is
/// Newtonian-gauged and describes itself.
///
/// Verifies: NUM-009, API-002
#[test]
fn compensated_accumulation_and_description() {
    let (bodies, x, v) = fixture();
    let mk = || -> Vec<Box<dyn ForceTerm<Icrf>>> {
        vec![Box::new(NewtonianMutual), Box::new(j2_term())]
    };
    let plain = Model::new(&bodies, MoonVariables::Barycentric, mk()).unwrap();
    let comp = Model::new(&bodies, MoonVariables::Barycentric, mk())
        .unwrap()
        .with_compensation(true);
    let (a, b) = (accel(&plain, &x, &v), accel(&comp, &x, &v));
    for (p, q) in a.iter().zip(&b) {
        assert!((p - q).abs() <= 8.0 * eps() * p.abs().max(q.abs())); // lint: allow-literal(round-off budget)
    }
    assert_eq!(plain.gauge(), Gauge::Newtonian);
    let d = comp.describe();
    assert!(
        d.contains("Neumaier") && d.contains("solar-j2-with-reaction") && d.contains("J2 = 0.001")
    );
    assert_eq!(plain.dimension(), 3 * bodies.len());
}

/// Invalid models are refused with coded errors (API-002).
///
/// Verifies: API-002, NUM-008
#[test]
fn invalid_models_are_coded_errors() {
    let (bodies, x, v) = fixture();
    let emrat = 81.0; // lint: allow-literal(structural fixture)
    let code = |r: Result<Model, crate::ForceError>| r.unwrap_err().code();
    // A geocentric pair without the Moon term, and the Moon term without a pair.
    assert_eq!(
        code(Model::new(
            &bodies,
            MoonVariables::Geocentric { emrat },
            vec![Box::new(NewtonianMutual)]
        )),
        ErrorCode::ForceModelInvalid
    );
    assert_eq!(
        code(Model::new(
            &bodies,
            MoonVariables::Barycentric,
            vec![Box::new(GeocentricMoon::new(12).unwrap())]
        )),
        ErrorCode::ForceModelInvalid
    );
    // NUM-008: fewer than 4 Taylor terms.
    assert_eq!(
        GeocentricMoon::<f64>::new(3).unwrap_err().code(),
        ErrorCode::ForceModelInvalid
    );
    // J2 without a Sun; duplicated id; negative GM; no bodies.
    let no_sun: Vec<NBody> = bodies.iter().skip(1).copied().collect();
    assert_eq!(
        code(Model::new(
            &no_sun,
            MoonVariables::Barycentric,
            vec![Box::new(j2_term())]
        )),
        ErrorCode::ForceModelInvalid
    );
    let mut dup = bodies.clone();
    dup[1].id = NaifId::SUN;
    assert_eq!(
        code(Model::new(&dup, MoonVariables::Barycentric, vec![])),
        ErrorCode::ForceModelInvalid
    );
    let mut neg = bodies.clone();
    neg[2].gm = -1.0;
    assert_eq!(
        code(Model::new(&neg, MoonVariables::Barycentric, vec![])),
        ErrorCode::ForceModelInvalid
    );
    assert_eq!(
        code(Model::new(&[], MoonVariables::Barycentric, vec![])),
        ErrorCode::ForceModelInvalid
    );
    // Coincident bodies are a singular configuration.
    let m = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutual)],
    )
    .unwrap();
    let mut xs = x.clone();
    let n = bodies.len();
    for k in 0..3 {
        xs[k * n + 2] = xs[k * n + 1];
    }
    let mut a = vec![0.0; x.len()];
    assert_eq!(
        m.acceleration(0.0, &xs, &v, &mut a).unwrap_err().code(),
        ErrorCode::ForceModelSingular
    );
    assert!(m.acceleration(0.0, &x[1..], &v, &mut a).is_err());
}

/// Pack/unpack round trips (API-001): exact for ordinary bodies and for the DE Earth–Moon pair.
///
/// Verifies: API-001, NUM-008
#[test]
fn pack_and_unpack_round_trip() {
    let (bodies, x, v) = fixture();
    let n = bodies.len();
    let emrat = bodies[3].gm / bodies[4].gm;
    let m = Model::new(
        &bodies,
        MoonVariables::Geocentric { emrat },
        vec![
            Box::new(NewtonianMutual),
            Box::new(GeocentricMoon::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        ],
    )
    .unwrap();
    let st = |i: usize| -> State<Icrf, Barycentric> {
        let l = |z: f64| Length::from_km(z).unwrap();
        let w = |z: f64| Velocity::from_km_per_s(z).unwrap();
        State::new(
            Position::new(l(x[i]), l(x[n + i]), l(x[2 * n + i])),
            [w(v[i]), w(v[n + i]), w(v[2 * n + i])],
        )
    };
    let mut sys = SystemState::<Icrf, Barycentric>::new();
    for (i, b) in bodies.iter().enumerate() {
        sys.push(b.id, &st(i)).unwrap();
    }
    let pair = EarthMoon::from_bodies(&st(3), &st(4), emrat);
    let (xp, vp) = m.pack(&sys, Some(&pair)).unwrap();
    assert_eq!(m.earth_moon(&xp, &vp).unwrap(), pair);
    let back = m.unpack(&xp, &vp).unwrap();
    for i in [0, 1, 2, 5] {
        assert_eq!(back.state_at(i).unwrap(), st(i));
    }
    let (e, mo) = pair.split(emrat);
    assert_eq!(back.state(NaifId::EARTH).unwrap(), e);
    assert_eq!(back.state(NaifId::MOON).unwrap(), mo);
    let _: Option<State<Icrf, Geocentric>> = m.earth_moon(&xp, &vp).map(|p| p.moon);
    // Missing body.
    let mut short = SystemState::<Icrf, Barycentric>::new();
    short.push(NaifId::SUN, &st(0)).unwrap();
    assert!(m.pack(&short, None).is_err());
}

type Dd = DoubleDouble;

fn dd(a: &[f64]) -> Vec<Dd> {
    a.iter().copied().map(Dd::from_f64).collect()
}

fn j2_term_dd() -> SolarJ2WithReaction<Icrf, Dd> {
    SolarJ2WithReaction::new(
        Dd::from_f64(1e-3), // lint: allow-literal(same synthetic J2)
        Length::from_km(Dd::from_f64(7.0e5)).unwrap(), // lint: allow-literal(same synthetic radius)
        Pole::from_components(pole().unit().map(Dd::from_f64)).unwrap(),
    )
    .unwrap()
}

/// `|Σ μᵢ vᵢ·aᵢ + dU/dt| / Σ |μᵢ vᵢ·aᵢ|` for a barycentric model, all in double-double (the
/// kinetic energy dwarfs a single term's potential rate, so f64 would measure round-off): the
/// power of the forces must equal minus the rate of the model's potential (`energy_over_g` with
/// the velocities held, so the kinetic part cancels), the rate taken by a central difference
/// along `v`.
fn power_mismatch(m: &Model<Dd>, x: &[f64], v: &[f64]) -> f64 {
    let (x, v) = (dd(x), dd(v));
    let a = accel(m, &x, &v);
    let n = m.layout().len();
    let (mut p, mut scale) = (Dd::ZERO, Dd::ZERO);
    for i in 0..n {
        let g = m.layout().gm(i);
        let mut pi = Dd::ZERO;
        for k in 0..3 {
            pi += g * v[k * n + i] * a[k * n + i];
        }
        p += pi;
        scale += pi.abs();
    }
    let h = Dd::from_f64(10.0); // lint: allow-literal(finite-difference step, s)
    let shift = |s: Dd| -> Vec<Dd> { x.iter().zip(&v).map(|(&xi, &vi)| xi + s * vi).collect() };
    let e = |xs: &[Dd]| m.energy_over_g(xs, &v).unwrap();
    let du = (e(&shift(h)) - e(&shift(-h))) / (Dd::TWO * h);
    ((p + du).abs() / scale).to_f64()
}

/// The energy diagnostic (NUM-023) is consistent with the forces of every term: for each term
/// alone the force power balances the rate of its potential, and the geocentric-pair energy
/// equals the barycentric one for the same physical state. Before the WP12 review a wrong
/// sign of the J2 potential, or a Minor/Ring potential inconsistent with its force, left every
/// PR-tier test green (the energy is only used by the nightly Brouwer ensemble).
///
/// Verifies: NUM-023, NUM-019, NUM-009
#[test]
fn energy_diagnostic_is_consistent_with_the_forces() {
    let (bodies, x, v) = fixture();
    let n = bodies.len();
    let bodies_dd: Vec<NBody<Dd>> = bodies.iter().map(|b| body(b.id, b.gm, b.tier)).collect();
    // Central-difference truncation ~ (h |v| / r)^2 ~ 1e-13 (h = 10 s); double-double round-off
    // is far below.
    let tol = 1e-9; // lint: allow-literal(finite-difference budget)
    let one = |t: Box<dyn ForceTerm<Icrf, Dd>>| {
        Model::<Dd>::new(&bodies_dd, MoonVariables::Barycentric, vec![t]).unwrap()
    };
    let newton = power_mismatch(&one(Box::new(NewtonianMutual)), &x, &v);
    let j2 = power_mismatch(&one(Box::new(j2_term_dd())), &x, &v);
    // Tiers: Mercury Minor, Venus a Ring mass at rest.
    let mut tiered = bodies_dd.clone();
    tiered[1].tier = Tier::Minor;
    tiered[2].tier = Tier::Ring;
    let mut vr = v.clone();
    for k in 0..3 {
        vr[k * n + 2] = 0.0;
    }
    let tier_model = |t: Box<dyn ForceTerm<Icrf, Dd>>| {
        Model::<Dd>::new(&tiered, MoonVariables::Barycentric, vec![t]).unwrap()
    };
    let minor = power_mismatch(
        &tier_model(Box::new(MinorFromMajors::new(MinorReaction::On))),
        &x,
        &vr,
    );
    let ring = power_mismatch(&tier_model(Box::new(RingMasses)), &x, &vr);
    println!(
        "power balance |P + dU/dt| / sum|P_i| (DD): Newtonian {newton:.3e}, J2 {j2:.3e}, Minor {minor:.3e}, Ring {ring:.3e}"
    );
    for (name, r) in [
        ("Newtonian", newton),
        ("J2", j2),
        ("Minor", minor),
        ("Ring", ring),
    ] {
        assert!(r < tol, "{name}: {r:e}");
    }
    // Geocentric pair: the same physical state gives the same energy.
    let emrat = bodies[3].gm / bodies[4].gm;
    let terms = || -> Vec<Box<dyn ForceTerm<Icrf>>> {
        vec![
            Box::new(NewtonianMutual),
            Box::new(GeocentricMoon::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
            Box::new(j2_term()),
        ]
    };
    let geo = Model::new(&bodies, MoonVariables::Geocentric { emrat }, terms()).unwrap();
    let bary = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutual), Box::new(j2_term())],
    )
    .unwrap();
    let (fe, fm) = (emrat / (1.0 + emrat), 1.0 / (1.0 + emrat));
    let (mut xg, mut vg) = (x.clone(), v.clone());
    for k in 0..3 {
        let (pe, pm) = (x[k * n + 3], x[k * n + 4]);
        xg[k * n + 3] = fe * pe + fm * pm;
        xg[k * n + 4] = pm - pe;
        let (we, wm) = (v[k * n + 3], v[k * n + 4]);
        vg[k * n + 3] = fe * we + fm * wm;
        vg[k * n + 4] = wm - we;
    }
    let (eg, eb) = (
        geo.energy_over_g(&xg, &vg).unwrap(),
        bary.energy_over_g(&x, &v).unwrap(),
    );
    let rel = (eg - eb).abs() / eb.abs();
    println!("geocentric vs barycentric energy: relative difference {rel:.3e}");
    assert!(rel < 1e-12, "{rel:e}"); // lint: allow-literal(round-off budget of the state conversion)
}
