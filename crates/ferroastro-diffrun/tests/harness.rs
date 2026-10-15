// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Harness mechanics on the real DE440 initial conditions (PR tier: header.440 from the pinned JPL
//! cache, CI-005). WP14 checks 1–3: identical ICs across a plan and the refusals, an interrupted
//! and resumed leg equals the one-piece leg bit for bit, and the combination arithmetic.

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

mod common;

use ferroastro_core::ErrorCode;
use ferroastro_diffrun::{
    BodyToggle, Combination, PassageSource, Plan, Protocol, RunOptions, check_identical_ics,
    evaluate, run_leg, run_plan,
};
use ferroastro_precession::park2017::FitModel;
use ferroastro_propagate::{DataSources, initial_conditions};
use ferroastro_scenario::{BodyName, Precision, ScenarioSpec};

// Run controls (not physical values): PR-tier lengths and output cadence.
const LEG_YEARS: f64 = 10.0; // lint: allow-literal(run length, PR-tier cost)
const CADENCE_DAYS: f64 = 1.0; // lint: allow-literal(output cadence of the estimator samples)
const SEGMENT_YEARS: f64 = 1.0; // lint: allow-literal(checkpoint segment length)
const STORE_LEG_YEARS: f64 = 6.0; // lint: allow-literal(run length, PR-tier cost)
const JACKKNIFE_BLOCKS: usize = 10; // lint: allow-literal(jack-knife blocks for ~80 passages)

fn template() -> ScenarioSpec {
    let mut s = common::scenario("sun-mercury-newton-header440.toml");
    s.output.cadence_days = Some(CADENCE_DAYS);
    s.output.perihelion = vec![BodyName::Mercury];
    s
}

fn small_plan() -> Plan {
    use BodyName::{Jupiter, Mercury, Sun, Venus};
    Plan::bodies(
        "wp14-pr",
        &template(),
        &[Sun, Mercury],
        &[Sun, Mercury, Venus, Jupiter],
        &[
            BodyToggle::new("Venus", &[Venus]),
            BodyToggle::new("Jupiter", &[Jupiter]),
        ],
        &[(0, 1)],
        &[-LEG_YEARS, LEG_YEARS],
    )
    .unwrap()
}

fn quadratic() -> FitModel {
    FitModel {
        quadratic: true,
        frequencies: vec![],
    }
}

/// Check 1 and 3: every leg of a body-toggle plan shares the base ICs (and every body's record),
/// the plan refuses a leg with other ICs (`FA-E1401`) and malformed plans (`FA-E1400`), and the
/// rows are the signed sums of the per-configuration rates.
///
/// Verifies: NUM-013, CI-009, PHY-012
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one plan run, then every assertion on its result"
)]
fn plan_shares_identical_ics_and_rows_are_signed_sums() {
    let (h, prov) = common::header();
    let data = DataSources::none().with_header(&h, &prov);
    let plan = small_plan();
    let labels: Vec<&str> = plan.configs.iter().map(|c| c.label.as_str()).collect();
    println!("configurations: {labels:?}; legs {}", plan.leg_count());
    assert_eq!(
        labels,
        [
            "base",
            "base+Venus",
            "base+Jupiter",
            "base+Venus+Jupiter",
            "full",
            "full-Venus",
            "full-Jupiter"
        ]
    );
    let est = common::park17_estimator(
        quadratic(),
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        None,
    );
    let opts = RunOptions {
        segment_years: SEGMENT_YEARS * 4.0,
        ..RunOptions::default()
    };
    let r = run_plan(&plan, &data, &est, &opts).unwrap();
    println!("{}", r.markdown(&|_, _| None));
    // Each configuration's legs report the IC hash of its own body set; the base hash is shared.
    for c in &r.configs {
        assert_eq!(c.ic_hashes.len(), 2);
        assert_eq!(c.ic_hashes[0], c.ic_hashes[1], "{}", c.label);
        assert!(c.estimate.passages > 0);
    }
    let base_only = r.configs.iter().find(|c| c.label == "base").unwrap();
    let full = r.configs.iter().find(|c| c.label == "full").unwrap();
    assert_ne!(base_only.ic_hashes[0], full.ic_hashes[0]);
    assert_eq!(r.shared_ic_hash.len(), 64);
    // Rows are the definitions, bit for bit.
    let rate = |l: &str| {
        r.configs
            .iter()
            .find(|c| c.label == l)
            .unwrap()
            .estimate
            .rate
    };
    let add = |x: &str| {
        r.rows_of(Protocol::Add)
            .find(|w| w.combination.label == x)
            .unwrap()
            .value
    };
    let rem = |x: &str| {
        r.rows_of(Protocol::Remove)
            .find(|w| w.combination.label == x)
            .unwrap()
            .value
    };
    let cross = r.rows_of(Protocol::CrossTerm).next().unwrap().value;
    let signed = |terms: &[(f64, &str)]| terms.iter().fold(0.0, |s, (w, l)| w.mul_add(rate(l), s));
    assert_eq!(
        add("Venus").to_bits(),
        signed(&[(1.0, "base+Venus"), (-1.0, "base")]).to_bits()
    );
    assert_eq!(
        rem("Jupiter").to_bits(),
        signed(&[(1.0, "full"), (-1.0, "full-Jupiter")]).to_bits()
    );
    assert_eq!(
        cross.to_bits(),
        signed(&[
            (1.0, "base+Venus+Jupiter"),
            (-1.0, "base+Venus"),
            (-1.0, "base+Jupiter"),
            (1.0, "base")
        ])
        .to_bits()
    );
    // The base run (Sun + Mercury, Newtonian) has no secular precession: its rate is the
    // estimator's numerical floor over 20 years, printed.
    println!(
        "base rate {:.3e} arcsec/cy (two-body: true value 0), jack-knife sigma {:.3e}",
        base_only.estimate.rate, base_only.estimate.jackknife_sigma
    );

    // FA-E1401: a leg whose ICs differ (double-double assembly of the same header).
    let mut dd = plan.configs[1].legs[0].clone();
    dd.integrator.precision = Precision::Dd;
    dd.name.push_str("-dd");
    let mut initial: Vec<_> = plan
        .configs
        .iter()
        .flat_map(|c| &c.legs)
        .map(|s| (s.name.clone(), initial_conditions(s, &data).unwrap()))
        .collect();
    assert_eq!(
        check_identical_ics(&plan, &initial).unwrap(),
        r.shared_ic_hash
    );
    // A non-base body (Venus) whose record differs by one ulp in one leg is refused too, although
    // the base hash is unchanged.
    let mut tampered = initial.clone();
    let venus = BodyName::Venus.naif();
    let (_, last) = tampered
        .iter_mut()
        .rev()
        .find(|(_, ic)| ic.records.iter().any(|x| x.naif == venus))
        .unwrap();
    let rec = last.records.iter_mut().find(|x| x.naif == venus).unwrap();
    rec.position[0][0] = f64::from_bits(rec.position[0][0].to_bits() + 1);
    let e = check_identical_ics(&plan, &tampered).unwrap_err();
    println!("refused: {e}");
    assert_eq!(e.code(), ErrorCode::DiffrunIcMismatch);
    assert!(e.to_string().contains(&format!("NAIF {venus}")));
    initial.push((dd.name.clone(), initial_conditions(&dd, &data).unwrap()));
    let e = check_identical_ics(&plan, &initial).unwrap_err();
    println!("refused: {e}");
    assert_eq!(e.code(), ErrorCode::DiffrunIcMismatch);

    let mut unknown = plan.clone();
    unknown.combinations.push(Combination {
        label: "x".into(),
        protocol: Protocol::Add,
        terms: vec![(1, "base+Mars".into())],
    });
    assert_eq!(
        unknown.validate().unwrap_err().code(),
        ErrorCode::DiffrunPlan
    );
    assert_eq!(
        evaluate(&unknown, &r.configs).unwrap_err().code(),
        ErrorCode::DiffrunPlan
    );
}

/// Check 1 (refusals): malformed body plans are refused with `FA-E1400`.
///
/// Verifies: NUM-013
#[test]
fn malformed_plans_are_refused() {
    use BodyName::{Mercury, Sun, Venus};
    let t = template();
    let bad = [
        Plan::bodies(
            "p",
            &t,
            &[Sun, Mercury],
            &[],
            &[BodyToggle::new("Sun", &[Sun])],
            &[],
            &[1.0],
        ),
        Plan::bodies(
            "p",
            &t,
            &[Sun, Mercury],
            &[],
            &[BodyToggle::new("V", &[Venus])],
            &[(0, 1)],
            &[1.0],
        ),
        Plan::bodies(
            "p",
            &t,
            &[Sun, Mercury],
            &[Sun, Mercury],
            &[BodyToggle::new("V", &[Venus])],
            &[],
            &[1.0],
        ),
        Plan::bodies(
            "p",
            &t,
            &[Sun, Mercury],
            &[],
            &[BodyToggle::new("V", &[])],
            &[],
            &[1.0],
        ),
    ];
    for b in bad {
        let e = b.unwrap_err();
        println!("refused: {e}");
        assert_eq!(e.code(), ErrorCode::DiffrunPlan);
    }
}

/// Check 2: a leg interrupted after two checkpointed segments (store on disk) and resumed by a
/// new call equals the leg run in one piece, bit for bit; the store file is removed when the leg
/// completes; a store file of another scenario is refused (`FA-E1402`).
///
/// Verifies: CI-010, NUM-013
#[test]
fn interrupted_leg_resumes_from_its_store_bitwise() {
    let (h, prov) = common::header();
    let data = DataSources::none().with_header(&h, &prov);
    let mut spec = template();
    spec.bodies = vec![BodyName::Sun, BodyName::Mercury, BodyName::Venus];
    spec.span.years = STORE_LEG_YEARS;
    spec.name = "wp14-store".into();
    let est = common::park17_estimator(
        quadratic(),
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        None,
    );
    let dir = std::env::temp_dir().join(format!("fa-diffrun-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = RunOptions {
        segment_years: SEGMENT_YEARS,
        store: Some(dir.clone()),
        threads: 1,
    };
    let one_piece = run_leg(
        &spec,
        &data,
        &est,
        &RunOptions {
            segment_years: STORE_LEG_YEARS,
            ..RunOptions::default()
        },
        None,
    )
    .unwrap()
    .unwrap();
    assert!(
        run_leg(&spec, &data, &est, &opts, Some(2))
            .unwrap()
            .is_none()
    );
    let file = dir.join(format!("{}.leg", spec.digest()));
    assert!(file.exists());
    let resumed = run_leg(&spec, &data, &est, &opts, None).unwrap().unwrap();
    assert!(resumed.resumed_from_store);
    // It continues with segment 3 of 6 (not from segment 1 again).
    assert_eq!(resumed.segments_run, 4); // lint: allow-literal(6 segments minus the 2 already run)
    assert!(!file.exists(), "a completed leg's store file is removed");
    let words = one_piece.track.differing_words(&resumed.track);
    println!(
        "one piece: {} samples, {} passages, {} steps; resumed after 2 of {} segments ({} run now): differing words {words:?}",
        one_piece.track.t.len(),
        one_piece.track.passages.len(),
        one_piece.steps,
        STORE_LEG_YEARS / SEGMENT_YEARS,
        resumed.segments_run
    );
    assert_eq!(words, Some(0));
    assert_eq!(one_piece.steps, resumed.steps);
    // A foreign store file under this scenario's name is refused.
    assert!(
        run_leg(&spec, &data, &est, &opts, Some(1))
            .unwrap()
            .is_none()
    );
    let mut other = spec.clone();
    other.span.years += 1.0;
    std::fs::rename(&file, dir.join(format!("{}.leg", other.digest()))).unwrap();
    let e = run_leg(&other, &data, &est, &opts, None).unwrap_err();
    println!("refused: {e}");
    assert_eq!(e.code(), ErrorCode::DiffrunStore);
    std::fs::remove_dir_all(&dir).unwrap();
}
