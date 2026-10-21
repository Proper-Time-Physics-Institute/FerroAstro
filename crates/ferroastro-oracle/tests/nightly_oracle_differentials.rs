// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP17 checks 4 and 5 (nightly, local): one Park2017 estimator, applied by FerroAstro's tooling
//! to every tool's Mercury series (CI-009; NUM-011 canonical, OI-68).
//!
//! * **P-add Venus** = ϖ̇(Sun + Mercury + Venus) − ϖ̇(Sun + Mercury), Newtonian, header.440 ICs,
//!   2000 Julian yr forward from JDEPOC, daily samples. Swiftest (WHM), Mercury6 (RA15) and
//!   REBOUND (IAS15) against FerroAstro IAS15 on the same two scenario files:
//!   `ORACLE-PADD-VENUS-TOOL-AGREEMENT` (PROVISIONAL 1e-3 ″/cy). Each value is also reported
//!   against Park17 Table 3 (`PARK17-T3-VENUS`) as information only: that vector gates P-remove
//!   in the PHY-012 test, and P-add is never asserted against Park17 (ADR-0014).
//! * **C-lite** (Swiftest only): the GR contribution ϖ̇(GR on) − ϖ̇(GR off) of the Sun + Mercury
//!   test particle, Swiftest's `general_relativity` against FerroAstro's `gr_1pn_central`:
//!   `ORACLE-CLITE-SWIFTEST-GR-AGREEMENT` (PROVISIONAL 1e-3 ″/cy).
//!
//! The oracle series are not committed (each P-add series is ~100 MB): they are produced locally
//! by `tools/oracles/nightly.sh` into `$FA_ORACLE_OUT_DIR` (default `target/oracle-out`), as
//! `<tool>/<scenario>/` with a MANIFEST.json, which this test verifies (CI-003) together with the
//! scenario and header.440 hashes the oracle ran on.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers: a failed unwrap, panic or out-of-range index is a test failure"
)]

use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_diffrun::{Estimate, Estimator, PassageSource, Track};
use ferroastro_oracle::{GoldenStates, Manifest, sha256_hex};
use ferroastro_precession::park2017::{
    FitModel, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane,
    park17_table2_frequencies,
};
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::{BodyName, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const PADD_BASE: &str = "oracle-padd-sun-mercury-2000yr";
const PADD_VENUS: &str = "oracle-padd-sun-mercury-venus-2000yr";
const CLITE_OFF: &str = "oracle-clite-sun-mercury-tp-newton-252yr";
const CLITE_ON: &str = "oracle-clite-sun-mercury-tp-gr1pn-252yr";
const MARGIN_DAYS: f64 = 30.0; // lint: allow-literal(fit-window margin inside the span, as WP14)
const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as WP14)
const NAIF_SUN: i32 = 10;
const NAIF_MERCURY: i32 = 1;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle_out() -> PathBuf {
    std::env::var_os("FA_ORACLE_OUT_DIR")
        .map_or_else(|| root().join("target/oracle-out"), PathBuf::from)
}

fn scenario_text(name: &str) -> String {
    std::fs::read_to_string(root().join("ci/scenarios").join(format!("{name}.toml"))).unwrap()
}

fn header() -> (Header, String, String) {
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    let prov = f.provenance();
    println!("de440-header: {prov}");
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    (h, prov, f.sha256)
}

/// The WP14 estimator: Park17 mean-orbit plane, projected longitude, sampled-state passages,
/// quadratic + the 14 Table-2 terms (SW92 Table 2a mean motions), 20 jack-knife blocks.
fn estimator(vs: &VectorSet, window_jd: (f64, f64)) -> Estimator {
    let n = |b: &str| {
        vs.get(&format!("SW92-T2A-{b}-L-RATE"))
            .unwrap()
            .value()
            .unwrap()
    };
    Estimator {
        park: Park2017 {
            plane: ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap(),
            definition: LongitudeDefinition::Projected,
            passages: PassageConfig::default(),
            model: FitModel {
                quadratic: true,
                frequencies: park17_table2_frequencies(
                    [
                        n("MERCURY"),
                        n("VENUS"),
                        n("EMB"),
                        n("JUPITER"),
                        n("SATURN"),
                    ],
                    "SW92 T2A",
                ),
            },
            jackknife_blocks: JACKKNIFE_BLOCKS,
        },
        source: PassageSource::SampledStates,
        body: BodyName::Mercury,
        central: BodyName::Sun,
        window_jd: Some(window_jd),
    }
}

/// Precondition of the nightly differentials (progress review after WP18, R3): every oracle
/// series they read must exist under [`oracle_out`]. A missing series is reported as a distinct,
/// named **precondition** failure (a configuration problem: the oracle step did not run or wrote
/// elsewhere), before any FerroAstro run and never as a physics disagreement.
fn require_oracle_series(series: &[(&str, &str)]) {
    let missing: Vec<String> = series
        .iter()
        .filter(|(tool, scenario)| !oracle_out().join(tool).join(scenario).is_dir())
        .map(|(tool, scenario)| format!("{tool}/{scenario}"))
        .collect();
    assert!(
        missing.is_empty(),
        "PRECONDITION FAILED (missing oracle series, not a physics result): {} under {} \
         (FA_ORACLE_OUT_DIR); run tools/oracles/nightly.sh, or point FA_ORACLE_OUT_DIR at a \
         complete oracle output directory",
        missing.join(", "),
        oracle_out().display()
    );
}

/// Mercury relative to the Sun from an oracle's state CSV (heliocentric, or SSB with the Sun's
/// rows), after verifying its manifest and the inputs it ran on.
fn oracle_track(tool: &str, scenario: &str, header_sha: &str) -> Option<Track> {
    let dir = oracle_out().join(tool).join(scenario);
    if !dir.is_dir() {
        return None;
    }
    let m = Manifest::verify_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let file = format!("{scenario}.{tool}.states.csv");
    let entry = m.entry(&file).unwrap();
    entry
        .check_input(
            &format!("{scenario}.toml"),
            &sha256_hex(scenario_text(scenario).as_bytes()),
        )
        .unwrap();
    entry.check_input("header.440", header_sha).unwrap();
    println!(
        "{tool}/{scenario}: {} image {} runner {} wall {} s {:?}",
        entry.version, entry.image_digest, entry.runner_sha, entry.wall_clock_s, entry.extra
    );
    let g = GoldenStates::parse(&file, &std::fs::read_to_string(dir.join(&file)).unwrap()).unwrap();
    let center = g.center_naif().unwrap();
    let mut track = Track::new(g.meta_f64("epoch_jd_tdb").unwrap());
    let mut sun: Option<(f64, [f64; 3], [f64; 3])> = None;
    for row in &g.rows {
        match (center, row.naif) {
            (NAIF_SUN, NAIF_MERCURY) => {
                track.t.push(row.t_days * SECONDS_PER_DAY);
                track.r.push(row.position_km);
                track.v.push(row.velocity_km_s);
            }
            (0, NAIF_SUN) => sun = Some((row.t_days, row.position_km, row.velocity_km_s)),
            (0, NAIF_MERCURY) => {
                let (ts, ps, vs) = sun.expect("the Sun's row precedes Mercury's");
                assert_eq!(ts.to_bits(), row.t_days.to_bits());
                track.t.push(row.t_days * SECONDS_PER_DAY);
                track.r.push([0, 1, 2].map(|k| row.position_km[k] - ps[k]));
                track
                    .v
                    .push([0, 1, 2].map(|k| row.velocity_km_s[k] - vs[k]));
            }
            _ => {}
        }
    }
    assert!(!track.t.is_empty(), "{tool}/{scenario}: no Mercury rows");
    Some(track)
}

/// FerroAstro IAS15 on the same scenario file: Mercury relative to the Sun at the same samples.
fn ferroastro_track(scenario: &str, h: &Header, prov: &str) -> (Track, String) {
    let spec = ScenarioSpec::parse(&scenario_text(scenario)).unwrap();
    let out = run(&spec, &DataSources::none().with_header(h, prov)).unwrap();
    let mut track = Track::new(out.epoch_jd_tdb[0] + out.epoch_jd_tdb[1]);
    let missing = track.absorb(&out, NAIF_MERCURY, NAIF_SUN);
    assert_eq!(missing, 0);
    println!(
        "FerroAstro {scenario}: {} IAS15 steps, IC hash {}, {} samples",
        out.stats.steps,
        out.ic_hash,
        track.t.len()
    );
    (track, out.ic_hash)
}

fn window(scenario: &str, epoch_jd: f64) -> (f64, f64) {
    let spec = ScenarioSpec::parse(&scenario_text(scenario)).unwrap();
    let span_days = spec.span.years * ferroastro_core::time::DAYS_PER_JULIAN_YEAR;
    (epoch_jd + MARGIN_DAYS, epoch_jd + span_days - MARGIN_DAYS)
}

fn show(label: &str, e: &Estimate) {
    println!(
        "  {label:<34} {:>14.6} ″/cy  σ_jk {:.2e}  passages {}  rms {:.2e}″  max interp err {:.1e}″",
        e.rate, e.jackknife_sigma, e.passages, e.rms_residual, e.max_direction_error_arcsec
    );
}

/// P-add Venus from Swiftest, Mercury6 and REBOUND series vs FerroAstro; Park17 reported as
/// information only (ADR-0014: PHY-012 asserts P-remove in `nightly_phy012`, not here).
///
/// Verifies: CI-009, REF-007, REF-008, REF-005
#[test]
#[ignore = "nightly: 2000-yr FerroAstro runs and ~100 MB oracle series from tools/oracles/nightly.sh; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_oracle_padd_venus_same_estimator_every_tool() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov, header_sha) = header();
    let jdepoc = h.value("JDEPOC").unwrap();
    let win = window(PADD_BASE, jdepoc);
    assert_eq!(win, window(PADD_VENUS, jdepoc));
    let est = estimator(&vs, win);
    println!("estimator: {}", est.id(PassageSource::SampledStates));
    require_oracle_series(&[
        ("swiftest", PADD_BASE),
        ("swiftest", PADD_VENUS),
        ("mercury6", PADD_BASE),
        ("mercury6", PADD_VENUS),
        ("rebound", PADD_BASE),
        ("rebound", PADD_VENUS),
    ]);
    let (fb, hash_b) = ferroastro_track(PADD_BASE, &h, &prov);
    let (fv, hash_v) = ferroastro_track(PADD_VENUS, &h, &prov);
    println!(
        "IC hashes: base {hash_b}, venus {hash_v} (differ by Venus only; per-body records are the header's)"
    );
    let eb = est.estimate(&fb, PassageSource::SampledStates).unwrap();
    let ev = est.estimate(&fv, PassageSource::SampledStates).unwrap();
    show("FerroAstro Sun+Mercury", &eb);
    show("FerroAstro Sun+Mercury+Venus", &ev);
    let fa = ev.rate - eb.rate;
    println!("FerroAstro P-add Venus = {fa:.6} ″/cy");

    let agree = vs.get("ORACLE-PADD-VENUS-TOOL-AGREEMENT").unwrap();
    let park = vs.get("PARK17-T3-VENUS").unwrap();
    let park_value = park.value().unwrap();
    let park_tol = park.tolerance.allowed(park_value);
    // Information only: PHY-012 asserts P-remove (ADR-0014, PROVISIONAL under OI-38); P-add is
    // never checked against Park17. If OI-38 reverts the protocol to P-add, this becomes a check.
    let report_park = |who: &str, x: f64| {
        let d = x - park_value;
        println!(
            "  {who} P-add Venus {x:.6} vs Park17 {park_value}: {d:+.6} ″/cy, margin {:.2} (information only, ADR-0014)",
            d.abs() / park_tol,
        );
    };
    report_park("FerroAstro", fa);
    let mut ran = Vec::new();
    for tool in ["swiftest", "mercury6", "rebound"] {
        let (Some(tb), Some(tv)) = (
            oracle_track(tool, PADD_BASE, &header_sha),
            oracle_track(tool, PADD_VENUS, &header_sha),
        ) else {
            unreachable!("{tool}: checked by require_oracle_series");
        };
        assert_eq!(tb.t.len(), fb.t.len(), "{tool}: sample count");
        let ob = est.estimate(&tb, PassageSource::SampledStates).unwrap();
        let ov = est.estimate(&tv, PassageSource::SampledStates).unwrap();
        println!("{tool}:");
        show("Sun+Mercury", &ob);
        show("Sun+Mercury+Venus", &ov);
        let x = ov.rate - ob.rate;
        let d = x - fa;
        println!(
            "  {tool} P-add Venus {x:.6} ″/cy; minus FerroAstro {d:+.3e} ″/cy (margin {:.3})",
            d.abs() / agree.tolerance.abs.unwrap()
        );
        report_park(tool, x);
        ran.push((tool, d));
    }
    for (tool, d) in ran {
        agree
            .check(d)
            .unwrap_or_else(|e| panic!("{tool}: P-add Venus differs from FerroAstro: {e}"));
    }
}

/// Swiftest C-lite: the GR contribution (GR on − GR off) vs FerroAstro's `gr_1pn_central`.
///
/// Verifies: CI-009, REF-007, PHY-005
#[test]
#[ignore = "nightly: 252-yr test-particle runs and the Swiftest C-lite series from tools/oracles/nightly.sh; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_oracle_clite_swiftest_gr_contribution() {
    let vs = VectorSet::load_canonical().unwrap();
    let (h, prov, header_sha) = header();
    let jdepoc = h.value("JDEPOC").unwrap();
    let win = window(CLITE_OFF, jdepoc);
    let est = estimator(&vs, win);
    require_oracle_series(&[("swiftest", CLITE_OFF), ("swiftest", CLITE_ON)]);
    let (f_off, hash_off) = ferroastro_track(CLITE_OFF, &h, &prov);
    let (f_on, hash_on) = ferroastro_track(CLITE_ON, &h, &prov);
    assert_eq!(
        hash_off, hash_on,
        "CI-009: identical IC hashes for GR on and off"
    );
    let e_off = est.estimate(&f_off, PassageSource::SampledStates).unwrap();
    let e_on = est.estimate(&f_on, PassageSource::SampledStates).unwrap();
    show("FerroAstro GR off", &e_off);
    show("FerroAstro GR on", &e_on);
    let fa = e_on.rate - e_off.rate;
    let s_off = oracle_track("swiftest", CLITE_OFF, &header_sha).expect("Swiftest C-lite GR off");
    let s_on = oracle_track("swiftest", CLITE_ON, &header_sha).expect("Swiftest C-lite GR on");
    let o_off = est.estimate(&s_off, PassageSource::SampledStates).unwrap();
    let o_on = est.estimate(&s_on, PassageSource::SampledStates).unwrap();
    show("Swiftest GR off", &o_off);
    show("Swiftest GR on", &o_on);
    let sw = o_on.rate - o_off.rate;
    let schw = vs.get("PHY004-GR-RATE").ok().and_then(|v| v.value().ok());
    println!(
        "GR contribution: FerroAstro {fa:.6}, Swiftest {sw:.6}, difference {:+.3e} ″/cy (analytic {})",
        sw - fa,
        schw.map_or_else(|| "-".to_owned(), |x| format!("{x}"))
    );
    vs.get("ORACLE-CLITE-SWIFTEST-GR-AGREEMENT")
        .unwrap()
        .check(sw - fa)
        .unwrap();
}
