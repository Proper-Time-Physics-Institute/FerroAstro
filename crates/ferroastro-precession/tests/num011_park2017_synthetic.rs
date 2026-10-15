// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Offline checks of the Park-2017 estimator (NUM-011) on synthetic, exactly known trajectories:
//! a Kepler ellipse whose argument of perihelion is driven as `ω(t) = ω₀ + ω̇t + A sin(νt)`.
//! The velocity is `R(ω) v_Kepler + ω̇_inst ẑ × r`, so `r·v = r'·v'` and passages occur
//! exactly at `E = 0`. The true perihelion longitude at each passage is `ω(t_p)`. These fixtures
//! test the algorithm, not physics, so their numbers are arbitrary and dimensionless in spirit.

#![expect(
    clippy::unwrap_used,
    clippy::suboptimal_flops,
    reason = "test code: failures are test failures, readability over fused operations"
)]

use ferroastro_core::frames::{EclipticJ2000, Icrf};
use ferroastro_core::time::{DAYS_PER_JULIAN_CENTURY, J2000_JD, SECONDS_PER_DAY};
use ferroastro_core::units::Angle;
use ferroastro_precession::kepler::eccentric_anomaly;
use ferroastro_precession::park2017::{
    FftConfig, FitModel, Frequency, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane,
    SampledOrbit, fft_frequencies, find_passages, fit_longitudes, perihelion_longitudes,
};
use ferroastro_vectors::VectorSet;
use std::f64::consts::TAU;

const CENTURY_S: f64 = DAYS_PER_JULIAN_CENTURY * SECONDS_PER_DAY;

/// Synthetic orbit parameters (arbitrary fixture values, not physics).
struct Synth {
    period_s: f64,
    e: f64,
    a_km: f64,
    omega0: f64,
    omega_dot_per_cy: f64,
    amp: f64,
    nu_per_cy: f64,
    incl: f64,
    node: f64,
}

impl Synth {
    fn omega(&self, t_s: f64) -> (f64, f64) {
        let tc = t_s / CENTURY_S;
        let w = self.omega0 + self.omega_dot_per_cy * tc + self.amp * (self.nu_per_cy * tc).sin();
        let wdot = (self.omega_dot_per_cy
            + self.amp * self.nu_per_cy * (self.nu_per_cy * tc).cos())
            / CENTURY_S;
        (w, wdot)
    }

    /// Heliocentric state at `t_s` in the reference frame (orbit inclined by `incl` about the
    /// node line at longitude `node`).
    fn state(&self, t_s: f64) -> ([f64; 3], [f64; 3]) {
        let n = TAU / self.period_s;
        let m = (n * t_s).rem_euclid(TAU);
        let big_e = eccentric_anomaly(m, self.e).unwrap();
        let (se, ce) = big_e.sin_cos();
        let s1 = (1.0 - self.e * self.e).sqrt();
        let (xp, yp) = (self.a_km * (ce - self.e), self.a_km * s1 * se);
        let den = 1.0 - self.e * ce;
        let (vxp, vyp) = (-self.a_km * n * se / den, self.a_km * n * s1 * ce / den);
        let (w, wdot) = self.omega(t_s);
        let (sw, cw) = w.sin_cos();
        // In-plane rotation by ω, plus the rotation rate term ω̇ ẑ' × r'.
        let (x, y) = (cw * xp - sw * yp, sw * xp + cw * yp);
        let (vx, vy) = (
            cw * vxp - sw * vyp - wdot * y,
            sw * vxp + cw * vyp + wdot * x,
        );
        // Tilt by incl about the x-axis, then rotate by node about z.
        let tilt = |p: [f64; 3]| {
            let (si, ci) = self.incl.sin_cos();
            let q = [p[0], ci * p[1] - si * p[2], si * p[1] + ci * p[2]];
            let (so, co) = self.node.sin_cos();
            [co * q[0] - so * q[1], so * q[0] + co * q[1], q[2]]
        };
        (tilt([x, y, 0.0]), tilt([vx, vy, 0.0]))
    }

    fn sample(&self, orbits: u32, per_orbit: u32) -> (Vec<f64>, Vec<[f64; 3]>, Vec<[f64; 3]>) {
        let dt = self.period_s / f64::from(per_orbit);
        // Start a little after a passage so that the first window is complete.
        let t0 = -0.3 * self.period_s; // lint: allow-literal(arbitrary start phase)
        let (mut jd, mut r, mut v) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..orbits * per_orbit {
            let t = t0 + f64::from(k) * dt;
            let (rr, vv) = self.state(t);
            jd.push(J2000_JD + t / SECONDS_PER_DAY);
            r.push(rr);
            v.push(vv);
        }
        (jd, r, v)
    }
}

fn fixture() -> Synth {
    Synth {
        period_s: CENTURY_S / 400.0, // lint: allow-literal(math or algorithm fixture, not physics)
        e: 0.25,
        a_km: 1.0e7, // lint: allow-literal(math or algorithm fixture, not physics)
        omega0: 1.0,
        omega_dot_per_cy: 0.01, // lint: allow-literal(fixture rate, rad per century)
        amp: 0.0,
        nu_per_cy: 1.0,
        incl: 0.0,
        node: 0.0,
    }
}

const ARCSEC_PER_RAD: f64 = 180.0 * 3600.0 / std::f64::consts::PI; // lint: allow-literal(math or algorithm fixture, not physics)

/// A uniformly precessing ellipse: every passage is found at `E = 0` and the fitted rate equals
/// the driven one.
///
/// Verifies: NUM-011
#[test]
fn recovers_a_driven_precession_rate_exactly() {
    let s = fixture();
    let (jd, r, v) = s.sample(400, 40);
    let orbit = SampledOrbit::<EclipticJ2000>::from_raw(&jd, &r, &v).unwrap();
    let est = Park2017 {
        plane: ReferencePlane::<EclipticJ2000>::xy_plane(),
        definition: LongitudeDefinition::Projected,
        passages: PassageConfig::default(),
        model: FitModel {
            quadratic: true,
            frequencies: vec![],
        },
        jackknife_blocks: 20,
    };
    let res = est.estimate(&orbit).unwrap();
    // Passages at multiples of the period (E = 0 at t = k·P).
    for p in &res.passages {
        let k = (p.t_s / s.period_s).round();
        assert!(
            (p.t_s - k * s.period_s).abs() < 1e-3, // lint: allow-literal(math or algorithm fixture, not physics)
            "passage time {}",
            p.t_s
        ); // lint: allow-literal(1 ms on a 79-day orbit)
        assert!(p.direction_error_rad < 1e-9, "{}", p.direction_error_rad); // lint: allow-literal(40 samples per orbit)
    }
    assert!(res.passages.len() >= 395); // lint: allow-literal(400 orbits minus window edges)
    let want = s.omega_dot_per_cy * ARCSEC_PER_RAD;
    let got = res.fit.arcsec_per_century();
    assert!((got / want - 1.0).abs() < 1e-9, "{got} vs {want}"); // lint: allow-literal(interpolation-limited)
    assert!(res.fit.q_arcsec_per_cy2.abs() < 1e-6 * want); // lint: allow-literal(no quadratic term driven)
    assert!(res.fit.jackknife_sigma_arcsec_per_century() < 1e-6 * want); // lint: allow-literal(exact data)
    // The report carries the PHY-015 metadata.
    let rep = res
        .report::<ferroastro_core::frames::Heliocentric>(
            "synthetic",
            "none",
            "none",
            "kinematic fixture",
        )
        .unwrap();
    assert!(rep.frame.contains("ECLIPJ2000") && rep.fit_model.contains("Q*t^2"));
}

/// A periodic term with a known frequency is recovered by the supplied-frequency fit and by the
/// FFT search, and omitting it biases neither the search nor the final rate.
///
/// Verifies: NUM-011
#[test]
fn recovers_periodic_terms_supplied_and_fft_derived() {
    let span_cy = 1.0;
    let s = Synth {
        amp: 2e-5,                      // lint: allow-literal(fixture amplitude, about 4 arcsec)
        nu_per_cy: TAU * 7.3 / span_cy, // lint: allow-literal(7.3 cycles over the span)
        ..fixture()
    };
    let (jd, r, v) = s.sample(400, 40);
    let orbit = SampledOrbit::<EclipticJ2000>::from_raw(&jd, &r, &v).unwrap();
    let passages = find_passages(&orbit, &PassageConfig::default()).unwrap();
    let plane = ReferencePlane::<EclipticJ2000>::xy_plane();
    let lon = perihelion_longitudes(&passages, &plane, LongitudeDefinition::Projected).unwrap();
    let t: Vec<f64> = passages.iter().map(|p| p.t_s).collect();
    let want = s.omega_dot_per_cy * ARCSEC_PER_RAD;
    let supplied = FitModel {
        quadratic: true,
        frequencies: vec![Frequency {
            rad_per_century: s.nu_per_cy,
            label: "fixture".into(),
        }],
    };
    let f = fit_longitudes(&t, &lon, &supplied, 10).unwrap();
    assert!((f.arcsec_per_century() / want - 1.0).abs() < 1e-9); // lint: allow-literal(exact model)
    let amp = f.terms[0].sin_arcsec.hypot(f.terms[0].cos_arcsec);
    assert!((amp / (s.amp * ARCSEC_PER_RAD) - 1.0).abs() < 1e-8, "{amp}"); // lint: allow-literal(exact model)
    let found = fft_frequencies(
        &t,
        &lon,
        &FftConfig {
            min_amplitude: Angle::from_arcsec(0.5).unwrap(),
            max_terms: 5,
            min_cycles: 2.0,
            oversample: 16,
            quadratic: true,
        },
    )
    .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!((found[0].rad_per_century / s.nu_per_cy - 1.0).abs() < 1e-6); // lint: allow-literal(golden-section on 400 samples)
    let g = fit_longitudes(
        &t,
        &lon,
        &FitModel {
            quadratic: true,
            frequencies: found,
        },
        10,
    )
    .unwrap();
    assert!((g.arcsec_per_century() / want - 1.0).abs() < 1e-6); // lint: allow-literal(frequency known to 1e-6)
    // Without the term, the rate is biased: the test would notice a broken periodic model.
    let plain = FitModel {
        quadratic: true,
        frequencies: vec![],
    };
    let h = fit_longitudes(&t, &lon, &plain, 10).unwrap();
    assert!((h.arcsec_per_century() / want - 1.0).abs() > 1e-6); // lint: allow-literal(bias exceeds the fit precision)
    assert!(h.jackknife_sigma_arcsec_per_century() > f.jackknife_sigma_arcsec_per_century());
}

/// For an inclined orbit, the dog-leg longitude is `Ω + ω` and the projected longitude obeys
/// `tan(λ − Ω) = cos i · tan ω`; the two differ, so their rates differ.
///
/// Verifies: NUM-011, NUM-012
#[test]
fn dog_leg_and_projected_longitudes_differ_as_spherical_trigonometry_says() {
    let s = Synth {
        incl: 0.12,  // lint: allow-literal(fixture inclination, about 7 degrees)
        node: 0.84,  // lint: allow-literal(fixture node longitude)
        omega0: 0.5, // fixture argument of perihelion
        ..fixture()
    };
    let (jd, r, v) = s.sample(40, 40);
    let orbit = SampledOrbit::<EclipticJ2000>::from_raw(&jd, &r, &v).unwrap();
    let passages = find_passages(&orbit, &PassageConfig::default()).unwrap();
    let plane = ReferencePlane::<EclipticJ2000>::xy_plane();
    let dog = perihelion_longitudes(&passages, &plane, LongitudeDefinition::DogLeg).unwrap();
    let proj = perihelion_longitudes(&passages, &plane, LongitudeDefinition::Projected).unwrap();
    for ((p, d), l) in passages.iter().zip(&dog).zip(&proj) {
        let (w, _) = s.omega(p.t_s);
        let d_true = s.node + w;
        assert!(((d - d_true) / TAU).round().mul_add(-TAU, d - d_true).abs() < 1e-10); // lint: allow-literal(interpolation precision)
        let l_true = s.node + (s.incl.cos() * w.sin()).atan2(w.cos());
        assert!(((l - l_true) / TAU).round().mul_add(-TAU, l - l_true).abs() < 1e-10); // lint: allow-literal(interpolation precision)
    }
    // A planar orbit has no node: the dog-leg definition must refuse it.
    let flat = fixture();
    let (jd, r, v) = flat.sample(4, 40);
    let orbit = SampledOrbit::<EclipticJ2000>::from_raw(&jd, &r, &v).unwrap();
    let passages = find_passages(&orbit, &PassageConfig::default()).unwrap();
    assert!(perihelion_longitudes(&passages, &plane, LongitudeDefinition::DogLeg).is_err());
}

/// Bad inputs are refused with FA-E0603 (estimator input) or FA-E0601 (degenerate fit).
///
/// Verifies: NUM-011, API-002
#[test]
fn refuses_coarse_unsorted_or_degenerate_input() {
    use ferroastro_core::ErrorCode;
    let s = fixture();
    let (jd, r, v) = s.sample(40, 10);
    let coarse = SampledOrbit::<EclipticJ2000>::from_raw(&jd, &r, &v).unwrap();
    let e = find_passages(&coarse, &PassageConfig::default()).unwrap_err();
    assert_eq!(e.code(), ErrorCode::EstimatorInput, "{e}");
    let mut jd2 = jd.clone();
    jd2.swap(3, 4);
    let e = SampledOrbit::<EclipticJ2000>::from_raw(&jd2, &r, &v).unwrap_err();
    assert_eq!(e.code(), ErrorCode::EstimatorInput);
    let e = SampledOrbit::<EclipticJ2000>::from_raw(&jd[..5], &r[..5], &v[..5]).unwrap_err();
    assert_eq!(e.code(), ErrorCode::EstimatorInput);
    let t: Vec<f64> = (0..50).map(f64::from).collect();
    let dup = FitModel {
        quadratic: false,
        frequencies: vec![
            Frequency {
                rad_per_century: 1.0,
                label: "a".into(),
            },
            Frequency {
                rad_per_century: 1.0,
                label: "b".into(),
            },
        ],
    };
    let e = fit_longitudes(&t, &t, &dup, 5).unwrap_err();
    assert_eq!(e.code(), ErrorCode::FitDegenerate);
    assert_eq!(
        fit_longitudes(&t, &t, &dup, 1).unwrap_err().code(),
        ErrorCode::FitDegenerate
    );
}

/// The built-in mean-orbit pole equals the transcribed Park17 values and yields Park's triad:
/// orthonormal, right-handed, y perpendicular to the ICRF x-axis.
///
/// Verifies: NUM-011
#[test]
fn mean_orbit_frame_matches_park17_definition() {
    let vs = VectorSet::load_workspace().unwrap();
    let ra = vs.get("PARK17-MEANORBIT-POLE-RA").unwrap();
    let dec = vs.get("PARK17-MEANORBIT-POLE-DEC").unwrap();
    ra.check(ferroastro_precession::park2017::PARK2017_POLE_RA_DEG)
        .unwrap();
    dec.check(ferroastro_precession::park2017::PARK2017_POLE_DEC_DEG)
        .unwrap();
    let plane = ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap();
    let z = plane.pole();
    let (x, y) = plane.axes();
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    for (a, b) in [(x, y), (y, z), (z, x)] {
        assert!(dot(a, b).abs() < 1e-15); // lint: allow-literal(orthogonality to round-off)
    }
    for a in [x, y, z] {
        assert!((dot(a, a) - 1.0).abs() < 1e-15); // lint: allow-literal(unit length to round-off)
    }
    assert!(y[0].abs() < 1e-15); // lint: allow-literal(y is perpendicular to the ICRF x-axis)
    let zx = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    assert!(dot(zx, z) > 0.0);
    let dec_back = z[2].asin().to_degrees();
    let ra_back = z[1].atan2(z[0]).to_degrees().rem_euclid(360.0); // lint: allow-literal(math or algorithm fixture, not physics)
    assert!((dec_back - dec.value().unwrap()).abs() < 1e-12); // lint: allow-literal(round trip)
    assert!((ra_back - ra.value().unwrap()).abs() < 1e-12); // lint: allow-literal(round trip)
    assert!(plane.label().contains("280.9876") && plane.label().contains("61.4481"));
}
