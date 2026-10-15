// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! State-vector goldens (stage A and later): the plain CSV the N-body oracle runners write.
//!
//! Format: `#`-prefixed `key: value` header lines (engine, `header_sha256`, `epoch_jd_tdb`,
//! `au_km`, units), then the column line [`STATE_COLUMNS`], then one row per body and output
//! time: TDB days since the epoch, NAIF id, SSB-centred ICRF position (km) and velocity (km/s).
//! Numbers are shortest round-trip decimals, so parsing recovers the oracle's binary64 values.

use crate::OracleError;
use std::collections::BTreeMap;

/// The column line of a state golden.
pub const STATE_COLUMNS: &str = "t_days,naif,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s";

/// One body's state at one output time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoldenState {
    /// TDB days since the epoch.
    pub t_days: f64,
    /// NAIF id.
    pub naif: i32,
    /// Position (km), SSB-centred, ICRF axes.
    pub position_km: [f64; 3],
    /// Velocity (km/s).
    pub velocity_km_s: [f64; 3],
}

/// A parsed state golden.
#[derive(Debug, Clone, PartialEq)]
pub struct GoldenStates {
    /// File name (for messages).
    pub file: String,
    /// Header `key: value` lines.
    pub meta: BTreeMap<String, String>,
    /// Rows in file order.
    pub rows: Vec<GoldenState>,
}

impl GoldenStates {
    /// Parse a state golden.
    ///
    /// # Errors
    /// `FA-E1502` for a missing or different column line, a malformed row or a non-finite value.
    pub fn parse(file: &str, text: &str) -> Result<Self, OracleError> {
        let bad = |problem: String| OracleError::Golden {
            file: file.to_owned(),
            problem,
        };
        let mut meta = BTreeMap::new();
        let mut rows = Vec::new();
        let mut columns = false;
        for (i, line) in text.lines().enumerate() {
            if let Some(c) = line.strip_prefix('#') {
                if let Some((k, v)) = c.split_once(':') {
                    meta.insert(k.trim().to_owned(), v.trim().to_owned());
                }
                continue;
            }
            if !columns {
                if line.trim() != STATE_COLUMNS {
                    return Err(bad(format!("column line {line:?}, want {STATE_COLUMNS:?}")));
                }
                columns = true;
                continue;
            }
            let f: Vec<&str> = line.split(',').collect();
            let n = f.len();
            let [t, naif, x, y, z, vx, vy, vz]: [&str; 8] = f
                .try_into()
                .map_err(|_| bad(format!("line {}: {n} fields, want 8", i + 1)))?;
            let num = |field: &str| -> Result<f64, OracleError> {
                let v: f64 = field
                    .trim()
                    .parse()
                    .map_err(|e| bad(format!("line {}: field {field:?}: {e}", i + 1)))?;
                if v.is_finite() {
                    Ok(v)
                } else {
                    Err(bad(format!(
                        "line {}: field {field:?} is not finite",
                        i + 1
                    )))
                }
            };
            let naif: i32 = naif
                .trim()
                .parse()
                .map_err(|e| bad(format!("line {}: naif {naif:?}: {e}", i + 1)))?;
            rows.push(GoldenState {
                t_days: num(t)?,
                naif,
                position_km: [num(x)?, num(y)?, num(z)?],
                velocity_km_s: [num(vx)?, num(vy)?, num(vz)?],
            });
        }
        if !columns || rows.is_empty() {
            return Err(bad("no column line or no rows".into()));
        }
        Ok(Self {
            file: file.to_owned(),
            meta,
            rows,
        })
    }

    /// A numeric header value (`epoch_jd_tdb`, `au_km`).
    ///
    /// # Errors
    /// `FA-E1502` if the key is absent or not a finite number.
    pub fn meta_f64(&self, key: &str) -> Result<f64, OracleError> {
        self.meta
            .get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| OracleError::Golden {
                file: self.file.clone(),
                problem: format!("header line `# {key}: <number>` missing or invalid"),
            })
    }

    /// The distinct output times (days), in file order.
    #[must_use]
    pub fn times(&self) -> Vec<f64> {
        let mut t: Vec<f64> = Vec::new();
        for r in &self.rows {
            if t.last().is_none_or(|x| x.to_bits() != r.t_days.to_bits()) {
                t.push(r.t_days);
            }
        }
        t
    }
}
