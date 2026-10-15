// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-014 / PHY-010: the frame definitions of `ferroastro-frames` match their cited sources
//! (vectors `PARK17-MEANORBIT-POLE-*`, `ECLIPJ2000-OBLIQUITY`, `SUN-POLE-IAU-*`), the rotations
//! are proper and invert, a configurable solar pole is recorded in the rotation's metadata, and
//! inertial ICRF is reachable from every frame only through an explicit rotation.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_core::ErrorCode;
use ferroastro_core::frames::{
    EclipticJ2000, Frame, Heliocentric, Icrf, MercuryMeanOrbit, Position, SolarEquator,
    SunEquatorIau,
};
use ferroastro_core::time::{Epoch, Tt};
use ferroastro_core::units::Length;
use ferroastro_frames::{
    ECLIPJ2000_OBLIQUITY_ARCSEC, PARK2017_POLE_DEC_DEG, PARK2017_POLE_RA_DEG, Pole, Rotation,
    SOLAR_POLE_IAU_DEC_DEG, SOLAR_POLE_IAU_RA_DEG, icrf_to_ecliptic_j2000,
    icrf_to_ecliptic_of_date, icrf_to_mean_orbit, icrf_to_solar_equator, icrf_to_sun_equator_iau,
};
use ferroastro_vectors::VectorSet;

fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

fn assert_proper<F: Frame, T: Frame>(r: &Rotation<F, T>) {
    // Rebuilding through the checked constructor proves orthonormality and det = +1.
    Rotation::<F, T>::from_matrix(*r.matrix(), r.definition()).unwrap();
}

/// Verifies: NUM-014, NUM-011
#[test]
fn mean_orbit_frame_is_park17_pole_with_park17_axes() {
    let v = vectors();
    v.get("PARK17-MEANORBIT-POLE-RA")
        .unwrap()
        .check(PARK2017_POLE_RA_DEG)
        .unwrap();
    v.get("PARK17-MEANORBIT-POLE-DEC")
        .unwrap()
        .check(PARK2017_POLE_DEC_DEG)
        .unwrap();
    let pole = Pole::<f64>::park2017_mercury_mean_orbit().unwrap();
    let r = icrf_to_mean_orbit(&pole).unwrap();
    assert_proper(&r);
    let z = r.axis(2).unwrap();
    // z is the pole (to rounding), y is perpendicular to the ICRF x-axis (Park17's y = z × x̂).
    let p = pole.unit_vector();
    assert!(1.0 - dot(z, p) < 4.0 * f64::EPSILON);
    assert!(r.axis(1).unwrap()[0].abs() < 4.0 * f64::EPSILON);
    assert!(r.definition().contains("280.9876") && r.definition().contains("61.4481"));
}

/// Verifies: NUM-014
#[test]
fn ecliptic_j2000_is_the_horizons_obliquity_rotation() {
    let v = vectors();
    v.get("ECLIPJ2000-OBLIQUITY")
        .unwrap()
        .check(ECLIPJ2000_OBLIQUITY_ARCSEC)
        .unwrap();
    let r = icrf_to_ecliptic_j2000::<f64>();
    assert_proper(&r);
    // The ecliptic pole in ICRF is (0, −sin ε, cos ε); the x-axis is the ICRF x-axis.
    let [x, _, z] = [r.axis(0).unwrap(), r.axis(1).unwrap(), r.axis(2).unwrap()];
    assert!(
        x.iter()
            .zip([1.0, 0.0, 0.0])
            .all(|(a, b)| (a - b).abs() == 0.0)
    );
    let eps = (-z[1]).atan2(z[2]).to_degrees() * 3600.0; // lint: allow-literal(arcsec per degree)
    let tol = 64.0 * f64::EPSILON * ECLIPJ2000_OBLIQUITY_ARCSEC; // lint: allow-literal(round-off bound in epsilons, not physics)
    assert!((eps - ECLIPJ2000_OBLIQUITY_ARCSEC).abs() < tol);
    // Report (not assert) how far the IAU 2006 J2000 ecliptic (ecm06 at J2000, frame bias and
    // eps0 = 84381.406″ included) lies from ECLIPJ2000: the angle between the two poles and
    // between the two x-axes.
    let iau = icrf_to_ecliptic_of_date(Epoch::<Tt>::j2000());
    let ang = |a: [f64; 3], b: [f64; 3]| dot(a, b).clamp(-1.0, 1.0).acos().to_degrees() * 3600.0; // lint: allow-literal(arcsec per degree)
    println!(
        "IAU 2006 J2000 ecliptic vs ECLIPJ2000: poles {:.6} arcsec apart, x-axes {:.6} arcsec apart",
        ang(z, iau.axis(2).unwrap()),
        ang(x, iau.axis(0).unwrap())
    );
}

/// Verifies: PHY-010, NUM-014
#[test]
fn solar_equator_pole_is_configurable_and_recorded() {
    let v = vectors();
    v.get("SUN-POLE-IAU-RA")
        .unwrap()
        .check(SOLAR_POLE_IAU_RA_DEG)
        .unwrap();
    v.get("SUN-POLE-IAU-DEC")
        .unwrap()
        .check(SOLAR_POLE_IAU_DEC_DEG)
        .unwrap();
    let iau: Rotation<Icrf, SunEquatorIau> = icrf_to_sun_equator_iau().unwrap();
    assert_proper(&iau);
    // The node (x-axis) lies in the ICRF equator; z is the pole.
    assert!(iau.axis(0).unwrap()[2].abs() < 4.0 * f64::EPSILON);
    // A different pole (here: the Park17 mean-orbit pole, as an arbitrary test direction) gives a
    // different rotation and is named in its definition.
    let custom =
        Pole::<f64>::from_degrees(PARK2017_POLE_RA_DEG, PARK2017_POLE_DEC_DEG, "test pole")
            .unwrap();
    let r: Rotation<Icrf, SolarEquator> = icrf_to_solar_equator(&custom).unwrap();
    assert_proper(&r);
    assert!(r.definition().contains("test pole") && r.definition().contains("280.9876"));
    assert!(iau.definition().contains("Archinal") && iau.definition().contains("286.13"));
    assert_ne!(r.matrix(), iau.matrix());
}

/// Verifies: NUM-014, API-001
#[test]
fn rotations_invert_compose_and_keep_frames_explicit() {
    let km = |x: f64| Length::from_km(x).unwrap();
    let p = Position::<Icrf, Heliocentric>::new(km(4.6e7), km(-2.1e7), km(-6.0e6)); // lint: allow-literal(arbitrary test position)
    let to_ecl = icrf_to_ecliptic_j2000::<f64>();
    let to_mo = icrf_to_mean_orbit(&Pole::park2017_mercury_mean_orbit().unwrap()).unwrap();
    // ECLIPJ2000 → ICRF → mean orbit, then back to ICRF.
    let ecl_to_mo: Rotation<EclipticJ2000, MercuryMeanOrbit> = to_ecl.inverse().then(&to_mo);
    let q = ecl_to_mo.position(&to_ecl.position(&p).unwrap()).unwrap();
    let back: Position<Icrf, Heliocentric> = to_mo.inverse().position(&q).unwrap();
    let d = (back - p).norm().km();
    assert!(d < 1e-15 * 5.1e7, "round trip moved the position by {d} km"); // lint: allow-literal(round-off bound for |r| ~ 5e7 km)
    // Default output frame: nothing converts implicitly; the identity is the only same-frame map.
    let id = Rotation::<Icrf, Icrf>::identity();
    assert_eq!(id.position(&p).unwrap(), p);
}

/// Verifies: NUM-014, API-002
#[test]
fn non_rotations_and_degenerate_poles_are_rejected_with_codes() {
    let shear = [[1.0, 0.1, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]; // lint: allow-literal(arbitrary shear, not a rotation)
    let e = Rotation::<Icrf, EclipticJ2000>::from_matrix(shear, "shear").unwrap_err();
    assert_eq!(e.code(), ErrorCode::FrameDefinition);
    assert!(e.to_string().starts_with("FA-E0900"));
    let reflection = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]];
    assert!(Rotation::<Icrf, EclipticJ2000>::from_matrix(reflection, "mirror").is_err());
    // A pole along the ICRF x-axis has no Park17 y-axis; one along z has no equator node.
    let along_x = Pole::<f64>::from_degrees(0.0, 0.0, "x").unwrap();
    assert_eq!(
        icrf_to_mean_orbit(&along_x).unwrap_err().code(),
        ErrorCode::FrameDefinition
    );
    let along_z = Pole::<f64>::from_degrees(0.0, 90.0, "z").unwrap(); // lint: allow-literal(celestial pole)
    assert!(icrf_to_solar_equator(&along_z).is_err());
}
