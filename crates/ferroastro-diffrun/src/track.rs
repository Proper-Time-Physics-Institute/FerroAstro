// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The tracked body's heliocentric (central-body-relative) samples and perihelion passages, kept
//! from the run outputs segment by segment so that a 2000-year run never holds every body's
//! states at once.

use ferroastro_propagate::{PassageRecord, RunOutput, StateRecord};
use ferroastro_scenario::Parts;
use serde::{Deserialize, Serialize};

fn sum(p: Parts) -> f64 {
    p[0] + p[1]
}

fn diff3(a: &[Parts; 3], b: &[Parts; 3]) -> [f64; 3] {
    [0, 1, 2].map(|k| match (a.get(k), b.get(k)) {
        (Some(x), Some(y)) => (x[0] - y[0]) + (x[1] - y[1]),
        _ => f64::NAN,
    })
}

/// One perihelion passage located on dense output (NUM-010), relative to the central body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackPassage {
    /// TDB seconds since the run's initial epoch.
    pub t: f64,
    /// Relative position (km), ICRF axes.
    pub r: [f64; 3],
    /// Relative velocity (km/s), ICRF axes.
    pub v: [f64; 3],
}

/// Relative states of the tracked body at the output cadence, and its perihelion passages, of
/// one configuration (all legs merged in time order).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Initial epoch of the runs, TDB Julian date.
    pub epoch_jd_tdb: f64,
    /// Sample times, TDB seconds since the initial epoch.
    pub t: Vec<f64>,
    /// Relative positions (km), ICRF axes.
    pub r: Vec<[f64; 3]>,
    /// Relative velocities (km/s), ICRF axes.
    pub v: Vec<[f64; 3]>,
    /// Perihelion passages located on dense output.
    pub passages: Vec<TrackPassage>,
}

impl Track {
    /// An empty track for runs starting at `epoch_jd_tdb`.
    #[must_use]
    pub fn new(epoch_jd_tdb: f64) -> Self {
        Self {
            epoch_jd_tdb,
            ..Self::default()
        }
    }

    fn push_state(&mut self, s: &StateRecord, body: i32, central: i32) -> bool {
        let find = |id: i32| s.bodies.iter().position(|&b| b == id);
        let (Some(ib), Some(ic)) = (find(body), find(central)) else {
            return false;
        };
        let (Some(pb), Some(pc), Some(vb), Some(vc)) = (
            s.positions.get(ib),
            s.positions.get(ic),
            s.velocities.get(ib),
            s.velocities.get(ic),
        ) else {
            return false;
        };
        self.t.push(sum(s.t));
        self.r.push(diff3(pb, pc));
        self.v.push(diff3(vb, vc));
        true
    }

    fn push_passage(&mut self, p: &PassageRecord) {
        self.passages.push(TrackPassage {
            t: sum(p.t),
            r: p.r.map(sum),
            v: p.v.map(sum),
        });
    }

    /// Append the samples and passages of `body` relative to `central` from one run segment.
    /// Returns the number of states that did not contain both bodies (0 for a well-formed run).
    pub fn absorb(&mut self, out: &RunOutput, body: i32, central: i32) -> usize {
        let missing = out
            .states
            .iter()
            .filter(|s| !self.push_state(s, body, central))
            .count();
        for p in out
            .passages
            .iter()
            .filter(|p| p.body == body && p.central == central)
        {
            self.push_passage(p);
        }
        missing
    }

    /// Merge tracks of legs that start at the same epoch (e.g. one backwards and one forwards in
    /// time): samples and passages in increasing time, with an exact duplicate (the shared
    /// initial sample) kept once.
    #[must_use]
    pub fn merge(legs: Vec<Track>) -> Self {
        let epoch = legs.first().map_or(0.0, |l| l.epoch_jd_tdb);
        let mut samples: Vec<(f64, [f64; 3], [f64; 3])> = Vec::new();
        let mut passages: Vec<TrackPassage> = Vec::new();
        for l in legs {
            samples.extend(
                l.t.iter()
                    .zip(&l.r)
                    .zip(&l.v)
                    .map(|((t, r), v)| (*t, *r, *v)),
            );
            passages.extend(l.passages);
        }
        samples.sort_by(|a, b| a.0.total_cmp(&b.0));
        samples.dedup_by(|a, b| a.0.to_bits() == b.0.to_bits());
        passages.sort_by(|a, b| a.t.total_cmp(&b.t));
        passages.dedup_by(|a, b| a.t.to_bits() == b.t.to_bits());
        Self {
            epoch_jd_tdb: epoch,
            t: samples.iter().map(|s| s.0).collect(),
            r: samples.iter().map(|s| s.1).collect(),
            v: samples.iter().map(|s| s.2).collect(),
            passages,
        }
    }

    /// Number of differing 64-bit words between two tracks, or `None` if their shapes differ.
    #[must_use]
    pub fn differing_words(&self, other: &Self) -> Option<usize> {
        if self.t.len() != other.t.len() || self.passages.len() != other.passages.len() {
            return None;
        }
        let words = |t: &Self| -> Vec<u64> {
            let mut w = vec![t.epoch_jd_tdb.to_bits()];
            w.extend(t.t.iter().map(|x| x.to_bits()));
            w.extend(t.r.iter().chain(&t.v).flatten().map(|x| x.to_bits()));
            for p in &t.passages {
                w.push(p.t.to_bits());
                w.extend(p.r.iter().chain(&p.v).map(|x| x.to_bits()));
            }
            w
        };
        let (a, b) = (words(self), words(other));
        (a.len() == b.len()).then(|| a.iter().zip(&b).filter(|(x, y)| x != y).count())
    }
}
