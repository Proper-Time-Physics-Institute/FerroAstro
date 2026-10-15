// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Oracle stage B (REF-006, WP17): Mercury as a massless particle in the DE440-driven Newtonian
//! field. FerroAstro (ephemeris-field mode, IAS15, positions from `de440.bsp`) against the cached
//! Orekit 13.1.9 golden (`DormandPrince853`, positions from the JPL binary `linux_p1550p2650.440`:
//! the same DE440 Chebyshev data). No container is started here (CI-005).

use ferroastro_constants::De440;
use ferroastro_ephem::Spk;
use ferroastro_oracle::{GoldenStates, Manifest, compare_states, sha256_hex};
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_system::Catalogue;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const SCENARIO: &str = "oracle-stage-b-mercury-de440-field-100yr";
const JPL_FILE: &str = "linux_p1550p2650.440";
/// Orekit's GMs (m³/s²) against the header.440 values (km³/s²): the same DE440 constants, read
/// from the binary file's header and converted by each code, so equal to a few ulp.
const GM_RELATIVE_ULPS: f64 = 4.0; // lint: allow-literal(a few ulp of two unit conversions)

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Stage B: FerroAstro against the Orekit golden; the largest position difference over the 7306
/// five-day samples is asserted against `ORACLE-STAGE-B-OREKIT-MAX-DR` (PROVISIONAL 1 km). The
/// GMs Orekit read from the binary file equal header.440's to a few ulp.
///
/// Verifies: REF-006, CI-002, CI-005
#[test]
fn stage_b_ferroastro_matches_the_orekit_golden_in_the_de440_field() {
    let dir = root().join("ci/golden/orekit").join(SCENARIO);
    let m = Manifest::verify_dir(&dir).unwrap();
    let csv = format!("{SCENARIO}.orekit.states.csv");
    let entry = m.entry(&csv).unwrap();
    let text = std::fs::read_to_string(root().join("ci/scenarios").join(format!("{SCENARIO}.toml")))
        .unwrap();
    entry
        .check_input(&format!("{SCENARIO}.toml"), &sha256_hex(text.as_bytes()))
        .unwrap();
    let pin = ferroastro_testdata::product("de440-linux-binary").unwrap().sha256;
    entry.check_input(JPL_FILE, &pin).unwrap();
    println!(
        "golden: {} ({}), image {}, runner {}, {:?}",
        entry.file, entry.version, entry.image_digest, entry.runner_sha, entry.extra
    );

    // The same constants: every field GM Orekit used against the header.440 value.
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let spec = ScenarioSpec::parse(&text).unwrap();
    for b in &spec.bodies {
        if *b == ferroastro_scenario::BodyName::Mercury {
            continue;
        }
        let key = format!("orekit_gm_{}_m3_s2", b.label().to_lowercase());
        let orekit: f64 = entry.extra.get(&key).unwrap().parse().unwrap();
        let ours = if *b == ferroastro_scenario::BodyName::Sun {
            de.gm_sun().km3_per_s2()
        } else {
            cat.gm(ferroastro_ephem::NaifId(b.naif())).unwrap().km3_per_s2()
        } * 1e9; // lint: allow-literal(km^3 to m^3)
        let rel = (orekit - ours).abs() / ours;
        println!("GM {:<8} Orekit {orekit:e} m3/s2, header.440 {ours:e}, rel diff {rel:.2e}", b.label());
        assert!(rel <= GM_RELATIVE_ULPS * f64::EPSILON, "{}: {rel}", b.label());
    }

    let bsp = ferroastro_testdata::locate_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bsp.provenance());
    let bytes = std::fs::read(&bsp.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let prov = bsp.provenance();
    let t0 = std::time::Instant::now();
    let out = run(&spec, &DataSources::none().with_spk(&spk, &prov)).unwrap();
    println!(
        "FerroAstro: {} ({:.1} s), IC hash {}, IC source {}",
        out.model,
        t0.elapsed().as_secs_f64(),
        out.ic_hash,
        out.ic_source
    );
    let golden =
        GoldenStates::parse(&csv, &std::fs::read_to_string(dir.join(&csv)).unwrap()).unwrap();
    let cmp = compare_states(&golden, &out).unwrap();
    let worst = cmp.worst().unwrap();
    println!(
        "stage B orekit: {} rows at {} times; max |dr| = {:.6e} km at t = {} d; max |dv| = {:.3e} km/s",
        cmp.rows, cmp.times, worst.max_dr_km, worst.t_days_at_max_dr, worst.max_dv_km_s
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("ORACLE-STAGE-B-OREKIT-MAX-DR")
        .unwrap()
        .check(cmp.max_dr_km())
        .unwrap();
}
