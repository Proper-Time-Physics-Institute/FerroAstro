// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! One code path (DESIGN E12–E14): the viewer's passages and rate equal those of a
//! `ferroastro_propagate::run` of its exported scenario through the diffrun estimator; chunking
//! does not change a bit; the osculating ϖ of the elements series is the passage ϖ.

mod common;

use common::{IAS15, WHFAST, check, configured, run_to_end, vectors};
use ferroastro_analytic::vector::{cross, eccentricity_vector};
use ferroastro_constants::Body;
use ferroastro_core::frames::{Heliocentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_diffrun::{PassageSource, Track};
use ferroastro_precession::park2017::{
    PassageConfig, ReferencePlane, SampledOrbit, SampledPassage, find_passages,
};
use ferroastro_propagate::DataSources;
use ferroastro_scenario::ScenarioSpec;
use ferroastro_viewer::config::bits;
use ferroastro_viewer::{FAV_OK, Viewer, estimator, ic};

const YEARS: u32 = 20;

fn passages_bits(ps: &[SampledPassage]) -> Vec<u64> {
    ps.iter()
        .flat_map(|p| {
            let mut w = vec![p.t_s.to_bits(), p.direction_error_rad.to_bits()];
            w.extend(p.r.iter().chain(&p.v).map(|x| x.to_bits()));
            w
        })
        .collect()
}

/// Verifies: CI-009, API-004
#[test]
fn e12_viewer_equals_propagate_and_the_diffrun_estimator() {
    if !common::system_mode_or_skip("e12_viewer_equals_propagate_and_the_diffrun_estimator") {
        return;
    }
    let vs = vectors();
    for engine in [WHFAST, IAS15] {
        let mask = bits::VENUS | bits::GR_1PN;
        let mut v = configured(mask, &[engine], YEARS);
        run_to_end(&mut v, 0.25);
        assert_eq!(v.fit(f64::NAN, f64::NAN), FAV_OK, "{}", v.error());
        // The exported TOML is the scenario that ran.
        let toml = String::from_utf8(v.scenario_bytes().to_vec()).unwrap();
        let spec = ScenarioSpec::parse(&toml).unwrap();
        assert_eq!(spec.digest(), v.scenario().unwrap().digest());
        let data = DataSources::none().with_header(ic::header().unwrap(), ic::header_provenance());
        let out = ferroastro_propagate::run(&spec, &data).unwrap();
        assert_eq!(out.ic_hash, v.ic_hash());
        let mut track = Track::new(out.epoch_jd_tdb[0] + out.epoch_jd_tdb[1]);
        assert_eq!(track.absorb(&out, 1, 10), 0);
        // Passages: one find_passages over the whole track, built as the diffrun estimator does.
        let epochs: Vec<Epoch<Tdb>> = track
            .t
            .iter()
            .map(|t| Epoch::from_jd_parts(track.epoch_jd_tdb, t / SECONDS_PER_DAY).unwrap())
            .collect();
        let states: Vec<State<Icrf, Heliocentric>> = track
            .r
            .iter()
            .zip(&track.v)
            .map(|(r, w)| {
                let l = |x: f64| Length::from_km(x).unwrap();
                let s = |x: f64| Velocity::from_km_per_s(x).unwrap();
                State::new(
                    Position::new(l(r[0]), l(r[1]), l(r[2])),
                    [s(w[0]), s(w[1]), s(w[2])],
                )
            })
            .collect();
        let orbit = SampledOrbit::from_states(&epochs, &states).unwrap();
        let whole = find_passages(&orbit, &PassageConfig::default()).unwrap();
        assert_eq!(whole.len(), v.passage_records().len());
        assert_eq!(passages_bits(&whole), passages_bits(v.passage_records()));
        // Rate: the diffrun Estimator with the viewer's fit model and block count.
        let (model, blocks) = v.last_fit().unwrap();
        let est = estimator(model.clone(), blocks).unwrap();
        let e = est.estimate(&track, PassageSource::SampledStates).unwrap();
        let mine = v.fit_record()[0];
        println!(
            "{engine:?}: viewer {mine} diffrun {} ({})",
            e.rate,
            est.id(PassageSource::SampledStates)
        );
        check(
            &vs,
            "VIEWER-E12-SAME-PATH-REL",
            ((mine - e.rate) / e.rate).abs(),
        );
    }
}

fn run_in_chunks(mask: u32, chunk: f64) -> Viewer {
    let mut v = configured(mask, &[IAS15], YEARS);
    run_to_end(&mut v, chunk);
    v
}

fn bits_of(x: &[f64]) -> Vec<u64> {
    x.iter().map(|z| z.to_bits()).collect()
}

/// Verifies: CI-010, API-004
#[test]
fn e13_chunking_does_not_change_a_bit() {
    if !common::system_mode_or_skip("e13_chunking_does_not_change_a_bit") {
        return;
    }
    let mask = bits::VENUS | bits::GR_1PN | bits::LENSE_THIRRING;
    let one = run_in_chunks(mask, f64::from(YEARS));
    let many = run_in_chunks(mask, 0.25);
    assert!(!one.passage_records().is_empty());
    assert_eq!(
        passages_bits(one.passage_records()),
        passages_bits(many.passage_records())
    );
    assert_eq!(bits_of(one.passages()), bits_of(many.passages()));
    assert_eq!(bits_of(one.elements()), bits_of(many.elements()));
    assert_eq!(bits_of(one.bodies()), bits_of(many.bodies()));
}

/// Verifies: NUM-011, API-004
#[test]
fn e14_osculating_varpi_is_the_passage_varpi() {
    if !common::system_mode_or_skip("e14_osculating_varpi_is_the_passage_varpi") {
        return;
    }
    let vs = vectors();
    let v = run_in_chunks(bits::VENUS | bits::JUPITER | bits::GR_1PN, 5.0);
    let de = ic::de440().unwrap();
    let mu = de.gm_sun().km3_per_s2() + de.gm(Body::Mercury).km3_per_s2();
    let plane = ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap();
    let worst = v
        .passage_records()
        .iter()
        .map(|p| {
            let e = eccentricity_vector(p.r, p.v, mu);
            let a = plane.dog_leg_angle(e, cross(cross(p.r, p.v), e)).unwrap();
            let b = plane.dog_leg_angle(p.r, p.v).unwrap();
            let d = (a - b).rem_euclid(std::f64::consts::TAU);
            d.min(std::f64::consts::TAU - d)
        })
        .fold(0.0, f64::max);
    let interp = v
        .passage_records()
        .iter()
        .map(|p| p.direction_error_rad)
        .fold(0.0, f64::max);
    let resid = v
        .passage_records()
        .iter()
        .map(|p| {
            let rn = p.r.iter().map(|x| x * x).sum::<f64>().sqrt();
            let vn = p.v.iter().map(|x| x * x).sum::<f64>().sqrt();
            (p.r.iter().zip(&p.v).map(|(a, b)| a * b).sum::<f64>() / (rn * vn)).abs()
        })
        .fold(0.0, f64::max);
    println!(
        "E14: worst {worst:e} rad; largest interpolation direction error {interp:e} rad; largest \
         |r.v|/(|r||v|) at a passage {resid:e}"
    );
    check(&vs, "VIEWER-E14-VARPI-IDENTITY", worst);
}
