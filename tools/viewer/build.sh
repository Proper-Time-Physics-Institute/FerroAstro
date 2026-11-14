#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Build the orbit viewer's wasm module (release, wasm32-unknown-unknown), copy it next to the web
# page (crates/ferroastro-viewer/web/ferroastro_viewer.wasm, git-ignored: a build artefact) and run
# both node smoke tests: the engine-level one (tools/viewer/smoke.mjs, raw ABI) and the page-level
# one (crates/ferroastro-viewer/web/smoke.mjs, through fav.js). docs/viewer/DESIGN.md section 9.
#
#   tools/viewer/build.sh            build, copy, smoke
#   tools/viewer/build.sh --no-smoke build and copy only
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
web="$root/crates/ferroastro-viewer/web"
cd "$root"
# Cargo's target directory (CARGO_TARGET_DIR or a build.target-dir config may move it).
target_dir=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
target_dir=${target_dir:-$root/target}
# System mode needs the JPL header.440, which the `embed-header` feature includes at build time from
# the data cache ($FA_DATA_DIR, else <target>/fa-data), SHA-256 checked against
# tests/data/MANIFEST.toml (docs/viewer/DESIGN.md section 5). It is never committed (#22): fetch it
# once with `(cd tools && uv run python fetch_jpl.py de440-header)` or point FA_DATA_DIR at a
# directory that holds it. The build fails with FA-E0303 if it is missing.
cargo build -p ferroastro-viewer --release --target wasm32-unknown-unknown --features embed-header
cp "$target_dir/wasm32-unknown-unknown/release/ferroastro_viewer.wasm" "$web/ferroastro_viewer.wasm"
echo "copied ferroastro_viewer.wasm ($(wc -c < "$web/ferroastro_viewer.wasm") bytes) to $web"

if [ "${1:-}" != "--no-smoke" ]; then
  node "$root/tools/viewer/smoke.mjs" "$web/ferroastro_viewer.wasm"
  if [ -f "$web/smoke.mjs" ]; then
    node "$web/smoke.mjs" "$web/ferroastro_viewer.wasm"
  fi
fi
