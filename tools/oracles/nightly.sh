#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Nightly all-oracle job (CI-006, CI-009; WP17). Runs on the local machine (maintainer decision
# 2026-10-14: nightly and long tiers run locally; GitHub runners do PR-fast work), e.g. from cron:
#   17 2 * * *  cd /path/to/FerroAstro && tools/oracles/nightly.sh --open-issue >> target/nightly.log 2>&1
#
#   tools/oracles/nightly.sh [--open-issue] [--skip-rust]
#
# 1. Every N-body oracle (rebound, swiftest, mercury6) on every ci/scenarios/*.toml: a scenario a
#    runner does not implement is refused (exit 3, REF-011) and recorded as such; plus the erfa
#    and jpl images. Outputs: $FA_ORACLE_OUT_DIR/<tool>/<scenario>/ (default target/oracle-out),
#    plain files with MANIFEST.json. Images are built locally and never pushed (LIC-002); no
#    FerroAstro build enters them (run_oracles.sh stages the contexts).
# 2. Drift (tools/oracles/drift.py): every committed ci/golden/<tool>/<name>/ and the ERFA goldens
#    must be reproduced bit for bit. Drift writes a report and opens a BLOCKING issue
#    (labels golden-drift, blocking) when --open-issue is given; without it, a dry run.
# 3. The FerroAstro side (separate process, no container): the nightly oracle differentials
#    (one Park2017 estimator on every tool's series, CI-009) read the outputs of step 1.
# 4. Speed-table stub: in the report (wall clock per tool and scenario, from the manifests).
#
# Exit status: 0 when every step passed and nothing drifted; 1 otherwise.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
out="${FA_ORACLE_OUT_DIR:-$root/target/oracle-out}"
export FA_ORACLE_OUT_DIR="$out"
open_issue=""
skip_rust=0
for a in "$@"; do
  case "$a" in
    --open-issue) open_issue="--open-issue" ;;
    --skip-rust) skip_rust=1 ;;
    *) echo "unknown argument $a" >&2; exit 2 ;;
  esac
done
mkdir -p "$out"
status="$out/status.tsv"
: > "$status"
fail=0
R=tools/oracles/run_oracles.sh

for tool in rebound swiftest mercury6 erfa jpl; do
  $R build "$tool" > "$out/build-$tool.log" 2>&1 || { echo -e "$tool\t(build)\t1\tbuild failed" >> "$status"; fail=1; }
done

for tool in rebound swiftest mercury6; do
  for s in ci/scenarios/*.toml; do
    name="$(basename "$s" .toml)"
    rm -rf "${out:?}/$tool/$name"
    log="$out/$tool-$name.log"
    $R "$tool" "$s" "$out/$tool/$name" > "$log" 2>&1
    code=$?
    note="$(grep -E '^(REFUSED|SETUP ERROR|wrote)' "$log" | head -n1 | cut -c1-160 | tr '\t|' '  ')"
    echo -e "$tool\t$name\t$code\t$note" >> "$status"
    if [[ $code == 3 ]]; then rm -rf "${out:?}/$tool/$name"; fi
    if [[ $code != 0 && $code != 3 ]]; then fail=1; fi
  done
done
$R erfa "$out/erfa/erfa-goldens" > "$out/erfa.log" 2>&1; code=$?
echo -e "erfa\terfa-goldens\t$code\t" >> "$status"; [[ $code == 0 ]] || fail=1
$R jpl "$out/jpl/ref001-pins" > "$out/jpl.log" 2>&1; code=$?
echo -e "jpl\tref001-pins\t$code\t" >> "$status"; [[ $code == 0 ]] || fail=1

(cd tools && uv run python -m oracles.drift --candidate "$out" --status "$status" \
  --report "$out/report.md" $open_issue) || fail=1

if [[ $skip_rust == 0 ]]; then
  # The FerroAstro side runs here, on the host, after the containers have exited.
  cargo nextest run -p ferroastro-oracle --cargo-profile release --profile nightly \
    --run-ignored only --no-capture > "$out/ferroastro-nightly.log" 2>&1 || fail=1
  grep -E 'P-add Venus|GR contribution|PASS|FAIL' "$out/ferroastro-nightly.log" | tail -n 40
fi
echo "nightly oracle job: $([[ $fail == 0 ]] && echo PASS || echo FAIL); report $out/report.md"
exit "$fail"
