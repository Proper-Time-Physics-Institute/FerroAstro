// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The speed table (CI-008): one entry per tool and scenario, all from one harness invocation on
//! one machine, with the host's CPU model, flags and runner class; JSON (consumed later by
//! `ferroastro-report`) and markdown; and the run-to-run stability check of two tables.

use crate::BenchError;
use crate::stats::{MIN_RUNS, Timing};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Version of the JSON layout.
pub const TABLE_SCHEMA_VERSION: u32 = 1;

/// The machine and build a table was measured on (CI-008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Host {
    /// Runner class (CI-008's "fixed runner class"; here the local workstation, see WP18 notes).
    pub runner_class: String,
    /// CPU model string.
    pub cpu_model: String,
    /// Logical CPUs.
    pub logical_cpus: usize,
    /// The SIMD-relevant CPU flags present (`sse4_2`, `avx`, `avx2`, `fma`, `avx512*`).
    pub cpu_flags: Vec<String>,
    /// Whether any AVX-512 flag is present.
    pub avx512: bool,
    /// Kernel release.
    pub kernel: String,
    /// `rustc --version` of the FerroAstro build.
    pub rustc: String,
    /// Cargo profile and flags of the FerroAstro build.
    pub build: String,
    /// Git commit of the FerroAstro tree (with `-dirty` when it had local changes).
    pub git_commit: String,
}

/// One accuracy value, or the reason there is none. `basis` says how the value was obtained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measure {
    /// The value, if measured.
    pub value: Option<f64>,
    /// Unit of `value`.
    pub unit: String,
    /// How it was computed, or why there is no value (never empty).
    pub basis: String,
}

impl Measure {
    /// A measured value.
    #[must_use]
    pub fn of(value: f64, unit: &str, basis: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            unit: unit.to_owned(),
            basis: basis.into(),
        }
    }

    /// No value, with the reason.
    #[must_use]
    pub fn none(unit: &str, reason: impl Into<String>) -> Self {
        Self {
            value: None,
            unit: unit.to_owned(),
            basis: reason.into(),
        }
    }

    fn cell(&self) -> String {
        self.value.map_or_else(|| "–".to_owned(), |v| format!("{v:.3e}"))
    }
}

/// The accuracy columns of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Accuracy {
    /// `|E(T) − E(0)| / |E(0)|`.
    pub relative_energy_change: Measure,
    /// Largest heliocentric position difference of Mercury from the scenario's reference run.
    pub mercury_dr_km: Measure,
    /// Mercury's perihelion rate minus FerroAstro's (″/cy), same estimator (CI-009).
    pub rate_minus_ferroastro: Measure,
    /// Mercury's perihelion rate minus the published reference value (″/cy).
    pub rate_minus_reference: Measure,
}

/// Outcome of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Timed runs completed.
    Timed,
    /// The runner refused the scenario (exit 3, REF-011).
    Refused,
    /// A run failed for another reason.
    Failed,
}

/// One tool on one scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Scenario file name (without `.toml`).
    pub scenario: String,
    /// Tool name.
    pub tool: String,
    /// Tool version (empty when refused).
    pub version: String,
    /// Integrator and settings actually used.
    pub settings: String,
    /// Outcome.
    pub status: Status,
    /// The refusal or failure message (empty when timed).
    pub message: String,
    /// The timed runs (`None` unless timed).
    pub timing: Option<Timing>,
    /// 1-min load average before/after each run, when recorded.
    pub load_avg: Vec<[f64; 2]>,
    /// What the wall clock covers for this tool.
    pub timer_scope: String,
    /// Mercury's perihelion rate (″/cy) from the daily twin, if measured.
    pub rate_arcsec_cy: Option<f64>,
    /// Accuracy columns.
    pub accuracy: Accuracy,
}

/// A scenario of the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioInfo {
    /// Scenario file name (without `.toml`).
    pub name: String,
    /// Short label (`(a)`, `(b)`, …) and description.
    pub label: String,
    /// The daily twin used for the rate columns, if any.
    pub daily_twin: Option<String>,
    /// SHA-256 of the scenario file.
    pub sha256: String,
}

/// A speed table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedTable {
    /// [`TABLE_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Identifier of the harness invocation (one "workflow run", CI-008).
    pub run_id: String,
    /// UTC time the table was assembled.
    pub generated_utc: String,
    /// Machine and build.
    pub host: Host,
    /// 1-min load average at the start and the end of the harness invocation.
    pub load_avg_run: [f64; 2],
    /// Scenarios in table order.
    pub scenarios: Vec<ScenarioInfo>,
    /// Entries, grouped by scenario.
    pub entries: Vec<Entry>,
    /// Free-text notes (provenance of reference values, caveats).
    pub notes: Vec<String>,
}

fn table_err(what: &str, problem: impl Into<String>) -> BenchError {
    BenchError::Table {
        what: what.to_owned(),
        problem: problem.into(),
    }
}

impl SpeedTable {
    /// The entry of `tool` on `scenario`.
    #[must_use]
    pub fn entry(&self, scenario: &str, tool: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.scenario == scenario && e.tool == tool)
    }

    /// CI-008 and WP18 structure: every timed entry has ≥ [`MIN_RUNS`] runs and a version; every
    /// refused or failed entry has a message; every accuracy column without a value says why;
    /// every entry's scenario is listed.
    ///
    /// # Errors
    /// `FA-E1601` naming the first violation.
    pub fn validate(&self) -> Result<(), BenchError> {
        if self.schema_version != TABLE_SCHEMA_VERSION {
            return Err(table_err(&self.run_id, "schema_version"));
        }
        if self.host.cpu_model.is_empty() || self.host.runner_class.is_empty() {
            return Err(table_err(&self.run_id, "host CPU model or runner class missing"));
        }
        for e in &self.entries {
            let what = format!("{} / {}", e.scenario, e.tool);
            if !self.scenarios.iter().any(|s| s.name == e.scenario) {
                return Err(table_err(&what, "scenario not listed"));
            }
            match e.status {
                Status::Timed => {
                    let t = e
                        .timing
                        .as_ref()
                        .ok_or_else(|| table_err(&what, "timed without timing"))?;
                    if !t.meets_minimum() {
                        return Err(table_err(
                            &what,
                            format!("{} runs, CI-008 needs >= {MIN_RUNS}", t.runs_s.len()),
                        ));
                    }
                    if e.version.is_empty() || e.timer_scope.is_empty() {
                        return Err(table_err(&what, "version or timer scope missing"));
                    }
                }
                Status::Refused | Status::Failed => {
                    if e.message.is_empty() {
                        return Err(table_err(&what, "refusal or failure without a message"));
                    }
                }
            }
            for m in [
                &e.accuracy.relative_energy_change,
                &e.accuracy.mercury_dr_km,
                &e.accuracy.rate_minus_ferroastro,
                &e.accuracy.rate_minus_reference,
            ] {
                if m.basis.is_empty() {
                    return Err(table_err(&what, "an accuracy column has no basis"));
                }
            }
        }
        Ok(())
    }

    /// The markdown rendering: host block, then one table per scenario with the accuracy bases
    /// as notes under it.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        let h = &self.host;
        let _ = writeln!(s, "# FerroAstro speed table — run `{}`\n", self.run_id);
        let _ = writeln!(
            s,
            "Generated {} UTC. Every entry is the median of >= {MIN_RUNS} runs within this one harness invocation on one machine (CI-008); compare entries only within this table.\n",
            self.generated_utc
        );
        let _ = writeln!(s, "| host | value |\n|---|---|");
        let _ = writeln!(s, "| runner class | {} |", h.runner_class);
        let _ = writeln!(s, "| CPU | {} ({} logical CPUs) |", h.cpu_model, h.logical_cpus);
        let _ = writeln!(
            s,
            "| CPU flags | {} (AVX-512: {}) |",
            h.cpu_flags.join(" "),
            if h.avx512 { "yes" } else { "no" }
        );
        let _ = writeln!(s, "| kernel | {} |", h.kernel);
        let _ = writeln!(s, "| FerroAstro build | {}; {} |", h.rustc, h.build);
        let _ = writeln!(s, "| FerroAstro commit | `{}` |", h.git_commit);
        let _ = writeln!(
            s,
            "| 1-min load average (start → end) | {:.1} → {:.1} |\n",
            self.load_avg_run[0], self.load_avg_run[1]
        );
        for sc in &self.scenarios {
            let _ = writeln!(s, "## {} `{}`\n", sc.label, sc.name);
            let _ = writeln!(
                s,
                "Scenario sha256 `{}`{}.\n",
                sc.sha256,
                sc.daily_twin
                    .as_ref()
                    .map_or_else(String::new, |d| format!("; rate columns from the daily twin `{d}`"))
            );
            let _ = writeln!(
                s,
                "| tool | version | settings | status | runs | median (s) | range (s) | load avg | rel. energy change | Mercury \\|Δr\\| (km) | rate − FerroAstro (″/cy) | rate − reference (″/cy) |"
            );
            let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|");
            let mut notes = Vec::new();
            for e in self.entries.iter().filter(|e| e.scenario == sc.name) {
                let (runs, med, range) = e.timing.as_ref().map_or_else(
                    || ("–".to_owned(), "–".to_owned(), "–".to_owned()),
                    |t| {
                        (
                            t.runs_s.len().to_string(),
                            format!("{:.3}", t.median_s),
                            format!("{:.3}", t.range_s()),
                        )
                    },
                );
                let load = if e.load_avg.is_empty() {
                    "–".to_owned()
                } else {
                    let lo = e.load_avg.iter().flatten().copied().fold(f64::INFINITY, f64::min);
                    let hi = e.load_avg.iter().flatten().copied().fold(0.0, f64::max);
                    format!("{lo:.0}–{hi:.0}")
                };
                let status = match e.status {
                    Status::Timed => "timed".to_owned(),
                    Status::Refused => "refused (REF-011)".to_owned(),
                    Status::Failed => "FAILED".to_owned(),
                };
                let a = &e.accuracy;
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {status} | {runs} | {med} | {range} | {load} | {} | {} | {} | {} |",
                    e.tool,
                    if e.version.is_empty() { "–" } else { &e.version },
                    e.settings.replace('|', "/"),
                    a.relative_energy_change.cell(),
                    a.mercury_dr_km.cell(),
                    a.rate_minus_ferroastro.cell(),
                    a.rate_minus_reference.cell(),
                );
                if !e.message.is_empty() {
                    notes.push(format!("**{}**: {}", e.tool, e.message.replace('\n', " ")));
                }
                notes.push(format!("**{}** timer: {}", e.tool, e.timer_scope));
                if let Some(r) = e.rate_arcsec_cy {
                    notes.push(format!("**{}** perihelion rate {r:.6} ″/cy", e.tool));
                }
                for (col, m) in [
                    ("energy", &a.relative_energy_change),
                    ("Mercury Δr", &a.mercury_dr_km),
                    ("rate − FerroAstro", &a.rate_minus_ferroastro),
                    ("rate − reference", &a.rate_minus_reference),
                ] {
                    notes.push(format!("**{}** {col}: {}", e.tool, m.basis));
                }
            }
            let _ = writeln!(s);
            for n in notes {
                let _ = writeln!(s, "- {n}");
            }
            let _ = writeln!(s);
        }
        if !self.notes.is_empty() {
            let _ = writeln!(s, "## Notes\n");
            for n in &self.notes {
                let _ = writeln!(s, "- {n}");
            }
        }
        s
    }
}

/// One row of the run-to-run stability check of two tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stability {
    /// Scenario.
    pub scenario: String,
    /// Tool.
    pub tool: String,
    /// Median of the earlier table (s).
    pub median_prev_s: f64,
    /// Median of the later table (s).
    pub median_next_s: f64,
    /// Run-to-run spread (range) recorded in the earlier table (s).
    pub range_prev_s: f64,
    /// Run-to-run spread recorded in the later table (s).
    pub range_next_s: f64,
    /// `|median_next − median_prev| < range_prev` (WP18 acceptance, pre-registered).
    pub pass: bool,
    /// `|median_next − median_prev| < range_next` (reported).
    pub pass_reverse: bool,
}

impl Stability {
    /// `|median_next − median_prev|` (s).
    #[must_use]
    pub fn delta_s(&self) -> f64 {
        (self.median_next_s - self.median_prev_s).abs()
    }
}

/// The WP18 acceptance check: every timed entry of `tool` in `prev` must be timed in `next`, and
/// its medians must differ by less than the run-to-run spread recorded in `prev`.
///
/// # Errors
/// `FA-E1601` if the tables are from the same run, from different runner classes or CPUs
/// (CI-008: no comparison across them), or an entry of `prev` is not timed in `next`.
pub fn stability(
    prev: &SpeedTable,
    next: &SpeedTable,
    tool: &str,
) -> Result<Vec<Stability>, BenchError> {
    if prev.run_id == next.run_id {
        return Err(table_err(&next.run_id, "both tables come from the same run"));
    }
    if prev.host.runner_class != next.host.runner_class || prev.host.cpu_model != next.host.cpu_model
    {
        return Err(table_err(
            &next.run_id,
            "different runner class or CPU: CI-008 forbids comparing them",
        ));
    }
    let mut rows = Vec::new();
    for e in prev.entries.iter().filter(|e| e.tool == tool) {
        let Some(tp) = e.timing.as_ref() else {
            continue;
        };
        let tn = next
            .entry(&e.scenario, tool)
            .and_then(|x| x.timing.as_ref())
            .ok_or_else(|| table_err(&next.run_id, format!("{} / {tool} not timed", e.scenario)))?;
        let d = (tn.median_s - tp.median_s).abs();
        rows.push(Stability {
            scenario: e.scenario.clone(),
            tool: tool.to_owned(),
            median_prev_s: tp.median_s,
            median_next_s: tn.median_s,
            range_prev_s: tp.range_s(),
            range_next_s: tn.range_s(),
            pass: d < tp.range_s(),
            pass_reverse: d < tn.range_s(),
        });
    }
    if rows.is_empty() {
        return Err(table_err(&prev.run_id, format!("no timed {tool} entries")));
    }
    Ok(rows)
}
