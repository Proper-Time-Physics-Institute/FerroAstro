// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-015: every rung output of the single-body catalogue carries frame, epoch, fit span, fit
//! model and constants version (plus gauge and estimator). Short runs: only the metadata matter.

mod common;

use ferroastro_ladder::single_body::single_body_rungs;

/// Verifies: PHY-015, PHY-008, NUM-012
#[test]
fn every_single_body_rung_reports_full_metadata() {
    let mut c = common::ctx();
    c.setup.orbits = 5;
    for rung in single_body_rungs(&c.scn, &c.de).unwrap() {
        let r = c.setup.run(&rung).unwrap();
        let m = &r.report;
        for (name, field) in [
            ("frame", &m.frame),
            ("fit model", &m.fit_model),
            ("constants", &m.constants),
            ("gauge", &m.gauge),
            ("estimator", &m.estimator),
            ("dynamics", &m.dynamics),
        ] {
            assert!(!field.trim().is_empty(), "{}: empty {name}", rung.id);
        }
        assert!(m.span.seconds() > 0.0);
        assert!(
            m.constants.contains("sha256"),
            "constants version must be pinned: {}",
            m.constants
        );
        assert!(m.epoch.jd().is_finite());
        println!("{}: OK ({})", rung.id, m.gauge);
    }
}
