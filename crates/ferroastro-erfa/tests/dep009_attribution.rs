// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-009 / LIC-004 attribution and naming checks (inspection made executable):
//! * `NOTICE` carries the ERFA copyright, conditions and disclaimer;
//! * every ported source file states `Apache-2.0 AND BSD-3-Clause`, the ERFA copyright, the ERFA
//!   file it was ported from, and ends with the ERFA licence block;
//! * no public symbol anywhere in the workspace starts with SOFA's `iau` prefix.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const ERFA_COPYRIGHT: &str = "Copyright (C) 2013-2023, NumFOCUS Foundation.";
const ERFA_CONDITION_1: &str = "Redistributions of source code must retain the above copyright";
const ERFA_DISCLAIMER_END: &str = "POSSIBILITY OF SUCH DAMAGE.";

/// Verifies: DEP-009, LIC-004
#[test]
fn notice_carries_the_erfa_bsd3_notice() {
    let notice = fs::read_to_string(root().join("NOTICE")).unwrap();
    for needle in [
        "ERFA 2.0.1",
        ERFA_COPYRIGHT,
        "derived, with permission, from the International",
        ERFA_CONDITION_1,
        "Neither the name of the Standards Of Fundamental Astronomy Board",
        ERFA_DISCLAIMER_END,
    ] {
        assert!(notice.contains(needle), "NOTICE lacks {needle:?}");
    }
    assert!(root().join("LICENSES/BSD-3-Clause.txt").is_file());
}

/// Verifies: DEP-009, LIC-004
#[test]
fn every_ported_file_carries_spdx_copyright_source_and_licence() {
    let src = root().join("crates/ferroastro-erfa/src");
    let mut ported = 0;
    for entry in fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().is_some_and(|n| n == "lib.rs") {
            continue; // FerroAstro's own module docs and error type (Apache-2.0)
        }
        let text = fs::read_to_string(&path).unwrap();
        let name = path.display();
        assert!(
            text.contains("SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause"),
            "{name}"
        );
        assert!(
            text.contains("SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation"),
            "{name}"
        );
        assert!(
            text.contains("ERFA 2.0.1"),
            "{name} must cite its ERFA source"
        );
        assert!(
            text.contains(ERFA_COPYRIGHT)
                && text.contains(ERFA_CONDITION_1)
                && text.trim_end().ends_with(ERFA_DISCLAIMER_END),
            "{name} must end with the ERFA licence block"
        );
        assert!(!text.to_lowercase().contains("sofa_"), "{name}");
        ported += 1;
    }
    assert!(ported >= 7, "found only {ported} ported files");
}

/// Verifies: DEP-009
#[test]
fn no_public_symbol_uses_the_sofa_iau_prefix() {
    let mut offenders = Vec::new();
    let crates = root().join("crates");
    let mut stack = vec![crates];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            for (i, line) in text.lines().enumerate() {
                let t = line.trim_start();
                if !t.starts_with("pub") {
                    continue;
                }
                let words: Vec<&str> = t
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .filter(|w| !w.is_empty())
                    .collect();
                // pub [(crate)] <kind> <name>
                let kinds = [
                    "fn", "const", "static", "struct", "enum", "trait", "type", "mod", "use",
                ];
                if let Some(k) = words.iter().position(|w| kinds.contains(w))
                    && let Some(name) = words.get(k + 1)
                    && name.to_ascii_lowercase().starts_with("iau")
                {
                    offenders.push(format!("{}:{}: {t}", path.display(), i + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "iau-prefixed public symbols: {offenders:#?}"
    );
}
