// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP18 PR tier: the speed-table library on the committed oracle goldens (real REBOUND, Swiftest,
//! Mercury6 and Orekit output) and on a FerroAstro stage A run from the pinned header.440, plus
//! the CI-008 table rules and the two-table stability check on synthetic tables.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_bench::accuracy::{
    com_energy, mercury_max_dr, mercury_track, relative_energy_change,
};
use ferroastro_bench::series::NAIF_MERCURY;
use ferroastro_bench::{
    Accuracy, Entry, Host, MIN_RUNS, Measure, ScenarioInfo, Series, SpeedTable, Status, Timing,
    read_run, stability,
};
use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, initial_conditions, run, run_with_energy};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_vectors::VectorSet;
use std::collections::BTreeMap;
use std::path::PathBuf;

const STAGE_A: &str = "stage-a-sun-8planets-newton-100yr";
// Two independent sums of the same ~45 energy terms of mixed sign, each rounded once per term.
const ENERGY_ULPS: f64 = 16.0; // lint: allow-literal(round-off allowance of two energy sums)
// One distance computed with and without fused multiply-add.
const DR_ULPS: f64 = 4.0; // lint: allow-literal(round-off allowance of one 3-vector norm)

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

fn spec(name: &str) -> ScenarioSpec {
    ScenarioSpec::parse(
        &std::fs::read_to_string(root().join("ci/scenarios").join(format!("{name}.toml"))).unwrap(),
    )
    .unwrap()
}

fn golden(tool: &str, scenario: &str) -> (PathBuf, Series) {
    let dir = root().join("ci/golden").join(tool).join(scenario);
    let s = Series::read(&dir, scenario, tool).unwrap();
    (dir, s)
}

/// Every committed state golden parses with this crate's reader (one row per data line), and its
/// MANIFEST.json gives the timing record the speed table uses (CI-003 → CI-008).
///
/// Verifies: CI-008, CI-003
#[test]
fn committed_oracle_goldens_read_as_series_and_run_records() {
    let mut n = 0;
    for tool in std::fs::read_dir(root().join("ci/golden")).unwrap() {
        let tool = tool.unwrap().path();
        for d in std::fs::read_dir(&tool).unwrap() {
            let d = d.unwrap().path();
            for f in std::fs::read_dir(&d).unwrap() {
                let f = f.unwrap().path();
                let name = f.file_name().unwrap().to_string_lossy().into_owned();
                if !name.ends_with(".states.csv") {
                    continue;
                }
                let text = std::fs::read_to_string(&f).unwrap();
                let s = Series::parse(&name, &text).unwrap();
                let data_lines = text
                    .lines()
                    .filter(|l| !l.starts_with('#') && !l.starts_with("t_days"))
                    .count();
                assert_eq!(s.rows.len(), data_lines, "{name}");
                let info = read_run(&d).unwrap();
                assert_eq!(info.file, name);
                assert!(info.wall_clock_s > 0.0 && !info.version.is_empty());
                println!(
                    "{name}: {} rows, centre {}, {} {} wall {} s",
                    s.rows.len(),
                    s.center_naif,
                    info.tool,
                    info.version,
                    info.wall_clock_s
                );
                n += 1;
            }
        }
    }
    assert!(
        n >= 4,
        "expected the stage A and stage B goldens, found {n}"
    );
}

/// The accuracy columns on real output: a FerroAstro stage A run (header.440) against the
/// committed REBOUND golden through this crate's code path stays inside the WP16 stage A bound;
/// the FerroAstro series round-trips through the CSV writer bit for bit; and the energy column
/// computed from each tool's output is printed next to the tool's own report.
///
/// Verifies: CI-008, REF-005, NUM-023
#[test]
fn stage_a_accuracy_columns_from_real_output() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec(STAGE_A);
    let (out, energy) = run_with_energy(&s, &data).unwrap();
    let file = format!("{STAGE_A}.ferroastro.states.csv");
    let fa = Series::from_run(&file, &out);
    let back = Series::parse(&file, &fa.to_csv(&["test".to_owned()])).unwrap();
    assert_eq!(back.rows.len(), fa.rows.len());
    for (a, b) in back.rows.iter().zip(&fa.rows) {
        assert_eq!(a.naif, b.naif);
        assert_eq!(a.t_days.to_bits(), b.t_days.to_bits());
        for k in 0..3 {
            assert_eq!(a.r[k].to_bits(), b.r[k].to_bits());
            assert_eq!(a.v[k].to_bits(), b.v[k].to_bits());
        }
    }
    let gms: BTreeMap<i32, f64> = initial_conditions(&s, &data)
        .unwrap()
        .records
        .iter()
        .map(|r| (r.naif, r.gm[0]))
        .collect();
    let (fa_e, n) = relative_energy_change(&fa, &gms).unwrap();
    let model = energy.unwrap();
    println!(
        "ferroastro: |dE/E| from states {fa_e:.3e} ({n} bodies); model {:.3e}",
        model.relative_change()
    );
    // The energy column's formula against the propagator's own model energy (an independent
    // implementation, `NBodyModel::energy_over_g`) at the first and the last output time: a
    // wrong kinetic or potential term in `com_energy` fails here.
    let slices = fa.slices();
    for (slice, reference) in [
        (slices.first().unwrap(), model.initial),
        (slices.last().unwrap(), model.last),
    ] {
        // The model energy is taken in the DE440 barycentric frame, `com_energy` in the sampled
        // system's centre-of-mass frame. They differ by the kinetic energy of that centre of
        // mass, ½ M |v_cm|², which is not zero because DE440's barycentre includes masses
        // this run omits.
        let m: f64 = slice.iter().map(|r| gms[&r.naif]).sum();
        let p = (0..3).map(|k| slice.iter().map(|r| gms[&r.naif] * r.v[k]).sum::<f64>());
        let cm_kinetic = 0.5 * p.map(|x| x * x).sum::<f64>() / m;
        let e = com_energy(slice, fa.center_naif, &gms).unwrap();
        let rel = ((e + cm_kinetic - reference) / reference).abs();
        println!(
            "com_energy {e:.17e} + CM kinetic {cm_kinetic:.3e} vs model {reference:.17e}: rel {rel:.2e}"
        );
        assert!(rel <= ENERGY_ULPS * f64::EPSILON, "{rel}");
    }
    let (_, reb) = golden("rebound", STAGE_A);
    let (dr, t, matched) = mercury_max_dr(&fa, &reb).unwrap();
    println!(
        "ferroastro vs rebound golden: Mercury max |dr| {dr:.4e} km at {t} d ({matched} samples)"
    );
    assert_eq!(matched, out.states.len());
    // The column is the maximum over the common output times (a brute-force pass over both
    // heliocentric tracks, which share their sample times here): a minimum, or a single sample,
    // would pass the upper-bound vector below with 0 km at t = 0.
    let ha = fa.heliocentric(NAIF_MERCURY).unwrap();
    let hb = reb.heliocentric(NAIF_MERCURY).unwrap();
    assert_eq!(ha.len(), hb.len());
    let brute = ha
        .iter()
        .zip(&hb)
        .map(|((_, ra, _), (_, rb, _))| (0..3).map(|k| (ra[k] - rb[k]).powi(2)).sum::<f64>().sqrt())
        .fold(0.0, f64::max);
    assert!(
        (dr - brute).abs() <= DR_ULPS * f64::EPSILON * brute,
        "{dr} vs {brute}"
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("ORACLE-STAGE-A-REBOUND-MAX-DR")
        .unwrap()
        .check(dr)
        .unwrap();
    for tool in ["rebound", "swiftest", "mercury6"] {
        let (dir, g) = golden(tool, STAGE_A);
        let (e, nb) = relative_energy_change(&g, &gms).unwrap();
        let own = read_run(&dir)
            .unwrap()
            .extra
            .get("relative_energy_change")
            .cloned()
            .unwrap_or_default();
        println!(
            "{tool}: |dE/E| from states {e:.3e} ({nb} bodies incl. the Sun); own report {own}"
        );
        assert!(e.is_finite() && nb == gms.len());
        let (dr, _, _) = mercury_max_dr(&g, &reb).unwrap();
        println!("{tool} vs rebound: Mercury max |dr| {dr:.4e} km");
    }
    // The Mercury track has one sample per output time.
    assert_eq!(mercury_track(&fa).unwrap().t.len(), out.states.len());
    assert_eq!(
        fa.heliocentric(NAIF_MERCURY).unwrap().len(),
        out.states.len()
    );
    // The plain run is the same output (the energy is evaluated outside the integration).
    assert_eq!(run(&s, &data).unwrap().differing_words(&out), Some(0));
}

fn measure() -> Measure {
    Measure::none("", "synthetic")
}

fn entry(scenario: &str, tool: &str, runs: Vec<f64>) -> Entry {
    Entry {
        scenario: scenario.into(),
        tool: tool.into(),
        version: "v".into(),
        settings: "s".into(),
        status: Status::Timed,
        message: String::new(),
        timing: Timing::new(runs),
        load_avg: Vec::new(),
        timer_scope: "scope".into(),
        rate_arcsec_cy: None,
        accuracy: Accuracy {
            relative_energy_change: measure(),
            mercury_dr_km: measure(),
            rate_minus_ferroastro: measure(),
            rate_minus_reference: measure(),
        },
    }
}

fn table(run_id: &str, cpu: &str, runs: Vec<f64>) -> SpeedTable {
    let mut refused = entry("x", "orekit", vec![]);
    refused.status = Status::Refused;
    refused.message = "REFUSED: orekit cannot run scenario 'x'".into();
    SpeedTable {
        schema_version: ferroastro_bench::table::TABLE_SCHEMA_VERSION,
        run_id: run_id.into(),
        generated_utc: "t".into(),
        host: Host {
            runner_class: "local".into(),
            cpu_model: cpu.into(),
            logical_cpus: 1,
            cpu_flags: vec!["avx2".into()],
            avx512: false,
            kernel: "k".into(),
            rustc: "r".into(),
            build: "b".into(),
            git_commit: "c".into(),
        },
        load_avg_run: [0.0, 0.0],
        scenarios: vec![ScenarioInfo {
            name: "x".into(),
            label: "(x)".into(),
            daily_twin: None,
            sha256: "0".into(),
        }],
        entries: vec![entry("x", "ferroastro", runs), refused],
        notes: Vec::new(),
    }
}

/// CI-008 table rules (≥ 5 runs per timed entry, refusals carry their message, every empty
/// accuracy column says why), the markdown lists every tool, and the stability check passes,
/// fails and refuses exactly as pre-registered (WP18 E6).
///
/// Verifies: CI-008
#[test]
fn table_rules_markdown_and_stability() {
    let five = vec![10.0, 11.0, 12.0, 13.0, 14.0]; // lint: allow-literal(synthetic wall-clock seconds for the table logic)
    let t1 = table("r1", "cpu", five.clone());
    t1.validate().unwrap();
    let md = t1.to_markdown();
    assert!(md.contains("| ferroastro |") && md.contains("| orekit |"));
    assert!(md.contains("refused (REF-011)") && md.contains("REFUSED: orekit"));

    let mut few = table("r0", "cpu", vec![1.0; MIN_RUNS - 1]);
    assert!(few.validate().is_err());
    few = table("r0", "cpu", five.clone());
    few.entries[1].message.clear();
    assert!(few.validate().is_err(), "a refusal needs its message");
    few = table("r0", "cpu", five.clone());
    few.entries[0].accuracy.mercury_dr_km.basis.clear();
    assert!(few.validate().is_err(), "an empty column needs its reason");

    // Range of t1 = 4 s, median 12 s.
    let close = table("r2", "cpu", vec![12.5, 13.0, 15.0, 15.5, 16.0]); // lint: allow-literal(synthetic wall-clock seconds for the table logic)
    let rows = stability(&t1, &close, "ferroastro").unwrap();
    assert!(rows[0].pass, "|15 - 12| = 3 < 4");
    let far = table("r3", "cpu", vec![16.0, 16.5, 17.0, 17.5, 18.0]); // lint: allow-literal(synthetic wall-clock seconds for the table logic)
    assert!(
        !stability(&t1, &far, "ferroastro").unwrap()[0].pass,
        "5 >= 4"
    );
    // The criterion uses the earlier table's range (pre-registered), not the later one's.
    let wide = table("r5", "cpu", vec![6.0, 10.0, 17.0, 20.0, 24.0]); // lint: allow-literal(synthetic wall-clock seconds for the table logic)
    let w = &stability(&t1, &wide, "ferroastro").unwrap()[0];
    assert!(!w.pass && w.pass_reverse, "|17 - 12| = 5 >= 4 but < 18");
    assert!(stability(&t1, &t1, "ferroastro").is_err(), "same run");
    assert!(
        stability(&t1, &table("r4", "other cpu", five), "ferroastro").is_err(),
        "CI-008: no comparison across machines"
    );
}
