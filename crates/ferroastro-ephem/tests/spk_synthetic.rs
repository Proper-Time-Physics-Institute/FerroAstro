// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-011 / REF-004: SPK type 2 **and type 3**, in **both byte orders**, against jplephem 2.24.
//!
//! The synthetic SPK (dyadic coefficients, no JPL data) is written by
//! `tools/goldens/jplephem_de440.py` and evaluated there with jplephem; this test rebuilds the same
//! bytes (their SHA-256 must equal the digests in the golden header, so both sides read the very
//! same file) and evaluates them with FerroAstro. Offline: PR tier.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{Max, SYNTH_A, SYNTH_B, check, load, sha256, synthetic_spk, vectors};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_ephem::{Endian, Ephemeris, NaifId, Spk, SpkType};
use ferroastro_real::{DoubleDouble, Real};

/// Verifies: DEP-011, REF-004
#[test]
fn synthetic_type2_and_type3_both_byte_orders_match_jplephem() {
    let v = vectors();
    let g = load(
        "jplephem-synthetic.csv",
        &v,
        "SPK-SYNTHETIC-JPLEPHEM-POS-MAX-DIFF",
    );
    let files = [("LTL", synthetic_spk(false)), ("BIG", synthetic_spk(true))];
    for (tag, bytes) in &files {
        assert_eq!(
            sha256(bytes),
            g.meta(&format!("synthetic_sha256_{tag}")),
            "{tag}: the Rust builder and the Python generator must produce identical bytes"
        );
    }
    let (mut pos, mut vel) = (Max::default(), Max::default());
    let mut per_type = [0usize; 2];
    for (tag, bytes) in &files {
        let spk = Spk::parse(bytes).unwrap();
        let want = if *tag == "BIG" {
            Endian::Big
        } else {
            Endian::Little
        };
        assert_eq!(spk.daf().endian(), want);
        assert_eq!(spk.segments().len(), 2);
        assert_eq!(spk.segments()[0].data_type, SpkType::Chebyshev2);
        assert_eq!(spk.segments()[1].data_type, SpkType::Chebyshev3);
        for row in g.rows.iter().filter(|r| g.s(r, "kind") == *tag) {
            let seg = &spk.segments()[usize::try_from(g.i(row, "seg")).unwrap()];
            assert_eq!(seg.target, g.i(row, "target"));
            assert_eq!(seg.center, g.i(row, "center"));
            let s = seg.state(g.epoch(row)).unwrap();
            let at = || {
                format!(
                    "{tag} seg {} jd {}+{}",
                    seg.index,
                    g.s(row, "jd1"),
                    g.s(row, "jd2")
                )
            };
            for (k, c) in ["x", "y", "z"].iter().enumerate() {
                pos.add((s.position[k] - g.f(row, c)).abs(), at);
            }
            for (k, c) in ["vx", "vy", "vz"].iter().enumerate() {
                vel.add((s.velocity[k] - g.f(row, c)).abs(), at);
            }
            per_type[seg.index] += 1;
        }
    }
    println!(
        "rows evaluated: type 2 {}, type 3 {}",
        per_type[0], per_type[1]
    );
    assert!(per_type.iter().all(|&n| n >= 20), "{per_type:?}");
    check(&v, "SPK-SYNTHETIC-JPLEPHEM-POS-MAX-DIFF", "f64", &pos);
    check(&v, "SPK-SYNTHETIC-JPLEPHEM-VEL-MAX-DIFF", "f64", &vel);
}

/// Verifies: DEP-011, DEP-007
#[test]
fn byte_orders_agree_bitwise_and_chain_sums_segments() {
    let (le, be) = (synthetic_spk(false), synthetic_spk(true));
    let (a, b) = (Spk::parse(&le).unwrap(), Spk::parse(&be).unwrap());
    // 1002 is defined relative to 1001, which is relative to the barycentre.
    let e = Epoch::<Tdb>::from_jd_parts(2_451_545.0 - 2.0, 0.25).unwrap(); // lint: allow-literal(synthetic epoch inside both segments)
    let (sa, fa) = a.relative_state_km(SYNTH_B, NaifId::SSB, e).unwrap();
    let (sb, fb) = b.relative_state_km(SYNTH_B, NaifId::SSB, e).unwrap();
    assert_eq!((fa, fb), (1, 1));
    for k in 0..3 {
        assert_eq!(sa.position[k].to_bits(), sb.position[k].to_bits());
        assert_eq!(sa.velocity[k].to_bits(), sb.velocity[k].to_bits());
    }
    let s1 = a.segments()[0].state(e).unwrap();
    let s2 = a.segments()[1].state(e).unwrap();
    for k in 0..3 {
        assert_eq!(
            sa.position[k].to_bits(),
            (s2.position[k] + s1.position[k]).to_bits()
        );
    }
    // The reverse direction is the negation; 1001 relative to 1002 stops at their common node.
    let (r, _) = a.relative_state_km(NaifId::SSB, SYNTH_B, e).unwrap();
    let (m, _) = a.relative_state_km(SYNTH_A, SYNTH_B, e).unwrap();
    for k in 0..3 {
        assert_eq!(r.position[k].to_bits(), (-sa.position[k]).to_bits());
        assert_eq!(m.position[k].to_bits(), (-s2.position[k]).to_bits());
    }
    // Double-double on dyadic coefficients and a dyadic s agrees with f64 (both exact here).
    let edd = e.cast::<DoubleDouble>();
    let dd = a.segments()[1].state(edd).unwrap();
    for k in 0..3 {
        let x = dd.position[k].to_f64();
        assert!((x - s2.position[k]).abs() <= f64::EPSILON * s2.position[k].abs());
    }
}
