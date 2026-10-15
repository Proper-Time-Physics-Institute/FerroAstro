// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Generate `crates/ferroastro-constants/data/de440-constants.toml` from JPL `header.440`.
//!
//! ```sh
//! cargo run -p ferroastro-constants --example gen_de440_table           # write the table
//! cargo run -p ferroastro-constants --example gen_de440_table -- --stdout
//! ```
//!
//! Downloads the `de440-header` product of `tests/data/MANIFEST.toml` (pinned SHA-256, cached in
//! `$FA_DATA_DIR` or `target/fa-data`), parses GROUP 1040/1041 and renders the table. No value
//! is typed by hand (REF-002).

use ferroastro_constants::header440::Header;
use ferroastro_constants::table::{SourceInfo, render};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fetched = ferroastro_testdata::fetch_product("de440-header")?;
    eprintln!("de440-header {}", fetched.provenance());
    let text = std::fs::read_to_string(&fetched.path)?;
    let header = Header::parse(&text)?;
    let source = SourceInfo {
        product: "de440-header".to_owned(),
        url: fetched.url.clone(),
        sha256: fetched.sha256.clone(),
        bytes: fetched.bytes,
        retrieved: fetched.retrieved.clone(),
        last_modified: fetched.last_modified.clone(),
        title: String::new(),
        constant_count: 0,
        ma_count: 0,
    };
    let out = render(&header, &source)?;
    if std::env::args().any(|a| a == "--stdout") {
        print!("{out}");
    } else {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/de440-constants.toml");
        std::fs::write(&path, out)?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}
