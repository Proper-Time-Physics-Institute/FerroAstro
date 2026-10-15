// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Reported perihelion rates with mandatory metadata (PHY-015, PHY-008, NUM-012, AGENTS rule 4).
//!
//! A [`RateReport`] can only be made through [`RateReportBuilder::build`], which refuses
//! (`FA-E0602`) a report that lacks — or has a blank — **frame, epoch, fit span, fit model,
//! constants version**, gauge, estimator definition, dynamics or rung name. The rate itself
//! must also be present and finite.

use crate::PrecessionError;
use crate::analytic::rate_arcsec_per_century;
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{AngularRate, TimeSpan};
use std::fmt;

/// A perihelion rate with everything needed to interpret it.
#[derive(Debug, Clone, PartialEq)]
pub struct RateReport {
    /// Rung name, e.g. `"GR-SCHW numerical"`.
    pub rung: String,
    /// The secular rate.
    pub rate: AngularRate,
    /// Frame in which the perihelion longitude is measured.
    pub frame: String,
    /// Epoch of the initial conditions.
    pub epoch: Epoch<Tdb>,
    /// Span covered by the fit.
    pub span: TimeSpan,
    /// Fit model.
    pub fit_model: String,
    /// Constants set and version (name and checksum).
    pub constants: String,
    /// Coordinate gauge of the dynamics (PHY-008).
    pub gauge: String,
    /// Estimator definition (NUM-012).
    pub estimator: String,
    /// Force model.
    pub dynamics: String,
}

impl RateReport {
    /// The rate in ″/cy.
    #[must_use]
    pub fn arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.rate)
    }
}

impl fmt::Display for RateReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}: varpi_dot = {:.7} arcsec/cy",
            self.rung,
            self.arcsec_per_century()
        )?;
        writeln!(f, "  frame     : {}", self.frame)?;
        writeln!(
            f,
            "  epoch     : JD {} {}; fit span {:.3} d",
            self.epoch.jd(),
            self.epoch.scale_name(),
            self.span.seconds() / SECONDS_PER_DAY
        )?;
        writeln!(f, "  fit model : {}", self.fit_model)?;
        writeln!(f, "  estimator : {}", self.estimator)?;
        writeln!(f, "  gauge     : {}", self.gauge)?;
        writeln!(f, "  dynamics  : {}", self.dynamics)?;
        write!(f, "  constants : {}", self.constants)
    }
}

/// Builder for [`RateReport`]; every field is mandatory.
#[derive(Debug, Clone, Default)]
pub struct RateReportBuilder {
    rung: Option<String>,
    rate: Option<AngularRate>,
    frame: Option<String>,
    epoch: Option<Epoch<Tdb>>,
    span: Option<TimeSpan>,
    fit_model: Option<String>,
    constants: Option<String>,
    gauge: Option<String>,
    estimator: Option<String>,
    dynamics: Option<String>,
}

macro_rules! setter {
    ($name:ident, $ty:ty, $doc:literal) => {
        #[doc = $doc]
        #[must_use]
        pub fn $name(mut self, value: impl Into<$ty>) -> Self {
            self.$name = Some(value.into());
            self
        }
    };
}

fn text(field: &'static str, v: Option<String>) -> Result<String, PrecessionError> {
    match v {
        Some(s) if !s.trim().is_empty() => Ok(s),
        _ => Err(PrecessionError::MissingMetadata { field }),
    }
}

impl RateReportBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    setter!(rung, String, "Set the rung name.");
    setter!(rate, AngularRate, "Set the rate.");
    setter!(frame, String, "Set the frame.");
    setter!(epoch, Epoch<Tdb>, "Set the epoch.");
    setter!(span, TimeSpan, "Set the fit span.");
    setter!(fit_model, String, "Set the fit model.");
    setter!(constants, String, "Set the constants set and version.");
    setter!(gauge, String, "Set the gauge label.");
    setter!(estimator, String, "Set the estimator definition.");
    setter!(dynamics, String, "Set the force-model description.");

    /// Build the report.
    ///
    /// # Errors
    /// `FA-E0602` naming the first missing or blank field (a zero or non-finite span or a
    /// non-finite rate counts as missing).
    pub fn build(self) -> Result<RateReport, PrecessionError> {
        let rate = self
            .rate
            .filter(|r| r.rad_per_s().is_finite())
            .ok_or(PrecessionError::MissingMetadata { field: "rate" })?;
        let span = self
            .span
            .filter(|s| s.seconds() > 0.0)
            .ok_or(PrecessionError::MissingMetadata { field: "fit span" })?;
        let epoch = self
            .epoch
            .ok_or(PrecessionError::MissingMetadata { field: "epoch" })?;
        Ok(RateReport {
            rung: text("rung", self.rung)?,
            rate,
            frame: text("frame", self.frame)?,
            epoch,
            span,
            fit_model: text("fit model", self.fit_model)?,
            constants: text("constants version", self.constants)?,
            gauge: text("gauge", self.gauge)?,
            estimator: text("estimator", self.estimator)?,
            dynamics: text("dynamics", self.dynamics)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;

    fn complete() -> RateReportBuilder {
        RateReportBuilder::new()
            .rung("test rung")
            .rate(AngularRate::from_rad_per_s(1.0).unwrap())
            .frame("initial osculating orbit plane")
            .epoch(Epoch::<Tdb>::j2000())
            .span(TimeSpan::from_seconds(1.0).unwrap())
            .fit_model("OLS")
            .constants("DE440")
            .gauge("PPN isotropic/harmonic")
            .estimator("perihelion passages")
            .dynamics("test particle")
    }

    /// Every rung output missing any metadata field (or with a blank one) is rejected.
    ///
    /// Verifies: PHY-015, PHY-008, NUM-012
    #[test]
    fn reports_without_any_metadata_field_are_rejected() {
        assert!(complete().build().is_ok());
        let blank = |b: RateReportBuilder| b.build().unwrap_err();
        let cases = [
            ("frame", blank(complete().frame(" "))),
            ("fit model", blank(complete().fit_model(""))),
            ("constants version", blank(complete().constants(""))),
            ("gauge", blank(complete().gauge(""))),
            ("estimator", blank(complete().estimator(""))),
            ("fit span", blank(complete().span(TimeSpan::ZERO))),
        ];
        for (field, err) in cases {
            assert_eq!(err.code(), ErrorCode::ReportMetadataMissing);
            assert!(err.to_string().contains(field), "{err}");
        }
        // Builders that never set a field: epoch, gauge, frame.
        let mut b = complete();
        b.epoch = None;
        assert!(b.build().is_err());
        let mut b = complete();
        b.gauge = None;
        let e = b.build().unwrap_err();
        assert!(e.to_string().contains("FA-E0602") && e.to_string().contains("gauge"));
        let mut b = complete();
        b.frame = None;
        assert!(b.build().is_err());
    }
}
