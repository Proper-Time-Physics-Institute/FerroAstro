// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! A cached, chunked, rate-limited client for JPL Horizons state-vector tables (PHY-016).
//!
//! The Horizons API (<https://ssd-api.jpl.nasa.gov/doc/horizons.html>) refuses an answer longer
//! than 90 024 lines ("Projected output length (~91313) exceeds 90024 -- change step-size"), so a
//! long span is split into chunks of at most [`MAX_ROWS_PER_REQUEST`] rows. Chunks are fetched
//! **serially**, with a [`POLITE_PAUSE`] before every network request (Horizons documents no rate
//! limit), and each answer is cached under a name derived from the full query, so a second run
//! makes no request at all. Answers are never committed (JPL redistribution terms are an open
//! item); only the numbers derived from them are reported.
//!
//! Every chunk keeps its provenance: the URL, the retrieval date and SHA-256 (from
//! [`crate::fetch_url`]), the API `signature.version`, the ephemeris source strings Horizons
//! prints for the target and centre (e.g. `DE441`), the reference-frame line, the time scale and
//! the output units. The parser rejects an answer whose header does not state TDB, `KM-S`,
//! geometric states and the expected frame, so a silent change of the service shows up as
//! `FA-E0203` instead of as a wrong number.

use crate::{DataError, Fetched, data_dir, fetch_url};
use std::fmt::Write as _;
use std::time::Duration;

/// Base URL of the Horizons API.
pub const API: &str = "https://ssd.jpl.nasa.gov/api/horizons.api";

/// Largest number of table rows requested at once (Horizons' limit is 90 024 output lines,
/// header included).
pub const MAX_ROWS_PER_REQUEST: u64 = 80_000;

/// Pause before every network request to Horizons.
pub const POLITE_PAUSE: Duration = Duration::from_secs(1);

/// The Horizons `REF_PLANE` of a vector table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefPlane {
    /// `FRAME`: the axes of `REF_SYSTEM` itself (ICRF equator and equinox).
    Frame,
    /// `ECLIPTIC`: Horizons' "Ecliptic of J2000.0".
    Ecliptic,
    /// `BODY EQUATOR`: the centre body's equator (e.g. `IAU_SUN`).
    BodyEquator,
}

impl RefPlane {
    /// The value of the `REF_PLANE` parameter.
    #[must_use]
    pub const fn param(self) -> &'static str {
        match self {
            RefPlane::Frame => "FRAME",
            RefPlane::Ecliptic => "ECLIPTIC",
            RefPlane::BodyEquator => "BODY EQUATOR",
        }
    }

    /// Text the answer's `Reference frame :` line must contain.
    const fn frame_marker(self) -> &'static str {
        match self {
            RefPlane::Frame => "ICRF",
            RefPlane::Ecliptic => "Ecliptic of J2000.0",
            RefPlane::BodyEquator => "body equator",
        }
    }

    const fn tag(self) -> &'static str {
        match self {
            RefPlane::Frame => "icrf",
            RefPlane::Ecliptic => "eclj2000",
            RefPlane::BodyEquator => "bodyeq",
        }
    }
}

/// A geometric state-vector table request (`EPHEM_TYPE='VECTORS'`, `VEC_TABLE='2'`, TDB, km and
/// km/s, `REF_SYSTEM='ICRF'`).
#[derive(Debug, Clone, PartialEq)]
pub struct VectorQuery {
    /// Target (`COMMAND`), e.g. `"199"` for Mercury.
    pub command: String,
    /// Origin (`CENTER`), e.g. `"500@10"` for the Sun's body centre.
    pub center: String,
    /// Reference plane.
    pub ref_plane: RefPlane,
    /// First epoch, Julian date TDB. Must be a multiple of the step from the stop epoch.
    pub start_jd_tdb: f64,
    /// Last epoch, Julian date TDB.
    pub stop_jd_tdb: f64,
    /// Step in whole days.
    pub step_days: u32,
}

/// Provenance of one Horizons answer.
#[derive(Debug, Clone)]
pub struct ChunkProvenance {
    /// The cached file, URL, retrieval date and SHA-256.
    pub fetched: Fetched,
    /// `signature.version` of the API.
    pub version: String,
    /// Ephemeris source of the target (e.g. `DE441`).
    pub target_source: String,
    /// Ephemeris source of the centre.
    pub center_source: String,
    /// The `Reference frame :` line.
    pub frame: String,
    /// Rows in this chunk.
    pub rows: usize,
}

impl ChunkProvenance {
    /// One line for test output.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "Horizons {} signature.version={} target_source={} center_source={} \
             reference_frame={:?} rows={}",
            self.fetched.provenance(),
            self.version,
            self.target_source,
            self.center_source,
            self.frame,
            self.rows
        )
    }
}

/// A merged state-vector table with the provenance of every chunk.
#[derive(Debug, Clone, Default)]
pub struct VectorTable {
    /// Epochs, Julian date TDB, strictly increasing.
    pub jd_tdb: Vec<f64>,
    /// Positions (km).
    pub r_km: Vec<[f64; 3]>,
    /// Velocities (km/s).
    pub v_km_s: Vec<[f64; 3]>,
    /// One entry per Horizons answer, in time order.
    pub chunks: Vec<ChunkProvenance>,
}

impl VectorTable {
    /// The distinct `signature.version` values seen (normally one).
    #[must_use]
    pub fn versions(&self) -> Vec<String> {
        let mut v: Vec<String> = self.chunks.iter().map(|c| c.version.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// The distinct target ephemeris sources seen (normally one, e.g. `DE441`).
    #[must_use]
    pub fn sources(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .chunks
            .iter()
            .map(|c| c.target_source.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

/// One parsed Horizons answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAnswer {
    /// `signature.version`.
    pub version: String,
    /// Ephemeris source of the target.
    pub target_source: String,
    /// Ephemeris source of the centre.
    pub center_source: String,
    /// The `Reference frame :` line.
    pub frame: String,
    /// Epochs (JD TDB).
    pub jd_tdb: Vec<f64>,
    /// Positions (km).
    pub r_km: Vec<[f64; 3]>,
    /// Velocities (km/s).
    pub v_km_s: Vec<[f64; 3]>,
}

fn service(message: impl Into<String>) -> DataError {
    DataError::Service(message.into())
}

/// The text after `key` on the first line that starts with `key`.
fn header_value<'a>(result: &'a str, key: &str) -> Option<&'a str> {
    result
        .lines()
        .find_map(|l| l.strip_prefix(key))
        .map(str::trim)
}

/// The `{source: …}` string on the first line that starts with `key`.
fn source_of(result: &str, key: &str) -> Option<String> {
    let line = result.lines().find(|l| l.starts_with(key))?;
    let after = line.split("source:").nth(1)?;
    Some(after.trim().trim_end_matches('}').trim().to_owned())
}

/// Parse a Horizons JSON answer to a `VEC_TABLE='2'`, `CSV_FORMAT='YES'` vector request.
///
/// # Errors
/// [`DataError::Service`] (`FA-E0203`) if the answer is not JSON, carries an `error`, lacks the
/// `$$SOE`/`$$EOE` table, has a malformed row, or its header does not state TDB, `KM-S`,
/// geometric states and the frame of `plane`.
pub fn parse_vector_answer(json: &str, plane: RefPlane) -> Result<ParsedAnswer, DataError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| service(format!("answer is not JSON: {e}")))?;
    if let Some(err) = value.get("error") {
        return Err(service(format!("Horizons error: {err}")));
    }
    let version = value
        .get("signature")
        .and_then(|s| s.get("version"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| service("answer lacks signature.version"))?
        .to_owned();
    let result = value
        .get("result")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| service("answer lacks a result string"))?;
    let soe = result.find("$$SOE");
    let eoe = result.find("$$EOE");
    let (Some(soe), Some(eoe)) = (soe, eoe) else {
        let head: String = result.trim().chars().take(300).collect();
        return Err(service(format!(
            "no $$SOE/$$EOE table; answer begins: {head:?}"
        )));
    };
    let target_source = source_of(result, "Target body name")
        .ok_or_else(|| service("no ephemeris source on the 'Target body name' line"))?;
    let center_source = source_of(result, "Center body name")
        .ok_or_else(|| service("no ephemeris source on the 'Center body name' line"))?;
    let frame = header_value(result, "Reference frame :")
        .ok_or_else(|| service("no 'Reference frame :' line"))?
        .to_owned();
    if !frame.contains(plane.frame_marker()) {
        return Err(service(format!(
            "reference frame {frame:?} does not match REF_PLANE {}",
            plane.param()
        )));
    }
    let checks = [
        ("Start time      :", "TDB"),
        ("Output units    :", "KM-S"),
        ("Output type     :", "GEOMETRIC cartesian states"),
    ];
    for (key, needle) in checks {
        let v = header_value(result, key).unwrap_or_default();
        if !v.contains(needle) {
            return Err(service(format!(
                "header {key:?} is {v:?}, expected {needle:?}"
            )));
        }
    }
    let table = result.get(soe + "$$SOE".len()..eoe).unwrap_or_default();
    let mut out = ParsedAnswer {
        version,
        target_source,
        center_source,
        frame,
        jd_tdb: Vec::new(),
        r_km: Vec::new(),
        v_km_s: Vec::new(),
    };
    for line in table.lines().filter(|l| !l.trim().is_empty()) {
        // JDTDB, Calendar Date (TDB), X, Y, Z, VX, VY, VZ,
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        let num = |i: usize| -> Result<f64, DataError> {
            fields
                .get(i)
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|x| x.is_finite())
                .ok_or_else(|| service(format!("malformed row (column {i}): {line:?}")))
        };
        out.jd_tdb.push(num(0)?);
        out.r_km.push([num(2)?, num(3)?, num(4)?]);
        out.v_km_s.push([num(5)?, num(6)?, num(7)?]);
    }
    if out.jd_tdb.is_empty() {
        return Err(service("empty $$SOE table"));
    }
    Ok(out)
}

/// The request URL of one chunk.
#[must_use]
pub fn vector_url(q: &VectorQuery, start_jd: f64, stop_jd: f64) -> String {
    let p = |s: &str| format!("%27{}%27", s.replace(' ', "%20").replace('@', "%40"));
    let mut url = format!("{API}?format=json");
    let params: [(&str, String); 14] = [
        ("COMMAND", q.command.clone()),
        ("OBJ_DATA", "NO".into()),
        ("MAKE_EPHEM", "YES".into()),
        ("EPHEM_TYPE", "VECTORS".into()),
        ("CENTER", q.center.clone()),
        ("START_TIME", format!("JD{start_jd:.1}")),
        ("STOP_TIME", format!("JD{stop_jd:.1}")),
        ("STEP_SIZE", format!("{} d", q.step_days)),
        ("REF_PLANE", q.ref_plane.param().into()),
        ("REF_SYSTEM", "ICRF".into()),
        ("OUT_UNITS", "KM-S".into()),
        ("CSV_FORMAT", "YES".into()),
        ("VEC_TABLE", "2".into()),
        ("VEC_LABELS", "NO".into()),
    ];
    for (k, v) in params {
        let _ = write!(url, "&{k}={}", p(&v));
    }
    url
}

/// Chunk boundaries `(start_jd, stop_jd, rows)` covering the query with at most
/// [`MAX_ROWS_PER_REQUEST`] rows each; consecutive chunks share their boundary epoch.
///
/// # Errors
/// [`DataError::Service`] for a zero step, a non-finite or empty span, or a span that is not a
/// whole number of steps.
pub fn chunks(q: &VectorQuery) -> Result<Vec<(f64, f64, u64)>, DataError> {
    let step = f64::from(q.step_days);
    let span = q.stop_jd_tdb - q.start_jd_tdb;
    if q.step_days == 0 || !span.is_finite() || span <= 0.0 {
        return Err(service(format!(
            "invalid query span {} .. {} step {} d",
            q.start_jd_tdb, q.stop_jd_tdb, q.step_days
        )));
    }
    let steps_f = (span / step).round();
    if steps_f.mul_add(step, -span).abs() > 1e-9 * span.max(1.0) || steps_f > 1e9 {
        return Err(service(format!(
            "span {span} d is not a whole number of {step} d steps"
        )));
    }
    // steps_f is a positive integer below 1e9 here, so the conversion is exact.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked: positive integer below 1e9"
    )]
    let steps = steps_f as u64;
    let n = (steps + 1).div_ceil(MAX_ROWS_PER_REQUEST - 1).max(1);
    let per = steps.div_ceil(n);
    let mut out = Vec::new();
    let mut k = 0;
    while k < steps {
        let end = (k + per).min(steps);
        #[expect(clippy::cast_precision_loss, reason = "step counts below 2^53")]
        let (a, b) = (
            (k as f64).mul_add(step, q.start_jd_tdb),
            (end as f64).mul_add(step, q.start_jd_tdb),
        );
        out.push((a, b, end - k + 1));
        k = end;
    }
    Ok(out)
}

fn cache_name(q: &VectorQuery, start: f64, stop: f64) -> String {
    let center: String = q
        .center
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!(
        "horizons-vec-{}-{center}-{}-jd{start:.1}-jd{stop:.1}-{}d.json",
        q.command,
        q.ref_plane.tag(),
        q.step_days
    )
}

fn fetch_chunk(
    q: &VectorQuery,
    start: f64,
    stop: f64,
) -> Result<(Fetched, ParsedAnswer), DataError> {
    let url = vector_url(q, start, stop);
    let file = cache_name(q, start, stop);
    for attempt in 0..2 {
        let cached = data_dir().join(&file).exists();
        if !cached {
            std::thread::sleep(POLITE_PAUSE);
        }
        let fetched = fetch_url(&url, &file, None)?;
        let text = std::fs::read_to_string(&fetched.path)
            .map_err(|e| service(format!("read {}: {e}", fetched.path.display())))?;
        match parse_vector_answer(&text, q.ref_plane) {
            Ok(parsed) => return Ok((fetched, parsed)),
            Err(e) if attempt == 0 => {
                // A bad answer must not poison the cache: drop it and ask once more.
                let _ = std::fs::remove_file(&fetched.path);
                let _ = std::fs::remove_file(
                    fetched
                        .path
                        .with_file_name(format!("{file}.provenance.toml")),
                );
                if !cached {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(service(format!(
        "could not obtain a valid answer for {url}"
    )))
}

/// Fetch (or reuse from the cache) a state-vector table over the whole query span.
///
/// Consecutive chunks share their boundary epoch; the shared row must be **bit-identical** in
/// both answers (a check that the chunks come from the same ephemeris) and is kept once.
///
/// # Errors
/// [`DataError`] from the download, `FA-E0203` for an unusable answer, a row count or spacing
/// different from the request, or chunks that disagree at their shared epoch.
pub fn fetch_vectors(q: &VectorQuery) -> Result<VectorTable, DataError> {
    let mut table = VectorTable::default();
    for (start, stop, rows) in chunks(q)? {
        let (fetched, parsed) = fetch_chunk(q, start, stop)?;
        let n = parsed.jd_tdb.len();
        if u64::try_from(n).ok() != Some(rows) {
            return Err(service(format!(
                "chunk {start}..{stop}: {n} rows, expected {rows}"
            )));
        }
        let step = f64::from(q.step_days);
        if parsed
            .jd_tdb
            .windows(2)
            .any(|w| matches!(w, [a, b] if ((b - a) - step).abs() > 1e-6))
        {
            return Err(service(format!(
                "chunk {start}..{stop}: epochs are not spaced by {step} d"
            )));
        }
        let mut skip = 0;
        if let (Some(&last_t), Some(&first_t)) = (table.jd_tdb.last(), parsed.jd_tdb.first()) {
            if last_t.to_bits() != first_t.to_bits() {
                return Err(service(format!(
                    "chunk boundary mismatch: previous ends at JD {last_t}, next starts at {first_t}"
                )));
            }
            let same = table.r_km.last() == parsed.r_km.first()
                && table.v_km_s.last() == parsed.v_km_s.first();
            if !same {
                return Err(service(format!(
                    "chunks disagree at their shared epoch JD {first_t}"
                )));
            }
            skip = 1;
        }
        table.jd_tdb.extend(parsed.jd_tdb.iter().skip(skip));
        table.r_km.extend(parsed.r_km.iter().skip(skip));
        table.v_km_s.extend(parsed.v_km_s.iter().skip(skip));
        table.chunks.push(ChunkProvenance {
            fetched,
            version: parsed.version,
            target_source: parsed.target_source,
            center_source: parsed.center_source,
            frame: parsed.frame,
            rows: n,
        });
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{"signature":{"source":"NASA/JPL Horizons API","version":"1.2"},
"result":"\n*****\nTarget body name: Mercury (199)                   {source: DE441}\nCenter body name: Sun (10)                        {source: DE441}\nStart time      : A.D. 2000-Jan-01 12:00:00.0000 TDB\nOutput units    : KM-S\nOutput type     : GEOMETRIC cartesian states\nReference frame : ICRF\n$$SOE\n2451545.000000000, A.D. 2000-Jan-01 12:00:00.0000, -1.0E+07, -2.0E+07, -3.0E+07,  1.0E+01, -2.0E+00, -3.0E+00,\n2451547.000000000, A.D. 2000-Jan-03 12:00:00.0000, -1.5E+07, -2.5E+07, -3.5E+07,  1.5E+01, -2.5E+00, -3.5E+00,\n$$EOE\n"}"#;

    /// Verifies: PHY-016
    #[test]
    fn parses_a_vector_answer_with_its_provenance() {
        let a = parse_vector_answer(FIXTURE, RefPlane::Frame).unwrap();
        assert_eq!(a.version, "1.2");
        assert_eq!(a.target_source, "DE441");
        assert_eq!(a.center_source, "DE441");
        assert_eq!(a.frame, "ICRF");
        assert_eq!(a.jd_tdb.len(), 2);
        // Parsing is exact: compare bit patterns with the same decimal strings parsed directly.
        let bits = |v: [f64; 3]| v.map(f64::to_bits);
        let parse = |s: [&str; 3]| s.map(|x| x.parse::<f64>().unwrap().to_bits());
        assert_eq!(bits(a.r_km[1]), parse(["-1.5E+07", "-2.5E+07", "-3.5E+07"]));
        assert_eq!(
            bits(a.v_km_s[0]),
            parse(["1.0E+01", "-2.0E+00", "-3.0E+00"])
        );
    }

    /// Verifies: PHY-016
    #[test]
    fn rejects_errors_wrong_frames_and_bad_rows() {
        let code = |s: &str, p: RefPlane| parse_vector_answer(s, p).unwrap_err().code();
        let service = ferroastro_core::ErrorCode::DataService;
        assert_eq!(code(FIXTURE, RefPlane::Ecliptic), service);
        assert_eq!(code("not json", RefPlane::Frame), service);
        assert_eq!(
            code(
                r#"{"signature":{"version":"1.2"},"error":"bad step"}"#,
                RefPlane::Frame
            ),
            service
        );
        let limit = r#"{"signature":{"version":"1.2"},"result":"Projected output length (~91313) exceeds 90024 -- change step-size"}"#;
        assert_eq!(code(limit, RefPlane::Frame), service);
        assert_eq!(
            code(&FIXTURE.replace("KM-S", "AU-D"), RefPlane::Frame),
            service
        );
        assert_eq!(
            code(&FIXTURE.replace("TDB\\n", "UT\\n"), RefPlane::Frame),
            service
        );
        assert_eq!(
            code(&FIXTURE.replace("-2.5E+07", "x"), RefPlane::Frame),
            service
        );
    }

    /// Verifies: PHY-016
    #[test]
    fn chunks_respect_the_row_limit_and_share_boundaries() {
        // 2000 Julian years at 2 d: 365 250 steps, 365 251 rows.
        let q = VectorQuery {
            command: "199".into(),
            center: "500@10".into(),
            ref_plane: RefPlane::Frame,
            start_jd_tdb: 2_086_295.0, // lint: allow-literal(J2000 - 1000 Julian years, request fixture)
            stop_jd_tdb: 2_816_795.0, // lint: allow-literal(J2000 + 1000 Julian years, request fixture)
            step_days: 2,
        };
        let c = chunks(&q).unwrap();
        assert_eq!(c.len(), 5);
        assert!(c.iter().all(|&(_, _, n)| n <= MAX_ROWS_PER_REQUEST));
        assert!(c.windows(2).all(|w| w[0].1.to_bits() == w[1].0.to_bits()));
        let rows: u64 = c.iter().map(|&(_, _, n)| n).sum::<u64>() - (c.len() as u64 - 1);
        assert_eq!(rows, 365_251); // lint: allow-literal(row count of the fixture span)
        assert_eq!(c[0].0.to_bits(), q.start_jd_tdb.to_bits());
        assert_eq!(c[4].1.to_bits(), q.stop_jd_tdb.to_bits());
        let url = vector_url(&q, c[0].0, c[0].1);
        assert!(url.contains("CENTER=%27500%4010%27") && url.contains("STEP_SIZE=%272%20d%27"));
        let bad = VectorQuery {
            step_days: 7,
            ..q.clone()
        };
        assert!(chunks(&bad).is_err());
    }
}
