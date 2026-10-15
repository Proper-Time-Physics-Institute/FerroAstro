// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The [`Ephemeris`] trait, NAIF ids, and segment chaining.
//!
//! A state of a target relative to a centre is assembled from the segment chains of both bodies
//! towards the solar-system barycentre (NAIF 0): each chain follows "target → the centre of its
//! covering segment → …". The two chains are cut at their **lowest common ancestor**, so that
//! for example Moon relative to Earth is `(301 wrt 3) − (399 wrt 3)` and never passes through
//! the barycentre (which would cost precision).

use crate::EphemError;
use crate::spk::{Spk, StateKm};
use ferroastro_core::frames::{Barycentric, Center, Heliocentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_real::Real;

/// A NAIF integer body code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NaifId(pub i32);

impl NaifId {
    /// Solar-system barycentre.
    pub const SSB: NaifId = NaifId(0);
    /// Mercury barycentre.
    pub const MERCURY_BARYCENTER: NaifId = NaifId(1);
    /// Venus barycentre.
    pub const VENUS_BARYCENTER: NaifId = NaifId(2);
    /// Earth–Moon barycentre.
    pub const EARTH_MOON_BARYCENTER: NaifId = NaifId(3);
    /// Mars barycentre.
    pub const MARS_BARYCENTER: NaifId = NaifId(4);
    /// Jupiter barycentre.
    pub const JUPITER_BARYCENTER: NaifId = NaifId(5);
    /// Saturn barycentre.
    pub const SATURN_BARYCENTER: NaifId = NaifId(6);
    /// Uranus barycentre.
    pub const URANUS_BARYCENTER: NaifId = NaifId(7);
    /// Neptune barycentre.
    pub const NEPTUNE_BARYCENTER: NaifId = NaifId(8);
    /// Pluto barycentre.
    pub const PLUTO_BARYCENTER: NaifId = NaifId(9);
    /// Sun.
    pub const SUN: NaifId = NaifId(10);
    /// Mercury.
    pub const MERCURY: NaifId = NaifId(199);
    /// Venus.
    pub const VENUS: NaifId = NaifId(299);
    /// Moon.
    pub const MOON: NaifId = NaifId(301);
    /// Earth.
    pub const EARTH: NaifId = NaifId(399);
}

/// The NAIF frame id that JPL DE SPK files carry (`J2000`). For the `DE4xx` ephemerides this label
/// denotes the ICRF-aligned frame of the integration (Park et al. 2021), so states in this frame
/// are returned as [`Icrf`].
pub const NAIF_FRAME_J2000: i32 = 1;

/// A typed centre with a NAIF id.
pub trait NaifCenter: Center {
    /// The NAIF id of this centre.
    const NAIF: NaifId;
}

impl NaifCenter for Barycentric {
    const NAIF: NaifId = NaifId::SSB;
}

impl NaifCenter for Heliocentric {
    const NAIF: NaifId = NaifId::SUN;
}

/// A source of body states (DEP-011): any backend `R`, TDB epochs.
pub trait Ephemeris {
    /// The state of `target` relative to `center` at `epoch`, in km and km/s, and the NAIF frame
    /// id of the result.
    ///
    /// # Errors
    /// [`EphemError::NoPath`] when no segment chain connects the two bodies (or it mixes frames),
    /// [`EphemError::Coverage`] when a needed segment does not cover the epoch, and the
    /// evaluation errors of the segments.
    fn relative_state_km<R: Real>(
        &self,
        target: NaifId,
        center: NaifId,
        epoch: Epoch<Tdb, R>,
    ) -> Result<(StateKm<R>, i32), EphemError>;

    /// The typed ICRF state of `target` relative to the typed centre `C` (for example
    /// [`Barycentric`] or [`Heliocentric`]).
    ///
    /// # Errors
    /// As [`Ephemeris::relative_state_km`], and [`EphemError::NoPath`] if the chain's frame is not
    /// NAIF frame 1 (the ICRF-aligned DE frame).
    fn state<C: NaifCenter, R: Real>(
        &self,
        target: NaifId,
        epoch: Epoch<Tdb, R>,
    ) -> Result<State<Icrf, C, R>, EphemError> {
        let (s, frame) = self.relative_state_km(target, C::NAIF, epoch)?;
        if frame != NAIF_FRAME_J2000 {
            return Err(EphemError::NoPath {
                target: target.0,
                center: C::NAIF.0,
                reason: format!("the chain is in NAIF frame {frame}, not 1 (ICRF-aligned J2000)"),
                fix: "use relative_state_km and rotate explicitly with ferroastro-frames",
            });
        }
        let nf = |what: &'static str, unit: &'static str| EphemError::NonFinite {
            target: target.0,
            center: C::NAIF.0,
            quantity: what,
            unit,
        };
        let km = |x: R| Length::from_km(x).map_err(|_| nf("position", "km"));
        let kms = |x: R| Velocity::from_km_per_s(x).map_err(|_| nf("velocity", "km/s"));
        let [x, y, z] = s.position;
        let [vx, vy, vz] = s.velocity;
        Ok(State::new(
            Position::new(km(x)?, km(y)?, km(z)?),
            [kms(vx)?, kms(vy)?, kms(vz)?],
        ))
    }
}

/// Maximum chain length (guards against cyclic files).
const MAX_CHAIN: usize = 32;

/// One chain link: a body, its state relative to the next body, and the segment's frame.
type Link<R> = (NaifId, StateKm<R>, i32);

impl Spk<'_> {
    /// The chain of bodies from `body` towards the root, with each link's state and frame:
    /// `[(body, state wrt next, frame), …]`, ending at a body with no covering segment.
    fn chain<R: Real>(
        &self,
        body: NaifId,
        epoch: Epoch<Tdb, R>,
    ) -> Result<(Vec<Link<R>>, NaifId), EphemError> {
        let mut links = Vec::new();
        let mut at = body;
        while let Some(seg) = self.segment_for(at.0, epoch) {
            if links.len() >= MAX_CHAIN {
                return Err(EphemError::NoPath {
                    target: body.0,
                    center: seg.center,
                    reason: format!("segment chain longer than {MAX_CHAIN} (cycle?)"),
                    fix: "the file's segments form a cycle; check its contents",
                });
            }
            links.push((at, seg.state(epoch)?, seg.frame));
            at = NaifId(seg.center);
        }
        Ok((links, at))
    }
}

impl Ephemeris for Spk<'_> {
    fn relative_state_km<R: Real>(
        &self,
        target: NaifId,
        center: NaifId,
        epoch: Epoch<Tdb, R>,
    ) -> Result<(StateKm<R>, i32), EphemError> {
        if target == center {
            return Ok((StateKm::zero(), NAIF_FRAME_J2000));
        }
        let (tchain, troot) = self.chain(target, epoch)?;
        let (cchain, croot) = self.chain(center, epoch)?;
        // Bodies visited by each chain, in order (the body itself first, the root last).
        let tnodes: Vec<NaifId> = tchain.iter().map(|l| l.0).chain([troot]).collect();
        let cnodes: Vec<NaifId> = cchain.iter().map(|l| l.0).chain([croot]).collect();
        let Some((ti, ci)) = tnodes
            .iter()
            .enumerate()
            .find_map(|(i, n)| cnodes.iter().position(|m| m == n).map(|j| (i, j)))
        else {
            // Report coverage if a segment for the body exists but not at this epoch.
            for b in [target, center] {
                if let Some(seg) = self.segments().iter().rev().find(|s| s.target == b.0) {
                    if !seg.covers(epoch) {
                        return Err(seg.state(epoch).err().unwrap_or(EphemError::NoPath {
                            target: target.0,
                            center: center.0,
                            reason: "no covering segment".into(),
                            fix: "use an epoch inside the file's coverage",
                        }));
                    }
                }
            }
            return Err(EphemError::NoPath {
                target: target.0,
                center: center.0,
                reason: format!(
                    "the target's chain {tnodes:?} and the centre's chain {cnodes:?} share no body"
                ),
                fix: "load an SPK that connects both bodies (e.g. both to the barycentre)",
            });
        };
        let mut frame = None;
        let mut sum = |links: &[Link<R>]| -> Result<StateKm<R>, EphemError> {
            let mut acc = StateKm::zero();
            for (_, s, f) in links {
                if *frame.get_or_insert(*f) != *f {
                    return Err(EphemError::NoPath {
                        target: target.0,
                        center: center.0,
                        reason: "the segment chain mixes reference frames".into(),
                        fix: "rotate segments into one frame before chaining",
                    });
                }
                acc = acc.add(s);
            }
            Ok(acc)
        };
        let t = sum(tchain.get(..ti).unwrap_or_default())?;
        let c = sum(cchain.get(..ci).unwrap_or_default())?;
        Ok((t.sub(&c), frame.unwrap_or(NAIF_FRAME_J2000)))
    }
}
