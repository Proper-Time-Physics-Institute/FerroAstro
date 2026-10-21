// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung SR-b (PHY-008): velocity-dependent 1PN terms only, in two gauges, always labelled.

mod common;

use ferroastro_core::time::Epoch;
use ferroastro_core::units::{AngularRate, TimeSpan};
use ferroastro_forces::Gauge;
use ferroastro_precession::report::RateReportBuilder;

/// Verifies: PHY-008, NUM-015
#[test]
fn sr_b_harmonic_and_areal_give_seven_and_five_sixths() {
    let c = common::ctx();
    for (id, vector, gauge) in [
        (
            "SR-b/harmonic",
            "PHY008-SRB-HARMONIC-RATE",
            Gauge::PpnIsotropicHarmonic,
        ),
        (
            "SR-b/areal",
            "PHY008-SRB-AREAL-RATE",
            Gauge::SchwarzschildAreal,
        ),
    ] {
        let r = c.setup.run(&c.rung(id)).unwrap();
        common::show(&r);
        assert_eq!(r.gauge, gauge);
        assert_eq!(
            r.report.gauge,
            gauge.label(),
            "SR-b output carries its gauge label"
        );
        c.check(vector, r.numerical_arcsec_per_century());
        c.check_num015(&r);
    }
}

/// SR-b output without a gauge label is rejected; the docs say SR-b is not an observable.
///
/// Verifies: PHY-008, PHY-015
#[test]
fn sr_b_output_without_gauge_is_rejected_and_docs_say_not_observable() {
    let unlabelled = RateReportBuilder::new()
        .rung("SR-b")
        .rate(AngularRate::ZERO)
        .frame("f")
        .epoch(Epoch::j2000())
        .span(TimeSpan::from_seconds(1.0).unwrap())
        .fit_model("m")
        .constants("k")
        .estimator("e")
        .dynamics("d")
        .build();
    let e = unlabelled.unwrap_err();
    assert_eq!(e.code(), ferroastro_core::ErrorCode::ReportMetadataMissing);
    assert!(e.to_string().contains("gauge"));
    let docs = include_str!("../src/lib.rs");
    assert!(docs.contains("**Not an observable:**") && docs.contains("never \"the SR part of"));
}
