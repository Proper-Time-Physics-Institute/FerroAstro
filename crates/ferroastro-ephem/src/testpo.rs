// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Parser for JPL `testpo.4xx` test points (the input of JPL's `testeph.f`).
//!
//! After a free-form header ending with a line `EOT`, each line is
//! `de  yyyy.mm.dd  jd  t  c  x  value`: the ephemeris number, the calendar date, the TDB Julian
//! date, the target and centre codes (`testeph` numbering, below), the coordinate index 1–6
//! (x, y, z, ẋ, ẏ, ż) and the value in au or au/day (radians and radians/day for nutations and
//! librations). JPL's `testeph.f` accepts a reader when `|value − computed| < 1e-13` (REF-001).
//!
//! `testeph` codes: 1 Mercury … 9 Pluto (barycentres for 4–9), 10 Moon, 11 Sun, 12 solar-system
//! barycentre, 13 Earth–Moon barycentre, 14 nutations, 15 librations (and higher codes for other
//! non-body quantities). Only codes 1–13 are bodies with SPK states.

use crate::EphemError;
use crate::ephemeris::NaifId;

/// One `testpo` line.
#[derive(Debug, Clone, PartialEq)]
pub struct TestPoint {
    /// 1-based line number in the file.
    pub line: usize,
    /// Ephemeris number (e.g. 440).
    pub de: u32,
    /// The calendar date as written (`yyyy.mm.dd`).
    pub date: String,
    /// TDB Julian date.
    pub jd_tdb: f64,
    /// Target code (`testeph` numbering).
    pub target: u8,
    /// Centre code (`testeph` numbering; 0 for nutations and librations).
    pub center: u8,
    /// Coordinate index 1–6.
    pub coordinate: u8,
    /// Expected value (au, au/day, rad or rad/day).
    pub value: f64,
}

/// The NAIF id of a `testeph` body code 1–13 (`None` for 0 and for non-body codes ≥ 14).
///
/// Codes 1 and 2 map to the Mercury and Venus barycentres, which coincide with the planets in
/// the DE ephemerides; 3 is the Earth, 10 the Moon, 11 the Sun, 12 the barycentre and 13 the
/// Earth–Moon barycentre.
#[must_use]
pub const fn naif_id_of_testeph_code(code: u8) -> Option<NaifId> {
    Some(match code {
        1 => NaifId::MERCURY_BARYCENTER,
        2 => NaifId::VENUS_BARYCENTER,
        3 => NaifId::EARTH,
        4 => NaifId::MARS_BARYCENTER,
        5 => NaifId::JUPITER_BARYCENTER,
        6 => NaifId::SATURN_BARYCENTER,
        7 => NaifId::URANUS_BARYCENTER,
        8 => NaifId::NEPTUNE_BARYCENTER,
        9 => NaifId::PLUTO_BARYCENTER,
        10 => NaifId::MOON,
        11 => NaifId::SUN,
        12 => NaifId::SSB,
        13 => NaifId::EARTH_MOON_BARYCENTER,
        _ => return None,
    })
}

fn bad(line: usize, quantity: &'static str, value: &str, valid: &'static str) -> EphemError {
    EphemError::Testpo {
        line,
        quantity,
        value: value.to_owned(),
        valid,
        fix: "use an unmodified JPL testpo file (verify its SHA-256)",
    }
}

/// Parse a `testpo` file.
///
/// # Errors
/// [`EphemError::Testpo`] (`FA-E1006`) if the `EOT` line is missing or a data line is malformed.
pub fn parse_testpo(text: &str) -> Result<Vec<TestPoint>, EphemError> {
    let mut lines = text.lines().enumerate();
    if !lines.by_ref().any(|(_, l)| l.trim() == "EOT") {
        return Err(bad(
            0,
            "header terminator",
            "",
            "a line \"EOT\" before the data",
        ));
    }
    let mut out = Vec::new();
    for (i, l) in lines {
        let line = i + 1;
        if l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split_whitespace().collect();
        let [de, date, jd, t, c, x, v] = f.as_slice() else {
            return Err(bad(line, "field count", l, "7 whitespace-separated fields"));
        };
        let int = |s: &str, q: &'static str, valid: &'static str| {
            s.parse::<u8>().map_err(|_| bad(line, q, s, valid))
        };
        let coordinate = int(x, "coordinate index", "1..=6")?;
        if !(1..=6).contains(&coordinate) {
            return Err(bad(line, "coordinate index", x, "1..=6"));
        }
        let num = |s: &str, q: &'static str| {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| bad(line, q, s, "a finite decimal number"))
        };
        out.push(TestPoint {
            line,
            de: de
                .parse()
                .map_err(|_| bad(line, "ephemeris number", de, "an unsigned integer"))?,
            date: (*date).to_owned(),
            jd_tdb: num(jd, "Julian date")?,
            target: int(t, "target code", "0..=255")?,
            center: int(c, "centre code", "0..=255")?,
            coordinate,
            value: num(v, "value")?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;

    /// Verifies: REF-001
    #[test]
    fn parses_header_and_points() {
        // Field layout of testpo.440 (values are placeholders, not ephemeris data).
        let text = "KSIZE= 2036\nde#  -- date -- -- jed -- t# c# x# -- coordinate ---\nEOT\n\
                    440  1550.01.01 2287195.5 14  0  1        0.25\n\
                    440  1550.02.01 2287226.5  9 13  5       -0.5\n";
        let p = parse_testpo(text).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!((p[1].target, p[1].center, p[1].coordinate), (9, 13, 5));
        assert_eq!(p[1].line, 5);
        assert_eq!(p[0].date, "1550.01.01");
        assert_eq!(naif_id_of_testeph_code(3), Some(NaifId::EARTH));
        assert_eq!(naif_id_of_testeph_code(14), None);
    }

    /// Verifies: API-002
    #[test]
    fn malformed_lines_are_fa_e1006() {
        for text in [
            "no terminator\n",
            "EOT\n440 1550.01.01 2287195.5 3 12\n",
            "EOT\n440 1550.01.01 2287195.5 3 12 7 0.5\n",
            "EOT\n440 1550.01.01 x 3 12 1 0.5\n",
            "EOT\n440 1550.01.01 2287195.5 3 12 1 nan\n",
        ] {
            let e = parse_testpo(text).unwrap_err();
            assert_eq!(e.code(), ErrorCode::TestpoParse, "{text:?}");
            assert!(e.to_string().contains("FA-E1006"));
        }
    }
}
