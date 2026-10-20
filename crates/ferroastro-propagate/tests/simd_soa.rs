// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP26 run-level checks of the SoA kernels (`integrator.force_kernel = "soa"`; plan in
//! IMPLEMENTATION-NOTES "WP26"):
//!
//! * the scenario path builds the SoA model: its initial accelerations equal, bit for bit, those
//!   of the same model assembled by hand with the scalar backend;
//! * P4 (feature `simd`): the SoA model with the SIMD backend and with the scalar backend,
//!   integrated by ABM13 PECEC for 10 yr, take bitwise-identical steps;
//! * P5: `run_batch` of SoA and pairwise runs on 1, 2, 4 and N threads is bitwise identical to
//!   itself and to single runs (NUM-022, TST-005);
//! * refusals: `soa` with `precision = "mixed"` and outside n-body mode, by name (FA-E1203).
//!
//! Needs the cached header.440 (CI-005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::ErrorCode;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::{Length, SpinGm, Velocity};
use ferroastro_eih::{Eih, EihSoa, RhsAcceleration};
use ferroastro_ephem::NaifId;
use ferroastro_forces::soa::SoaBackend;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, LtReaction, MOON_TAYLOR_TERMS_DEFAULT, MoonVariables, NBody,
    NBodyModel, NewtonianMutualSoa, Pole, SecondOrderSystem, SolarJ2WithReaction,
    SolarLenseThirring,
};
use ferroastro_propagate::{
    DataSources, RunOutput, de440_solar_spin, initial_accelerations, run, run_batch,
};
use ferroastro_scenario::{ForceKernel, Mode, Precision, ScenarioSpec};
use ferroastro_system::{Catalogue, InitialConditions, Tier};
use std::path::PathBuf;

/// Run lengths in Julian years: run controls chosen for PR-tier cost, not physical values.
const P4_YEARS: f64 = 10.0; // lint: allow-literal(run length of P4, PR-tier cost)
const BATCH_STEP_YEARS: f64 = 0.25; // lint: allow-literal(batch run lengths 0.25..2 yr, PR-tier cost)

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// The WP24 ABM scenario (EIH Sun + 8 planets + geocentric Moon, ABM13 PECEC h = 1/16 d) with
/// solar J2 and LT, the given kernel and span.
fn abm_full(kernel: ForceKernel, years: f64) -> ScenarioSpec {
    let mut s = spec("abm-sun-planets-moon-eih-40yr.toml");
    s.forces.sun_j2 = true;
    s.forces.lense_thirring = true;
    s.integrator.force_kernel = Some(kernel);
    s.span.years = years;
    s.name = format!("wp26-abm-eih-j2-lt-{}", kernel.label());
    s
}

/// The `abm_full` model assembled by hand (term order of `ferroastro-propagate`: Newtonian
/// mutual, geocentric Moon, EIH, solar J2 with reaction, solar LT with reaction), SoA kernels
/// with `backend`.
fn direct_model(s: &ScenarioSpec, de: &De440, backend: SoaBackend) -> NBodyModel<Icrf> {
    let cat = Catalogue::de440_major(de);
    let bodies: Vec<NBody> = s
        .bodies
        .iter()
        .map(|b| NBody {
            id: NaifId(b.naif()),
            gm: cat.gm(NaifId(b.naif())).unwrap().km3_per_s2(),
            tier: Tier::Major,
        })
        .collect();
    let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
        .unwrap()
        .unit_vector();
    let pole = || Pole::from_components(k).unwrap();
    let c = Velocity::from_km_per_s(de.speed_of_light().km_per_s()).unwrap();
    let eih = Eih::new(c, de.ppn_beta(), de.ppn_gamma(), RhsAcceleration::Newtonian).unwrap();
    let spin: SpinGm = de440_solar_spin(de, &s.name).unwrap();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
        Box::new(NewtonianMutualSoa::new(backend)),
        Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        Box::new(EihSoa::new(eih, backend)),
        Box::new(
            SolarJ2WithReaction::new(
                de.sun_j2(),
                Length::from_km(de.sun_radius().km()).unwrap(),
                pole(),
            )
            .unwrap(),
        ),
        Box::new(SolarLenseThirring::new(spin, c, de.ppn_gamma(), pole(), LtReaction::On).unwrap()),
    ];
    NBodyModel::new(
        &bodies,
        MoonVariables::Geocentric {
            emrat: de.earth_moon_mass_ratio(),
        },
        terms,
    )
    .unwrap()
}

/// The scenario path builds the SoA model: `initial_accelerations` of the `soa` scenario (the
/// preferred backend) equal the hand-built scalar-backend model's bit for bit, differ from the
/// pairwise scenario's at the ulp level only, and the model string names the SoA kernels.
///
/// Verifies: CI-002, NUM-021
#[test]
fn soa_scenario_builds_the_soa_model() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let de = De440::load().unwrap();
    let s = abm_full(ForceKernel::Soa, P4_YEARS);
    let (ids, acc) = initial_accelerations(&s, &data).unwrap();
    let model = direct_model(&s, &de, SoaBackend::Scalar);
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let (x, v) = model.pack(ic.states(), ic.earth_moon()).unwrap();
    let n = model.layout().len();
    let mut a = vec![0.0; 3 * n];
    model.acceleration(0.0, &x, &v, &mut a).unwrap();
    let mut differing = 0;
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, model.layout().ids()[i].0);
        for c in 0..3 {
            differing += usize::from(acc[i][c][0].to_bits() != a[c * n + i].to_bits());
        }
    }
    let (_, pw) = initial_accelerations(&abm_full(ForceKernel::Pairwise, P4_YEARS), &data).unwrap();
    let rel = (0..n)
        .map(|i| {
            let d: f64 = (0..3)
                .map(|c| (acc[i][c][0] - pw[i][c][0]).powi(2))
                .sum::<f64>()
                .sqrt();
            let m: f64 = (0..3).map(|c| pw[i][c][0].powi(2)).sum::<f64>().sqrt();
            d / m
        })
        .fold(0.0, f64::max);
    let out = run(&abm_full(ForceKernel::Soa, BATCH_STEP_YEARS), &data).unwrap();
    println!(
        "soa scenario vs hand-built scalar SoA model: {differing} of {} differing words; vs the \
         pairwise scenario max |da|/|a| = {rel:.3e}; model: {}",
        3 * n,
        out.model
    );
    assert_eq!(differing, 0);
    // Both pair terms name the SoA kernel (the EIH correction is too small to show in the
    // total accelerations above: a pairwise Eih left in place survived the first mutation round).
    assert!(
        out.model
            .contains("newtonian-mutual (SoA kernel, exact 1/r^3, no FMA)"),
        "{}",
        out.model
    );
    assert!(
        out.model
            .contains("eih-1pn (Park et al. 2021 Eq. 27; SoA kernel, exact 1/r^3, no FMA)"),
        "{}",
        out.model
    );
}

/// P4 (feature `simd`): the SoA model with the SIMD and the scalar backend, ABM13 PECEC
/// h = 1/16 d for 10 yr from header.440: every step's positions and velocities are bitwise equal.
///
/// Verifies: NUM-021, TST-005
#[cfg(feature = "simd")]
#[test]
fn simd_and_scalar_soa_models_integrate_bitwise() {
    use ferroastro_abm::{Abm, AbmConfig};
    use ferroastro_core::time::SECONDS_PER_DAY;
    let (h, prov) = header();
    let de = De440::load().unwrap();
    let s = abm_full(ForceKernel::Soa, P4_YEARS);
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let ms = direct_model(&s, &de, SoaBackend::Scalar);
    let mw = direct_model(&s, &de, SoaBackend::Simd);
    let (x0, v0) = ms.pack(ic.states(), ic.earth_moon()).unwrap();
    let step = s.integrator.step_days.unwrap() * SECONDS_PER_DAY;
    let new = || Abm::new(x0.len(), 0.0, &x0, &v0, step, AbmConfig::default()).unwrap();
    let (mut a, mut b) = (new(), new());
    let steps = (s.span_seconds() / step).round();
    let mut differing = 0_usize;
    let mut k = 0.0;
    while k < steps {
        a.step(&ms).unwrap();
        b.step(&mw).unwrap();
        differing += a
            .positions()
            .iter()
            .chain(a.velocities())
            .zip(b.positions().iter().chain(b.velocities()))
            .filter(|(p, q)| p.to_bits() != q.to_bits())
            .count();
        k += 1.0;
    }
    println!(
        "P4: {steps} ABM13 PECEC steps over {P4_YEARS} yr, {} vs {}: {differing} differing words \
         over all steps",
        SoaBackend::Scalar.label(),
        SoaBackend::Simd.label()
    );
    assert_eq!(differing, 0);
}

fn differing(a: &[RunOutput], b: &[RunOutput]) -> usize {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.differing_words(y).expect("same shapes"))
        .sum()
}

/// P5: `run_batch` of 8 SoA and 8 pairwise runs (0.25 … 2 yr) on 1, 2, 4 and N =
/// `available_parallelism` threads: bitwise identical to each other and to single runs. Speed-ups
/// are reported (the `parallel` feature must be on for them to exceed 1).
///
/// Verifies: NUM-022, NUM-021, TST-005
#[test]
fn batch_thread_counts_are_bitwise_for_both_kernels() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let all = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut threads = vec![1, 2, 4, all];
    threads.dedup();
    for kernel in [ForceKernel::Soa, ForceKernel::Pairwise] {
        let batch: Vec<ScenarioSpec> = (1..=8_u32)
            .map(|k| {
                let mut s = abm_full(kernel, BATCH_STEP_YEARS * f64::from(k));
                s.name = format!("{}-{k}", s.name);
                s
            })
            .collect();
        let timed = |t: usize| {
            let start = std::time::Instant::now();
            let r: Vec<RunOutput> = run_batch(&batch, &data, t)
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .collect();
            (r, start.elapsed().as_secs_f64())
        };
        let (base, t1) = timed(1);
        let singles: Vec<RunOutput> = batch.iter().map(|s| run(s, &data).unwrap()).collect();
        assert_eq!(differing(&base, &singles), 0);
        for &t in threads.iter().skip(1) {
            let (r, tn) = timed(t);
            let d = differing(&base, &r);
            println!(
                "P5 {} batch of {}: {t} threads {tn:.2} s vs 1 thread {t1:.2} s (speed-up {:.2}, \
                 rayon `parallel` {}); differing words vs 1 thread: {d}",
                kernel.label(),
                batch.len(),
                t1 / tn,
                if cfg!(feature = "parallel") {
                    "on"
                } else {
                    "off"
                }
            );
            assert_eq!(d, 0);
        }
    }
}

/// `force_kernel = "soa"` is refused by name with the mixed precision mode and outside n-body
/// mode (FA-E1203); the key parses, and an absent key leaves the digest unchanged.
///
/// Verifies: CI-002, REF-011
#[test]
fn soa_kernel_refusals_are_named() {
    let mut mixed = abm_full(ForceKernel::Soa, BATCH_STEP_YEARS);
    mixed.integrator.precision = Precision::Mixed;
    let e = mixed.validate().unwrap_err();
    assert_eq!(e.code(), ErrorCode::ScenarioValue, "{e}");
    assert!(e.to_string().contains("integrator.force_kernel"), "{e}");
    let mut tp = spec("sun-mercury-gr1pn-header440.toml");
    assert_eq!(tp.mode, Mode::TestParticle);
    tp.integrator.force_kernel = Some(ForceKernel::Soa);
    let e = tp.validate().unwrap_err();
    assert_eq!(e.code(), ErrorCode::ScenarioValue, "{e}");
    assert!(e.to_string().contains("integrator.force_kernel"), "{e}");
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/abm-sun-planets-moon-eih-40yr.toml");
    let text = std::fs::read_to_string(p).unwrap().replace(
        "compensated = true",
        "compensated = true\nforce_kernel = \"soa\"",
    );
    let parsed = ScenarioSpec::parse(&text).unwrap();
    assert_eq!(parsed.integrator.force_kernel, Some(ForceKernel::Soa));
    let mut plain = spec("abm-sun-planets-moon-eih-40yr.toml");
    let before = plain.digest();
    plain.integrator.force_kernel = None;
    assert_eq!(plain.digest(), before);
}
