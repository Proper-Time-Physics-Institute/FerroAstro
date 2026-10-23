// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Criterion benches of the WP18 speed-table scenarios, shortened so one iteration takes well
//! under a second to a few seconds: (a) Sun + 8 planets Newtonian, (b) the 1PN Sun + Mercury
//! test particle, (c) the full system with 343 Minor bodies, KBOs and ring. They compile on every
//! PR (`cargo clippy --all-targets`) and are measured only on the fixed machine (CI-008; OI-69
//! provisional resolution). The speed table itself uses the full spans
//! (`tools/bench/speed_table.sh`). Needs the cached header.440 and the committed REF-003 states.
//!
//! Run: `cargo bench -p ferroastro-bench --bench scenarios`.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "benchmark set-up: a failure aborts the run"
)]
#![expect(
    missing_docs,
    reason = "criterion_group! generates an undocumented pub fn"
)]

use criterion::{Criterion, criterion_group, criterion_main};
use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_system::SmallBodyStates;
use std::hint::black_box;
use std::path::PathBuf;

/// (scenario, shortened span in Julian years).
const CASES: [(&str, f64); 3] = [
    ("bench-a-sun-8planets-newton-1000yr", 10.0),
    ("bench-b-sun-mercury-tp-gr1pn-1000yr", 100.0),
    ("bench-c-full-newton-minor343-100yr", 0.5),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scenarios(c: &mut Criterion) {
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
    let mut g = c.benchmark_group("wp18-scenarios");
    g.sample_size(10);
    for (name, years) in CASES {
        let mut spec = ScenarioSpec::parse(
            &std::fs::read_to_string(root().join("ci/scenarios").join(format!("{name}.toml")))
                .unwrap(),
        )
        .unwrap();
        spec.span.years = years;
        spec.output.cadence_days = None;
        g.bench_function(format!("{name}-{years}yr"), |b| {
            b.iter(|| black_box(run(black_box(&spec), &data).unwrap()));
        });
    }
    g.finish();
}

criterion_group!(benches, scenarios);
criterion_main!(benches);
