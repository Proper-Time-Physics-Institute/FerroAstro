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

/// Two halves, so that the test never passes without checking something (issue #38):
///
/// * spawned by [`fa_data_dir_override_is_reverified`] (`FA_TESTDATA_CHILD_PROBE=1`, `FA_DATA_DIR`
///   at a directory holding wrong content): the lookup is refused with `DataMissing`, and the
///   refusal names the file in the overriding directory (the parent checks the printed line for
///   the code and the wrong bytes' SHA-256);
/// * run on its own (the PR-fast tier, with the cache `tools/fetch_jpl.py` fills): the lookup
///   through the same [`locate_product`](ferroastro_testdata::locate_product) path succeeds, the
///   located file is the one under [`data_dir`](ferroastro_testdata::data_dir), and an
///   independent re-hash of its bytes equals the `tests/data/MANIFEST.toml` pin.
///
/// Verifies: CI-005, REF-002
#[test]
fn child_locate_probe() {
    let dir = ferroastro_testdata::data_dir();
    let p = product("de440-header").unwrap();
    let located = ferroastro_testdata::locate_product("de440-header");
    if std::env::var_os("FA_TESTDATA_CHILD_PROBE").is_some() {
        let err = located.unwrap_err();
        assert_eq!(err.code(), ErrorCode::DataMissing);
        let msg = err.to_string();
        assert!(
            msg.contains(&dir.join(&p.file).display().to_string()),
            "the refusal does not name the file under the FA_DATA_DIR override {}: {msg}",
            dir.display()
        );
        println!("probe: {err}");
        return;
    }
    let f = located
        .unwrap_or_else(|e| panic!("de440-header must be located in {}: {e}", dir.display()));
    assert_eq!(f.path, dir.join(&p.file));
    let bytes = std::fs::read(&f.path).unwrap();
    assert_eq!(bytes.len() as u64, f.bytes);
    assert_eq!(
        sha256_hex(&bytes),
        p.sha256,
        "{} does not match its manifest pin",
        f.path.display()
    );
    assert_eq!(f.sha256, p.sha256);
    println!(
        "located {} ({} bytes, SHA-256 {} = manifest pin)",
        f.path.display(),
        f.bytes,
        f.sha256
    );
}
