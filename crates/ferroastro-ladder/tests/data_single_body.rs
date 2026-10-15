// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real-data tier (`#[ignore]`d, network; `cargo nextest run --workspace --profile data
//! --run-ignored only`) for the single-body rungs. Pre-registered in IMPLEMENTATION-NOTES WP4:
//!
//! * D1: rung GR-SCHW from the JPL Horizons DE441 J2000 Mercury state, against Park et al. 2017
//!   Table 3 (3σ diagnostic, as WP3) and against the analytic rate of the same osculating orbit.
//! * D2: rungs J2 (Park17 J2) and LT (DE440 spin) in the **real** geometry: the Horizons DE441
//!   Mercury state in the Sun's IAU body-equator frame (`IAU_SUN`), pole = +z. Oracles: Park17
//!   Table 3 (1σ as published) and Park17 Eqs. 2–3 (NUM-015).
//! * D3: the solar spin `G·S` recomputed from a fresh download of DE440 header.440.
//!
//! Every test prints URL, retrieval date, SHA-256 and the Horizons `signature.version`.

mod common;
mod horizons;

use ferroastro_analytic::advance::schwarzschild_advance;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{EclipticJ2000, Frame, SunEquatorIau};
use ferroastro_ladder::single_body::{Model, PoleSpec, Rung, SingleBody, de440_solar_spin};
use ferroastro_testdata::fetch_product;

/// Verifies: PHY-005, NUM-015
#[test]
#[ignore = "data: downloads a JPL Horizons DE441 state (network); run with --profile data"]
fn data_gr_rung_from_horizons_de441_state() {
    let c = common::ctx();
    let h = horizons::mercury_state("ECLIPTIC", "horizons-199-sun-j2000-vectors-wp4.json");
    println!("{}", h.provenance());
    let setup = SingleBody::<EclipticJ2000>::from_state(
        h.r,
        h.v,
        &c.de,
        c.scn.orbits,
        format!(
            "initial osculating orbit plane (axes fixed in {} = Horizons '{}'); zero at the initial periapsis",
            EclipticJ2000::NAME,
            h.frame
        ),
        format!(
            "DE440 (header.440 sha256 {}); initial state: Horizons {} sha256 {}",
            c.de.table().source().sha256,
            h.source,
            h.fetched.sha256
        ),
    )
    .unwrap();
    let r = setup.run(&c.rung("GR-SCHW")).unwrap();
    common::show(&r);
    let num = r.numerical_arcsec_per_century();
    c.check_num015(&r);
    let el = setup.initial_elements().unwrap();
    let gm_me = c.de.gm(ferroastro_constants::Body::Mercury);
    let osc = schwarzschild_advance(
        c.de.gm_sun(),
        c.de.speed_of_light(),
        el.a,
        el.e,
        c.de.gm_sun() + gm_me,
    )
    .unwrap()
    .arcsec_per_century();
    println!(
        "osculating J2000 (mu = GM_sun) a = {:.3} km, e = {:.10}: analytic {osc:.6} arcsec/cy; numerical {num:.6}",
        el.a.km(),
        el.e
    );
    c.check("PHY005-GR-NUM-MINUS-ANALYTIC", num - osc);
    let park = c.value("PARK17-GE-RATE");
    println!(
        "numerical - Park17 Table 3 GE ({park}) = {:+.6} arcsec/cy (3-sigma diagnostic)",
        num - park
    );
    c.check("PARK17-GE-RATE", num);
}

#[expect(clippy::unwrap_used, reason = "test helper: failure is a test failure")]
fn iau_sun_setup(c: &common::Ctx) -> (SingleBody<SunEquatorIau>, horizons::HorizonsState) {
    let h = horizons::mercury_state(
        "BODY EQUATOR",
        "horizons-199-sun-j2000-iau-sun-vectors.json",
    );
    println!("{}", h.provenance());
    assert!(
        h.frame.contains("IAU_SUN"),
        "unexpected Horizons frame: {}",
        h.frame
    );
    let s = SingleBody::<SunEquatorIau>::from_state(
        h.r,
        h.v,
        &c.de,
        c.scn.orbits,
        format!(
            "initial osculating orbit plane; axes fixed in {} (Horizons '{}'); zero at the initial periapsis",
            SunEquatorIau::NAME,
            h.frame
        ),
        format!(
            "DE440 (header.440 sha256 {}); initial state and solar pole: Horizons {} sha256 {}",
            c.de.table().source().sha256,
            h.source,
            h.fetched.sha256
        ),
    )
    .unwrap();
    (s, h)
}

/// Verifies: PHY-010, PHY-011, NUM-015
#[test]
#[ignore = "data: downloads a JPL Horizons DE441 state in the IAU_SUN frame (network); run with --profile data"]
fn data_j2_and_lt_rungs_in_real_solar_equator_geometry() {
    let c = common::ctx();
    let (setup, _h) = iau_sun_setup(&c);
    let pole = PoleSpec::Fixed([0.0, 0.0, 1.0]);
    let mut j2 = Rung::custom(
        "J2-REAL",
        "J2 (Park17 J2), real IAU solar pole",
        Model::J2 {
            j2: c.value("PARK17-J2SUN"),
        },
    )
    .with_pole(pole);
    j2.extra_constants = "J2 = 2.25e-7 (Park17 Table 1)".into();
    let r = setup.run(&j2).unwrap();
    common::show(&r);
    println!(
        "inclination of Mercury's J2000 osculating orbit to the IAU solar equator: {:.6} deg",
        r.inclination.radians().to_degrees()
    );
    c.check_num015(&r);
    c.check("PARK17-J2-RATE", r.numerical_arcsec_per_century());

    let lt = Rung::custom(
        "LT-REAL",
        "LT (DE440 spin), real IAU solar pole",
        Model::LenseThirring,
    )
    .with_pole(pole);
    let r = setup.run(&lt).unwrap();
    common::show(&r);
    c.check_num015(&r);
    c.check("PARK17-LT-RATE", r.numerical_arcsec_per_century());
}

/// Verifies: PHY-011, PHY-002
#[test]
#[ignore = "data: downloads JPL DE440 header.440 (network); run with --profile data"]
fn data_solar_spin_from_fresh_header_440() {
    let c = common::ctx();
    let fetched = fetch_product("de440-header").unwrap();
    println!("de440-header {}", fetched.provenance());
    let h = Header::parse(&std::fs::read_to_string(&fetched.path).unwrap()).unwrap();
    let v = |n: &str| h.value(n).unwrap();
    let day = ferroastro_core::time::SECONDS_PER_DAY;
    let gms = v("GMS") * v("AU").powi(3) / (day * day);
    let gs = gms * v("MOISUN") * v("ASUN") * v("ASUN") * v("OMGSUN") / day;
    let committed = de440_solar_spin(&c.de).unwrap().km5_per_s3();
    println!(
        "G S from header.440: {gs:.16e} km^5/s^3 (tokens GMS {}, MOISUN {}, ASUN {}, OMGSUN {}); committed table: {committed:.16e}; rel. diff {:.2e}",
        h.token("GMS").unwrap(),
        h.token("MOISUN").unwrap(),
        h.token("ASUN").unwrap(),
        h.token("OMGSUN").unwrap(),
        (gs - committed).abs() / committed
    );
    assert!((gs - committed).abs() <= 8.0 * f64::EPSILON * committed); // lint: allow-literal(f64 round-off of a six-factor product, in ulps)
}
