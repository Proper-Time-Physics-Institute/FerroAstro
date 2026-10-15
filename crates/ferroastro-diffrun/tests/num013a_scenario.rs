// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP14 check 6, NUM-013(a) through the harness: Sun + Mercury test particle from header.440
//! (PR tier, pinned cache, CI-005). Each single-body term (`gr_1pn_central`, `sun_j2`,
//! `lense_thirring`) is toggled on against the Newtonian scenario from the same ICs (one IC hash,
//! asserted), the Park2017 estimator is applied to every run, and the differential rate is
//! compared with the analytic rate of the same term for the initial osculating orbit
//! (vector `NUM013-ISOLATION-DIFF-SCENARIO`, comparison basis in its rationale).

mod common;

use ferroastro_analytic::advance::{
    j2_inclined_rate, lense_thirring_rate, rate_arcsec_per_century, schwarzschild_advance,
};
use ferroastro_analytic::kepler::Elements;
use ferroastro_analytic::vector::{cross, dot, norm};
use ferroastro_constants::De440;
use ferroastro_core::units::Angle;
use ferroastro_diffrun::{PassageSource, Plan, Protocol, RunOptions, SwitchToggle, run_plan};
use ferroastro_propagate::{DataSources, de440_solar_spin};
use ferroastro_vectors::VectorSet;

const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as the Park17 data test)

/// Verifies: NUM-013, CI-009
#[test]
fn single_body_toggles_through_the_harness_match_their_analytic_rates() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov) = common::header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut template = common::scenario("sun-mercury-gr1pn-header440.toml");
    template.forces.gr_1pn_central = false;
    template.name = "wp14-num013a".into();
    let toggles = [
        SwitchToggle {
            label: "GR-SCHW".into(),
            apply: |f| f.gr_1pn_central = true,
        },
        SwitchToggle {
            label: "J2".into(),
            apply: |f| f.sun_j2 = true,
        },
        SwitchToggle {
            label: "LT".into(),
            apply: |f| f.lense_thirring = true,
        },
    ];
    let plan = Plan::switches("wp14-num013a", &template, &toggles, &[template.span.years]).unwrap();
    let est = common::park17_estimator(
        common::table2_model(&vs, true),
        JACKKNIFE_BLOCKS,
        PassageSource::DenseOutputEvents,
        None,
    );
    let r = run_plan(&plan, &data, &est, &RunOptions::default()).unwrap();
    println!("{}", r.markdown(&|_, _| None));
    // One IC hash for every run (the switches are not ICs).
    let first = &r.configs[0].ic_hashes[0];
    assert!(
        r.configs
            .iter()
            .all(|c| c.ic_hashes.iter().all(|x| x == first))
    );

    // Analytic rates of the initial osculating orbit (mu = GM_sun, the integrated dynamics).
    let de = De440::load().unwrap();
    let sp = plan.configs[0].legs[0].clone();
    let ic = ferroastro_propagate::initial_conditions(&sp, &data).unwrap();
    let particle = ferroastro_scenario::BodyName::Mercury.naif();
    let mercury = ic.records.iter().find(|x| x.naif == particle).unwrap();
    let (r0, v0) = (
        mercury.position.map(|p| p[0]),
        mercury.velocity.map(|p| p[0]),
    );
    let mu = de.gm_sun();
    let el = Elements::from_vectors(r0, v0, mu.km3_per_s2()).unwrap();
    let pole = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
        .unwrap()
        .unit_vector();
    let h_dir = cross(r0, v0);
    let inc = Angle::from_radians((dot(h_dir, pole) / norm(h_dir)).acos()).unwrap();
    let c = de.speed_of_light();
    let gr = schwarzschild_advance(mu, c, el.a, el.e, mu).unwrap();
    let analytic = |id: &str| -> f64 {
        match id {
            "GR-SCHW" => gr.arcsec_per_century(),
            "J2" => rate_arcsec_per_century(
                j2_inclined_rate(mu, de.sun_j2(), de.sun_radius(), el.a, el.e, inc).unwrap(),
            ),
            _ => rate_arcsec_per_century(
                lense_thirring_rate(
                    de440_solar_spin(&de, "num013a").unwrap(),
                    c,
                    de.ppn_gamma(),
                    el.a,
                    el.e,
                    inc,
                )
                .unwrap(),
            ),
        }
    };
    println!(
        "initial osculating orbit (mu = GM_sun): a = {:.3} km, e = {:.10}, inclination to the WGCCRE 2015 solar equator {:.6} deg",
        el.a.km(),
        el.e,
        inc.radians().to_degrees()
    );
    let oracle = vs.get("NUM013-ISOLATION-DIFF-SCENARIO").unwrap();
    for row in r.rows_of(Protocol::Add) {
        let id = row.combination.label.as_str();
        let a = analytic(id);
        let d = row.value - a;
        println!(
            "NUM-013(a) scenario {id}: differential {:.9} (dense-output passages; sampled path n/a for test particles), analytic {a:.9}, difference {d:+.3e} arcsec/cy (tolerance {:?}), margin {:.3}",
            row.value,
            oracle.tolerance.abs,
            d.abs() / oracle.tolerance.abs.unwrap_or(f64::NAN)
        );
        oracle.check(d).unwrap();
    }
}
