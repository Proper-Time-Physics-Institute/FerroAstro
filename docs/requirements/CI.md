<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# CI — Oracle containers, goldens, job tiers and benchmarking

**Scope.** These requirements define how continuous integration runs FerroAstro against its oracles (REF-*). That covers one pinned Docker image per reference, a single tool-agnostic scenario format, golden artifacts with provenance, the job tiers (PR-fast, PR-label `oracle`, Nightly, Weekly/release), the shared precession estimator, and speed-benchmark hygiene. Source: Journal #3 (`docs/strategy/journal/03-reference-implementations-ci.md`), as revised by Journal #6 (`06-maintainer-decisions.md`). **Revised 2026-10-15 (Journal #10):** long runs (full nightly oracles, Stage D, multi-millennia) and benchmarks run on the **maintainer's local machine for now**. GitHub-hosted runners do PR-fast work and a short nightly. Long runs still checkpoint and resume (CI-010) so they can move to hosted runners later. The code-quality gates (fmt, clippy, docs, ruff, reuse) are in [STD.md](STD.md).

---

### CI-001 — One pinned Docker image per reference

```yaml
id: CI-001
title: "One fully pinned Docker image per reference, referenced by digest"
status: draft
criticality: medium
verification: inspection
statement: >-
  There SHALL be one Docker image per reference under ci/oracles/<tool>/. Base
  images SHALL be pinned by digest, packages by exact version, and sources by
  git SHA or tarball SHA-256. Test workflows SHALL reference images by digest.
acceptance: >-
  Every Dockerfile under ci/oracles/ uses a FROM …@sha256: base, == package
  pins and SHA/hash-pinned sources. Every test workflow references images by
  digest. Images are pushed to a private GHCR registry (never public, LIC-002)
  tagged <tool>-<version>-<yyyymmdd>, or rebuilt with layer caching in CI.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 and CI-001.
parent: null
related: [REF-005, REF-006, REF-007, REF-009, LIC-002, LIC-005]
notes: >-
  Images: jpl, orekit, rebound (GPL, isolated), swiftest (GPL, isolated),
  fortran (isolated), erfa.
```

### CI-002 — Single tool-agnostic scenario spec

```yaml
id: CI-002
title: "One scenario spec (ci/scenarios/*.toml) drives FerroAstro and every oracle"
status: draft
criticality: medium
verification: test
statement: >-
  A single tool-agnostic scenario spec (ci/scenarios/*.toml) SHALL drive
  FerroAstro and all oracle runners. It names the bodies, initial-condition
  source, GM set, force-model switches, integrator, tolerance/step, span and
  output cadence.
acceptance: >-
  FerroAstro and each oracle runner consume the same scenario file; a schema
  check validates every scenario file in CI.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 and CI-002.
parent: null
related: [REF-011, CI-003]
notes: >-
  Force switches: newton, sun_j2, gr_1pn_central, gr_eih, ppn{beta,gamma},
  lense_thirring, asteroids:N. IC sources: de440-header | de440-bsp@JD |
  horizons@JD.
```

### CI-003 — Golden provenance manifest

```yaml
id: CI-003
title: "Every golden output carries a MANIFEST.json provenance record"
status: draft
criticality: high
verification: test
statement: >-
  Golden outputs SHALL carry a MANIFEST.json provenance record: tool, version,
  image digest, runner SHA, input SHA-256s, CPU, wall-clock, output SHA-256,
  and units/frame/time scale.
acceptance: >-
  A CI check validates that every file under ci/golden/ has a MANIFEST.json
  entry with all listed fields non-empty and that output_sha256 matches the
  file.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 and CI-003.
parent: null
related: [TST-003, PHY-015, CI-004]
notes: >-
  Large goldens go to release assets or OCI artifacts; small CSVs stay in the
  repo.
```

### CI-004 — Goldens regenerated only deliberately

```yaml
id: CI-004
title: "Goldens regenerated only by explicit dispatch or nightly drift job; changes need a reviewed PR"
status: draft
criticality: high
verification: inspection
statement: >-
  Goldens SHALL be (re)generated only by explicit workflow_dispatch or by the
  nightly drift job. Changing a golden SHALL require a reviewed PR.
acceptance: >-
  No workflow other than the dispatch and nightly-drift workflows writes to
  ci/golden/; branch rules require review for changes there.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 and CI-004.
parent: null
related: [TST-004, CI-003]
notes: null
```

### CI-005 — PR-fast tier

```yaml
id: CI-005
title: "Per-PR CI compares only against cached goldens and finishes in under 10 min"
status: draft
criticality: medium
verification: test
statement: >-
  Per-PR CI SHALL compare only against cached goldens, without starting oracle
  containers, and SHALL finish in under 10 min. Oracle re-runs on PRs SHALL be
  opt-in via the `oracle` label (budget < 45 min).
acceptance: >-
  The PR-fast workflow starts no oracle container and its wall-clock is
  < 10 min. Content: testpo.440 points, jplephem states at ~50 epochs,
  2-body/Schwarzschild analytic precession (42.98 ″/cy class), 100-yr Sun +
  8 planets Newtonian versus a REBOUND-IAS15 golden, ERFA time/frame goldens.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 job tiers and
  CI-005.
parent: null
related: [STD-008, REF-004, REF-009]
notes: null
```

### CI-006 — Nightly tier

```yaml
id: CI-006
title: "Nightly CI runs every oracle on every scenario, rebuilds the speed table and flags drift"
status: draft
criticality: medium
verification: test
statement: >-
  Nightly CI SHALL run every oracle on every scenario (stages A–C, 1000-yr
  spans), regenerate the speed table on the fixed runner class (CI-008), and
  flag golden drift.
acceptance: >-
  A re-run oracle reproduces its golden bit-for-bit or within its stated
  tolerance; otherwise the nightly run flags drift. The speed table artifact is
  produced each night.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 job tiers and
  CI-006; docs/strategy/journal/06-maintainer-decisions.md.
parent: null
related: [CI-008, CI-010, STD-008]
notes: >-
  REVISED 2026-10-15 (Journal #10): a short nightly runs on GitHub-hosted
  runners; the full all-oracle / 1000-yr nightly runs on the maintainer's local
  machine for now, with results uploaded as artifacts with manifests (CI-003).
  Supersedes the Journal #6 larger-runner plan.
```

### CI-007 — Weekly/release Stage D DE440 reproduction

```yaml
id: CI-007
title: "Weekly/release CI runs full DE440 reproduction (Stage D) over 1550–2650 and publishes a report"
status: draft
criticality: high
verification: demonstration
statement: >-
  Weekly or release CI SHALL run the full DE440 reproduction (Stage D: 343
  asteroids + KBOs + ring, EIH, Sun J2 + LT) over 1550–2650 against de440.bsp
  and SHALL publish an error report with Mercury position residuals and the
  precession residual in ″/cy as a release asset.
acceptance: >-
  The weekly/tagged workflow completes (chained per CI-010) and attaches the
  report with Mercury position residuals and the ″/cy precession residual.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 job tiers and
  CI-007; docs/strategy/journal/06-maintainer-decisions.md.
parent: null
related: [PHY-014, REF-001, CI-010]
notes: >-
  Runs on the maintainer's local machine for now (Journal #10, 2026-10-15),
  superseding the Journal #6 larger-runner plan; the report is uploaded with
  its manifest. No residual pass
  threshold was set in the journals; PHY-014 sets the precession acceptance.
  Planet orbits in DE440 and DE441 differ by < 1 m over 1550–2650.
```

### CI-008 — Benchmark hygiene on a fixed larger-runner class

```yaml
id: CI-008
title: "Benchmarks on one fixed, recorded machine, median of ≥ 5 runs, same-run comparison only"
status: draft
criticality: medium
verification: test
statement: >-
  Benchmarks SHALL run on one fixed, recorded machine (currently the
  maintainer's local machine; Journal #10). Speed
  results SHALL be reported as the median of ≥ 5 runs with CPU model and flags,
  and SHALL be compared only within the same run and runner class.
acceptance: >-
  The speed table records the runner class, CPU model, CPU flags (e.g. AVX512)
  and the median of ≥ 5 runs per entry. No comparison spans different runs or
  runner classes.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 and CI-008, as
  refined by docs/strategy/journal/06-maintainer-decisions.md.
parent: null
related: [NUM-024, DEP-014, CI-006]
notes: >-
  REVISED by Journal #10 (2026-10-15): local machine instead of the Journal #6
  larger-runner class. Journal #5's bench.yml
  said "self-hosted or larger pinned runner" with trends informational, not
  gating; that is consistent with this refinement. Run-to-run variance on
  shared runners is UNVERIFIED.
```

### CI-009 — Same estimator for every tool

```yaml
id: CI-009
title: "Same precession estimator for every tool; GR isolated as GR-on minus GR-off"
status: draft
criticality: high
verification: test
statement: >-
  Mercury precession SHALL be computed with the same estimator for every tool,
  applied by FerroAstro's tooling to each tool's state output. The GR
  contribution SHALL be isolated as a GR-on minus GR-off difference from
  identical initial conditions.
acceptance: >-
  The comparison report lists one estimator ID for all tools. Each GR
  contribution is the difference of two runs whose initial-condition hashes are
  identical.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 and CI-009.
parent: null
related: [NUM-011, NUM-012, NUM-013, PHY-012]
notes: >-
  CONFLICT resolved: Journal #3 named the estimator as an LRL-vector longitude
  fit over a fixed number of orbits. The Journal #4 lead review made the
  Park2017 estimator (NUM-011) the canonical observable. NUM-011 is
  authoritative for reported rates; the LRL-vector estimator (NUM-012) is kept
  as a secondary cross-tool estimator. The "same estimator for all tools" rule
  holds either way.
```

### CI-010 — Checkpoint and resume across the 6 h job cap

```yaml
id: CI-010
title: "Long jobs checkpoint and resume across chained jobs within the 6 h per-job limit"
status: draft
criticality: medium
verification: test
statement: >-
  Long CI jobs (nightly, Stage D and multi-millennia runs) SHALL checkpoint
  their state and resume in a follow-on job or session, so that runs survive
  interruption on the local machine (Journal #10) and can later move to
  GitHub-hosted runners, which cap each job at 6 h.
acceptance: >-
  A short scenario interrupted at a checkpoint and
  resumed in a new job produces output identical to an uninterrupted run on the
  same runner class.
source: >-
  docs/strategy/journal/06-maintainer-decisions.md (new CI-010);
  docs/strategy/journal/03-reference-implementations-ci.md lead review notes.
parent: CI-007
related: [CI-006, CI-008, TST-005, NUM-021]
notes: >-
  NEW in Journal #6. The bitwise-identity acceptance is derived from the
  determinism requirements (TST-005, NUM-021); Journal #6 itself gives no
  numeric criterion.
```
