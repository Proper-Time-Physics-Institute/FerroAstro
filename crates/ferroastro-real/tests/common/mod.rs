// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Minimal reader of `tests/vectors/real-backends.toml` for this crate's tests.
//!
//! `ferroastro-vectors` cannot be used here (it depends on `ferroastro-core`, which depends on
//! `ferroastro-real`: a dev-edge cycle). This reader applies the same ADR-0004 rule — a measured
//! value `x` passes when `|x − value| ≤ max(abs, rel·|value|)` — always to the committed file
//! (there is no directory override), and the full schema is still validated by
//! `ferroastro-vectors` and `tools/trace.py --check`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    unreachable_pub,
    reason = "test helper: a missing or malformed vector is a test failure"
)]

/// One vector's value, tolerance and unit.
pub struct Bound {
    pub id: String,
    pub value: f64,
    pub abs: f64,
    pub rel: f64,
    pub unit: String,
}

impl Bound {
    /// Look up `id` in `tests/vectors/real-backends.toml`.
    pub fn load(id: &str) -> Self {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/vectors/real-backends.toml"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let doc: toml::Table = toml::from_str(&text).unwrap();
        let vectors = doc["vector"].as_array().unwrap();
        let v = vectors
            .iter()
            .find(|v| v["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("no vector {id} in {path}"));
        let num = |x: &toml::Value| {
            x.as_float().or_else(|| {
                x.as_integer()
                    .and_then(|i| i32::try_from(i).ok())
                    .map(f64::from)
            })
        };
        let tol = v["tolerance"].as_table().unwrap();
        Self {
            id: id.to_owned(),
            value: num(&v["value"]).unwrap(),
            abs: tol.get("abs").and_then(num).unwrap_or(0.0),
            rel: tol.get("rel").and_then(num).unwrap_or(0.0),
            unit: v["unit"].as_str().unwrap().to_owned(),
        }
    }

    /// The allowed absolute deviation, `max(abs, rel·|value|)`.
    pub fn allowed(&self) -> f64 {
        self.abs.max(self.rel * self.value.abs())
    }

    /// Assert that `measured` is within the tolerance.
    pub fn check(&self, measured: f64) {
        let ok = (measured - self.value).abs() <= self.allowed();
        assert!(
            ok,
            "{}: measured {measured} {}, expected {} within {}",
            self.id,
            self.unit,
            self.value,
            self.allowed()
        );
    }
}
