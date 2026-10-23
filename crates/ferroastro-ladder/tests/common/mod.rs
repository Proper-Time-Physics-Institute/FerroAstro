// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared set-up of the single-body rung tests: DE440, the scenario, the test vectors, and
//! checks that print measured value, oracle and tolerance before asserting.

#![allow(
    dead_code,
    reason = "each test binary uses a different subset of the helpers"
)]
#![expect(
    clippy::unwrap_used,
    reason = "test-only helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_constants::De440;
use ferroastro_core::frames::EclipticJ2000;
use ferroastro_ladder::Scenario;
use ferroastro_ladder::single_body::{Rung, RungResult, SingleBody, single_body_rungs};
use ferroastro_vectors::VectorSet;

/// Everything a rung test needs.
pub(crate) struct Ctx {
    pub(crate) de: De440,
    pub(crate) scn: Scenario,
    pub(crate) vs: VectorSet,
    pub(crate) setup: SingleBody<EclipticJ2000>,
}

pub(crate) fn ctx() -> Ctx {
    let de = De440::load().unwrap();
    let scn = Scenario::mercury_single_body().unwrap();
    let vs = VectorSet::load_canonical().unwrap();
    let setup = SingleBody::mercury(&scn, &de).unwrap();
    Ctx { de, scn, vs, setup }
}

impl Ctx {
    /// A rung of the catalogue by id.
    pub(crate) fn rung(&self, id: &str) -> Rung {
        single_body_rungs(&self.scn, &self.de)
            .unwrap()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
    }

    /// Value of a vector.
    pub(crate) fn value(&self, id: &str) -> f64 {
        self.vs.get(id).unwrap().value().unwrap()
    }

    /// Print and assert `measured` against vector `id`.
    pub(crate) fn check(&self, id: &str, measured: f64) {
        let v = self.vs.get(id).unwrap();
        let expected = v.value().unwrap();
        println!(
            "{id}: measured {measured:.10e} {u}; expected {expected} {u}; |diff| {:.3e}; allowed {:.3e}",
            (measured - expected).abs(),
            v.tolerance.allowed(expected),
            u = v.unit
        );
        v.check(measured).unwrap();
    }

    /// NUM-015: pass if the per-orbit relative deviation is within NUM015-REL-TOL **or** the
    /// absolute deviation within NUM015-ABS-FLOOR (red-team floor for tiny effects).
    pub(crate) fn check_num015(&self, r: &RungResult) {
        let rel = self.vs.get("NUM015-REL-TOL").unwrap();
        let abs = self.vs.get("NUM015-ABS-FLOOR").unwrap();
        let (d, dr) = (r.per_orbit_difference(), r.per_orbit_relative_deviation());
        let (rel_ok, abs_ok) = (rel.check(dr).is_ok(), abs.check(d).is_ok());
        println!(
            "NUM-015 [{}]: numerical {:.12e} rad/orbit, analytic {:.12e}; relative {dr:.3e} (allowed {:e}: {}), absolute {d:.3e} rad (floor {:e}: {})",
            r.rung.id,
            r.fit.advance.radians(),
            r.analytic_per_orbit.radians(),
            rel.tolerance.allowed(0.0),
            if rel_ok { "ok" } else { "exceeded" },
            abs.tolerance.allowed(0.0),
            if abs_ok { "ok" } else { "exceeded" },
        );
        assert!(rel_ok || abs_ok, "NUM-015 failed for {}", r.rung.id);
    }
}

/// Print the full result: PHY-015 report plus run diagnostics.
pub(crate) fn show(r: &RungResult) {
    println!("{}", r.report);
    println!(
        "  numerical {:.9} arcsec/cy; analytic {:.9} arcsec/cy; LRL cross-check {:.9}; \
         {} orbits, anomalistic period {:.9} d (Keplerian {:.9} d); {} steps, {} rejected, \
         {} unconverged; max |r.v|/(|r||v|) at passages {:.1e}; wall {:.2} s",
        r.numerical_arcsec_per_century(),
        r.analytic_arcsec_per_century(),
        r.lrl_arcsec_per_century(),
        r.fit.orbits,
        r.fit.period.seconds() / ferroastro_core::time::SECONDS_PER_DAY,
        r.kepler_period.seconds() / ferroastro_core::time::SECONDS_PER_DAY,
        r.stats.steps,
        r.stats.rejected,
        r.stats.unconverged,
        r.max_residual,
        r.wall_seconds
    );
}
