// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! IAU 2015 Resolution B3 nominal solar and planetary **conversion** constants (PHY-003).
//!
//! These are conversion values, not best estimates (Prša et al. 2016, AJ 152, 41, Table 1;
//! arXiv:1605.09788). They exist for converting to and from nominal units only. The precision
//! ladder must use the DE440 constants ([`crate::De440`]) instead. Two guards keep them apart:
//!
//! 1. every value is wrapped in [`Nominal`], which does not deref or convert into the bare
//!    quantity; reading it takes an explicit [`Nominal::conversion_value`] call;
//! 2. the test `nominal_quarantine` fails if any source file outside this module (and its own
//!    tests) refers to the `nominal` namespace.
//!
//! Values are stored in km-based units, decimal-exact from the SI values of Table 1
//! (for example (GM)☉ᴺ = 1.3271244×10²⁰ m³ s⁻² = 1.3271244×10¹¹ km³ s⁻²).

use ferroastro_core::units::{GravParam, Length};

/// A nominal conversion constant. Deliberately has no `Deref`, `From` or arithmetic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nominal<T>(T);

impl<T: Copy> Nominal<T> {
    /// The value, for unit conversion only (never as a physical best estimate).
    #[must_use]
    pub const fn conversion_value(self) -> T {
        self.0
    }
}

/// Citation of the values in this module.
pub const CITATION: &str = "IAU 2015 Resolution B3, as printed in Prša, A. et al. 2016, AJ 152, \
    41, doi:10.3847/0004-6256/152/2/41, Table 1";

// IAU 2015 B3 Table 1, in km-based units (decimal-exact conversions from m). They are finite, so
// the constructors below cannot fail; the `unwrap_or` fallbacks are unreachable.
const GM_SUN_KM3_S2: f64 = 1.327_124_4e11;
const R_SUN_KM: f64 = 6.957e5;
const GM_EARTH_KM3_S2: f64 = 3.986_004e5;
const GM_JUPITER_KM3_S2: f64 = 1.266_865_3e8;

/// Nominal solar mass parameter (GM)☉ᴺ = 1.3271244×10²⁰ m³ s⁻².
#[must_use]
pub fn solar_mass_parameter() -> Nominal<GravParam> {
    Nominal(GravParam::from_km3_per_s2(GM_SUN_KM3_S2).unwrap_or(GravParam::ZERO))
}

/// Nominal solar radius R☉ᴺ = 6.957×10⁸ m. Differs from DE440 `ASUN` (696 000 km).
#[must_use]
pub fn solar_radius() -> Nominal<Length> {
    Nominal(Length::from_km(R_SUN_KM).unwrap_or(Length::ZERO))
}

/// Nominal terrestrial mass parameter (GM)ᴇᴺ = 3.986004×10¹⁴ m³ s⁻².
#[must_use]
pub fn terrestrial_mass_parameter() -> Nominal<GravParam> {
    Nominal(GravParam::from_km3_per_s2(GM_EARTH_KM3_S2).unwrap_or(GravParam::ZERO))
}

/// Nominal jovian mass parameter (GM)ᴊᴺ = 1.2668653×10¹⁷ m³ s⁻².
#[must_use]
pub fn jovian_mass_parameter() -> Nominal<GravParam> {
    Nominal(GravParam::from_km3_per_s2(GM_JUPITER_KM3_S2).unwrap_or(GravParam::ZERO))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_vectors::VectorSet;

    /// Verifies: PHY-003
    #[test]
    fn nominal_values_match_iau_2015_b3() {
        let v = VectorSet::load_workspace().unwrap();
        let checks = [
            (
                "IAU2015B3-GM-SUN-N",
                solar_mass_parameter().conversion_value().km3_per_s2(),
            ),
            ("IAU2015B3-R-SUN-N", solar_radius().conversion_value().km()),
            (
                "IAU2015B3-GM-EARTH-N",
                terrestrial_mass_parameter().conversion_value().km3_per_s2(),
            ),
            (
                "IAU2015B3-GM-JUPITER-N",
                jovian_mass_parameter().conversion_value().km3_per_s2(),
            ),
        ];
        for (id, got) in checks {
            let vec = v.get(id).unwrap();
            assert!(vec.requirement.iter().any(|r| r == "PHY-003"), "{id}");
            vec.check(got).unwrap();
        }
    }

    /// Verifies: PHY-003
    #[test]
    fn nominal_differs_from_de440() {
        // The quarantine matters because the values differ from the DE440 ones.
        let de = crate::De440::load().unwrap();
        let r_nom = solar_radius().conversion_value();
        assert!((r_nom.km() - de.sun_radius().km()).abs() > 1.0);
        let gm_nom = solar_mass_parameter().conversion_value();
        assert!((gm_nom.km3_per_s2() - de.gm_sun().km3_per_s2()).abs() > 1.0);
    }
}
