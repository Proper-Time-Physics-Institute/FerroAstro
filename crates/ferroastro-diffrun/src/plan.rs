// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Differential plans: configurations (each one or more legs from the same initial conditions)
//! and the signed combinations of their rates that the table reports.

use crate::DiffrunError;
use crate::error::plan_error;
use ferroastro_scenario::{BodyName, Forces, ScenarioSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// How a combination isolates its term (NUM-013, PHY-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    /// `ϖ̇(base + X) − ϖ̇(base)`: switch the body or term on against the minimal system
    /// (PHY-012 primary protocol, maintainer decision; provisional while OI-38 is open).
    Add,
    /// `ϖ̇(full) − ϖ̇(full without X)` (informational).
    Remove,
    /// `ϖ̇(base + X + Y) − ϖ̇(base + X) − ϖ̇(base + Y) + ϖ̇(base)`: the interaction of two
    /// toggles under [`Protocol::Add`] (reported separately).
    CrossTerm,
    /// `ϖ̇(full) − ϖ̇(full − X) − ϖ̇(full − Y) + ϖ̇(full − X − Y)`: the interaction of two toggles
    /// under [`Protocol::Remove`] (informational).
    CrossTermRemove,
}

impl Protocol {
    /// Label for tables.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Protocol::Add => "P-add",
            Protocol::Remove => "P-remove",
            Protocol::CrossTerm => "cross term (P-add)",
            Protocol::CrossTermRemove => "cross term (P-remove)",
        }
    }
}

/// A run configuration: legs that share their initial conditions (e.g. backwards and forwards
/// in time from the IC epoch).
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Unique label within the plan.
    pub label: String,
    /// The legs.
    pub legs: Vec<ScenarioSpec>,
}

/// A signed sum of configuration rates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Combination {
    /// Row label (e.g. `"Venus"`, `"Jupiter+Saturn"`).
    pub label: String,
    /// How it isolates its term.
    pub protocol: Protocol,
    /// `(coefficient, configuration label)`.
    pub terms: Vec<(i32, String)>,
}

/// A differential plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Plan name.
    pub name: String,
    /// NAIF ids whose initial conditions every configuration must contain (the shared IC hash
    /// is taken over them).
    pub base: Vec<i32>,
    /// The configurations.
    pub configs: Vec<Config>,
    /// The combinations reported.
    pub combinations: Vec<Combination>,
}

/// A body toggle: a row label and the bodies switched together (e.g. Earth and Moon).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyToggle {
    /// Row label.
    pub label: String,
    /// Bodies switched together.
    pub bodies: Vec<BodyName>,
}

impl BodyToggle {
    /// A toggle of `bodies` labelled `label`.
    #[must_use]
    pub fn new(label: &str, bodies: &[BodyName]) -> Self {
        Self {
            label: label.to_owned(),
            bodies: bodies.to_vec(),
        }
    }
}

/// A force-switch toggle: a row label and the change it makes to the template's switches.
#[derive(Debug, Clone)]
pub struct SwitchToggle {
    /// Row label.
    pub label: String,
    /// The change.
    pub apply: fn(&mut Forces),
}

/// Signed spans of the legs, Julian years (e.g. `[-969.6, 1030.6]`).
pub type LegYears<'a> = &'a [f64];

fn legs(
    plan: &str,
    label: &str,
    template: &ScenarioSpec,
    years: LegYears<'_>,
) -> Vec<ScenarioSpec> {
    years
        .iter()
        .enumerate()
        .map(|(i, &y)| {
            let mut s = template.clone();
            s.name = format!("{plan}/{label}/leg{i}");
            s.span.years = y;
            s
        })
        .collect()
}

fn with_bodies(template: &ScenarioSpec, order: &[BodyName], set: &BTreeSet<i32>) -> ScenarioSpec {
    let mut s = template.clone();
    s.bodies = order
        .iter()
        .copied()
        .filter(|b| set.contains(&b.naif()))
        .collect();
    s.description = format!(
        "{} [bodies: {}]",
        template.description,
        s.bodies
            .iter()
            .map(|b| b.label())
            .collect::<Vec<_>>()
            .join(", ")
    );
    s
}

fn naifs(bodies: &[BodyName]) -> BTreeSet<i32> {
    bodies.iter().map(|b| b.naif()).collect()
}

/// Storage order of a body plan (`full` if given, else base then the toggles in order), after
/// checking the toggles against the base and the full system.
fn storage_order(
    name: &str,
    base: &[BodyName],
    full: &[BodyName],
    toggles: &[BodyToggle],
) -> Result<Vec<BodyName>, DiffrunError> {
    let base_set = naifs(base);
    let full_set = naifs(full);
    // Storage order: `full` if given, else base then toggles in order.
    let mut order: Vec<BodyName> = if full.is_empty() {
        base.to_vec()
    } else {
        full.to_vec()
    };
    for t in toggles {
        if t.bodies.is_empty() {
            return Err(plan_error(
                name,
                format!("toggle {} is empty", t.label),
                "name at least one body",
            ));
        }
        for b in &t.bodies {
            if base_set.contains(&b.naif()) {
                return Err(plan_error(
                    name,
                    format!("toggle {} switches base body {}", t.label, b.label()),
                    "toggle only bodies outside the base",
                ));
            }
            if !full.is_empty() && !full_set.contains(&b.naif()) {
                return Err(plan_error(
                    name,
                    format!(
                        "toggle {} body {} is not in the full system",
                        t.label,
                        b.label()
                    ),
                    "add it to the full system",
                ));
            }
            if !order.contains(b) {
                order.push(*b);
            }
        }
    }
    if !full.is_empty() && !base_set.is_subset(&full_set) {
        return Err(plan_error(
            name,
            "the base is not part of the full system",
            "add the base bodies to the full system",
        ));
    }
    Ok(order)
}

impl Plan {
    /// Body toggles (PHY-012): configurations `base`, `base + X` for every toggle and
    /// `base + X + Y` for every pair; when `full` is not empty also `full`, `full − X` and
    /// `full − X − Y`. Combinations: P-add for every toggle, the P-add cross term of every pair,
    /// and (if `full`) P-remove for every toggle and the P-remove cross term of every pair. `template` supplies everything but the body list and the span.
    ///
    /// # Errors
    /// `FA-E1400` if a toggle is empty, repeats a base body, is missing from `full`, labels
    /// repeat, or a pair index is out of range.
    pub fn bodies(
        name: &str,
        template: &ScenarioSpec,
        base: &[BodyName],
        full: &[BodyName],
        toggles: &[BodyToggle],
        pairs: &[(usize, usize)],
        years: LegYears<'_>,
    ) -> Result<Self, DiffrunError> {
        let base_set = naifs(base);
        let full_set = naifs(full);
        let order = storage_order(name, base, full, toggles)?;
        let mut configs = Vec::new();
        let mut combinations = Vec::new();
        let mut add = |label: String, set: &BTreeSet<i32>| {
            configs.push(Config {
                legs: legs(name, &label, &with_bodies(template, &order, set), years),
                label,
            });
        };
        add("base".into(), &base_set);
        let toggle_set = |t: &BodyToggle| naifs(&t.bodies);
        for t in toggles {
            let set: BTreeSet<i32> = base_set.union(&toggle_set(t)).copied().collect();
            add(format!("base+{}", t.label), &set);
            combinations.push(Combination {
                label: t.label.clone(),
                protocol: Protocol::Add,
                terms: vec![(1, format!("base+{}", t.label)), (-1, "base".into())],
            });
        }
        for &(i, j) in pairs {
            let (Some(x), Some(y)) = (toggles.get(i), toggles.get(j)) else {
                return Err(plan_error(
                    name,
                    format!("pair ({i}, {j}) is out of range"),
                    "index the toggle list",
                ));
            };
            let set: BTreeSet<i32> = base_set
                .union(&toggle_set(x))
                .copied()
                .collect::<BTreeSet<i32>>()
                .union(&toggle_set(y))
                .copied()
                .collect();
            let label = format!("{}+{}", x.label, y.label);
            add(format!("base+{label}"), &set);
            combinations.push(Combination {
                label,
                protocol: Protocol::CrossTerm,
                terms: vec![
                    (1, format!("base+{}+{}", x.label, y.label)),
                    (-1, format!("base+{}", x.label)),
                    (-1, format!("base+{}", y.label)),
                    (1, "base".into()),
                ],
            });
        }
        if !full.is_empty() {
            add("full".into(), &full_set);
            for t in toggles {
                let set: BTreeSet<i32> = full_set.difference(&toggle_set(t)).copied().collect();
                add(format!("full-{}", t.label), &set);
                combinations.push(Combination {
                    label: t.label.clone(),
                    protocol: Protocol::Remove,
                    terms: vec![(1, "full".into()), (-1, format!("full-{}", t.label))],
                });
            }
            for &(i, j) in pairs {
                if let (Some(x), Some(y)) = (toggles.get(i), toggles.get(j)) {
                    let both: BTreeSet<i32> =
                        toggle_set(x).union(&toggle_set(y)).copied().collect();
                    let set: BTreeSet<i32> = full_set.difference(&both).copied().collect();
                    add(format!("full-{}-{}", x.label, y.label), &set);
                    combinations.push(Combination {
                        label: format!("{}+{}", x.label, y.label),
                        protocol: Protocol::CrossTermRemove,
                        terms: vec![
                            (1, "full".into()),
                            (-1, format!("full-{}", x.label)),
                            (-1, format!("full-{}", y.label)),
                            (1, format!("full-{}-{}", x.label, y.label)),
                        ],
                    });
                }
            }
        }
        let plan = Self {
            name: name.to_owned(),
            base: base.iter().map(|b| b.naif()).collect(),
            configs,
            combinations,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Force-switch toggles (NUM-013(a)): configuration `base` (the template) and one per toggle;
    /// combination `toggle − base` ([`Protocol::Add`]) for each.
    ///
    /// # Errors
    /// `FA-E1400` for repeated labels or a toggle that changes nothing.
    pub fn switches(
        name: &str,
        template: &ScenarioSpec,
        toggles: &[SwitchToggle],
        years: LegYears<'_>,
    ) -> Result<Self, DiffrunError> {
        let mut configs = vec![Config {
            label: "base".into(),
            legs: legs(name, "base", template, years),
        }];
        let mut combinations = Vec::new();
        for t in toggles {
            let mut s = template.clone();
            (t.apply)(&mut s.forces);
            if s.forces == template.forces {
                return Err(plan_error(
                    name,
                    format!("toggle {} changes no switch", t.label),
                    "make the toggle change the template's forces",
                ));
            }
            configs.push(Config {
                label: t.label.clone(),
                legs: legs(name, &t.label, &s, years),
            });
            combinations.push(Combination {
                label: t.label.clone(),
                protocol: Protocol::Add,
                terms: vec![(1, t.label.clone()), (-1, "base".into())],
            });
        }
        let plan = Self {
            name: name.to_owned(),
            base: template.bodies.iter().map(|b| b.naif()).collect(),
            configs,
            combinations,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Check labels, legs and combinations.
    ///
    /// # Errors
    /// `FA-E1400` for a repeated or unknown label, a configuration without legs or without a
    /// base body, or an empty combination.
    pub fn validate(&self) -> Result<(), DiffrunError> {
        let n = self.name.as_str();
        let mut seen = BTreeSet::new();
        for c in &self.configs {
            if !seen.insert(c.label.as_str()) {
                return Err(plan_error(
                    n,
                    format!("configuration {} appears twice", c.label),
                    "use unique labels",
                ));
            }
            if c.legs.is_empty() {
                return Err(plan_error(
                    n,
                    format!("configuration {} has no legs", c.label),
                    "give every configuration at least one leg",
                ));
            }
            for s in &c.legs {
                for b in &self.base {
                    if !s.bodies.iter().any(|x| x.naif() == *b) {
                        return Err(plan_error(
                            n,
                            format!("configuration {} lacks base body NAIF {b}", c.label),
                            "keep the base bodies in every configuration",
                        ));
                    }
                }
            }
        }
        for k in &self.combinations {
            if k.terms.is_empty() {
                return Err(plan_error(
                    n,
                    format!("combination {} is empty", k.label),
                    "give it terms",
                ));
            }
            for (_, l) in &k.terms {
                if !seen.contains(l.as_str()) {
                    return Err(plan_error(
                        n,
                        format!("combination {} names unknown configuration {l}", k.label),
                        "name a configuration of the plan",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Number of legs over all configurations.
    #[must_use]
    pub fn leg_count(&self) -> usize {
        self.configs.iter().map(|c| c.legs.len()).sum()
    }
}
