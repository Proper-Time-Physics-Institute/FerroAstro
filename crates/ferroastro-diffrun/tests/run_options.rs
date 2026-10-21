// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `RunOptions` validation (PR #13 judge review R13-CODE-5): a zero, negative or non-finite
//! `segment_years` never advanced an N-body leg (an endless loop), and a tiny one could overflow
//! the segment counter. Both are refused with `FA-E1400` before any integration (PR tier,
//! header.440 from the pinned cache, CI-005).

mod common;

use ferroastro_core::ErrorCode;
use ferroastro_diffrun::{MAX_SEGMENTS, PassageSource, RunOptions, run_leg};
use ferroastro_precession::park2017::FitModel;
use ferroastro_propagate::DataSources;

/// Verifies: API-002, CI-009
#[test]
fn invalid_segment_lengths_are_refused_before_running() {
    let spec = common::scenario("sun-planets-moon-newton-j2-10yr.toml");
    let ok = RunOptions::default();
    ok.validate().unwrap();
    assert!(ok.segments(&spec).unwrap() >= 1);
    let tiny = spec.span.years.abs() / (f64::from(MAX_SEGMENTS) * 2.0);
    let (h, prov) = common::header();
    let data = DataSources::none().with_header(&h, &prov);
    let est = common::park17_estimator(
        FitModel {
            quadratic: true,
            frequencies: vec![],
        },
        2,
        PassageSource::SampledStates,
        None,
    );
    let t0 = std::time::Instant::now();
    for segment_years in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE, tiny] {
        let opts = RunOptions {
            segment_years,
            ..RunOptions::default()
        };
        let e = opts.segments(&spec).unwrap_err();
        assert_eq!(e.code(), ErrorCode::DiffrunPlan, "{segment_years}: {e}");
        let e = run_leg(&spec, &data, &est, &opts, None).unwrap_err();
        assert_eq!(e.code(), ErrorCode::DiffrunPlan, "{segment_years}: {e}");
        println!("segment_years {segment_years:e}: {e}");
    }
    assert!(
        t0.elapsed().as_secs_f64() < 5.0, // lint: allow-literal(wall-clock bound: refusals run no integration)
        "refusals must not integrate"
    );
}
