<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# FerroAstro

**Agentic-first astrodynamics in Rust.** FerroAstro rebuilds the Fortran/Java scientific-computing stack for
astrodynamics as an open, verifiable Rust ecosystem covering Newtonian dynamics, general relativity and proper time.
It is benchmarked in CI against Orekit, REBOUND/REBOUNDx, Swiftest, Mercury6, ERFA and JPL DE440.

**Flagship:** the Mercury perihelion precession ladder, which goes from Newtonian 2-body (0″/cy) through every
planet, solar J2, special relativity and Schwarzschild GR (42.98″/cy) to the full 1PN N-body total of
**575.31″/cy** (Park et al. 2017), and finally a faster reproduction of JPL DE440.

> Status: **pre-M0**. Strategy, requirements and decisions are in place, and code starts at milestone M0.

| Start here | |
|---|---|
| [Mission](docs/strategy/MISSION.md) | Goals, the ladder, non-goals |
| [Roadmap](docs/strategy/ROADMAP.md) | Milestones M0–M6 |
| [Architecture](docs/strategy/ARCHITECTURE.md) | Crate layering and verification tiers |
| [Requirements](docs/requirements/README.md) | Traceable requirement catalogue |
| [ADRs](docs/adr/README.md) | Decisions |
| [Standards](docs/standards/README.md) | Rust, Python and testing standards |
| [AGENTS.md](AGENTS.md) | Instructions for AI agents and contributors |

Licensed under [Apache-2.0](LICENSE). © 2026 Proper-Time Physics Institute. See [CONTRIBUTING](CONTRIBUTING.md).
