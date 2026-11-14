<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Contributing to FerroAstro

Thank you for helping build an open, verifiable astrodynamics stack.

## Sign-off (DCO)
Every commit must carry a [Developer Certificate of Origin](https://developercertificate.org/) sign-off:
```
Signed-off-by: Your Name <you@example.org>
```
Use `git commit -s`. For commits written by AI agents, the human or organisation operating the agent signs off
and is responsible for the contribution. Commits must also be signed (SSH or OpenPGP) with a key registered in
`.github/signing/`, whose delegation record names the operator in the `Signed-off-by` (ORG-008, ADR-0015).

## How PRs merge
A PR merges when every required check passes (ORG-009). A maintainer's approval is needed only if it touches the
files listed in `.github/CODEOWNERS` (ORG-011) or the `ratchet` check reports a weakening of the evidence (TST-006).
A maintainer can stop any merge with the `hold` label. A Contributor License Agreement administered by the Proper-Time Physics
Institute will replace or supplement the DCO after legal review (ADR-0005).

## Before you open a PR
- Run every command in [AGENTS.md](AGENTS.md#commands-all-must-pass-before-a-pr).
- Cite the requirement(s) your change verifies (`Verifies: <ID>`), and add or update requirement YAML if needed.
- Physical values come from `tests/vectors/*.toml` with full provenance (ADR-0004).
- Significant design choices get an ADR in `docs/adr/`.
- PR titles follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) (`feat:`, `fix:`, `docs:` …).

## Clean-room policy
Integrators and force models are implemented from published papers. Do not consult GPL-licensed REBOUND,
REBOUNDx, ASSIST or Swiftest source for implementation work. Running them as CI oracles in separate containers is fine.

## Licensing
Contributions are licensed under Apache-2.0. New files need SPDX headers (`reuse lint`).
