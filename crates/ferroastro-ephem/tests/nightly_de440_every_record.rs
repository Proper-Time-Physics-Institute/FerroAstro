// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Nightly tier (`#[ignore]`, nextest `nightly` profile, release build): DEP-007 f64 vs
//! double-double on **every record** of every de440.bsp segment, at three points per record,
//! against the same pre-registered round-off ratio as the PR-tier golden epochs.
//!
//! Reads the cached de440.bsp (pinned SHA-256); a missing cache fails loudly.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{Max, cached_product, check, vectors};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_ephem::{J2000_JD, SECONDS_PER_DAY, Spk};
use ferroastro_real::{DoubleDouble, Real};

/// Points inside each record, as fractions of its interval.
const FRACTIONS: [f64; 3] = [0.125, 0.5, 0.9375]; // lint: allow-literal(sample points in a record)

/// Verifies: DEP-007, DEP-011
#[test]
#[ignore = "nightly tier: ~1.2 million f64 and double-double evaluations"]
fn nightly_de440_double_double_vs_f64_every_record() {
    let v = vectors();
    let bytes = cached_product("de440-bsp");
    let spk = Spk::parse(&bytes).unwrap();
    let u = f64::EPSILON / 2.0;
    let (mut rho, mut n) = (Max::default(), 0usize);
    for seg in spk.segments() {
        let l = seg.layout.unwrap();
        for rec in 0..l.n {
            let (mid, radius) = seg.record_mid_radius(rec).unwrap();
            let start_days = (mid - radius) / SECONDS_PER_DAY;
            // Two-part date: the record start (an exact half-day multiple) + a fraction.
            let (jd1, base) = (J2000_JD + start_days, 2.0 * radius / SECONDS_PER_DAY);
            for f in FRACTIONS {
                let e = Epoch::<Tdb>::from_jd_parts(jd1, f * base).unwrap();
                let a = seg.state(e).unwrap();
                let b = seg.state(e.cast::<DoubleDouble>()).unwrap();
                for k in 0..3 {
                    let c = seg.coefficients(rec, k).unwrap();
                    let nc = f64::from(u32::try_from(c.len()).unwrap());
                    let sp: f64 = c.iter().map(|x| x.abs()).sum::<f64>() * nc * u;
                    let sv: f64 = c
                        .iter()
                        .enumerate()
                        .map(|(i, x)| f64::from(u32::try_from(i * i).unwrap()) * x.abs())
                        .sum::<f64>()
                        * nc
                        * u
                        * 2.0
                        / l.intlen;
                    let dp = (a.position[k] - b.position[k].to_f64()).abs();
                    let dv = (a.velocity[k] - b.velocity[k].to_f64()).abs();
                    let at = || format!("seg {} rec {rec} f {f} comp {k}", seg.index);
                    for (d, s) in [(dp, sp), (dv, sv)] {
                        if s > 0.0 {
                            rho.add(d / s, at);
                        } else {
                            assert_eq!(d.to_bits(), 0, "{}", at());
                        }
                    }
                }
                n += 1;
            }
        }
    }
    println!("{n} evaluations (f64 and double-double) over every record of every segment");
    check(&v, "EPHEM-F64-DD-ROUNDOFF-RATIO", "f64 vs DD", &rho);
}
