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
  BUILD EVIDENCE (sync 2026-10-20; WP16, WP17 on m3-newtonian-nbody): six
  images exist, ci/oracles/{rebound,erfa,jpl,swiftest,mercury6,orekit}/Dockerfile (the "fortran" image is mercury6). Each uses a digest-pinned
  base (python:3.12-slim-bookworm@sha256:34386ef0…, or the Debian/OpenJDK
  equivalent for Orekit), == package pins with SHA-256 hashes, apt version
  pins, and ADD --checksum or SHA-256-pinned jars/tarballs for sources;
  tools/tests/test_oracles.py checks this pinning for every Dockerfile and
  requirements file. Images are built locally by tools/oracles/run_oracles.sh
  and tagged fa-oracle-<tool>:local-<yyyymmdd>; they are never pushed (no
  GHCR push anywhere), following the maintainer decision that GPL images stay
  local or CI-cache only (LIC-002 allows "rebuilt" instead of a registry).
  The image Id (digest) and the runner commit are recorded in every
  MANIFEST.json entry instead of a registry reference; a runner refuses to
  write a manifest without them. Whether recording the built image's digest
  satisfies "test workflows reference images by digest" is NEEDS HUMAN REVIEW
  (WP16 item 2); the acceptance text is unchanged. apt version pins stop
  resolving after a Debian point release (WP16 item 6).
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
  BUILD EVIDENCE (sync 2026-10-20; WP13, WP15, WP17, WP18 on
  m3-newtonian-nbody). Schema: ci/scenarios/scenario.schema.json is generated
  by ferroastro-scenario (schemars) and committed;
  crates/ferroastro-scenario/tests/ci_scenarios.rs fails if it drifts and
  validates every ci/scenarios/*.toml in Rust, and tools/validate_scenarios.py
  validates them against the committed schema in Python. Implemented
  vocabulary (schema_version 1): mode = n-body | test-particle |
  ephemeris-field (the last added in WP17 for Orekit's stage B: a massless
  particle in the DE440-driven field); [initial_conditions] source =
  de440-header | de440-bsp | horizons with epoch_jd_tdb; [forces] has the
  seven switches above plus two additions not in this list, kbos (the
  identified KBOs) and kbo_ring (the 36-point ring) (WP15); asteroids = N
  takes the first N numbered asteroids of header.440; [output] adds
  sample_bodies (record only the named bodies, WP15). New fields are skipped
  when unset, so existing scenario digests do not change. Integrator mapping:
  a scenario's [integrator] block describes FerroAstro's IAS15; the
  symplectic and legacy tools run tool-specific committed settings
  (ci/oracles/{swiftest,mercury6,orekit}/settings.toml, keyed by scenario
  name: Swiftest WHM dt 1/128 d, Mercury6 RA15 1e-12, Orekit DP853), and
  every MANIFEST entry records the mapping (WP17 item 1). Scenario files
  added by agents (sun-mercury-*, sun-8planets-*, phy012-*, stage-a-*,
  oracle-*, bench-*) and the new switch names, mode and integrator mapping
  are NEEDS HUMAN REVIEW (WP13 item 1, WP15 item 5, WP17 items 1 and 4, WP18).
  The ERFA and JPL runners take no scenario and refuse a scenario argument,
  since time/frame goldens and file pins have no force switches (WP16
  item 4).
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
notes: >-
  BUILD EVIDENCE on golden regeneration (sync 2026-10-20; WP8–WP10, WP15–WP17
  on m3-newtonian-nbody). Where goldens live: oracle-container outputs are
  under ci/golden/<tool>/<scenario>/ with MANIFEST.json (rebound, swiftest
  and mercury6 stage A, orekit stage B, jpl ref001-pins); reader and library
  goldens are under tests/goldens/ (erfa-time-pyerfa.csv, erfa-dat-pyerfa.csv,
  erfa-frames-pyerfa.csv, jplephem-de440.csv, jplephem-synthetic.csv,
  jplephem-sb441.csv, sb441-n373s-jdepoc-states.csv), each pinned by SHA-256
  in the notes of the vectors that cite it, so a regenerated file fails the
  tests until the vector changes. How they were generated: the committed
  oracle goldens were produced locally by tools/oracles/run_oracles.sh (the
  local equivalent of the dispatch workflow, following the maintainer decision
  that oracle and long runs execute locally) and entered through PR #13; the
  tests/goldens files come from tools/goldens/*.py and tools/sample_sb441.py
  run locally. .github/workflows/oracles.yml and oracle-nightly.yml exist but
  are workflow_dispatch only and had not run on GitHub as of WP18. Whether a
  locally generated golden satisfies this requirement's "only by explicit
  workflow_dispatch or by the nightly drift job" is NEEDS HUMAN REVIEW (WP16
  item 3). A PR test refuses a committed golden whose manifest records a
  dirty runner tree (WP16 item 7). The 100-MB P-add oracle series are not
  committed; only their manifests are verified (WP17 item 8).
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
notes: >-
  BUILD EVIDENCE (sync 2026-10-20; WP10, WP11 on m3-newtonian-nbody). The
  PR tier reads pinned JPL products (de440.bsp, header.440, testpo.440)
  from a local data cache: in CI the rust and msrv jobs restore .fa-data with
  actions/cache (pinned by SHA, key = hash of tests/data/MANIFEST.toml) and run
  tools/fetch_jpl.py, which downloads nothing on a cache hit; locally the
  cache is $FA_DATA_DIR or <target>/fa-data, filled once with
  (cd tools && uv run python fetch_jpl.py). A fresh clone without the cache
  fails loudly (FA-E0204 naming that command) rather than skipping. On a
  cache miss the CI jobs download about 120 MB from JPL. This changes the
  developer contract and is NEEDS HUMAN REVIEW (WP10 deviation 4). Every
  cached file is re-verified against its pin on load, and a substituted file
  is refused (crates/ferroastro-testdata/tests/ci005_cache_verification.rs,
  tools/tests/test_fetch_jpl.py; WP11 correction F2). No PR-tier test starts
  an oracle container; stage A/B oracle goldens are compared from the
  committed files (crates/ferroastro-oracle/tests/stage_a.rs, stage_b.rs).
  The < 10 min wall-clock has not been measured as a requirement check.
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
  BUILD EVIDENCE (sync 2026-10-20; WP17, WP18 on m3-newtonian-nbody).
  Mechanism: tools/oracles/nightly.sh (local, meant for cron) re-runs every
  N-body oracle on every scenario its runner accepts (refusals recorded), the
  ERFA and JPL images, the drift check tools/oracles/drift.py (every
  committed golden bit for bit; JSON goldens without run-date keys; a missing
  candidate counts as drift) and FerroAstro's nightly differentials.
  "Flag drift" is implemented as a blocking issue: with --open-issue the job
  runs gh issue create with labels golden-drift and blocking; without it the
  issue is a dry run. The labels do not exist in the repository yet, and
  whether an open drift issue blocks merges is a merge-policy question left
  to humans (WP17 item 6). .github/workflows/oracle-nightly.yml runs the same
  on manual dispatch, with the drift check in a separate files-only job. The
  speed table is a stub in the nightly report (wall clock and CPU per tool
  from the manifests; informational, WP17 item 7); the full table is produced
  by tools/bench/speed_table.sh (WP18, CI-008). Status: the all-oracle nightly
  has never completed end to end; three local runs stopped inside Swiftest's
  2000-yr P-add run, and no cron entry exists (progress review after WP18,
  R3), so this acceptance is not yet met.
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
  BUILD EVIDENCE (sync 2026-10-20; WP13, WP14, WP15 on m3-newtonian-nbody).
  "Resumed in a new job" is tested as "resumed in a new process" on the same
  machine, through ferroastro scenario run|resume
  (crates/ferroastro-cli/tests/cli_scenario.rs::checkpoint_resume_in_a_fresh_process_is_bitwise_{f64,double_double}, and
  the nightly 1000-yr runs interrupted at 500 yr, f64 and double-double):
  0 differing 64-bit words over every sample, passage and the final state,
  with equal step counts. A new process is the weaker of the two readings
  (same machine, no artifact hand-off); chaining across CI jobs by uploading
  the checkpoint as an artifact is not built yet and is left to the first
  workflow that needs it (WP13 deviation 2). The checkpoint (Ias15Snapshot
  as exact (hi, lo) binary64 pairs plus driver state, postcard-encoded, tied
  to the scenario's SHA-256) refuses a foreign scenario with FA-E1302. A run
  stops after the first accepted step past the requested time and never
  shortens a step, so interrupted and uninterrupted runs take the same steps.
  The diffrun harness runs legs in 50-yr segments with an on-disk store
  (FA-E1402 for a foreign store), and the WP15 PHY-013 nightly resumed two
  legs from that store after a 30-min timeout (WP15). Test-particle mode has
  no checkpoints (FA-E1302).
```
