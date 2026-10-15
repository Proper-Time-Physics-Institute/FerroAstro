#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Host driver for the oracle containers (CI-001, CI-003, REF-005, REF-009, REF-001; LIC-001/002).
#
#   tools/oracles/run_oracles.sh build   <rebound|swiftest|mercury6|orekit|erfa|jpl>
#   tools/oracles/run_oracles.sh <rebound|swiftest|mercury6> <scenario.toml> <out-dir>
#   tools/oracles/run_oracles.sh orekit  <scenario.toml> <out-dir>
#   tools/oracles/run_oracles.sh erfa    <out-dir>
#   tools/oracles/run_oracles.sh jpl     <out-dir>
#
# Rules this script enforces:
# * Images are built locally from a staged context that holds ONLY ci/oracles/<tool>/ and
#   tools/oracles/*.py (plus tools/goldens/erfa_*.py for erfa). The staging step refuses any
#   wheel, shared object, Cargo manifest or target directory, so no FerroAstro build can enter a
#   (GPL) oracle image (red-team L4). FerroAstro is never built inside a container.
# * Images are tagged fa-oracle-<tool>:local-<yyyymmdd> and NEVER pushed: this script has no push,
#   and the GPL images must not reach any registry (LIC-002, red-team L3; maintainer decision).
# * Containers run as the calling user, with the inputs mounted read-only and only the output
#   directory writable; rebound and erfa run with --network none, jpl needs the network to fetch.
# * The image digest (docker image Id) and the runner commit are passed in FA_ORACLE_IMAGE_DIGEST
#   and FA_ORACLE_RUNNER_SHA, so every MANIFEST.json entry is complete (CI-003).
# Environment: FA_DATA_DIR (default target/fa-data) holds header.440 and the other JPL products.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
data="${FA_DATA_DIR:-$root/target/fa-data}"

usage() { sed -n '6,11p' "${BASH_SOURCE[0]}" | sed 's/^# *//' >&2; exit 2; }

runner_sha() {
  local sha
  sha="$(git rev-parse HEAD)"
  if [[ -n "$(git status --porcelain -- tools/oracles tools/goldens ci/oracles)" ]]; then
    sha="${sha}-dirty"
  fi
  echo "$sha"
}

image_tag() { echo "fa-oracle-$1:local-$(date -u +%Y%m%d)"; }

build() {
  local tool="$1" ctx
  [[ -f "ci/oracles/$tool/Dockerfile" ]] || { echo "no ci/oracles/$tool/Dockerfile" >&2; exit 2; }
  ctx="$(mktemp -d)"
  trap 'rm -rf "$ctx"' RETURN
  cp ci/oracles/"$tool"/* "$ctx"/
  mkdir -p "$ctx/oracles"
  cp tools/oracles/*.py "$ctx/oracles/"
  if [[ "$tool" == erfa ]]; then
    mkdir -p "$ctx/goldens"
    cp tools/goldens/erfa_time.py tools/goldens/erfa_frames.py "$ctx/goldens/"
  fi
  # Red-team L4: nothing built by FerroAstro may enter the context.
  if find "$ctx" \( -name '*.whl' -o -name '*.so' -o -name '*.rlib' -o -name 'Cargo.toml' \
       -o -name target -o -iname '*ferroastro*' \) | grep -q .; then
    echo "refusing to build: the staged context contains a build artefact" >&2
    find "$ctx" >&2
    exit 1
  fi
  echo "## staged context for $tool:" >&2
  (cd "$ctx" && find . -type f | sort | sed 's/^/   /') >&2
  docker build --pull=false -t "$(image_tag "$tool")" "$ctx" >&2
  docker image inspect --format '{{.Id}}' "$(image_tag "$tool")"
}

run_container() {
  local tool="$1"; shift
  local digest
  digest="$(build "$tool")"
  echo "## image $(image_tag "$tool") ${digest}" >&2
  docker run --rm --user "$(id -u):$(id -g)" \
    -e FA_ORACLE_IMAGE_DIGEST="$digest" -e FA_ORACLE_RUNNER_SHA="$(runner_sha)" \
    "$@"
}

cmd="${1:-}"; shift || true
case "$cmd" in
  build)
    [[ $# == 1 ]] || usage
    build "$1"
    ;;
  rebound|swiftest|mercury6)
    [[ $# == 2 ]] || usage
    scenario="$(realpath "$1")"; out="$2"
    mkdir -p "$out"; out="$(realpath "$out")"
    run_container "$cmd" --network none \
      -v "$scenario:/in/$(basename "$scenario"):ro" \
      -v "$data/header.440:/in/header.440:ro" \
      -v "$out:/out" \
      "$(image_tag "$cmd")" \
      --scenario "/in/$(basename "$scenario")" --header /in/header.440 --out-dir /out
    ;;
  erfa)
    [[ $# == 1 ]] || usage
    out="$1"; mkdir -p "$out"; out="$(realpath "$out")"
    run_container erfa --network none -v "$out:/out" "$(image_tag erfa)" --out-dir /out
    ;;
  jpl)
    [[ $# == 1 ]] || usage
    out="$1"; mkdir -p "$out" "$data"; out="$(realpath "$out")"
    run_container jpl \
      -v "$root/tests/data/MANIFEST.toml:/in/MANIFEST.toml:ro" \
      -v "$(realpath "$data"):/cache" \
      -v "$out:/out" \
      "$(image_tag jpl)" --manifest /in/MANIFEST.toml --cache /cache --out-dir /out
    ;;
  *) usage ;;
esac
