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
//! * D4 (WP6, pre-registered with vectors NUM013-ISOLATION-DIFF and NUM012-ESTIMATOR-SPREAD
//!   before the first run): NUM-013(a) isolation and the NUM-012 estimator spread from the real
//!   DE441 state in the real `IAU_SUN` geometry (inclined orbit, so the J2/LT oracles are the
//!   inclined Park17 Eqs. 3 and 2).
//!
//! * D5 (WP7, pre-registered with vector PHY005-F64-DD-AGREEMENT before the first run): rung
//!   GR-SCHW from the DE440 initial conditions in a fresh header.440 (Mercury minus Sun at
//!   JDEPOC), integrated in f64 and in double-double. Each must meet PHY-005 and they must agree
//!   within 1e-6 ″/cy.
//!
//! Every test prints URL, retrieval date, SHA-256 and the Horizons `signature.version`.

mod common;
mod horizons;

use ferroastro_analytic::advance::schwarzschild_advance;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{EclipticJ2000, Frame, SunEquatorIau};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::TimeSpan;
use ferroastro_forces::PnCoefficients;
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

/// NUM-019: single-body part only (J2 and LT contributions vs Park17 Table 3, LT with its
/// (1 + γ) dependence and the real solar pole); the momentum-conservation clause needs the
/// N-body J2 with reaction on the Sun (M4) and is not covered here.
///
/// Verifies: PHY-010, PHY-011, NUM-015, NUM-019
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

/// D4: NUM-013(a) single-body isolation and the NUM-012 estimator spread, from the Horizons
/// DE441 J2000 state of Mercury in the real `IAU_SUN` geometry.
///
/// Verifies: NUM-013, NUM-012
#[test]
#[ignore = "data: downloads a JPL Horizons DE441 state in the IAU_SUN frame (network); run with --profile data"]
fn data_isolation_and_estimator_spread_from_horizons_de441_state() {
    let c = common::ctx();
    let (setup, _h) = iau_sun_setup(&c);
    let pole = PoleSpec::Fixed([0.0, 0.0, 1.0]);
    let zero = PnCoefficients {
        a: 0.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
    };
    let newton = setup
        .run(&Rung::custom(
            "N-TP",
            "Newtonian test particle",
            Model::Lagrangian(zero),
        ))
        .unwrap();
    println!(
        "basis: Newtonian test particle from the DE441 state, IC hash {}, rate {:.3e} arcsec/cy (numerical error)",
        newton.ic_hash,
        newton.numerical_arcsec_per_century()
    );
    let gr = Rung::custom(
        "GR-REAL",
        "GR-SCHW from the DE441 state",
        Model::Schwarzschild {
            beta: c.de.ppn_beta(),
            gamma: c.de.ppn_gamma(),
        },
    );
    let j2 = Rung::custom(
        "J2-REAL",
        "J2 (Park17 J2), real IAU solar pole",
        Model::J2 {
            j2: c.value("PARK17-J2SUN"),
        },
    )
    .with_pole(pole);
    let lt = Rung::custom(
        "LT-REAL",
        "LT (DE440 spin), real IAU solar pole",
        Model::LenseThirring,
    )
    .with_pole(pole);
    for rung in [&gr, &j2, &lt] {
        let r = setup.run(rung).unwrap();
        assert_eq!(r.ic_hash, newton.ic_hash, "{}: identical ICs", rung.id);
        let delta = r.numerical_arcsec_per_century() - newton.numerical_arcsec_per_century();
        println!(
            "NUM-013(a) {}: differential {delta:.9}, analytic {:.9} arcsec/cy (inclination {:.6} deg)",
            rung.id,
            r.analytic_arcsec_per_century(),
            r.inclination.radians().to_degrees()
        );
        c.check(
            "NUM013-ISOLATION-DIFF",
            delta - r.analytic_arcsec_per_century(),
        );
    }
    let cadence = TimeSpan::from_seconds(2.0 * SECONDS_PER_DAY).unwrap();
    for rung in [&gr, &j2, &lt] {
        let s = setup.estimator_spread(rung, cadence).unwrap();
        println!(
            "NUM-012 {}: passages {:.9}, LRL {:.9}, Park2017 linear {:.9} (jk {:.1e}), quadratic {:.9}; spread {:.2e} arcsec/cy",
            s.rung,
            s.passages,
            s.lrl,
            s.park_linear,
            s.park_linear_jackknife,
            s.park_quadratic,
            s.spread()
        );
        c.check("NUM012-ESTIMATOR-SPREAD", s.spread());
    }
}

/// D5: GR-SCHW from header.440 initial conditions, f64 and double-double (WP7).
///
/// The header's GROUP 1041 holds the DE440 integration's initial states at JDEPOC. Their centre,
/// axes and units (solar-system barycentre, ICRF, au and au/day) are inferred, not documented
/// (OI-23). They are used here only as an **initial condition**, never as an oracle: the
/// oracles are PHY-005's 42.980 ± 0.001 ″/cy and the f64/double-double agreement. A units error
/// would move the rate by orders of magnitude (it scales as a^-5/2).
///
/// Verifies: PHY-005, DEP-007, NUM-006, NUM-015
#[test]
#[ignore = "data: downloads JPL header.440 (network); run with --profile data"]
fn data_gr_schw_f64_and_double_double_from_header440_initial_conditions() {
    use ferroastro_core::frames::Icrf;
    use ferroastro_real::DoubleDouble;
    let c = common::ctx();
    let fetched = fetch_product("de440-header").unwrap();
    println!("de440-header {}", fetched.provenance());
    let h = Header::parse(&std::fs::read_to_string(&fetched.path).unwrap()).unwrap();
    let v = |n: &str| h.value(n).unwrap();
    let (au, day) = (v("AU"), ferroastro_core::time::SECONDS_PER_DAY);
    let r0 = ["X", "Y", "Z"].map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au);
    let v0 = ["XD", "YD", "ZD"].map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au / day);
    println!(
        "header.440 JDEPOC {} (CENTER {}): Mercury - Sun r0 = {r0:?} km, v0 = {v0:?} km/s \
         (centre/axes/units inferred: OI-23)",
        v("JDEPOC"),
        v("CENTER")
    );
    let setup = SingleBody::<Icrf>::from_state(
        r0,
        v0,
        &c.de,
        c.scn.orbits,
        format!(
            "initial osculating orbit plane (axes fixed in {} as inferred for header.440, OI-23); zero at the initial periapsis",
            Icrf::NAME
        ),
        format!(
            "DE440 (header.440 sha256 {}); initial state: header.440 GROUP 1041 X1..ZD1 minus XS..ZDS at JDEPOC",
            fetched.sha256
        ),
    )
    .unwrap()
    // Date the run at JDEPOC (report metadata only: single-body dynamics are epoch-free; WP9).
    .with_epoch(ferroastro_core::time::Epoch::from_jd_parts(v("JDEPOC"), 0.0).unwrap());
    let el = setup.initial_elements().unwrap();
    println!(
        "osculating heliocentric orbit at JDEPOC (mu = GM_sun): a = {:.3} km ({:.8} au), e = {:.10}",
        el.a.km(),
        el.a.km() / au,
        el.e
    );
    let rung = c.rung("GR-SCHW");
    let f = setup.run(&rung).unwrap();
    let d = setup.run_in::<DoubleDouble>(&rung).unwrap();
    common::show(&f);
    common::show(&d);
    let (rf, rd) = (
        f.numerical_arcsec_per_century(),
        d.numerical_arcsec_per_century(),
    );
    println!(
        "D5 GR-SCHW from header.440 ICs: f64 {rf:.12}, double-double {rd:.12} arcsec/cy; \
         f64 - DD = {:+.3e}; analytic (same osculating orbit) {:.9}",
        rf - rd,
        f.analytic_arcsec_per_century()
    );
    c.check("PHY005-GR-NUM-RATE", rf);
    c.check("PHY005-GR-NUM-RATE", rd);
    c.check("PHY005-F64-DD-AGREEMENT", rf - rd);
    c.check_num015(&f);
    c.check_num015(&d);
}
