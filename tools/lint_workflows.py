# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""GitHub Actions hygiene checks (STD-012, red-team §2 control 2).

* Every third-party ``uses:`` reference is pinned to a full 40-hex commit SHA.
* No workflow uses the ``pull_request_target`` trigger.
* Every workflow declares a top-level ``permissions:`` block.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
USES_RE = re.compile(r"^\s*(?:-\s*)?uses:\s*['\"]?(?P<ref>[^\s'\"#]+)")
SHA_RE = re.compile(r"@[0-9a-f]{40}$")


def lint_text(name: str, text: str) -> list[str]:
    """Return findings for one workflow file."""
    findings: list[str] = []
    for n, line in enumerate(text.splitlines(), 1):
        m = USES_RE.match(line)
        if m:
            ref = m.group("ref")
            local = ref.startswith(("./", "docker://"))
            if not local and not SHA_RE.search(ref):
                findings.append(f"{name}:{n}: action {ref} is not pinned to a commit SHA")
        if re.search(r"\bpull_request_target\b", line.split("#", 1)[0]):
            findings.append(f"{name}:{n}: pull_request_target is forbidden")
    if not re.search(r"^permissions:", text, re.MULTILINE):
        findings.append(f"{name}: no top-level `permissions:` block")
    return findings


def lint_repo(root: Path = ROOT) -> list[str]:
    """Lint every workflow under ``.github/workflows``."""
    findings: list[str] = []
    for path in sorted((root / ".github" / "workflows").glob("*.y*ml")):
        findings += lint_text(path.relative_to(root).as_posix(), path.read_text(encoding="utf-8"))
    return findings


def main() -> int:
    """Entry point; returns the process exit code."""
    findings = lint_repo()
    for finding in findings:
        print(finding, file=sys.stderr)
    print(f"lint_workflows: {len(findings)} finding(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
