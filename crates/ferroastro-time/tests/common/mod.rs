// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared loader for the pyerfa golden tables in `tests/goldens/` (computed-reference data,
//! TST-003). Each table is pinned by the SHA-256 recorded in the `notes` of the vector that
//! cites it; a regenerated or edited table fails until the vector is updated (golden change).
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test support: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_vectors::VectorSet;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A parsed golden CSV table.
pub(crate) struct Golden {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
    cols: HashMap<String, usize>,
    pub(crate) rows: Vec<Vec<String>>,
}

impl Golden {
    /// Column `name` of `row` as f64 (exact: the table is written with Python `repr`).
    pub(crate) fn f(&self, row: &[String], name: &str) -> f64 {
        let i = self.cols[name];
        row[i]
            .parse()
            .unwrap_or_else(|e| panic!("{name}={:?}: {e}", row[i]))
    }

    /// Column `name` of `row` as i32.
    pub(crate) fn i(&self, row: &[String], name: &str) -> i32 {
        let i = self.cols[name];
        row[i]
            .parse()
            .unwrap_or_else(|e| panic!("{name}={:?}: {e}", row[i]))
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
    let mut lines = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty());
    let header = lines.next().unwrap();
    let cols = header
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
    let generated_by = text
        .lines()
        .find(|l| l.starts_with("# generated_by:"))
        .unwrap_or("# generated_by: ?");
    let engine = text
        .lines()
        .find(|l| l.starts_with("# engine:"))
        .unwrap_or("# engine: ?");
    println!(
        "golden {file}: {} rows, sha256 {sha256}\n  {generated_by}\n  {engine}",
        rows.len()
    );
    Golden {
        path,
        sha256,
        cols,
        rows,
    }
}
