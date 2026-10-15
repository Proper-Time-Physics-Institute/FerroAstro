// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-014: the ecliptic-of-date perihelion rate minus the inertial rate equals the IAU 2006
//! general precession `p_A`, both measured in the same reference plane. Pre-registered in
//! IMPLEMENTATION-NOTES WP9 (checks 3, 3b, 3c).
//!
//! A Sun+Mercury+GR (Schwarzschild rung) run over 200 years gives the perihelion passages in ICRF.
//! Each passage position is expressed
//! * **inertially**: in ICRF, with its longitude measured in the IAU 2006 J2000 ecliptic plane
//!   (`ecm06` at J2000) from that frame's fixed x-axis;
//! * **in the ecliptic of date**: rotated into `EclipticOfDate` with `ecm06` at the passage's TT
//!   date, with its longitude measured **in the same plane** (the J2000 ecliptic pole expressed in
//!   the date frame), from the projection of the equinox of date.
//!
//! Linear fits in TT Julian centuries give the two rates. Their difference is compared with:
//! * `NUM014-RATE-DIFF-VS-PA`: the linear-fit slope of `p_A(t)` from the ported `p06e`, over the
//!   same epochs (tolerance 1e-3 ″/cy; expected +4.4e-4 from the second-order projection term);
//! * `NUM014-RATE-DIFF-VS-GEOMETRY`: the slope of the exact projected-equinox longitude from
//!   `p06e`'s `π_A`, `Π_A` and `p_A` (tolerance 1e-5 ″/cy).
//!
//! The offline test starts from the scenario's committed SW92 elements at J2000; the data test from
//! the DE440 initial conditions in a fresh header.440 at JDEPOC.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

mod common;

use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{EclipticOfDate, Heliocentric, Icrf, Position, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_CENTURY, Epoch, J2000_JD, Tdb, Tt};
use ferroastro_core::units::{Length, TimeSpan, Velocity};
use ferroastro_frames::{ecliptic_precession, icrf_to_ecliptic_j2000, icrf_to_ecliptic_of_date};
use ferroastro_ladder::single_body::{Rung, SingleBody};
use ferroastro_precession::fit::fit_line;
use ferroastro_precession::park2017::ReferencePlane;
use ferroastro_testdata::fetch_product;
use ferroastro_time::Convert;
use ferroastro_vectors::VectorSet;
use std::f64::consts::TAU;

/// Span of the run (NUM-014 / WP9 brief: 200 years).
const SPAN_JULIAN_YEARS: f64 = 200.0; // lint: allow-literal(run length set by the WP9 brief)

fn arcsec_per_rad() -> f64 {
    180.0 * 3600.0 / std::f64::consts::PI // lint: allow-literal(arcsec per radian)
}

/// Unwrap a sequence of angles (rad) so consecutive values differ by less than π.
fn unwrap(xs: &mut [f64]) {
    for k in 1..xs.len() {
        let d = xs[k] - xs[k - 1];
        xs[k] -= TAU * (d / TAU).round();
    }
}

/// Results of one NUM-014 run (″/cy).
struct Num014 {
    inertial: f64,
    of_date: f64,
    pa_slope: f64,
    geometry_slope: f64,
    passages: usize,
    t_first: f64,
    t_last: f64,
}

fn num014(setup: &SingleBody<Icrf>, rung: &Rung) -> Num014 {
    let run = setup.perihelion_passages(rung).unwrap();
    let epoch0: Epoch<Tdb> = setup.epoch();
    // The reference plane of both measurements: the IAU 2006 mean ecliptic of J2000.
    let j2000_ecliptic = icrf_to_ecliptic_of_date(Epoch::<Tt>::j2000());
    let (pole_icrf, x_icrf) = (
        j2000_ecliptic.axis(2).unwrap(),
        j2000_ecliptic.axis(0).unwrap(),
    );
    let inertial_plane = ReferencePlane::<Icrf>::from_normal_and_x(
        pole_icrf,
        x_icrf,
        "IAU 2006 mean ecliptic of J2000, longitude from its fixed x-axis",
    )
    .unwrap();
    let km = |x: f64| Length::from_km(x).unwrap();
    let kms = |x: f64| Velocity::from_km_per_s(x).unwrap();
    let (mut t, mut li, mut ld, mut pa, mut geo) = (vec![], vec![], vec![], vec![], vec![]);
    for p in &run.passages {
        let tdb = epoch0 + TimeSpan::from_seconds(p.t).unwrap();
        let tt: Epoch<Tt> = tdb.to_scale().unwrap();
        t.push((tt.jd() - J2000_JD) / DAYS_PER_JULIAN_CENTURY);
        li.push(inertial_plane.projected_angle(p.r));
        // Express the passage state in the ecliptic of date (typed rotation).
        let s = State::<Icrf, Heliocentric>::new(
            Position::new(km(p.r[0]), km(p.r[1]), km(p.r[2])),
            p.v.map(kms),
        );
        let rot = icrf_to_ecliptic_of_date(tt);
        let s_date: State<EclipticOfDate, Heliocentric> = rot.state(&s).unwrap();
        let r_date = s_date.position().components().map(Length::km);
        // Same plane: the J2000 ecliptic pole in date axes; zero at the projected equinox of date.
        let plane = ReferencePlane::<EclipticOfDate>::from_normal_and_x(
            rot.apply(pole_icrf),
            [1.0, 0.0, 0.0],
            "IAU 2006 mean ecliptic of J2000, longitude from the projected equinox of date",
        )
        .unwrap();
        ld.push(plane.projected_angle(r_date));
        let e = ecliptic_precession(tt).unwrap();
        let (pi, big_pi, p_a) = (e.pi_a.radians(), e.big_pi_a.radians(), e.p_a.radians());
        pa.push(p_a);
        // Exact longitude, on the J2000 ecliptic, of the projected equinox of date; λ_d − λ_i is
        // its negative.
        let lambda_eq = big_pi - ((big_pi + p_a).sin() * pi.cos()).atan2((big_pi + p_a).cos());
        geo.push(-lambda_eq);
    }
    for xs in [&mut li, &mut ld, &mut pa, &mut geo] {
        unwrap(xs);
    }
    let slope = |y: &[f64]| fit_line(&t, y).unwrap().slope * arcsec_per_rad();
    Num014 {
        inertial: slope(&li),
        of_date: slope(&ld),
        pa_slope: slope(&pa),
        geometry_slope: slope(&geo),
        passages: run.passages.len(),
        t_first: t[0],
        t_last: t[t.len() - 1],
    }
}

fn report_and_check(v: &VectorSet, label: &str, setup: &SingleBody<Icrf>, n: &Num014) {
    let diff = n.of_date - n.inertial;
    println!(
        "NUM-014 {label}: {} passages, TT centuries from J2000 {:+.4}..{:+.4} (epoch {} JD {:.1})",
        n.passages,
        n.t_first,
        n.t_last,
        setup.epoch().scale_name(),
        setup.epoch().jd()
    );
    println!(
        "  frame: perihelion longitudes in the IAU 2006 J2000 ecliptic plane; inertial = from the \
         fixed x-axis (ICRF-fixed), of date = from the projected IAU 2006 equinox of date; fit: \
         linear least squares in TT Julian centuries"
    );
    println!(
        "  inertial {:.9} arcsec/cy, ecliptic of date {:.9} arcsec/cy, difference {diff:.9}",
        n.inertial, n.of_date
    );
    println!(
        "  p_A slope (p06e port) {:.9}; difference - p_A slope = {:+.3e}; exact-geometry slope {:.9}; difference - geometry = {:+.3e}",
        n.pa_slope,
        diff - n.pa_slope,
        n.geometry_slope,
        diff - n.geometry_slope
    );
    v.get("NUM014-RATE-DIFF-VS-PA")
        .unwrap()
        .check(diff - n.pa_slope)
        .unwrap();
    v.get("NUM014-RATE-DIFF-VS-GEOMETRY")
        .unwrap()
        .check(diff - n.geometry_slope)
        .unwrap();
}

/// Orbits covering [`SPAN_JULIAN_YEARS`].
fn orbits_for_span(setup: &SingleBody<Icrf>) -> u32 {
    let el = setup.initial_elements().unwrap();
    let period = ferroastro_analytic::advance::keplerian_period(setup.mu(), el.a).unwrap();
    let span_s = SPAN_JULIAN_YEARS * 365.25 * 86_400.0; // lint: allow-literal(Julian year in seconds)
    // A positive count below 1e4 (200 yr / 88 d).
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "positive, about 830"
    )]
    let n = (span_s / period.seconds()).floor() as u32;
    n
}

/// 3c (offline, PR tier): from the scenario's SW92 elements (ECLIPJ2000), rotated to ICRF with
/// the typed inverse rotation, epoch J2000.
///
/// Verifies: NUM-014
#[test]
fn num014_ecliptic_of_date_minus_inertial_is_p_a_offline() {
    let c = common::ctx();
    let (r0, v0) = c.setup.initial_state();
    let to_icrf = icrf_to_ecliptic_j2000::<f64>().inverse();
    let mut setup = SingleBody::<Icrf>::from_state(
        to_icrf.apply(r0),
        to_icrf.apply(v0),
        &c.de,
        c.scn.orbits,
        "ICRF (scenario SW92 elements rotated from ECLIPJ2000)".into(),
        "DE440 constants; initial state: scenario SW92 elements at J2000".into(),
    )
    .unwrap();
    setup.orbits = orbits_for_span(&setup);
    let n = num014(&setup, &c.rung("GR-SCHW"));
    report_and_check(&c.vs, "offline (SW92 elements, J2000)", &setup, &n);
}

/// 3 and 3b (data tier): from the DE440 initial conditions in a fresh header.440 (Mercury minus
/// Sun at JDEPOC; centre/axes/units inferred, OI-23), epoch JDEPOC.
///
/// Verifies: NUM-014
#[test]
#[ignore = "data: downloads JPL header.440 (network); run with --profile data"]
fn data_num014_ecliptic_of_date_minus_inertial_is_p_a_from_header440() {
    let c = common::ctx();
    let fetched = fetch_product("de440-header").unwrap();
    println!("de440-header {}", fetched.provenance());
    let h = Header::parse(&std::fs::read_to_string(&fetched.path).unwrap()).unwrap();
    let val = |n: &str| h.value(n).unwrap();
    let (au, day) = (val("AU"), ferroastro_core::time::SECONDS_PER_DAY);
    let r0 = ["X", "Y", "Z"].map(|k| (val(&format!("{k}1")) - val(&format!("{k}S"))) * au);
    let v0 = ["XD", "YD", "ZD"].map(|k| (val(&format!("{k}1")) - val(&format!("{k}S"))) * au / day);
    let jdepoc = val("JDEPOC");
    let epoch = Epoch::<Tdb>::from_jd_parts(jdepoc, 0.0).unwrap();
    let mut setup = SingleBody::<Icrf>::from_state(
        r0,
        v0,
        &c.de,
        c.scn.orbits,
        "ICRF (header.440 axes as inferred, OI-23)".into(),
        format!(
            "DE440 (header.440 sha256 {}); initial state: header.440 GROUP 1041 X1..ZD1 minus XS..ZDS at JDEPOC",
            fetched.sha256
        ),
    )
    .unwrap()
    .with_epoch(epoch);
    setup.orbits = orbits_for_span(&setup);
    let n = num014(&setup, &c.rung("GR-SCHW"));
    report_and_check(&c.vs, "header.440 ICs, JDEPOC", &setup, &n);
}
