// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-scenario
//!
//! The tool-agnostic scenario spec of CI-002 (CRATE-PLAN L5): one TOML file under
//! `ci/scenarios/` names the bodies, the initial-condition source, the GM set, the force
//! switches, the integrator and its settings, the span and the output cadence, and drives
//! FerroAstro (`ferroastro-propagate`) and every oracle runner alike.
//!
//! * [`ScenarioSpec::parse`]: TOML → validated spec. An unknown force switch is `FA-E1202`
//!   (named); [`ScenarioSpec::refuse_unsupported`] lets a runner refuse what it does not
//!   implement with `FA-E1201` (named), never omitting it silently (REF-011).
//! * [`json_schema`]: the JSON Schema (schemars) committed as `ci/scenarios/scenario.schema.json`
//!   and used by `tools/validate_scenarios.py`.
//! * [`ic_hash`]: SHA-256 of the canonicalised initial conditions plus the constants version;
//!   differential runs assert it is identical (NUM-013).
//!
//! ```
//! use ferroastro_scenario::{Mode, ScenarioSpec};
//! let spec = ScenarioSpec::parse(r#"
//! schema_version = 1
//! name = "sun-mercury"
//! description = "Sun + Mercury, Newtonian"
//! mode = "n-body"
//! bodies = ["Sun", "Mercury"]
//! central_body = "Sun"
//! initial_conditions = { source = "de440-header" }
//! constants = { gm_set = "de440" }
//! forces = { newton = true }
//! integrator = { name = "ias15", precision = "f64" }
//! span = { years = 10.0 }
//! output = { perihelion = ["Mercury"] }
//! "#)?;
//! assert_eq!(spec.mode, Mode::NBody);
//! assert_eq!(spec.forces.requested(), ["newton"]);
//! let unknown = ScenarioSpec::parse(&r#"
//! schema_version = 1
//! name = "x"
//! description = "x"
//! mode = "n-body"
//! bodies = ["Sun", "Mercury"]
//! central_body = "Sun"
//! initial_conditions = { source = "de440-header" }
//! constants = { gm_set = "de440" }
//! forces = { newton = true, dark_matter = true }
//! integrator = { name = "ias15", precision = "f64" }
//! span = { years = 10.0 }
//! "#).unwrap_err();
//! assert!(unknown.to_string().starts_with("FA-E1202"));
//! # Ok::<(), ferroastro_scenario::ScenarioError>(())
//! ```

mod error;
mod hash;
mod spec;

pub use error::ScenarioError;
pub use hash::{IcRecord, Parts, canonical_ic_text, ic_hash};
pub use spec::{
    AbmCorrector, BodyName, ConstantsSpec, DAYS_PER_JULIAN_YEAR, Forces, GmSet, IcSource,
    InitialConditionsSpec, IntegratorName, IntegratorSpec, Mode, OutputSpec, Ppn, Precision,
    SCHEMA_VERSION, SWITCHES, ScenarioSpec, Span, json_schema,
};
