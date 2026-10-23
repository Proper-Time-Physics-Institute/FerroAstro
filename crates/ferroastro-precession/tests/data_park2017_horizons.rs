// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real observational-data checks of the Park2017 estimator (NUM-011) on JPL's DE441, served by
//! the Horizons API (PHY-016 groundwork). These tests are network-dependent (`data` tier).
//!
//! * (a) Fixed J2000 ecliptic, 1800–2050: the dog-leg `ϖ = Ω + ω` rate vs SW92 Table 1.
//! * (b) Park17 mean-orbit frame, J2000 ± 1000 yr: the projected perihelion rate vs Park17's
//!   575.3100 ± 0.0015 ″/cy, with the NUM-011 acceptance band and a 3σ consistency band.
//!
//! Every chunk's URL, retrieval date, SHA-256, `signature.version`, ephemeris source and frame
//! line are printed. Pre-registration and derivations: IMPLEMENTATION-NOTES.md, WP5.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::suboptimal_flops,
    reason = "test code: any failure is a test failure; the data test reads top to bottom"
)]

use ferroastro_core::frames::{EclipticJ2000, Frame, Heliocentric, Icrf};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, J2000_JD};
use ferroastro_precession::park2017::{
    FftConfig, FitModel, Frequency, LongitudeDefinition, Park2017, Park2017Result, PassageConfig,
    ReferencePlane, SampledOrbit, fft_frequencies, fit_longitudes,
};
use ferroastro_testdata::horizons::{RefPlane, VectorQuery, VectorTable, fetch_vectors};
use ferroastro_vectors::VectorSet;

/// Gauge / dynamics labels for reports: the trajectory is JPL's, not ours.
const GAUGE: &str = "DE441 as integrated by JPL (PPN, isotropic/harmonic gauge, beta = gamma = 1)";
const DYNAMICS: &str = "JPL DE441 ephemeris (Park et al. 2021), Mercury heliocentric state vectors \
     sampled by the Horizons API; no FerroAstro force model involved";

fn value(vs: &VectorSet, id: &str) -> f64 {
    vs.get(id).unwrap().value().unwrap()
}

fn mercury(start_jd: f64, stop_jd: f64, step_days: u32, plane: RefPlane) -> VectorTable {
    let q = VectorQuery {
        command: "199".into(),
        center: "500@10".into(),
        ref_plane: plane,
        start_jd_tdb: start_jd,
        stop_jd_tdb: stop_jd,
        step_days,
    };
    let t = fetch_vectors(&q).unwrap();
    for c in &t.chunks {
        println!("{}", c.line());
    }
    let sources = t.sources();
    println!(
        "Horizons table: {} rows, JD {} .. {} TDB, step {step_days} d, REF_PLANE {}, \
         signature.version {:?}, ephemeris source {:?}",
        t.jd_tdb.len(),
        t.jd_tdb[0],
        t.jd_tdb[t.jd_tdb.len() - 1],
        plane.param(),
        t.versions(),
        sources
    );
    assert!(
        sources.len() == 1 && sources[0].starts_with("DE"),
        "unexpected ephemeris source(s) {sources:?}"
    );
    t
}

fn constants_label(t: &VectorTable) -> String {
    format!(
        "JPL {} via Horizons API v{} (answers retrieved {}; per-chunk SHA-256 printed above)",
        t.sources().join("+"),
        t.versions().join("+"),
        t.chunks[0].fetched.retrieved
    )
}

/// Park17 Table 2: the 14 frequencies as integer combinations of the mean motions of
/// (Mercury, Venus, Earth, Jupiter, Saturn), in the table's order.
const PARK17_TABLE2: [(i32, i32, i32, i32, i32, &str); 14] = [
    (0, 2, 0, 0, 0, "0nM + 2nV"),
    (0, 1, 0, 0, 0, "0nM + 1nV"),
    (1, -2, 0, 0, 0, "1nM - 2nV"),
    (2, -3, 0, 0, 0, "2nM - 3nV"),
    (1, -3, 0, 0, 0, "1nM - 3nV"),
    (2, -4, 0, 0, 0, "2nM - 4nV"),
    (2, -5, 0, 0, 0, "2nM - 5nV"),
    (1, 0, -2, 0, 0, "1nM - 2nE"),
    (1, 0, -4, 0, 0, "1nM - 4nE"),
    (0, 0, 0, 3, 0, "0nM + 3nJ"),
    (0, 0, 0, 2, 0, "0nM + 2nJ"),
    (0, 0, 0, 1, 0, "0nM + 1nJ"),
    (1, 0, 0, -2, 0, "1nM - 2nJ"),
    (0, 0, 0, 0, 2, "0nM + 2nS"),
];

/// Park17 Table 2 frequencies from SW92 mean motions (`table` = "T1" or "T2A").
fn park17_frequencies(vs: &VectorSet, table: &str) -> Vec<Frequency> {
    let n = |body: &str| value(vs, &format!("SW92-{table}-{body}-L-RATE")).to_radians();
    let (m, v, e, j, s) = (
        n("MERCURY"),
        n("VENUS"),
        n("EMB"),
        n("JUPITER"),
        n("SATURN"),
    );
    PARK17_TABLE2
        .iter()
        .map(|&(a, b, c, d, f, label)| Frequency {
            rad_per_century: f64::from(a) * m
                + f64::from(b) * v
                + f64::from(c) * e
                + f64::from(d) * j
                + f64::from(f) * s,
            label: format!("{label} (SW92 {table})"),
        })
        .collect()
}

fn print_result<F: Frame>(what: &str, r: &Park2017Result) {
    let f = &r.fit;
    println!(
        "{what}: varpi_dot = {:.6} arcsec/cy, formal sigma {:.6}, jack-knife sigma {:.6} ({} blocks), \
         Q = {:.8} arcsec/cy^2, B = {:.4} arcsec, rms residual {:.4} arcsec, {} passages, \
         span JD {:.1} .. {:.1}, max passage-direction error {:.3e} arcsec, frame {}",
        f.arcsec_per_century(),
        f.formal_sigma_arcsec_per_century(),
        f.jackknife_sigma_arcsec_per_century(),
        f.jackknife_blocks,
        f.q_arcsec_per_cy2,
        f.b.arcsec(),
        f.rms_residual.arcsec(),
        f.samples,
        J2000_JD + f.span_s.0 / 86_400.0, // lint: allow-literal(seconds per day, a definition)
        J2000_JD + f.span_s.1 / 86_400.0, // lint: allow-literal(seconds per day, a definition)
        r.max_direction_error_rad.to_degrees() * 3600.0, // lint: allow-literal(arcsec per degree)
        F::NAME
    );
}

/// (a) SW92 Table 1 rate of Mercury's longitude of perihelion (fixed J2000 ecliptic, fit to DE200
/// over 1800–2050) vs the dog-leg `Ω + ω` at DE441 perihelion passages over the same window,
/// linear fit. Tolerance ±20 ″/cy from SW92's 15″ fit quality (vector rationale).
///
/// Verifies: NUM-011, PHY-015, PHY-016
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_sw92_rate_from_horizons_de441_ecliptic() {
    let vs = VectorSet::load_canonical().unwrap();
    // 1800-Jan-01 0h and 2050-Jan-01 0h TDB (Gregorian), 1-day step (two chunks).
    let (start, stop) = (2_378_496.5, 2_469_807.5); // lint: allow-literal(calendar dates of SW92's fit window)
    let t = mercury(start, stop, 1, RefPlane::Ecliptic);
    let orbit = SampledOrbit::<EclipticJ2000>::from_raw(&t.jd_tdb, &t.r_km, &t.v_km_s).unwrap();
    let est = Park2017 {
        plane: ReferencePlane::<EclipticJ2000>::xy_plane(),
        definition: LongitudeDefinition::DogLeg,
        passages: PassageConfig::default(),
        model: FitModel {
            quadratic: false,
            frequencies: vec![],
        },
        jackknife_blocks: 10,
    };
    let r = est.estimate(&orbit).unwrap();
    print_result::<EclipticJ2000>("(a) dog-leg, J2000 ecliptic, linear, 1800-2050", &r);
    let report = r
        .report::<Heliocentric>(
            "Mercury varpi_dot, DE441, J2000 ecliptic (SW92 comparison)",
            &constants_label(&t),
            GAUGE,
            DYNAMICS,
        )
        .unwrap();
    println!("{report}");

    let oracle = vs.get("SW92-MERCURY-VARPI-RATE-VS-DE441").unwrap();
    let got = r.fit.arcsec_per_century();
    let t2a = value(&vs, "SW92-T2A-MERCURY-VARPI-RATE") * 3600.0; // lint: allow-literal(arcsec per degree)
    let varpi0 = value(&vs, "SW92-MERCURY-VARPI") * 3600.0; // lint: allow-literal(arcsec per degree)
    let b = r.fit.b.arcsec().rem_euclid(1_296_000.0); // lint: allow-literal(arcsec per full turn)
    println!(
        "(a) DE441 dog-leg rate {got:.4} - SW92 Table 1 {:.6} = {:+.4} arcsec/cy (tolerance +/- {}); \
         - SW92 Table 2a {t2a:.6} = {:+.4}; varpi(J2000) {b:.3} - SW92 {varpi0:.3} = {:+.3} arcsec",
        oracle.value().unwrap(),
        got - oracle.value().unwrap(),
        oracle.tolerance.abs.unwrap_or(0.0),
        got - t2a,
        b - varpi0
    );

    // Diagnostics (not asserted): the projected angle, and the Park17 fit model, on the same data.
    let proj = Park2017 {
        definition: LongitudeDefinition::Projected,
        ..est.clone()
    }
    .estimate(&orbit)
    .unwrap();
    print_result::<EclipticJ2000>("(a) diagnostic: projected angle, linear", &proj);
    let periodic = Park2017 {
        model: FitModel {
            quadratic: true,
            frequencies: park17_frequencies(&vs, "T2A"),
        },
        ..est.clone()
    }
    .estimate(&orbit)
    .unwrap();
    print_result::<EclipticJ2000>(
        "(a) diagnostic: dog-leg, quadratic + 14 Park17 frequencies",
        &periodic,
    );

    oracle.check(got).unwrap();
}

/// (b) Park17's total rate 575.3100 ± 0.0015 ″/cy in Mercury's mean-orbit frame vs the same
/// estimator applied to DE441 over J2000 ± 1000 Julian years (Park17's 1000–3000 span).
///
/// Verifies: NUM-011, PHY-015, PHY-016
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_park17_rate_from_horizons_de441_mean_orbit() {
    let vs = VectorSet::load_canonical().unwrap();
    let half = 1000.0 * DAYS_PER_JULIAN_YEAR; // lint: allow-literal(1000-year half-span, Park17 2000-yr fit)
    let t = mercury(J2000_JD - half, J2000_JD + half, 2, RefPlane::Frame);
    let orbit = SampledOrbit::<Icrf>::from_raw(&t.jd_tdb, &t.r_km, &t.v_km_s).unwrap();
    let est = Park2017 {
        plane: ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap(),
        definition: LongitudeDefinition::Projected,
        passages: PassageConfig::default(),
        model: FitModel {
            quadratic: true,
            frequencies: park17_frequencies(&vs, "T2A"),
        },
        jackknife_blocks: 20,
    };
    let r = est.estimate(&orbit).unwrap();
    print_result::<Icrf>(
        "(b) Park2017, mean-orbit frame, quadratic + 14 Table 2 frequencies",
        &r,
    );
    for term in &r.fit.terms {
        println!(
            "    {:<26} nu = {:>12.4} rad/cy  S = {:+8.3}  C = {:+8.3}  amplitude {:.3} arcsec",
            term.frequency.label,
            term.frequency.rad_per_century,
            term.sin_arcsec,
            term.cos_arcsec,
            term.sin_arcsec.hypot(term.cos_arcsec)
        );
    }
    let report = r
        .report::<Heliocentric>(
            "Mercury total varpi_dot, DE441, Park17 mean-orbit frame",
            &constants_label(&t),
            GAUGE,
            DYNAMICS,
        )
        .unwrap();
    println!("{report}");

    let oracle = vs.get("PARK17-TOTAL-RATE").unwrap();
    let park = oracle.value().unwrap();
    let sigma_park = value(&vs, "PARK17-TOTAL-RATE-SIGMA");
    let got = r.fit.arcsec_per_century();
    let sigma_jk = r.fit.jackknife_sigma_arcsec_per_century();
    let combined = sigma_park.hypot(sigma_jk);
    let diff = got - park;
    println!(
        "(b) DE441 {got:.5} - Park17 {park:.4} = {diff:+.5} arcsec/cy = {:+.2} combined sigma \
         (sigma_Park {sigma_park}, sigma_jk {sigma_jk:.5}); NUM-011 band +/- {}; 3-sigma band +/- {:.5}",
        diff / combined,
        oracle.tolerance.abs.unwrap_or(0.0),
        3.0 * combined
    );
    println!(
        "(b) Q = {:.8} vs Park17 Q = {} arcsec/cy^2 (reported only)",
        r.fit.q_arcsec_per_cy2,
        value(&vs, "PARK17-QUADRATIC-TERM")
    );
    // Known model differences, reported (not subtracted): DE440 J2 vs Park17 J2 scales the Park17
    // solar-oblateness contribution; Park17's fitted beta shifts GE by -(beta-1)/3 of it.
    let j2_de440 = ferroastro_constants::De440::load().unwrap().sun_j2();
    let j2_park = value(&vs, "PARK17-J2SUN");
    let j2_shift = value(&vs, "PARK17-J2-RATE") * (j2_de440 / j2_park - 1.0);
    println!(
        "(b) expected model offsets (reported only): J2 {j2_de440:.6e} vs {j2_park:.3e} -> {j2_shift:+.5} arcsec/cy"
    );

    // The interpolation error per passage must be negligible against the fit residual.
    let err_arcsec = r.max_direction_error_rad.to_degrees() * 3600.0; // lint: allow-literal(arcsec per degree)
    assert!(
        err_arcsec < 0.01 * r.fit.rms_residual.arcsec(), // lint: allow-literal(1 percent of the residual rms)
        "passage interpolation error {err_arcsec} arcsec"
    );

    // ---- sensitivity runs (reported only) ----
    let times: Vec<f64> = r.passages.iter().map(|p| p.t_s).collect();
    let refit = |what: &str, model: &FitModel, blocks: usize| {
        let f = fit_longitudes(&times, &r.longitudes_rad, model, blocks).unwrap();
        println!(
            "(b) sensitivity {what}: {:.5} arcsec/cy ({:+.5}), jack-knife sigma {:.5}, formal {:.5}, Q {:.6}",
            f.arcsec_per_century(),
            f.arcsec_per_century() - got,
            f.jackknife_sigma_arcsec_per_century(),
            f.formal_sigma_arcsec_per_century(),
            f.q_arcsec_per_cy2
        );
    };
    refit(
        "linear only",
        &FitModel {
            quadratic: false,
            frequencies: vec![],
        },
        20,
    );
    refit(
        "quadratic only",
        &FitModel {
            quadratic: true,
            frequencies: vec![],
        },
        20,
    );
    refit(
        "SW92 Table 1 mean motions",
        &FitModel {
            quadratic: true,
            frequencies: park17_frequencies(&vs, "T1"),
        },
        20,
    );
    refit("10 jack-knife blocks", &est.model, 10);
    refit("40 jack-knife blocks", &est.model, 40);
    let fft = fft_frequencies(
        &times,
        &r.longitudes_rad,
        &FftConfig {
            min_amplitude: ferroastro_core::units::Angle::from_arcsec(0.5).unwrap(),
            max_terms: 40,
            min_cycles: 2.0,
            oversample: 16,
            quadratic: true,
        },
    )
    .unwrap();
    println!("(b) FFT-derived frequencies: {}", fft.len());
    for f in &fft {
        println!("    {} nu = {:.4} rad/cy", f.label, f.rad_per_century);
    }
    refit(
        "FFT-derived frequencies",
        &FitModel {
            quadratic: true,
            frequencies: fft,
        },
        20,
    );
    let coarse = est.estimate(&orbit.decimate(2).unwrap()).unwrap();
    println!(
        "(b) sensitivity 4-day sampling: {:.5} arcsec/cy ({:+.5}), max passage-direction error {:.3e} arcsec",
        coarse.fit.arcsec_per_century(),
        coarse.fit.arcsec_per_century() - got,
        coarse.max_direction_error_rad.to_degrees() * 3600.0 // lint: allow-literal(arcsec per degree)
    );

    // ---- assertions (pre-registered, IMPLEMENTATION-NOTES WP5) ----
    oracle.check(got).unwrap();
    assert!(
        diff.abs() <= 3.0 * combined,
        "DE441 rate {got} differs from Park17 {park} by {diff}, more than 3 combined sigma ({})",
        3.0 * combined
    );
}
