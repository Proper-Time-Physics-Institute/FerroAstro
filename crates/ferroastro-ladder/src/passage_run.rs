// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The IAS15-driven perihelion-passage run of a single-body rung. Since WP13 it lives in
//! [`ferroastro_propagate::passages`], shared with scenario-driven test-particle runs (one code
//! path, bit-identical results); this module re-exports it unchanged.

pub use ferroastro_propagate::passages::{
    PassageRun, UniformSamples, find_perihelia, perihelia_until, sample_uniform,
};
