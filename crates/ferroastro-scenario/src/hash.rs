// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The canonical initial-condition hash (CI-002, NUM-013): SHA-256 of the canonicalised ICs plus
//! the constants version.
//!
//! Canonical text, one line each, `\n`-terminated, every number as the 16 lower-case hex digits
//! of the IEEE-754 binary64 bit pattern of its `(hi, lo)` parts (so f64 and double-double ICs
//! hash differently, as they are different ICs):
//!
//! ```text
//! ferroastro-ic-hash v1
//! constants <constants version>
//! mode <n-body | test-particle>
//! epoch_jd_tdb <hi> <lo>
//! body <naif> gm <hi> <lo> r <x.hi> <x.lo> <y.hi> <y.lo> <z.hi> <z.lo> v <…same…>
//! ```
//!
//! Bodies appear in scenario (storage) order; GMs in km³/s², positions in km, velocities in km/s.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// A binary64 pair `(hi, lo)` whose exact sum is one value (`lo = 0` for f64).
pub type Parts = [f64; 2];

/// One body's initial condition, as hashed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IcRecord {
    /// NAIF id.
    pub naif: i32,
    /// GM (km³/s²).
    pub gm: Parts,
    /// Position (km).
    pub position: [Parts; 3],
    /// Velocity (km/s).
    pub velocity: [Parts; 3],
}

fn push_parts(s: &mut String, p: Parts) {
    let _ = write!(s, " {:016x} {:016x}", p[0].to_bits(), p[1].to_bits());
}

/// The canonical text that [`ic_hash`] digests.
#[must_use]
pub fn canonical_ic_text(
    constants_version: &str,
    mode: &str,
    epoch_jd_tdb: Parts,
    bodies: &[IcRecord],
) -> String {
    let mut s = String::from("ferroastro-ic-hash v1\n");
    let _ = writeln!(s, "constants {}", constants_version.replace('\n', " "));
    let _ = writeln!(s, "mode {mode}");
    s.push_str("epoch_jd_tdb");
    push_parts(&mut s, epoch_jd_tdb);
    s.push('\n');
    for b in bodies {
        let _ = write!(s, "body {} gm", b.naif);
        push_parts(&mut s, b.gm);
        s.push_str(" r");
        for p in b.position {
            push_parts(&mut s, p);
        }
        s.push_str(" v");
        for p in b.velocity {
            push_parts(&mut s, p);
        }
        s.push('\n');
    }
    s
}

/// SHA-256 (64 hex digits) of [`canonical_ic_text`].
#[must_use]
pub fn ic_hash(
    constants_version: &str,
    mode: &str,
    epoch_jd_tdb: Parts,
    bodies: &[IcRecord],
) -> String {
    let d =
        Sha256::digest(canonical_ic_text(constants_version, mode, epoch_jd_tdb, bodies).as_bytes());
    d.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
