// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Library part of the `ferroastro` CLI (argument parsing and the ladder report), kept separate
//! from `main` so that it can be tested.

use ferroastro_constants::De440;
use ferroastro_core::ErrorCode;
use ferroastro_ladder::single_body::{RungResult, SingleBody, rung_set};
use ferroastro_ladder::{LadderError, Scenario};
use ferroastro_vectors::VectorSet;
use std::fmt::Write as _;
use std::io::Write;

mod scenario_cmd;
pub use scenario_cmd::{SCENARIO_USAGE, run_scenario_command};

/// Usage text.
pub const USAGE: &str =
    "usage: ferroastro ladder --rungs single-body [--orbits N] [--details] [--no-check]";

/// Parsed command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Rung set name.
    pub rungs: String,
    /// Orbits per run (default: the scenario's).
    pub orbits: Option<u32>,
    /// Print the PHY-015 metadata block of every rung.
    pub details: bool,
    /// Compare with the test vectors when they are available.
    pub check: bool,
}

/// Errors of the CLI (`FA-E0700` for usage errors).
#[derive(Debug)]
pub struct CliError(pub String, pub ErrorCode);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<LadderError> for CliError {
    fn from(e: LadderError) -> Self {
        let code = e.code();
        Self(e.to_string(), code)
    }
}

fn usage(msg: &str) -> CliError {
    CliError(format!("FA-E0700: {msg}"), ErrorCode::LadderSelection)
}

/// Parse the arguments after the program name.
///
/// # Errors
/// `FA-E0700` for an unknown command or option, or a malformed value.
pub fn parse_args(args: &[String]) -> Result<Options, CliError> {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("ladder") => {}
        Some(other) => return Err(usage(&format!("unknown command {other:?}"))),
        None => return Err(usage("missing command")),
    }
    let mut opts = Options {
        rungs: String::new(),
        orbits: None,
        details: false,
        check: true,
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--rungs" => {
                opts.rungs
                    .clone_from(it.next().ok_or_else(|| usage("--rungs needs a value"))?);
            }
            "--orbits" => {
                let v = it.next().ok_or_else(|| usage("--orbits needs a value"))?;
                let n: u32 = v
                    .parse()
                    .ok()
                    .filter(|n| *n >= 2)
                    .ok_or_else(|| usage(&format!("--orbits {v:?}: need an integer >= 2")))?;
                opts.orbits = Some(n);
            }
            "--details" => opts.details = true,
            "--no-check" => opts.check = false,
            other => return Err(usage(&format!("unknown option {other:?}"))),
        }
    }
    if opts.rungs.is_empty() {
        return Err(usage("--rungs is required"));
    }
    Ok(opts)
}

/// The requirement oracle of a rung id: `(vector id, compared quantity)`.
fn oracle_of(id: &str) -> Option<(&'static str, &'static str)> {
    match id {
        "SR-a" => Some(("PHY007-SRA-NUM-RATE", "rate")),
        "SR-b/harmonic" => Some(("PHY008-SRB-HARMONIC-RATE", "rate")),
        "SR-b/areal" => Some(("PHY008-SRB-AREAL-RATE", "rate")),
        "GR-SCHW" => Some(("PHY005-GR-NUM-RATE", "rate")),
        "J2" | "J2-DE440" => Some(("PHY010-J2-NUM-MINUS-ANALYTIC", "num-ana")),
        "LT" => Some(("PHY011-LT-NUM-RATE", "rate")),
        _ => None,
    }
}

/// NUM-015 verdict: pass if the relative OR the absolute-floor vector passes.
fn num015(vs: &VectorSet, r: &RungResult) -> Option<bool> {
    let rel = vs.get("NUM015-REL-TOL").ok()?;
    let abs = vs.get("NUM015-ABS-FLOOR").ok()?;
    Some(
        rel.check(r.per_orbit_relative_deviation()).is_ok()
            || abs.check(r.per_orbit_difference()).is_ok(),
    )
}

/// The oracle column, the verdict word and whether the row passed.
fn verdict(vectors: Option<&VectorSet>, id: &str, r: &RungResult) -> (String, &'static str, bool) {
    let Some(vs) = vectors else {
        return ("(test vectors not found)".into(), "n/a", true);
    };
    let num_ok = num015(vs, r).unwrap_or(false);
    let Some((vid, what)) = oracle_of(id) else {
        return (
            "NUM-015 only".into(),
            if num_ok { "PASS" } else { "FAIL" },
            num_ok,
        );
    };
    let Ok(v) = vs.get(vid) else {
        return ("vector missing".into(), "n/a", true);
    };
    let (measured, shown) = if what == "rate" {
        (r.numerical_arcsec_per_century(), "")
    } else {
        (
            r.numerical_arcsec_per_century() - r.analytic_arcsec_per_century(),
            "num-ana ",
        )
    };
    let ok = v.check(measured).is_ok() && num_ok;
    let expected = v.value().unwrap_or(f64::NAN);
    (
        format!(
            "{vid}: {shown}{expected} +/- {:e}",
            v.tolerance.allowed(expected)
        ),
        if ok { "PASS" } else { "FAIL" },
        ok,
    )
}

/// Run the selected rungs and write the report. Returns `Ok(false)` if any check failed.
///
/// # Errors
/// Constants, scenario, model or integration errors.
pub fn run<W: Write>(opts: &Options, out: &mut W) -> Result<bool, CliError> {
    let io = |e: std::io::Error| CliError(format!("output: {e}"), ErrorCode::LadderSelection);
    let de = De440::load().map_err(LadderError::from)?;
    let scn = Scenario::mercury_single_body()?;
    let rungs = rung_set(&opts.rungs, &scn, &de)?;
    let mut setup = SingleBody::mercury(&scn, &de)?;
    if let Some(n) = opts.orbits {
        setup.orbits = n;
    }
    // The report is evidence: an FA_VECTORS_DIR override is refused (FA-E0106), never silently
    // ignored; a checkout without tests/vectors only loses the oracle column.
    let vectors = if opts.check {
        match VectorSet::load_canonical() {
            Ok(v) => Some(v),
            Err(e @ ferroastro_vectors::VectorError::NonCanonical { .. }) => {
                return Err(CliError(e.to_string(), e.code()));
            }
            Err(_) => None,
        }
    } else {
        None
    };
    let mut text = String::new();
    let _ = writeln!(
        text,
        "FerroAstro ladder: {} rungs, scenario {} ({} orbits per run, test particle, mu = GM_sun DE440)",
        opts.rungs, scn.name, setup.orbits
    );
    let _ = writeln!(
        text,
        "{:<14} {:<34} {:>13} {:>13} {:>10}  {:<44} verdict",
        "rung", "gauge", "numerical", "analytic", "NUM-015", "oracle (requirement)"
    );
    let _ = writeln!(
        text,
        "{:<14} {:<34} {:>13} {:>13} {:>10}",
        "", "", "arcsec/cy", "arcsec/cy", "rel.dev"
    );
    out.write_all(text.as_bytes()).map_err(io)?;
    let mut all_ok = true;
    let mut details = String::new();
    for rung in &rungs {
        let r = setup.run(rung)?;
        let gauge: String = r.gauge.label().chars().take(34).collect();
        let (oracle, verdict, ok) = verdict(vectors.as_ref(), &rung.id, &r);
        all_ok &= ok;
        let line = format!(
            "{:<14} {:<34} {:>13.7} {:>13.7} {:>10.2e}  {:<44} {}\n",
            rung.id,
            gauge,
            r.numerical_arcsec_per_century(),
            r.analytic_arcsec_per_century(),
            r.per_orbit_relative_deviation(),
            oracle,
            verdict
        );
        out.write_all(line.as_bytes()).map_err(io)?;
        let _ = writeln!(
            details,
            "{}\n  requirements: {}; inclination of pole to orbit {:.4} deg; LRL cross-check {:.7} arcsec/cy; \
             {} orbits, {} steps, {} force evaluations, max |r.v|/(|r||v|) at passages {:.1e}, wall {:.2} s\n",
            r.report,
            rung.requirements,
            r.inclination.radians().to_degrees(),
            r.lrl_arcsec_per_century(),
            r.fit.orbits,
            r.stats.steps,
            r.stats.force_evaluations,
            r.max_residual,
            r.wall_seconds
        );
    }
    let mut tail = String::from(
        "\nSR-b is pedagogical: the velocity-dependent share of the 1PN advance is gauge dependent and is \
         not an observable; only the full 1PN sum (2+2gamma-beta)/3 x 6 pi GM/(c^2 p) is.\n\
         SR-a uses H = gamma m c^2 - mu m/r; other SR couplings give other fractions of GR (e.g. 1/3).\n",
    );
    if opts.details {
        tail.push('\n');
        tail.push_str(&details);
    }
    out.write_all(tail.as_bytes()).map_err(io)?;
    Ok(all_ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    /// Verifies: API-002
    #[test]
    fn argument_errors_are_coded() {
        let ok = parse_args(&args("ladder --rungs single-body --orbits 20 --details")).unwrap();
        assert_eq!(ok.orbits, Some(20));
        assert!(ok.details && ok.check);
        for bad in [
            "",
            "plot",
            "ladder",
            "ladder --rungs",
            "ladder --rungs x --orbits 1",
            "ladder --frobnicate",
        ] {
            let e = parse_args(&args(bad)).unwrap_err();
            assert_eq!(e.1, ErrorCode::LadderSelection, "{bad}");
            assert!(e.0.starts_with("FA-E0700"));
        }
    }
}
