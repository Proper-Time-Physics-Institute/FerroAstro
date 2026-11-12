// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
// SSB barycentring needs a barycentric system; a heliocentric one does not compile.
use ferroastro_constants::De440;
use ferroastro_core::frames::{Heliocentric, Icrf};
use ferroastro_system::{Catalogue, MassModel, SystemState, barycentre};

fn main() {
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let helio = SystemState::<Icrf, Heliocentric>::new();
    let _ = barycentre(&cat, &helio, MassModel::Newtonian, de.speed_of_light());
}
