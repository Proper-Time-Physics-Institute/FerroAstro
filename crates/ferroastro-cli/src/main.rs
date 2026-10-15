// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `ferroastro`: the FerroAstro command-line tool.
//!
//! ```text
//! ferroastro ladder --rungs single-body [--orbits N] [--details] [--no-check]
//! ferroastro scenario validate|run|resume …   (see `ferroastro_cli::SCENARIO_USAGE`)
//! ```
//!
//! Runs the rung set and prints one table row per rung: numerical and analytic rate (″/cy),
//! the NUM-015 per-orbit deviation, and — when the test vectors of a FerroAstro checkout are
//! found (the workspace `tests/vectors`; an `FA_VECTORS_DIR` override is refused with
//! FA-E0106) — the requirement oracle and a PASS/FAIL verdict. `--details` prints the PHY-015 metadata block of every rung.

use ferroastro_cli::{Options, parse_args, run};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("scenario") {
        return match ferroastro_cli::run_scenario_command(
            args.get(1..).unwrap_or(&[]),
            &mut std::io::stdout(),
        ) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(1),
            Err(e) => {
                eprintln!("{e}");
                eprintln!("{}", ferroastro_cli::SCENARIO_USAGE);
                ExitCode::from(2)
            }
        };
    }
    let opts: Options = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("{}", ferroastro_cli::USAGE);
            return ExitCode::from(2);
        }
    };
    match run(&opts, &mut std::io::stdout()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
