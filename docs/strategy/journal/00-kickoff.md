## 📓 Journal #0 — Kickoff (2026-10-14)

This issue is the running research journal for the FerroAstro scientific-computing strategy. Each entry records findings and the **build-out requirements** they imply. A consolidated plan will land as markdown under `docs/strategy/` on branch `1-scientific-computing-strategy`.

### Decisions so far
| Area | Decision |
|---|---|
| License | Apache-2.0, © Proper-Time Physics Institute, contributions under an org CLA |
| Flagship problem | **Mercury perihelion precession ladder**, where each rung is a tested requirement with a cited expected value |
| Newtonian rungs | 2-body (zero precession) → +planets → +Moon/asteroids → +solar J2 → DE440-equivalent (reproduce from published ICs, faster; fitting to observations is a later stage) |
| Relativistic rungs | SR two ways, with caveats on why they differ: SR-Kepler Lagrangian (~7″/cy) and velocity-only 1PN terms → Schwarzschild test particle → 1PN Sun-only → full EIH N-body (PPN β, γ free) |
| Truth data | Modern ephemeris fits (DE440, INPOP, EPM, MESSENGER) + JPL Horizons elements |
| CI benchmarks | Orekit (Java), a Fortran N-body integrator, REBOUND/REBOUNDx; DE440 as accuracy truth |
| Precision | f64, plus extended precision for multi-millennia runs |
| Showcase | CLI + generated report, WASM web demo, MCP server for agents |
| Dependencies | Maximally FOSS; reuse mature crates where the license is Apache-2.0-compatible |
| CI | GitHub Actions + Docker images for reference codes |

### Research streams (running in parallel)
1. **Physics truth data**: per-rung expected precession values with primary citations
2. **Rust ecosystem & license audit**: crates for numerics, time, units, extended precision, MCP, WASM
3. **Reference implementations**: Orekit, Fortran N-body, REBOUND/REBOUNDx, DE440/Horizons; licenses and CI containerization
4. **Numerical methods**: long-term integrators, extended precision, how to *measure* precession from a trajectory
5. **Organization & engineering standards**: requirements/traceability structure, coding standards (rustfmt/clippy for Rust; **ruff** for all Python tooling), license gating, CLA, agent-facing conventions
