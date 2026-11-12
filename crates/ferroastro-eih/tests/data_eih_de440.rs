// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP19 check C7 (data tier, real data): EIH + solar J2 vs Newtonian + solar J2 against
//! de440.bsp. Sun + 8 planets + geocentric Moon from header.440 GROUP 1041 at JDEPOC
//! (1969-06-28), 10 Julian yr, every 10 d, IAS15 default. Both runs are made here from the same
//! initial conditions; the Newtonian one is the WP12 check-4 model. Both JPL products are
//! downloaded fresh (or taken from the cache after re-verifying the pinned SHA-256), with URL,
//! retrieval date and SHA-256 printed.
//!
//! Asserted (PROVISIONAL sanity, WP19 task): Mercury's maximum barycentric residual improves by
//! at least 10x (`EIH-DE440-MERCURY-IMPROVEMENT-MIN`). Every residual is published.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{Gr, epochs, ids, model, name, norm, propagator, sub, vectors};
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Barycentric, Geocentric, Icrf, State};
use ferroastro_eih::RhsAcceleration;
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_integrators::Ias15Config;
use ferroastro_system::InitialConditions;

const SAMPLES: u32 = 365; // lint: allow-literal(10-d sampling over 10 yr, as WP12 check 4)
const STEP_DAYS: f64 = 10.0; // lint: allow-literal(output cadence, days)

fn pos<C: ferroastro_core::frames::Center>(s: &State<Icrf, C>) -> [f64; 3] {
    s.position()
        .components()
        .map(ferroastro_core::units::Length::km)
}

#[derive(Default, Clone, Copy)]
struct Row {
    bary: f64,
    helio: f64,
    last: f64,
}

/// One 10-yr run; per-body rows (order of `ids()`), the geocentric Moon row and the step count.
fn residuals(h: &Header, prov: &str, spk: &Spk<'_>, gr: Gr) -> (Vec<(NaifId, Row)>, Row, u64) {
    let de = De440::load().unwrap();
    let ic = InitialConditions::<f64>::from_header(h, prov).unwrap();
    let body_ids = ids();
    let m = model(&de, &body_ids, true, gr, true);
    println!("{}", m.describe());
    let mut p = propagator(m, &ic, Ias15Config::default());
    let mut rows = vec![Row::default(); body_ids.len()];
    let mut moon = Row::default();
    for e in epochs(ic.epoch(), STEP_DAYS, SAMPLES) {
        p.advance_to(e).unwrap();
        let s = p.state().unwrap();
        let sun = pos(&s.state(NaifId::SUN).unwrap());
        let sun_ref = pos(&spk.state::<Barycentric, f64>(NaifId::SUN, e).unwrap());
        for (k, &id) in body_ids.iter().enumerate() {
            let got = pos(&s.state(id).unwrap());
            let want = pos(&spk.state::<Barycentric, f64>(id, e).unwrap());
            let db = norm(sub(got, want));
            let dh = norm(sub(sub(got, sun), sub(want, sun_ref)));
            rows[k].bary = rows[k].bary.max(db);
            rows[k].helio = rows[k].helio.max(dh);
            rows[k].last = db;
        }
        let em = p.earth_moon().unwrap();
        let d = norm(sub(
            pos(&em.moon),
            pos(&spk.state::<Geocentric, f64>(NaifId::MOON, e).unwrap()),
        ));
        moon.bary = moon.bary.max(d);
        moon.last = d;
    }
    (
        body_ids.into_iter().zip(rows).collect(),
        moon,
        p.stats().steps,
    )
}

/// C7: EIH + J2 vs Newtonian + J2 against de440.bsp over 1969-06-28 → +10 yr.
///
/// Verifies: NUM-017, NUM-019
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_eih_ten_years_from_header_ics_vs_de440_bsp() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let (newton, newton_moon, newton_steps) = residuals(&h, &hprov, &spk, Gr::None);
    let (eih, eih_moon, eih_steps) = residuals(
        &h,
        &hprov,
        &spk,
        Gr::Eih(1.0, 1.0, RhsAcceleration::Newtonian),
    );
    println!(
        "C7: Sun + 8 planets + geocentric Moon, solar J2 (DE440 J2SUN/ASUN, IAU 2015 pole), \
         header.440 ICs, IAS15 default; Newtonian {newton_steps} steps, EIH {eih_steps} steps; \
         {SAMPLES} samples every {STEP_DAYS} d"
    );
    println!(
        "  {:<16} {:>16} {:>16} {:>16} {:>16} {:>9}",
        "body", "Newton bary km", "EIH bary km", "Newton helio km", "EIH helio km", "factor"
    );
    let mut mercury_factor = 0.0;
    for ((id, n), (_, e)) in newton.iter().zip(&eih) {
        let f = n.bary / e.bary;
        println!(
            "  {:<16} {:>16.4} {:>16.4} {:>16.4} {:>16.4} {:>9.2}",
            name(*id),
            n.bary,
            e.bary,
            n.helio,
            e.helio,
            f
        );
        if *id == NaifId::MERCURY_BARYCENTER {
            mercury_factor = f;
        }
    }
    println!(
        "  {:<16} {:>16.4} {:>16.4} {:>16} {:>16} {:>9.2}",
        "Moon - Earth",
        newton_moon.bary,
        eih_moon.bary,
        "-",
        "-",
        newton_moon.bary / eih_moon.bary
    );
    let last = |rows: &[(NaifId, Row)]| {
        rows.iter()
            .find(|r| r.0 == NaifId::MERCURY_BARYCENTER)
            .unwrap()
            .1
            .last
    };
    println!(
        "  final-sample barycentric Mercury residual: Newton {:.4} km, EIH {:.4} km",
        last(&newton),
        last(&eih)
    );
    let min = vectors()
        .get("EIH-DE440-MERCURY-IMPROVEMENT-MIN")
        .unwrap()
        .value()
        .unwrap();
    println!("C7: Mercury improvement factor {mercury_factor:.3} (must be >= {min})");
    assert!(mercury_factor >= min, "{mercury_factor}");
}
