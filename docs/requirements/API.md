<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# API — Public library API

**Scope.** These requirements shape FerroAstro's public Rust API so that physics mistakes fail early and loudly. Units, frames and time scales are encoded in types, so a mismatch is a compile error. Invalid input produces a typed error with a stable code instead of a panic. Every public function carries a runnable, cited example. Source: Journal #5 §6 (`docs/strategy/journal/05-organization-standards.md`), with the API-001 design fixed by Journal #6 (`06-maintainer-decisions.md`): home-grown newtypes generic over the crate's `Real` scalar, and no `uom`.

---

### API-001 — Units, frames and time scales in the type system

```yaml
id: API-001
title: "Home-grown newtypes generic over Real make frame/time-scale/unit mismatches fail to compile"
status: draft
criticality: high
verification: test
statement: >-
  Physical quantities, time scales and reference frames SHALL be encoded as
  home-grown newtypes generic over the crate's `Real` scalar (e.g. Epoch<Tdb>,
  State<Icrf, Barycentric>, Length), such that frame, time-scale or unit
  mismatches fail to compile. The uom crate SHALL NOT be used.
acceptance: >-
  A trybuild compile-fail suite contains at least one case per mismatch class
  (frame, centre, time scale, unit) and all of them fail to compile with the
  expected error; the newtypes compile with both f64 and the DD backend.
  `uom` does not appear in Cargo.lock.
source: >-
  docs/strategy/journal/06-maintainer-decisions.md (API-001 design fixed;
  ADR-0001); docs/strategy/journal/05-organization-standards.md §6 and API-001.
parent: null
related: [NUM-006, DEP-007, DEP-010]
notes: >-
  REVISED by Journal #6. Journal #2's open question on whether uom accepts a
  custom DD storage type is moot. Journal #5's example spelled Length<Km>;
  Journal #6 spells Length. The unit-parameter design is left to ADR-0001.
```

### API-002 — No panics; typed errors with stable codes

```yaml
id: API-002
title: "Library code does not panic on invalid input; typed errors with stable FA-Exxx codes"
status: draft
criticality: medium
verification: test
statement: >-
  Library code SHALL NOT panic on invalid input. Errors SHALL be typed enums
  (thiserror) with stable variant names and codes (FA-Exxx), and messages
  SHALL name the offending quantity, its unit, the valid range and a
  remediation hint.
acceptance: >-
  clippy unwrap_used = deny passes; property tests feed out-of-range inputs to
  public functions and observe Err values, never panics; a test checks every
  error message contains a code, quantity, unit and range.
source: >-
  docs/strategy/journal/05-organization-standards.md §6 item 2 and API-002.
parent: null
related: [STD-002]
notes: >-
  Example: FA-E012: eccentricity 1.02 ≥ 1 for elliptic propagator; use
  HyperbolicPropagator.
```

### API-003 — Doctests and per-rung examples

```yaml
id: API-003
title: "Each public function has a doctest with real cited values; each rung has an examples/ program"
status: draft
criticality: low
verification: test
statement: >-
  Each public function SHALL have a doctest example using real, cited values
  (citing the vector ID), and each ladder rung SHALL have a runnable program in
  examples/ (e.g. cargo run --example mercury_1pn).
acceptance: >-
  `cargo test --doc` passes; a check confirms every public fn has a doc
  example and every PHY- rung has a matching example that runs in CI.
source: >-
  docs/strategy/journal/05-organization-standards.md §6 item 3 and API-003.
parent: null
related: [STD-004, ORG-002, AGT-001]
notes: null
```
