// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! State series and run records of one benchmark run.
//!
//! * [`Series`] reads the plain CSV that every N-body oracle runner writes
//!   (`tools/oracles/nbody_io.py`; the format `ferroastro-oracle` reads as a golden) and writes a
//!   FerroAstro [`RunOutput`] in the same format, so the accuracy columns treat every tool alike.
//!   This crate sits on the same layer as `ferroastro-oracle` (L11) and so cannot depend on it;
//!   the reader is deliberately small, and a PR test parses every committed golden with it.
//! * [`RunInfo`] is one run's timing record: an oracle's `MANIFEST.json` entry (CI-003), or the
//!   `ferroastro.json` the speed-table driver writes for a FerroAstro run.

use crate::BenchError;
use ferroastro_propagate::RunOutput;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// The column line of a state series.
pub const STATE_COLUMNS: &str = "t_days,naif,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s";
/// NAIF id of the Sun.
pub const NAIF_SUN: i32 = 10;
/// NAIF id of Mercury.
pub const NAIF_MERCURY: i32 = 1;
/// The FerroAstro run record's file name in a run directory.
pub const FERROASTRO_RECORD: &str = "ferroastro.json";
/// The oracle manifest's file name (CI-003).
pub const MANIFEST: &str = "MANIFEST.json";
/// The harness's load-average file in a run directory (`<before> <after>`, 1-min averages).
pub const LOAD_FILE: &str = "load.txt";

const SECONDS_PER_DAY: f64 = ferroastro_core::time::SECONDS_PER_DAY;
/// The final state is appended when it is later than the last sample by more than this (s).
const TAIL_MIN_S: f64 = 1e-6;

/// One body's state at one output time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    /// TDB days since the epoch.
    pub t_days: f64,
    /// NAIF id.
    pub naif: i32,
    /// Position (km), ICRF axes, relative to the series' centre.
    pub r: [f64; 3],
    /// Velocity (km/s).
    pub v: [f64; 3],
}

/// A state series: header values and rows in file order.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// File name (for messages).
    pub file: String,
    /// NAIF id of the centre (0 = solar-system barycentre).
    pub center_naif: i32,
    /// Initial epoch, TDB Julian date.
    pub epoch_jd_tdb: f64,
    /// Rows in file order (grouped by output time).
    pub rows: Vec<Row>,
}

/// A body's heliocentric sample: `(t_days, position km, velocity km/s)`.
pub type HelioSample = (f64, [f64; 3], [f64; 3]);

/// `a − b` component-wise.
#[must_use]
pub fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn sum(p: [f64; 2]) -> f64 {
    p[0] + p[1]
}

impl Series {
    /// Parse a state CSV: `# key: value` header lines (`epoch_jd_tdb` required, `center_naif`
    /// optional with 0 as default), the column line [`STATE_COLUMNS`], then rows.
    ///
    /// # Errors
    /// `FA-E1600` for a missing column line, header value or row field, or a non-finite value.
    pub fn parse(file: &str, text: &str) -> Result<Self, BenchError> {
        let bad = |p: String| BenchError::input(file, p);
        let mut meta = BTreeMap::new();
        let mut rows = Vec::new();
        let mut columns = false;
        for (i, line) in text.lines().enumerate() {
            if let Some(c) = line.strip_prefix('#') {
                if let Some((k, v)) = c.split_once(':') {
                    meta.insert(k.trim().to_owned(), v.trim().to_owned());
                }
                continue;
            }
            if !columns {
                if line.trim() != STATE_COLUMNS {
                    return Err(bad(format!("column line {line:?}, want {STATE_COLUMNS:?}")));
                }
                columns = true;
                continue;
            }
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            let [t, naif, x, y, z, vx, vy, vz]: [&str; 8] = f
                .try_into()
                .map_err(|_| bad(format!("line {}: want 8 fields", i + 1)))?;
            let num = |s: &str| -> Result<f64, BenchError> {
                s.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| {
                        bad(format!(
                            "line {}: field {s:?} is not a finite number",
                            i + 1
                        ))
                    })
            };
            rows.push(Row {
                t_days: num(t)?,
                naif: naif
                    .parse()
                    .map_err(|e| bad(format!("line {}: naif {naif:?}: {e}", i + 1)))?,
                r: [num(x)?, num(y)?, num(z)?],
                v: [num(vx)?, num(vy)?, num(vz)?],
            });
        }
        if !columns || rows.is_empty() {
            return Err(bad("no column line or no rows".into()));
        }
        let epoch_jd_tdb = meta
            .get("epoch_jd_tdb")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| bad("header line `# epoch_jd_tdb: <number>` missing".into()))?;
        let center_naif = match meta.get("center_naif") {
            None => 0,
            Some(c) => c
                .parse()
                .map_err(|e| bad(format!("center_naif {c:?}: {e}")))?,
        };
        Ok(Self {
            file: file.to_owned(),
            center_naif,
            epoch_jd_tdb,
            rows,
        })
    }

    /// A FerroAstro run's sampled states (solar-system barycentric), summing each `(hi, lo)`
    /// pair to binary64, followed by the final state when it lies after the last sample (WP28:
    /// a 40-yr run sampled every 20 d ends between samples).
    #[must_use]
    pub fn from_run(file: &str, out: &RunOutput) -> Self {
        let mut rows = Vec::new();
        let last_t = out.states.last().map(|s| sum(s.t));
        let tail = out
            .final_state
            .as_ref()
            .filter(|f| last_t.is_none_or(|t| sum(f.t) > t + TAIL_MIN_S));
        for s in out.states.iter().chain(tail) {
            let t_days = sum(s.t) / SECONDS_PER_DAY;
            for ((id, p), w) in s.bodies.iter().zip(&s.positions).zip(&s.velocities) {
                rows.push(Row {
                    t_days,
                    naif: *id,
                    r: p.map(sum),
                    v: w.map(sum),
                });
            }
        }
        Self {
            file: file.to_owned(),
            center_naif: 0,
            epoch_jd_tdb: sum(out.epoch_jd_tdb),
            rows,
        }
    }

    /// The CSV text (`comment` lines first, then `epoch_jd_tdb` and `center_naif`).
    #[must_use]
    pub fn to_csv(&self, comment: &[String]) -> String {
        let mut s = String::new();
        for c in comment {
            let _ = writeln!(s, "# {c}");
        }
        let _ = writeln!(s, "# epoch_jd_tdb: {:?}", self.epoch_jd_tdb);
        let _ = writeln!(s, "# center_naif: {}", self.center_naif);
        let _ = writeln!(s, "{STATE_COLUMNS}");
        for r in &self.rows {
            let _ = writeln!(
                s,
                "{:?},{},{:?},{:?},{:?},{:?},{:?},{:?}",
                r.t_days, r.naif, r.r[0], r.r[1], r.r[2], r.v[0], r.v[1], r.v[2]
            );
        }
        s
    }

    /// The rows grouped by output time, in file order.
    #[must_use]
    pub fn slices(&self) -> Vec<&[Row]> {
        self.rows
            .chunk_by(|a, b| a.t_days.to_bits() == b.t_days.to_bits())
            .collect()
    }

    /// `body` relative to the Sun at every output time that has it: `(t_days, r, v)`. A
    /// heliocentric series gives its rows directly; a barycentric one subtracts the Sun's row of
    /// the same time.
    ///
    /// # Errors
    /// `FA-E1600` if a barycentric time has `body` but no Sun, or the series is centred on
    /// another body.
    pub fn heliocentric(&self, body: i32) -> Result<Vec<HelioSample>, BenchError> {
        let mut out = Vec::new();
        for slice in self.slices() {
            let Some(b) = slice.iter().find(|r| r.naif == body) else {
                continue;
            };
            match self.center_naif {
                NAIF_SUN => out.push((b.t_days, b.r, b.v)),
                0 => {
                    let s = slice.iter().find(|r| r.naif == NAIF_SUN).ok_or_else(|| {
                        BenchError::input(
                            &self.file,
                            format!("t = {} d: body {body} without the Sun", b.t_days),
                        )
                    })?;
                    out.push((b.t_days, sub3(b.r, s.r), sub3(b.v, s.v)));
                }
                c => {
                    return Err(BenchError::input(
                        &self.file,
                        format!("centre {c}: only barycentric or heliocentric series"),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Read `<dir>/<scenario>.<tool>.states.csv`.
    ///
    /// # Errors
    /// `FA-E1600` if the file is missing or malformed.
    pub fn read(dir: &Path, scenario: &str, tool: &str) -> Result<Self, BenchError> {
        let name = format!("{scenario}.{tool}.states.csv");
        let text = read_text(&dir.join(&name))?;
        Self::parse(&name, &text)
    }
}

/// One run's timing record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunInfo {
    /// Tool name (`ferroastro`, `rebound`, …).
    pub tool: String,
    /// Tool version string.
    pub version: String,
    /// Wall-clock seconds of the timed section.
    pub wall_clock_s: f64,
    /// The output file in the run directory.
    pub file: String,
    /// Tool-specific values (integrator settings, steps, energy, image digest, …).
    pub extra: BTreeMap<String, String>,
    /// 1-min load average before and after the run, when recorded.
    pub load_avg: Option<[f64; 2]>,
}

fn read_text(path: &Path) -> Result<String, BenchError> {
    std::fs::read_to_string(path)
        .map_err(|e| BenchError::input(path.display().to_string(), e.to_string()))
}

fn scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key).map(scalar).unwrap_or_default()
}

/// Read a run directory's timing record: `ferroastro.json` if present, else the first entry of
/// `MANIFEST.json` (oracle runs; image digest, runner commit, CPU and date go into `extra`). The
/// harness's `load.txt`, if present, supplies the load averages.
///
/// # Errors
/// `FA-E1600` if neither record exists or a record is malformed.
pub fn read_run(dir: &Path) -> Result<RunInfo, BenchError> {
    let fa = dir.join(FERROASTRO_RECORD);
    let mut info = if fa.is_file() {
        serde_json::from_str::<RunInfo>(&read_text(&fa)?)
            .map_err(|e| BenchError::input(fa.display().to_string(), e.to_string()))?
    } else {
        let mpath = dir.join(MANIFEST);
        let shown = mpath.display().to_string();
        let m: serde_json::Value = serde_json::from_str(&read_text(&mpath)?)
            .map_err(|e| BenchError::input(&shown, e.to_string()))?;
        let e = m
            .get("entries")
            .and_then(|e| e.get(0))
            .ok_or_else(|| BenchError::input(&shown, "no entries"))?;
        let wall = e
            .get("wall_clock_s")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| BenchError::input(&shown, "no wall_clock_s"))?;
        let mut extra: BTreeMap<String, String> = e
            .get("extra")
            .and_then(serde_json::Value::as_object)
            .map(|o| o.iter().map(|(k, v)| (k.clone(), scalar(v))).collect())
            .unwrap_or_default();
        for k in ["image_digest", "runner_sha", "cpu", "generated_utc"] {
            extra.insert(k.to_owned(), str_field(e, k));
        }
        RunInfo {
            tool: str_field(e, "tool"),
            version: str_field(e, "version"),
            wall_clock_s: wall,
            file: str_field(e, "file"),
            extra,
            load_avg: None,
        }
    };
    let load = dir.join(LOAD_FILE);
    if load.is_file() {
        let v: Vec<f64> = read_text(&load)?
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if let [a, b] = v.as_slice() {
            info.load_avg = Some([*a, *b]);
        }
    }
    Ok(info)
}
