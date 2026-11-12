// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared helpers of the diffrun tests: the cached header.440 (CI-005), scenario files and the
//! Park17 estimator settings.

#![allow(dead_code, reason = "each test binary uses a subset")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: a failed unwrap is a test failure"
)]

use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_diffrun::{Estimator, PassageSource};
use ferroastro_precession::park2017::{
    FitModel, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane,
    park17_table2_frequencies,
};
use ferroastro_scenario::{BodyName, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

/// header.440 from the pinned cache, with its provenance line.
pub(crate) fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

/// A committed scenario file.
pub(crate) fn scenario(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Park17 Table 2 frequencies from the SW92 Table 2a mean motions (vectors), as WP5.
pub(crate) fn table2_model(vs: &VectorSet, quadratic: bool) -> FitModel {
    let n = |b: &str| {
        vs.get(&format!("SW92-T2A-{b}-L-RATE"))
            .unwrap()
            .value()
            .unwrap()
    };
    FitModel {
        quadratic,
        frequencies: park17_table2_frequencies(
            [
                n("MERCURY"),
                n("VENUS"),
                n("EMB"),
                n("JUPITER"),
                n("SATURN"),
            ],
            "SW92 T2A",
        ),
    }
}

/// The Park2017 estimator of Mercury relative to the Sun in the Park17 mean-orbit frame.
pub(crate) fn park17_estimator(
    model: FitModel,
    blocks: usize,
    source: PassageSource,
    window_jd: Option<(f64, f64)>,
) -> Estimator {
    Estimator {
        park: Park2017 {
            plane: ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap(),
            definition: LongitudeDefinition::Projected,
            passages: PassageConfig::default(),
            model,
            jackknife_blocks: blocks,
        },
        source,
        body: BodyName::Mercury,
        central: BodyName::Sun,
        window_jd,
    }
}
