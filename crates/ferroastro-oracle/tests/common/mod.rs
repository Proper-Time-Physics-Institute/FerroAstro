// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared checks of the oracle comparison (progress review after WP18, R2).
//!
//! [`check_against_brute_force`] recomputes |Δr| for **every** golden row with its own loop and
//! its own norm, independent of `compare_states`, and asserts that each body's reported
//! `max_dr_km` is the brute-force maximum (≤ [`MAX_ULPS`] ulp: the two norms round differently)
//! and that `t_days_at_max_dr` is the time of that maximum. A comparison that keeps the minimum,
//! the first or the last row instead of the maximum fails here.

#![allow(
    dead_code,
    reason = "each test binary uses a different subset of the helpers"
)]
#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helper: a failed unwrap or index is a test failure; indices are 0..3"
)]

use ferroastro_oracle::{GoldenStates, StateComparison, TIME_MATCH_S};
use ferroastro_propagate::RunOutput;

/// Largest allowed difference between the reported and the brute-force maxima, in units in the
/// last place: `compare_states` uses `hypot`, this check `sqrt` of a fused sum of squares; each is within
/// ~1–2 ulp of the exact norm.
pub(crate) const MAX_ULPS: f64 = 4.0; // lint: allow-literal(ulp count of two norm formulas, review R2)

/// Assert that `cmp` reports, per body and overall, the brute-force maximum |Δr| over every row
/// of `golden` against `run`, at the time of that maximum.
pub(crate) fn check_against_brute_force(
    golden: &GoldenStates,
    run: &RunOutput,
    cmp: &StateComparison,
) {
    let center = golden.center_naif().unwrap();
    let val = |p: [f64; 2]| p[0] + p[1];
    let records: Vec<_> = run.states.iter().chain(run.final_state.as_ref()).collect();
    // (naif, max dr, t at max) per body, in first-seen order.
    let mut brute: Vec<(i32, f64, f64)> = Vec::new();
    for g in &golden.rows {
        let t_s = g.t_days * ferroastro_core::time::SECONDS_PER_DAY;
        let rec = records
            .iter()
            .find(|r| (val(r.t) - t_s).abs() <= TIME_MATCH_S)
            .unwrap();
        let pos = |naif: i32| {
            let k = rec.bodies.iter().position(|&b| b == naif).unwrap();
            rec.positions[k].map(val)
        };
        let mut p = pos(g.naif);
        if center != 0 {
            let c = pos(center);
            p = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        }
        let d = [0, 1, 2].map(|k| p[k] - g.position_km[k]);
        let dr = d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])).sqrt();
        match brute.iter_mut().find(|b| b.0 == g.naif) {
            Some(b) if dr > b.1 => {
                b.1 = dr;
                b.2 = g.t_days;
            }
            Some(_) => {}
            None => brute.push((g.naif, dr, g.t_days)),
        }
    }
    assert_eq!(brute.len(), cmp.bodies.len(), "bodies compared");
    let mut overall: f64 = 0.0;
    for (naif, max, t) in &brute {
        let b = cmp.bodies.iter().find(|b| b.naif == *naif).unwrap();
        let ulps = (b.max_dr_km - max).abs() / (max.abs() * f64::EPSILON).max(f64::MIN_POSITIVE);
        println!(
            "R2 brute force NAIF {naif}: reported {:.9e} km at {} d, brute {max:.9e} km at {t} d \
             ({ulps:.2} ulp)",
            b.max_dr_km, b.t_days_at_max_dr
        );
        assert!(ulps <= MAX_ULPS, "NAIF {naif}: {ulps} ulp");
        assert!(
            b.t_days_at_max_dr.to_bits() == t.to_bits(),
            "NAIF {naif}: max at {} d, brute force at {t} d",
            b.t_days_at_max_dr
        );
        overall = overall.max(*max);
    }
    let rel = (cmp.max_dr_km() - overall).abs() / (overall * f64::EPSILON);
    assert!(rel <= MAX_ULPS, "overall max: {rel} ulp");
}
