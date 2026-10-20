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

/// The central body, the particle, the field bodies (every scenario body except those two, in
/// scenario order), μ of the central body and each field body's NAIF id and GM (km³/s²).
type FieldBodies = (BodyName, BodyName, Vec<BodyName>, f64, Vec<(NaifId, f64)>);

fn field_bodies(spec: &ScenarioSpec, de: &De440) -> Result<FieldBodies, PropagateError> {
    let central = spec.central_body;
    let particle = spec
        .bodies
        .iter()
        .copied()
        .find(|&b| b != central)
        .ok_or_else(|| {
            setup(
                &spec.name,
                "no particle",
                "list the central body, the particle and the field",
            )
        })?;
    let names: Vec<BodyName> = spec
        .bodies
        .iter()
        .copied()
        .filter(|&b| b != central && b != particle)
        .collect();
    let mu = gm_of(de, central)?;
    let mut bodies = Vec::with_capacity(names.len());
    for &b in &names {
        bodies.push((NaifId(b.naif()), gm_of(de, b)?));
    }
    Ok((central, particle, names, mu, bodies))
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
    let (central, particle, field_bodies, mu, bodies) = field_bodies(spec, &de)?;
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

#[cfg(test)]
mod tests {
    //! Progress review after WP18, R4: the field bodies and GMs of the stage B model are pinned
    //! (PR tier: the pinned header.440 and de440.bsp from the JPL cache).

    use super::{Field, field_bodies};
    use ferroastro_constants::De440;
    use ferroastro_constants::header440::Header;
    use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
    use ferroastro_ephem::{Ephemeris, NaifId, Spk};
    use ferroastro_integrators::SecondOrderSystem;
    use ferroastro_scenario::ScenarioSpec;

    /// Relative tolerance of the field acceleration against the independent sum below: the same
    /// formula in another operation order (round-off ~1e-15 relative). Omitting any field body
    /// changes the acceleration at the probe points by >= 1 %.
    const ACC_REL_TOL: f64 = 1e-12; // lint: allow-literal(round-off bound of two summation orders)
    /// Probe points sit this fraction of a field body's distance from it, so that its term is a
    /// percent-level share of the acceleration.
    const PROBE_OFFSET: f64 = 1e-3; // lint: allow-literal(probe geometry, not physics)

    fn spec() -> ScenarioSpec {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ci/scenarios/oracle-stage-b-mercury-de440-field-100yr.toml");
        ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    /// The header.440 symbol(s) of a body's GM, converted to km³/s² with the header's AU and
    /// 86400 s/d; Earth and Moon split `GMB` with `EMRAT` (DE440 convention).
    fn header_gm(h: &Header, naif: i32) -> f64 {
        let au = h.value("AU").unwrap();
        let emrat = h.value("EMRAT").unwrap();
        let au3d2 = match naif {
            10 => h.value("GMS").unwrap(),
            399 => h.value("GMB").unwrap() * (emrat / (1.0 + emrat)),
            301 => h.value("GMB").unwrap() * (1.0 / (1.0 + emrat)),
            n => h.value(&format!("GM{n}")).unwrap(),
        };
        au3d2 * (au * au * au / (SECONDS_PER_DAY * SECONDS_PER_DAY))
    }

    /// R4: the field bodies are the scenario's bodies minus the central body and the particle,
    /// in scenario order; μ and every field GM equal header.440 bitwise; and the acceleration at
    /// a probe point next to each field body equals an independent direct + indirect sum over
    /// exactly those bodies (kills an omitted body in the acceleration loop, e.g. Pluto).
    ///
    /// Verifies: REF-006, CI-002
    #[test]
    fn field_bodies_and_gms_equal_the_scenario_and_header440() {
        let s = spec();
        let de = De440::load().unwrap();
        let hf = ferroastro_testdata::locate_product("de440-header").unwrap();
        println!("de440-header: {}", hf.provenance());
        let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
        let (central, particle, names, mu, bodies) = field_bodies(&s, &de).unwrap();
        let expected: Vec<i32> = s
            .bodies
            .iter()
            .filter(|&&b| b != s.central_body && b != particle)
            .map(|b| b.naif())
            .collect();
        assert_eq!(central, s.central_body);
        assert_eq!(
            particle.naif(),
            s.bodies[1].naif(),
            "the particle is listed second"
        );
        assert_eq!(bodies.iter().map(|b| b.0.0).collect::<Vec<_>>(), expected);
        assert_eq!(names.len(), expected.len());
        assert_eq!(mu.to_bits(), header_gm(&h, central.naif()).to_bits(), "mu");
        for &(id, gm) in &bodies {
            let want = header_gm(&h, id.0);
            println!(
                "field NAIF {:>3}: GM {gm:e} km3/s2, header.440 {want:e}",
                id.0
            );
            assert_eq!(gm.to_bits(), want.to_bits(), "GM of NAIF {}", id.0);
        }

        let bsp = ferroastro_testdata::locate_product("de440-bsp").unwrap();
        println!("de440-bsp: {}", bsp.provenance());
        let bytes = std::fs::read(&bsp.path).unwrap();
        let spk = Spk::parse(&bytes).unwrap();
        let jd0 = s.initial_conditions.epoch_jd_tdb.unwrap();
        let field = Field {
            spk: &spk,
            jd0,
            central: NaifId(central.naif()),
            mu,
            bodies: bodies.clone(),
        };
        let e: Epoch<Tdb> = Epoch::from_jd_parts(jd0, 0.0).unwrap();
        let rel = |id: i32| {
            spk.relative_state_km::<f64>(NaifId(id), NaifId(central.naif()), e)
                .unwrap()
                .0
                .position
        };
        let n3 = |x: [f64; 3]| x[0].mul_add(x[0], x[1].mul_add(x[1], x[2] * x[2])).sqrt();
        for &probe_id in &expected {
            let rk = rel(probe_id);
            let x = rk.map(|c| c * (1.0 + PROBE_OFFSET));
            let mut a = [0.0; 3];
            field.acceleration(0.0, &x, &[0.0; 3], &mut a).unwrap();
            // Independent sum over the expected list, GMs from the header.
            let r = n3(x);
            let mut want = x.map(|c| -header_gm(&h, central.naif()) * c / (r * r * r));
            for &j in &expected {
                let rj = rel(j);
                let d = [rj[0] - x[0], rj[1] - x[1], rj[2] - x[2]];
                let (dn, jn, g) = (n3(d), n3(rj), header_gm(&h, j));
                for c in 0..3 {
                    want[c] += g * d[c] / (dn * dn * dn) - g * rj[c] / (jn * jn * jn);
                }
            }
            let err = n3([a[0] - want[0], a[1] - want[1], a[2] - want[2]]) / n3(want);
            println!("probe next to NAIF {probe_id:>3}: |da|/|a| = {err:.2e}");
            assert!(err <= ACC_REL_TOL, "probe next to NAIF {probe_id}: {err}");
        }
    }
}
