---
status: accepted (revised 2026-10-15)
date: 2026-10-14
decision-makers: maintainers (decision recorded in Journal #6)
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Benchmark and long-run infrastructure

## Decision Outcome
Benchmarks, nightly oracle runs and DE440 reproduction run on **GitHub paid larger runners** of a single fixed class. Speed results are the median of at least 5 runs with CPU model and flags recorded, and are compared only within the same workflow run. GitHub-hosted jobs are capped at 6 h, so long integrations **checkpoint state (with integrator history) and resume across chained jobs**, with bit-identical continuation tested.

### Confirmation
CI-008, CI-010. A resume test proves that a checkpointed run equals the uninterrupted run bit-for-bit (f64 and DD).

## Revision 2026-10-15 (Journal #10)
The maintainer moved long runs (the full nightly oracle set, Stage D and multi-millennia runs) and benchmarks to the **maintainer's local machine for now**. GitHub-hosted runners keep the PR-fast tier and a short nightly. Checkpoint/resume (CI-010) is kept, so the runs can move to hosted larger runners later without rework. Benchmark numbers are compared only within the same machine and session.
