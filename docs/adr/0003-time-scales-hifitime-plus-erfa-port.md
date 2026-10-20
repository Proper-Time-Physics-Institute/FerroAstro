---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Time scales: hifitime plus an ERFA port

## Decision Outcome
Use `hifitime` 4.x (MPL-2.0, unmodified dependency, which supports TAI/TT/TDB/TCB/TCG/UTC) for civil time and leap seconds. Port the needed TT↔TDB↔TCB↔TCG and frame routines from **ERFA (BSD-3)**, never SOFA, with no `iau` prefix. Ported files carry an SPDX license-identifier tag of `Apache-2.0 AND BSD-3-Clause`, the ERFA copyright and the BSD-3 disclaimer, with attribution also in NOTICE (DEP-009). Internally propagate TDB seconds in `Real`. `sofars` serves only as a test oracle until its SOFA-terms status is legally cleared. `anise` is not a dependency; whether to adopt it is deferred to M5 (OI-16, OI-72: its graph adds BlueOak-1.0.0 and CDLA-Permissive-2.0 licences, a RustSec advisory and build-time network access). The MPL-2.0 exception list is {hifitime}.

### Confirmation
Cross-validation against `hifitime` and `pyerfa` 2.0.1.5 goldens (DEP-009/010, REF-009). `cargo-deny` MPL exception list (DEP-002).

