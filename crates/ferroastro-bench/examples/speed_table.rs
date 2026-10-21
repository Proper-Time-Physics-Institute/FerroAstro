// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Speed-table driver (WP18; CI-008, DEP-014, NUM-009). Called by `tools/bench/speed_table.sh`.
//!
//! ```text
//! speed_table ferroastro --scenario <file.toml> --runs <n> --run-dir <dir>
//! speed_table assemble   --run-dir <dir> --out-json <file> --out-md <file>
//! speed_table compare    <earlier.json> <later.json>
//! ```
//!
//! * `ferroastro`: runs the scenario `n` times in this process (release build), timing
//!   `ferroastro_propagate::run_with_energy` (IC assembly, model build, integration; the JPL files
//!   are parsed before the timer), and writes `<dir>/ferroastro/<scenario>/run-<k>/` with
//!   `ferroastro.json` and the states in the oracle CSV format.
//! * `assemble`: reads every `<dir>/<tool>/<scenario>/` the harness and this driver wrote, computes
//!   the accuracy columns identically for every tool, and writes the JSON + markdown table.
//!   Reference values (Park17 Table 3 rows, `PARK17-GE-RATE`, SW92 mean motions for the
//!   estimator) come from `tests/vectors` by id; that is why this driver is an example (it may use
//!   the test-support crates as dev-dependencies; the library may not).
//! * `compare`: the WP18 acceptance check of two consecutive tables (FerroAstro medians differ by
//!   less than the run-to-run spread recorded in the earlier table). Exit status 1 on failure.

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "a command-line driver: a failure aborts the run with its message"
)]

use ferroastro_bench::accuracy::{Centre, body_vs_spk, max_dr};
use ferroastro_bench::accuracy::{mercury_max_dr, mercury_track, relative_energy_change};
use ferroastro_bench::series::{FERROASTRO_RECORD, RunInfo, Series};
use ferroastro_bench::{
    Accuracy, Entry, Host, Measure, Published, ReferenceScore, ScenarioInfo, SpeedTable, Status,
    Timing, read_run, stability,
};
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_diffrun::{Estimator, PassageSource};
use ferroastro_ephem::Spk;
use ferroastro_precession::park2017::{
    FitModel, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane,
    park17_table2_frequencies,
};
use ferroastro_propagate::{DataSources, initial_conditions, run_with_energy};
use ferroastro_scenario::{BodyName, Mode, ScenarioSpec};
use ferroastro_system::SmallBodyStates;
use ferroastro_vectors::VectorSet;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

const TOOLS: [&str; 5] = ["ferroastro", "rebound", "swiftest", "mercury6", "orekit"];
const MARGIN_DAYS: f64 = 30.0; // fit-window margin inside the span, as WP14/WP17
const JACKKNIFE_BLOCKS: usize = 20; // as WP14/WP17
const SMALL_BODIES: &str = "tests/goldens/sb441-n373s-jdepoc-states.csv";

/// What the accuracy columns of a scenario use.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Sun + 8 planets: energy from states, Mercury vs REBOUND, rate vs the sum of the `Park17` rows.
    PlanetsNewton,
    /// Sun + Mercury 1PN test particle: Mercury vs FerroAstro, rate vs PARK17-GE-RATE.
    MercuryGr,
    /// Full system with small bodies: FerroAstro model energy only.
    FullSmallBodies,
    /// Mercury in the DE440 field: Mercury vs FerroAstro.
    EphemerisField,
    /// Sun + 8 planets under EIH 1PN (WP21): Mercury vs REBOUND(x `gr_full`); no conserved
    /// Newtonian energy.
    PlanetsEih,
    /// WP28 (g): Sun + 8 planets under EIH with ABM13: no oracle runs ABM, so no Δr reference.
    PlanetsEihAbm,
    /// WP28 (h)–(j): the NUM-024 / DE440-class ephemeris runs (FerroAstro only).
    Ephemeris,
}

struct Planned {
    name: &'static str,
    label: &'static str,
    daily: Option<&'static str>,
    kind: Kind,
    /// WP28: the full double-double twin (untimed) behind the round-off column.
    truth: Option<&'static str>,
    /// WP28: whether the table carries Mercury vs de440.bsp at 40 yr.
    de440: bool,
}

/// WP28 (NUM-024): the f64 and mixed scenarios that the published Subbotin rows are matched with.
const NUM024_F64: &str = "bench-h-num024-epm16-f64-40yr";
const NUM024_MIXED: &str = "ephemeris-mixed-epm16-40yr";
/// The epoch offset of the de440.bsp column (days): 40 Julian years.
const DE440_COLUMN_YEARS: f64 = 40.0;

const PLAN: [Planned; 10] = [
    Planned {
        name: "bench-a-sun-8planets-newton-1000yr",
        label: "(a) Sun + 8 planets, Newtonian, 1000 yr",
        daily: Some("bench-a-sun-8planets-newton-1000yr-daily"),
        kind: Kind::PlanetsNewton,
        truth: None,
        de440: false,
    },
    Planned {
        name: "bench-b-sun-mercury-tp-gr1pn-1000yr",
        label: "(b) Sun + Mercury with GR (1PN test particle), 1000 yr",
        daily: Some("bench-b-sun-mercury-tp-gr1pn-1000yr-daily"),
        kind: Kind::MercuryGr,
        truth: None,
        de440: false,
    },
    Planned {
        name: "bench-c-full-newton-minor343-100yr",
        label: "(c) full Newtonian + 343 Minor asteroids + KBOs + ring, 100 yr",
        daily: None,
        kind: Kind::FullSmallBodies,
        truth: None,
        de440: false,
    },
    Planned {
        name: "oracle-stage-b-mercury-de440-field-100yr",
        label: "(d) Mercury in the DE440-driven field (stage B), 100 yr",
        daily: None,
        kind: Kind::EphemerisField,
        truth: None,
        de440: false,
    },
    Planned {
        name: "bench-e-sun-8planets-eih-1000yr",
        label: "(e) Sun + 8 planets, EIH 1PN (gr_eih; REBOUNDx gr_full), 1000 yr",
        daily: None,
        kind: Kind::PlanetsEih,
        truth: None,
        de440: false,
    },
    Planned {
        name: "bench-f-sun-8planets-eih-100yr",
        label: "(f) Sun + 8 planets, EIH 1PN, IAS15 f64 (REBOUNDx gr_full), 100 yr",
        daily: None,
        kind: Kind::PlanetsEih,
        truth: Some("bench-f-sun-8planets-eih-100yr-dd"),
        de440: true,
    },
    Planned {
        name: "bench-g-sun-8planets-eih-100yr-abm",
        label: "(g) Sun + 8 planets, EIH 1PN, ABM13 PECEC h = 1/16 d f64, 100 yr",
        daily: None,
        kind: Kind::PlanetsEihAbm,
        truth: Some("bench-g-sun-8planets-eih-100yr-abm-dd"),
        de440: true,
    },
    Planned {
        name: NUM024_F64,
        label: "(h) NUM-024 f64: 16 EIH Majors + 338 Minor, ABM13 h = 1/16 d, plain sums, 40 yr",
        daily: None,
        kind: Kind::Ephemeris,
        truth: Some("bench-h-num024-epm16-dd-40yr"),
        de440: true,
    },
    Planned {
        name: NUM024_MIXED,
        label: "(i) NUM-024 mixed DD: 16 EIH Majors + 338 Minor, ABM13 h = 1/16 d, 40 yr",
        daily: None,
        kind: Kind::Ephemeris,
        truth: Some("bench-h-num024-epm16-dd-40yr"),
        de440: true,
    },
    Planned {
        name: "bench-j-de440class-mixed-40yr",
        label: "(j) DE440-class model (EIH, J2, LT, 338 asteroids + 30 KBOs, ring), mixed DD, 40 yr",
        daily: None,
        kind: Kind::Ephemeris,
        truth: Some("bench-j-de440class-dd-40yr"),
        de440: true,
    },
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scenario_path(name: &str) -> PathBuf {
    root().join("ci/scenarios").join(format!("{name}.toml"))
}

fn load_avg() -> f64 {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse().ok()))
        .unwrap_or(f64::NAN)
}

struct Data {
    header: Header,
    header_prov: String,
    small: SmallBodyStates,
    spk_bytes: Option<(Vec<u8>, String)>,
}

fn load_data(need_spk: bool) -> Data {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let header_prov = f.provenance();
    let header = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let small =
        SmallBodyStates::parse(&std::fs::read_to_string(root().join(SMALL_BODIES)).unwrap())
            .unwrap();
    let spk_bytes = need_spk.then(|| {
        let b = ferroastro_testdata::locate_product("de440-bsp").expect("cached de440.bsp");
        (std::fs::read(&b.path).unwrap(), b.provenance())
    });
    Data {
        header,
        header_prov,
        small,
        spk_bytes,
    }
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

fn need(args: &[String], key: &str) -> String {
    arg(args, key).unwrap_or_else(|| panic!("missing {key}"))
}

/// `ferroastro`: n timed runs of one scenario.
fn cmd_ferroastro(args: &[String]) {
    let path = PathBuf::from(need(args, "--scenario"));
    let runs: usize = need(args, "--runs").parse().unwrap();
    let run_dir = PathBuf::from(need(args, "--run-dir"));
    let text = std::fs::read_to_string(&path).unwrap();
    let spec = ScenarioSpec::parse(&text).unwrap();
    let data = load_data(spec.mode == Mode::EphemerisField);
    let spk = data
        .spk_bytes
        .as_ref()
        .map(|(b, p)| (Spk::parse(b).unwrap(), p.clone()));
    let mut sources = DataSources::none()
        .with_header(&data.header, &data.header_prov)
        .with_small_bodies(&data.small);
    if let Some((s, p)) = &spk {
        sources = sources.with_spk(s, p);
    }
    let commit = std::env::var("FA_BENCH_GIT_COMMIT").unwrap_or_default();
    for k in 1..=runs {
        let before = load_avg();
        let t0 = Instant::now();
        let (out, energy) = run_with_energy(&spec, &sources).unwrap();
        let wall = t0.elapsed().as_secs_f64();
        let after = load_avg();
        let dir = run_dir
            .join("ferroastro")
            .join(&spec.name)
            .join(format!("run-{k}"));
        std::fs::create_dir_all(&dir).unwrap();
        let file = format!("{}.ferroastro.states.csv", spec.name);
        // Only the first run's states are read by `assemble` (WP28: a 354-body 40-yr series is
        // ~40 MB); the later runs are bitwise repeats and keep their timing record only.
        let series = Series::from_run(&file, &out);
        if k == 1 {
            std::fs::write(
                dir.join(&file),
                series.to_csv(&[
                    format!(
                        "FerroAstro states of scenario {} (WP18 speed table)",
                        spec.name
                    ),
                    format!("engine: ferroastro {commit}"),
                    format!("ic_hash: {}", out.ic_hash),
                ]),
            )
            .unwrap();
        }
        let mut extra = BTreeMap::new();
        extra.insert("model".to_owned(), out.model.clone());
        extra.insert("steps".to_owned(), out.stats.steps.to_string());
        extra.insert("ic_hash".to_owned(), out.ic_hash.clone());
        extra.insert("ic_source".to_owned(), out.ic_source.clone());
        if let Some(e) = energy {
            extra.insert(
                "model_energy_over_g_initial".to_owned(),
                format!("{:e}", e.initial),
            );
            extra.insert(
                "model_energy_over_g_last".to_owned(),
                format!("{:e}", e.last),
            );
            extra.insert(
                "model_relative_energy_change".to_owned(),
                format!("{:e}", e.relative_change()),
            );
        }
        let info = RunInfo {
            tool: "ferroastro".into(),
            version: format!("ferroastro {} ({commit})", env!("CARGO_PKG_VERSION")),
            wall_clock_s: wall,
            file,
            extra,
            load_avg: Some([before, after]),
        };
        std::fs::write(
            dir.join(FERROASTRO_RECORD),
            serde_json::to_string_pretty(&info).unwrap(),
        )
        .unwrap();
        println!(
            "ferroastro {} run {k}/{runs}: {wall:.3} s, {} steps, load {before:.1} -> {after:.1}{}",
            spec.name,
            out.stats.steps,
            energy.map_or_else(String::new, |e| format!(
                ", |dE/E| {:.3e}",
                e.relative_change()
            ))
        );
    }
}

fn cpu_host(run: &serde_json::Value) -> Host {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let field = |k: &str| {
        cpuinfo
            .lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_owned())
            .unwrap_or_default()
    };
    let flags: Vec<String> = field("flags")
        .split_whitespace()
        .filter(|f| matches!(*f, "sse4_2" | "avx" | "avx2" | "fma") || f.starts_with("avx512"))
        .map(str::to_owned)
        .collect();
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let s = |k: &str| run.get(k).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    Host {
        runner_class: s("runner_class"),
        cpu_model: field("model name"),
        logical_cpus: std::thread::available_parallelism().map_or(0, std::num::NonZero::get),
        avx512: flags.iter().any(|f| f.starts_with("avx512")),
        cpu_flags: flags,
        kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .trim()
            .to_owned(),
        rustc,
        build: format!(
            "cargo --release (workspace release profile), RUSTFLAGS = {:?}",
            std::env::var("RUSTFLAGS").unwrap_or_default()
        ),
        git_commit: s("git_commit"),
        pin: run
            .get("pin")
            .and_then(|v| v.as_str())
            .filter(|p| !p.is_empty())
            .map(str::to_owned),
        reference_score: run.get("reference_score").and_then(|r| {
            let ratings: Vec<f64> = r
                .get("ratings")?
                .as_array()?
                .iter()
                .filter_map(serde_json::Value::as_f64)
                .collect();
            let command = r.get("command")?.as_str()?.to_owned();
            let version = r.get("version")?.as_str()?.to_owned();
            (!ratings.is_empty()).then(|| ReferenceScore {
                command,
                version,
                ratings,
                unit: "MIPS (7-Zip LZMA total rating)".to_owned(),
            })
        }),
    }
}

/// The run directories of one tool on one scenario, in run order.
fn run_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<(u32, PathBuf)> = std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.strip_prefix("run-")
                        .and_then(|k| k.parse().ok())
                        .map(|k| (k, e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|x| x.1).collect()
}

fn timer_scope(tool: &str) -> &'static str {
    match tool {
        "ferroastro" => {
            "run_with_energy in-process (IC assembly, model build, IAS15 integration, sampling); header.440/SPK parsed before the timer; release build; 5 runs in one process"
        }
        "rebound" => {
            "runner's wall clock around sim set-up + integrate (Python API), inside the container"
        }
        "swiftest" => {
            "runner's wall clock around Swiftest set-up, run and output read-back (fresh interpreter per attempt), inside the container"
        }
        "mercury6" => {
            "runner's wall clock around writing inputs, mercury6 + element6 and reading outputs, inside the container"
        }
        "orekit" => {
            "runner's wall clock around the JVM run of StageB.java (JVM start-up and ephemeris loading included), inside the container"
        }
        _ => "unknown",
    }
}

fn settings_of(info: &RunInfo) -> String {
    let e = &info.extra;
    if info.tool == "ferroastro" {
        return e
            .get("model")
            .and_then(|m| m.split("; IAS15 ").nth(1))
            .map_or_else(|| "IAS15".to_owned(), |s| format!("IAS15 {s}"));
    }
    if info.tool == "rebound" {
        return format!(
            "IAS15 epsilon {}, adaptive_mode {}",
            e.get("ias15_epsilon").map_or("?", String::as_str),
            e.get("ias15_adaptive_mode").map_or("?", String::as_str)
        );
    }
    e.get("integrator_mapping")
        .and_then(|m| m.split("tool settings ").nth(1))
        .map_or_else(String::new, |s| {
            s.split(';').next().unwrap_or(s).trim().to_owned()
        })
}

struct Collected {
    entry: Entry,
    first: Option<(PathBuf, RunInfo)>,
}

fn collect(run_dir: &Path, scenario: &str, tool: &str) -> Collected {
    let dir = run_dir.join(tool).join(scenario);
    let blank = |status: Status, message: String| Entry {
        scenario: scenario.into(),
        tool: tool.into(),
        version: String::new(),
        settings: String::new(),
        status,
        message,
        timing: None,
        load_avg: Vec::new(),
        timer_scope: timer_scope(tool).into(),
        rate_arcsec_cy: None,
        accuracy: Accuracy {
            relative_energy_change: Measure::none("", "no timed run"),
            mercury_dr_km: Measure::none("km", "no timed run"),
            rate_minus_ferroastro: Measure::none("arcsec/cy", "no timed run"),
            rate_minus_reference: Measure::none("arcsec/cy", "no timed run"),
            roundoff_vs_dd_km: None,
            mercury_de440_km: None,
        },
        ic_hash: None,
        steps: None,
    };
    for (file, status) in [
        ("refused.txt", Status::Refused),
        ("failed.txt", Status::Failed),
    ] {
        if let Ok(m) = std::fs::read_to_string(dir.join(file)) {
            return Collected {
                entry: blank(status, m.trim().to_owned()),
                first: None,
            };
        }
    }
    let dirs = run_dirs(&dir);
    let infos: Vec<RunInfo> = dirs.iter().map(|d| read_run(d).unwrap()).collect();
    let Some(first) = infos.first().cloned() else {
        return Collected {
            entry: blank(Status::Failed, format!("no runs under {}", dir.display())),
            first: None,
        };
    };
    let mut e = blank(Status::Timed, String::new());
    e.version.clone_from(&first.version);
    e.settings = settings_of(&first);
    e.timing = Timing::new(infos.iter().map(|i| i.wall_clock_s).collect());
    e.load_avg = infos.iter().filter_map(|i| i.load_avg).collect();
    if tool == "ferroastro" {
        e.ic_hash = first.extra.get("ic_hash").cloned();
        e.steps = first.extra.get("steps").and_then(|x| x.parse().ok());
    }
    Collected {
        entry: e,
        first: dirs.first().cloned().map(|d| (d, first)),
    }
}

fn series_of(c: &Collected, scenario: &str, tool: &str) -> Option<Series> {
    c.first
        .as_ref()
        .map(|(d, _)| Series::read(d, scenario, tool).unwrap())
}

fn daily_series(run_dir: &Path, daily: &str, tool: &str) -> Option<Series> {
    let d = run_dirs(&run_dir.join(tool).join(daily))
        .into_iter()
        .next()?;
    Series::read(&d, daily, tool).ok()
}

fn estimator(vs: &VectorSet, window_jd: (f64, f64)) -> Estimator {
    let n = |b: &str| {
        vs.get(&format!("SW92-T2A-{b}-L-RATE"))
            .unwrap()
            .value()
            .unwrap()
    };
    Estimator {
        park: Park2017 {
            plane: ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap(),
            definition: LongitudeDefinition::Projected,
            passages: PassageConfig::default(),
            model: FitModel {
                quadratic: true,
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
            },
            jackknife_blocks: JACKKNIFE_BLOCKS,
        },
        source: PassageSource::SampledStates,
        body: BodyName::Mercury,
        central: BodyName::Sun,
        window_jd: Some(window_jd),
    }
}

/// The Park17 reference of a scenario: (value, description).
fn park_reference(vs: &VectorSet, kind: Kind) -> Option<(f64, String)> {
    match kind {
        Kind::PlanetsNewton => {
            let mut ids: Vec<String> = vs
                .iter()
                .map(|(_, v)| v.id.clone())
                .filter(|id| {
                    id.starts_with("PARK17-T3-")
                        && !id.ends_with("ASTEROIDS")
                        && !id.contains("KBO")
                        && !id.contains("PLUTO")
                })
                .collect();
            ids.sort();
            let sum: f64 = ids
                .iter()
                .map(|id| vs.get(id).unwrap().value().unwrap())
                .sum();
            Some((
                sum,
                format!(
                    "sum of Park17 Table 3 rows {} = {sum:.4} arcsec/cy; informational: every row is blocked_by OI-38, and this run has EMB as one body, no Pluto and a different window (JDEPOC forward)",
                    ids.join(", ")
                ),
            ))
        }
        Kind::MercuryGr => {
            let v = vs.get("PARK17-GE-RATE").unwrap().value().unwrap();
            Some((
                v,
                format!(
                    "PARK17-GE-RATE = {v} arcsec/cy; informational: Park's value is the N-body gravitoelectric term in the mean-orbit frame, this run a test particle"
                ),
            ))
        }
        Kind::FullSmallBodies
        | Kind::EphemerisField
        | Kind::PlanetsEih
        | Kind::PlanetsEihAbm
        | Kind::Ephemeris => None,
    }
}

fn reference_tool(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::PlanetsNewton | Kind::PlanetsEih => Some("rebound"),
        Kind::MercuryGr | Kind::EphemerisField => Some("ferroastro"),
        Kind::FullSmallBodies | Kind::PlanetsEihAbm | Kind::Ephemeris => None,
    }
}

/// WP28: the round-off column (Mercury's barycentric `|r − r_DD|` against the scenario's
/// double-double twin) and Mercury vs de440.bsp at 40 yr, for one tool's entry.
fn wp28_columns(c: &mut Collected, p: &Planned, tool: &str, run_dir: &Path, spk: Option<&Spk<'_>>) {
    let reason = match c.entry.status {
        Status::Timed => None,
        Status::Refused => Some("refused".to_owned()),
        Status::Failed => Some("failed".to_owned()),
    };
    let series = if reason.is_none() {
        series_of(c, p.name, tool)
    } else {
        None
    };
    if let Some(truth) = p.truth {
        let m = match (&reason, &series) {
            (Some(r), _) => Measure::none("km", r.clone()),
            (None, _) if tool != "ferroastro" => Measure::none(
                "km",
                format!(
                    "the double-double twin `{truth}` is a FerroAstro run; another tool's difference from it is not its round-off"
                ),
            ),
            (None, None) => Measure::none("km", "no output series"),
            (None, Some(s)) => match daily_series(run_dir, truth, "ferroastro") {
                None => Measure::none("km", format!("the truth run `{truth}` is missing")),
                Some(t) => match max_dr(
                    s,
                    &t,
                    ferroastro_bench::series::NAIF_MERCURY,
                    Centre::Barycentre,
                ) {
                    Ok((dr, at, n)) => Measure::of(
                        dr,
                        "km",
                        format!(
                            "Mercury barycentric max |r − r_DD| against the full double-double twin `{truth}` (same integrator and scenario, precision dd), max at t = {at} d over {n} samples incl. the final state{}",
                            if p.kind == Kind::PlanetsEih {
                                "; IAS15 adapts its steps to the DD state, so this includes the step-sequence difference"
                            } else {
                                "; fixed step: round-off only"
                            }
                        ),
                    ),
                    Err(e) => Measure::none("km", format!("not comparable: {e}")),
                },
            },
        };
        c.entry.accuracy.roundoff_vs_dd_km = Some(m);
    }
    if p.de440 {
        let m = match (&reason, &series, spk) {
            (Some(r), _, _) => Measure::none("km", r.clone()),
            (None, None, _) => Measure::none("km", "no output series"),
            (None, _, None) => Measure::none("km", "de440.bsp not loaded"),
            (None, Some(s), Some(spk)) => match body_vs_spk(
                s,
                spk,
                ferroastro_bench::series::NAIF_MERCURY,
                DE440_COLUMN_YEARS * ferroastro_core::time::DAYS_PER_JULIAN_YEAR,
            ) {
                Ok((at40, worst, t)) => Measure::of(
                    at40,
                    "km",
                    format!(
                        "Mercury heliocentric |Δr| against de440.bsp at t = {} d (40 Julian yr); max over 0–40 yr {worst:.4} km at t = {t} d. Model accuracy, not round-off: the model omits what de440 has beyond it (see the scenario)",
                        DE440_COLUMN_YEARS * ferroastro_core::time::DAYS_PER_JULIAN_YEAR
                    ),
                ),
                Err(e) => Measure::none("km", e.to_string()),
            },
        };
        c.entry.accuracy.mercury_de440_km = Some(m);
    }
}

/// The Subbotin et al. 2023 Table 1 wall times (NUM-024), when the NUM-024 rows are in the table.
fn published_rows(vs: &VectorSet, scenarios: &[ScenarioInfo]) -> Vec<Published> {
    let has = |n: &str| scenarios.iter().any(|s| s.name == n);
    if !has(NUM024_F64) && !has(NUM024_MIXED) {
        return Vec::new();
    }
    let spec = ScenarioSpec::parse(&std::fs::read_to_string(scenario_path(NUM024_MIXED)).unwrap())
        .unwrap();
    let step_days = spec.integrator.step_days.unwrap();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive step count far below 2^53, rounded"
    )]
    let steps =
        (spec.span.years * ferroastro_core::time::DAYS_PER_JULIAN_YEAR / step_days).round() as u64;
    let note = "Subbotin, Kodukov & Pavlov 2023 (CeMDA, doi:10.1007/s10569-023-10139-2; arXiv:2304.04458v1) Table 1: EPM2021 model (16 EIH Majors; 277 asteroids, 30 TNOs, annulus and ring), ABM13 PECEC h = 1/16 d, 40 yr, their CPU unspecified (OI-44: normalisation UNVERIFIED). FerroAstro's model has DE440's 343 asteroids (338 Minor), no TNOs or ring in (h)/(i).";
    let row = |id: &str, label: &str, sc: Option<&str>| Published {
        label: label.to_owned(),
        vector_id: id.to_owned(),
        wall_s: vs.get(id).unwrap().value().unwrap(),
        steps,
        ferroastro_scenario: sc.filter(|n| has(n)).map(str::to_owned),
        note: note.to_owned(),
    };
    vec![
        row(
            "SUBBOTIN23-T1-WALL-F64-S",
            "Subbotin 2023 f64",
            Some(NUM024_F64),
        ),
        row(
            "SUBBOTIN23-T1-WALL-MIXED-S",
            "Subbotin 2023 mixed DD",
            Some(NUM024_MIXED),
        ),
        row("SUBBOTIN23-T1-WALL-DD-S", "Subbotin 2023 full DD", None),
    ]
}

/// `assemble`: the speed table of one run directory.
fn cmd_assemble(args: &[String]) -> ExitCode {
    let run_dir = PathBuf::from(need(args, "--run-dir"));
    let run: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("run.json")).unwrap()).unwrap();
    let vs = VectorSet::load_canonical().unwrap();
    let wp28 = PLAN
        .iter()
        .any(|p| p.de440 && TOOLS.iter().any(|t| run_dir.join(t).join(p.name).exists()));
    let data = load_data(wp28);
    let spk = data
        .spk_bytes
        .as_ref()
        .map(|(b, prov)| (Spk::parse(b).unwrap(), prov.clone()));
    let sources = DataSources::none()
        .with_header(&data.header, &data.header_prov)
        .with_small_bodies(&data.small);
    let jdepoc = data.header.value("JDEPOC").unwrap();
    let mut scenarios = Vec::new();
    let mut entries = Vec::new();
    let mut notes = vec![
        format!("header.440: {}", data.header_prov),
        "Accuracy columns are computed by ferroastro-bench from each tool's own output files, identically for every tool; a '–' always carries its reason in the notes under the table.".to_owned(),
        "Mercury |Δr|: heliocentric, max over the common output times, against the scenario's reference run in this same harness invocation (REBOUND for (a) and (e); FerroAstro for (b) and (d)).".to_owned(),
        "Perihelion rates: the WP14/WP17 Park2017 estimator (sampled-state passages, Park17 mean-orbit plane, quadratic + 14 Table-2 terms with SW92 T2A mean motions, 20 jack-knife blocks), window epoch + 30 d to end − 30 d, applied to each tool's daily twin (CI-009).".to_owned(),
    ];
    for p in &PLAN {
        // A harness invocation restricted with --only (WP21) has no directory for the other
        // scenarios: they are left out of the table, with a note, rather than listed as failed.
        if TOOLS.iter().all(|t| !run_dir.join(t).join(p.name).exists()) {
            notes.push(format!(
                "{}: not run in this invocation (harness --only)",
                p.label
            ));
            continue;
        }
        let text = std::fs::read_to_string(scenario_path(p.name)).unwrap();
        let spec = ScenarioSpec::parse(&text).unwrap();
        scenarios.push(ScenarioInfo {
            name: p.name.into(),
            label: p.label.into(),
            daily_twin: p.daily.map(str::to_owned),
            sha256: ferroastro_testdata::sha256_hex(text.as_bytes()),
        });
        let gms: BTreeMap<i32, f64> = if spec.mode == Mode::NBody {
            initial_conditions(&spec, &sources)
                .unwrap()
                .records
                .iter()
                .map(|r| (r.naif, r.gm[0]))
                .collect()
        } else {
            BTreeMap::new()
        };
        let collected: Vec<Collected> =
            TOOLS.iter().map(|t| collect(&run_dir, p.name, t)).collect();
        let reference = reference_tool(p.kind).and_then(|rt| {
            collected
                .iter()
                .zip(TOOLS)
                .find(|(_, t)| *t == rt)
                .and_then(|(c, t)| series_of(c, p.name, t).map(|s| (t, s)))
        });
        // Rates from the daily twins.
        let mut rates: BTreeMap<&str, f64> = BTreeMap::new();
        if let Some(daily) = p.daily {
            let dspec =
                ScenarioSpec::parse(&std::fs::read_to_string(scenario_path(daily)).unwrap())
                    .unwrap();
            let span_days = dspec.span.years * ferroastro_core::time::DAYS_PER_JULIAN_YEAR;
            let est = estimator(
                &vs,
                (jdepoc + MARGIN_DAYS, jdepoc + span_days - MARGIN_DAYS),
            );
            for tool in TOOLS {
                if let Some(s) = daily_series(&run_dir, daily, tool) {
                    let tr = mercury_track(&s).unwrap();
                    match est.estimate(&tr, PassageSource::SampledStates) {
                        Ok(e) => {
                            println!(
                                "{daily} {tool}: rate {:.6} arcsec/cy, sigma_jk {:.2e}, {} passages",
                                e.rate, e.jackknife_sigma, e.passages
                            );
                            rates.insert(tool, e.rate);
                        }
                        Err(err) => println!("{daily} {tool}: estimator failed: {err}"),
                    }
                }
            }
            notes.push(format!(
                "{}: estimator {}",
                p.label,
                est.id(PassageSource::SampledStates)
            ));
        }
        let park = park_reference(&vs, p.kind);
        for (mut c, tool) in collected.into_iter().zip(TOOLS) {
            if c.entry.status == Status::Timed {
                let series = series_of(&c, p.name, tool);
                let a = &mut c.entry.accuracy;
                let own = c
                    .first
                    .as_ref()
                    .and_then(|(_, i)| {
                        i.extra
                            .get("relative_energy_change")
                            .or_else(|| i.extra.get("model_relative_energy_change"))
                    })
                    .map_or_else(String::new, |x| format!("; the tool's own report: {x}"));
                a.relative_energy_change = match p.kind {
                    Kind::PlanetsNewton => {
                        let s = series.as_ref().unwrap();
                        let (r, n) = relative_energy_change(s, &gms).unwrap();
                        Measure::of(
                            r,
                            "",
                            format!(
                                "Newtonian energy of the {n} sampled bodies at the first and last output, centre-of-mass frame, header.440 GMs{own}"
                            ),
                        )
                    }
                    Kind::FullSmallBodies => c
                        .first
                        .as_ref()
                        .and_then(|(_, i)| i.extra.get("model_relative_energy_change"))
                        .and_then(|x| x.parse().ok())
                        .map_or_else(
                            || Measure::none("", "the run reported no model energy"),
                            |r| {
                                Measure::of(
                                    r,
                                    "",
                                    "FerroAstro model energy (Newtonian mutual + Minor-from-Majors with reaction + fixed ring + geocentric Moon), initial vs last",
                                )
                            },
                        ),
                    Kind::MercuryGr => Measure::none(
                        "",
                        "1PN test particle: the Newtonian energy is not a conserved quantity of the model",
                    ),
                    Kind::EphemerisField => {
                        Measure::none("", "test particle in a time-dependent field: no conserved energy")
                    }
                    Kind::PlanetsEih | Kind::PlanetsEihAbm | Kind::Ephemeris => Measure::none(
                        "",
                        "EIH 1PN: the Newtonian energy is not a conserved quantity of the model (the 1PN energy is checked in ferroastro-eih)",
                    ),
                };
                a.mercury_dr_km = match (&reference, &series) {
                    (Some((rt, _)), _) if *rt == tool => {
                        Measure::none("km", "this run is the column's reference (0 by definition)")
                    }
                    (Some((rt, rs)), Some(s)) => match mercury_max_dr(s, rs) {
                        Ok((dr, t, n)) => Measure::of(
                            dr,
                            "km",
                            format!("vs {rt} (same invocation), max at t = {t} d over {n} samples"),
                        ),
                        Err(e) => Measure::none("km", format!("not comparable: {e}")),
                    },
                    (None, _) => Measure::none("km", "no reference tool runs this scenario"),
                    (Some((rt, _)), None) => {
                        Measure::none("km", format!("no output to compare with {rt}"))
                    }
                };
                let fa_rate = rates.get("ferroastro").copied();
                let rate = rates.get(tool).copied();
                c.entry.rate_arcsec_cy = rate;
                a.rate_minus_ferroastro = match (p.daily, rate, fa_rate) {
                    (None, _, _) => Measure::none(
                        "arcsec/cy",
                        "no daily twin: span too short for the 14-term fit or no Mercury series",
                    ),
                    (Some(_), Some(_), Some(_)) if tool == "ferroastro" => {
                        Measure::none("arcsec/cy", "FerroAstro is the column's reference")
                    }
                    (Some(d), Some(r), Some(f)) => Measure::of(
                        r - f,
                        "arcsec/cy",
                        format!("daily twin {d}, same estimator"),
                    ),
                    (Some(d), _, _) => Measure::none(
                        "arcsec/cy",
                        format!(
                            "no rate: the daily twin {d} has no output for this tool or for FerroAstro"
                        ),
                    ),
                };
                a.rate_minus_reference = match (park.as_ref(), rate) {
                    (Some((v, desc)), Some(r)) => Measure::of(r - v, "arcsec/cy", desc.clone()),
                    (Some(_), None) => Measure::none("arcsec/cy", "no rate for this tool"),
                    (None, _) => Measure::none(
                        "arcsec/cy",
                        match p.kind {
                            Kind::FullSmallBodies => {
                                "100-yr span; the N-SUM rate of this model is PHY-013 (WP15 nightly, 2000-yr legs)"
                            }
                            _ => "no published rate for this configuration",
                        },
                    ),
                };
            } else {
                let reason = format!(
                    "{}: {}",
                    if c.entry.status == Status::Refused {
                        "refused"
                    } else {
                        "failed"
                    },
                    c.entry.message.lines().next().unwrap_or("")
                );
                let a = &mut c.entry.accuracy;
                a.relative_energy_change = Measure::none("", reason.clone());
                a.mercury_dr_km = Measure::none("km", reason.clone());
                a.rate_minus_ferroastro = Measure::none("arcsec/cy", reason.clone());
                a.rate_minus_reference = Measure::none("arcsec/cy", reason);
            }
            if p.truth.is_some() || p.de440 {
                wp28_columns(&mut c, p, tool, &run_dir, spk.as_ref().map(|x| &x.0));
            }
            entries.push(c.entry);
        }
    }
    if let Some((_, prov)) = &spk {
        notes.push(format!("de440.bsp: {prov}"));
    }
    for p in PLAN
        .iter()
        .filter(|p| scenarios.iter().any(|s| s.name == p.name))
    {
        if let Some(truth) = p.truth {
            let info = run_dirs(&run_dir.join("ferroastro").join(truth))
                .first()
                .and_then(|d| read_run(d).ok());
            notes.push(info.map_or_else(
                || format!("{}: truth run {truth} missing", p.label),
                |i| {
                    format!(
                        "{}: round-off truth `{truth}` (scenario sha256 {}), one untimed run of {:.1} s (run in parallel with the other truth runs before the timed runs; not a CI-008 timing), IC hash {}, {}",
                        p.label,
                        ferroastro_testdata::sha256_hex(
                            std::fs::read_to_string(scenario_path(truth))
                                .unwrap_or_default()
                                .as_bytes()
                        ),
                        i.wall_clock_s,
                        i.extra.get("ic_hash").map_or("?", String::as_str),
                        i.extra.get("model").map_or("", String::as_str)
                    )
                },
            ));
        }
    }
    let published = published_rows(&vs, &scenarios);
    let f = |k: &str| {
        run.get(k)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(f64::NAN)
    };
    let mut table = SpeedTable {
        schema_version: ferroastro_bench::table::TABLE_SCHEMA_VERSION,
        run_id: run
            .get("run_id")
            .and_then(serde_json::Value::as_str)
            .unwrap()
            .to_owned(),
        generated_utc: run
            .get("finished_utc")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned(),
        host: cpu_host(&run),
        load_avg_run: [f("load_start"), f("load_end")],
        scenarios,
        entries,
        notes,
        published,
    };
    let valid = table.validate();
    if let Err(e) = &valid {
        table
            .notes
            .push(format!("NOT A VALID CI-008 TABLE (smoke run only): {e}"));
    }
    std::fs::write(
        need(args, "--out-json"),
        serde_json::to_string_pretty(&table).unwrap() + "\n",
    )
    .unwrap();
    std::fs::write(need(args, "--out-md"), table.to_markdown()).unwrap();
    println!("{}", table.to_markdown());
    match valid {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_compare(args: &[String]) -> ExitCode {
    let read = |p: &String| -> SpeedTable {
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    };
    let (Some(a), Some(b)) = (args.first(), args.get(1)) else {
        panic!("compare <earlier.json> <later.json>");
    };
    let (prev, next) = (read(a), read(b));
    let rows = stability(&prev, &next, "ferroastro").unwrap();
    let mut ok = true;
    println!(
        "| scenario | median {} (s) | median {} (s) | |Δ| (s) | range {} (s) | range {} (s) | |Δ| < range₁ | |Δ| < range₂ |",
        prev.run_id, next.run_id, prev.run_id, next.run_id
    );
    println!("|---|---|---|---|---|---|---|---|");
    for r in &rows {
        println!(
            "| {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} |",
            r.scenario,
            r.median_prev_s,
            r.median_next_s,
            r.delta_s(),
            r.range_prev_s,
            r.range_next_s,
            if r.pass { "pass" } else { "FAIL" },
            if r.pass_reverse { "pass" } else { "fail" }
        );
        ok &= r.pass;
    }
    println!(
        "WP18 stability acceptance: {}",
        if ok { "PASS" } else { "FAIL" }
    );
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("ferroastro") => cmd_ferroastro(&args),
        Some("assemble") => return cmd_assemble(&args),
        Some("compare") => return cmd_compare(args.get(1..).unwrap_or(&[])),
        _ => {
            eprintln!("usage: speed_table ferroastro|assemble|compare ... (see the file's docs)");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
