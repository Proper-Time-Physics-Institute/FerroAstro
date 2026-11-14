// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Print FerroAstro's run-time parse of the pinned `header.440` as JSON, for the live
//! cross-check `SYSTEM-HEADER-LIVE-CROSSCHECK-MAX-ULP` (REF-002, PHY-002; #22).
//!
//! ```sh
//! cargo run -p ferroastro-constants --example header_constants_json            # data cache
//! cargo run -p ferroastro-constants --example header_constants_json -- PATH    # explicit file
//! ```
//!
//! The header is verified against the pinned SHA-256 first (FA-E0304). The output lists, for
//! every constant of `data/de440-metadata.toml`, the shortest round-trip decimal and the
//! binary64 bits. It is compared with `tools/oracles/header440.py` and with Orekit by
//! `tools/oracles/header_crosscheck.py`; it is never committed.

use ferroastro_constants::De440;
use std::fmt::Write as _;

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (de, source) = if let Some(path) = std::env::args().nth(1) {
        (De440::from_header_bytes(&std::fs::read(&path)?)?, path)
    } else {
        let f = ferroastro_testdata::locate_product("de440-header")?;
        (De440::load_cached()?, f.path.display().to_string())
    };
    let t = de.table();
    let mut s = String::from("{\n");
    let _ = writeln!(
        s,
        "  \"reader\": \"FerroAstro ferroastro-constants (De440 table)\","
    );
    let _ = writeln!(s, "  \"source\": {},", json_str(&source));
    let _ = writeln!(s, "  \"sha256\": {},", json_str(&t.source().sha256));
    s.push_str("  \"constants\": {\n");
    let n = t.entries().len();
    for (i, e) in t.entries().iter().enumerate() {
        let v = e.value();
        let _ = writeln!(
            s,
            "    {}: {{\"repr\": \"{v:?}\", \"bits\": \"{:016x}\"}}{}",
            json_str(&e.name),
            v.to_bits(),
            if i + 1 == n { "" } else { "," }
        );
    }
    s.push_str("  }\n}\n");
    print!("{s}");
    Ok(())
}
