// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-024 criterion suite (WP28): the Subbotin et al. 2023 configuration (16 EIH Majors with the
//! EPM2021 tiering, 338 Minor asteroids, ABM13 PECEC at h = 1/16 d, relativistic SSB) in binary64
//! with plain sums (`bench-h-num024-epm16-f64-40yr`), in the NUM-007 mixed double-double mode
//! (`ephemeris-mixed-epm16-40yr`) and the DE440-class mixed run (`bench-j-de440class-mixed-40yr`),
//! over a shortened span. Criterion needs >= 10 samples, so the full 40-yr wall times (median of 5,
//! CI-008) come from `tools/bench/speed_table.sh --suite wp28`; this bench tracks the per-step cost
//! (criterion's throughput is in steps). Span: `FA_NUM024_YEARS` (default 0.25 yr = 1461 steps).
//! It compiles on every PR and is measured only on the fixed machine (CI-008; OI-69). Needs the
//! cached header.440 and the committed REF-003 states.
//!
//! Run: `taskset -c 6 cargo bench -p ferroastro-bench --bench num024`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "benchmark set-up: a failure aborts the run"
)]
#![expect(
    missing_docs,
    reason = "criterion_group! generates an undocumented pub fn"
)]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_system::SmallBodyStates;
use std::hint::black_box;
use std::path::PathBuf;

/// The NUM-024 scenarios.
const CASES: [&str; 3] = [
    "bench-h-num024-epm16-f64-40yr",
    "ephemeris-mixed-epm16-40yr",
    "bench-j-de440class-mixed-40yr",
];
/// Default shortened span (Julian years).
const DEFAULT_YEARS: f64 = 0.25;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn num024(c: &mut Criterion) {
    let years = std::env::var("FA_NUM024_YEARS")
        .ok()
        .and_then(|y| y.parse().ok())
        .unwrap_or(DEFAULT_YEARS);
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let header = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let small = SmallBodyStates::parse(
        &std::fs::read_to_string(root().join("tests/goldens/sb441-n373s-jdepoc-states.csv"))
            .unwrap(),
    )
    .unwrap();
    let data = DataSources::none()
        .with_header(&header, &prov)
        .with_small_bodies(&small);
    let mut g = c.benchmark_group("num024");
    g.sample_size(10);
    for name in CASES {
        let mut spec = ScenarioSpec::parse(
            &std::fs::read_to_string(root().join("ci/scenarios").join(format!("{name}.toml")))
                .unwrap(),
        )
        .unwrap();
        spec.span.years = years;
        spec.output.cadence_days = None;
        let steps = run(&spec, &data).unwrap().stats.steps;
        g.throughput(Throughput::Elements(steps));
        g.bench_function(format!("{name}-{years}yr"), |b| {
            b.iter(|| black_box(run(black_box(&spec), &data).unwrap()));
        });
    }
    g.finish();
}

criterion_group!(benches, num024);
criterion_main!(benches);
