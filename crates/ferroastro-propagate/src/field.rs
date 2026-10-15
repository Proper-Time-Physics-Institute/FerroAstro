// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Ephemeris-field mode (oracle stage B, REF-006; WP17): a massless particle integrated in the
//! central body's frame and pulled by bodies whose positions come from the DE440 SPK.
//!
//! In the central body's (non-rotating, ICRF-axes) frame, with `r` the particle and `r_k(t)` the
//! field bodies relative to the central body (from the SPK at the same TDB time):
//!
//! `a = −μ r/|r|³ + Σ_k GM_k [ (r_k − r)/|r_k − r|³ − r_k/|r_k|³ ]`,
//!
//! the direct and indirect third-body terms (the central body's acceleration by the field
//! bodies, removed because the frame follows it). μ = GM of the central body (the particle is
//! massless). This is the model of Orekit's `NewtonianAttraction` plus `ThirdBodyAttraction`
//! (Montenbruck & Gill 2000, Eq. 3.37), so that the Orekit stage B oracle and FerroAstro run the
//! same physics. States are sampled on an exact grid at the output cadence (f64).

use crate::output::{RunOutput, StateRecord, parts};
use crate::passages::{grid_samples, sample_uniform};
use crate::setup::{DataSources, constants_version, epoch, relative_state, setup};
use crate::{PropagateError, ias15_config};
use ferroastro_constants::De440;
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
use ferroastro_scenario::{BodyName, IcRecord, ScenarioSpec, ic_hash};
use ferroastro_system::Catalogue;

/// The field: SPK, epoch, central body and its μ (km³/s²), field bodies with their GMs.
struct Field<'a> {
    spk: &'a Spk<'a>,
    jd0: f64,
    central: NaifId,
    mu: f64,
    bodies: Vec<(NaifId, f64)>,
}

fn norm3(x: [f64; 3]) -> f64 {
    x[0].hypot(x[1]).hypot(x[2])
}

impl Field<'_> {
    fn relative(&self, id: NaifId, t: f64) -> Result<[f64; 3], IntegratorError> {
        let dynamics = |e: &dyn std::fmt::Display, code| IntegratorError::Dynamics {
            code,
            message: format!("ephemeris field at t = {t} s: {e}"),
        };
        let e: Epoch<Tdb> = Epoch::from_jd_parts(self.jd0, t / SECONDS_PER_DAY)
            .map_err(|e| dynamics(&e, ferroastro_core::ErrorCode::NonFinite))?;
        self.spk
            .relative_state_km(id, self.central, e)
            .map(|(s, _)| s.position)
            .map_err(|err| dynamics(&err, err.code()))
    }
}

impl SecondOrderSystem for Field<'_> {
    fn dimension(&self) -> usize {
        3
    }

    fn acceleration(
        &self,
        t: f64,
        x: &[f64],
        _v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        let r = [
            x.first().copied().unwrap_or(f64::NAN),
            x.get(1).copied().unwrap_or(f64::NAN),
            x.get(2).copied().unwrap_or(f64::NAN),
        ];
        let rn = norm3(r);
        let k = -self.mu / (rn * rn * rn);
        let mut acc = r.map(|c| k * c);
        for &(id, gm) in &self.bodies {
            let rk = self.relative(id, t)?;
            let d = [rk[0] - r[0], rk[1] - r[1], rk[2] - r[2]];
            let dn = norm3(d);
            let kn = norm3(rk);
            let (fd, fi) = (gm / (dn * dn * dn), gm / (kn * kn * kn));
            for c in 0..3 {
                if let (Some(ac), Some(dc), Some(rc)) = (acc.get_mut(c), d.get(c), rk.get(c)) {
                    *ac += fd * dc - fi * rc;
                }
            }
        }
        for (out, v) in a.iter_mut().zip(acc) {
            *out = v;
        }
        Ok(())
    }
}

/// The GM (km³/s²) of a body from the DE440 Major catalogue (header.440 values).
fn gm_of(de: &De440, b: BodyName) -> Result<f64, PropagateError> {
    if b == BodyName::Sun {
        return Ok(de.gm_sun().km3_per_s2());
    }
    Ok(Catalogue::de440_major(de)
        .gm(NaifId(b.naif()))?
        .km3_per_s2())
}

/// The hashed IC records: the central body (μ) at the origin, the particle's relative state,
/// and each field body's GM (its positions come from the SPK and enter through the constants).
fn ic_records(
    central: BodyName,
    particle: BodyName,
    mu: f64,
    field: &[(NaifId, f64)],
    r0: [f64; 3],
    v0: [f64; 3],
) -> Vec<IcRecord> {
    let p64 = |z: [f64; 3]| z.map(|c| [c, 0.0]);
    let zero = [[0.0, 0.0]; 3];
    let mut records = vec![
        IcRecord {
            naif: central.naif(),
            gm: parts(mu),
            position: zero,
            velocity: zero,
        },
        IcRecord {
            naif: particle.naif(),
            gm: [0.0, 0.0],
            position: p64(r0),
            velocity: p64(v0),
        },
    ];
    records.extend(field.iter().map(|&(id, gm)| IcRecord {
        naif: id.0,
        gm: parts(gm),
        position: zero,
        velocity: zero,
    }));
    records
}

/// Run an ephemeris-field scenario (f64, samples at the output cadence).
pub(crate) fn run(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<RunOutput, PropagateError> {
    let n = spec.name.as_str();
    let de = De440::load()?;
    let (spk, prov) = data
        .spk()
        .ok_or_else(|| setup(n, "no SPK", "pass DataSources::with_spk (de440.bsp)"))?;
    let (_, jd) = epoch::<f64>(spec, data)?;
    let (r0, v0, source) = relative_state(spec, data, jd)?;
    let central = spec.central_body;
    let particle = spec
        .bodies
        .iter()
        .copied()
        .find(|&b| b != central)
        .ok_or_else(|| {
            setup(
                n,
                "no particle",
                "list the central body, the particle and the field",
            )
        })?;
    let field_bodies: Vec<BodyName> = spec
        .bodies
        .iter()
        .copied()
        .filter(|&b| b != central && b != particle)
        .collect();
    let mu = gm_of(&de, central)?;
    let mut bodies = Vec::with_capacity(field_bodies.len());
    for &b in &field_bodies {
        bodies.push((NaifId(b.naif()), gm_of(&de, b)?));
    }
    let field = Field {
        spk,
        jd0: jd,
        central: NaifId(central.naif()),
        mu,
        bodies,
    };
    let cadence_days = spec.output.cadence_days.ok_or_else(|| {
        setup(
            n,
            "no output.cadence_days",
            "ephemeris-field runs sample states",
        )
    })?;
    if spec.span.years <= 0.0 || !spec.output.perihelion.is_empty() {
        return Err(setup(
            n,
            "ephemeris-field runs integrate forwards and sample states only",
            "use a positive span and no perihelion output",
        ));
    }
    let cadence_s = cadence_days * SECONDS_PER_DAY;
    let samples = grid_samples(spec.span_seconds(), cadence_s);
    let cfg = ias15_config(spec, false);
    let (t, r, v) = sample_uniform(&field, r0, v0, cadence_s, samples, cfg)?;

    let p64 = |z: [f64; 3]| z.map(|c| [c, 0.0]);
    let zero = [[0.0, 0.0]; 3];
    let constants = format!("{}; field from {prov}", constants_version(&de));
    let records = ic_records(central, particle, mu, &field.bodies, r0, v0);
    let (c_id, p_id) = (central.naif(), particle.naif());
    let states = t
        .iter()
        .zip(r.iter().zip(&v))
        .map(|(tk, (rk, vk))| StateRecord {
            t: [*tk, 0.0],
            bodies: vec![c_id, p_id],
            positions: vec![zero, p64(*rk)],
            velocities: vec![zero, p64(*vk)],
        })
        .collect();
    let names: Vec<&str> = field_bodies.iter().map(|b| b.label()).collect();
    Ok(RunOutput {
        scenario: spec.name.clone(),
        scenario_digest: spec.digest(),
        ic_hash: ic_hash(&constants, spec.mode.label(), parts(jd), &records),
        precision: spec.integrator.precision.label().into(),
        constants,
        ic_source: source,
        epoch_jd_tdb: parts(jd),
        model: format!(
            "massless {} about {} (mu = GM_{}) with direct + indirect third-body terms of {} from \
             the SPK; IAS15 epsilon = {:e}, safety = {}, PC tolerance = {:e}, f64, samples every \
             {cadence_days} d on an exact grid",
            particle.label(),
            central.label(),
            central.label(),
            names.join(", "),
            cfg.epsilon,
            cfg.safety,
            cfg.pc_tolerance,
        ),
        states,
        passages: Vec::new(),
        final_state: None,
        stats: ferroastro_integrators::Ias15Stats::default(),
        finished: true,
    })
}
