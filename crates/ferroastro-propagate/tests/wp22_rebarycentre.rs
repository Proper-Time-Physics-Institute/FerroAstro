// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 check E7 (PR tier; NUM-018, OI-75 decision of 2026-10-21): the opt-in scenario key
//! `initial_conditions.barycentre = "relativistic"` moves the n-body initial conditions to the
//! relativistic SSB with the velocity condition Σ(μ̇*r + μ*v) = 0; absent, the header.440 ICs are
//! used unchanged; the key is refused outside n-body mode. Bounds pre-registered in the WP22 plan.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::suboptimal_flops,
    clippy::float_cmp,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              exact comparisons are intended (bitwise-unchanged records, rest velocities)"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::units::Velocity;
use ferroastro_eih::{BarycentreBody, mu_star, mu_star_rate};
use ferroastro_propagate::{DataSources, InitialRecords, initial_conditions};
use ferroastro_scenario::{BodyName, IcBarycentre, ScenarioSpec};
use ferroastro_system::KBO_RING;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const MOON: i32 = 301;
const EARTH: i32 = 399;

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

/// The physical barycentric bodies of the records (a geocentric Earth–Moon pair reconstructed
/// with `emrat`) and the ids.
fn bodies(rec: &InitialRecords, emrat: Option<f64>) -> (Vec<i32>, Vec<BarycentreBody>) {
    let v3 = |p: &[[f64; 2]; 3]| p.map(|q| q[0] + q[1]);
    let mut out: Vec<BarycentreBody> = rec
        .records
        .iter()
        .map(|r| BarycentreBody {
            gm: r.gm[0] + r.gm[1],
            position: v3(&r.position),
            velocity: v3(&r.velocity),
        })
        .collect();
    let ids: Vec<i32> = rec.records.iter().map(|r| r.naif).collect();
    if let Some(emrat) = emrat {
        let e = ids.iter().position(|&i| i == EARTH).unwrap();
        let m = ids.iter().position(|&i| i == MOON).unwrap();
        let (fe, fm) = (emrat / (1.0 + emrat), 1.0 / (1.0 + emrat));
        let (b, rho) = (out[e], out[m]);
        for k in 0..3 {
            out[e].position[k] = b.position[k] - fm * rho.position[k];
            out[e].velocity[k] = b.velocity[k] - fm * rho.velocity[k];
            out[m].position[k] = b.position[k] + fe * rho.position[k];
            out[m].velocity[k] = b.velocity[k] + fe * rho.velocity[k];
        }
    }
    (ids, out)
}

/// |Σμ*r|/Σμ* (km) and |Σ(μ̇*r + μ*v)|/Σμ* (km/s).
fn residuals(b: &[BarycentreBody], c: Velocity) -> (f64, f64) {
    let mu = mu_star(b, c).unwrap();
    let rate = mu_star_rate(b, c).unwrap();
    let total: f64 = mu.iter().sum();
    let (mut r, mut w) = ([0.0_f64; 3], [0.0_f64; 3]);
    for ((q, &m), &md) in b.iter().zip(&mu).zip(&rate) {
        for k in 0..3 {
            r[k] += m * q.position[k];
            w[k] += md * q.position[k] + m * q.velocity[k];
        }
    }
    let n = |a: [f64; 3]| a[0].hypot(a[1]).hypot(a[2]) / total;
    (n(r), n(w))
}

/// E7a/b: the key moves the ICs to the relativistic SSB (Sun + planets with an EMB body; the
/// PHY-012 system with the geocentric Moon, Pluto and the KBO ring), the Ring masses stay at rest,
/// and without the key the ICs are untouched.
///
/// Verifies: NUM-018
#[test]
fn relativistic_barycentre_key_moves_the_initial_conditions_to_the_relativistic_ssb() {
    let Some(de) = De440::load_cached_or_skip(
        "relativistic_barycentre_key_moves_the_initial_conditions_to_the_relativistic_ssb",
    )
    .unwrap() else {
        return;
    };
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let c = de.speed_of_light();
    let vs = VectorSet::load_canonical().unwrap();
    let (bound_r, bound_v) = (
        vs.get("EIH-REBARY-RESIDUAL-KM").unwrap(),
        vs.get("EIH-REBARY-RESIDUAL-KMS").unwrap(),
    );
    let mut emb = spec("phy012-newton-full-header440.toml");
    emb.bodies = vec![
        BodyName::Sun,
        BodyName::Mercury,
        BodyName::Venus,
        BodyName::Emb,
        BodyName::Mars,
        BodyName::Jupiter,
        BodyName::Saturn,
        BodyName::Uranus,
        BodyName::Neptune,
    ];
    emb.forces.gr_eih = true;
    emb.name = "wp22-e7-emb".into();
    let mut geo = spec("phy012-newton-full-header440.toml");
    geo.forces.gr_eih = true;
    geo.forces.kbo_ring = true;
    geo.name = "wp22-e7-geocentric-ring".into();
    for (template, emrat) in [(emb, None), (geo, Some(de.earth_moon_mass_ratio()))] {
        let plain = initial_conditions(&template, &data).unwrap();
        let mut keyed = template.clone();
        keyed.initial_conditions.barycentre = Some(IcBarycentre::Relativistic);
        keyed.validate().unwrap();
        let moved = initial_conditions(&keyed, &data).unwrap();
        let (_, before) = bodies(&plain, emrat);
        let (ids, after) = bodies(&moved, emrat);
        let (r0, v0) = residuals(&before, c);
        let (r1, v1) = residuals(&after, c);
        println!(
            "E7 {}: {} bodies; header ICs |sum mu* r|/sum mu* = {r0:.3e} km, \
             |sum(mu*' r + mu* v)|/sum mu* = {v0:.3e} km/s; re-barycentred {r1:.3e} km, \
             {v1:.3e} km/s",
            template.name,
            ids.len()
        );
        bound_r.check(r1).unwrap();
        bound_v.check(v1).unwrap();
        assert_ne!(plain.ic_hash, moved.ic_hash);
        let again = initial_conditions(&template, &data).unwrap();
        assert_eq!(plain, again, "without the key the ICs must not change");
        let ring: Vec<i32> = (0..KBO_RING.points)
            .map(|k| KBO_RING.point(k).1.0)
            .collect();
        let mut rings_seen = 0;
        for (i, id) in ids.iter().enumerate() {
            if ring.contains(id) {
                rings_seen += 1;
                assert_eq!(
                    after[i].velocity, [0.0; 3],
                    "ring mass {id} must stay at rest"
                );
                assert_ne!(after[i].position, before[i].position);
            }
            if *id == MOON && emrat.is_some() {
                let (p, q) = (&plain.records[i], &moved.records[i]);
                assert_eq!(p.position, q.position, "Moon - Earth is a difference");
                assert_eq!(p.velocity, q.velocity, "Moon - Earth is a difference");
            }
        }
        let want = if emrat.is_some() { ring.len() } else { 0 };
        assert_eq!(rings_seen, want);
    }
}

/// E7c: the key is refused outside n-body mode with a stable error code.
///
/// Verifies: NUM-018
#[test]
fn relativistic_barycentre_key_is_refused_outside_n_body_mode() {
    let mut s = spec("sun-mercury-gr1pn-header440.toml");
    s.initial_conditions.barycentre = Some(IcBarycentre::Relativistic);
    let e = s.validate().unwrap_err();
    println!("E7c: {e} ({})", e.code());
    assert!(
        e.to_string().contains("initial_conditions.barycentre"),
        "{e}"
    );
    assert_eq!(e.code().to_string(), "FA-E1203");
}
