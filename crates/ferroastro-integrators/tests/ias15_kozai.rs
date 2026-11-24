// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The Kozai–Lidov test of Rein & Spiegel 2015, Sec. 4.3 (NUM-001 acceptance: "Passes the Kepler
//! and Kozai tests of Rein & Spiegel 2015").
//!
//! Set-up, from the paper: a central binary of two 1 M☉ stars separated by 1 au, and a 1 M☉
//! perturber at 10 au whose orbit is inclined by 89.9° to the binary's orbital plane. One full
//! Kozai–Lidov cycle is integrated with the default IAS15 settings. Published results: the
//! binary's eccentricity reaches `e_max` ≈ 0.992, the cycle lasts "a few thousand binary orbits",
//! energy is preserved to 10⁻¹² and angular momentum to 10⁻¹⁵, and the result does not depend
//! on the unit system.
//!
//! The paper does not give the initial eccentricities or phases. Choices made here (recorded in
//! `tests/vectors/ias15-kozai.toml`, vector `IAS15-KOZAI-EMAX`): both orbits start circular, the
//! perturber's distance is measured from the binary's barycentre, both relative position vectors
//! lie on the line of nodes, and the total barycentre is at rest at the origin.
//!
//! The asserted energy error is that of the integrator's full compensated state, `x − x_comp`
//! and `v − v_comp` taken from [`Ias15::snapshot`] (IAS15 adds each step with compensated
//! summation and keeps the bits that the f64 positions cannot hold in the compensation words).
//! The energy of the rounded f64 state that [`Ias15::positions`] exposes is printed, not asserted:
//! near pericentre one ulp of a position 3.3 au from the origin moves the energy by about 1e-11.
//! The quantity was changed to the compensated state after the first measurement, by maintainer
//! decision #70 (2026-11-23); the bound was not moved (vector `IAS15-KOZAI-ENERGY-ERROR` notes).
//!
//! Every expected value and bound is loaded from `tests/vectors/ias15-kozai.toml`, which was
//! committed (pre-registered) before this test was written or run. Clean-room (DEP-008): written
//! from the paper only; no REBOUND, `REBOUNDx`, ASSIST or Swiftest source was consulted.

#![expect(
    clippy::indexing_slicing,
    reason = "the test system indexes fixed-length state slices of its own dimension"
)]
#![expect(
    clippy::suboptimal_flops,
    reason = "plain multiply-add in the test force and diagnostics: the results recorded for the \
              pre-registered bounds were measured without fused operations"
)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_integrators::{
    Ias15, Ias15Config, Ias15Snapshot, IntegratorError, SecondOrderSystem,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

const BODIES: usize = 3;
const DIM: usize = 3 * BODIES;
/// First trial step as a fraction of the binary period; the controller adapts it.
const FIRST_STEP_PER_PERIOD: f64 = 1.0 / 64.0; // lint: allow-literal(first trial step fraction)
/// The cycle is "armed" once the binary's eccentricity has exceeded this value …
const E_ARM: f64 = 0.5;
/// … and ends at the first accepted step where it is below this one (vector notes).
const E_RETURN: f64 = 0.1; // lint: allow-literal(cycle-end threshold, IAS15-KOZAI-EMAX notes)

/// Newtonian point masses, `ẍᵢ = Σⱼ GMⱼ (xⱼ − xᵢ)/|xⱼ − xᵢ|³`, state `[x₀ y₀ z₀ x₁ …]`.
struct ThreeBody {
    gm: [f64; BODIES],
}

impl SecondOrderSystem for ThreeBody {
    fn dimension(&self) -> usize {
        DIM
    }
    fn acceleration(
        &self,
        _t: f64,
        x: &[f64],
        _v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        a.fill(0.0);
        for i in 0..BODIES {
            for j in (i + 1)..BODIES {
                let d = [
                    x[3 * j] - x[3 * i],
                    x[3 * j + 1] - x[3 * i + 1],
                    x[3 * j + 2] - x[3 * i + 2],
                ];
                let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                let k = 1.0 / (r2 * r2.sqrt());
                for c in 0..3 {
                    a[3 * i + c] += self.gm[j] * k * d[c];
                    a[3 * j + c] -= self.gm[i] * k * d[c];
                }
            }
        }
        Ok(())
    }
}

type Dd = DoubleDouble;

fn dd3(s: &[f64], body: usize) -> [Dd; 3] {
    [
        Dd::from_f64(s[3 * body]),
        Dd::from_f64(s[3 * body + 1]),
        Dd::from_f64(s[3 * body + 2]),
    ]
}

/// The f64 state widened to double-double without change.
fn widen(s: &[f64]) -> [Dd; DIM] {
    let mut w = [Dd::ZERO; DIM];
    for (wi, &si) in w.iter_mut().zip(s) {
        *wi = Dd::from_f64(si);
    }
    w
}

/// The integrator's full compensated state, `x − x_comp` and `v − v_comp` (Kahan: the sum minus
/// its compensation), formed exactly in double-double from a snapshot.
fn compensated_state(snap: &Ias15Snapshot) -> ([Dd; DIM], [Dd; DIM]) {
    let join = |s: &[[f64; 2]], c: &[[f64; 2]]| {
        let mut w = [Dd::ZERO; DIM];
        for ((wi, si), ci) in w.iter_mut().zip(s).zip(c) {
            *wi = Dd::from_parts(si[0], si[1]) - Dd::from_parts(ci[0], ci[1]);
        }
        w
    };
    (join(&snap.x, &snap.x_comp), join(&snap.v, &snap.v_comp))
}

fn dot(a: &[Dd; 3], b: &[Dd; 3]) -> Dd {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn body3(s: &[Dd; DIM], body: usize) -> [Dd; 3] {
    [s[3 * body], s[3 * body + 1], s[3 * body + 2]]
}

/// `G·E = Σ ½ GMᵢ vᵢ² − Σ GMᵢ GMⱼ / rᵢⱼ` of a double-double state, evaluated in double-double.
fn energy(gm: &[f64; BODIES], x: &[Dd; DIM], v: &[Dd; DIM]) -> Dd {
    let mut e = Dd::ZERO;
    for i in 0..BODIES {
        let vi = body3(v, i);
        e += Dd::from_f64(gm[i]) * dot(&vi, &vi) * Dd::HALF;
        let xi = body3(x, i);
        for j in (i + 1)..BODIES {
            let xj = body3(x, j);
            let d = [xj[0] - xi[0], xj[1] - xi[1], xj[2] - xi[2]];
            e -= Dd::from_f64(gm[i]) * Dd::from_f64(gm[j]) / dot(&d, &d).sqrt();
        }
    }
    e
}

/// `G·L = Σ GMᵢ xᵢ × vᵢ` of the f64 state, evaluated in double-double.
fn angular_momentum(gm: &[f64; BODIES], x: &[f64], v: &[f64]) -> [Dd; 3] {
    let mut l = [Dd::ZERO; 3];
    for (i, &gm_i) in gm.iter().enumerate() {
        let (r, w, m) = (dd3(x, i), dd3(v, i), Dd::from_f64(gm_i));
        l[0] += m * (r[1] * w[2] - r[2] * w[1]);
        l[1] += m * (r[2] * w[0] - r[0] * w[2]);
        l[2] += m * (r[0] * w[1] - r[1] * w[0]);
    }
    l
}

/// Osculating eccentricity of the inner binary: `|e|`, `μ e = (v² − μ/r) r − (r·v) v` for the
/// relative orbit of body 1 about body 0, `μ = GM₀ + GM₁`.
fn binary_eccentricity(gm: &[f64; BODIES], x: &[f64], v: &[f64]) -> f64 {
    let mu = gm[0] + gm[1];
    let r = [x[3] - x[0], x[4] - x[1], x[5] - x[2]];
    let w = [v[3] - v[0], v[4] - v[1], v[5] - v[2]];
    let rn = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let w2 = w[0] * w[0] + w[1] * w[1] + w[2] * w[2];
    let rw = r[0] * w[0] + r[1] * w[1] + r[2] * w[2];
    let ev = [0, 1, 2].map(|c| ((w2 - mu / rn) * r[c] - rw * w[c]) / mu);
    (ev[0] * ev[0] + ev[1] * ev[1] + ev[2] * ev[2]).sqrt()
}

/// What one integrated cycle measured.
struct Cycle {
    completed: bool,
    e_max: f64,
    /// Asserted: worst energy error of the compensated state.
    energy_error: f64,
    angmom_error: f64,
    /// Reported only: the worst energy error of the rounded f64 state that `positions()` and
    /// `velocities()` expose, and where it fell (binary periods, eccentricity).
    energy_error_rounded: f64,
    worst_rounded_at: (f64, f64),
    /// Reported only: the errors at the last step of the cycle (energy: compensated state), and
    /// where the worst compensated energy sample fell (binary periods since the start, and the
    /// eccentricity there).
    energy_error_final: f64,
    angmom_error_final: f64,
    worst_energy_at: (f64, f64),
    binary_orbits: f64,
    steps: u64,
    rejected: u64,
    unconverged: u64,
}

fn count(n: u64) -> f64 {
    f64::from(u32::try_from(n).unwrap())
}

/// Masses, barycentric initial state and binary period in the unit system where the solar GM is
/// `gm_sun` and the astronomical unit is `au`.
fn initial_state(
    vs: &VectorSet,
    gm_sun: f64,
    au: f64,
) -> ([f64; BODIES], [f64; DIM], [f64; DIM], f64) {
    let val = |id: &str| vs.get(id).unwrap().value().unwrap();
    let gm_body = val("IAS15-KOZAI-BODY-MASS-MSUN") * gm_sun;
    let gm = [gm_body; BODIES];
    let a_in = val("IAS15-KOZAI-BINARY-SEPARATION-AU") * au;
    let a_out = val("IAS15-KOZAI-PERTURBER-DISTANCE-AU") * au;
    let inc = val("IAS15-KOZAI-MUTUAL-INCLINATION-DEG").to_radians();

    // Jacobi set-up: inner relative orbit (body 1 − body 0) and outer relative orbit (perturber −
    // binary barycentre), both circular, both starting on the line of nodes (the x axis).
    let (gm_bin, gm_tot) = (gm[0] + gm[1], gm[0] + gm[1] + gm[2]);
    let r_in = [a_in, 0.0, 0.0];
    let v_in = [0.0, (gm_bin / a_in).sqrt(), 0.0];
    let vc = (gm_tot / a_out).sqrt();
    let r_out = [a_out, 0.0, 0.0];
    let v_out = [0.0, vc * inc.cos(), vc * inc.sin()];
    // Barycentric states: binary barycentre at −(GM₂/GM_tot)·out, perturber at +(GM_bin/GM_tot)·out.
    let place = |rel_in: &[f64; 3], rel_out: &[f64; 3]| {
        let mut s = [0.0; DIM];
        for c in 0..3 {
            let bin = -(gm[2] / gm_tot) * rel_out[c];
            s[c] = bin - (gm[1] / gm_bin) * rel_in[c];
            s[3 + c] = bin + (gm[0] / gm_bin) * rel_in[c];
            s[6 + c] = (gm_bin / gm_tot) * rel_out[c];
        }
        s
    };
    let period = std::f64::consts::TAU * (a_in * a_in * a_in / gm_bin).sqrt();
    (gm, place(&r_in, &r_out), place(&v_in, &v_out), period)
}

/// Integrate one Kozai–Lidov cycle in the unit system where the solar GM is `gm_sun` and the
/// astronomical unit is `au` (time unit: whatever makes them consistent).
fn kozai_cycle(vs: &VectorSet, gm_sun: f64, au: f64) -> Cycle {
    let (gm, x0, v0, period) = initial_state(vs, gm_sun, au);
    let cap = vs.get("IAS15-KOZAI-CYCLE-BINARY-ORBITS").unwrap();
    let max_orbits = cap.value().unwrap() + cap.tolerance.allowed(cap.value().unwrap());
    let t_cap = max_orbits * period;
    let sys = ThreeBody { gm };
    let energy0 = energy(&gm, &widen(&x0), &widen(&v0));
    let l0 = angular_momentum(&gm, &x0, &v0);
    let l0_norm = dot(&l0, &l0).sqrt();

    let mut ias = Ias15::new(
        DIM,
        0.0,
        &x0,
        &v0,
        period * FIRST_STEP_PER_PERIOD,
        Ias15Config::default(),
    )
    .unwrap();
    let mut out = Cycle {
        completed: false,
        e_max: 0.0,
        energy_error: 0.0,
        angmom_error: 0.0,
        energy_error_rounded: 0.0,
        worst_rounded_at: (0.0, 0.0),
        energy_error_final: 0.0,
        angmom_error_final: 0.0,
        worst_energy_at: (0.0, 0.0),
        binary_orbits: 0.0,
        steps: 0,
        rejected: 0,
        unconverged: 0,
    };
    let mut armed = false;
    loop {
        let report = ias.step(&sys, t_cap).unwrap();
        let (x, v) = (ias.positions(), ias.velocities());
        let e = binary_eccentricity(&gm, x, v);
        out.e_max = out.e_max.max(e);
        let rel = |e: Dd| ((e - energy0) / energy0).to_f64().abs();
        let de_rounded = rel(energy(&gm, &widen(x), &widen(v)));
        if de_rounded > out.energy_error_rounded {
            out.energy_error_rounded = de_rounded;
            out.worst_rounded_at = (ias.time() / period, e);
        }
        let (xc, vc) = compensated_state(&ias.snapshot().unwrap());
        let de = rel(energy(&gm, &xc, &vc));
        if de > out.energy_error {
            out.energy_error = de;
            out.worst_energy_at = (ias.time() / period, e);
        }
        out.energy_error_final = de;
        let l = angular_momentum(&gm, x, v);
        let dl = [l[0] - l0[0], l[1] - l0[1], l[2] - l0[2]];
        let dl = (dot(&dl, &dl).sqrt() / l0_norm).to_f64();
        out.angmom_error = out.angmom_error.max(dl);
        out.angmom_error_final = dl;
        armed |= e > E_ARM;
        if armed && e < E_RETURN {
            out.completed = true;
            break;
        }
        if report.reached_limit {
            break;
        }
    }
    let st = ias.stats();
    out.binary_orbits = ias.time() / period;
    out.steps = st.steps;
    out.rejected = st.rejected;
    out.unconverged = st.unconverged;
    println!(
        "Kozai-Lidov cycle (GM_sun = {gm_sun:e}, au = {au:e}): completed = {}, {:.1} binary \
         periods, {} steps, {} rejected, {} unconverged, {} force evaluations; e_max = {:.6}, \
         max |dE/E| (compensated state) = {:.3e}, max |dL|/|L| = {:.3e}; reported only: worst \
         compensated energy sample at {:.1} binary periods where e = {:.4}; max |dE/E| of the \
         rounded f64 state = {:.3e}, at {:.1} binary periods where e = {:.4}; at the end of the \
         cycle |dE/E| (compensated) = {:.3e}, |dL|/|L| = {:.3e}",
        out.completed,
        out.binary_orbits,
        st.steps,
        st.rejected,
        st.unconverged,
        st.force_evaluations,
        out.e_max,
        out.energy_error,
        out.angmom_error,
        out.worst_energy_at.0,
        out.worst_energy_at.1,
        out.energy_error_rounded,
        out.worst_rounded_at.0,
        out.worst_rounded_at.1,
        out.energy_error_final,
        out.angmom_error_final,
    );
    out
}

/// Check one cycle against every published value; returns the failures, having printed each
/// comparison.
fn check_cycle(vs: &VectorSet, label: &str, c: &Cycle) -> Vec<String> {
    let mut failures = Vec::new();
    if !c.completed {
        failures.push(format!(
            "{label}: no full cycle within {:.0} binary periods",
            c.binary_orbits
        ));
    }
    if c.unconverged != 0 {
        failures.push(format!("{label}: {} unconverged steps", c.unconverged));
    }
    for (id, measured) in [
        ("IAS15-KOZAI-EMAX", c.e_max),
        ("IAS15-KOZAI-CYCLE-BINARY-ORBITS", c.binary_orbits),
        ("IAS15-KOZAI-ENERGY-ERROR", c.energy_error),
        ("IAS15-KOZAI-ANGMOM-ERROR", c.angmom_error),
    ] {
        let vec = vs.get(id).unwrap();
        let verdict = vec.check(measured);
        println!(
            "  {label}: {id}: measured {measured:e}, expected {} +/- {:e}: {}",
            vec.value().unwrap(),
            vec.tolerance.allowed(vec.value().unwrap()),
            if verdict.is_ok() { "ok" } else { "FAIL" }
        );
        if let Err(e) = verdict {
            failures.push(format!("{label}: {e}"));
        }
    }
    failures
}

/// Rein & Spiegel 2015, Sec. 4.3, in the paper's units (solar masses, au, G = 1): one cycle
/// reaches the published maximum eccentricity within the published number of binary orbits, and
/// conserves energy (of the integrator's compensated state) and angular momentum at the published
/// level.
///
/// Verifies: NUM-001
#[test]
fn kozai_lidov_cycle_matches_rein_spiegel_2015() {
    let vs = VectorSet::load_canonical().unwrap();
    let cycle = kozai_cycle(&vs, 1.0, 1.0);
    let failures = check_cycle(&vs, "G = 1", &cycle);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Rein & Spiegel 2015, Sec. 4.3, scale independence: the same cycle in km and seconds (DE440
/// GM☉, IAU 2012 au) meets the same published values, and the step control takes the same number
/// of steps as in G = 1 units.
///
/// Verifies: NUM-001
#[test]
fn kozai_lidov_cycle_is_scale_independent() {
    let vs = VectorSet::load_canonical().unwrap();
    let val = |id: &str| vs.get(id).unwrap().value().unwrap();
    let unit = kozai_cycle(&vs, 1.0, 1.0);
    let km_s = kozai_cycle(&vs, val("DE440-GMS-KM3S2"), val("CONST-AU-IAU2012"));
    let mut failures = check_cycle(&vs, "km, s", &km_s);
    let ratio = count(km_s.steps) / count(unit.steps);
    let vec = vs.get("IAS15-KOZAI-SCALE-STEP-RATIO").unwrap();
    println!(
        "  step-count ratio (km, s)/(G = 1): {} / {} = {ratio:.6}; rejected {} / {}",
        km_s.steps, unit.steps, km_s.rejected, unit.rejected
    );
    if let Err(e) = vec.check(ratio) {
        failures.push(e.to_string());
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
