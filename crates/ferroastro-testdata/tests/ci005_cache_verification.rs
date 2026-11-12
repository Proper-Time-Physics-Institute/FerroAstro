// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! CI-005 / REF-002: the PR-fast JPL cache is re-verified on every lookup. A cached file whose
//! bytes do not match the manifest pin is refused with a stable code (it is never used as an
//! oracle), a missing file is refused with the `fetch_jpl.py` command, and the `FA_DATA_DIR`
//! override is honoured (review F2 after WP10: a disabled SHA-256 re-check used to survive).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_core::ErrorCode;
use ferroastro_testdata::{DATA_DIR_ENV, locate_product_in, product, sha256_hex};
use std::path::PathBuf;
use std::process::Command;

/// A fresh, empty scratch directory for one test (removed by the caller).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fa-testdata-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const WRONG: &[u8] = b"this is not header.440; a substituted oracle must be refused\n";

/// Verifies: CI-005, REF-002
#[test]
fn a_cached_file_with_the_wrong_content_is_refused() {
    let dir = scratch("wrong");
    let p = product("de440-header").unwrap();
    // Missing first.
    let err = locate_product_in("de440-header", &dir).unwrap_err();
    assert_eq!(err.code(), ErrorCode::DataMissing);
    let msg = err.to_string();
    assert!(
        msg.contains("missing") && msg.contains("fetch_jpl.py"),
        "{msg}"
    );
    // Wrong bytes under the right name: the SHA-256 re-check refuses them.
    std::fs::write(dir.join(&p.file), WRONG).unwrap();
    let err = locate_product_in("de440-header", &dir).unwrap_err();
    assert_eq!(err.code(), ErrorCode::DataMissing);
    let msg = err.to_string();
    for needle in [
        "FA-E0204",
        "SHA-256",
        &sha256_hex(WRONG),
        &p.sha256,
        "fetch_jpl.py",
    ] {
        assert!(msg.contains(needle), "{msg:?} lacks {needle:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The `FA_DATA_DIR` path end to end: a child process of this test binary runs
/// [`child_locate_probe`] with `FA_DATA_DIR` pointing at a directory that holds wrong content.
/// (Setting the variable in this process would need `unsafe` under edition 2024.)
///
/// Verifies: CI-005, REF-002
#[test]
fn fa_data_dir_override_is_reverified() {
    let dir = scratch("env");
    let p = product("de440-header").unwrap();
    std::fs::write(dir.join(&p.file), WRONG).unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_locate_probe",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(DATA_DIR_ENV, &dir)
        .env("FA_TESTDATA_CHILD_PROBE", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("probe: FA-E0204") && stdout.contains(&sha256_hex(WRONG)),
        "child output:\n{stdout}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Child half of [`fa_data_dir_override_is_reverified`]: only meaningful when that test spawns it
/// with `FA_TESTDATA_CHILD_PROBE=1`; otherwise it checks nothing and passes.
///
/// Verifies: CI-005
#[test]
fn child_locate_probe() {
    if std::env::var_os("FA_TESTDATA_CHILD_PROBE").is_none() {
        return;
    }
    let err = ferroastro_testdata::locate_product("de440-header").unwrap_err();
    assert_eq!(err.code(), ErrorCode::DataMissing);
    println!("probe: {err}");
}
