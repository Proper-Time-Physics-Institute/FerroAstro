// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real-data tier (`#[ignore]`d, network; `cargo nextest run --workspace --profile data
//! --run-ignored only`) for the frame definitions. Pre-registered in IMPLEMENTATION-NOTES WP9:
//!
//! * our ICRF → `EclipticJ2000` and ICRF → `SunEquatorIau` rotations applied to JPL Horizons DE441
//!   heliocentric Mercury states (`REF_PLANE='FRAME'`, i.e. ICRF) must reproduce Horizons' own
//!   `ECLIPTIC` and `BODY EQUATOR` answers at the same epochs, 1900–2100;
//! * the IAU solar pole in a fresh NAIF `pck00011.tpc` equals the vectors `SUN-POLE-IAU-*`.
//!
//! Every test prints URL, retrieval date, SHA-256 and the Horizons `signature.version`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_core::frames::{Frame, Icrf};
use ferroastro_frames::{Rotation, icrf_to_ecliptic_j2000, icrf_to_sun_equator_iau};
use ferroastro_testdata::fetch_url;
use ferroastro_testdata::horizons::{RefPlane, VectorQuery, VectorTable, fetch_vectors};
use ferroastro_vectors::VectorSet;

const START_JD: f64 = 2_415_020.5; // lint: allow-literal(1900 January 1 0h, sampling choice)
const STEP_DAYS: u32 = 1000; // lint: allow-literal(sampling choice)
const STEPS: u32 = 73; // lint: allow-literal(sampling choice: to 2099 Nov 13)

fn mercury(plane: RefPlane) -> VectorTable {
    let q = VectorQuery {
        command: "199".into(),
        center: "500@10".into(),
        ref_plane: plane,
        start_jd_tdb: START_JD,
        stop_jd_tdb: START_JD + f64::from(STEPS * STEP_DAYS),
        step_days: STEP_DAYS,
    };
    let t = fetch_vectors(&q).unwrap();
    for c in &t.chunks {
        println!("{} {}", plane.param(), c.line());
    }
    t
}

fn rel(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0].hypot(d[1]).hypot(d[2]) / b[0].hypot(b[1]).hypot(b[2])
}

fn compare<T: Frame>(
    v: &VectorSet,
    id: &str,
    icrf: &VectorTable,
    rot: &Rotation<Icrf, T>,
    other: &VectorTable,
) {
    assert_eq!(icrf.jd_tdb, other.jd_tdb, "epoch grids differ");
    let (mut worst_r, mut worst_v, mut at) = (0.0_f64, 0.0_f64, 0.0);
    for k in 0..icrf.jd_tdb.len() {
        let dr = rel(rot.apply(icrf.r_km[k]), other.r_km[k]);
        let dv = rel(rot.apply(icrf.v_km_s[k]), other.v_km_s[k]);
        if dr.max(dv) > worst_r.max(worst_v) {
            at = icrf.jd_tdb[k];
        }
        worst_r = worst_r.max(dr);
        worst_v = worst_v.max(dv);
    }
    println!(
        "{id}: {} epochs JD {}..{}; max |dr|/|r| = {worst_r:.3e}, max |dv|/|v| = {worst_v:.3e} (worst at JD {at}); rotation: {}",
        icrf.jd_tdb.len(),
        icrf.jd_tdb[0],
        icrf.jd_tdb[icrf.jd_tdb.len() - 1],
        rot.definition()
    );
    v.get(id).unwrap().check(worst_r.max(worst_v)).unwrap();
}

/// Verifies: NUM-014, PHY-010
#[test]
#[ignore = "data: downloads JPL Horizons DE441 Mercury vectors in three reference planes (network); run with --profile data"]
fn data_frame_rotations_reproduce_horizons_ecliptic_and_body_equator() {
    let v = VectorSet::load_canonical().unwrap();
    let icrf = mercury(RefPlane::Frame);
    let ecl = mercury(RefPlane::Ecliptic);
    let body = mercury(RefPlane::BodyEquator);
    compare(
        &v,
        "HORIZONS-ECLIPJ2000-ROTATION",
        &icrf,
        &icrf_to_ecliptic_j2000(),
        &ecl,
    );
    compare(
        &v,
        "HORIZONS-IAU-SUN-ROTATION",
        &icrf,
        &icrf_to_sun_equator_iau().unwrap(),
        &body,
    );
}

/// Verifies: PHY-010, NUM-014
#[test]
#[ignore = "data: downloads NAIF pck00011.tpc (network); run with --profile data"]
fn data_naif_pck_solar_pole() {
    let v = VectorSet::load_canonical().unwrap();
    let f = fetch_url(
        "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/pck00011.tpc",
        "naif-pck00011.tpc",
        None,
    )
    .unwrap();
    println!("pck00011.tpc {}", f.provenance());
    let text = std::fs::read_to_string(&f.path).unwrap();
    let first = |key: &str| -> f64 {
        let line = text
            .lines()
            .find(|l| l.trim_start().starts_with(key))
            .unwrap_or_else(|| panic!("{key} not in pck00011.tpc"));
        let inner = line.split('(').nth(1).unwrap();
        inner.split_whitespace().next().unwrap().parse().unwrap()
    };
    let (ra, dec) = (first("BODY10_POLE_RA"), first("BODY10_POLE_DEC"));
    println!("pck00011.tpc: BODY10_POLE_RA = {ra}, BODY10_POLE_DEC = {dec}");
    v.get("SUN-POLE-IAU-RA").unwrap().check(ra).unwrap();
    v.get("SUN-POLE-IAU-DEC").unwrap().check(dec).unwrap();
}
