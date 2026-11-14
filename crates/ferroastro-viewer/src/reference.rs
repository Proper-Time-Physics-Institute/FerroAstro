// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The engine-computed analytic reference of the conformal-gravity **comparison model** (toggle
//! bit 12, ABI v2; ADR-0016, PHY-020): the Gauss secular rates of
//! [`ferroastro_analytic::advance::conformal_linear_advance`] and
//! [`ferroastro_analytic::advance::conformal_quadratic_advance`] for Mercury and the current
//! parameters 13–16, so the page never evaluates a formula itself (DESIGN §1: no physics in JS).
//!
//! Nothing here is new physics: it composes the analytic crate with the DE440 constants of the
//! embedded header.440 and the SW92 J2000 mean elements of the Rung lab
//! (`ferroastro_ladder::SingleBody::mercury`), and converts per orbit to ″/cy with the two-body
//! mean motion `n = √((GM☉ + GM☿)/a³)` (the ladder's `RATE_CONVENTION`).

#![forbid(unsafe_code)]

use crate::ViewerError;
use crate::config::{ConformalSettings, Gamma0Centring};
use ferroastro_analytic::advance::{
    conformal_linear_advance, conformal_quadratic_advance, keplerian_period,
};
use ferroastro_core::time::{DAYS_PER_JULIAN_CENTURY, SECONDS_PER_DAY};
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_ladder::Scenario;
use ferroastro_ladder::single_body::SingleBody;
use ferroastro_scenario::CM_PER_KM;
use std::sync::OnceLock;

/// What the analytic reference is, for the metadata and the page.
pub const BASIS: &str = "comparison model (ADR-0016, PHY-020), not part of the GR ladder: \
     first-order Gauss secular averages, linear term -pi gamma a^2 sqrt(1-e^2)/beta and quadratic \
     term +3 pi kappa a^3 sqrt(1-e^2)/beta per orbit, beta = GM_sun/c^2 (DE440), Mercury's SW92 \
     J2000 mean a and e, rate = per orbit x n with n = sqrt((GM_sun + GM_Mercury)/a^3); a uniform \
     gamma0 contributes exactly 0 here (it moves every body equally)";

/// The caveat of the uniform centring, shown wherever its zero is.
pub const UNIFORM_CAVEAT: &str = "a uniform gamma0 leaves heliocentric motion unchanged exactly \
     only with velocity-independent forces; with EIH or Lense-Thirring on, the common drift \
     enters at O(v dv/c^2), a tiny but non-zero change far below any resolvable rate";

/// Mercury's analytic rate under the conformal-gravity terms, ″/cy (comparison model).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConformalReference {
    /// The Sun-sourced linear term (γ\* plus γ0 when Sun-centred), ″/cy.
    pub linear: f64,
    /// The quadratic term, ″/cy.
    pub quadratic: f64,
    /// Their sum, ″/cy.
    pub total: f64,
    /// The γ0 part of [`ConformalReference::linear`] (0 for the uniform centring), ″/cy.
    pub gamma0_part: f64,
}

/// The constants of the reference: `GM☉`, `c`, the two-body `GM☉ + GM☿`, Mercury's `a` and `e`.
#[derive(Debug, Clone, Copy)]
struct Inputs {
    gm_sun: GravParam,
    c: Velocity,
    a: Length,
    e: f64,
    orbits_per_century: f64,
}

fn inputs() -> Result<Inputs, ViewerError> {
    static I: OnceLock<Result<Inputs, String>> = OnceLock::new();
    I.get_or_init(|| {
        let de = crate::ic::de440().map_err(|e| e.message)?;
        let scn = Scenario::mercury_single_body().map_err(|e| e.to_string())?;
        let sb = SingleBody::mercury(&scn, de).map_err(|e| e.to_string())?;
        let el = sb.initial_elements().map_err(|e| e.to_string())?;
        let mu = GravParam::from_km3_per_s2(
            de.gm_sun().km3_per_s2() + de.gm(ferroastro_constants::Body::Mercury).km3_per_s2(),
        )
        .map_err(|e| e.to_string())?;
        let period = keplerian_period(mu, el.a).map_err(|e| e.to_string())?;
        Ok(Inputs {
            gm_sun: de.gm_sun(),
            c: de.speed_of_light(),
            a: el.a,
            e: el.e,
            orbits_per_century: DAYS_PER_JULIAN_CENTURY * SECONDS_PER_DAY / period.seconds(),
        })
    })
    .clone()
    .map_err(ViewerError::run)
}

/// The analytic reference for `settings` (comparison model).
///
/// # Errors
/// `FAV_E_UNSUPPORTED` in a build without `embed-header` (no DE440 constants);
/// `FAV_E_RUN` if an analytic formula refuses its inputs.
pub fn conformal_reference(
    settings: &ConformalSettings,
) -> Result<ConformalReference, ViewerError> {
    if crate::ic::HEADER_440.is_none() {
        return Err(ViewerError::new(
            crate::FAV_E_UNSUPPORTED,
            crate::ic::WITHOUT_EMBED_HEADER,
        ));
    }
    let i = inputs()?;
    let per_cy = |x: ferroastro_core::units::Angle| x.arcsec() * i.orbits_per_century;
    let lin = |gamma_per_cm: f64| {
        conformal_linear_advance(gamma_per_cm * CM_PER_KM, i.gm_sun, i.c, i.a, i.e)
            .map(per_cy)
            .map_err(|e| ViewerError::run(e.to_string()))
    };
    let linear = lin(settings.sun_gamma_per_cm())?;
    let quadratic = conformal_quadratic_advance(
        settings.kappa_per_cm2 * CM_PER_KM * CM_PER_KM,
        i.gm_sun,
        i.c,
        i.a,
        i.e,
    )
    .map(per_cy)
    .map_err(|e| ViewerError::run(e.to_string()))?;
    let gamma0_part = match settings.centre {
        Gamma0Centring::Sun => lin(settings.gamma0_per_cm)?,
        Gamma0Centring::Uniform => 0.0,
    };
    Ok(ConformalReference {
        linear,
        quadratic,
        total: linear + quadratic,
        gamma0_part,
    })
}
