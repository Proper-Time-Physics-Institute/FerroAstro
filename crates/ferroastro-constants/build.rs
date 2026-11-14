// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Build script for the opt-in `embed-header` feature (#22).
//!
//! With the feature on, the fetched JPL `header.440` is copied from the FerroAstro data cache
//! (`$FA_DATA_DIR`, else `$CARGO_TARGET_DIR/fa-data`, else `<workspace>/target/fa-data`: the
//! lookup of `ferroastro-testdata` and `tools/fetch_jpl.py`) into `OUT_DIR`, where
//! `header440::EMBEDDED` includes it. The file is never committed, the feature is never enabled
//! for published builds, and `De440::embedded` verifies the SHA-256 at run time. Without the
//! feature this script does nothing.

use std::path::PathBuf;

fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("FA_DATA_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        return Some(PathBuf::from(target).join("fa-data"));
    }
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
    Some(manifest.join("../../target/fa-data"))
}

fn main() -> Result<(), String> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FA_DATA_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
    if std::env::var_os("CARGO_FEATURE_EMBED_HEADER").is_none() {
        return Ok(());
    }
    let src = data_dir()
        .ok_or("FA-E0303: cannot locate the data cache (no FA_DATA_DIR, CARGO_TARGET_DIR or CARGO_MANIFEST_DIR)")?
        .join("header.440");
    println!("cargo:rerun-if-changed={}", src.display());
    let out =
        PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?).join("header.440");
    std::fs::copy(&src, &out).map_err(|e| {
        format!(
            "FA-E0303: the `embed-header` feature needs the fetched header.440, but {} cannot be \
             read ({e}); fix: run `(cd tools && uv run python fetch_jpl.py de440-header)` or set \
             FA_DATA_DIR to the directory that holds it (the file is never committed)",
            src.display()
        )
    })?;
    Ok(())
}
