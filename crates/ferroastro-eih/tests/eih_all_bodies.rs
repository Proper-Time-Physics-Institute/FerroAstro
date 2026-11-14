// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 checks E1–E3 (PR tier): the 1PN EIH terms for every moving body over the tiered
//! interaction graph (maintainer decision D1 of 2026-10-21; NUM-009, NUM-020, PHY-014). Bounds
//! pre-registered in the WP22 plan (IMPLEMENTATION-NOTES) and kept in `tests/vectors/eih.toml`:
//!
//! * E1: with a single Minor body there is no Minor–Minor pair, so the tiered graph is complete
//!   and the corrections must equal those with the same body made Major;
//! * E2: the production term against an independent O(N³) evaluation of Eq. 27 with an explicit
//!   edge predicate (synthetic system with heavy Minor bodies and Ring masses), with a negative
//!   control that adds the Minor–Minor edges;
//! * E3: header.440 system with the four giant planets as Minor bodies: the 1PN centre of mass
//!   (μ* with the potential over the graph) moves on a straight line.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    clippy::too_many_lines,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; the \
              reference formulas are written as typeset"
)]

mod common;

use common::{epochs, header, ids, norm, sub, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::{Barycentric, Icrf, State};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::{TimeSpan, Velocity};
use ferroastro_eih::{Eih, RhsAcceleration};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MinorFromMajors, MinorReaction,
    MoonVariables, NBody, NBodyModel, NBodyPropagator, NewtonianMutual,
};
use ferroastro_integrators::{Ias15Config, SecondOrderSystem};
use ferroastro_system::{
    Catalogue, InitialConditions, SmallBodyStates, SystemState, Tier, place_small_bodies,
};
use std::path::PathBuf;

/// PPN test points: GR and the WP19 review's general point.
const PPN_POINTS: [(f64, f64); 2] = [(1.0, 1.0), (1.1, 0.9)]; // lint: allow-literal(PPN test points, WP19 review)
/// Ceres (NAIF 2 000 001).
const CERES: NaifId = NaifId(2_000_001); // lint: allow-literal(NAIF id of (1) Ceres)
/// Samples of the E3 run: every 10 d over 10 yr.
const SAMPLES: u32 = 365; // lint: allow-literal(10-d sampling over 10 yr)
const STEP_DAYS: f64 = 10.0; // lint: allow-literal(output cadence, days)

fn per_body(a: &[f64], n: usize) -> Vec<[f64; 3]> {
    (0..n).map(|i| [a[i], a[n + i], a[2 * n + i]]).collect()
}

/// The accelerations of a model whose only term is the EIH term (= its corrections).
fn eih_only(bodies: &[NBody], term: Eih, x: &[f64], v: &[f64]) -> Vec<[f64; 3]> {
    let m =
        NBodyModel::<Icrf>::new(bodies, MoonVariables::Barycentric, vec![Box::new(term)]).unwrap();
    let mut a = vec![0.0; x.len()];
    m.acceleration(0.0, x, v, &mut a).unwrap();
    per_body(&a, bodies.len())
}

fn rel_diff(a: [f64; 3], b: [f64; 3]) -> f64 {
    norm(sub(a, b)) / norm(b)
}

/// E1: Sun + 8 planets + Moon (barycentric) + Ceres, Ceres Minor vs Major.
///
/// Verifies: NUM-009, NUM-020, PHY-014
#[test]
fn a_single_minor_body_gets_the_same_eih_terms_as_a_major_body() {
    let Some(de) =
        De440::load_cached_or_skip("a_single_minor_body_gets_the_same_eih_terms_as_a_major_body")
            .unwrap()
    else {
        return;
    };
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let cat = Catalogue::from_header(&h, &prov).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let mut states = ic.states().clone();
    for (id, s) in place_small_bodies(&file, &[CERES], &sun)
        .unwrap()
        .states()
        .unwrap()
    {
        states.push(id, &s).unwrap();
    }
    let mut list: Vec<(NaifId, Tier)> = ids().into_iter().map(|id| (id, Tier::Major)).collect();
    list.push((CERES, Tier::Minor));
    let bodies = |ceres: Tier| -> Vec<NBody> {
        list.iter()
            .map(|&(id, t)| NBody {
                id,
                gm: cat.gm(id).unwrap().km3_per_s2(),
                tier: if id == CERES { ceres } else { t },
            })
            .collect()
    };
    let (minor, major) = (bodies(Tier::Minor), bodies(Tier::Major));
    let probe = NBodyModel::<Icrf>::new(
        &major,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutual)],
    )
    .unwrap();
    let (x, v) = probe.pack(&states, None).unwrap();
    let vs = vectors();
    let bound = vs.get("EIH-TIER-SINGLE-MINOR-REL").unwrap();
    let c = de.speed_of_light();
    for (beta, gamma) in PPN_POINTS {
        let term = Eih::new(c, beta, gamma, RhsAcceleration::Newtonian).unwrap();
        let a_minor = eih_only(&minor, term, &x, &v);
        let a_major = eih_only(&major, term, &x, &v);
        let mut worst = 0.0_f64;
        for (i, (p, q)) in a_minor.iter().zip(&a_major).enumerate() {
            let r = rel_diff(*p, *q);
            if i + 1 == list.len() {
                println!(
                    "E1 ({beta}, {gamma}): Ceres |delta a| = {:.4e} km/s^2, rel diff {r:.3e}",
                    norm(*q)
                );
            }
            worst = worst.max(r);
        }
        println!("E1 ({beta}, {gamma}): max over 11 bodies rel diff Minor vs Major = {worst:.3e}");
        assert!(norm(a_minor[list.len() - 1]) > 0.0);
        bound.check(worst).unwrap();
    }
}

/// One body of the E2 reference: tier, GM, position (km), velocity (km/s).
#[derive(Clone, Copy)]
struct Body {
    tier: Tier,
    gm: f64,
    r: [f64; 3],
    v: [f64; 3],
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Eq. 27 minus its Newtonian part, written again as typeset, with every sum restricted to
/// `edge` and each `U` and `a_j` recomputed from scratch for every pair (O(N³)). `src_gm(k)` is
/// body k's GM as a source; Ring bodies have `a = 0` and receive nothing.
fn reference(
    b: &[Body],
    edge: &dyn Fn(usize, usize) -> bool,
    src_gm: &dyn Fn(usize) -> f64,
    beta: f64,
    gamma: f64,
    c: f64,
) -> Vec<[f64; 3]> {
    let n = b.len();
    let dist = |i: usize, j: usize| norm(sub(b[i].r, b[j].r));
    let pot = |i: usize| -> f64 {
        (0..n)
            .filter(|&k| k != i && edge(i, k))
            .map(|k| src_gm(k) / dist(i, k))
            .sum()
    };
    let newton = |j: usize| -> [f64; 3] {
        let mut a = [0.0; 3];
        if b[j].tier == Tier::Ring {
            return a;
        }
        for k in (0..n).filter(|&k| k != j && edge(j, k)) {
            let d = sub(b[k].r, b[j].r);
            let r = dist(j, k);
            for (ac, dc) in a.iter_mut().zip(d) {
                *ac += src_gm(k) * dc / (r * r * r);
            }
        }
        a
    };
    let c2 = c * c;
    (0..n)
        .map(|i| {
            let mut out = [0.0; 3];
            if b[i].tier == Tier::Ring {
                return out;
            }
            let (ri, vi) = (b[i].r, b[i].v);
            for j in (0..n).filter(|&j| j != i && edge(i, j)) {
                let gj = src_gm(j);
                let (rj, vj) = (b[j].r, b[j].v);
                let aj = newton(j);
                let r = dist(i, j);
                let rji = sub(rj, ri);
                let rij = sub(ri, rj);
                let proj2 = (dot(rij, vj) / r).powi(2);
                let brace = -2.0 * (beta + gamma) / c2 * pot(i) - (2.0 * beta - 1.0) / c2 * pot(j)
                    + gamma * dot(vi, vi) / c2
                    + (1.0 + gamma) * dot(vj, vj) / c2
                    - 2.0 * (1.0 + gamma) / c2 * dot(vi, vj)
                    - 3.0 / (2.0 * c2) * proj2
                    + 0.5 / c2 * dot(rji, aj);
                let w: [f64; 3] =
                    [0, 1, 2].map(|k| (2.0 + 2.0 * gamma) * vi[k] - (1.0 + 2.0 * gamma) * vj[k]);
                let second = gj / (c2 * r * r * r) * dot(rij, w);
                for k in 0..3 {
                    out[k] += gj * rji[k] / (r * r * r) * brace
                        + second * (vi[k] - vj[k])
                        + (3.0 + 4.0 * gamma) / (2.0 * c2) * gj * aj[k] / r;
                }
            }
            out
        })
        .collect()
}

/// The E2 synthetic system: Sun, 3 planets, 4 Minor bodies of GM 1e-4 GM☉ (two of them 0.1 au
/// apart) and 2 Ring masses at 40 au with zero velocity. Structural fixture, not physical values.
fn synthetic() -> Vec<Body> {
    let au = 1.496e8; // lint: allow-literal(structural fixture length scale)
    let gs = 1.327e11; // lint: allow-literal(structural fixture: solar-order GM)
    let mk = |tier, gm: f64, r: [f64; 3], v: [f64; 3]| Body {
        tier,
        gm,
        r: r.map(|x| x * au),
        v,
    };
    // Rows: tier, GM / GM_sun, position (au), velocity (km/s).
    #[rustfmt::skip]
    let rows: [(Tier, f64, [f64; 3], [f64; 3]); 10] = [
        (Tier::Major, 1.0, [0.003, -0.001, 0.0002], [0.01, -0.012, 0.0003]), // lint: allow-literal(structural fixture)
        (Tier::Major, 1.7e-7, [0.35, -0.12, 0.03], [20.0, 41.0, 3.0]), // lint: allow-literal(structural fixture)
        (Tier::Major, 3.0e-6, [-0.6, 0.8, -0.01], [-24.0, -18.0, 1.0]), // lint: allow-literal(structural fixture)
        (Tier::Major, 9.5e-4, [3.4, -3.7, -0.1], [9.0, 8.0, -0.3]), // lint: allow-literal(structural fixture)
        (Tier::Minor, 1e-4, [1.0, 0.2, 0.05], [-5.0, 29.0, 0.5]), // lint: allow-literal(structural fixture)
        (Tier::Minor, 1e-4, [1.08, 0.26, 0.04], [-7.0, 27.0, 0.2]), // lint: allow-literal(structural fixture)
        (Tier::Minor, 1e-4, [-2.1, -1.4, 0.3], [11.0, -15.0, 1.5]), // lint: allow-literal(structural fixture)
        (Tier::Minor, 1e-4, [0.1, 2.6, -0.4], [-18.0, 1.0, 2.0]), // lint: allow-literal(structural fixture)
        (Tier::Ring, 1e-8, [40.0, 0.0, 0.0], [0.0; 3]), // lint: allow-literal(structural fixture)
        (Tier::Ring, 1e-8, [0.0, -40.0, 0.0], [0.0; 3]), // lint: allow-literal(structural fixture)
    ];
    rows.iter()
        .map(|&(tier, f, r, v)| mk(tier, gs * f, r, v))
        .collect()
}

/// The tiered graph: no Minor–Minor and no Ring–Ring edge.
fn tiered(b: &[Body]) -> impl Fn(usize, usize) -> bool + '_ {
    move |i, j| {
        !matches!(
            (b[i].tier, b[j].tier),
            (Tier::Minor, Tier::Minor) | (Tier::Ring, Tier::Ring)
        )
    }
}

/// E2: production vs the independent reference, reaction On and Off, two PPN points; negative
/// control with the Minor–Minor edges added.
///
/// Verifies: NUM-009, NUM-020, PHY-014
#[test]
fn eih_over_the_tiered_graph_matches_an_independent_evaluation() {
    let Some(de) =
        De440::load_cached_or_skip("eih_over_the_tiered_graph_matches_an_independent_evaluation")
            .unwrap()
    else {
        return;
    };
    let c = de.speed_of_light();
    let b = synthetic();
    let n = b.len();
    let bodies: Vec<NBody> = b
        .iter()
        .enumerate()
        .map(|(i, x)| NBody {
            id: NaifId(3_000_000 + i32::try_from(i).unwrap()), // lint: allow-literal(synthetic NAIF ids)
            gm: x.gm,
            tier: x.tier,
        })
        .collect();
    let x: Vec<f64> = (0..3).flat_map(|k| b.iter().map(move |q| q.r[k])).collect();
    let v: Vec<f64> = (0..3).flat_map(|k| b.iter().map(move |q| q.v[k])).collect();
    let vs = vectors();
    let bound = vs.get("EIH-TIER-SINGLE-MINOR-REL").unwrap();
    let edge = tiered(&b);
    let all = |i: usize, j: usize| !matches!((b[i].tier, b[j].tier), (Tier::Ring, Tier::Ring));
    for reaction in [MinorReaction::On, MinorReaction::Off] {
        let src_gm = |k: usize| {
            if reaction == MinorReaction::Off && b[k].tier == Tier::Minor {
                0.0
            } else {
                b[k].gm
            }
        };
        for (beta, gamma) in PPN_POINTS {
            let term = Eih::new(c, beta, gamma, RhsAcceleration::Newtonian)
                .unwrap()
                .with_minor_reaction(reaction);
            let got = eih_only(&bodies, term, &x, &v);
            let want = reference(&b, &edge, &src_gm, beta, gamma, c.km_per_s());
            let mut worst = 0.0_f64;
            for i in 0..n {
                if b[i].tier == Tier::Ring {
                    assert!(
                        got[i].iter().all(|&z| z.to_bits() == 0),
                        "Ring body {i} must get nothing"
                    );
                } else {
                    worst = worst.max(rel_diff(got[i], want[i]));
                }
            }
            println!(
                "E2 ({reaction:?}, beta {beta}, gamma {gamma}): max rel diff production vs \
                 reference over 8 moving bodies = {worst:.3e}"
            );
            bound.check(worst).unwrap();
            if reaction == MinorReaction::On {
                let control = reference(&b, &all, &src_gm, beta, gamma, c.km_per_s());
                let seen = (0..n)
                    .filter(|&i| b[i].tier == Tier::Minor)
                    .map(|i| rel_diff(got[i], control[i]))
                    .fold(0.0, f64::max);
                println!(
                    "E2 negative control (Minor-Minor edges added): max rel diff on Minor \
                     bodies = {seen:.3e}"
                );
                assert!(seen > 1e-10, "{seen:e}"); // lint: allow-literal(pre-registered control threshold, WP22 plan E2)
            }
        }
    }
}

fn pos(s: &State<Icrf, Barycentric>) -> [f64; 3] {
    s.position()
        .components()
        .map(ferroastro_core::units::Length::km)
}

fn vel(s: &State<Icrf, Barycentric>) -> [f64; 3] {
    s.velocity().map(Velocity::km_per_s)
}

/// Largest distance of the points `p(t)` from their least-squares straight line (as C6).
fn max_linear_deviation(t: &[f64], p: &[[f64; 3]]) -> f64 {
    let n = t.len() as f64;
    let tm = t.iter().sum::<f64>() / n;
    let stt: f64 = t.iter().map(|x| (x - tm) * (x - tm)).sum();
    let mut fit = [(0.0, 0.0); 3];
    for (k, f) in fit.iter_mut().enumerate() {
        let pm = p.iter().map(|q| q[k]).sum::<f64>() / n;
        let stp: f64 = t.iter().zip(p).map(|(x, q)| (x - tm) * (q[k] - pm)).sum();
        let slope = stp / stt;
        *f = ((-slope).mul_add(tm, pm), slope);
    }
    t.iter()
        .zip(p)
        .map(|(&x, q)| norm([0, 1, 2].map(|k| q[k] - fit[k].1.mul_add(x, fit[k].0))))
        .fold(0.0, f64::max)
}

/// E3: header.440 Sun, Mercury, Venus, Earth, Moon (geocentric), Mars Major; Jupiter–Neptune
/// Minor; Newton (tiered) + EIH, 10 yr. The 1PN centre of mass with μᵢ* = GMᵢ(1 + vᵢ²/2c² −
/// Uᵢ/2c²), Uᵢ over the graph, moves on a straight line; the Newtonian barycentre does not.
///
/// Verifies: NUM-009, NUM-018, PHY-014
#[test]
fn the_tiered_eih_model_conserves_its_relativistic_centre_of_mass() {
    let Some(de) = De440::load_cached_or_skip(
        "the_tiered_eih_model_conserves_its_relativistic_centre_of_mass",
    )
    .unwrap() else {
        return;
    };
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let cat = Catalogue::de440_major(&de);
    let giants = [
        NaifId::JUPITER_BARYCENTER,
        NaifId::SATURN_BARYCENTER,
        NaifId::URANUS_BARYCENTER,
        NaifId::NEPTUNE_BARYCENTER,
    ];
    let list: Vec<(NaifId, Tier)> = ids()
        .into_iter()
        .map(|id| {
            (
                id,
                if giants.contains(&id) {
                    Tier::Minor
                } else {
                    Tier::Major
                },
            )
        })
        .collect();
    let bodies: Vec<NBody> = list
        .iter()
        .map(|&(id, tier)| NBody {
            id,
            gm: cat.gm(id).unwrap().km3_per_s2(),
            tier,
        })
        .collect();
    let c = de.speed_of_light();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
        Box::new(NewtonianMutual),
        Box::new(MinorFromMajors::new(MinorReaction::On)),
        Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        Box::new(Eih::new(c, 1.0, 1.0, RhsAcceleration::Newtonian).unwrap()),
    ];
    let model = NBodyModel::<Icrf>::new(
        &bodies,
        MoonVariables::Geocentric {
            emrat: de.earth_moon_mass_ratio(),
        },
        terms,
    )
    .unwrap();
    println!("{}", model.describe());
    let dt0 = TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap();
    let mut p = NBodyPropagator::new(
        model,
        ic.states(),
        ic.earth_moon(),
        ic.epoch(),
        dt0,
        Ias15Config::default(),
    )
    .unwrap();
    let mut t = vec![0.0];
    let mut states: Vec<SystemState<Icrf, Barycentric>> = vec![p.state().unwrap()];
    for (k, e) in epochs(ic.epoch(), STEP_DAYS, SAMPLES)
        .into_iter()
        .enumerate()
    {
        p.advance_to(e).unwrap();
        t.push((k + 1) as f64 * STEP_DAYS * SECONDS_PER_DAY);
        states.push(p.state().unwrap());
    }
    let c2 = c.km_per_s() * c.km_per_s();
    let edge = |i: usize, j: usize| !(list[i].1 == Tier::Minor && list[j].1 == Tier::Minor);
    let gm: Vec<f64> = bodies.iter().map(|b| b.gm).collect();
    let total: f64 = gm.iter().sum();
    let mut rel = Vec::with_capacity(states.len());
    let mut newt = Vec::with_capacity(states.len());
    for s in &states {
        let r: Vec<[f64; 3]> = list
            .iter()
            .map(|&(id, _)| pos(&s.state(id).unwrap()))
            .collect();
        let w: Vec<[f64; 3]> = list
            .iter()
            .map(|&(id, _)| vel(&s.state(id).unwrap()))
            .collect();
        let n = r.len();
        let mu: Vec<f64> = (0..n)
            .map(|i| {
                let u: f64 = (0..n)
                    .filter(|&k| k != i && edge(i, k))
                    .map(|k| gm[k] / norm(sub(r[i], r[k])))
                    .sum();
                gm[i] * (1.0 + (dot(w[i], w[i]) - u) / (2.0 * c2))
            })
            .collect();
        let mt: f64 = mu.iter().sum();
        let mut rs = [0.0; 3];
        let mut rn = [0.0; 3];
        for i in 0..n {
            for k in 0..3 {
                rs[k] += mu[i] * r[i][k];
                rn[k] += gm[i] * r[i][k];
            }
        }
        rel.push(rs.map(|z| z / mt));
        newt.push(rn.map(|z| z / total));
    }
    let d_rel = max_linear_deviation(&t, &rel);
    let d_newt = max_linear_deviation(&t, &newt);
    println!(
        "E3: 10 yr, {} IAS15 steps, {} samples: max deviation from a straight line: \
         relativistic centre of mass (U over the graph) {d_rel:.4e} km, Newtonian barycentre \
         {d_newt:.4e} km (ratio {:.3e})",
        p.stats().steps,
        t.len(),
        d_newt / d_rel
    );
    vectors()
        .get("EIH-REL-SSB-DEVIATION-KM")
        .unwrap()
        .check(d_rel)
        .unwrap();
    let min_ratio = vectors()
        .get("EIH-NEWTON-SSB-RATIO-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(d_newt / d_rel >= min_ratio, "ratio {}", d_newt / d_rel);
}

/// WP22 review: `Eih::with_minor_reaction` must match `MinorFromMajors`; `NBodyModel::new`
/// refuses a model whose terms disagree (FA-E0500), and accepts matching settings.
///
/// Verifies: NUM-009
#[test]
fn eih_minor_reaction_must_match_minor_from_majors() {
    let Some(de) =
        De440::load_cached_or_skip("eih_minor_reaction_must_match_minor_from_majors").unwrap()
    else {
        return;
    };
    let b = synthetic();
    let bodies: Vec<NBody> = b
        .iter()
        .enumerate()
        .map(|(i, x)| NBody {
            id: NaifId(3_000_000 + i32::try_from(i).unwrap()), // lint: allow-literal(synthetic NAIF ids)
            gm: x.gm,
            tier: x.tier,
        })
        .collect();
    let build = |minor: MinorReaction, eih: MinorReaction| {
        let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(MinorFromMajors::new(minor)),
            Box::new(
                Eih::new(de.speed_of_light(), 1.0, 1.0, RhsAcceleration::Newtonian)
                    .unwrap()
                    .with_minor_reaction(eih),
            ),
        ];
        NBodyModel::<Icrf>::new(&bodies, MoonVariables::Barycentric, terms)
    };
    for r in [MinorReaction::On, MinorReaction::Off] {
        assert!(build(r, r).is_ok());
    }
    for (m, e) in [
        (MinorReaction::On, MinorReaction::Off),
        (MinorReaction::Off, MinorReaction::On),
    ] {
        let err = build(m, e).unwrap_err();
        println!("{err}");
        assert_eq!(err.code().to_string(), "FA-E0500");
    }
}

/// WP22 review: bodies that do not move (Ring) enter Eq. 27 with v = a = 0 whatever velocity is
/// stored for them; a non-zero stored Ring velocity changes nothing, bit for bit.
///
/// Verifies: NUM-009, NUM-020
#[test]
fn ring_bodies_enter_eih_with_zero_velocity_whatever_is_stored() {
    let Some(de) =
        De440::load_cached_or_skip("ring_bodies_enter_eih_with_zero_velocity_whatever_is_stored")
            .unwrap()
    else {
        return;
    };
    let b = synthetic();
    let bodies: Vec<NBody> = b
        .iter()
        .enumerate()
        .map(|(i, x)| NBody {
            id: NaifId(3_000_000 + i32::try_from(i).unwrap()), // lint: allow-literal(synthetic NAIF ids)
            gm: x.gm,
            tier: x.tier,
        })
        .collect();
    let x: Vec<f64> = (0..3).flat_map(|k| b.iter().map(move |q| q.r[k])).collect();
    let v0: Vec<f64> = (0..3).flat_map(|k| b.iter().map(move |q| q.v[k])).collect();
    let mut v1 = v0.clone();
    let n = b.len();
    for (i, q) in b.iter().enumerate() {
        if q.tier == Tier::Ring {
            for k in 0..3 {
                v1[k * n + i] = 5.0; // lint: allow-literal(arbitrary non-zero stored ring velocity, km/s)
            }
        }
    }
    for rhs in [RhsAcceleration::Newtonian, RhsAcceleration::Iterated] {
        let term = Eih::new(de.speed_of_light(), 1.0, 1.0, rhs).unwrap();
        let a0 = eih_only(&bodies, term, &x, &v0);
        let a1 = eih_only(&bodies, term, &x, &v1);
        for (p, q) in a0.iter().zip(&a1) {
            for k in 0..3 {
                assert_eq!(p[k].to_bits(), q[k].to_bits());
            }
        }
    }
}
