// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `ferroastro scenario …`: validate, run and resume scenario files (CI-002, CI-010).
//!
//! ```text
//! ferroastro scenario validate FILE...
//! ferroastro scenario run FILE [--header PATH] [--spk PATH] [--small-bodies CSV] --output OUT
//!                             [--stop-years Y --checkpoint CK]
//! ferroastro scenario resume FILE [--header PATH] [--spk PATH] [--small-bodies CSV] --checkpoint CK
//!                             --output OUT
//! ```
//!
//! Data products are passed as paths (the CLI does no network I/O); each is reported with its
//! SHA-256 (`--small-bodies` is the REF-003 sampled-state CSV of `forces.asteroids`/`kbos`, WP26),
//! and so is the output file. Outputs and checkpoints are postcard files (`ferroastro_propagate::RunOutput`,
//! `ferroastro_propagate::Checkpoint`). A scenario that requests something FerroAstro does not
//! implement exits with status 2 and an `FA-E1201` message naming it (REF-011).

use crate::CliError;
use ferroastro_constants::header440::Header;
use ferroastro_core::ErrorCode;
use ferroastro_ephem::Spk;
use ferroastro_propagate::{Checkpoint, DataSources, PropagateError, RunOutput};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_system::SmallBodyStates;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::Write;
use std::path::PathBuf;

/// Usage text of the `scenario` command.
pub const SCENARIO_USAGE: &str = "usage: ferroastro scenario validate FILE...\n       \
     ferroastro scenario run FILE [--header PATH] [--spk PATH] [--small-bodies CSV] --output OUT [--stop-years Y --checkpoint CK]\n       \
     ferroastro scenario resume FILE [--header PATH] [--spk PATH] [--small-bodies CSV] --checkpoint CK --output OUT";

impl From<PropagateError> for CliError {
    fn from(e: PropagateError) -> Self {
        let code = e.code();
        Self(e.to_string(), code)
    }
}

fn usage(msg: &str) -> CliError {
    CliError(format!("FA-E0700: {msg}"), ErrorCode::LadderSelection)
}

fn io(what: &str, e: &std::io::Error) -> CliError {
    CliError(format!("FA-E1303: {what}: {e}"), ErrorCode::PropagateOutput)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[derive(Debug, Default)]
struct Args {
    file: Option<PathBuf>,
    header: Option<PathBuf>,
    spk: Option<PathBuf>,
    small_bodies: Option<PathBuf>,
    output: Option<PathBuf>,
    checkpoint: Option<PathBuf>,
    stop_years: Option<f64>,
}

fn parse(args: &[String]) -> Result<Args, CliError> {
    let mut a = Args::default();
    let mut it = args.iter();
    while let Some(s) = it.next() {
        let mut val = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| usage(&format!("{name} needs a value")))
        };
        match s.as_str() {
            "--header" => a.header = Some(val("--header")?.into()),
            "--spk" => a.spk = Some(val("--spk")?.into()),
            "--small-bodies" => a.small_bodies = Some(val("--small-bodies")?.into()),
            "--output" => a.output = Some(val("--output")?.into()),
            "--checkpoint" => a.checkpoint = Some(val("--checkpoint")?.into()),
            "--stop-years" => {
                let v = val("--stop-years")?;
                a.stop_years = Some(
                    v.parse::<f64>()
                        .ok()
                        .filter(|y| y.is_finite())
                        .ok_or_else(|| usage(&format!("--stop-years {v:?}: need a number")))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!("unknown option {other:?}")));
            }
            other if a.file.is_none() => a.file = Some(other.into()),
            other => return Err(usage(&format!("unexpected argument {other:?}"))),
        }
    }
    Ok(a)
}

fn read(path: &PathBuf, what: &str) -> Result<Vec<u8>, CliError> {
    std::fs::read(path).map_err(|e| io(&format!("cannot read {what} {}", path.display()), &e))
}

fn load_spec(path: &PathBuf) -> Result<ScenarioSpec, CliError> {
    let text = String::from_utf8(read(path, "scenario")?)
        .map_err(|_| usage(&format!("{} is not UTF-8", path.display())))?;
    ScenarioSpec::parse(&text).map_err(|e| CliError(e.to_string(), e.code()))
}

/// The REF-003 sampled small-body states of `--small-bodies`, with a provenance line.
fn load_small_bodies(
    path: Option<&PathBuf>,
) -> Result<Option<(SmallBodyStates, String)>, CliError> {
    path.map(|p| {
        let b = read(p, "small-body states")?;
        let parsed = SmallBodyStates::parse(&String::from_utf8_lossy(&b))
            .map_err(|e| CliError(e.to_string(), e.code()))?;
        Ok((parsed, format!("{} sha256 {}", p.display(), sha256_hex(&b))))
    })
    .transpose()
}

/// Write the encoded run output and report it (summary and the file's SHA-256).
fn write_output<W: Write>(
    out: &mut W,
    output: &PathBuf,
    result: &RunOutput,
) -> Result<(), CliError> {
    let encoded = result.encode()?;
    std::fs::write(output, &encoded)
        .map_err(|e| io(&format!("cannot write {}", output.display()), &e))?;
    summary(out, result)?;
    writeln!(
        out,
        "  output   : {} sha256 {}",
        output.display(),
        sha256_hex(&encoded)
    )
    .map_err(|e| io("stdout", &e))
}

fn summary<W: Write>(out: &mut W, o: &RunOutput) -> Result<(), CliError> {
    let w = |e: std::io::Error| io("stdout", &e);
    writeln!(
        out,
        "scenario {} ({}): IC hash {}; {} states, {} passages, {} steps ({} rejected); finished: {}",
        o.scenario,
        o.precision,
        o.ic_hash,
        o.states.len(),
        o.passages.len(),
        o.stats.steps,
        o.stats.rejected,
        o.finished
    )
    .map_err(w)?;
    writeln!(out, "  constants: {}", o.constants).map_err(w)?;
    writeln!(out, "  ICs      : {}", o.ic_source).map_err(w)?;
    writeln!(out, "  model    : {}", o.model.replace('\n', "; ")).map_err(w)?;
    Ok(())
}

/// Run `ferroastro scenario <args>`; returns whether every step succeeded.
///
/// # Errors
/// Usage errors (`FA-E0700`), scenario errors (`FA-E12xx`), run errors (`FA-E13xx` and the
/// codes of the crates the run drives), I/O errors (`FA-E1303`).
pub fn run_scenario_command<W: Write>(args: &[String], out: &mut W) -> Result<bool, CliError> {
    let Some((cmd, rest)) = args.split_first() else {
        return Err(usage("missing scenario subcommand"));
    };
    if cmd == "validate" {
        if rest.is_empty() {
            return Err(usage("validate needs at least one FILE"));
        }
        for f in rest {
            let spec = load_spec(&f.into())?;
            ferroastro_propagate::refuse_unsupported(&spec)?;
            writeln!(
                out,
                "{f}: valid ({}, {} bodies, switches {:?})",
                spec.mode.label(),
                spec.bodies.len(),
                spec.forces.requested()
            )
            .map_err(|e| io("stdout", &e))?;
        }
        return Ok(true);
    }
    let a = parse(rest)?;
    let file = a
        .file
        .clone()
        .ok_or_else(|| usage("missing scenario FILE"))?;
    let spec = load_spec(&file)?;
    let output = a
        .output
        .clone()
        .ok_or_else(|| usage("--output is required"))?;
    let header_bytes = a
        .header
        .as_ref()
        .map(|p| read(p, "header.440"))
        .transpose()?;
    let spk_bytes = a.spk.as_ref().map(|p| read(p, "SPK")).transpose()?;
    let header = header_bytes
        .as_deref()
        .map(|b| {
            Header::parse(&String::from_utf8_lossy(b))
                .map_err(|e| CliError(e.to_string(), e.code()))
        })
        .transpose()?;
    let header_prov = match (&a.header, &header_bytes) {
        (Some(p), Some(b)) => format!("header.440 {} sha256 {}", p.display(), sha256_hex(b)),
        _ => String::new(),
    };
    let spk = spk_bytes
        .as_deref()
        .map(|b| Spk::parse(b).map_err(|e| CliError(e.to_string(), e.code())))
        .transpose()?;
    let spk_prov = match (&a.spk, &spk_bytes) {
        (Some(p), Some(b)) => format!("SPK {} sha256 {}", p.display(), sha256_hex(b)),
        _ => String::new(),
    };
    let small = load_small_bodies(a.small_bodies.as_ref())?;
    let mut data = DataSources::none();
    if let Some((sb, prov)) = &small {
        writeln!(out, "small bodies: {prov}").map_err(|e| io("stdout", &e))?;
        data = data.with_small_bodies(sb);
    }
    if let Some(h) = &header {
        data = data.with_header(h, &header_prov);
    }
    if let Some(s) = &spk {
        data = data.with_spk(s, &spk_prov);
    }
    let result = match cmd.as_str() {
        "run" => match (a.stop_years, &a.checkpoint) {
            (Some(y), Some(ck_path)) => {
                let (o, ck) = ferroastro_propagate::run_until(&spec, &data, y)?;
                if let Some(ck) = ck {
                    std::fs::write(ck_path, ck.encode()?)
                        .map_err(|e| io(&format!("cannot write {}", ck_path.display()), &e))?;
                    writeln!(out, "checkpoint written: {}", ck_path.display())
                        .map_err(|e| io("stdout", &e))?;
                }
                o
            }
            (None, None) => ferroastro_propagate::run(&spec, &data)?,
            _ => return Err(usage("--stop-years and --checkpoint go together")),
        },
        "resume" => {
            let ck_path = a
                .checkpoint
                .clone()
                .ok_or_else(|| usage("resume needs --checkpoint"))?;
            let ck = Checkpoint::decode(&read(&ck_path, "checkpoint")?)?;
            ferroastro_propagate::resume(&spec, &data, &ck)?
        }
        other => return Err(usage(&format!("unknown scenario subcommand {other:?}"))),
    };
    write_output(out, &output, &result)?;
    Ok(true)
}
