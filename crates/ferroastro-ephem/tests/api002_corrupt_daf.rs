// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! API-002 / DEP-011: truncated, corrupted and garbage DAF bytes give `FA-E10xx` errors, never a
//! panic, and every error message names its code, the quantity, the valid range and a fix.
//!
//! Inputs are the synthetic SPK of `tests/common` (both byte orders); offline, PR tier.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{SYNTH_A, SYNTH_B, synthetic_spk};
use ferroastro_core::ErrorCode;
use ferroastro_core::frames::Barycentric;
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_ephem::{EphemError, Ephemeris, NaifId, Spk};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

const J2000: f64 = ferroastro_ephem::J2000_JD;
/// Byte offset of the first summary in the synthetic file (record 2, after NEXT/PREV/NSUM).
const SUMMARY0: usize = ferroastro_ephem::RECORD_BYTES + 24;

fn is_ephem_code(e: &EphemError) -> bool {
    matches!(
        e.code(),
        ErrorCode::DafFormat
            | ErrorCode::DafBounds
            | ErrorCode::SpkSegment
            | ErrorCode::EphemCoverage
            | ErrorCode::EphemNoPath
            | ErrorCode::EphemNonFinite
    )
}

/// Parse and, if that succeeds, evaluate everything the API offers at `epochs`; return the
/// first error. Never panics by contract (API-002): a panic fails the test.
fn exercise(bytes: &[u8], epochs: &[(f64, f64)]) -> Result<(), EphemError> {
    let spk = Spk::parse(bytes)?;
    let mut first = None;
    for &(jd1, jd2) in epochs {
        let Ok(e) = Epoch::<Tdb>::from_jd_parts(jd1, jd2) else {
            continue;
        };
        for seg in spk.segments() {
            if let Err(err) = seg.state(e).and_then(|_| {
                seg.state(e.cast::<ferroastro_real::DoubleDouble>())
                    .map(|_| ())
            }) {
                first.get_or_insert(err);
            }
            for r in 0..4 {
                let _ = seg.coefficients(r, 0);
                let _ = seg.record_mid_radius(r);
            }
        }
        for (t, c) in [
            (SYNTH_B, NaifId::SSB),
            (SYNTH_A, SYNTH_B),
            (NaifId::SSB, SYNTH_B),
        ] {
            if let Err(err) = spk.relative_state_km(t, c, e) {
                first.get_or_insert(err);
            }
        }
        // A body that is not in the file is always an error, whatever the bytes.
        let none = spk
            .relative_state_km(NaifId(5), NaifId::SSB, e)
            .unwrap_err();
        assert!(is_ephem_code(&none), "{none}");
        if let Err(err) = spk.state::<Barycentric, f64>(SYNTH_B, e) {
            first.get_or_insert(err);
        }
    }
    first.map_or(Ok(()), Err)
}

fn epochs() -> Vec<(f64, f64)> {
    vec![
        (J2000 - 2.0, 0.25),
        (J2000 - 8.0, 0.0),
        (J2000 + 8.0, 0.0),
        (J2000, 0.5),
    ]
}

/// Verifies: API-002, DEP-011
#[test]
fn every_truncation_into_records_or_data_is_an_error() {
    for big in [false, true] {
        let full = synthetic_spk(big);
        let spk = Spk::parse(&full).unwrap();
        let data_end = spk.segments().iter().map(|s| s.addresses.1).max().unwrap() * 8;
        let mut errors = 0;
        for len in 0..full.len() {
            let r = exercise(&full[..len], &[(J2000 - 2.0, 0.25)]);
            if len < data_end {
                let e = r.expect_err("truncation inside the file must fail");
                assert!(
                    matches!(e.code(), ErrorCode::DafFormat | ErrorCode::DafBounds),
                    "len {len}: {e}"
                );
                errors += 1;
            } else {
                // Only zero padding after the last array was removed: same results.
                r.unwrap();
            }
        }
        println!(
            "{}: {errors} truncations inside records/data -> FA-E1000/E1001; {} padding-only truncations parse",
            if big { "BIG" } else { "LTL" },
            full.len() - errors
        );
    }
}

/// Verifies: API-002, DEP-011
#[test]
fn random_corruption_and_garbage_never_panic() {
    let cfg = Config {
        cases: 2000, // lint: allow-literal(proptest case count)
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(cfg, TestRng::deterministic_rng(RngAlgorithm::ChaCha));
    let base = [synthetic_spk(false), synthetic_spk(true)];
    let len = base[0].len();
    let strategy = (
        any::<bool>(),
        proptest::collection::vec((0..len, any::<u8>()), 1..12),
        proptest::collection::vec((-12.0..12.0f64, -2.0..2.0f64), 1..4), // lint: allow-literal(epoch range in days around the synthetic coverage)
    );
    let (ok, err) = (std::cell::Cell::new(0u32), std::cell::Cell::new(0u32));
    runner
        .run(&strategy, |(big, flips, mut eps)| {
            let mut bytes = base[usize::from(big)].clone();
            for (i, b) in flips {
                bytes[i] = b;
            }
            eps.push((J2000 - 2.0, 0.25));
            let eps: Vec<(f64, f64)> = eps.into_iter().map(|(a, b)| (J2000 + a, b)).collect();
            if Spk::parse(&bytes).is_ok() {
                ok.set(ok.get() + 1);
            } else {
                err.set(err.get() + 1);
            }
            if let Err(e) = exercise(&bytes, &eps) {
                prop_assert!(is_ephem_code(&e), "unexpected code {}", e.code());
            }
            Ok(())
        })
        .unwrap();
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 500,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::deterministic_rng(RngAlgorithm::ChaCha),
    );
    runner
        .run(
            &proptest::collection::vec(any::<u8>(), 0..6000),
            |garbage| {
                if let Err(e) = exercise(&garbage, &epochs()) {
                    prop_assert!(is_ephem_code(&e));
                }
                Ok(())
            },
        )
        .unwrap();
    println!(
        "corruption cases: {} still parse (then evaluated without panic), {} rejected at parse with FA-E10xx; 500 garbage inputs",
        ok.get(),
        err.get()
    );
}

fn patch_i32(b: &mut [u8], at: usize, x: i32) {
    b[at..at + 4].copy_from_slice(&x.to_le_bytes());
}

fn assert_message(e: &EphemError, code: &str, needles: &[&str]) {
    let m = e.to_string();
    assert!(m.starts_with(code), "{m}");
    assert!(m.contains("fix") || m.contains("valid"), "{m}");
    for n in needles {
        assert!(m.contains(n), "{m:?} lacks {n:?}");
    }
}

/// Verifies: API-002
#[test]
fn each_error_has_its_code_quantity_range_and_fix() {
    let good = synthetic_spk(false);
    let e0 = Epoch::<Tdb>::from_jd_parts(J2000 - 2.0, 0.25).unwrap();

    let mut b = good.clone();
    b[88..96].copy_from_slice(b"VAX-GFLT");
    assert_message(
        &Spk::parse(&b).unwrap_err(),
        "FA-E1000",
        &["LOCFMT", "LTL-IEEE"],
    );

    let mut b = good.clone();
    b[699 + 7] = b'\n'; // an ASCII-mode transfer rewrites line endings
    assert_message(&Spk::parse(&b).unwrap_err(), "FA-E1000", &["FTPSTR"]);

    let mut b = good.clone();
    b[..8].copy_from_slice(b"NOTADAF ");
    assert_message(&Spk::parse(&b).unwrap_err(), "FA-E1000", &["LOCIDW"]);

    let mut b = good.clone();
    patch_i32(&mut b, 12, 5); // NI = 5: a DAF, but not an SPK
    assert_message(&Spk::parse(&b).unwrap_err(), "FA-E1000", &["NI = 6"]);

    let mut b = good.clone();
    patch_i32(&mut b, SUMMARY0 + 36, 1_000_000); // lint: allow-literal(final address far beyond the file)
    assert_message(
        &Spk::parse(&b).unwrap_err(),
        "FA-E1001",
        &["final array address", "word"],
    );

    let mut b = good.clone();
    let last = i32::from_le_bytes(b[SUMMARY0 + 36..SUMMARY0 + 40].try_into().unwrap());
    let n_at = usize::try_from(last - 1).unwrap() * 8; // directory word N of segment 0
    b[n_at..n_at + 8].copy_from_slice(&5.0f64.to_le_bytes());
    assert_message(
        &Spk::parse(&b).unwrap_err(),
        "FA-E1002",
        &["N·RSIZE + 4", "words"],
    );

    let mut b = good.clone();
    patch_i32(&mut b, SUMMARY0 + 28, 9); // type 9 (Lagrange): opens, cannot be evaluated
    let spk = Spk::parse(&b).unwrap();
    assert_message(
        &spk.segments()[0].state(e0).unwrap_err(),
        "FA-E1002",
        &["data type", "2 (Chebyshev"],
    );

    let spk = Spk::parse(&good).unwrap();
    let late = Epoch::<Tdb>::from_jd_parts(J2000 + 100.0, 0.0) // lint: allow-literal(epoch outside the synthetic coverage)
        .unwrap();
    assert_message(
        &spk.segments()[0].state(late).unwrap_err(),
        "FA-E1003",
        &["outside the coverage", "s TDB"],
    );
    assert_message(
        &spk.relative_state_km(NaifId(5), NaifId::SSB, e0)
            .unwrap_err(),
        "FA-E1004",
        &["NAIF 5", "share no body"],
    );

    let mut b = good.clone();
    let first_coef = (3 * 128 + 2) * 8; // word 387: first X coefficient of record 0
    b[first_coef..first_coef + 8].copy_from_slice(&f64::NAN.to_le_bytes());
    let spk = Spk::parse(&b).unwrap();
    let e = Epoch::<Tdb>::from_jd_parts(J2000 - 7.0, 0.0).unwrap();
    assert_message(
        &spk.segments()[0].state(e).unwrap_err(),
        "FA-E1005",
        &["position", "km"],
    );
}
