// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-bench
//!
//! Speed tables (CRATE-PLAN L11, `publish = false`; CI-008, DEP-014, NUM-009): FerroAstro and
//! the WP16/WP17 oracle containers (REBOUND, Swiftest, Mercury6, Orekit) on identical scenario
//! files, each entry the median of ≥ 5 runs within one harness invocation, with the CPU model,
//! flags and runner class recorded, and accuracy columns next to every timing so that speed is
//! never traded away silently.
//!
//! * [`series`]: the oracles' plain state CSV (read) and FerroAstro outputs in the same format
//!   (write); per-run timing records.
//! * [`accuracy`]: relative energy change, Mercury position difference and the Mercury track for
//!   the Park2017 estimator, computed identically for every tool.
//! * [`stats`]: median and run-to-run spread.
//! * [`table`]: the [`SpeedTable`] (JSON + markdown) and the two-table stability check.
//!
//! The harness is `tools/bench/speed_table.sh`; the table driver is the example
//! `speed_table` (`cargo run --release -p ferroastro-bench --example speed_table -- --help`),
//! an example so that it may read reference values from `tests/vectors` (a dev-dependency)
//! without this library depending on test-support crates. The criterion benches
//! (`benches/scenarios.rs`) compile on every PR and are measured only on the fixed runner
//! (OI-69 provisional resolution).
//!
//! ```
//! use ferroastro_bench::stats::Timing;
//! let t = Timing::new(vec![2.0, 1.0, 3.0, 2.5, 1.5]).ok_or("no runs")?;
//! assert!(t.meets_minimum());
//! assert_eq!(t.median_s.to_bits(), 2.0_f64.to_bits());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod accuracy;
mod error;
pub mod series;
pub mod stats;
pub mod table;

pub use error::BenchError;
pub use series::{RunInfo, Series, read_run};
pub use stats::{MIN_RUNS, Timing};
pub use table::{
    Accuracy, Entry, Host, Measure, ScenarioInfo, SpeedTable, Stability, Status, stability,
};
