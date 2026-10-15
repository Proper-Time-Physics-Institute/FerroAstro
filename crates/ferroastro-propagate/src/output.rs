// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Run outputs: sampled states, perihelion passages and the final state, as exact `(hi, lo)`
//! binary64 pairs (so f64 and double-double runs are recorded without rounding), plus the
//! metadata of the run. Encoded with postcard ([`RunOutput::encode`]).

use crate::PropagateError;
use ferroastro_integrators::{Ias15Stats, Parts};
use ferroastro_real::Real;
use serde::{Deserialize, Serialize};

/// Exact parts of one backend value.
pub(crate) fn parts<R: Real>(z: R) -> Parts {
    let (h, l) = z.to_parts();
    [h, l]
}

/// Exact parts of a 3-vector.
pub(crate) fn parts3<R: Real>(z: [R; 3]) -> [Parts; 3] {
    z.map(parts)
}

/// The barycentric (n-body) or central-body-relative (test-particle) state of every body at
/// one time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateRecord {
    /// TDB seconds since the initial epoch.
    pub t: Parts,
    /// NAIF ids, in storage order.
    pub bodies: Vec<i32>,
    /// Positions (km), ICRF axes.
    pub positions: Vec<[Parts; 3]>,
    /// Velocities (km/s), ICRF axes.
    pub velocities: Vec<[Parts; 3]>,
}

/// One perihelion passage of `body` relative to `central` (root of `r·v`, rising; NUM-010).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PassageRecord {
    /// NAIF id of the body.
    pub body: i32,
    /// NAIF id of the central body.
    pub central: i32,
    /// TDB seconds since the initial epoch.
    pub t: Parts,
    /// Relative position (km), ICRF axes.
    pub r: [Parts; 3],
    /// Relative velocity (km/s), ICRF axes.
    pub v: [Parts; 3],
}

/// Everything a run produced. A run stopped at a checkpoint holds the records up to the stop;
/// its resumption holds the rest, so the two concatenate to the uninterrupted run's records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunOutput {
    /// Scenario name.
    pub scenario: String,
    /// SHA-256 of the scenario's canonical form ([`ferroastro_scenario::ScenarioSpec::digest`]).
    pub scenario_digest: String,
    /// Canonical IC hash ([`ferroastro_scenario::ic_hash`]).
    pub ic_hash: String,
    /// `f64` or `dd`.
    pub precision: String,
    /// Constants version (DE440 table SHA-256).
    pub constants: String,
    /// IC source and its provenance.
    pub ic_source: String,
    /// Initial epoch, TDB Julian date.
    pub epoch_jd_tdb: Parts,
    /// The force model (gauge, terms, parameters).
    pub model: String,
    /// States at the output cadence (k = 0 is the initial state; none after a resume).
    pub states: Vec<StateRecord>,
    /// Perihelion passages.
    pub passages: Vec<PassageRecord>,
    /// The state where this run (or run segment) stopped.
    pub final_state: Option<StateRecord>,
    /// Integrator counters at the stop (cumulative across a resume).
    pub stats: Ias15Stats,
    /// True when the run reached the end of its span (or its passage count).
    pub finished: bool,
}

impl RunOutput {
    /// postcard encoding.
    ///
    /// # Errors
    /// `FA-E1303` if encoding fails.
    pub fn encode(&self) -> Result<Vec<u8>, PropagateError> {
        postcard::to_allocvec(self).map_err(|e| PropagateError::Output {
            problem: format!("cannot encode the run output: {e}"),
        })
    }

    /// Decode [`RunOutput::encode`]'s bytes.
    ///
    /// # Errors
    /// `FA-E1303` for malformed bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, PropagateError> {
        postcard::from_bytes(bytes).map_err(|e| PropagateError::Output {
            problem: format!("cannot decode a run output: {e}"),
        })
    }

    /// Append a resumed segment: its records follow this one's; its final state, counters and
    /// status replace this one's.
    ///
    /// # Errors
    /// `FA-E1303` if `next` belongs to another scenario or initial conditions.
    pub fn extend_with(&mut self, next: Self) -> Result<(), PropagateError> {
        if next.scenario_digest != self.scenario_digest || next.ic_hash != self.ic_hash {
            return Err(PropagateError::Output {
                problem: format!(
                    "cannot join outputs of different runs ({} / {} vs {} / {})",
                    self.scenario, self.ic_hash, next.scenario, next.ic_hash
                ),
            });
        }
        self.states.extend(next.states);
        self.passages.extend(next.passages);
        self.final_state = next.final_state;
        self.stats = next.stats;
        self.finished = next.finished;
        Ok(())
    }

    /// Number of differing 64-bit words between the numerical records of two outputs (states,
    /// passages, final state; counters compared separately), or `None` if their shapes differ.
    #[must_use]
    pub fn differing_words(&self, other: &Self) -> Option<usize> {
        fn words_state(s: &StateRecord) -> Vec<u64> {
            let mut w: Vec<u64> = s.t.iter().map(|z| z.to_bits()).collect();
            for p in s.positions.iter().chain(&s.velocities) {
                w.extend(p.iter().flatten().map(|z| z.to_bits()));
            }
            w
        }
        fn words_passage(p: &PassageRecord) -> Vec<u64> {
            let mut w: Vec<u64> = p.t.iter().map(|z| z.to_bits()).collect();
            w.extend(p.r.iter().chain(&p.v).flatten().map(|z| z.to_bits()));
            w
        }
        if self.states.len() != other.states.len()
            || self.passages.len() != other.passages.len()
            || self.final_state.is_some() != other.final_state.is_some()
        {
            return None;
        }
        let mut a: Vec<u64> = Vec::new();
        let mut b: Vec<u64> = Vec::new();
        for (x, y) in self.states.iter().zip(&other.states) {
            a.extend(words_state(x));
            b.extend(words_state(y));
        }
        for (x, y) in self.passages.iter().zip(&other.passages) {
            a.extend(words_passage(x));
            b.extend(words_passage(y));
        }
        if let (Some(x), Some(y)) = (&self.final_state, &other.final_state) {
            a.extend(words_state(x));
            b.extend(words_state(y));
        }
        if a.len() != b.len() {
            return None;
        }
        Some(a.iter().zip(&b).filter(|(p, q)| p != q).count())
    }
}
