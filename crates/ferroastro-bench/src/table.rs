// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The speed table (CI-008): one entry per tool and scenario, all from one harness invocation on
//! one machine, with the host's CPU model, flags and runner class; JSON (consumed later by
//! `ferroastro-report`) and markdown; and the run-to-run stability check of two tables.

use crate::BenchError;
use crate::stats::{MIN_RUNS, Timing};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Version of the JSON layout written now. Version 2 (WP28) adds the IC hash and step count of
/// each entry, the round-off and de440.bsp accuracy columns, the CPU pin and single-core reference
/// score of the host, and the published (NUM-024) rows; every addition is optional in the JSON so
/// that version-1 tables still read.
pub const TABLE_SCHEMA_VERSION: u32 = 2;

/// The JSON layout versions [`SpeedTable::validate`] accepts.
pub const TABLE_SCHEMA_VERSIONS: [u32; 2] = [1, 2];

/// A single-core reference benchmark score of the host (NUM-024, OI-44): recorded so that a
/// hardware normalisation can be applied later; none is applied here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceScore {
    /// The command, as run (including any CPU pin).
    pub command: String,
    /// The benchmark's version line.
    pub version: String,
    /// Its total rating at the start and the end of the harness invocation.
    pub ratings: Vec<f64>,
    /// Unit of `ratings`.
    pub unit: String,
}

/// A published timing of another code on other hardware (NUM-024: Subbotin et al. 2023). Listed
/// next to FerroAstro's measured median; comparing them crosses hardware, so the normalisation is
/// open (OI-44) and the comparison is never a gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Published {
    /// What was published (source and configuration).
    pub label: String,
    /// Vector id the value was read from (`tests/vectors`).
    pub vector_id: String,
    /// Published wall time (s).
    pub wall_s: f64,
    /// Integration steps of the published run.
    pub steps: u64,
    /// The FerroAstro scenario of this table that runs the same configuration, if any.
    pub ferroastro_scenario: Option<String>,
    /// Caveats (hardware, model differences).
    pub note: String,
}

impl Published {
    /// Published wall time per step (ms).
    #[must_use]
    pub fn ms_per_step(&self) -> f64 {
        self.wall_s * MS_PER_S / step_count(self.steps)
    }
}

/// Milliseconds per second.
const MS_PER_S: f64 = 1000.0;

#[expect(clippy::cast_precision_loss, reason = "step counts are far below 2^53")]
fn step_count(steps: u64) -> f64 {
    steps as f64
}

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
    /// CPU pin of the timed FerroAstro runs (`taskset` list), if any (schema 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// Single-core reference benchmark score (schema 2; NUM-024, OI-44).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_score: Option<ReferenceScore>,
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
        self.value
            .map_or_else(|| "–".to_owned(), |v| format!("{v:.3e}"))
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
    /// Round-off: max over the samples of Mercury's barycentric `|r − r_DD|` against the
    /// scenario's full double-double twin (schema 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roundoff_vs_dd_km: Option<Measure>,
    /// Mercury's heliocentric |Δr| against de440.bsp at t = 40 Julian years (schema 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mercury_de440_km: Option<Measure>,
}

impl Accuracy {
    /// Every column, with its markdown name (the schema-2 columns only when present).
    #[must_use]
    pub fn columns(&self) -> Vec<(&'static str, &Measure)> {
        let mut v = vec![
            ("energy", &self.relative_energy_change),
            ("Mercury Δr", &self.mercury_dr_km),
            ("rate − FerroAstro", &self.rate_minus_ferroastro),
            ("rate − reference", &self.rate_minus_reference),
        ];
        if let Some(m) = &self.roundoff_vs_dd_km {
            v.push(("round-off vs DD", m));
        }
        if let Some(m) = &self.mercury_de440_km {
            v.push(("Mercury vs de440.bsp at 40 yr", m));
        }
        v
    }
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
    /// The run's initial-condition hash (schema 2; FerroAstro entries), so that a timing can be
    /// rerun from the exact scenario file and inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ic_hash: Option<String>,
    /// Integration steps of one run (schema 2; FerroAstro entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u64>,
}

impl Entry {
    /// Median wall time per step (ms), when both are known.
    #[must_use]
    pub fn ms_per_step(&self) -> Option<f64> {
        let (t, n) = (self.timing.as_ref()?, self.steps?);
        (n > 0).then(|| t.median_s * MS_PER_S / step_count(n))
    }
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
    /// Published timings listed for comparison (schema 2; NUM-024).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<Published>,
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
        if !TABLE_SCHEMA_VERSIONS.contains(&self.schema_version) {
            return Err(table_err(&self.run_id, "schema_version"));
        }
        let v2 = self.schema_version >= 2;
        if self.host.cpu_model.is_empty() || self.host.runner_class.is_empty() {
            return Err(table_err(
                &self.run_id,
                "host CPU model or runner class missing",
            ));
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
                    if v2
                        && e.tool == "ferroastro"
                        && (e.ic_hash.as_deref().is_none_or(str::is_empty) || e.steps.is_none())
                    {
                        return Err(table_err(
                            &what,
                            "schema 2: a timed FerroAstro entry needs its IC hash and step count",
                        ));
                    }
                }
                Status::Refused | Status::Failed => {
                    if e.message.is_empty() {
                        return Err(table_err(&what, "refusal or failure without a message"));
                    }
                }
            }
            for (_, m) in e.accuracy.columns() {
                if m.basis.is_empty() {
                    return Err(table_err(&what, "an accuracy column has no basis"));
                }
            }
        }
        for p in &self.published {
            if p.vector_id.is_empty() || p.note.is_empty() || p.steps == 0 {
                return Err(table_err(
                    &p.label,
                    "a published row needs its vector id, a note and the step count",
                ));
            }
            if !(p.wall_s.is_finite() && p.wall_s > 0.0) {
                return Err(table_err(&p.label, "published wall time"));
            }
            if let Some(sc) = &p.ferroastro_scenario {
                if !self.scenarios.iter().any(|s| &s.name == sc) {
                    return Err(table_err(&p.label, format!("scenario {sc} not listed")));
                }
            }
        }
        Ok(())
    }

    /// The median of FerroAstro's entry on `scenario`, if timed.
    #[must_use]
    pub fn ferroastro_median_s(&self, scenario: &str) -> Option<f64> {
        self.entry(scenario, "ferroastro")
            .and_then(|e| e.timing.as_ref())
            .map(|t| t.median_s)
    }

    /// The markdown rendering: host block, then one table per scenario with the accuracy bases
    /// as notes under it.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one linear rendering: host block, then each scenario's table and notes"
    )]
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
        let _ = writeln!(
            s,
            "| CPU | {} ({} logical CPUs) |",
            h.cpu_model, h.logical_cpus
        );
        let _ = writeln!(
            s,
            "| CPU flags | {} (AVX-512: {}) |",
            h.cpu_flags.join(" "),
            if h.avx512 { "yes" } else { "no" }
        );
        let _ = writeln!(s, "| kernel | {} |", h.kernel);
        let _ = writeln!(s, "| FerroAstro build | {}; {} |", h.rustc, h.build);
        let _ = writeln!(s, "| FerroAstro commit | `{}` |", h.git_commit);
        if let Some(pin) = &h.pin {
            let _ = writeln!(
                s,
                "| CPU pin of the timed FerroAstro runs | `taskset -c {pin}` |"
            );
        }
        if let Some(r) = &h.reference_score {
            let ratings: Vec<String> = r.ratings.iter().map(|x| format!("{x:.0}")).collect();
            let _ = writeln!(
                s,
                "| single-core reference score (start, end) | {} {} (`{}`; {}) |",
                ratings.join(", "),
                r.unit,
                r.command,
                r.version
            );
        }
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
                sc.daily_twin.as_ref().map_or_else(String::new, |d| format!(
                    "; rate columns from the daily twin `{d}`"
                ))
            );
            let v2_cols = self.entries.iter().any(|e| {
                e.scenario == sc.name
                    && (e.accuracy.roundoff_vs_dd_km.is_some()
                        || e.accuracy.mercury_de440_km.is_some()
                        || e.ic_hash.is_some())
            });
            let _ = writeln!(
                s,
                "| tool | version | settings | status | runs | median (s) | range (s) | load avg | rel. energy change | Mercury \\|Δr\\| (km) | rate − FerroAstro (″/cy) | rate − reference (″/cy) |{}",
                if v2_cols {
                    " steps | ms/step | round-off vs DD (km) | Mercury vs de440.bsp at 40 yr (km) | IC hash |"
                } else {
                    ""
                }
            );
            let _ = writeln!(
                s,
                "|---|---|---|---|---|---|---|---|---|---|---|---|{}",
                if v2_cols { "---|---|---|---|---|" } else { "" }
            );
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
                    let lo = e
                        .load_avg
                        .iter()
                        .flatten()
                        .copied()
                        .fold(f64::INFINITY, f64::min);
                    let hi = e.load_avg.iter().flatten().copied().fold(0.0, f64::max);
                    format!("{lo:.0}–{hi:.0}")
                };
                let status = match e.status {
                    Status::Timed => "timed".to_owned(),
                    Status::Refused => "refused (REF-011)".to_owned(),
                    Status::Failed => "FAILED".to_owned(),
                };
                let a = &e.accuracy;
                let extra = if v2_cols {
                    let opt = |m: Option<&Measure>| m.map_or_else(|| "–".to_owned(), Measure::cell);
                    format!(
                        " {} | {} | {} | {} | {} |",
                        e.steps.map_or_else(|| "–".to_owned(), |n| n.to_string()),
                        e.ms_per_step()
                            .map_or_else(|| "–".to_owned(), |x| format!("{x:.4}")),
                        opt(a.roundoff_vs_dd_km.as_ref()),
                        opt(a.mercury_de440_km.as_ref()),
                        e.ic_hash.as_ref().map_or_else(
                            || "–".to_owned(),
                            |h| format!("`{}…`", h.get(..12).unwrap_or(h))
                        ),
                    )
                } else {
                    String::new()
                };
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {status} | {runs} | {med} | {range} | {load} | {} | {} | {} | {} |{extra}",
                    e.tool,
                    if e.version.is_empty() {
                        "–"
                    } else {
                        &e.version
                    },
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
                if let Some(h) = &e.ic_hash {
                    notes.push(format!("**{}** IC hash `{h}`", e.tool));
                }
                for (col, m) in a.columns() {
                    notes.push(format!("**{}** {col}: {}", e.tool, m.basis));
                }
            }
            let _ = writeln!(s);
            for n in notes {
                let _ = writeln!(s, "- {n}");
            }
            let _ = writeln!(s);
        }
        self.published_markdown(&mut s);
        if !self.notes.is_empty() {
            let _ = writeln!(s, "## Notes\n");
            for n in &self.notes {
                let _ = writeln!(s, "- {n}");
            }
        }
        s
    }
}

impl SpeedTable {
    /// The NUM-024 section: each published row next to FerroAstro's measured median, and the
    /// cost ratios of the published rows that share a FerroAstro scenario.
    fn published_markdown(&self, s: &mut String) {
        if self.published.is_empty() {
            return;
        }
        let _ = writeln!(
            s,
            "## Published timings (NUM-024): different machine, hardware normalisation UNVERIFIED (OI-44)\n"
        );
        let _ = writeln!(
            s,
            "Every ratio below crosses hardware: it is context, never a gate (CI-008 compares only within this table). NUM-024's \"within 1×\" target is reported, not gated.\n"
        );
        let _ = writeln!(
            s,
            "| published | wall (s) | steps | ms/step | FerroAstro scenario | FerroAstro median (s) | range (s) | ms/step | FerroAstro / published |\n|---|---|---|---|---|---|---|---|---|"
        );
        for p in &self.published {
            let fa = p
                .ferroastro_scenario
                .as_deref()
                .and_then(|sc| self.entry(sc, "ferroastro"));
            let t = fa.and_then(|e| e.timing.as_ref());
            let _ = writeln!(
                s,
                "| {} (`{}`) | {:.0} | {} | {:.4} | {} | {} | {} | {} | {} |",
                p.label,
                p.vector_id,
                p.wall_s,
                p.steps,
                p.ms_per_step(),
                p.ferroastro_scenario
                    .as_deref()
                    .map_or_else(|| "–".to_owned(), |x| format!("`{x}`")),
                t.map_or_else(|| "–".to_owned(), |t| format!("{:.3}", t.median_s)),
                t.map_or_else(|| "–".to_owned(), |t| format!("{:.3}", t.range_s())),
                fa.and_then(Entry::ms_per_step)
                    .map_or_else(|| "–".to_owned(), |x| format!("{x:.4}")),
                t.map_or_else(
                    || "–".to_owned(),
                    |t| format!("{:.3}", t.median_s / p.wall_s)
                ),
            );
        }
        let _ = writeln!(s);
        for r in self.cost_ratios() {
            let _ = writeln!(s, "- {r}");
        }
        for p in &self.published {
            let _ = writeln!(s, "- **{}**: {}", p.label, p.note);
        }
        let _ = writeln!(s);
    }

    /// Each pair of published rows (b over a, in list order) with the matching ratio of
    /// FerroAstro's medians: e.g. mixed / f64 against the published mixed / f64.
    #[must_use]
    pub fn cost_ratios(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, a) in self.published.iter().enumerate() {
            for b in self.published.iter().skip(i + 1) {
                let fa = |p: &Published| {
                    p.ferroastro_scenario
                        .as_deref()
                        .and_then(|sc| self.ferroastro_median_s(sc))
                };
                let ours = match (fa(b), fa(a)) {
                    (Some(x), Some(y)) => format!("{:.3}", x / y),
                    _ => "–".to_owned(),
                };
                out.push(format!(
                    "cost ratio {} / {}: FerroAstro {ours} (same machine, same invocation), published {:.3}",
                    b.label,
                    a.label,
                    b.wall_s / a.wall_s
                ));
            }
        }
        out
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
        return Err(table_err(
            &next.run_id,
            "both tables come from the same run",
        ));
    }
    if prev.host.runner_class != next.host.runner_class
        || prev.host.cpu_model != next.host.cpu_model
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
