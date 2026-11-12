// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! REF-001 / DEP-011: every body point of JPL's testpo.440 evaluated on de440.bsp, against the
//! acceptance threshold of JPL's `testeph.f` (DEL < 1e-13 au or au/day).
//!
//! PR-fast tier (CI-005): reads the cached de440.bsp and testpo.440 (pinned SHA-256; AU from the
//! committed DE440 constants table);
//! a missing cache fails loudly with the `fetch_jpl.py` command.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{Max, cached_product, check, vectors};
use ferroastro_constants::De440;
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_ephem::{Ephemeris, Spk, naif_id_of_testeph_code, parse_testpo};
use std::collections::BTreeMap;

/// Verifies: REF-001, DEP-011
#[test]
fn every_testpo440_body_point_is_within_the_testeph_threshold() {
    let v = vectors();
    let bytes = cached_product("de440-bsp");
    let spk = Spk::parse(&bytes).unwrap();
    let text = String::from_utf8(cached_product("de440-testpo")).unwrap();
    let points = parse_testpo(&text).unwrap();
    // AU from the committed DE440 constants table (verified against header.440 by WP2's tests).
    let au = De440::load().unwrap().au().km();
    let day = ferroastro_ephem::SECONDS_PER_DAY;
    println!(
        "testpo.440: {} points; AU = {au} km (DE440 table)",
        points.len()
    );

    let mut del = Max::default();
    let mut skipped: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut evaluated = 0usize;
    for p in &points {
        assert_eq!(p.de, 440, "line {}", p.line);
        let (Some(t), Some(c)) = (
            naif_id_of_testeph_code(p.target),
            naif_id_of_testeph_code(p.center),
        ) else {
            *skipped.entry((p.target, p.center)).or_default() += 1;
            continue;
        };
        // testpo dates are TDB Julian dates at 0h (x.5): exact as one binary64.
        let e = Epoch::<Tdb>::from_jd_parts(p.jd_tdb, 0.0).unwrap();
        let (s, frame) = spk.relative_state_km(t, c, e).unwrap();
        assert_eq!(frame, 1);
        let k = usize::from(p.coordinate - 1);
        let ours = if k < 3 {
            s.position[k] / au
        } else {
            s.velocity[k - 3] * day / au
        };
        del.add((ours - p.value).abs(), || {
            format!(
                "line {} {} t{} c{} x{}",
                p.line, p.date, p.target, p.center, p.coordinate
            )
        });
        evaluated += 1;
    }
    let n_skipped: usize = skipped.values().sum();
    println!("evaluated {evaluated} body points; skipped {n_skipped} non-body points {skipped:?}");
    // Every skipped point must be a nutation (14) or libration (15) point: nothing else is dropped.
    assert!(skipped.keys().all(|(t, _)| *t >= 14), "{skipped:?}");
    assert_eq!(evaluated + n_skipped, points.len());
    check(&v, "TESTPO440-MAX-DEL", "f64", &del);
}
