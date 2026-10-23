// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The newtypes compile and compose with the double-double backend (API-001).
use ferroastro_core::frames::{Barycentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::{Angle, Length, TimeSpan, Velocity};
use ferroastro_real::{DoubleDouble, Real};

fn main() {
    let one = DoubleDouble::ONE;
    let l: Length<DoubleDouble> = Length::from_km(one).unwrap();
    let t: TimeSpan<DoubleDouble> = TimeSpan::from_seconds(one + one).unwrap();
    let v: Velocity<DoubleDouble> = l / t;
    let p = Position::<Icrf, Barycentric, DoubleDouble>::new(l, l, l);
    let s = State::new(p, [v, v, v]);
    let d = (s.position() - p).norm();
    let e = Epoch::<Tdb, DoubleDouble>::from_jd_parts(one, one).unwrap();
    let dt: TimeSpan<DoubleDouble> = (e + t) - e;
    let w = Angle::from_radians(one).unwrap() / dt;
    let widened: Length<DoubleDouble> = Length::from_km(1.0).unwrap().cast();
    let _ = (d, w, widened, v * t);
}
