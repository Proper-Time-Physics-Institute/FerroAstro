// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The engine-computed analytic references of the **comparison models** (ADR-0016): for conformal
//! gravity (toggle bit 12, ABI v2; PHY-020) the Gauss secular rates of
//! [`ferroastro_analytic::advance::conformal_linear_advance`] and
//! [`ferroastro_analytic::advance::conformal_quadratic_advance`] for Mercury and the current
//! parameters 13–16, and for the Proper Time (Gill) kernel (bit 13, ABI v3; PHY-021) the
//! first-order [`ferroastro_analytic::advance::proper_time_advance`], −1/6 of GR's, so the page
//! never evaluates a formula itself (DESIGN §1: no physics in JS).
//!
//! Nothing here is new physics: it composes the analytic crate with the DE440 constants of the
//! embedded header.440 and the SW92 J2000 mean elements of the Rung lab
//! (`ferroastro_ladder::SingleBody::mercury`), and converts per orbit to ″/cy with the two-body
//! mean motion `n = √((GM☉ + GM☿)/a³)` (the ladder's `RATE_CONVENTION`).

#![forbid(unsafe_code)]

use crate::ViewerError;
use crate::config::{ConformalSettings, Gamma0Centring};
use ferroastro_analytic::advance::{
    conformal_linear_advance, conformal_quadratic_advance, keplerian_period, proper_time_advance,
    schwarzschild_advance,
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

/// What the Proper Time analytic reference is, for the metadata and the page.
pub const PROPER_TIME_BASIS: &str = "comparison model (ADR-0016 D6, PHY-021), not part of the GR \
     ladder: first-order advance of Gill's flat-space force law a = -(grad V/m)(1 + V/(m c^2)), \
     -pi GM_sun/(c^2 a (1-e^2)) per orbit = -1/6 of GR's 6 pi GM_sun/(c^2 a (1-e^2)) (the PPN \
     factor (2 + 2 gamma - beta)/3 at beta = 1/2, gamma = -1), DE440 GM_sun and c, Mercury's SW92 \
     J2000 mean a and e, rate = per orbit x n with n = sqrt((GM_sun + GM_Mercury)/a^3)";

/// Bridge A, the statement that makes the PPN (1/2, −1) run a second route to the same physics.
pub const BRIDGE_A: &str = "Bridge A: Gill's interaction Hamiltonian H1 = sqrt(c^2 p^2 + (m c^2 + \
     V)^2) is exactly a constant-mass particle on a geodesic of the conformally flat metric (1 + \
     V/(m c^2))^2 eta = (1 - GM/(r c^2))^2 eta (Weyl tensor zero), whose PPN parameters are \
     beta = 1/2, gamma = -1 (relativistic-dynamics#88, research/2026-11-14, RD88-A)";

/// Mercury's analytic rate under the Proper Time (Gill) kernel and under GR 1PN, ″/cy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProperTimeReference {
    /// The Proper Time kernel's first-order rate, ″/cy (comparison model).
    pub rate: f64,
    /// GR's first-order rate for the same inputs, ″/cy.
    pub gr: f64,
}

/// The Proper Time analytic reference for Mercury (comparison model, PHY-021).
///
/// # Errors
/// `FAV_E_UNSUPPORTED` in a build without `embed-header` (no DE440 constants);
/// `FAV_E_RUN` if an analytic formula refuses its inputs.
pub fn proper_time_reference() -> Result<ProperTimeReference, ViewerError> {
    if crate::ic::HEADER_440.is_none() {
        return Err(ViewerError::new(
            crate::FAV_E_UNSUPPORTED,
            crate::ic::WITHOUT_EMBED_HEADER,
        ));
    }
    let i = inputs()?;
    let run = |e: &dyn std::fmt::Display| ViewerError::run(e.to_string());
    let rate = proper_time_advance(i.gm_sun, i.c, i.a, i.e).map_err(|e| run(&e))?;
    let gr = schwarzschild_advance(i.gm_sun, i.c, i.a, i.e, i.gm_sun).map_err(|e| run(&e))?;
    Ok(ProperTimeReference {
        rate: rate.arcsec() * i.orbits_per_century,
        gr: gr.per_orbit.arcsec() * i.orbits_per_century,
    })
}

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
pub(crate) struct Inputs {
    pub(crate) gm_sun: GravParam,
    pub(crate) c: Velocity,
    pub(crate) a: Length,
    pub(crate) e: f64,
    pub(crate) orbits_per_century: f64,
}

/// The shared inputs (also used by [`crate::dashboard`]'s analytic estimates).
pub(crate) fn inputs() -> Result<Inputs, ViewerError> {
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
