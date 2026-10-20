#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Local pre-push routine: the AGENTS.md command list plus the MSRV check that CI's `msrv` job
# runs (progress review after WP15, F8/F14). The `msrv` job failed on 15 consecutive pushes because
# no local routine compiled the workspace with the declared `rust-version`; this script does.
#
# Usage: tools/pre_push.sh [--quick]   (--quick skips the MSRV test run, keeping the MSRV check)
# Needs: the pinned toolchain (rust-toolchain.toml), toolchain 1.85.0 (`rustup toolchain install
# 1.85.0 --profile minimal`), cargo-nextest, cargo-deny, uv and reuse.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
quick=0
if [[ "${1:-}" == "--quick" ]]; then quick=1; fi

msrv="$(sed -n 's/^rust-version *= *"\([0-9.]*\)".*/\1/p' Cargo.toml | head -n1)"
if [[ -z "$msrv" ]]; then echo "no rust-version in Cargo.toml" >&2; exit 2; fi
# rustup wants a full x.y.z toolchain name.
if [[ "$msrv" =~ ^[0-9]+\.[0-9]+$ ]]; then msrv="${msrv}.0"; fi

step() { echo "## $*"; "$@"; }

step cargo fmt --all --check
step cargo clippy --workspace --all-targets --all-features -- -D warnings
step cargo nextest run --workspace
step cargo test --doc --workspace
step cargo deny check
RUSTDOCFLAGS="-D warnings" step cargo doc --no-deps --workspace
(cd tools && step uv run ruff check && step uv run ruff format --check && step uv run mypy . && step uv run pytest -q)
step reuse lint
(cd tools && step uv run python trace.py --check)
step cargo "+${msrv}" check --workspace --all-targets
if [[ "$quick" == 0 ]]; then
  # As CI's msrv job: the trybuild snapshots are specific to the pinned toolchain.
  step cargo "+${msrv}" nextest run --workspace -E 'not binary(compile_fail)'
fi
echo "pre-push routine: all steps passed (MSRV ${msrv})"
