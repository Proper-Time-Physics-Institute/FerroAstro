// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Compare a FerroAstro run with a state golden (stage A, CI-005).

use crate::{GoldenStates, OracleError};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_propagate::{RunOutput, StateRecord};

/// Output times of the run and the golden are matched when they differ by at most this many
/// seconds: both are `k · cadence`, computed in different units (days vs seconds), so they agree
/// to round-off (~1e-7 s at 100 yr), far below any orbital time scale.
pub const TIME_MATCH_S: f64 = 1e-6;

/// The largest differences of one body over the compared times.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyDifference {
    /// NAIF id.
    pub naif: i32,
    /// Largest |Δr| (km), Euclidean.
    pub max_dr_km: f64,
    /// Time of the largest |Δr| (TDB days since the epoch).
    pub t_days_at_max_dr: f64,
    /// Largest |Δv| (km/s), Euclidean.
    pub max_dv_km_s: f64,
}

/// The result of [`compare_states`].
#[derive(Debug, Clone, PartialEq)]
pub struct StateComparison {
    /// Golden rows compared (every row of the golden).
    pub rows: usize,
    /// Distinct output times compared.
    pub times: usize,
    /// Per body, in golden order.
    pub bodies: Vec<BodyDifference>,
}

impl StateComparison {
    /// The body with the largest |Δr|.
    #[must_use]
    pub fn worst(&self) -> Option<&BodyDifference> {
        self.bodies
            .iter()
            .max_by(|a, b| a.max_dr_km.total_cmp(&b.max_dr_km))
    }

    /// Largest |Δr| over every body and time (km).
    #[must_use]
    pub fn max_dr_km(&self) -> f64 {
        self.worst().map_or(0.0, |b| b.max_dr_km)
    }
}

fn val(p: [f64; 2]) -> f64 {
    p[0] + p[1]
}

fn norm(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Compare every row of `golden` with the FerroAstro state at the same time and body. The run's
/// samples and its final state are both candidates; the epochs must be bitwise equal.
///
/// The golden's origin is its `center_naif` header line (absent: 0, the solar-system
/// barycentre). For a heliocentric golden (`center_naif: 10`, the tools that integrate relative
/// to the Sun) the FerroAstro states are taken relative to the same body before differencing,
/// which also removes the uniform drift of a body subset's barycentre.
///
/// # Errors
/// `FA-E1503` if the epochs differ or a golden time or body (or the centre) has no FerroAstro
/// counterpart, `FA-E1502` if the golden lacks its epoch or has an invalid centre.
pub fn compare_states(
    golden: &GoldenStates,
    run: &RunOutput,
) -> Result<StateComparison, OracleError> {
    let what = || format!("{} vs FerroAstro run {}", golden.file, run.scenario);
    let epoch = golden.meta_f64("epoch_jd_tdb")?;
    let run_epoch = val(run.epoch_jd_tdb);
    if epoch.to_bits() != run_epoch.to_bits() {
        return Err(OracleError::Comparison {
            what: what(),
            problem: format!("epochs differ: golden JD {epoch}, run JD {run_epoch}"),
        });
    }
    let center = golden.center_naif()?;
    let records: Vec<&StateRecord> = run.states.iter().chain(run.final_state.as_ref()).collect();
    let mut bodies: Vec<BodyDifference> = Vec::new();
    for g in &golden.rows {
        let t_s = g.t_days * SECONDS_PER_DAY;
        let rec = records
            .iter()
            .find(|r| (val(r.t) - t_s).abs() <= TIME_MATCH_S)
            .ok_or_else(|| OracleError::Comparison {
                what: what(),
                problem: format!("no FerroAstro state at t = {} d", g.t_days),
            })?;
        let state_of = |naif: i32| {
            rec.bodies
                .iter()
                .zip(rec.positions.iter().zip(&rec.velocities))
                .find_map(|(b, (p, v))| (*b == naif).then(|| (p.map(val), v.map(val))))
                .ok_or_else(|| OracleError::Comparison {
                    what: what(),
                    problem: format!(
                        "NAIF {naif} not in the FerroAstro state at t = {} d",
                        g.t_days
                    ),
                })
        };
        let (mut p, mut v) = state_of(g.naif)?;
        if center != 0 {
            let (pc, vc) = state_of(center)?;
            p = sub(p, pc);
            v = sub(v, vc);
        }
        let dr = norm(p, g.position_km);
        let dv = norm(v, g.velocity_km_s);
        match bodies.iter_mut().find(|b| b.naif == g.naif) {
            Some(b) => {
                if dr > b.max_dr_km {
                    b.max_dr_km = dr;
                    b.t_days_at_max_dr = g.t_days;
                }
                b.max_dv_km_s = b.max_dv_km_s.max(dv);
            }
            None => bodies.push(BodyDifference {
                naif: g.naif,
                max_dr_km: dr,
                t_days_at_max_dr: g.t_days,
                max_dv_km_s: dv,
            }),
        }
    }
    Ok(StateComparison {
        rows: golden.rows.len(),
        times: golden.times().len(),
        bodies,
    })
}
