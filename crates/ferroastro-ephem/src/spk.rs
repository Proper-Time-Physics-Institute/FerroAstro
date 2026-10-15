// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! SPK segments (NAIF "SPK Required Reading") and Chebyshev types 2 and 3.
//!
//! An SPK is a DAF with `ND = 2`, `NI = 6`. Each segment descriptor holds the initial and final
//! epoch (TDB seconds past J2000) and the target, centre, frame, data type and initial and final
//! word addresses. Types 2 and 3 end with a directory `INIT, INTLEN, RSIZE, N`; each of the `N`
//! records is `MID, RADIUS` followed by the coefficients of 3 (type 2: position) or 6 (type 3:
//! position and velocity) components. Type-2 velocities are the derivative of the position
//! series.

use crate::EphemError;
use crate::cheb::clenshaw_with_derivative;
use crate::daf::{Daf, Endian, WORD_BYTES, f64_at, f64_to_usize};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_real::Real;

/// Seconds per day (TDB days of 86 400 SI seconds, the SPK convention).
pub const SECONDS_PER_DAY: f64 = 86_400.0;
/// Julian date of J2000.0 (the origin of SPK epochs).
pub const J2000_JD: f64 = 2_451_545.0;
/// Slack allowed between a descriptor's coverage and the record grid, in seconds.
const COVERAGE_SLACK_S: f64 = 1e-3;

/// SPK data type of a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpkType {
    /// Type 2: Chebyshev polynomials for position; velocity by differentiation.
    Chebyshev2,
    /// Type 3: Chebyshev polynomials for position and velocity.
    Chebyshev3,
    /// Any other type (kept so that a file with such segments still opens; evaluating one is an
    /// error).
    Unsupported(i32),
}

impl SpkType {
    fn from_code(code: i32) -> Self {
        match code {
            2 => SpkType::Chebyshev2,
            3 => SpkType::Chebyshev3,
            other => SpkType::Unsupported(other),
        }
    }

    /// The NAIF type code.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            SpkType::Chebyshev2 => 2,
            SpkType::Chebyshev3 => 3,
            SpkType::Unsupported(c) => c,
        }
    }

    const fn components(self) -> Option<usize> {
        match self {
            SpkType::Chebyshev2 => Some(3),
            SpkType::Chebyshev3 => Some(6),
            SpkType::Unsupported(_) => None,
        }
    }
}

/// The directory of a Chebyshev (type 2/3) segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChebyshevLayout {
    /// Initial epoch of the first record, TDB seconds past J2000.
    pub init: f64,
    /// Length of each record's interval, seconds.
    pub intlen: f64,
    /// Words per record.
    pub rsize: usize,
    /// Number of records.
    pub n: usize,
    /// Coefficients per component (polynomial degree + 1).
    pub ncoef: usize,
}

/// A raw Cartesian state in kilometres and kilometres per second (frame and centre given by the
/// segment or chain that produced it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateKm<R: Real = f64> {
    /// Position, km.
    pub position: [R; 3],
    /// Velocity, km/s.
    pub velocity: [R; 3],
}

impl<R: Real> StateKm<R> {
    /// The zero state.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            position: [R::ZERO; 3],
            velocity: [R::ZERO; 3],
        }
    }

    /// Component-wise sum.
    #[must_use]
    pub fn add(&self, o: &Self) -> Self {
        let mut s = *self;
        for i in 0..3 {
            if let (Some(a), Some(b)) = (s.position.get_mut(i), o.position.get(i)) {
                *a += *b;
            }
            if let (Some(a), Some(b)) = (s.velocity.get_mut(i), o.velocity.get(i)) {
                *a += *b;
            }
        }
        s
    }

    /// Component-wise difference.
    #[must_use]
    pub fn sub(&self, o: &Self) -> Self {
        let neg = Self {
            position: o.position.map(|x| -x),
            velocity: o.velocity.map(|x| -x),
        };
        self.add(&neg)
    }
}

/// One SPK segment over borrowed file bytes.
#[derive(Debug, Clone)]
pub struct Segment<'a> {
    /// 0-based index in file order (later segments take precedence).
    pub index: usize,
    /// NAIF id of the target.
    pub target: i32,
    /// NAIF id of the centre.
    pub center: i32,
    /// NAIF frame id (1 = `J2000`, the ICRF-aligned frame of the JPL DE files).
    pub frame: i32,
    /// Data type.
    pub data_type: SpkType,
    /// Initial epoch of coverage, TDB seconds past J2000.
    pub start_et: f64,
    /// Final epoch of coverage, TDB seconds past J2000.
    pub end_et: f64,
    /// Segment name.
    pub name: String,
    /// Initial and final word addresses (1-based, inclusive).
    pub addresses: (usize, usize),
    /// The Chebyshev directory (types 2 and 3 only).
    pub layout: Option<ChebyshevLayout>,
    data: &'a [u8],
    endian: Endian,
}

/// Split a two-part TDB Julian date into the record index and the offset into that record,
/// following the three-step `divmod` of jplephem 2.24 so that whole days and fractions stay
/// apart. Returns `(index, offset_s)` with `0 ≤ offset_s < intlen` (before the end-point rule).
fn record_and_offset<R: Real>(jd1: R, jd2: R, init: R, intlen: R) -> (R, R) {
    let day = R::from_f64(SECONDS_PER_DAY);
    let divmod = |a: R, b: R| -> (R, R) {
        let r = a.rem_euclid(b);
        ((a - r) / b, r)
    };
    let (i1, o1) = divmod((jd1 - R::from_f64(J2000_JD)) * day - init, intlen);
    let (i2, o2) = divmod(jd2 * day, intlen);
    let (i3, offset) = divmod(o1 + o2, intlen);
    ((i1 + i2 + i3).round(), offset)
}

/// TDB seconds past J2000 of a two-part Julian date, in the backend's precision.
pub fn et_seconds<R: Real>(epoch: Epoch<Tdb, R>) -> R {
    let (jd1, jd2) = epoch.jd_parts();
    let day = R::from_f64(SECONDS_PER_DAY);
    (jd1 - R::from_f64(J2000_JD)) * day + jd2 * day
}

impl Segment<'_> {
    fn seg_err(
        &self,
        quantity: &'static str,
        value: String,
        unit: &'static str,
        valid: String,
    ) -> EphemError {
        EphemError::Segment {
            segment: self.index,
            target: self.target,
            center: self.center,
            quantity,
            value,
            unit,
            valid,
            fix: "the segment is corrupt or of an unsupported type; verify the file's SHA-256 or \
                  use a file with SPK type 2/3 segments",
        }
    }

    /// True when the segment covers the epoch (`start_et ≤ et ≤ end_et`).
    pub fn covers<R: Real>(&self, epoch: Epoch<Tdb, R>) -> bool {
        let et = et_seconds(epoch);
        et >= R::from_f64(self.start_et) && et <= R::from_f64(self.end_et)
    }

    fn coef(&self, word: usize) -> f64 {
        f64_at(self.data, word * WORD_BYTES, self.endian).unwrap_or(f64::NAN)
    }

    /// The raw coefficients of record `record` (0-based) for component `component`.
    ///
    /// # Errors
    /// [`EphemError::Segment`] for an unsupported type or an out-of-range record or component.
    pub fn coefficients(&self, record: usize, component: usize) -> Result<Vec<f64>, EphemError> {
        let (Some(l), Some(nc)) = (self.layout, self.data_type.components()) else {
            return Err(self.seg_err(
                "data type",
                self.data_type.code().to_string(),
                "",
                "2 or 3".into(),
            ));
        };
        if record >= l.n || component >= nc {
            return Err(self.seg_err(
                "record/component",
                format!("{record}/{component}"),
                "",
                format!("record < {}, component < {nc}", l.n),
            ));
        }
        let base = record * l.rsize + 2 + component * l.ncoef;
        Ok((0..l.ncoef).map(|k| self.coef(base + k)).collect())
    }

    /// `(MID, RADIUS)` of record `record` (0-based), seconds.
    ///
    /// # Errors
    /// [`EphemError::Segment`] for an unsupported type or an out-of-range record.
    pub fn record_mid_radius(&self, record: usize) -> Result<(f64, f64), EphemError> {
        let Some(l) = self.layout.filter(|l| record < l.n) else {
            return Err(self.seg_err(
                "record",
                record.to_string(),
                "",
                "a record of a type 2/3 segment".into(),
            ));
        };
        Ok((self.coef(record * l.rsize), self.coef(record * l.rsize + 1)))
    }

    /// The state of the target relative to the centre at `epoch`, in the segment's frame,
    /// evaluated in backend `R` (f64, double-double, …).
    ///
    /// # Errors
    /// [`EphemError::Segment`] for an unsupported type, [`EphemError::Coverage`] for an epoch
    /// outside `[start_et, end_et]`, [`EphemError::NonFinite`] for corrupt coefficients.
    pub fn state<R: Real>(&self, epoch: Epoch<Tdb, R>) -> Result<StateKm<R>, EphemError> {
        let (Some(l), Some(nc)) = (self.layout, self.data_type.components()) else {
            return Err(self.seg_err(
                "data type",
                self.data_type.code().to_string(),
                "",
                "2 (Chebyshev position) or 3 (Chebyshev position and velocity)".into(),
            ));
        };
        if !self.covers(epoch) {
            return Err(self.coverage_error(epoch));
        }
        let (jd1, jd2) = epoch.jd_parts();
        let intlen = R::from_f64(l.intlen);
        let (index, mut offset) = record_and_offset(jd1, jd2, R::from_f64(l.init), intlen);
        let mut record = f64_to_usize(index.to_f64()).ok_or_else(|| self.coverage_error(epoch))?;
        if record == l.n {
            // The final epoch of the last record belongs to that record (as in jplephem).
            record = l.n - 1;
            offset += intlen;
        }
        if record >= l.n {
            return Err(self.coverage_error(epoch));
        }
        let s = R::TWO * offset / intlen - R::ONE;
        let base = record * l.rsize + 2;
        let mut out = StateKm::<R>::zero();
        for comp in 0..nc {
            let first = base + comp * l.ncoef;
            let c = (first..first + l.ncoef).map(|w| R::from_f64(self.coef(w)));
            let (v, d) = clenshaw_with_derivative(c, s);
            match (comp, self.data_type) {
                (0..=2, _) => {
                    if let Some(p) = out.position.get_mut(comp) {
                        *p = v;
                    }
                    if self.data_type == SpkType::Chebyshev2 {
                        if let Some(dv) = out.velocity.get_mut(comp) {
                            // d/dt = d/ds · ds/dt = d/ds · 2/intlen (jplephem order: /intlen, ×2).
                            *dv = d / intlen * R::TWO;
                        }
                    }
                }
                _ => {
                    if let Some(dv) = out.velocity.get_mut(comp - 3) {
                        *dv = v;
                    }
                }
            }
        }
        if !out.position.iter().all(|x| x.is_finite()) {
            return Err(EphemError::NonFinite {
                target: self.target,
                center: self.center,
                quantity: "position",
                unit: "km",
            });
        }
        if !out.velocity.iter().all(|x| x.is_finite()) {
            return Err(EphemError::NonFinite {
                target: self.target,
                center: self.center,
                quantity: "velocity",
                unit: "km/s",
            });
        }
        Ok(out)
    }

    fn coverage_error<R: Real>(&self, epoch: Epoch<Tdb, R>) -> EphemError {
        let et = et_seconds(epoch).to_f64();
        EphemError::Coverage {
            body: self.target,
            center: self.center,
            et_s: et,
            jd_tdb: epoch.jd().to_f64(),
            valid: format!("[{}, {}] s TDB past J2000", self.start_et, self.end_et),
        }
    }
}

/// An SPK file: a DAF with `ND = 2`, `NI = 6`, and its segments.
#[derive(Debug, Clone)]
pub struct Spk<'a> {
    daf: Daf<'a>,
    segments: Vec<Segment<'a>>,
}

impl<'a> Spk<'a> {
    /// Parse an SPK file held in memory (no mmap, no `unsafe`). Every segment's addresses and,
    /// for types 2 and 3, its directory are validated here, so evaluation cannot read outside
    /// the file.
    ///
    /// # Errors
    /// [`EphemError::Format`] if the file is not an SPK (`ND ≠ 2` or `NI ≠ 6`, wrong id word),
    /// [`EphemError::Bounds`] for addresses outside the file, [`EphemError::Segment`] for an
    /// inconsistent descriptor or directory.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, EphemError> {
        let daf = Daf::parse(bytes)?;
        if !(daf.id_word() == "DAF/SPK" || daf.id_word() == "NAIF/DAF")
            || daf.nd() != 2
            || daf.ni() != 6
        {
            return Err(EphemError::Format {
                quantity: "LOCIDW/ND/NI",
                value: format!("{}/{}/{}", daf.id_word(), daf.nd(), daf.ni()),
                valid: "an SPK: id word DAF/SPK (or NAIF/DAF), ND = 2, NI = 6",
                fix: "pass an SPK (.bsp) file",
            });
        }
        let summaries = daf.summaries()?;
        let mut segments = Vec::with_capacity(summaries.len());
        for (index, s) in summaries.into_iter().enumerate() {
            let d = |i: usize| s.doubles.get(i).copied().unwrap_or(f64::NAN);
            let n = |i: usize| s.ints.get(i).copied().unwrap_or(0);
            let (first, last) = (n(4), n(5));
            let addr = |a: i32, q: &'static str| {
                usize::try_from(a).map_err(|_| EphemError::Bounds {
                    quantity: q,
                    value: 0,
                    unit: "word",
                    min: 1,
                    max: u64::try_from(daf.word_count()).unwrap_or(u64::MAX),
                })
            };
            let (first, last) = (
                addr(first, "initial array address")?,
                addr(last, "final array address")?,
            );
            let data = daf.words(first, last)?;
            let mut seg = Segment {
                index,
                target: n(0),
                center: n(1),
                frame: n(2),
                data_type: SpkType::from_code(n(3)),
                start_et: d(0),
                end_et: d(1),
                name: s.name,
                addresses: (first, last),
                layout: None,
                data,
                endian: daf.endian(),
            };
            if !(seg.start_et.is_finite() && seg.end_et.is_finite() && seg.start_et <= seg.end_et) {
                return Err(seg.seg_err(
                    "coverage [start, end]",
                    format!("[{}, {}]", seg.start_et, seg.end_et),
                    "s TDB",
                    "finite with start <= end".into(),
                ));
            }
            if let Some(nc) = seg.data_type.components() {
                seg.layout = Some(chebyshev_layout(&seg, nc)?);
            }
            segments.push(seg);
        }
        Ok(Self { daf, segments })
    }

    /// The DAF container.
    #[must_use]
    pub const fn daf(&self) -> &Daf<'a> {
        &self.daf
    }

    /// The segments in file order.
    #[must_use]
    pub fn segments(&self) -> &[Segment<'a>] {
        &self.segments
    }

    /// The segment that provides `target` at `epoch`: the **last** covering segment in file
    /// order (NAIF data precedence). `None` if no segment for `target` covers the epoch.
    pub fn segment_for<R: Real>(&self, target: i32, epoch: Epoch<Tdb, R>) -> Option<&Segment<'a>> {
        self.segments
            .iter()
            .rev()
            .find(|s| s.target == target && s.covers(epoch))
    }
}

fn chebyshev_layout(seg: &Segment<'_>, ncomp: usize) -> Result<ChebyshevLayout, EphemError> {
    let words = seg.data.len() / WORD_BYTES;
    if words < 4 {
        return Err(seg.seg_err(
            "array length",
            words.to_string(),
            "words",
            ">= 4 (directory)".into(),
        ));
    }
    let dir = |i: usize| seg.coef(words - 4 + i);
    let (init, intlen, rsize_f, n_f) = (dir(0), dir(1), dir(2), dir(3));
    if !(init.is_finite() && intlen.is_finite() && intlen > 0.0) {
        return Err(seg.seg_err(
            "INIT/INTLEN",
            format!("{init}/{intlen}"),
            "s",
            "finite, INTLEN > 0".into(),
        ));
    }
    let rsize = f64_to_usize(rsize_f)
        .filter(|&r| r >= 2 + ncomp && (r - 2) % ncomp == 0)
        .ok_or_else(|| {
            seg.seg_err(
                "RSIZE",
                rsize_f.to_string(),
                "words",
                format!("2 + {ncomp}·(degree + 1)"),
            )
        })?;
    let n = f64_to_usize(n_f).filter(|&n| n >= 1).ok_or_else(|| {
        seg.seg_err(
            "N (records)",
            n_f.to_string(),
            "records",
            "an integer >= 1".into(),
        )
    })?;
    if n.checked_mul(rsize).and_then(|x| x.checked_add(4)) != Some(words) {
        return Err(seg.seg_err(
            "N·RSIZE + 4",
            format!("{n}·{rsize} + 4"),
            "words",
            format!("the array length {words}"),
        ));
    }
    let span = f64::from(u32::try_from(n).unwrap_or(u32::MAX)) * intlen;
    if seg.start_et < init - COVERAGE_SLACK_S || seg.end_et > init + span + COVERAGE_SLACK_S {
        return Err(seg.seg_err(
            "coverage [start, end]",
            format!("[{}, {}]", seg.start_et, seg.end_et),
            "s TDB",
            format!("inside the record grid [{init}, {}]", init + span),
        ));
    }
    Ok(ChebyshevLayout {
        init,
        intlen,
        rsize,
        n,
        ncoef: (rsize - 2) / ncomp,
    })
}
