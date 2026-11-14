// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Criterion benchmark of WP26 check P9 (NUM-022): run-level throughput against the number of
//! threads, with at least as many runs as logical CPUs. The batch is `available_parallelism()`
//! (at least 32) independent 1-yr runs of the WP24 ABM scenario (EIH Sun + 8 planets + geocentric
//! Moon from header.440, ABM13 PECEC h = 1/16 d, f64, pairwise kernels), each with a distinct
//! name; `run_batch` executes them on 1, 2, 4, 8, 16, 24 and 32 threads (rayon). Throughput is
//! runs per second (criterion's `Elements`); the efficiency E(T) = thr(T) / (T · thr(1)) is
//! computed from the report.
//!
//! ```text
//! cargo bench -p ferroastro-bench --bench run_scaling
//! ```
//!
//! Needs the cached header.440 (CI-005).

#![expect(
    clippy::unwrap_used,
    reason = "benchmark set-up: a failure aborts the run"
)]
#![expect(
    missing_docs,
    reason = "criterion_group! generates an undocumented pub fn"
)]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, run_batch};
use ferroastro_scenario::ScenarioSpec;
use std::hint::black_box;
use std::path::PathBuf;

/// Thread counts of the scan.
const THREADS: [usize; 7] = [1, 2, 4, 8, 16, 24, 32];
/// Minimum batch size (runs >= cores on the 32-thread fixed machine).
const MIN_RUNS: usize = 32;

fn scaling(c: &mut Criterion) {
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    let prov = f.provenance();
    let header = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let data = DataSources::none().with_header(&header, &prov);
    let base = ScenarioSpec::parse(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../ci/scenarios/abm-sun-planets-moon-eih-40yr.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let runs = cores.max(MIN_RUNS);
    let batch: Vec<ScenarioSpec> = (0..runs)
        .map(|k| {
            let mut s = base.clone();
            s.span.years = 1.0;
            s.output.cadence_days = None;
            s.name = format!("wp26-p9-{k}");
            s
        })
        .collect();
    println!(
        "P9: {runs} runs on a machine with {cores} logical CPUs (ferroastro-propagate built with \
         `parallel` through this crate's dev-dependency)"
    );
    let mut g = c.benchmark_group("wp26-run-scaling");
    g.sample_size(10);
    g.throughput(Throughput::Elements(u64::try_from(runs).unwrap()));
    for t in THREADS.into_iter().filter(|&t| t <= cores.max(1)) {
        g.bench_function(format!("threads-{t:02}"), |b| {
            b.iter(|| black_box(run_batch(black_box(&batch), &data, t).unwrap()));
        });
    }
    g.finish();
}

criterion_group!(benches, scaling);
criterion_main!(benches);
