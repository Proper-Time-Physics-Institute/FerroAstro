// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung GR-SCHW analytic (PHY-004): `6πGM/(c²a(1−e²))` per orbit from DE440 GM☉, c, au and the
//! SW92 J2000 a, e; period from GM☉ + GM☿ (caveat C7).
//!
//! `cargo run -p ferroastro-ladder --example phy004_gr_analytic`

#[path = "common/mod.rs"]
mod common;

use ferroastro_analytic::advance::schwarzschild_advance;
use ferroastro_constants::Body;

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    let el = c.scn.kepler_elements(c.de.au())?;
    let (gm_sun, gm_me) = (c.de.gm_sun(), c.de.gm(Body::Mercury));
    let adv = schwarzschild_advance(gm_sun, c.de.speed_of_light(), el.a, el.e, gm_sun + gm_me)?;
    println!(
        "GR-SCHW analytic: {:.9} arcsec/cy ({:e} rad/orbit, {:.6} orbits/cy); constants DE440 \
         (header.440 sha256 {})",
        adv.arcsec_per_century(),
        adv.per_orbit.radians(),
        adv.orbits_per_century,
        c.de.table().source().sha256
    );
    c.check("PHY004-GR-RATE", adv.arcsec_per_century())
}
