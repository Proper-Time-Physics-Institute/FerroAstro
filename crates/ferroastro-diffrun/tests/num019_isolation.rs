// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP20 checks I2, I4 and I5 (PR tier, pinned header.440, CI-005): solar J2 and Lense–Thirring
//! isolated in **N-body** mode. Sun + Mercury as two bodies with their DE440 GMs from header.440,
//! 1001 perihelion passages; each term toggled against the Newtonian run through the harness
//! (one IC hash, asserted), the Park2017 estimator on the passages.
//!
//! * I2: pole along Mercury's initial heliocentric orbit normal (equatorial geometry), J2 at
//!   Park17's 2.25e-7 (`PARK17-J2SUN`), LT at the DE440 spin; compared with the planar analytic
//!   rates of the initial osculating orbit of the relative motion (`μ′ = GM☉ + GM_Me`,
//!   `GS′ = GS(1 + GM_Me/GM☉)`: both reactions are on) and with the requirement's printed values.
//! * I5: as I2 with the DE440 J2.
//! * I4: the real WGCCRE 2015 pole; reported only (the inclined J2 formula's citation is OI-37,
//!   UNVERIFIED, so it is not an oracle). The pole used is printed (PHY-010 metadata).
//!
//! Comparison basis and bounds: vector `NUM019-ISOLATION-NBODY-DIFF` (IMPLEMENTATION-NOTES WP20).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use ferroastro_analytic::advance::{
    j2_inclined_rate, lense_thirring_rate, rate_arcsec_per_century,
};
use ferroastro_analytic::kepler::Elements;
use ferroastro_analytic::vector::{cross, dot, norm};
use ferroastro_constants::De440;
use ferroastro_core::units::{Angle, GravParam, SpinGm};
use ferroastro_diffrun::{
    PassageSource, Plan, PlanResult, Protocol, RunOptions, SwitchToggle, run_plan,
};
use ferroastro_propagate::{DataSources, SolarOverrides, de440_solar_spin};
use ferroastro_scenario::{BodyName, ScenarioSpec};
use ferroastro_vectors::VectorSet;

const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as num013a_scenario)

fn template() -> ScenarioSpec {
    let mut t = common::scenario("sun-mercury-eih-header440.toml");
    t.forces.gr_eih = false;
    t.name = "wp20-num019".into();
    t
}

fn toggles(lt: bool) -> Vec<SwitchToggle> {
    let mut v = vec![SwitchToggle {
        label: "J2".into(),
        apply: |f| f.sun_j2 = true,
    }];
    if lt {
        v.push(SwitchToggle {
            label: "LT".into(),
            apply: |f| f.lense_thirring = true,
        });
    }
    v
}

/// The initial relative orbit: (r, v, μ′, `GM_Me/GM☉`).
fn initial_orbit(data: &DataSources<'_>) -> ([f64; 3], [f64; 3], f64, f64) {
    let ic = ferroastro_propagate::initial_conditions(&template(), data).unwrap();
    let rec = |b: BodyName| ic.records.iter().find(|x| x.naif == b.naif()).unwrap();
    let (s, m) = (rec(BodyName::Sun), rec(BodyName::Mercury));
    let d = |a: [[f64; 2]; 3], b: [[f64; 2]; 3]| [0, 1, 2].map(|k| a[k][0] - b[k][0]);
    (
        d(m.position, s.position),
        d(m.velocity, s.velocity),
        s.gm[0] + m.gm[0],
        m.gm[0] / s.gm[0],
    )
}

fn run(data: &DataSources<'_>, lt: bool, vs: &VectorSet) -> PlanResult {
    let t = template();
    let plan = Plan::switches("wp20-num019", &t, &toggles(lt), &[t.span.years]).unwrap();
    let est = common::park17_estimator(
        common::table2_model(vs, true),
        JACKKNIFE_BLOCKS,
        PassageSource::DenseOutputEvents,
        None,
    );
    let r = run_plan(&plan, data, &est, &RunOptions::default()).unwrap();
    println!("{}", r.markdown(&|_, _| None));
    let first = &r.configs[0].ic_hashes[0];
    assert!(
        r.configs
            .iter()
            .all(|c| c.ic_hashes.iter().all(|x| x == first))
    );
    for c in &r.configs {
        println!("model [{}]: {}", c.label, c.model);
    }
    r
}

struct Analytic {
    j2_planar: f64,
    j2_inclined: f64,
    lt_planar: f64,
    lt_inclined: f64,
    inclination_deg: f64,
}

fn analytic(data: &DataSources<'_>, j2: f64, pole: [f64; 3]) -> Analytic {
    let de = De440::load().unwrap();
    let (r0, v0, mu1, ratio) = initial_orbit(data);
    let el = Elements::from_vectors(r0, v0, mu1).unwrap();
    let mu = GravParam::from_km3_per_s2(mu1).unwrap();
    let h = cross(r0, v0);
    let inc = (dot(h, pole) / (norm(h) * norm(pole)))
        .clamp(-1.0, 1.0)
        .acos();
    let gs = de440_solar_spin(&de, "wp20").unwrap().km5_per_s3() * (1.0 + ratio);
    let gs = SpinGm::from_km5_per_s3(gs).unwrap();
    let c = de.speed_of_light();
    let at = |i: f64| Angle::from_radians(i).unwrap();
    let j2r = |i: f64| {
        rate_arcsec_per_century(
            j2_inclined_rate(mu, j2, de.sun_radius(), el.a, el.e, at(i)).unwrap(),
        )
    };
    let ltr = |i: f64| {
        rate_arcsec_per_century(
            lense_thirring_rate(gs, c, de.ppn_gamma(), el.a, el.e, at(i)).unwrap(),
        )
    };
    println!(
        "initial osculating relative orbit (mu' = GM_sun + GM_Me = {mu1} km^3/s^2): a = {:.3} km, e = {:.10}; GS' = GS (1 + {ratio:.6e})",
        el.a.km(),
        el.e
    );
    Analytic {
        j2_planar: j2r(0.0),
        j2_inclined: j2r(inc),
        lt_planar: ltr(0.0),
        lt_inclined: ltr(inc),
        inclination_deg: inc.to_degrees(),
    }
}

fn row(r: &PlanResult, label: &str) -> f64 {
    r.rows_of(Protocol::Add)
        .find(|w| w.combination.label == label)
        .unwrap()
        .value
}

fn equatorial_pole() -> [f64; 3] {
    let (h, prov) = common::header();
    let data = DataSources::none().with_header(&h, &prov);
    let (r0, v0, _, _) = initial_orbit(&data);
    let n = cross(r0, v0);
    let k = norm(n);
    n.map(|x| x / k)
}

/// I2 and I5: equatorial geometry, Park17 J2 and LT, then the DE440 J2.
///
/// Verifies: NUM-019, NUM-013, PHY-010, PHY-011
#[test]
fn equatorial_nbody_j2_and_lt_match_their_analytic_rates() {
    let vs = VectorSet::load_canonical().unwrap();
    let oracle = vs.get("NUM019-ISOLATION-NBODY-DIFF").unwrap();
    let (h, prov) = common::header();
    let pole = equatorial_pole();
    let park_j2 = vs.get("PARK17-J2SUN").unwrap().value().unwrap();
    let ov = SolarOverrides {
        j2: Some(park_j2),
        pole: Some(pole),
        spin_gm: None,
        provenance:
            "WP20 I2: J2 = PARK17-J2SUN; pole = Mercury's initial heliocentric orbit normal".into(),
    };
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_solar_overrides(&ov);
    println!("I2 pole (ICRF unit vector) {pole:?}; {}", ov.describe());
    let r = run(&data, true, &vs);
    let a = analytic(&data, park_j2, pole);
    println!(
        "I2 inclination of the orbit to the pole: {:.3e} deg",
        a.inclination_deg
    );
    let (j2, lt) = (row(&r, "J2"), row(&r, "LT"));
    let print = |what: &str, got: f64, want: f64, id: &str| {
        let d = got - want;
        println!(
            "{what}: differential {got:.9}, {id} {want:.9}, difference {d:+.3e} arcsec/cy (tolerance {:?}), margin {:.3}",
            oracle.tolerance.abs,
            d.abs() / oracle.tolerance.abs.unwrap_or(f64::NAN)
        );
        d
    };
    let printed = |id: &str| vs.get(id).unwrap().value().unwrap();
    let checks = [
        print("I2 J2 (2.25e-7)", j2, a.j2_planar, "analytic (osculating)"),
        print(
            "I2 J2 (2.25e-7)",
            j2,
            printed("PHY010-J2-PARK-ANALYTIC-RATE"),
            "PHY-010 printed",
        ),
        print("I2 LT", lt, a.lt_planar, "analytic (osculating)"),
        print(
            "I2 LT",
            lt,
            printed("PHY011-LT-ANALYTIC-RATE"),
            "PHY-011 printed",
        ),
    ];

    // I5: the DE440 J2 (no J2 override; same pole).
    let ov5 = SolarOverrides {
        j2: None,
        pole: Some(pole),
        spin_gm: None,
        provenance: "WP20 I5: DE440 J2SUN; pole = Mercury's initial heliocentric orbit normal"
            .into(),
    };
    let data5 = DataSources::none()
        .with_header(&h, &prov)
        .with_solar_overrides(&ov5);
    let de = De440::load().unwrap();
    let r5 = run(&data5, false, &vs);
    let a5 = analytic(&data5, de.sun_j2(), pole);
    let j2_de = row(&r5, "J2");
    let i5 = print(
        "I5 J2 (DE440)",
        j2_de,
        a5.j2_planar,
        "analytic (osculating)",
    );
    let de440_printed = printed("PHY010-J2-DE440-ANALYTIC-RATE");
    let i5_req = j2_de - de440_printed;
    println!(
        "I5 J2 (DE440): differential {j2_de:.9} vs PHY-010's 0.02792: {i5_req:+.3e} arcsec/cy (PHY010-J2-NUM-MINUS-ANALYTIC +/- 5e-4)"
    );
    for d in checks {
        oracle.check(d).unwrap();
    }
    oracle.check(i5).unwrap();
    vs.get("PHY010-J2-NUM-MINUS-ANALYTIC")
        .unwrap()
        .check(i5_req)
        .unwrap();
}

/// I4: the real solar pole (WGCCRE 2015), Park17 J2 and LT: reported, not asserted.
///
/// Verifies: PHY-010, PHY-011, NUM-019
#[test]
fn inclined_nbody_j2_and_lt_are_reported_with_the_pole_used() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov) = common::header();
    let park_j2 = vs.get("PARK17-J2SUN").unwrap().value().unwrap();
    let ov = SolarOverrides {
        j2: Some(park_j2),
        pole: None,
        spin_gm: None,
        provenance: "WP20 I4: J2 = PARK17-J2SUN; WGCCRE 2015 pole".into(),
    };
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_solar_overrides(&ov);
    let p = ferroastro_frames::Pole::<f64>::sun_wgccre2015().unwrap();
    let k = p.unit_vector();
    let ra = k[1].atan2(k[0]).to_degrees().rem_euclid(360.0); // lint: allow-literal(degrees in a turn)
    let dec = k[2].asin().to_degrees();
    println!(
        "I4 solar pole: WGCCRE 2015 (Archinal et al. 2018), ICRF unit vector {k:?}, RA {ra:.4} deg, Dec {dec:.4} deg"
    );
    let r = run(&data, true, &vs);
    let a = analytic(&data, park_j2, k);
    let (j2, lt) = (row(&r, "J2"), row(&r, "LT"));
    println!(
        "I4 inclination of Mercury's initial orbit to the solar equator: {:.4} deg",
        a.inclination_deg
    );
    println!(
        "I4 J2 (2.25e-7): differential {j2:.6} (Park17 mean-orbit frame, projected longitude); planar analytic {:.6}; inclined analytic along the orbit plane {:.6} (OI-37, not an oracle); differential - planar {:+.3e}, differential - inclined {:+.3e} arcsec/cy",
        a.j2_planar,
        a.j2_inclined,
        j2 - a.j2_planar,
        j2 - a.j2_inclined
    );
    println!(
        "I4 LT: differential {lt:.7}; planar analytic {:.7}; inclined (cos i) {:.7}; differential - planar {:+.3e} arcsec/cy",
        a.lt_planar,
        a.lt_inclined,
        lt - a.lt_planar
    );
    assert!(
        r.configs
            .iter()
            .any(|c| c.model.contains("WGCCRE 2015 pole"))
    );
}
