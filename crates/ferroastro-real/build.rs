// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Probe whether the compiler is a nightly (or dev) build, so that the `f128` feature can enable
//! `#![feature(f128)]` there and compile to nothing on stable (DEP-007: f128 is nightly-only and
//! behind a non-default feature; `cargo clippy --all-features` must still pass on stable).

use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(fa_nightly)");
    println!("cargo::rerun-if-env-changed=RUSTC");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let nightly = Command::new(rustc)
        .arg("-vV")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .is_some_and(|v| {
            v.lines()
                .next()
                .is_some_and(|l| l.contains("nightly") || l.contains("-dev"))
        });
    if nightly {
        println!("cargo::rustc-cfg=fa_nightly");
    }
}
