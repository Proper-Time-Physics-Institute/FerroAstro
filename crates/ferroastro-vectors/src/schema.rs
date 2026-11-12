// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The ADR-0004 vector schema: raw serde shapes and their validation.

use crate::{Deviation, VectorError};
use serde::Deserialize;
use std::path::Path;

/// Requirement-ID prefixes (ORG-003).
const PREFIXES: [&str; 11] = [
    "ORG", "STD", "PHY", "NUM", "TST", "API", "AGT", "DEP", "REF", "CI", "LIC",
];

/// Markers that make a value ineligible as an oracle (PHY-018).
const FORBIDDEN_MARKERS: [&str; 2] = ["UNVERIFIED", "[S]"];

/// How the expected value was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Exact by definition (SI, IAU resolutions).
    Defined,
    /// A published measurement or fit.
    Measured,
    /// Produced by an independent reference engine (TST-003).
    ComputedReference,
    /// A closed-form formula evaluated from other vectors.
    Analytic,
}

/// The expected value as written in the file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum VectorValue {
    /// TOML integer.
    Int(i64),
    /// TOML float.
    Float(f64),
    /// Decimal string, kept verbatim so extended-precision consumers can parse it themselves.
    Text(String),
}

/// Tolerance: pass when `|x − v| ≤ max(abs, rel·|v|)`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tolerance {
    /// Absolute tolerance, in the vector's unit.
    pub abs: Option<f64>,
    /// Relative tolerance (dimensionless).
    pub rel: Option<f64>,
}

impl Tolerance {
    /// The allowed absolute deviation around `expected`.
    #[must_use]
    pub fn allowed(&self, expected: f64) -> f64 {
        let abs = self.abs.unwrap_or(0.0);
        let rel = self.rel.map_or(0.0, |r| r * expected.abs());
        abs.max(rel)
    }
}

/// Reference engine of a computed-reference vector (TST-003).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// Engine name, e.g. `"JPL Horizons"`.
    pub name: String,
    /// Engine version string as reported by the engine.
    pub version: String,
    /// Settings that determine the value (integrator, step, ephemeris, …).
    pub settings: String,
}

/// One validated test vector.
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    /// Unique id.
    pub id: String,
    /// Requirement ids this vector serves.
    pub requirement: Vec<String>,
    /// What the value is.
    pub quantity: String,
    /// The expected value.
    pub value: VectorValue,
    /// Unit of `value` and of `tolerance.abs`.
    pub unit: String,
    /// Provenance class.
    pub kind: Kind,
    /// Where the value comes from (document, data product, engine).
    pub source: String,
    /// Bibliographic citation with locator (DOI, table, page).
    pub citation: String,
    /// Date the source was retrieved, `YYYY-MM-DD`.
    pub retrieved: String,
    /// Allowed deviation.
    pub tolerance: Tolerance,
    /// Why the tolerance is what it is.
    pub tolerance_rationale: String,
    /// Who or what checked the value against the primary source.
    pub verified_by: String,
    /// Generating script `path@sha` (computed-reference only).
    pub generated_by: Option<String>,
    /// Reference engine (computed-reference only).
    pub engine: Option<Engine>,
    /// Free-form notes.
    pub notes: Option<String>,
    /// An open item (`OI-NN`) that keeps this oracle non-gating while it is open in
    /// `docs/requirements/OPEN-ITEMS.md` (e.g. an unverified protocol behind a published value).
    pub blocked_by: Option<String>,
    /// An open item (`OI-NN`) under which this vector's assertion is PROVISIONAL (e.g. a protocol
    /// chosen before a published method was confirmed; ADR-0014, OI-38). Unlike `blocked_by` it
    /// does not change gating: the vector is checked like any other. It labels the result, and it
    /// must name an item recorded in `docs/requirements/OPEN-ITEMS.md`
    /// ([`Vector::provisional_open_item`]), where the revert condition lives. Excludes
    /// `blocked_by`.
    pub provisional: Option<String>,
}

/// The state of a vector's `provisional` open item, from [`Vector::provisional_open_item`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provisional<'a> {
    /// The open item, `OI-NN`.
    pub open_item: &'a str,
    /// True while the item is unticked in `OPEN-ITEMS.md`.
    pub open: bool,
}

impl std::fmt::Display for Provisional<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let oi = self.open_item;
        if self.open {
            write!(
                f,
                "PROVISIONAL ({oi} open): this assertion reverts if {oi} is closed against its \
                 protocol; revert condition in OPEN-ITEMS.md {oi} and the vector's tolerance_rationale"
            )
        } else {
            write!(
                f,
                "PROVISIONAL label stale: {oi} is closed in OPEN-ITEMS.md; drop `provisional` or \
                 apply the revert condition recorded in {oi} and the vector's tolerance_rationale"
            )
        }
    }
}

/// `Some(true)` if `oi` is an unticked item of `open_items_md`, `Some(false)` if ticked
/// (`- [x] **OI-NN**`), `None` if the file does not list it.
fn open_item_state(open_items_md: &str, oi: &str) -> Option<bool> {
    let tag = format!("**{oi}**");
    let mut state = None;
    for l in open_items_md.lines() {
        let l = l.trim_start();
        let Some(rest) = l.strip_prefix("- [") else {
            continue;
        };
        let open = match rest.get(..2) {
            Some("x]" | "X]") => false,
            Some(" ]") => true,
            _ => continue,
        };
        if rest
            .get(2..)
            .is_some_and(|r| r.trim_start().starts_with(&tag))
        {
            if !open {
                return Some(false);
            }
            state = Some(true);
        }
    }
    state
}

impl Vector {
    /// The open item that keeps this vector non-gating, if `blocked_by` names one that
    /// `open_items_md` (the text of `docs/requirements/OPEN-ITEMS.md`) does not mark closed
    /// (`- [x] **OI-NN**`). An item missing from the file counts as open.
    #[must_use]
    pub fn blocking_open_item(&self, open_items_md: &str) -> Option<&str> {
        let oi = self.blocked_by.as_deref()?;
        (open_item_state(open_items_md, oi) != Some(false)).then_some(oi)
    }

    /// The `provisional` open item of this vector and whether it is still open in
    /// `open_items_md` (the text of `docs/requirements/OPEN-ITEMS.md`); `None` for a vector
    /// that is not provisional. A provisional vector gates like any other; callers print the
    /// returned label ([`Provisional`]'s `Display`) next to every result it judges.
    ///
    /// # Errors
    /// [`VectorError::Schema`] (`FA-E0102`) if the item is not listed in `open_items_md` at all:
    /// a provisional assertion must point at a recorded open item and its revert condition
    /// (unlike `blocked_by`, where a missing item counts as open).
    pub fn provisional_open_item<'a>(
        &'a self,
        open_items_md: &str,
    ) -> Result<Option<Provisional<'a>>, VectorError> {
        let Some(oi) = self.provisional.as_deref() else {
            return Ok(None);
        };
        match open_item_state(open_items_md, oi) {
            Some(open) => Ok(Some(Provisional {
                open_item: oi,
                open,
            })),
            None => Err(VectorError::Schema {
                path: Path::new("docs/requirements/OPEN-ITEMS.md").to_path_buf(),
                id: self.id.clone(),
                field: "provisional",
                problem: format!(
                    "names {oi}, which OPEN-ITEMS.md does not list; fix: record the open item and \
                     its revert condition there, or drop `provisional`"
                ),
            }),
        }
    }

    /// The expected value as `f64`.
    ///
    /// # Errors
    /// [`VectorError::Schema`] if a decimal-string value does not parse (validated at load, so
    /// this only fails for vectors constructed by hand).
    pub fn value(&self) -> Result<f64, VectorError> {
        match &self.value {
            #[expect(
                clippy::cast_precision_loss,
                reason = "integers above 2^53 are rejected at load"
            )]
            VectorValue::Int(i) => Ok(*i as f64),
            VectorValue::Float(f) => Ok(*f),
            VectorValue::Text(s) => s.trim().parse::<f64>().map_err(|e| VectorError::Schema {
                path: Path::new("<memory>").to_path_buf(),
                id: self.id.clone(),
                field: "value",
                problem: format!("decimal string {s:?} does not parse: {e}"),
            }),
        }
    }

    /// Check a measured value against this vector's tolerance; returns `|measured − expected|`.
    ///
    /// # Errors
    /// [`VectorError::ToleranceExceeded`] (with every number needed to judge it) on failure.
    pub fn check(&self, measured: f64) -> Result<f64, VectorError> {
        let expected = self.value()?;
        let diff = (measured - expected).abs();
        let allowed = self.tolerance.allowed(expected);
        if diff <= allowed {
            Ok(diff)
        } else {
            Err(VectorError::ToleranceExceeded(Box::new(Deviation {
                id: self.id.clone(),
                quantity: self.quantity.clone(),
                unit: self.unit.clone(),
                measured,
                expected,
                diff,
                allowed,
                tolerance: self.tolerance,
                provisional: self.provisional.clone(),
            })))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    schema_version: i64,
    #[serde(default)]
    vector: Vec<RawVector>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVector {
    id: Option<String>,
    requirement: Option<Vec<String>>,
    quantity: Option<String>,
    value: Option<VectorValue>,
    unit: Option<String>,
    kind: Option<Kind>,
    source: Option<String>,
    citation: Option<String>,
    retrieved: Option<String>,
    tolerance: Option<Tolerance>,
    tolerance_rationale: Option<String>,
    verified_by: Option<String>,
    generated_by: Option<String>,
    engine: Option<Engine>,
    notes: Option<String>,
    blocked_by: Option<String>,
    provisional: Option<String>,
}

struct Ctx<'a> {
    path: &'a Path,
    id: String,
}

impl Ctx<'_> {
    fn err(&self, field: &'static str, problem: impl Into<String>) -> VectorError {
        VectorError::Schema {
            path: self.path.to_path_buf(),
            id: self.id.clone(),
            field,
            problem: problem.into(),
        }
    }

    fn text(&self, field: &'static str, v: Option<String>) -> Result<String, VectorError> {
        match v {
            Some(s) if !s.trim().is_empty() => Ok(s),
            Some(_) => Err(self.err(field, "is empty; every provenance field needs content")),
            None => Err(self.err(field, "is missing (required by ADR-0004)")),
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('-').all(|w| {
            !w.is_empty()
                && w.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
}

fn valid_requirement(r: &str) -> bool {
    match r.rsplit_once('-') {
        Some((prefix, num)) => {
            PREFIXES.contains(&prefix) && num.len() == 3 && num.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}

fn check_computed_reference(
    ctx: &Ctx<'_>,
    kind: Kind,
    generated_by: Option<&str>,
    engine: Option<&Engine>,
) -> Result<(), VectorError> {
    if kind != Kind::ComputedReference {
        return Ok(());
    }
    if !generated_by.is_some_and(|g| {
        g.split_once('@')
            .is_some_and(|(p, sha)| !p.is_empty() && sha.len() >= 7)
    }) {
        return Err(ctx.err(
            "generated_by",
            "computed-reference needs `path@sha` (TST-003)",
        ));
    }
    if !engine.is_some_and(|e| {
        [&e.name, &e.version, &e.settings]
            .iter()
            .all(|s| !s.trim().is_empty())
    }) {
        return Err(ctx.err("engine", "computed-reference needs name, version, settings"));
    }
    Ok(())
}

/// `blocked_by` and `provisional` each name one well-formed `OI-NN` id, and never both.
fn check_open_item_refs(
    ctx: &Ctx<'_>,
    blocked_by: Option<&str>,
    provisional: Option<&str>,
) -> Result<(), VectorError> {
    // ADR-0004 revision 2026-10-21: `provisional` matches `^OI-\d{2,}$`; `blocked_by` keeps its
    // original `^OI-\d+$`.
    for (field, v, min_digits) in [
        ("blocked_by", blocked_by, 1),
        ("provisional", provisional, 2),
    ] {
        let well_formed = |v: &str| {
            v.strip_prefix("OI-")
                .is_some_and(|n| n.len() >= min_digits && n.chars().all(|c| c.is_ascii_digit()))
        };
        match v {
            Some(v) if !well_formed(v) => {
                return Err(ctx.err(field, format!("{v:?} is not an OI-NN open-item id")));
            }
            _ => {}
        }
    }
    if blocked_by.is_some() && provisional.is_some() {
        return Err(ctx.err(
            "provisional",
            "cannot be combined with `blocked_by`: a provisional vector gates, a blocked one \
             does not; keep one",
        ));
    }
    Ok(())
}

/// Largest integer that converts to f64 exactly.
const MAX_EXACT_INT: i64 = 1 << 53;

fn validate(path: &Path, raw: RawVector) -> Result<Vector, VectorError> {
    let mut ctx = Ctx {
        path,
        id: raw.id.clone().unwrap_or_default(),
    };
    let id = ctx.text("id", raw.id)?;
    if !valid_id(&id) {
        return Err(ctx.err("id", "must be upper-case A-Z0-9 words joined by '-'"));
    }
    ctx.id.clone_from(&id);

    let requirement = raw
        .requirement
        .ok_or_else(|| ctx.err("requirement", "is missing"))?;
    if requirement.is_empty() {
        return Err(ctx.err("requirement", "must list at least one requirement id"));
    }
    if let Some(bad) = requirement.iter().find(|r| !valid_requirement(r)) {
        return Err(ctx.err("requirement", format!("{bad:?} is not a <PREFIX>-NNN id")));
    }

    let value = raw.value.ok_or_else(|| ctx.err("value", "is missing"))?;
    match &value {
        VectorValue::Float(f) if !f.is_finite() => return Err(ctx.err("value", "must be finite")),
        VectorValue::Int(i) if i.unsigned_abs() > MAX_EXACT_INT.unsigned_abs() => {
            return Err(ctx.err(
                "value",
                "integer exceeds 2^53; write it as a decimal string",
            ));
        }
        VectorValue::Text(s) if !s.trim().parse::<f64>().is_ok_and(f64::is_finite) => {
            return Err(ctx.err(
                "value",
                format!("decimal string {s:?} is not a finite number"),
            ));
        }
        _ => {}
    }

    let kind = raw.kind.ok_or_else(|| ctx.err("kind", "is missing"))?;
    let source = ctx.text("source", raw.source)?;
    let citation = ctx.text("citation", raw.citation)?;
    for (field, text) in [("source", &source), ("citation", &citation)] {
        if let Some(m) = FORBIDDEN_MARKERS.iter().find(|m| text.contains(*m)) {
            return Err(ctx.err(
                field,
                format!("contains {m}: such values are not oracles until closed in OPEN-ITEMS.md"),
            ));
        }
    }
    let retrieved = ctx.text("retrieved", raw.retrieved)?;
    if !valid_date(&retrieved) {
        return Err(ctx.err("retrieved", "must be YYYY-MM-DD"));
    }

    let tolerance = raw
        .tolerance
        .ok_or_else(|| ctx.err("tolerance", "is missing"))?;
    if tolerance.abs.is_none() && tolerance.rel.is_none() {
        return Err(ctx.err("tolerance", "needs `abs` and/or `rel`"));
    }
    for t in [tolerance.abs, tolerance.rel].into_iter().flatten() {
        if !(t.is_finite() && t >= 0.0) {
            return Err(ctx.err("tolerance", "values must be finite and >= 0"));
        }
    }

    let generated_by = raw.generated_by;
    let engine = raw.engine;
    check_computed_reference(&ctx, kind, generated_by.as_deref(), engine.as_ref())?;
    check_open_item_refs(&ctx, raw.blocked_by.as_deref(), raw.provisional.as_deref())?;

    Ok(Vector {
        quantity: ctx.text("quantity", raw.quantity)?,
        unit: ctx.text("unit", raw.unit)?,
        tolerance_rationale: ctx.text("tolerance_rationale", raw.tolerance_rationale)?,
        verified_by: ctx.text("verified_by", raw.verified_by)?,
        id,
        requirement,
        value,
        kind,
        source,
        citation,
        retrieved,
        tolerance,
        generated_by,
        engine,
        notes: raw.notes,
        blocked_by: raw.blocked_by,
        provisional: raw.provisional,
    })
}

pub(crate) fn parse_file(path: &Path, text: &str) -> Result<Vec<Vector>, VectorError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| VectorError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    if raw.schema_version != 1 {
        return Err(VectorError::Parse {
            path: path.to_path_buf(),
            message: format!(
                "unsupported schema_version {}; expected 1",
                raw.schema_version
            ),
        });
    }
    raw.vector.into_iter().map(|v| validate(path, v)).collect()
}
