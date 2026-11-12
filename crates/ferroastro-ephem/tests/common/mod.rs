// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared test support: golden tables, the cached JPL products, and the synthetic SPK builder.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::suboptimal_flops,
    reason = "test support: a failed unwrap is a test failure with a clear message; the synthetic \
              SPK builder replicates the Python generator's binary64 operation order (no FMA)"
)]

use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_vectors::VectorSet;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// A parsed golden CSV table (`#` lines are the provenance header).
pub(crate) struct Golden {
    pub(crate) sha256: String,
    pub(crate) header: Vec<String>,
    cols: HashMap<String, usize>,
    pub(crate) rows: Vec<Vec<String>>,
}

impl Golden {
    pub(crate) fn f(&self, row: &[String], name: &str) -> f64 {
        let i = self.cols[name];
        row[i]
            .parse()
            .unwrap_or_else(|e| panic!("{name}={:?}: {e}", row[i]))
    }

    pub(crate) fn i(&self, row: &[String], name: &str) -> i32 {
        let i = self.cols[name];
        row[i]
            .parse()
            .unwrap_or_else(|e| panic!("{name}={:?}: {e}", row[i]))
    }

    pub(crate) fn s<'r>(&self, row: &'r [String], name: &str) -> &'r str {
        &row[self.cols[name]]
    }

    /// The value of a `# key: value` header line.
    pub(crate) fn meta(&self, key: &str) -> &str {
        let prefix = format!("{key}: ");
        self.header
            .iter()
            .find_map(|l| l.strip_prefix(&prefix))
            .unwrap_or_else(|| panic!("golden header lacks {key:?}"))
    }

    /// The two-part TDB epoch of a row.
    pub(crate) fn epoch(&self, row: &[String]) -> Epoch<Tdb> {
        Epoch::from_jd_parts(self.f(row, "jd1"), self.f(row, "jd2")).unwrap()
    }
}

pub(crate) fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

/// Load `tests/goldens/<file>` and check its SHA-256 against the `notes` of vector `pin_id`.
pub(crate) fn load(file: &str, v: &VectorSet, pin_id: &str) -> Golden {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens")
        .join(file);
    let bytes = std::fs::read(&path).unwrap();
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let notes = v.get(pin_id).unwrap().notes.clone().unwrap_or_default();
    assert!(
        notes.contains(&sha256),
        "{} has sha256 {sha256}, but vector {pin_id} pins {notes:?}: a changed golden needs a \
         golden-change PR that updates the vector",
        path.display()
    );
    let text = String::from_utf8(bytes).unwrap();
    let header: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_prefix("# "))
        .map(str::to_owned)
        .collect();
    let mut lines = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty());
    let cols = lines
        .next()
        .unwrap()
        .split(',')
        .enumerate()
        .map(|(i, c)| (c.to_owned(), i))
        .collect::<HashMap<_, _>>();
    let rows: Vec<Vec<String>> = lines
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect();
    for r in &rows {
        assert_eq!(r.len(), cols.len(), "ragged row in {file}");
    }
    let g = Golden {
        sha256,
        header,
        cols,
        rows,
    };
    println!(
        "golden {file}: {} rows, sha256 {}\n  generated_by: {}\n  engine: {}",
        g.rows.len(),
        g.sha256,
        g.meta("generated_by"),
        g.meta("engine")
    );
    g
}

/// The bytes of a pinned JPL product from the local cache (offline; re-verified SHA-256). A
/// missing product fails the test loudly with the `fetch_jpl.py` command (CI-005: the PR tier
/// reads a cache that CI fills; it never skips).
pub(crate) fn cached_product(name: &str) -> Vec<u8> {
    let f = ferroastro_testdata::locate_product(name).unwrap_or_else(|e| panic!("{e}"));
    println!("{name}: {}", f.provenance());
    std::fs::read(&f.path).unwrap()
}

// ----- synthetic SPK (mirrors tools/goldens/jplephem_de440.py: SYNTH_SEGMENTS, coefficient,
// synthetic_spk) -----

/// `(target, center, type, ncoef, n_records, init_days_from_j2000, intlen_days)`.
pub(crate) const SYNTH_SEGMENTS: [(i32, i32, i32, usize, usize, i32, i32); 2] = [
    (SYNTH_A.0, 0, 2, 5, 4, -8, 4),
    (SYNTH_B.0, SYNTH_A.0, 3, 4, 3, -6, 4),
];
/// Synthetic NAIF ids (not real bodies), mirrored from the Python generator.
pub(crate) const SYNTH_A: ferroastro_ephem::NaifId = ferroastro_ephem::NaifId(1001); // lint: allow-literal(synthetic NAIF id)
/// The type-3 synthetic body, defined relative to [`SYNTH_A`].
pub(crate) const SYNTH_B: ferroastro_ephem::NaifId = ferroastro_ephem::NaifId(1002); // lint: allow-literal(synthetic NAIF id)
const SYNTH_IFN: &[u8] = b"FERROASTRO SYNTHETIC SPK (WP10)";
const DAY: f64 = ferroastro_ephem::SECONDS_PER_DAY;
const REC: usize = ferroastro_ephem::RECORD_BYTES;

/// The synthetic coefficient: a small integer times 1024 / 2^k (exact in binary64).
pub(crate) fn coefficient(seg: usize, rec: usize, comp: usize, k: usize) -> f64 {
    let m = i32::try_from((seg * 1000 + rec * 100 + comp * 10 + k * 7) % 17).unwrap() - 8;
    f64::from(m) * 1024.0 / f64::from(1u32 << k) // lint: allow-literal(dyadic synthetic scale)
}

/// Build the synthetic SPK in little (`big = false`) or big-endian byte order.
pub(crate) fn synthetic_spk(big: bool) -> Vec<u8> {
    let d = |x: f64| {
        if big {
            x.to_be_bytes()
        } else {
            x.to_le_bytes()
        }
    };
    let i = |x: i32| {
        if big {
            x.to_be_bytes()
        } else {
            x.to_le_bytes()
        }
    };
    let mut data: Vec<f64> = Vec::new();
    let mut summaries: Vec<Vec<u8>> = Vec::new();
    let mut names: Vec<Vec<u8>> = Vec::new();
    let mut address: i32 = 3 * 128 + 1;
    for (si, &(target, center, dtype, ncoef, n, init_d, intlen_d)) in
        SYNTH_SEGMENTS.iter().enumerate()
    {
        let ncomp = if dtype == 2 { 3 } else { 6 };
        let (init, intlen) = (f64::from(init_d) * DAY, f64::from(intlen_d) * DAY);
        let rsize = 2 + ncomp * ncoef;
        let mut words = Vec::new();
        for r in 0..n {
            words.push(init + (f64::from(u32::try_from(r).unwrap()) + 0.5) * intlen);
            words.push(intlen / 2.0);
            for c in 0..ncomp {
                for k in 0..ncoef {
                    words.push(coefficient(si, r, c, k));
                }
            }
        }
        let nf = f64::from(u32::try_from(n).unwrap());
        words.extend([init, intlen, f64::from(u32::try_from(rsize).unwrap()), nf]);
        let first = address;
        let last = address + i32::try_from(words.len()).unwrap() - 1;
        let mut s = Vec::new();
        s.extend(d(init));
        s.extend(d(init + nf * intlen));
        for x in [target, center, 1, dtype, first, last] {
            s.extend(i(x));
        }
        summaries.push(s);
        let mut nm = format!("SYNTHETIC TYPE {dtype} SEGMENT {si}").into_bytes();
        nm.resize(40, b' ');
        names.push(nm);
        data.extend(words);
        address = last + 1;
    }
    let mut rec1 = vec![0u8; REC];
    rec1[..8].copy_from_slice(b"DAF/SPK ");
    rec1[8..12].copy_from_slice(&i(2));
    rec1[12..16].copy_from_slice(&i(6));
    let mut ifn = SYNTH_IFN.to_vec();
    ifn.resize(60, b' ');
    rec1[16..76].copy_from_slice(&ifn);
    rec1[76..80].copy_from_slice(&i(2));
    rec1[80..84].copy_from_slice(&i(2));
    rec1[84..88].copy_from_slice(&i(address));
    rec1[88..96].copy_from_slice(if big { b"BIG-IEEE" } else { b"LTL-IEEE" });
    rec1[699..699 + 28].copy_from_slice(ferroastro_ephem::FTPSTR);
    let mut rec2 = vec![0u8; REC];
    rec2[..8].copy_from_slice(&d(0.0));
    rec2[8..16].copy_from_slice(&d(0.0));
    rec2[16..24].copy_from_slice(&d(f64::from(u32::try_from(summaries.len()).unwrap())));
    for (k, s) in summaries.iter().enumerate() {
        rec2[24 + 40 * k..24 + 40 * (k + 1)].copy_from_slice(s);
    }
    let mut rec3 = vec![0u8; REC];
    for (k, nm) in names.iter().enumerate() {
        rec3[40 * k..40 * (k + 1)].copy_from_slice(nm);
    }
    let mut out = [rec1, rec2, rec3].concat();
    for x in data {
        out.extend(d(x));
    }
    out.resize(out.len().div_ceil(REC) * REC, 0);
    out
}

/// Lowercase hex SHA-256.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Track the largest value and where it occurred.
#[derive(Default, Debug)]
pub(crate) struct Max {
    pub(crate) value: f64,
    pub(crate) at: String,
}

impl Max {
    pub(crate) fn add(&mut self, x: f64, at: impl Fn() -> String) {
        assert!(x.is_finite(), "non-finite difference at {}", at());
        if x > self.value {
            self.value = x;
            self.at = at();
        }
    }
}

/// Check `m` against vector `id`, printing the measured value and margin.
pub(crate) fn check(v: &VectorSet, id: &str, label: &str, m: &Max) {
    let vector = v.get(id).unwrap();
    let allowed = vector.tolerance.allowed(vector.value().unwrap());
    let margin = if allowed > 0.0 {
        format!("{:.3}", m.value / allowed)
    } else if m.value == 0.0 {
        "exact".to_owned()
    } else {
        "inf".to_owned()
    };
    println!(
        "{id:<38} {label:<14} max |diff| {:.3e} {} (margin {margin}; at {})",
        m.value, vector.unit, m.at
    );
    if let Err(e) = vector.check(m.value) {
        panic!("{label}: {e}");
    }
}
