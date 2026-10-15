// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! REF-004 / DEP-011 / DEP-007: every de440.bsp segment against jplephem 2.24, and the
//! double-double evaluation against f64.
//!
//! PR-fast tier (CI-005): reads the **cached** de440.bsp (pinned SHA-256, re-verified) and the
//! committed golden table; a missing cache fails loudly with the `fetch_jpl.py` command.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{Max, cached_product, check, load, vectors};
use ferroastro_core::frames::{Barycentric, Heliocentric};
use ferroastro_ephem::{Ephemeris, NaifId, Segment, Spk, SpkType};
use ferroastro_real::{DoubleDouble, Real};
use std::collections::BTreeMap;

const MIN_RANDOM_EPOCHS: usize = 50;

/// Verifies: REF-004, DEP-011
#[test]
fn every_de440_segment_matches_jplephem_to_1_mm_and_1_um_per_s() {
    let v = vectors();
    let g = load("jplephem-de440.csv", &v, "REF004-JPLEPHEM-POS-MAX-DIFF");
    let bytes = cached_product("de440-bsp");
    assert!(
        g.meta("source").contains(&common::sha256(&bytes)),
        "golden made from another file"
    );
    let spk = Spk::parse(&bytes).unwrap();
    println!(
        "de440.bsp: {} ({} segments, {:?})",
        spk.daf().internal_name(),
        spk.segments().len(),
        spk.daf().endian()
    );
    let (mut pos, mut vel) = (Max::default(), Max::default());
    let mut per_seg: BTreeMap<usize, (usize, Max, Max)> = BTreeMap::new();
    for row in &g.rows {
        let seg = &spk.segments()[usize::try_from(g.i(row, "seg")).unwrap()];
        assert_eq!(
            (seg.target, seg.center),
            (g.i(row, "target"), g.i(row, "center"))
        );
        assert_eq!(seg.data_type, SpkType::Chebyshev2);
        let s = seg.state(g.epoch(row)).unwrap();
        let entry = per_seg.entry(seg.index).or_default();
        if g.s(row, "kind") == "random" {
            entry.0 += 1;
        }
        let at = || {
            format!(
                "seg {} ({}) jd {}+{}",
                seg.index,
                seg.target,
                g.s(row, "jd1"),
                g.s(row, "jd2")
            )
        };
        for (k, c) in ["x", "y", "z"].iter().enumerate() {
            let d = (s.position[k] - g.f(row, c)).abs();
            pos.add(d, at);
            entry.1.add(d, at);
        }
        for (k, c) in ["vx", "vy", "vz"].iter().enumerate() {
            let d = (s.velocity[k] - g.f(row, c)).abs();
            vel.add(d, at);
            entry.2.add(d, at);
        }
    }
    assert_eq!(
        per_seg.len(),
        spk.segments().len(),
        "every segment is compared"
    );
    for (i, (n, p, w)) in &per_seg {
        let seg = &spk.segments()[*i];
        println!(
            "  seg {i:>2} {:>3} wrt {:>2}: {n} random epochs, max |dr| {:.3e} km, max |dv| {:.3e} km/s",
            seg.target, seg.center, p.value, w.value
        );
        assert!(*n >= MIN_RANDOM_EPOCHS, "REF-004: >= 50 epochs per segment");
    }
    check(&v, "REF004-JPLEPHEM-POS-MAX-DIFF", "f64", &pos);
    check(&v, "REF004-JPLEPHEM-VEL-MAX-DIFF", "f64", &vel);
}

/// `n u Σ|c_k|` (positions) and `n u Σ k²|c_k| · 2/INTLEN` (velocities) for one component.
fn roundoff_scale(seg: &Segment<'_>, record: usize, comp: usize) -> (f64, f64) {
    let c = seg.coefficients(record, comp).unwrap();
    let n = f64::from(u32::try_from(c.len()).unwrap());
    let u = f64::EPSILON / 2.0;
    let p: f64 = c.iter().map(|x| x.abs()).sum();
    let d: f64 = c
        .iter()
        .enumerate()
        .map(|(k, x)| f64::from(u32::try_from(k * k).unwrap()) * x.abs())
        .sum();
    let intlen = seg.layout.unwrap().intlen;
    (n * u * p, n * u * d * 2.0 / intlen)
}

/// Verifies: DEP-007, DEP-011
#[test]
fn double_double_agrees_with_f64_to_binary64_roundoff() {
    let v = vectors();
    let g = load("jplephem-de440.csv", &v, "REF004-JPLEPHEM-POS-MAX-DIFF");
    let bytes = cached_product("de440-bsp");
    let spk = Spk::parse(&bytes).unwrap();
    let (mut rho, mut dd_vs_jpl) = (Max::default(), Max::default());
    for row in &g.rows {
        let seg = &spk.segments()[usize::try_from(g.i(row, "seg")).unwrap()];
        let e = g.epoch(row);
        let f = seg.state(e).unwrap();
        let d = seg.state(e.cast::<DoubleDouble>()).unwrap();
        let (record, _) = seg.locate(e).unwrap();
        let at = || {
            format!(
                "seg {} jd {}+{}",
                seg.index,
                g.s(row, "jd1"),
                g.s(row, "jd2")
            )
        };
        for k in 0..3 {
            let (sp, sv) = roundoff_scale(seg, record, k);
            let (dp, dv) = (
                (f.position[k] - d.position[k].to_f64()).abs(),
                (f.velocity[k] - d.velocity[k].to_f64()).abs(),
            );
            // An all-zero series (199 wrt 1, 299 wrt 2) has scale 0: it must evaluate to exactly 0.
            for (diff, scale) in [(dp, sp), (dv, sv)] {
                if scale > 0.0 {
                    rho.add(diff / scale, at);
                } else {
                    assert_eq!(diff.to_bits(), 0, "zero series at {}", at());
                }
            }
            let c = ["x", "y", "z"][k];
            dd_vs_jpl.add((d.position[k].to_f64() - g.f(row, c)).abs(), at);
        }
    }
    println!(
        "double-double vs jplephem (report only): max |dr| {:.3e} km at {}",
        dd_vs_jpl.value, dd_vs_jpl.at
    );
    check(&v, "EPHEM-F64-DD-ROUNDOFF-RATIO", "f64 vs DD", &rho);
}

/// Verifies: DEP-011, API-001
#[test]
fn typed_states_chain_segments_to_the_barycentre_and_the_sun() {
    let bytes = cached_product("de440-bsp");
    let spk = Spk::parse(&bytes).unwrap();
    let e = ferroastro_core::time::Epoch::<ferroastro_core::time::Tdb>::j2000();
    let seg = |t: i32| spk.segment_for(t, e).unwrap().state(e).unwrap();
    // Mercury (199) wrt SSB = (199 wrt 1) + (1 wrt 0); wrt the Sun = that − (10 wrt 0).
    let bary = spk.state::<Barycentric, f64>(NaifId::MERCURY, e).unwrap();
    let helio = spk.state::<Heliocentric, f64>(NaifId::MERCURY, e).unwrap();
    let (m, b, s) = (seg(199), seg(1), seg(10));
    for k in 0..3 {
        let want = m.position[k] + b.position[k];
        assert_eq!(
            bary.position().components()[k].km().to_bits(),
            want.to_bits()
        );
        let wh = want - s.position[k];
        assert_eq!(
            helio.position().components()[k].km().to_bits(),
            wh.to_bits()
        );
    }
    // Moon wrt Earth goes through the Earth–Moon barycentre, not the SSB.
    let (moon, frame) = spk
        .relative_state_km(NaifId::MOON, NaifId::EARTH, e)
        .unwrap();
    assert_eq!(frame, 1);
    let (lm, le) = (seg(301), seg(399));
    for k in 0..3 {
        assert_eq!(
            moon.position[k].to_bits(),
            (lm.position[k] - le.position[k]).to_bits()
        );
    }
    // Typed states in double-double too.
    let dd = spk
        .state::<Barycentric, DoubleDouble>(NaifId::EARTH, e.cast())
        .unwrap();
    let f = spk.state::<Barycentric, f64>(NaifId::EARTH, e).unwrap();
    for k in 0..3 {
        let a = dd.position().components()[k].km().to_f64();
        let b = f.position().components()[k].km();
        assert!((a - b).abs() <= 4.0 * f64::EPSILON * b.abs(), "{a} vs {b}");
    }
}
