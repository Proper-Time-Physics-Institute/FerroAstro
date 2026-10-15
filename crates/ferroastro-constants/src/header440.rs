// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Parser for the JPL ASCII ephemeris header `header.4xx` (REF-002).
//!
//! The header is a sequence of `GROUP   <n>` blocks. This module reads:
//!
//! * GROUP 1010: the title lines (e.g. `JPL Planetary Ephemeris DE440/LE440`);
//! * GROUP 1040: the number of constants `N` followed by `N` names;
//! * GROUP 1041: the number `N` again followed by `N` values in Fortran `D`-exponent notation.
//!
//! Every value is kept twice: the verbatim token (so a future double-double consumer can parse all
//! 18 printed digits, NUM-006) and the nearest `f64`. Units are **not** stated in the header; the
//! constants table ([`crate::table`]) attaches them with a citation per entry.
//!
//! ```
//! use ferroastro_constants::header440::Header;
//! # fn main() -> Result<(), ferroastro_constants::ConstantsError> {
//! let text = "GROUP   1010\n\nDEmo\n\nGROUP   1040\n\n 2\n  AA  BB\n\
//!             GROUP   1041\n\n 2\n  0.150000000000000000D+01  -0.200000000000000000D-02\n";
//! let h = Header::parse(text)?;
//! assert_eq!(h.len(), 2);
//! assert_eq!(h.value("BB")?, -2.0e-3);
//! assert_eq!(h.token("AA")?, "0.150000000000000000D+01");
//! # Ok(())
//! # }
//! ```

use crate::ConstantsError;
use std::collections::BTreeMap;

/// One constant of GROUP 1040/1041.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderConstant {
    /// Name from GROUP 1040, e.g. `"GMS"`.
    pub name: String,
    /// Value token from GROUP 1041, verbatim (Fortran `D` exponent).
    pub token: String,
    /// The token parsed to the nearest `f64`.
    pub value: f64,
}

/// The parsed constants of a `header.4xx` file, in header order.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    title: Vec<String>,
    constants: Vec<HeaderConstant>,
    index: BTreeMap<String, usize>,
}

fn parse_err(problem: impl Into<String>) -> ConstantsError {
    ConstantsError::HeaderParse {
        problem: problem.into(),
    }
}

/// Split the text into `(group number, body)` pairs. A group starts at a line whose first
/// whitespace-separated token is `GROUP` followed by an integer.
fn groups(text: &str) -> Result<BTreeMap<u32, String>, ConstantsError> {
    let mut out: BTreeMap<u32, String> = BTreeMap::new();
    let mut current: Option<u32> = None;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if words.next() == Some("GROUP") {
            let n = words
                .next()
                .ok_or_else(|| parse_err("a GROUP line has no number"))?
                .parse::<u32>()
                .map_err(|e| parse_err(format!("bad GROUP number in {line:?}: {e}")))?;
            if out.insert(n, String::new()).is_some() {
                return Err(parse_err(format!("GROUP {n} appears twice")));
            }
            current = Some(n);
        } else if let Some(n) = current {
            if let Some(body) = out.get_mut(&n) {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    Ok(out)
}

/// Parse a Fortran real such as `0.295912208284119561D-03` (also accepts `E`/`e`/`d`).
///
/// # Errors
/// [`ConstantsError::HeaderParse`] if the token is not a finite real number.
pub fn parse_fortran_real(token: &str) -> Result<f64, ConstantsError> {
    let v = token
        .replace(['D', 'd'], "E")
        .parse::<f64>()
        .map_err(|e| parse_err(format!("value token {token:?} is not a number: {e}")))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(parse_err(format!("value token {token:?} is not finite")))
    }
}

/// Read the leading count of GROUP 1040/1041 and the tokens after it.
fn counted(groups: &BTreeMap<u32, String>, n: u32) -> Result<(usize, Vec<&str>), ConstantsError> {
    let body = groups
        .get(&n)
        .ok_or_else(|| parse_err(format!("GROUP {n} is missing")))?;
    let mut words = body.split_whitespace();
    let count = words
        .next()
        .ok_or_else(|| parse_err(format!("GROUP {n} is empty")))?
        .parse::<usize>()
        .map_err(|e| parse_err(format!("GROUP {n} count is not an integer: {e}")))?;
    let tokens: Vec<&str> = words.collect();
    if tokens.len() != count {
        return Err(parse_err(format!(
            "GROUP {n} declares {count} entries but contains {}",
            tokens.len()
        )));
    }
    Ok((count, tokens))
}

impl Header {
    /// Parse the text of a `header.4xx` file.
    ///
    /// # Errors
    /// [`ConstantsError::HeaderParse`] if GROUP 1040 or 1041 is missing, their counts disagree
    /// with each other or with the number of entries, a name repeats, or a value is not a finite
    /// number.
    pub fn parse(text: &str) -> Result<Self, ConstantsError> {
        let groups = groups(text)?;
        let title = groups
            .get(&1010)
            .map(|b| {
                b.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let (n_names, names) = counted(&groups, 1040)?;
        let (n_values, values) = counted(&groups, 1041)?;
        if n_names != n_values {
            return Err(parse_err(format!(
                "GROUP 1040 declares {n_names} names but GROUP 1041 declares {n_values} values"
            )));
        }
        let mut constants = Vec::with_capacity(n_names);
        let mut index = BTreeMap::new();
        for (i, (name, token)) in names.into_iter().zip(values).enumerate() {
            if index.insert(name.to_owned(), i).is_some() {
                return Err(parse_err(format!(
                    "constant name {name:?} repeats in GROUP 1040"
                )));
            }
            constants.push(HeaderConstant {
                name: name.to_owned(),
                token: token.to_owned(),
                value: parse_fortran_real(token)?,
            });
        }
        Ok(Self {
            title,
            constants,
            index,
        })
    }

    /// The non-empty title lines of GROUP 1010.
    #[must_use]
    pub fn title(&self) -> &[String] {
        &self.title
    }

    /// Number of constants (GROUP 1040 count).
    #[must_use]
    pub fn len(&self) -> usize {
        self.constants.len()
    }

    /// True if the header declares no constants.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.constants.is_empty()
    }

    /// All constants in header order.
    #[must_use]
    pub fn constants(&self) -> &[HeaderConstant] {
        &self.constants
    }

    /// Look up one constant by its GROUP 1040 name.
    ///
    /// # Errors
    /// [`ConstantsError::ConstantMissing`] if the name is absent.
    pub fn get(&self, name: &str) -> Result<&HeaderConstant, ConstantsError> {
        self.index
            .get(name)
            .and_then(|&i| self.constants.get(i))
            .ok_or_else(|| ConstantsError::ConstantMissing {
                name: name.to_owned(),
                source_name: "header.440",
            })
    }

    /// The `f64` value of a constant.
    ///
    /// # Errors
    /// [`ConstantsError::ConstantMissing`] if the name is absent.
    pub fn value(&self, name: &str) -> Result<f64, ConstantsError> {
        self.get(name).map(|c| c.value)
    }

    /// The verbatim value token of a constant.
    ///
    /// # Errors
    /// [`ConstantsError::ConstantMissing`] if the name is absent.
    pub fn token(&self, name: &str) -> Result<&str, ConstantsError> {
        self.get(name).map(|c| c.token.as_str())
    }

    /// Number of constants whose name starts with `prefix` (e.g. `"MA"` for asteroid GMs).
    #[must_use]
    pub fn count_prefix(&self, prefix: &str) -> usize {
        self.constants
            .iter()
            .filter(|c| c.name.starts_with(prefix))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;

    // Synthetic headers below are structural fixtures, not physics: names and tokens are strings.
    const OK: &str = "KSIZE= 8    NCOEFF= 4\n \nGROUP   1010\n \nTest Ephemeris\nStart Epoch\n \n\
        GROUP   1040\n \n     3\n  AA      BB      MA0001\n \n\
        GROUP   1041\n \n     3\n  0.100000000000000000D+01  -0.250000000000000000D+00\n  \
        0.500000000000000000d-01\n \nGROUP   1050\n \n     3   171\n";

    /// Verifies: REF-002
    #[test]
    fn parses_names_tokens_values_and_title() {
        let h = Header::parse(OK).unwrap();
        assert_eq!(h.len(), 3);
        assert!(!h.is_empty());
        assert_eq!(h.title(), ["Test Ephemeris", "Start Epoch"]);
        assert_eq!(h.token("BB").unwrap(), "-0.250000000000000000D+00");
        assert_eq!(
            h.value("BB").unwrap().to_bits(),
            (-0.25_f64).to_bits(),
            "D exponent parsed"
        );
        assert_eq!(h.value("AA").unwrap().to_bits(), 1.0_f64.to_bits());
        assert_eq!(
            h.value("MA0001").unwrap().to_bits(),
            "0.05".parse::<f64>().unwrap().to_bits(),
            "lower-case d exponent parsed"
        );
        assert_eq!(h.count_prefix("MA"), 1);
        let names: Vec<&str> = h.constants().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["AA", "BB", "MA0001"]);
    }

    /// Verifies: REF-002
    #[test]
    fn missing_name_is_fa_e0301() {
        let h = Header::parse(OK).unwrap();
        let err = h.get("CC").unwrap_err();
        assert_eq!(err.code(), ErrorCode::ConstantMissing);
        assert!(err.to_string().contains("FA-E0301"));
    }

    /// Verifies: REF-002
    #[test]
    fn malformed_headers_are_fa_e0300() {
        let cases = [
            ("no 1041", OK.replace("GROUP   1041", "GROUP   1042")),
            (
                "count mismatch",
                OK.replace("     3\n  0.1", "     2\n  0.1"),
            ),
            (
                "short value list",
                OK.replace("  0.500000000000000000d-01\n", ""),
            ),
            ("duplicate name", OK.replace("BB ", "AA ")),
            (
                "bad token",
                OK.replace("-0.250000000000000000D+00", "-0.25X"),
            ),
            ("non-finite", OK.replace("-0.250000000000000000D+00", "inf")),
            ("duplicate group", format!("{OK}GROUP   1040\n 0\n")),
            ("bad group number", OK.replace("GROUP   1050", "GROUP   x")),
        ];
        for (what, text) in cases {
            let err = Header::parse(&text).unwrap_err();
            assert_eq!(err.code(), ErrorCode::HeaderParse, "{what}: {err}");
        }
    }
}
