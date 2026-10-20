// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP21 checks S2, S3 and S4 (nightly, local): the GR contribution to Mercury's perihelion rate
//! as GR on minus GR off from identical header.440 ICs (CI-009), FerroAstro against REBOUND 5.2.2
//! + `REBOUNDx` 5.1.0, one Park2017 estimator applied by FerroAstro's tooling to every series.
//!
//! Fixed in the WP21 plan before any run:
//! * **S2, full system** (Sun, 8 planets with Earth and Moon resolved): `gr_eih` vs `gr_full`,
//!   legs back and forward over 1000–3000 CE; the two contributions agree within
//!   `ORACLE-STAGE-C-GR-TOOL-AGREEMENT` (PROVISIONAL 1e-3 ″/cy). The contribution itself
//!   includes the planets' 1PN terms and is reported against PHY-005's 42.980, not asserted.
//! * **S3, single-body isolation** (Sun + Mercury as two bodies): FerroAstro `gr_eih` and
//!   `gr_1pn_central` (N-body), `REBOUNDx` `gr_full`, `gr` and `gr_potential`; each contribution is
//!   42.980 ± 0.001 (`PHY005-GR-NUM-RATE`), and `gr_full` − `gr_eih` is within
//!   `ORACLE-STAGE-C-GR-TOOL-AGREEMENT`.
//! * **S4** (CI-009): the FerroAstro IC hashes of each on/off pair are equal, the `REBOUNDx` initial
//!   rows of each on/off pair are bitwise equal, and one estimator id serves every tool.
//!
//! The `REBOUNDx` series are not committed (~200 MB per configuration): they are produced locally
//! by `tools/oracles/nightly.sh` (REBOUND only for `oracle-c-*`) into `$FA_ORACLE_OUT_DIR`
//! (default `target/oracle-out`) as `rebound/<scenario>/` with a MANIFEST.json, verified here
//! together with the scenario and header.440 hashes they ran on (CI-003).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers: a failed unwrap, panic or out-of-range index is a test failure"
)]

use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, J2000_JD, SECONDS_PER_DAY};
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

const HALF_SPAN_YEARS: f64 = 1000.0; // lint: allow-literal(Park17 fit span 1000-3000 CE, J2000 +/- 1000 Julian yr as WP5/N3)
const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as WP14/N3)
const NAIF_SUN: i32 = 10;
const NAIF_MERCURY: i32 = 1;
const LEGS: [&str; 2] = ["back", "fwd"];
const SM: &str = "oracle-c-sun-mercury";
const FULL: &str = "oracle-c-planets-moon";

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

/// The N3 estimator: Park17 mean-orbit plane, projected longitude, sampled-state passages,
/// quadratic + the 14 Table-2 terms (SW92 Table 2a mean motions), 20 jack-knife blocks, fit
/// window J2000 ± 1000 Julian yr.
fn estimator(vs: &VectorSet) -> Estimator {
    let n = |b: &str| {
        vs.get(&format!("SW92-T2A-{b}-L-RATE"))
            .unwrap()
            .value()
            .unwrap()
    };
    let lo = HALF_SPAN_YEARS.mul_add(-DAYS_PER_JULIAN_YEAR, J2000_JD);
    let hi = HALF_SPAN_YEARS.mul_add(DAYS_PER_JULIAN_YEAR, J2000_JD);
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
        window_jd: Some((lo, hi)),
    }
}

/// One `REBOUNDx` series: `rebound/<scenario>/<file>`, its manifest entry verified (CI-003) with
/// the scenario and header.440 it ran on. Returns Mercury relative to the Sun and the initial
/// rows (t = 0) as raw bits, for the CI-009 identity check.
fn oracle_leg(scenario: &str, file: &str, header_sha: &str) -> (Track, Vec<u64>) {
    let dir = oracle_out().join("rebound").join(scenario);
    let m = Manifest::verify_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let entry = m.entry(file).unwrap();
    entry
        .check_input(
            &format!("{scenario}.toml"),
            &sha256_hex(scenario_text(scenario).as_bytes()),
        )
        .unwrap();
    entry.check_input("header.440", header_sha).unwrap();
    println!(
        "  rebound/{scenario}/{file}: {} image {} runner {} wall {} s effect {:?} steps {:?}",
        entry.version,
        entry.image_digest,
        entry.runner_sha,
        entry.wall_clock_s,
        entry.extra.get("reboundx_effect"),
        entry.extra.get("steps_done")
    );
    let g = GoldenStates::parse(file, &std::fs::read_to_string(dir.join(file)).unwrap()).unwrap();
    assert_eq!(g.center_naif().unwrap(), 0, "SSB-centred series");
    let mut track = Track::new(g.meta_f64("epoch_jd_tdb").unwrap());
    let mut initial = Vec::new();
    let mut sun: Option<(f64, [f64; 3], [f64; 3])> = None;
    for row in &g.rows {
        if row.t_days == 0.0 {
            initial.push(row.t_days.to_bits());
            initial.extend(
                row.position_km
                    .iter()
                    .chain(&row.velocity_km_s)
                    .map(|x| x.to_bits()),
            );
        }
        match row.naif {
            NAIF_SUN => sun = Some((row.t_days, row.position_km, row.velocity_km_s)),
            NAIF_MERCURY => {
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
    assert!(!track.t.is_empty(), "{scenario}/{file}: no Mercury rows");
    (track, initial)
}

/// The two legs of a `REBOUNDx` configuration merged; `variant` selects a secondary series.
fn oracle_track(config: &str, variant: Option<&str>, header_sha: &str) -> (Track, Vec<u64>) {
    let mut legs = Vec::new();
    let mut initial = Vec::new();
    for leg in LEGS {
        let scenario = format!("{config}-{leg}");
        let file = variant.map_or_else(
            || format!("{scenario}.rebound.states.csv"),
            |v| format!("{scenario}.rebound.{v}.states.csv"),
        );
        let (t, i) = oracle_leg(&scenario, &file, header_sha);
        if initial.is_empty() {
            initial = i;
        } else {
            assert_eq!(initial, i, "{config}: both legs start from the same rows");
        }
        legs.push(t);
    }
    (Track::merge(legs), initial)
}

/// FerroAstro IAS15 on the same two scenario files: the merged track and the IC hash.
fn ferroastro_track(config: &str, h: &Header, prov: &str) -> (Track, String) {
    let mut legs = Vec::new();
    let mut hash = String::new();
    for leg in LEGS {
        let scenario = format!("{config}-{leg}");
        let spec = ScenarioSpec::parse(&scenario_text(&scenario)).unwrap();
        let out = run(&spec, &DataSources::none().with_header(h, prov)).unwrap();
        let mut track = Track::new(out.epoch_jd_tdb[0] + out.epoch_jd_tdb[1]);
        assert_eq!(track.absorb(&out, NAIF_MERCURY, NAIF_SUN), 0);
        println!(
            "  FerroAstro {scenario}: {} IAS15 steps, IC hash {}, {} samples",
            out.stats.steps,
            out.ic_hash,
            track.t.len()
        );
        if hash.is_empty() {
            hash.clone_from(&out.ic_hash);
        } else {
            assert_eq!(hash, out.ic_hash);
        }
        legs.push(track);
    }
    (Track::merge(legs), hash)
}

fn show(label: &str, e: &Estimate) {
    println!(
        "  {label:<40} {:>14.6} ″/cy  σ_jk {:.2e}  passages {}  rms {:.2e}″",
        e.rate, e.jackknife_sigma, e.passages, e.rms_residual
    );
}

/// Precondition (as the WP17 differentials, review R3): every `REBOUNDx` series must exist.
fn require_oracle_series(configs: &[&str]) {
    let missing: Vec<String> = configs
        .iter()
        .flat_map(|c| LEGS.map(|l| format!("{c}-{l}")))
        .filter(|s| !oracle_out().join("rebound").join(s).is_dir())
        .map(|s| format!("rebound/{s}"))
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

/// S2, S3, S4: GR on − GR off for FerroAstro and `REBOUNDx`, full system and Sun + Mercury.
///
/// Verifies: CI-009, REF-005, PHY-005, NUM-017
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "nightly evidence test; it reads top to bottom"
)]
#[ignore = "nightly: 2000-yr FerroAstro EIH runs and the REBOUNDx stage C series from tools/oracles/nightly.sh; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_stage_c_gr_contribution_reboundx_vs_ferroastro() {
    let t0 = std::time::Instant::now();
    let vs = VectorSet::load_canonical().unwrap();
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    let prov = f.provenance();
    println!("de440-header: {prov}");
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let header_sha = f.sha256.clone();
    let est = estimator(&vs);
    // S4: one estimator id for every tool and configuration.
    println!(
        "estimator (every tool): {}",
        est.id(PassageSource::SampledStates)
    );
    let configs = [
        format!("{SM}-newton"),
        format!("{SM}-eih"),
        format!("{SM}-gr1pn"),
        format!("{FULL}-newton"),
        format!("{FULL}-eih"),
    ];
    require_oracle_series(&configs.iter().map(String::as_str).collect::<Vec<_>>());
    let agree = vs.get("ORACLE-STAGE-C-GR-TOOL-AGREEMENT").unwrap();
    let phy005 = vs.get("PHY005-GR-NUM-RATE").unwrap();
    let rate = |label: &str, tr: &Track| {
        let e = est.estimate(tr, PassageSource::SampledStates).unwrap();
        show(label, &e);
        e.rate
    };
    let mut failures: Vec<String> = Vec::new();
    let mut check = |what: String, r: Result<f64, ferroastro_vectors::VectorError>| {
        if let Err(e) = r {
            println!("  FAIL {what}: {e}");
            failures.push(format!("{what}: {e}"));
        }
    };

    // ---- S3: Sun + Mercury -------------------------------------------------------------------
    println!("S3: Sun + Mercury, GR on − GR off, 1000–3000 CE");
    let (fa_off, hash_off) = ferroastro_track(&format!("{SM}-newton"), &h, &prov);
    let (fa_eih, hash_eih) = ferroastro_track(&format!("{SM}-eih"), &h, &prov);
    let (fa_gr1, hash_gr1) = ferroastro_track(&format!("{SM}-gr1pn"), &h, &prov);
    assert_eq!(
        hash_off, hash_eih,
        "CI-009: FerroAstro IC hashes, eih vs newton"
    );
    assert_eq!(
        hash_off, hash_gr1,
        "CI-009: FerroAstro IC hashes, gr1pn vs newton"
    );
    let (rb_off, init_off) = oracle_track(&format!("{SM}-newton"), None, &header_sha);
    let (rb_full, init_full) = oracle_track(&format!("{SM}-eih"), None, &header_sha);
    let (rb_gr, init_gr) = oracle_track(&format!("{SM}-gr1pn"), None, &header_sha);
    let (rb_pot, init_pot) =
        oracle_track(&format!("{SM}-gr1pn"), Some("gr_potential"), &header_sha);
    for (label, init) in [
        ("gr_full", &init_full),
        ("gr", &init_gr),
        ("gr_potential", &init_pot),
    ] {
        assert_eq!(
            &init_off, init,
            "CI-009: REBOUNDx initial rows, {label} vs Newtonian"
        );
    }
    println!(
        "  CI-009: FerroAstro IC hash {hash_off} (3 configurations); REBOUNDx initial rows bitwise \
         equal (4 series, {} words)",
        init_off.len()
    );
    println!(
        "  samples: FerroAstro {}, REBOUNDx {}",
        fa_off.t.len(),
        rb_off.t.len()
    );
    let r_fa_off = rate("FerroAstro newton", &fa_off);
    let r_fa_eih = rate("FerroAstro gr_eih", &fa_eih);
    let r_fa_gr1 = rate("FerroAstro gr_1pn_central (N-body)", &fa_gr1);
    let r_rb_off = rate("REBOUND newton", &rb_off);
    let r_rb_full = rate("REBOUNDx gr_full", &rb_full);
    let r_rb_gr = rate("REBOUNDx gr", &rb_gr);
    let r_rb_pot = rate("REBOUNDx gr_potential", &rb_pot);
    let c_fa_eih = r_fa_eih - r_fa_off;
    let c_fa_gr1 = r_fa_gr1 - r_fa_off;
    let c_rb_full = r_rb_full - r_rb_off;
    let c_rb_gr = r_rb_gr - r_rb_off;
    let c_rb_pot = r_rb_pot - r_rb_off;
    let p = phy005.value().unwrap();
    let ptol = phy005.tolerance.abs.unwrap();
    for (label, c) in [
        ("FerroAstro gr_eih", c_fa_eih),
        ("FerroAstro gr_1pn_central", c_fa_gr1),
        ("REBOUNDx gr_full", c_rb_full),
        ("REBOUNDx gr", c_rb_gr),
        ("REBOUNDx gr_potential", c_rb_pot),
    ] {
        println!(
            "  GR contribution {label:<26} {c:.6} ″/cy; − {p} = {:+.3e} (margin {:.3})",
            c - p,
            (c - p).abs() / ptol
        );
        check(format!("S3 {label} vs PHY-005"), phy005.check(c));
    }
    let d_sm = c_rb_full - c_fa_eih;
    println!(
        "  S3 REBOUNDx gr_full − FerroAstro gr_eih = {d_sm:+.3e} ″/cy (margin {:.3}); gr − \
         gr_1pn_central = {:+.3e}; gr_potential − gr = {:+.3e}",
        d_sm.abs() / agree.tolerance.abs.unwrap(),
        c_rb_gr - c_fa_gr1,
        c_rb_pot - c_rb_gr
    );
    check("S3 gr_full − gr_eih".into(), agree.check(d_sm));

    // ---- S2: full system ---------------------------------------------------------------------
    println!("S2: Sun + 8 planets + Moon, GR on − GR off, 1000–3000 CE");
    let (fa_off, hash_off) = ferroastro_track(&format!("{FULL}-newton"), &h, &prov);
    let (fa_on, hash_on) = ferroastro_track(&format!("{FULL}-eih"), &h, &prov);
    assert_eq!(
        hash_off, hash_on,
        "CI-009: FerroAstro IC hashes, full system"
    );
    let (rb_off, init_off) = oracle_track(&format!("{FULL}-newton"), None, &header_sha);
    let (rb_on, init_on) = oracle_track(&format!("{FULL}-eih"), None, &header_sha);
    assert_eq!(
        init_off, init_on,
        "CI-009: REBOUNDx initial rows, full system"
    );
    println!(
        "  CI-009: FerroAstro IC hash {hash_off}; REBOUNDx initial rows bitwise equal ({} words)",
        init_off.len()
    );
    let r_fa_off = rate("FerroAstro newton", &fa_off);
    let r_fa_on = rate("FerroAstro gr_eih", &fa_on);
    let r_rb_off = rate("REBOUND newton", &rb_off);
    let r_rb_on = rate("REBOUNDx gr_full", &rb_on);
    let c_fa = r_fa_on - r_fa_off;
    let c_rb = r_rb_on - r_rb_off;
    let d = c_rb - c_fa;
    println!(
        "  GR contribution, full system: FerroAstro {c_fa:.6}, REBOUNDx {c_rb:.6} ″/cy; \
         difference {d:+.3e} (margin {:.3})",
        d.abs() / agree.tolerance.abs.unwrap()
    );
    println!(
        "  (reported) full-system contribution − Sun + Mercury isolation: FerroAstro {:+.6}, \
         REBOUNDx {:+.6} ″/cy; − PHY-005 {p}: {:+.6}",
        c_fa - c_fa_eih,
        c_rb - c_rb_full,
        c_fa - p
    );
    println!(
        "  (reported) Newtonian full-system rate: FerroAstro {r_fa_off:.6}, REBOUND {r_rb_off:.6}, \
         difference {:+.3e} ″/cy",
        r_rb_off - r_fa_off
    );
    check("S2 full-system gr_full − gr_eih".into(), agree.check(d));
    println!("wall clock {:.1} s", t0.elapsed().as_secs_f64());
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
