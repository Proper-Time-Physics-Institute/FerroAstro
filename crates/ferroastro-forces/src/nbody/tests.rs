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

/// R13-CODE-2/3 (PR #13 review): a bad body index or a short state is a coded error or `None`,
/// never the origin or a silent zero.
///
/// Verifies: API-002
#[test]
fn view_and_diagnostics_refuse_bad_indices_and_lengths() {
    let (bodies, x, v) = fixture();
    let m = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutual)],
    )
    .unwrap();
    let layout = Layout::new(&bodies, MoonVariables::Barycentric).unwrap();
    let view = View::new(&layout, &x, &v);
    let n = bodies.len();
    assert_eq!(view.position(1).unwrap(), [x[1], x[n + 1], x[2 * n + 1]]);
    for i in [n, n + 1, usize::MAX] {
        assert_eq!(
            view.position(i).unwrap_err().code(),
            ErrorCode::ForceModelInvalid
        );
        assert_eq!(
            view.velocity(i).unwrap_err().code(),
            ErrorCode::ForceModelInvalid
        );
    }
    // A view over a short slice refuses every body instead of reading zeros.
    let short = View::new(&layout, &x[1..], &v);
    assert!(short.position(0).is_err());
    assert!(m.momentum_over_g(&x, &v).is_some());
    assert!(m.momentum_over_g(&x[1..], &v).is_none());
    assert!(m.momentum_over_g(&x, &v[..3]).is_none());
    assert!(m.energy_over_g(&x[1..], &v).is_none());
    let mut a = vec![0.0; x.len()];
    assert!(m.acceleration(0.0, &x, &v[1..], &mut a).is_err());
    assert!(m.acceleration(0.0, &x, &v, &mut a[1..]).is_err());
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

fn lt_term(gamma: f64, reaction: LtReaction) -> SolarLenseThirring<Icrf> {
    use ferroastro_core::units::SpinGm;
    SolarLenseThirring::new(
        SpinGm::from_km5_per_s3(1.27e16).unwrap(), // lint: allow-literal(synthetic G S of solar order, WP20 plan L1)
        Velocity::from_km_per_s(3.0e5).unwrap(),   // lint: allow-literal(synthetic c, WP20 plan L1)
        gamma,
        pole(),
        reaction,
    )
    .unwrap()
}

fn lt_model(gamma: f64, reaction: LtReaction) -> (Vec<NBody>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let (bodies, x, v) = fixture();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(lt_term(gamma, reaction))];
    let m = Model::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
    assert_eq!(m.gauge(), Gauge::PpnIsotropicHarmonic);
    let a = accel(&m, &x, &v);
    (bodies, x, v, a)
}

/// WP20 L1a, L1c: the Lense–Thirring term with its reaction leaves Σ μᵢ aᵢ = 0 to round-off;
/// without it the Sun gets exactly nothing and the other bodies are unchanged bit for bit.
///
/// Verifies: NUM-019
#[test]
fn solar_lense_thirring_reaction_balances_momentum() {
    for gamma in [1.0, 0.999] {
        // lint: allow-literal(PPN test points)
        let (bodies, _, _, a) = lt_model(gamma, LtReaction::On);
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
        println!("L1a (gamma = {gamma}): |sum GM_i a_i| / (eps sum GM_i |a_i|) = {ulps:.3}");
        vectors()
            .get("NBODY-LT-REACTION-MOMENTUM-ULPS")
            .unwrap()
            .check(ulps)
            .unwrap();
        assert!(a[0] != 0.0 || a[n] != 0.0 || a[2 * n] != 0.0);
        let (_, _, _, off) = lt_model(gamma, LtReaction::Off);
        assert_eq!([off[0], off[n], off[2 * n]], [0.0; 3]);
        for i in 1..n {
            for k in 0..3 {
                assert_eq!(off[k * n + i].to_bits(), a[k * n + i].to_bits());
            }
        }
    }
}

/// WP20 L1b: DE440 Eqs. 37–38 (2Ω×v, the N-body term) equal the Journal #4 R3b form (the
/// single-body [`crate::LenseThirring`]) at every body's heliocentric state, γ = 1 and 0.999.
///
/// Verifies: NUM-019, PHY-011
#[test]
fn solar_lense_thirring_equals_the_r3b_form() {
    use ferroastro_core::units::SpinGm;
    let vs = vectors();
    let tol = vs.get("LT-DE440-VS-R3B-REL").unwrap();
    for gamma in [1.0, 0.999] {
        // lint: allow-literal(PPN test points)
        let (bodies, x, v, a) = lt_model(gamma, LtReaction::Off);
        let n = bodies.len();
        let r3b = crate::LenseThirring::<Icrf>::new(
            SpinGm::from_km5_per_s3(1.27e16).unwrap(), // lint: allow-literal(same synthetic G S)
            Velocity::from_km_per_s(3.0e5).unwrap(),   // lint: allow-literal(same synthetic c)
            gamma,
            pole(),
        )
        .unwrap();
        let mut worst = 0.0_f64;
        for i in 1..n {
            let r = [x[i] - x[0], x[n + i] - x[n], x[2 * n + i] - x[2 * n]];
            let w = [v[i] - v[0], v[n + i] - v[n], v[2 * n + i] - v[2 * n]];
            let mut want = [0.0; 3];
            r3b.add_acceleration(r, w, &mut want);
            let got = [a[i], a[n + i], a[2 * n + i]];
            let d = (got[0] - want[0])
                .hypot(got[1] - want[1])
                .hypot(got[2] - want[2]);
            let rel = d / want[0].hypot(want[1]).hypot(want[2]);
            worst = worst.max(rel);
        }
        println!("L1b (gamma = {gamma}): max relative |DE440 Eq. 37-38 - R3b| = {worst:.3e}");
        tol.check(worst).unwrap();
    }
}

/// The GR effective potential (WP29, NUM-004): its forces are minus the gradient of its
/// potential (power balance in double-double, as for the other terms), its reaction conserves
/// momentum, and its acceleration has the Nobili–Roxburgh magnitude 6(GM☉)²/(c²r³).
///
/// Verifies: NUM-004
#[test]
fn gr_potential_is_conservative_with_reaction() {
    let (bodies, x, v) = fixture();
    let n = bodies.len();
    let c = 3.0e5; // lint: allow-literal(synthetic speed of light, km/s)
    let bodies_dd: Vec<NBody<Dd>> = bodies.iter().map(|b| body(b.id, b.gm, b.tier)).collect();
    let term_dd = GrPotential::new(Velocity::from_km_per_s(Dd::from_f64(c)).unwrap()).unwrap();
    let m = Model::<Dd>::new(
        &bodies_dd,
        MoonVariables::Barycentric,
        vec![Box::new(term_dd)],
    )
    .unwrap();
    let tol = 1e-9; // lint: allow-literal(finite-difference budget, as for the other terms)
    let mismatch = power_mismatch(&m, &x, &v);
    let a = accel(&m, &dd(&x), &dd(&v));
    let mut p = [Dd::ZERO; 3];
    let mut scale = Dd::ZERO;
    for i in 0..n {
        for (k, pk) in p.iter_mut().enumerate() {
            let t = m.layout().gm(i) * a[k * n + i];
            *pk += t;
            scale += t.abs();
        }
    }
    let mom = p.iter().map(|z| z.abs().to_f64()).fold(0.0, f64::max) / scale.to_f64();
    // Magnitude on Mercury (index 1) relative to the Sun (index 0).
    let gm_sun = bodies[0].gm;
    let r: Vec<f64> = (0..3).map(|k| x[k * n + 1] - x[k * n]).collect();
    let rn = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let want = 6.0 * gm_sun * gm_sun / (c * c * rn * rn * rn);
    let got = (0..3)
        .map(|k| a[k * n + 1].to_f64().powi(2))
        .sum::<f64>()
        .sqrt();
    println!(
        "GR potential: power balance {mismatch:.3e}, momentum {mom:.3e}, |a_Mercury| {got:e} vs \
         6(GM)^2/(c^2 r^3) = {want:e}"
    );
    assert!(mismatch < tol);
    assert!(mom < 1e-28); // lint: allow-literal(double-double round-off budget)
    assert!((got - want).abs() <= 1e-14 * want); // lint: allow-literal(f64 round-off budget)
    let model64 = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![
            Box::new(NewtonianMutual),
            Box::new(GrPotential::new(Velocity::from_km_per_s(c).unwrap()).unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(model64.gauge(), Gauge::GrEffectivePotential);
}

/// WP22 E4 (OI-35 decision of 2026-10-21, "LT all bodies"): the WP20 fixture plus one Minor body
/// and one Ring body. The Minor body receives the DE440 Eq. 37–38 acceleration, equal to the
/// single-body R3b form at its heliocentric state; the Sun's reaction includes it, so Σ μᵢ aᵢ = 0
/// to round-off; the Ring body gets nothing.
///
/// Verifies: NUM-019, PHY-014
#[test]
fn solar_lense_thirring_acts_on_minor_bodies() {
    use ferroastro_core::units::SpinGm;
    let vs = vectors();
    let (mut bodies, x6, v6) = fixture();
    bodies.push(body(NaifId(2_000_001), 62.6, Tier::Minor)); // lint: allow-literal(structural fixture: Ceres-like GM)
    bodies.push(body(NaifId(3_000_001), 1.0, Tier::Ring));
    let n = bodies.len();
    let extra_p = [[3.0e8, 2.0e8, 1.0e7], [6.0e9, 0.0, 0.0]]; // lint: allow-literal(structural fixture)
    let extra_v = [[-10.0, 14.0, 2.0], [0.0; 3]]; // lint: allow-literal(structural fixture)
    let grow = |flat: &[f64], extra: [[f64; 3]; 2]| -> Vec<f64> {
        (0..3)
            .flat_map(|k| {
                let mut c: Vec<f64> = flat[k * 6..(k + 1) * 6].to_vec();
                c.extend(extra.iter().map(|e| e[k]));
                c
            })
            .collect()
    };
    let (x, v) = (grow(&x6, extra_p), grow(&v6, extra_v));
    for gamma in [1.0, 0.999] {
        // lint: allow-literal(PPN test points)
        let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(lt_term(gamma, LtReaction::On))];
        let m = Model::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
        let a = accel(&m, &x, &v);
        let minor = 6;
        let r = [
            x[minor] - x[0],
            x[n + minor] - x[n],
            x[2 * n + minor] - x[2 * n],
        ];
        let w = [
            v[minor] - v[0],
            v[n + minor] - v[n],
            v[2 * n + minor] - v[2 * n],
        ];
        let r3b = crate::LenseThirring::<Icrf>::new(
            SpinGm::from_km5_per_s3(1.27e16).unwrap(), // lint: allow-literal(same synthetic G S as lt_term)
            Velocity::from_km_per_s(3.0e5).unwrap(), // lint: allow-literal(same synthetic c as lt_term)
            gamma,
            pole(),
        )
        .unwrap();
        let mut want = [0.0; 3];
        r3b.add_acceleration(r, w, &mut want);
        let got = [a[minor], a[n + minor], a[2 * n + minor]];
        let rel = (got[0] - want[0])
            .hypot(got[1] - want[1])
            .hypot(got[2] - want[2])
            / want[0].hypot(want[1]).hypot(want[2]);
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
        println!(
            "E4 (gamma = {gamma}): Minor body |a_LT| = {:.4e} km/s^2, rel diff vs R3b {rel:.3e}; \
             |sum GM_i a_i| / (eps sum GM_i |a_i|) = {ulps:.3}",
            want[0].hypot(want[1]).hypot(want[2])
        );
        assert!(got.iter().any(|&z| z != 0.0));
        assert_eq!([a[7], a[n + 7], a[2 * n + 7]], [0.0; 3]);
        vs.get("LT-DE440-VS-R3B-REL").unwrap().check(rel).unwrap();
        vs.get("NBODY-LT-REACTION-MOMENTUM-ULPS")
            .unwrap()
            .check(ulps)
            .unwrap();
    }
}

/// WP22 review item 3: with `MinorReaction::Off` the Minor bodies are test particles, so they send
/// no Lense–Thirring reaction to the Sun; their own LT acceleration and every Major body's are
/// unchanged bit for bit, and the Sun keeps the Major bodies' reactions.
///
/// Verifies: NUM-019, NUM-009
#[test]
fn minor_reaction_off_switches_off_the_lt_reaction_of_minor_bodies() {
    let (mut bodies, x6, v6) = fixture();
    bodies.push(body(NaifId(2_000_001), 62.6, Tier::Minor)); // lint: allow-literal(structural fixture: Ceres-like GM)
    let n = bodies.len();
    let extra_p = [3.0e8, 2.0e8, 1.0e7]; // lint: allow-literal(structural fixture)
    let extra_v = [-10.0, 14.0, 2.0]; // lint: allow-literal(structural fixture)
    let grow = |flat: &[f64], e: [f64; 3]| -> Vec<f64> {
        (0..3)
            .flat_map(|k| {
                let mut c: Vec<f64> = flat[k * 6..(k + 1) * 6].to_vec();
                c.push(e[k]);
                c
            })
            .collect()
    };
    let (x, v) = (grow(&x6, extra_p), grow(&v6, extra_v));
    let run = |minor: MinorReaction| {
        let t = lt_term(1.0, LtReaction::On).with_minor_reaction(minor);
        let m = Model::new(&bodies, MoonVariables::Barycentric, vec![Box::new(t)]).unwrap();
        accel(&m, &x, &v)
    };
    let (on, off) = (run(MinorReaction::On), run(MinorReaction::Off));
    let minor = 6;
    for i in 1..n {
        for k in 0..3 {
            assert_eq!(
                on[k * n + i].to_bits(),
                off[k * n + i].to_bits(),
                "body {i}"
            );
        }
    }
    // The Sun's difference is exactly the Minor body's reaction −(GM_m/GM☉) a_m.
    let ratio = bodies[minor].gm / bodies[0].gm;
    let mut worst = 0.0_f64;
    for k in 0..3 {
        let want = -(ratio * on[k * n + minor]);
        let got = on[k * n] - off[k * n];
        worst = worst.max((got - want).abs() / want.abs());
    }
    println!("Minor LT reaction: Sun on - off vs -(GM_m/GM_sun) a_m: max rel diff {worst:.3e}");
    assert!(on[minor] != 0.0);
    assert!(worst < 1e-6, "{worst:e}"); // lint: allow-literal(the reaction is ~1e-28 of the Sun's sum: cancellation budget)
    // With the Minor reaction off, Σ μ a over the Major bodies balances, the Minor body is extra.
    let mut sum = [0.0_f64; 3];
    for (i, b) in bodies.iter().enumerate().take(minor) {
        for k in 0..3 {
            sum[k] += b.gm * off[k * n + i];
        }
    }
    let scale: f64 = bodies
        .iter()
        .enumerate()
        .take(minor)
        .map(|(i, b)| b.gm * off[i].hypot(off[n + i]).hypot(off[2 * n + i]))
        .sum();
    let ulps = sum[0].hypot(sum[1]).hypot(sum[2]) / (eps() * scale);
    println!("Minor reaction off: Majors' |sum GM a| = {ulps:.3} eps");
    vectors()
        .get("NBODY-LT-REACTION-MOMENTUM-ULPS")
        .unwrap()
        .check(ulps)
        .unwrap();
}

/// WP22 review: terms that depend on the Minor bodies' reaction setting must agree; a model with
/// `MinorFromMajors(Off)` and a Lense–Thirring term left at `On` is refused (FA-E0500).
///
/// Verifies: NUM-009, NUM-019
#[test]
fn terms_with_different_minor_reaction_settings_are_refused() {
    let (mut bodies, _, _) = fixture();
    bodies.push(body(NaifId(2_000_001), 62.6, Tier::Minor)); // lint: allow-literal(structural fixture: Ceres-like GM)
    let build = |minor: MinorReaction, lt: MinorReaction| {
        let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(MinorFromMajors::new(minor)),
            Box::new(lt_term(1.0, LtReaction::On).with_minor_reaction(lt)),
        ];
        Model::new(&bodies, MoonVariables::Barycentric, terms)
    };
    for r in [MinorReaction::On, MinorReaction::Off] {
        assert!(build(r, r).is_ok());
    }
    let e = build(MinorReaction::Off, MinorReaction::On).unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(e.to_string().contains("Minor-body reaction"), "{e}");
}

/// Synthetic conformal-gravity coefficients for the structural fixture (ADR-0016, PHY-020): no
/// fit value (OI-79, decision D7). At r ≈ 5e7 km the linear term is ~1e-3 of GM☉/r² and the
/// quadratic term ~1e-3 of GM☉/r³·r, so both are resolved far above round-off.
const CG_C: f64 = 3.0e5; // lint: allow-literal(synthetic speed of light, km/s)
const CG_GAMMA_STAR: f64 = 4.0e-19; // lint: allow-literal(synthetic gamma_star, 1/km)
const CG_GAMMA0: f64 = 7.0e-19; // lint: allow-literal(synthetic gamma0, 1/km)
const CG_KAPPA: f64 = -1.0e-26; // lint: allow-literal(synthetic kappa, 1/km^2, attractive sign on purpose)

fn cg_sun<R: Real>(gamma0_sun: f64) -> ConformalPotential<R> {
    ConformalPotential::new(
        R::from_f64(CG_GAMMA_STAR),
        R::from_f64(gamma0_sun),
        R::from_f64(CG_KAPPA),
        Velocity::from_km_per_s(R::from_f64(CG_C)).unwrap(),
    )
    .unwrap()
}

fn cg_uniform<R: Real>() -> ConformalUniformGamma0<Icrf, R> {
    ConformalUniformGamma0::new(
        R::from_f64(CG_GAMMA0),
        Velocity::from_km_per_s(R::from_f64(CG_C)).unwrap(),
        // lint: allow-literal(structural direction, normalised by Pole)
        Pole::from_components([-0.3, 0.7, 0.2].map(R::from_f64)).unwrap(),
    )
    .unwrap()
}

/// `max_ik |∂(U/G)/∂x_ik + GM_i a_ik| / max_ik |GM_i a_ik|` by central differences in
/// double-double (velocities zero, so the energy is the potential alone).
fn gradient_mismatch(m: &Model<Dd>, x: &[f64]) -> f64 {
    let n = m.layout().len();
    let x = dd(x);
    let zero = vec![Dd::ZERO; x.len()];
    let a = accel(m, &x, &zero);
    let h = Dd::from_f64(1.0); // km; truncation ~ (h/r)^2 ~ 1e-16 relative
    let (mut worst, mut scale) = (0.0_f64, 0.0_f64);
    for i in 0..n {
        for k in 0..3 {
            let at = |s: Dd| {
                let mut y = x.clone();
                y[k * n + i] += s;
                m.energy_over_g(&y, &zero).unwrap()
            };
            let du = (at(h) - at(-h)) / (Dd::TWO * h);
            let f = m.layout().gm(i) * a[k * n + i];
            worst = worst.max((du + f).abs().to_f64());
            scale = scale.max(f.abs().to_f64());
        }
    }
    worst / scale
}

/// The conformal-gravity comparison terms are conservative with their stated potentials: the
/// finite-difference gradient of `Φ = (γc²/2) r − (κc²/2) r²` (with the Sun's reaction) and of the
/// uniform field's `Φ = −g·x` equals `−GMᵢ aᵢ` for every body, the Sun included, and the force
/// power balances the potential's rate along the motion (the energy diagnostic, NUM-023).
///
/// Verifies: PHY-020, NUM-023
#[test]
fn conformal_terms_are_minus_the_gradient_of_their_potential() {
    let (bodies, x, v) = fixture();
    let bodies_dd: Vec<NBody<Dd>> = bodies.iter().map(|b| body(b.id, b.gm, b.tier)).collect();
    let one = |t: Vec<Box<dyn ForceTerm<Icrf, Dd>>>| {
        Model::<Dd>::new(&bodies_dd, MoonVariables::Barycentric, t).unwrap()
    };
    let tol = 1e-9; // lint: allow-literal(finite-difference budget, as for the other terms)
    for (label, m) in [
        (
            "sun-sourced, gamma0 uniform",
            one(vec![Box::new(cg_sun(0.0))]),
        ),
        (
            "sun-sourced, gamma0 on the Sun",
            one(vec![Box::new(cg_sun(CG_GAMMA0))]),
        ),
        ("uniform gamma0", one(vec![Box::new(cg_uniform())])),
        (
            "both, with Newton",
            one(vec![
                Box::new(NewtonianMutual),
                Box::new(cg_sun(0.0)),
                Box::new(cg_uniform()),
            ]),
        ),
    ] {
        let grad = gradient_mismatch(&m, &x);
        let power = power_mismatch(&m, &x, &v);
        println!("{label}: gradient mismatch {grad:.3e}, power balance {power:.3e}");
        assert!(grad < tol, "{label}: {grad:e}");
        assert!(power < tol, "{label}: {power:e}");
    }
}

/// The Sun-sourced term gives every Major body the test-particle acceleration
/// `−(γc²/2) r̂ + κc² r` of [`crate::ConformalCentral`] at its heliocentric position, and its
/// reaction on the Sun conserves momentum; the uniform term gives every moving body exactly
/// `(γ0c²/2) n̂`.
///
/// Verifies: PHY-020
#[test]
fn conformal_terms_have_the_stated_accelerations() {
    let (bodies, x, v) = fixture();
    let n = bodies.len();
    let m = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(cg_sun::<f64>(0.0))],
    )
    .unwrap();
    let a = accel(&m, &x, &v);
    let central = crate::ConformalCentral::<Icrf>::new(
        CG_GAMMA_STAR,
        CG_KAPPA,
        Velocity::from_km_per_s(CG_C).unwrap(),
    )
    .unwrap();
    let mut worst = 0.0_f64;
    for i in 1..n {
        let r = [x[i] - x[0], x[n + i] - x[n], x[2 * n + i] - x[2 * n]];
        let mut want = [0.0; 3];
        CentralTerm::<Icrf>::add_acceleration(&central, r, [0.0; 3], &mut want);
        let got = [a[i], a[n + i], a[2 * n + i]];
        let d = (got[0] - want[0])
            .hypot(got[1] - want[1])
            .hypot(got[2] - want[2]);
        worst = worst.max(d / want[0].hypot(want[1]).hypot(want[2]));
    }
    let mut p = [0.0_f64; 3];
    let mut scale = 0.0;
    for i in 0..n {
        for (k, pk) in p.iter_mut().enumerate() {
            *pk += bodies[i].gm * a[k * n + i];
            scale += (bodies[i].gm * a[k * n + i]).abs();
        }
    }
    let mom = p.iter().map(|z| z.abs()).fold(0.0, f64::max) / scale;
    println!("sun-sourced: max |a - a_central|/|a| = {worst:.3e}, momentum {mom:.3e}");
    assert!(worst < 1e-14, "{worst:e}"); // lint: allow-literal(f64 round-off budget)
    assert!(mom < 1e-14, "{mom:e}"); // lint: allow-literal(f64 round-off budget)

    let u = cg_uniform::<f64>();
    let g = u.acceleration();
    let gn = g[0].hypot(g[1]).hypot(g[2]);
    let want = CG_GAMMA0 * CG_C * CG_C * 0.5;
    assert!((gn - want).abs() <= 1e-15 * want, "{gn:e} vs {want:e}"); // lint: allow-literal(f64 round-off budget)
    let mu = Model::new(&bodies, MoonVariables::Barycentric, vec![Box::new(u)]).unwrap();
    let au = accel(&mu, &x, &v);
    for i in 0..n {
        for (k, gk) in g.iter().enumerate() {
            assert_eq!(au[k * n + i], *gk, "body {i} component {k}");
        }
    }
}

/// Decision D1: the comparison model's gauge is not relativistic, so it combines with every
/// relativistic gauge without FA-E0502 (PPN isotropic/harmonic, the GR effective potential), and
/// the model then reports both gauges with the label "comparison model" (PHY-020 (e), (f)).
/// Alone it gives the model its own gauge; without it nothing changes.
///
/// Verifies: PHY-020, NUM-017
#[test]
fn conformal_gauge_combines_with_every_relativistic_gauge() {
    let g = Gauge::StaticWeakFieldPotential;
    assert!(!g.is_relativistic());
    assert!(g.is_comparison_model());
    assert!(g.label().contains("comparison model"), "{}", g.label());
    assert!(!Gauge::PpnIsotropicHarmonic.is_comparison_model());
    let (bodies, _, _) = fixture();
    let c = Velocity::from_km_per_s(CG_C).unwrap();
    let build =
        |t: Vec<Box<dyn ForceTerm<Icrf>>>| Model::new(&bodies, MoonVariables::Barycentric, t);

    let ppn = build(vec![
        Box::new(NewtonianMutual),
        Box::new(lt_term(1.0, LtReaction::On)),
        Box::new(cg_sun::<f64>(0.0)),
        Box::new(cg_uniform::<f64>()),
    ])
    .unwrap();
    assert_eq!(ppn.gauge(), Gauge::PpnIsotropicHarmonic);
    assert!(ppn.has_comparison_model());
    let d = ppn.describe();
    println!("{d}");
    assert!(
        d.starts_with("N-body model [PPN isotropic/harmonic; + comparison model"),
        "{d}"
    );
    assert!(d.contains("conformal-potential-sun") && d.contains("conformal-gamma0-uniform"));

    let grp = build(vec![
        Box::new(NewtonianMutual),
        Box::new(GrPotential::new(c).unwrap()),
        Box::new(cg_sun::<f64>(CG_GAMMA0)),
    ])
    .unwrap();
    assert_eq!(grp.gauge(), Gauge::GrEffectivePotential);
    assert!(grp.gauge_label().contains("comparison model"));

    let alone = build(vec![
        Box::new(NewtonianMutual),
        Box::new(cg_sun::<f64>(0.0)),
    ])
    .unwrap();
    assert_eq!(alone.gauge(), Gauge::StaticWeakFieldPotential);
    assert!(alone.describe().contains("comparison model"));

    let plain = build(vec![Box::new(NewtonianMutual)]).unwrap();
    assert!(!plain.has_comparison_model());
    assert_eq!(plain.gauge_label(), Gauge::Newtonian.label());
    assert!(!plain.describe().contains("comparison model"));

    // Two different relativistic gauges are still refused.
    let e = build(vec![
        Box::new(lt_term(1.0, LtReaction::On)),
        Box::new(GrPotential::new(c).unwrap()),
    ])
    .unwrap_err();
    assert_eq!(e.code().to_string(), "FA-E0502");

    // Test-particle form: 1PN Schwarzschild (PPN gauge) + the central conformal term.
    let mu = GravParam::from_km3_per_s2(bodies[0].gm).unwrap();
    let tp = crate::TestParticle::<Icrf>::new(
        mu,
        vec![
            Box::new(crate::SchwarzschildPpn::new(mu, c, 1.0, 1.0).unwrap()),
            Box::new(crate::ConformalCentral::new(CG_GAMMA_STAR, CG_KAPPA, c).unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(tp.gauge(), Gauge::PpnIsotropicHarmonic);
    assert!(
        tp.describe().contains("comparison model"),
        "{}",
        tp.describe()
    );
    let tp_alone = crate::TestParticle::<Icrf>::new(
        mu,
        vec![Box::new(
            crate::ConformalCentral::new(CG_GAMMA_STAR, CG_KAPPA, c).unwrap(),
        )],
    )
    .unwrap();
    assert_eq!(tp_alone.gauge(), Gauge::StaticWeakFieldPotential);
}

/// The conformal terms refuse non-finite coefficients, a bad speed of light and a layout
/// without a Major Sun (FA-E0500).
///
/// Verifies: PHY-020
#[test]
fn conformal_terms_refuse_invalid_input() {
    let c = Velocity::from_km_per_s(CG_C).unwrap();
    let e = ConformalPotential::new(f64::NAN, 0.0, 0.0, c).unwrap_err();
    assert_eq!(e.code().to_string(), "FA-E0500");
    let e = ConformalPotential::new(0.0, 0.0, f64::INFINITY, c).unwrap_err();
    assert_eq!(e.code().to_string(), "FA-E0500");
    let e = crate::ConformalCentral::<Icrf>::new(0.0, f64::NAN, c).unwrap_err();
    assert_eq!(e.code().to_string(), "FA-E0500");
    let e = ConformalUniformGamma0::<Icrf>::new(
        f64::NAN,
        c,
        Pole::from_components([0.0, 0.0, 1.0]).unwrap(),
    )
    .unwrap_err();
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(Pole::<Icrf>::from_components([0.0; 3]).is_err());
    let no_sun = vec![
        body(NaifId::MERCURY_BARYCENTER, 1.0, Tier::Major),
        body(NaifId::VENUS_BARYCENTER, 1.0, Tier::Major),
    ];
    let e = Model::new(
        &no_sun,
        MoonVariables::Barycentric,
        vec![Box::new(cg_sun::<f64>(0.0))],
    )
    .unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(
        e.to_string()
            .contains("conformal-gravity term needs the Sun"),
        "{e}"
    );
}

/// A non-zero uniform γ0 is refused beside a Ring body (FA-E0500 at bind): the ring is fixed, so
/// the field would not act on every body and relative motion would change (PHY-020 (c),
/// ADR-0016 item 5). A zero uniform γ0 is accepted, and so is the Sun-centred term.
///
/// Verifies: PHY-020
#[test]
fn uniform_gamma0_refuses_ring_bodies() {
    let bodies = vec![
        body(NaifId::SUN, 1.0, Tier::Major),
        body(NaifId::MERCURY_BARYCENTER, 1.0e-3, Tier::Major), // lint: allow-literal(structural mass ratio)
        body(NaifId(3_000_001), 1.0e-5, Tier::Ring), // lint: allow-literal(structural ring GM)
    ];
    let c = Velocity::from_km_per_s(CG_C).unwrap();
    let dir = Pole::from_components([0.0, 0.0, 1.0]).unwrap();
    let e = Model::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(cg_uniform::<f64>())],
    )
    .unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E0500");
    assert!(e.to_string().contains("Ring bodies"), "{e}");
    for gamma0 in [CG_GAMMA0, -CG_GAMMA0] {
        let without_ring = Model::new(
            &bodies[..2],
            MoonVariables::Barycentric,
            vec![Box::new(
                ConformalUniformGamma0::new(gamma0, c, dir).unwrap(),
            )],
        );
        assert!(without_ring.is_ok());
    }
    assert!(
        Model::new(
            &bodies,
            MoonVariables::Barycentric,
            vec![Box::new(ConformalUniformGamma0::new(0.0, c, dir).unwrap())],
        )
        .is_ok()
    );
    assert!(
        Model::new(
            &bodies,
            MoonVariables::Barycentric,
            vec![Box::new(cg_sun::<f64>(CG_GAMMA0))],
        )
        .is_ok()
    );
}
