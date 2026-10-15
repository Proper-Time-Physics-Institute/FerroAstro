// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung N-2B (PHY-001): a Newtonian Sun + Mercury run must show zero perihelion precession.
//! Any fitted rate is integrator error, and the output says so.

mod common;

use ferroastro_constants::De440;
use ferroastro_core::frames::{EclipticJ2000, Heliocentric, State};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::TimeSpan;
use ferroastro_forces::{ForceModel, NewtonianPointMasses};
use ferroastro_integrators::Ias15Config;
use ferroastro_precession::fit::{Metadata, fit_secular_rate};
use ferroastro_vectors::VectorSet;

/// Span of the run in Julian years (PHY-001: at least 1000).
const SPAN_YEARS: f64 = 1000.0; // lint: allow-literal(PHY-001 minimum span, from the requirement text)

/// Verifies: PHY-001, NUM-001
#[test]
fn n2b_sun_mercury_has_no_perihelion_precession() {
    let de = De440::load().unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let (gm_sun, gm_me) = common::gms(&de);
    // Caveat C7, documented: the relative orbit has mu = GM_sun + GM_Mercury (both bodies move).
    let mu = gm_sun + gm_me;
    let el = common::sw92_elements(&vs, &de);
    let rel: State<EclipticJ2000, Heliocentric> = el.to_state(mu).unwrap();
    let (x0, v0) = common::barycentric(&rel, gm_sun, gm_me);
    let model = NewtonianPointMasses::new(&[gm_sun, gm_me]).unwrap();
    let cfg = Ias15Config::default();
    let span = SPAN_YEARS * common::julian_year_s();
    let started = std::time::Instant::now();
    let run = common::run_lrl(&model, &x0, &v0, mu.km3_per_s2(), span, cfg);
    let wall = started.elapsed();
    let meta = Metadata {
        estimator: "LRL eccentricity-vector longitude of the Mercury-Sun relative orbit",
        frame: "initial osculating orbit plane of Mercury, axes fixed in the mean ecliptic and equinox of J2000 (SW92 frame); zero at the initial periapsis".into(),
        epoch: Epoch::<Tdb>::j2000(),
        span: TimeSpan::from_seconds(span).unwrap(),
        dynamics: format!("{} ({}), Sun + Mercury, mu = GM_sun + GM_Mercury", model.name(), model.gauge().label()),
        integrator: format!("IAS15 (FerroAstro clean-room), epsilon = {:e}, safety = {}, max iterations = {}, f64 + compensated summation", cfg.epsilon, cfg.safety, cfg.max_iterations),
        constants: format!("DE440 (header.440 sha256 {}); SW92 J2000 mean elements", de.table().source().sha256),
    };
    let est = fit_secular_rate(&run.times, &run.longitudes, meta).unwrap();
    let st = run.stats;
    println!("N-2B (PHY-001) result:\n{est}");
    println!(
        "  steps {} (rejected {}), force evaluations {}, unconverged {}, wall {:.2} s",
        st.steps,
        st.rejected,
        st.force_evaluations,
        st.unconverged,
        wall.as_secs_f64()
    );
    println!(
        "  NOTE: the true N-2B rate is exactly 0; the fitted {:e} arcsec/cy is NUMERICAL (integrator + round-off) ERROR, not physics.",
        est.arcsec_per_century()
    );
    let v = vs.get("PHY001-NEWTON-RATE").unwrap();
    v.check(est.arcsec_per_century()).unwrap();
    assert_eq!(
        st.unconverged, 0,
        "predictor-corrector failed to converge on accepted steps"
    );
}
