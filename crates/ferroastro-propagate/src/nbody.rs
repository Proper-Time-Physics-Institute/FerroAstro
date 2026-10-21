// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! N-body mode: the `ferroastro-forces` N-body model advanced by IAS15, with states sampled at
//! the output cadence from dense output, perihelion passages located on dense output (NUM-010),
//! and checkpoint/resume (CI-010).
//!
//! The driver never shortens a step except the last one, which lands on the end of the span:
//! samples and events come from the step polynomials, and a run stopped for a checkpoint stops
//! after the first accepted step that reaches the requested time. The interrupted and the
//! uninterrupted runs therefore take the same steps, and a resumed run continues bit for bit.

use crate::output::{PassageRecord, RunOutput, StateRecord, parts, parts3};
use crate::setup::{
    DataSources, constants_version, epoch, nbody_ics, setup, solar_j2, solar_pole, solar_spin,
    tag_overrides,
};
use crate::stepper::{IntegratorSnapshot, Stepper};
use crate::{PropagateError, abm_config, ias15_config};
use ferroastro_abm::Abm;
use ferroastro_constants::{Body, De440};
use ferroastro_core::frames::Barycentric;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::{Length, Velocity};
use ferroastro_eih::{
    BarycentreBody, Eih, EihSoa, HeliocentricSchwarzschild, RhsAcceleration,
    relativistic_barycentre_with_rate,
};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, GrPotential, LtReaction, MOON_TAYLOR_TERMS_DEFAULT, MinorFromMajors,
    MinorReaction, MixedForces, MoonVariables, NBody, NBodyModel, NewtonianMutual,
    NewtonianMutualSoa, RingMasses, SoaBackend, SolarJ2WithReaction, SolarLenseThirring,
};
use ferroastro_integrators::Ias15;
use ferroastro_integrators::Irk16;
use ferroastro_real::Real;
use ferroastro_scenario::{
    BodyName, ForceKernel, IcBarycentre, IcRecord, IntegratorName, MajorAsteroids, Precision,
    ScenarioError, ScenarioSpec, ic_hash,
};
use ferroastro_symplectic::Whfast;
use ferroastro_system::{
    ASTEROID_NAIF_BASE, Catalogue, KBO_IDENTIFICATION, KBO_RING, SystemState, Tier,
    kbo_ring_states, place_small_bodies,
};
use serde::{Deserialize, Serialize};

/// Driver state carried across a checkpoint besides the integrator's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DriverState {
    /// Index `k` of the next output sample (time `k · cadence`).
    pub(crate) next_sample: u64,
    /// Perihelion passages found so far.
    pub(crate) passages: u64,
}

/// True if the scenario selects the SoA pair kernels (`integrator.force_kernel = "soa"`, WP26).
/// The lane backend is [`SoaBackend::preferred`]: SIMD with the `simd` feature, scalar without,
/// bitwise identical either way.
fn soa_kernel(spec: &ScenarioSpec) -> bool {
    spec.integrator.force_kernel == Some(ForceKernel::Soa)
}

/// The relativistic term of an N-body scenario (WP19): `gr_eih` gives the EIH term (Park et al.
/// 2021 Eq. 27, Newtonian right-hand-side accelerations), `gr_1pn_central` the heliocentric
/// Sun-only 1PN rung; PPN β, γ from `forces.ppn`, else the DE440 values; c from DE440.
fn relativity<R: Real>(
    spec: &ScenarioSpec,
    de: &De440,
    has_sun: bool,
) -> Result<Vec<Box<dyn ForceTerm<Icrf, R>>>, PropagateError> {
    let f = &spec.forces;
    let value_err = |field: &'static str, problem: &str| -> PropagateError {
        ScenarioError::Value {
            name: spec.name.clone(),
            field,
            value: "true".into(),
            problem: problem.into(),
        }
        .into()
    };
    if f.gr_eih && f.gr_1pn_central {
        return Err(value_err(
            "forces.gr_1pn_central",
            "gr_eih already contains the Sun's 1PN field; gr_1pn_central (the Sun-only rung) is \
             an alternative to it",
        ));
    }
    let (beta, gamma) = f
        .ppn
        .map_or((de.ppn_beta(), de.ppn_gamma()), |p| (p.beta, p.gamma));
    let c = Velocity::from_km_per_s(R::from_f64(de.speed_of_light().km_per_s()))
        .map_err(|e| setup(&spec.name, e.to_string(), "check the DE440 table"))?;
    let (beta, gamma) = (R::from_f64(beta), R::from_f64(gamma));
    let mut terms: Vec<Box<dyn ForceTerm<Icrf, R>>> = Vec::new();
    if f.gr_eih {
        let eih = Eih::new(c, beta, gamma, RhsAcceleration::Newtonian)?;
        if soa_kernel(spec) {
            terms.push(Box::new(EihSoa::new(eih, SoaBackend::preferred())));
        } else {
            terms.push(Box::new(eih));
        }
    }
    if f.gr_1pn_central {
        if !has_sun {
            return Err(value_err(
                "forces.gr_1pn_central",
                "the Sun-only 1PN rung needs the Sun among the bodies",
            ));
        }
        terms.push(Box::new(HeliocentricSchwarzschild::new(c, beta, gamma)?));
    }
    if f.gr_potential {
        if !has_sun {
            return Err(value_err(
                "forces.gr_potential",
                "the GR effective potential of the Sun needs the Sun among the bodies",
            ));
        }
        terms.push(Box::new(GrPotential::new(c)?));
    }
    Ok(terms)
}

/// The model of an N-body scenario (term order: Newtonian mutual, Minor, Ring, geocentric Moon,
/// the relativistic term (WP19), solar J2 with reaction, solar Lense–Thirring with reaction
/// (WP20)) and the body order. The solar J2, pole and spin are DE440's unless `data` carries
/// [`crate::SolarOverrides`].
fn model<R: Real>(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    de: &De440,
    small: &[(NaifId, f64, Tier)],
) -> Result<(NBodyModel<Icrf, R>, Vec<IcRecordGm>), PropagateError> {
    let cat = Catalogue::de440_major(de);
    let mut gms = Vec::with_capacity(spec.bodies.len());
    let mut bodies = Vec::with_capacity(spec.bodies.len());
    for b in &spec.bodies {
        let id = NaifId(b.naif());
        let gm = if *b == BodyName::Emb {
            de.gm(Body::EarthMoon).km3_per_s2()
        } else {
            cat.gm(id)?.km3_per_s2()
        };
        gms.push((b.naif(), gm));
        bodies.push(NBody {
            id,
            gm: R::from_f64(gm),
            tier: Tier::Major,
        });
    }
    for &(id, gm, tier) in small {
        gms.push((id.0, gm));
        bodies.push(NBody {
            id,
            gm: R::from_f64(gm),
            tier,
        });
    }
    let has = |b: BodyName| spec.bodies.contains(&b);
    let mut terms: Vec<Box<dyn ForceTerm<Icrf, R>>> = if soa_kernel(spec) {
        vec![Box::new(NewtonianMutualSoa::new(SoaBackend::preferred()))]
    } else {
        vec![Box::new(NewtonianMutual)]
    };
    if small.iter().any(|b| b.2 == Tier::Minor) {
        terms.push(Box::new(MinorFromMajors::new(MinorReaction::On)));
    }
    if small.iter().any(|b| b.2 == Tier::Ring) {
        terms.push(Box::new(RingMasses));
    }
    let vars = if has(BodyName::Earth) && has(BodyName::Moon) {
        terms.push(Box::new(GeocentricMoon::<R>::new(
            MOON_TAYLOR_TERMS_DEFAULT,
        )?));
        MoonVariables::Geocentric {
            emrat: R::from_f64(de.earth_moon_mass_ratio()),
        }
    } else {
        MoonVariables::Barycentric
    };
    terms.extend(relativity::<R>(spec, de, has(BodyName::Sun))?);
    if spec.forces.sun_j2 {
        if !has(BodyName::Sun) {
            return Err(ScenarioError::Value {
                name: spec.name.clone(),
                field: "forces.sun_j2",
                value: "true".into(),
                problem: "solar J2 needs the Sun among the bodies".into(),
            }
            .into());
        }
        terms.push(Box::new(SolarJ2WithReaction::new(
            R::from_f64(solar_j2(de, data)),
            Length::from_km(R::from_f64(de.sun_radius().km()))
                .map_err(|e| setup(&spec.name, e.to_string(), "check the DE440 table"))?,
            solar_pole::<R>(data)?,
        )?));
    }
    if spec.forces.lense_thirring {
        if !has(BodyName::Sun) {
            return Err(ScenarioError::Value {
                name: spec.name.clone(),
                field: "forces.lense_thirring",
                value: "true".into(),
                problem: "solar Lense-Thirring needs the Sun among the bodies".into(),
            }
            .into());
        }
        let gamma = spec.forces.ppn.map_or(de.ppn_gamma(), |p| p.gamma);
        let c = Velocity::from_km_per_s(R::from_f64(de.speed_of_light().km_per_s()))
            .map_err(|e| setup(&spec.name, e.to_string(), "check the DE440 table"))?;
        terms.push(Box::new(SolarLenseThirring::new(
            solar_spin(de, data, &spec.name)?.cast(),
            c,
            R::from_f64(gamma),
            solar_pole::<R>(data)?,
            LtReaction::On,
        )?));
    }
    Ok((NBodyModel::new(&bodies, vars, terms)?, gms))
}

type IcRecordGm = (i32, f64);

/// The small bodies of a scenario (`asteroids`, `kbos`, `kbo_ring`): `(id, GM km³/s², tier)` in
/// storage order after the scenario's bodies, their barycentric states and a provenance line.
type SmallBodies<R> = (
    Vec<(NaifId, f64, Tier)>,
    SystemState<Icrf, Barycentric, R>,
    String,
);

/// Small bodies from header.440 (GMs) and the REF-003 sampled file (Sun-centred states at JDEPOC),
/// placed with the scenario's own Sun state (WP15). The major-body states are not touched.
fn small_bodies<R: Real>(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    majors: &SystemState<Icrf, Barycentric, R>,
    jd: f64,
) -> Result<SmallBodies<R>, PropagateError> {
    let n = spec.name.as_str();
    let f = &spec.forces;
    let mut list = Vec::new();
    let mut states = SystemState::new();
    if f.asteroids == 0 && !f.kbos && !f.kbo_ring {
        return Ok((list, states, String::new()));
    }
    let (h, hprov) = data.header().ok_or_else(|| {
        setup(
            n,
            "small bodies need header.440 (GMs)",
            "pass DataSources::with_header",
        )
    })?;
    let mut cat = Catalogue::from_header(h, hprov)?;
    let mut prov = Vec::new();
    let mut minor: Vec<NaifId> = Vec::new();
    if f.asteroids > 0 {
        let all: Vec<NaifId> = cat.ids_in(Tier::Minor).collect();
        let count = usize::try_from(f.asteroids).unwrap_or(usize::MAX);
        if count > all.len() {
            return Err(ScenarioError::Value {
                name: n.to_owned(),
                field: "forces.asteroids",
                value: f.asteroids.to_string(),
                problem: format!("header.440 has {} numbered asteroids", all.len()),
            }
            .into());
        }
        minor.extend(all.iter().take(count).copied());
    }
    if f.kbos {
        cat.with_identified_kbos()?;
        minor.extend(KBO_IDENTIFICATION.iter().map(|k| k.id));
    }
    if !minor.is_empty() {
        let file = data.small_bodies().ok_or_else(|| {
            setup(
                n,
                "asteroids/kbos need the REF-003 sampled states",
                "pass DataSources::with_small_bodies",
            )
        })?;
        let file_jd = file.jdepoc()?;
        if file_jd.to_bits() != jd.to_bits() {
            return Err(setup(
                n,
                format!("small-body states are at JD {file_jd}, the scenario epoch is JD {jd}"),
                "use de440-header initial conditions (JDEPOC)",
            ));
        }
        let sun = majors.state(NaifId::SUN).map_err(|_| {
            setup(
                n,
                "small bodies are placed relative to the Sun",
                "list the Sun",
            )
        })?;
        let placed = place_small_bodies(file, &minor, &sun)?;
        let major = major_asteroid_filter(spec, &minor)?;
        let mut promoted = 0_usize;
        for (id, st) in placed.states()? {
            states.push(id, &st)?;
            let tier = if major(id) {
                promoted += 1;
                Tier::Major
            } else {
                Tier::Minor
            };
            list.push((id, cat.gm(id)?.km3_per_s2(), tier));
        }
        prov.push(format!(
            "{} small bodies ({} Major: forces.major_asteroids = {}, {} Minor) placed from {} + \
             the header Sun",
            minor.len(),
            promoted,
            f.major_asteroids.label(),
            minor.len() - promoted,
            file.provenance()
        ));
    }
    if f.kbo_ring {
        cat.with_kbo_ring()?;
        let ring = kbo_ring_states::<R>(&KBO_RING, h.value("AU")?)?;
        for (id, st) in ring.states()? {
            states.push(id, &st)?;
            list.push((id, cat.gm(id)?.km3_per_s2(), Tier::Ring));
        }
        prov.push(format!(
            "KBO ring: {} fixed masses at {} au in ECLIPJ2000 (Park et al. 2021 section 2.2; phase \
             and fixed points FerroAstro's, OI-24)",
            KBO_RING.points, KBO_RING.radius_au
        ));
    }
    Ok((list, states, prov.join("; ")))
}

/// Which placed asteroids are Major under `forces.major_asteroids` (WP25), after checking that
/// every EPM2021 asteroid is among `minor`.
fn major_asteroid_filter(
    spec: &ScenarioSpec,
    minor: &[NaifId],
) -> Result<impl Fn(NaifId) -> bool, PropagateError> {
    let f = &spec.forces;
    let naif = |k: u32| i64::from(ASTEROID_NAIF_BASE) + i64::from(k);
    if f.major_asteroids == MajorAsteroids::Epm2021 {
        for k in MajorAsteroids::EPM2021 {
            if !minor.iter().any(|id| i64::from(id.0) == naif(k)) {
                return Err(ScenarioError::Value {
                    name: spec.name.clone(),
                    field: "forces.major_asteroids",
                    value: "epm2021".into(),
                    problem: format!(
                        "asteroid ({k}) is not among the first {} numbered asteroids of header.440",
                        f.asteroids
                    ),
                }
                .into());
            }
        }
    }
    let which = f.major_asteroids;
    Ok(move |id: NaifId| match which {
        MajorAsteroids::None => false,
        MajorAsteroids::All => true,
        MajorAsteroids::Epm2021 => MajorAsteroids::EPM2021
            .iter()
            .any(|&k| i64::from(id.0) == naif(k)),
    })
}

/// Fixed-point passes of [`rebarycentre`] (μ* depends on the velocities; translation leaves
/// every rᵢⱼ unchanged).
const REBARYCENTRE_PASSES: usize = 3;

/// Translate the packed initial conditions to the relativistic SSB (NUM-018; Subbotin et al.
/// 2023 Eq. 5), in `R`. Every slot moves by the same position and velocity except the geocentric
/// Moon slot (Moon − Earth). Returns the offset left after the last pass (km, km/s).
fn rebarycentre<R: Real>(
    model: &NBodyModel<Icrf, R>,
    gms: &[IcRecordGm],
    de: &De440,
    x: &mut [R],
    v: &mut [R],
) -> Result<(f64, f64), PropagateError> {
    let n = model.layout().len();
    let moon = model.layout().moon_pair().map(|p| p.moon);
    let c = Velocity::from_km_per_s(R::from_f64(de.speed_of_light().km_per_s()))
        .map_err(|e| setup("rebarycentre", e.to_string(), "check the DE440 table"))?;
    let offset = |x: &[R], v: &[R]| -> Result<([R; 3], [R; 3]), PropagateError> {
        let s = model.unpack(x, v)?;
        let bodies = s
            .states()?
            .iter()
            .zip(gms)
            .map(|(b, &(_, gm))| {
                let (p, w) = crate::setup::raw(&b.1);
                BarycentreBody {
                    gm: R::from_f64(gm),
                    position: p,
                    velocity: w,
                }
            })
            .collect::<Vec<_>>();
        Ok(relativistic_barycentre_with_rate(&bodies, c)?)
    };
    for _ in 0..REBARYCENTRE_PASSES {
        let (r, w) = offset(x, v)?;
        for i in (0..n).filter(|&i| Some(i) != moon) {
            for k in 0..3 {
                if let (Some(xs), Some(vs), Some(&rk), Some(&wk)) = (
                    x.get_mut(k * n + i),
                    v.get_mut(k * n + i),
                    r.get(k),
                    w.get(k),
                ) {
                    *xs -= rk;
                    *vs -= wk;
                }
            }
        }
    }
    let (r, w) = offset(x, v)?;
    let norm = |a: [R; 3]| {
        let [p, q, z] = a.map(R::to_f64);
        p.hypot(q).hypot(z)
    };
    Ok((norm(r), norm(w)))
}

/// Position and velocity of storage slot `i` in component-major flat vectors.
fn slot<R: Real>(x: &[R], n: usize, i: usize) -> [R; 3] {
    [0, 1, 2].map(|k| x.get(k * n + i).copied().unwrap_or(R::ZERO))
}

/// `(r_b − r_c)·(v_b − v_c)` of slots `b`, `c`.
fn rdotv<R: Real>(x: &[R], v: &[R], n: usize, b: usize, c: usize) -> R {
    let (rb, rc, vb, vc) = (slot(x, n, b), slot(x, n, c), slot(v, n, b), slot(v, n, c));
    (0..3).fold(R::ZERO, |s, k| {
        let dr = rb.get(k).copied().unwrap_or(R::ZERO) - rc.get(k).copied().unwrap_or(R::ZERO);
        let dv = vb.get(k).copied().unwrap_or(R::ZERO) - vc.get(k).copied().unwrap_or(R::ZERO);
        dr.mul_add(dv, s)
    })
}

/// One perihelion event: storage slots and NAIF ids of the body and the central body.
#[derive(Debug, Clone, Copy)]
struct Event {
    body: usize,
    central: usize,
    body_id: i32,
    central_id: i32,
}

/// A prepared N-body run in backend `R`.
pub(crate) struct NBodyRun<R: Real> {
    spec: ScenarioSpec,
    model: NBodyModel<Icrf, R>,
    /// The f64 force model of the NUM-007 mixed mode (`precision = "mixed"`); `None` otherwise.
    forces64: Option<NBodyModel<Icrf, f64>>,
    ias: Stepper<R>,
    n: usize,
    t_end: R,
    cadence: Option<R>,
    events: Vec<Event>,
    g_last: Vec<R>,
    driver: DriverState,
    meta: RunOutput,
    records: Vec<IcRecord>,
    ic_epoch: ferroastro_scenario::Parts,
}

impl<R: Real> std::fmt::Debug for NBodyRun<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NBodyRun")
            .field("scenario", &self.spec.name)
            .field("t", &self.ias.time().to_f64())
            .finish_non_exhaustive()
    }
}

impl<R: Real> NBodyRun<R> {
    /// Build the run from the scenario; with `restore`, continue from a checkpoint instead of
    /// the initial conditions (the ICs are still assembled, to check the IC hash).
    #[expect(
        clippy::too_many_lines,
        reason = "one linear set-up: ICs, model, hash, integrator or restore, events, metadata"
    )]
    pub(crate) fn new(
        spec: &ScenarioSpec,
        data: &DataSources<'_>,
        restore: Option<(&IntegratorSnapshot, &DriverState, &str)>,
    ) -> Result<Self, PropagateError> {
        let name = spec.name.as_str();
        let de = De440::load()?;
        let (ep, jd) = epoch::<R>(spec, data)?;
        let (mut states, pair, mut source) = nbody_ics::<R>(spec, data, ep)?;
        let (small, small_states, small_source) = small_bodies::<R>(spec, data, &states, jd)?;
        for (id, st) in small_states.states()? {
            states.push(id, &st)?;
        }
        if !small_source.is_empty() {
            source = format!("{source}; {small_source}");
        }
        let forces64 = if spec.integrator.precision == Precision::Mixed {
            Some(model::<f64>(spec, data, &de, &small)?.0)
        } else {
            None
        };
        let (model, gms) = model::<R>(spec, data, &de, &small)?;
        let (mut x0, mut v0) = model.pack(&states, pair.as_ref())?;
        let n = model.layout().len();
        if spec.initial_conditions.barycentre == Some(IcBarycentre::Relativistic) {
            let residual = rebarycentre(&model, &gms, &de, &mut x0, &mut v0)?;
            source = format!(
                "{source}; re-barycentred to the relativistic SSB (Park et al. 2021 Eqs. 1-2, \
                 Subbotin et al. 2023 Eq. 5) in {}, residual {:.3e} km, {:.3e} km/s",
                R::NAME,
                residual.0,
                residual.1
            );
        }
        let records: Vec<IcRecord> = gms
            .iter()
            .enumerate()
            .map(|(i, &(naif, gm))| IcRecord {
                naif,
                gm: [gm, 0.0],
                position: parts3(slot(&x0, n, i)),
                velocity: parts3(slot(&v0, n, i)),
            })
            .collect();
        let constants = constants_version(&de);
        let ic_epoch = parts(ep.jd());
        let hash = ic_hash(&constants, spec.mode.label(), ic_epoch, &records);
        let cfg = ias15_config(spec, true);
        let abm = abm_config(spec);
        let dt0_days = spec.integrator.dt0_days.unwrap_or(DEFAULT_DT0_DAYS);
        let step_days = spec.integrator.step_days.unwrap_or(DEFAULT_ABM_STEP_DAYS);
        let fixed_days = spec.integrator.step_days.unwrap_or(DEFAULT_FIXED_STEP_DAYS);
        let wh = crate::wh_config(spec);
        let irk = crate::irk16_config(spec);
        let dir = if spec.span.years < 0.0 { -1.0 } else { 1.0 };
        let ias = match restore {
            Some((snap, _, ic)) => {
                if ic != hash {
                    return Err(PropagateError::Checkpoint {
                        problem: format!(
                            "IC hash {ic} of the checkpoint differs from this run's {hash}"
                        ),
                    });
                }
                if snap.integrator() != spec.integrator.name.label() {
                    return Err(PropagateError::Checkpoint {
                        problem: format!(
                            "checkpoint holds an {} state, the scenario integrates with {}",
                            snap.integrator(),
                            spec.integrator.name.label()
                        ),
                    });
                }
                Stepper::restore(snap)?
            }
            None => match spec.integrator.name {
                IntegratorName::Abm => Stepper::Abm(Abm::new(
                    3 * n,
                    R::ZERO,
                    &x0,
                    &v0,
                    R::from_f64(dir * step_days * SECONDS_PER_DAY),
                    abm,
                )?),
                IntegratorName::Whfast => {
                    ferroastro_symplectic::check_model(&model)?;
                    let gm: Vec<R> = (0..n).map(|i| model.layout().gm(i)).collect();
                    Stepper::Whfast(Whfast::new(
                        &model,
                        &gm,
                        R::ZERO,
                        &x0,
                        &v0,
                        R::from_f64(dir * fixed_days * SECONDS_PER_DAY),
                        wh,
                    )?)
                }
                IntegratorName::Irk16 => Stepper::Irk16(Irk16::new(
                    3 * n,
                    R::ZERO,
                    &x0,
                    &v0,
                    R::from_f64(dir * fixed_days * SECONDS_PER_DAY),
                    irk,
                )?),
                IntegratorName::Ias15 => Stepper::Ias15(Ias15::new(
                    3 * n,
                    R::ZERO,
                    &x0,
                    &v0,
                    R::from_f64(dir * dt0_days * SECONDS_PER_DAY),
                    cfg,
                )?),
            },
        };
        let integrator = match spec.integrator.name {
            IntegratorName::Abm => format!("{}, h = {step_days} d", abm.describe()),
            IntegratorName::Whfast => format!("{}, h = {fixed_days} d", wh.describe()),
            IntegratorName::Irk16 => format!("{}, h = {fixed_days} d", irk.describe()),
            IntegratorName::Ias15 => format!(
                "IAS15 epsilon = {:e}, safety = {}, PC tolerance = {:e}, dense output",
                cfg.epsilon, cfg.safety, cfg.pc_tolerance
            ),
        };
        let mut events = Vec::new();
        let central = spec.central_body.naif();
        let index = |id: i32| model.layout().index_of(NaifId(id));
        let geocentric = model.layout().moon_pair().is_some();
        for b in &spec.output.perihelion {
            let id = b.naif();
            let moon_slot = |x: i32| geocentric && (x == 399 || x == 301);
            if moon_slot(id) || moon_slot(central) {
                return Err(ScenarioError::Unsupported {
                    name: name.to_owned(),
                    switch: format!(
                        "output.perihelion = {} with geocentric Moon variables",
                        b.label()
                    ),
                    runner: crate::RUNNER.into(),
                    reason: "events are located on plain barycentric slots only".into(),
                }
                .into());
            }
            let (Some(bi), Some(ci)) = (index(id), index(central)) else {
                return Err(setup(
                    name,
                    "perihelion body not in the model",
                    "list it in bodies",
                ));
            };
            events.push(Event {
                body: bi,
                central: ci,
                body_id: id,
                central_id: central,
            });
        }
        let g_last = events
            .iter()
            .map(|e| rdotv(ias.positions(), ias.velocities(), n, e.body, e.central))
            .collect();
        let driver = restore.map_or(
            DriverState {
                next_sample: 0,
                passages: 0,
            },
            |(_, d, _)| d.clone(),
        );
        let meta = RunOutput {
            scenario: spec.name.clone(),
            scenario_digest: spec.digest(),
            ic_hash: hash,
            precision: spec.integrator.precision.label().into(),
            constants,
            ic_source: source,
            epoch_jd_tdb: parts(R::from_f64(jd)),
            model: tag_overrides(
                format!(
                    "{}; {integrator}, {}",
                    model.describe(),
                    if spec.integrator.precision == Precision::Mixed {
                        "mixed (NUM-007): double-double state, integrator sums and coefficients, \
                         f64 force terms with compensated accumulation into double-double"
                    } else {
                        R::NAME
                    }
                ),
                data,
            ),
            states: Vec::new(),
            passages: Vec::new(),
            final_state: None,
            stats: ias.stats(),
            finished: false,
        };
        Ok(Self {
            spec: spec.clone(),
            model,
            forces64,
            ias,
            n,
            t_end: R::from_f64(spec.span_seconds()),
            // Signed like the span, so a backward run samples at t = 0, −c, −2c, … (WP14 fix:
            // WP13 sampled only forwards, so a backward run recorded its initial state only).
            cadence: spec
                .output
                .cadence_days
                .map(|d| R::from_f64(dir * d * SECONDS_PER_DAY)),
            events,
            g_last,
            driver,
            meta,
            records,
            ic_epoch,
        })
    }

    /// The initial conditions as hashed (constants version, epoch, per-body records, hash).
    pub(crate) fn initial(&self) -> crate::InitialRecords {
        crate::InitialRecords {
            constants: self.meta.constants.clone(),
            mode: self.spec.mode.label().to_owned(),
            epoch_jd_tdb: self.ic_epoch,
            records: self.records.clone(),
            ic_hash: self.meta.ic_hash.clone(),
        }
    }

    fn record(&self, t: R, x: &[R], v: &[R]) -> Result<StateRecord, PropagateError> {
        let s = self.model.unpack(x, v)?;
        let mut rec = StateRecord {
            t: parts(t),
            bodies: Vec::with_capacity(self.n),
            positions: Vec::with_capacity(self.n),
            velocities: Vec::with_capacity(self.n),
        };
        let keep = |id: i32| {
            self.spec
                .output
                .sample_bodies
                .as_ref()
                .is_none_or(|list| list.iter().any(|b| b.naif() == id))
        };
        for b in s.states()?.into_iter().filter(|b| keep(b.0.0)) {
            rec.bodies.push(b.0.0);
            let (p, w) = crate::setup::raw(&b.1);
            rec.positions.push(parts3(p));
            rec.velocities.push(parts3(w));
        }
        Ok(rec)
    }

    fn sample_time(&self, k: u64) -> Option<R> {
        // k < 2^53 in any feasible run, so the conversion is exact.
        #[expect(
            clippy::cast_precision_loss,
            reason = "sample indices stay far below 2^53"
        )]
        let kf = k as f64;
        self.cadence.map(|c| R::from_f64(kf) * c)
    }

    /// Advance until the end of the span, the passage count, or (if given) the first accepted
    /// step that reaches `stop_at` (seconds from the initial epoch). Returns the output of this
    /// segment.
    pub(crate) fn advance(&mut self, stop_at: Option<f64>) -> Result<RunOutput, PropagateError> {
        let mut out = self.meta.clone();
        let fwd = self.t_end >= R::ZERO;
        let after = |a: R, b: R| if fwd { a > b } else { a < b };
        let not_after = |a: R, b: R| !after(a, b);
        let stop_at = stop_at.map(R::from_f64);
        let (mut xs, mut vs) = (vec![R::ZERO; 3 * self.n], vec![R::ZERO; 3 * self.n]);
        // The initial sample (k = 0) of a fresh run.
        if self.driver.next_sample == 0 && self.ias.steps() == 0 && self.cadence.is_some() {
            out.states
                .push(self.record(R::ZERO, self.ias.positions(), self.ias.velocities())?);
            self.driver.next_sample = 1;
        }
        let wanted = self.spec.span.stop_after_passages.map(u64::from);
        loop {
            let rep = match &self.forces64 {
                Some(f) => self.ias.step(&MixedForces::new(f), self.t_end)?,
                None => self.ias.step(&self.model, self.t_end)?,
            };
            if !rep.moved {
                out.finished = true;
                break;
            }
            let (t0, t1) = self
                .ias
                .last_step_span()
                .ok_or_else(|| setup(&self.spec.name, "dense output is off", "internal error"))?;
            // Samples inside (t0, t1].
            while let Some(tk) = self.sample_time(self.driver.next_sample) {
                // ABM's last step may end after the span; samples stop at its end.
                if !(after(tk, t0) && not_after(tk, t1) && not_after(tk, self.t_end)) {
                    break;
                }
                self.ias.dense_state(&self.model, tk, &mut xs, &mut vs)?;
                out.states.push(self.record(tk, &xs, &vs)?);
                self.driver.next_sample += 1;
            }
            // Perihelion events (r·v rising through zero in time).
            for (j, e) in self.events.clone().iter().enumerate() {
                let g_now = rdotv(
                    self.ias.positions(),
                    self.ias.velocities(),
                    self.n,
                    e.body,
                    e.central,
                );
                let g_prev = self.g_last.get(j).copied().unwrap_or(g_now);
                let (early, late) = if fwd {
                    (g_prev, g_now)
                } else {
                    (g_now, g_prev)
                };
                if early < R::ZERO && late >= R::ZERO {
                    let n = self.n;
                    // An event after the end of the span (ABM's last step only) is not recorded.
                    if let Some(te) = self
                        .ias
                        .locate_event(&self.model, |_, x, v| rdotv(x, v, n, e.body, e.central))?
                        .filter(|&te| not_after(te, self.t_end))
                    {
                        self.ias.dense_state(&self.model, te, &mut xs, &mut vs)?;
                        let sub = |a: [R; 3], b: [R; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                        out.passages.push(PassageRecord {
                            body: e.body_id,
                            central: e.central_id,
                            t: parts(te),
                            r: parts3(sub(slot(&xs, n, e.body), slot(&xs, n, e.central))),
                            v: parts3(sub(slot(&vs, n, e.body), slot(&vs, n, e.central))),
                        });
                        self.driver.passages += 1;
                    }
                }
                if let Some(g) = self.g_last.get_mut(j) {
                    *g = g_now;
                }
            }
            if wanted.is_some_and(|w| self.driver.passages >= w) || rep.reached_end {
                out.finished = true;
                break;
            }
            if stop_at.is_some_and(|s| not_after(s, rep.t)) {
                break;
            }
        }
        // Nested `if let` (not a let-chain): the workspace MSRV is 1.85.
        if let Some(w) = wanted {
            if out.finished && self.driver.passages < w {
                return Err(PropagateError::Run {
                    scenario: self.spec.name.clone(),
                    problem: format!(
                        "only {} of {w} perihelion passages within span.years = {}",
                        self.driver.passages, self.spec.span.years
                    ),
                });
            }
        }
        out.final_state = Some(if out.finished && after(self.ias.time(), self.t_end) {
            // ABM's last step ended after the span: the state at its end, from dense output.
            self.ias
                .dense_state(&self.model, self.t_end, &mut xs, &mut vs)?;
            self.record(self.t_end, &xs, &vs)?
        } else {
            self.record(self.ias.time(), self.ias.positions(), self.ias.velocities())?
        });
        out.stats = self.ias.stats();
        Ok(out)
    }

    /// The accelerations of the integration variables at the integrator's current state (km/s²,
    /// storage order; with a geocentric pair the Earth slot is the EMB and the Moon slot Moon −
    /// Earth), as the integrator sees them: through [`MixedForces`] in the mixed mode.
    pub(crate) fn accelerations(&self) -> Result<Vec<[R; 3]>, PropagateError> {
        use ferroastro_integrators::SecondOrderSystem as _;
        let mut a = vec![R::ZERO; 3 * self.n];
        let (t, x, v) = (self.ias.time(), self.ias.positions(), self.ias.velocities());
        match &self.forces64 {
            Some(f) => MixedForces::new(f).acceleration(t, x, v, &mut a)?,
            None => self.model.acceleration(t, x, v, &mut a)?,
        }
        Ok((0..self.n).map(|i| slot(&a, self.n, i)).collect())
    }

    /// The model's total energy divided by G (km⁵/s⁴) at the integrator's current state, or
    /// `None` if a term is not conservative (NUM-023 diagnostic; WP18 speed-table accuracy).
    pub(crate) fn energy_over_g(&self) -> Option<f64> {
        self.model
            .energy_over_g(self.ias.positions(), self.ias.velocities())
            .map(R::to_f64)
    }

    /// The integrator snapshot and the driver state, for a checkpoint.
    pub(crate) fn checkpoint_parts(
        &self,
    ) -> Result<(IntegratorSnapshot, DriverState, String), PropagateError> {
        Ok((
            self.ias.snapshot()?,
            self.driver.clone(),
            self.meta.ic_hash.clone(),
        ))
    }
}

/// First trial step of N-body runs when the scenario sets none (days; as WP12).
pub const DEFAULT_DT0_DAYS: f64 = 0.1;

/// ABM fixed step when the scenario sets none: 1/16 d (Subbotin et al. 2023; WP24).
pub const DEFAULT_ABM_STEP_DAYS: f64 = 0.0625;

/// `WHFast` and IRK16 fixed step when the scenario sets none: 1 d (WP29).
pub const DEFAULT_FIXED_STEP_DAYS: f64 = 1.0;
