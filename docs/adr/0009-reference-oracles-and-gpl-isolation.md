---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Reference oracles and GPL isolation

## Decision Outcome
CI oracles: Orekit 13.1.9 / Hipparchus 4.0.3 (Apache-2.0); REBOUND 5.2.2 + REBOUNDx 5.1.0 (GPL-3); Swiftest 2026.7.1 (GPL-3+); Mercury6 (legacy, Newtonian only); pyerfa 2.0.1.5; jplephem 2.24; and JPL DE440 as truth. Each runs in its own pinned container under `ci/oracles/` and exchanges **only plain files** with FerroAstro. GPL code is never linked, bound via FFI, compiled by `build.rs` or vendored (not even under `ci/oracles/`, which holds only our own scripts that install GPL software at image build time). GPL oracle images are never published publicly or as release artifacts: they live in a private GHCR registry or are rebuilt (cached) in CI (maintainer decision 2026-10-14). A GPL container never contains a FerroAstro build; oracle and FerroAstro runs are separate jobs that exchange files (LIC-005). JPL, SOFA and NOVAS files are fetched with SHA-256 pins and never committed. (Engineering guidance based on the GPL FAQ, not legal advice.)

### Confirmation
`cargo-deny` bans (DEP-003). Code review of `ci/oracles/` (LIC-001…003, LIC-005). GHCR oracle package visibility is private (LIC-002).

