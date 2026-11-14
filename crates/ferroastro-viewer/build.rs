// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Build script for the opt-in `embed-header` feature (docs/viewer/DESIGN.md §5; the design of
//! FerroAstro #22, `ferroastro-constants/build.rs`).
//!
//! With the feature on, the fetched JPL `header.440` is located in the FerroAstro data cache
//! (`$FA_DATA_DIR`, else `$CARGO_TARGET_DIR/fa-data`, else `<workspace>/target/fa-data`: the lookup
//! of `ferroastro-testdata` and `tools/fetch_jpl.py`), its SHA-256 is checked against the
//! `de440-header` pin of `tests/data/MANIFEST.toml`, and it is copied into `OUT_DIR`, where
//! `ic.rs` includes it with `include_bytes!`. A missing file fails the build with `FA-E0303`, a
//! changed one with `FA-E0304`. The file is never committed (#22). Without the feature this
//! script does nothing, so the crate builds (and CI's clippy runs) with no data present.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const PRODUCT: &str = "de440-header";
const FIX: &str = "fetch it with `(cd tools && uv run python fetch_jpl.py de440-header)` or set \
                   FA_DATA_DIR to the directory that holds the pinned header.440 (the file is \
                   never committed, #22)";

fn data_dir(manifest_dir: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("FA_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(target).join("fa-data");
    }
    manifest_dir.join("../../target/fa-data")
}

/// The `de440-header` product's `(sha256, url, first_retrieved)` from the MANIFEST.
fn pin(manifest: &Path) -> Result<(String, String, String), String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("FA-E0303: cannot read {}: {e}", manifest.display()))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|e| format!("FA-E0303: {}: {e}", manifest.display()))?;
    let product = table
        .get("product")
        .and_then(toml::Value::as_array)
        .and_then(|ps| {
            ps.iter()
                .find(|p| p.get("name").and_then(toml::Value::as_str) == Some(PRODUCT))
        })
        .ok_or_else(|| {
            format!(
                "FA-E0303: {} has no product {PRODUCT:?}",
                manifest.display()
            )
        })?;
    let field = |k: &str| {
        product
            .get(k)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("FA-E0303: {PRODUCT} in {} lacks {k}", manifest.display()))
    };
    Ok((field("sha256")?, field("url")?, field("first_retrieved")?))
}

fn embed() -> Result<(), String> {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").ok_or("CARGO_MANIFEST_DIR is not set")?,
    );
    let manifest = manifest_dir.join("../../tests/data/MANIFEST.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let (expected, url, pinned) = pin(&manifest)?;
    let src = data_dir(&manifest_dir).join("header.440");
    println!("cargo:rerun-if-changed={}", src.display());
    let bytes = std::fs::read(&src).map_err(|e| {
        format!(
            "FA-E0303: the `embed-header` feature of ferroastro-viewer needs the fetched header.440, \
             but {} cannot be read ({e}); fix: {FIX}. To build without it, drop \
             `--features embed-header` (System mode then refuses at run time).",
            src.display()
        )
    })?;
    let got = Sha256::digest(&bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        });
    if got != expected {
        return Err(format!(
            "FA-E0304: {} has SHA-256 {got}, but tests/data/MANIFEST.toml pins {expected}; fix: \
             use the unmodified JPL file ({FIX})",
            src.display()
        ));
    }
    let out =
        PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?).join("header.440");
    std::fs::write(&out, &bytes).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    println!("cargo:rustc-env=FAV_HEADER_SHA256={got}");
    println!("cargo:rustc-env=FAV_HEADER_URL={url}");
    println!("cargo:rustc-env=FAV_HEADER_PINNED={pinned}");
    println!("cargo:rustc-env=FAV_HEADER_BYTES={}", bytes.len());
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FA_DATA_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
    if std::env::var_os("CARGO_FEATURE_EMBED_HEADER").is_none() {
        return;
    }
    if let Err(message) = embed() {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}
