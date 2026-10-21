// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-003: no ladder-rung code path references the IAU 2015 B3 `nominal` namespace.
//!
//! "Ladder code" is every Rust source under `crates/` except the `nominal` module itself and this
//! test. The scan strips comments and string literals, then flags any identifier `nominal` (the
//! module, however it is imported or re-exported) or `Nominal` (the wrapper type).
//! Together with the `Nominal<T>` wrapper (no `Deref`/`From`), this keeps the nominal values out of
//! the precision rungs.

use std::path::{Path, PathBuf};

const ALLOWED: [&str; 3] = [
    "crates/ferroastro-constants/src/nominal.rs",
    "crates/ferroastro-constants/src/lib.rs",
    "crates/ferroastro-constants/tests/nominal_quarantine.rs",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "ui") {
                continue;
            }
            rust_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Blank out comments, string and char literals so that prose cannot trigger the scan.
fn strip(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for d in chars.by_ref() {
                    if prev == '*' && d == '/' {
                        break;
                    }
                    prev = d;
                }
                out.push(' ');
            }
            '"' => {
                let mut escaped = false;
                for d in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if d == '\\' {
                        escaped = true;
                    } else if d == '"' {
                        break;
                    }
                }
                out.push_str("\"\"");
            }
            _ => out.push(c),
        }
    }
    out
}

/// True if code (already stripped) contains the identifier `nominal` or `Nominal`. This is
/// deliberately conservative: a local variable named exactly `nominal` is also flagged.
fn references_nominal(code: &str) -> bool {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|ident| ident == "nominal" || ident == "Nominal")
}

/// Verifies: PHY-003
#[test]
fn ladder_code_never_references_nominal() {
    let root = root();
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    assert!(
        files.len() > 10,
        "scan found too few files: {}",
        files.len()
    );
    let mut offenders = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&root)
            .unwrap_or(f)
            .to_string_lossy()
            .replace('\\', "/");
        if ALLOWED.contains(&rel.as_str()) {
            continue;
        }
        let src = std::fs::read_to_string(f).unwrap();
        if references_nominal(&strip(&src)) {
            offenders.push(rel);
        }
    }
    println!("scanned {} Rust files", files.len());
    assert!(
        offenders.is_empty(),
        "PHY-003: these files reference the IAU 2015 B3 `nominal` namespace: {offenders:?}"
    );
}

/// Verifies: PHY-003
#[test]
fn scanner_detects_references_and_ignores_prose() {
    for hit in [
        "use ferroastro_constants::nominal;",
        "let gm = ferroastro_constants::nominal::solar_mass_parameter();",
        "use ferroastro_constants::{De440, nominal};",
        "use ferroastro_constants :: nominal :: solar_radius;",
        "fn f(x: Nominal<GravParam>) {}",
    ] {
        assert!(references_nominal(&strip(hit)), "missed: {hit}");
    }
    for clean in [
        "// the nominal:: namespace is quarantined",
        "/* nominal::solar_radius */ let x = 1;",
        "let s = \"nominal::x\";",
        "let nominal_value = de.gm_sun(); let denominal = 1;",
    ] {
        assert!(
            !references_nominal(&strip(clean)),
            "false positive: {clean}"
        );
    }
}
