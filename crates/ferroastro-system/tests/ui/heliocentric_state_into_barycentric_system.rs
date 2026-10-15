// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
// A heliocentric state cannot be pushed into a barycentric system state.
use ferroastro_core::frames::{Barycentric, Heliocentric, Icrf, Position, State};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_system::SystemState;

fn main() {
    let helio: State<Icrf, Heliocentric> = State::new(
        Position::new(Length::ZERO, Length::ZERO, Length::ZERO),
        [Velocity::ZERO; 3],
    );
    let mut system = SystemState::<Icrf, Barycentric>::new();
    let _ = system.push(NaifId::MERCURY_BARYCENTER, &helio);
}
