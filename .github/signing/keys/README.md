<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# OpenPGP signing keys

ASCII-armoured public keys (`*.asc`) of registered OpenPGP commit-signing keys (ORG-008, ADR-0015).
Each key needs a `[[delegation]]` in [`../delegations.toml`](../delegations.toml) with its 40-hex
fingerprint. SSH keys go in [`../allowed_signers`](../allowed_signers) instead.
