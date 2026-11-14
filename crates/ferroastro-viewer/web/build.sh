#!/usr/bin/env sh
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Builds the viewer engine for wasm32-unknown-unknown and copies ferroastro_viewer.wasm next to
# index.html (docs/viewer/DESIGN.md section 9). The .wasm is a build artefact; it is git-ignored.
#
#   crates/ferroastro-viewer/web/build.sh            build and copy
#   crates/ferroastro-viewer/web/build.sh --smoke    also run the node smoke test and the glossary check
#   crates/ferroastro-viewer/web/build.sh --serve    also serve this directory on http://localhost:8000/
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
cd "$root"
# Ask cargo where its target directory is (CARGO_TARGET_DIR or a build.target-dir config may move it).
target_dir=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
target_dir=${target_dir:-$root/target}
# System mode needs the JPL header.440, which the `embed-header` feature includes at build time from
# the data cache ($FA_DATA_DIR, else <target>/fa-data), SHA-256 checked against
# tests/data/MANIFEST.toml (docs/viewer/DESIGN.md section 5). It is never committed (#22): fetch it
# once with `(cd tools && uv run python fetch_jpl.py de440-header)` or point FA_DATA_DIR at a
# directory that holds it. The build fails with FA-E0303 if it is missing.
cargo build -p ferroastro-viewer --release --target wasm32-unknown-unknown --features embed-header
cp "$target_dir/wasm32-unknown-unknown/release/ferroastro_viewer.wasm" "$here/ferroastro_viewer.wasm"
echo "copied ferroastro_viewer.wasm ($(wc -c < "$here/ferroastro_viewer.wasm") bytes) to $here"

for arg in "$@"; do
  case "$arg" in
    --smoke) node "$here/smoke.mjs" "$here/ferroastro_viewer.wasm" && node "$here/check-glossary.mjs" ;;
    --serve)
      echo "serving $here on http://localhost:${PORT:-8000}/ (Ctrl-C to stop)"
      cd "$here" && exec python3 -m http.server "${PORT:-8000}" --bind 127.0.0.1
      ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done
