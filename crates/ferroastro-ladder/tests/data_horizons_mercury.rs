// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real-data tests (network, `data` tier) against JPL Horizons (DE441 Mercury at J2000):
//!
//! 1. FerroAstro's state→elements conversion reproduces Horizons' osculating elements for the same
//!    heliocentric state and Keplerian GM.
//! 2. The analytic GR rate from the **osculating** J2000 a, e is compared with Park et al. 2017
//!    Table 3 at 3σ (a pre-registered diagnostic; see IMPLEMENTATION-NOTES WP3).
//! 3. Rung N-2B (PHY-001) repeated from the Horizons state.
//!
//! Every test prints the request URL, retrieval date, SHA-256, `signature.version` and the
//! ephemeris source string Horizons reports.

mod two_body;

use ferroastro_analytic::advance::schwarzschild_advance;
use ferroastro_analytic::kepler::Elements;
use ferroastro_constants::De440;
use ferroastro_core::frames::{EclipticJ2000, Heliocentric, Position, State};
use ferroastro_core::time::J2000_JD;
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_forces::NewtonianPointMasses;
use ferroastro_integrators::Ias15Config;
use ferroastro_precession::fit::{Metadata, fit_secular_rate};
use ferroastro_testdata::{Fetched, fetch_url};
use ferroastro_vectors::VectorSet;
use std::collections::BTreeMap;
use std::fmt::Write as _;

const API: &str = "https://ssd.jpl.nasa.gov/api/horizons.api";

/// One Horizons answer: provenance plus the first `$$SOE` row keyed by column name.
struct Answer {
    fetched: Fetched,
    version: String,
    source: String,
    result: String,
    row: BTreeMap<String, String>,
}

impl Answer {
    #[expect(
        clippy::expect_used,
        reason = "test helper: a missing column is a test failure"
    )]
    fn num(&self, key: &str) -> f64 {
        self.row
            .get(key)
            .expect("Horizons row lacks the requested column")
            .parse()
            .expect("Horizons column is not a number")
    }

    fn print(&self, what: &str) {
        println!(
            "Horizons {what}: {} signature.version={} ephemeris={}",
            self.fetched.provenance(),
            self.version,
            self.source
        );
    }
}

#[expect(clippy::unwrap_used, clippy::indexing_slicing, reason = "test helper")]
fn query(ephem_type: &str, file: &str) -> Answer {
    let q = |s: &str| format!("%27{s}%27");
    let mut url = format!(
        "{API}?format=json&COMMAND={}&OBJ_DATA={}&MAKE_EPHEM={}&EPHEM_TYPE={}&CENTER={}&TLIST={}\
         &REF_PLANE={}&REF_SYSTEM={}&OUT_UNITS={}&CSV_FORMAT={}",
        q("199"),
        q("NO"),
        q("YES"),
        q(ephem_type),
        q("500@10"),
        q(&format!("{J2000_JD:.1}")),
        q("ECLIPTIC"),
        q("ICRF"),
        q("KM-S"),
        q("YES"),
    );
    if ephem_type == "VECTORS" {
        let _ = write!(url, "&VEC_TABLE={}&VEC_LABELS={}", q("2"), q("NO"));
    } else {
        let _ = write!(url, "&TIME_TYPE={}", q("TDB"));
    }
    let fetched = fetch_url(&url, file, None).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fetched.path).unwrap()).unwrap();
    let version = json["signature"]["version"]
        .as_str()
        .unwrap_or("?")
        .to_owned();
    let result = json["result"].as_str().unwrap().to_owned();
    let source = result
        .lines()
        .find(|l| l.starts_with("Target body name"))
        .and_then(|l| l.split("source:").nth(1))
        .map_or_else(
            || "?".to_owned(),
            |s| s.trim().trim_end_matches('}').to_owned(),
        );
    let soe = result.find("$$SOE").unwrap();
    let header = result[..soe]
        .lines()
        .rev()
        .find(|l| l.contains("JDTDB"))
        .unwrap();
    let data = result[soe + 5..]
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap();
    let row = header
        .split(',')
        .map(str::trim)
        .zip(data.split(',').map(str::trim))
        .filter(|(k, _)| !k.is_empty())
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    Answer {
        fetched,
        version,
        source,
        result,
        row,
    }
}

/// Heliocentric J2000 state of Mercury from the VECTORS answer (km, km/s).
fn state(v: &Answer) -> ([f64; 3], [f64; 3]) {
    (
        [v.num("X"), v.num("Y"), v.num("Z")],
        [v.num("VX"), v.num("VY"), v.num("VZ")],
    )
}

/// The "Keplerian GM" Horizons used for its osculating elements (km³/s²).
#[expect(clippy::unwrap_used, reason = "test helper")]
fn keplerian_gm(e: &Answer) -> f64 {
    let line = e
        .result
        .lines()
        .find(|l| l.starts_with("Keplerian GM"))
        .unwrap();
    line.split(':')
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

/// Verifies: PHY-004
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_horizons_osculating_elements_match() {
    let vecs = query("VECTORS", "horizons-199-sun-j2000-vectors.json");
    let els = query("ELEMENTS", "horizons-199-sun-j2000-elements.json");
    vecs.print("VECTORS");
    els.print("ELEMENTS");
    let gm = keplerian_gm(&els);
    let (r, v) = state(&vecs);
    let mine = Elements::from_vectors(r, v, gm).unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let agree = vs.get("HORIZONS-ELEMENTS-AGREEMENT").unwrap();
    println!("Keplerian GM used by Horizons: {gm} km^3/s^2");
    for (name, ours, theirs) in [
        ("EC", mine.e, els.num("EC")),
        ("A (km)", mine.a.km(), els.num("A")),
    ] {
        let rel = (ours - theirs).abs() / theirs.abs();
        println!("{name}: FerroAstro {ours:.16e}, Horizons {theirs:.16e}, relative diff {rel:.2e}");
        agree.check(rel).unwrap();
    }
    // Angles are reported (degrees), not asserted: their oracle is the same conversion.
    for (name, ours, theirs) in [
        ("IN", mine.inclination.radians().to_degrees(), els.num("IN")),
        ("OM", mine.node.radians().to_degrees(), els.num("OM")),
        ("W", mine.arg_periapsis.radians().to_degrees(), els.num("W")),
        (
            "MA",
            mine.mean_anomaly.radians().to_degrees(),
            els.num("MA"),
        ),
    ] {
        println!(
            "{name}: FerroAstro {ours:.13} deg, Horizons {theirs:.13} deg, diff {:.2e} deg",
            ours - theirs
        );
    }
}

/// Verifies: PHY-004
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_horizons_gr_rate_from_osculating_elements_vs_park17() {
    let els = query("ELEMENTS", "horizons-199-sun-j2000-elements.json");
    els.print("ELEMENTS");
    let de = De440::load().unwrap();
    let (gm_sun, gm_me) = two_body::gms(&de);
    let a = Length::from_km(els.num("A")).unwrap();
    let e = els.num("EC");
    let adv = schwarzschild_advance(gm_sun, de.speed_of_light(), a, e, gm_sun + gm_me).unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let park = vs.get("PARK17-GE-RATE").unwrap();
    let rate = adv.arcsec_per_century();
    println!(
        "GR-SCHW analytic with osculating DE441 J2000 a = {} km, e = {e}: {rate:.6} arcsec/cy; \
         Park17 Table 3 GE {} +/- 0.0009; diff {:+.6} (allowed {:e}, 3 sigma)",
        a.km(),
        park.value().unwrap(),
        rate - park.value().unwrap(),
        park.tolerance.allowed(park.value().unwrap())
    );
    park.check(rate).unwrap();
}

/// Verifies: PHY-001
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_horizons_n2b_from_de441_state() {
    let vecs = query("VECTORS", "horizons-199-sun-j2000-vectors.json");
    vecs.print("VECTORS");
    let de = De440::load().unwrap();
    let (gm_sun, gm_me) = two_body::gms(&de);
    let (r, v) = state(&vecs);
    let l = |z: f64| Length::from_km(z).unwrap();
    let w = |z: f64| Velocity::from_km_per_s(z).unwrap();
    let rel: State<EclipticJ2000, Heliocentric> = State::new(
        Position::new(l(r[0]), l(r[1]), l(r[2])),
        [w(v[0]), w(v[1]), w(v[2])],
    );
    let mu: GravParam = gm_sun + gm_me;
    let (x0, v0) = two_body::barycentric(&rel, gm_sun, gm_me);
    let model = NewtonianPointMasses::new(&[gm_sun, gm_me]).unwrap();
    let span = 1000.0 * two_body::julian_year_s(); // lint: allow-literal(PHY-001 minimum span in years)
    let cfg = Ias15Config::default();
    let run = two_body::run_lrl(&model, &x0, &v0, mu.km3_per_s2(), span, cfg);
    let meta = Metadata {
        estimator: "LRL eccentricity-vector longitude of the Mercury-Sun relative orbit",
        frame: "initial osculating orbit plane, axes fixed in Horizons' 'Ecliptic of J2000.0' (REF_SYSTEM=ICRF)".into(),
        epoch: ferroastro_core::time::Epoch::j2000(),
        span: ferroastro_core::units::TimeSpan::from_seconds(span).unwrap(),
        dynamics: "newtonian-point-masses, Sun + Mercury, mu = GM_sun + GM_Mercury (DE440)".into(),
        integrator: format!(
            "IAS15 (FerroAstro clean-room, Rein & Spiegel 2015), epsilon = {:e}, safety = {}, PC tolerance = {:e}, f64 + compensated summation",
            cfg.epsilon, cfg.safety, cfg.pc_tolerance
        ),
        constants: format!("DE440 GMs; initial state from Horizons ({}) {}", vecs.source, vecs.fetched.sha256),
    };
    let est = fit_secular_rate(&run.times, &run.longitudes, meta).unwrap();
    println!("N-2B from the Horizons DE441 J2000 state:\n{est}");
    println!("  NOTE: any non-zero rate is NUMERICAL ERROR (true N-2B rate is exactly 0).");
    let vs = VectorSet::load_workspace().unwrap();
    vs.get("PHY001-NEWTON-RATE")
        .unwrap()
        .check(est.arcsec_per_century())
        .unwrap();
}
