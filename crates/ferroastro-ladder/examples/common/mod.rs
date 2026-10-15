// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared set-up of the per-rung examples (API-003): DE440 from the committed table, the cited
//! Mercury scenario, the test vectors, and a check that prints the vector it compares with.

#![allow(
    dead_code,
    reason = "each example uses a different subset of the helpers"
)]

use ferroastro_constants::De440;
use ferroastro_core::frames::EclipticJ2000;
use ferroastro_ladder::Scenario;
use ferroastro_ladder::single_body::{Rung, RungResult, SingleBody, single_body_rungs};
use ferroastro_vectors::VectorSet;

/// The error type of the examples.
pub(crate) type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// Anomalistic orbits per example run (the tests use 1000; 100 keeps the examples quick).
pub(crate) const ORBITS: u32 = 100;

/// Everything an example needs.
pub(crate) struct Ctx {
    pub(crate) de: De440,
    pub(crate) scn: Scenario,
    pub(crate) vs: VectorSet,
    pub(crate) setup: SingleBody<EclipticJ2000>,
}

/// Load DE440, the scenario and the vectors; a Mercury test particle run for [`ORBITS`] orbits.
pub(crate) fn ctx() -> Res<Ctx> {
    let de = De440::load()?;
    let scn = Scenario::mercury_single_body()?;
    let vs = VectorSet::load_canonical()?;
    let mut setup = SingleBody::mercury(&scn, &de)?;
    setup.orbits = ORBITS;
    Ok(Ctx { de, scn, vs, setup })
}

impl Ctx {
    /// A rung of the `single-body` catalogue by id.
    pub(crate) fn rung(&self, id: &str) -> Res<Rung> {
        single_body_rungs(&self.scn, &self.de)?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("rung {id} is not in the catalogue").into())
    }

    /// Run a catalogue rung and print its PHY-015 report.
    pub(crate) fn run(&self, id: &str) -> Res<RungResult> {
        let r = self.setup.run(&self.rung(id)?)?;
        println!("{}", r.report);
        println!(
            "  numerical {:.7} arcsec/cy, analytic {:.7} arcsec/cy ({} orbits, IC hash {})",
            r.numerical_arcsec_per_century(),
            r.analytic_arcsec_per_century(),
            r.fit.orbits,
            r.ic_hash
        );
        Ok(r)
    }

    /// Compare `measured` with test vector `id`, print both, and fail on a deviation.
    pub(crate) fn check(&self, id: &str, measured: f64) -> Res<()> {
        let v = self.vs.get(id)?;
        let expected = v.value()?;
        println!(
            "  {id}: measured {measured:.9e} {u}, expected {expected} {u} +/- {:e} ({})",
            v.tolerance.allowed(expected),
            v.citation,
            u = v.unit
        );
        v.check(measured)?;
        Ok(())
    }
}
