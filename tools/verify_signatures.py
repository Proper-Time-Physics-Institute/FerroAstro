# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
r"""Signed commits and delegation records (ORG-008, ADR-0005 revision 2026-11-14, ADR-0015).

A ``Signed-off-by`` trailer alone is not trusted. Every non-merge commit of a PR must be signed
(SSH or OpenPGP) by a key registered in ``.github/signing/``, and that key's **delegation record**
must name the operator (the human or organisation responsible for the agent) whose
``Signed-off-by`` the commit carries. The ``signatures`` workflow runs this file and reads the
registry from a checkout of the **base** SHA, so a PR cannot register its own key.

Registry (base tree):

* ``.github/signing/allowed_signers``: SSH public keys in ``ssh-keygen`` allowed-signers format
  (``principal namespaces="git" key``).
* ``.github/signing/keys/*.asc``: OpenPGP public keys (optional).
* ``.github/signing/delegations.toml``: ``mode`` (``report`` or ``enforce``), ``grandfathered``
  commit SHAs, and one ``[[delegation]]`` per key: ``fingerprint`` (``SHA256:…`` for SSH, 40 hex
  for OpenPGP), ``principal`` (SSH only), ``operator`` (exactly as in ``Signed-off-by``),
  ``agent``, ``valid_from``, ``valid_until`` (ISO dates, inclusive) and ``record`` (where the
  signed delegation statement is kept).

Validity window. The committer date (``%cs``) is set by whoever makes the commit, so it can be
backdated and proves nothing on its own. A commit therefore needs both: its committer date inside
``valid_from..valid_until``, **and** the check itself running no later than ``valid_until`` (UTC
date of the CI run). So an expired or ended delegation cannot be used by backdating a commit; a
backdated commit inside a still-valid window passes, which is no more than the key could sign
honestly. To revoke a key before its window ends, remove its record (maintainer-reviewed PR).

In ``report`` mode findings print as warnings and the check passes; ``enforce`` fails on them.
The mode lives on the base branch, so only a maintainer-reviewed PR can switch it (ORG-011).

Usage (from the base checkout's ``tools/``)::

    python verify_signatures.py --registry ../.github/signing --repo ../../head \\
        --base-sha BASE --head-sha HEAD
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SSH_GOOD_RE = re.compile(r'Good "git" signature for (\S+) with \S+ key (SHA256:\S+)')
GPG_VALID_RE = re.compile(r"^\[GNUPG:\] VALIDSIG ([0-9A-F]{40})\b", re.MULTILINE)
GPG_GOOD_RE = re.compile(r"^\[GNUPG:\] GOODSIG ", re.MULTILINE)


@dataclass(frozen=True)
class Delegation:
    """One registered signing key and the operator it is delegated from."""

    fingerprint: str
    operator: str
    agent: str
    valid_from: dt.date
    valid_until: dt.date
    record: str
    principal: str | None = None

    @classmethod
    def parse(cls, raw: Mapping[str, Any]) -> Delegation:
        """Build from one ``[[delegation]]`` table; raise ``ValueError`` when incomplete."""
        missing = [
            k
            for k in ("fingerprint", "operator", "agent", "valid_from", "valid_until", "record")
            if not str(raw.get(k, "")).strip()
        ]
        if missing:
            raise ValueError(f"delegation {raw.get('fingerprint')!r} lacks {', '.join(missing)}")
        return cls(
            fingerprint=str(raw["fingerprint"]).strip(),
            operator=str(raw["operator"]).strip(),
            agent=str(raw["agent"]).strip(),
            valid_from=dt.date.fromisoformat(str(raw["valid_from"])),
            valid_until=dt.date.fromisoformat(str(raw["valid_until"])),
            record=str(raw["record"]).strip(),
            principal=str(raw["principal"]).strip() if raw.get("principal") else None,
        )


@dataclass(frozen=True)
class Policy:
    """``delegations.toml``."""

    mode: str
    grandfathered: frozenset[str]
    delegations: tuple[Delegation, ...]

    @classmethod
    def load(cls, path: Path) -> Policy:
        """Parse and validate the policy file."""
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        mode = str(data.get("mode", ""))
        if mode not in ("report", "enforce"):
            raise ValueError(f"mode must be 'report' or 'enforce', not {mode!r}")
        return cls(
            mode=mode,
            grandfathered=frozenset(str(s).lower() for s in data.get("grandfathered", [])),
            delegations=tuple(Delegation.parse(d) for d in data.get("delegation", [])),
        )

    def find(self, fingerprint: str) -> Delegation | None:
        """Return the delegation for ``fingerprint`` (OpenPGP: case-insensitive)."""
        for d in self.delegations:
            if d.fingerprint == fingerprint or d.fingerprint.upper() == fingerprint.upper():
                return d
        return None


@dataclass(frozen=True)
class Signature:
    """A verified signature: who signed, with which key."""

    fingerprint: str
    principal: str | None


def _git(
    repo: Path, *args: str, env: Mapping[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # noqa: S603
        ["git", *args],  # noqa: S607
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
        env=None if env is None else {**os.environ, **env},
    )


def parse_ssh_output(stderr: str) -> Signature | None:
    """Read ``git verify-commit`` output for an SSH signature."""
    m = SSH_GOOD_RE.search(stderr)
    return Signature(m.group(2), m.group(1)) if m else None


def parse_gpg_status(stderr: str) -> Signature | None:
    """Read ``git verify-commit --raw`` status lines for an OpenPGP signature."""
    m = GPG_VALID_RE.search(stderr)
    return Signature(m.group(1), None) if m and GPG_GOOD_RE.search(stderr) else None


def verify_signature(repo: Path, sha: str, registry: Path, gnupg_home: Path) -> Signature | None:
    """Verify one commit's signature against the base registry; ``None`` when unverified."""
    raw = _git(repo, "cat-file", "commit", sha).stdout
    signers = (registry / "allowed_signers").resolve()
    if "BEGIN SSH SIGNATURE" in raw:
        res = _git(
            repo, "-c", f"gpg.ssh.allowedSignersFile={signers}", "verify-commit", "--raw", sha
        )
        return parse_ssh_output(res.stderr) if res.returncode == 0 else None
    if "BEGIN PGP SIGNATURE" in raw:
        res = _git(repo, "verify-commit", "--raw", sha, env={"GNUPGHOME": str(gnupg_home)})
        return parse_gpg_status(res.stderr) if res.returncode == 0 else None
    return None


def sign_offs(repo: Path, sha: str) -> list[str]:
    """Return the commit's ``Signed-off-by`` trailer values."""
    out = _git(repo, "log", "-1", "--format=%(trailers:key=Signed-off-by,valueonly)", sha).stdout
    return [line.strip() for line in out.splitlines() if line.strip()]


def check_commit(
    sha: str,
    signature: Signature | None,
    trailers: Sequence[str],
    commit_date: dt.date,
    policy: Policy,
) -> list[str]:
    """Return the findings for one commit (empty when it satisfies ORG-008)."""
    if sha.lower() in policy.grandfathered:
        return []
    if signature is None:
        return [f"{sha}: not signed by a key registered in .github/signing/"]
    delegation = policy.find(signature.fingerprint)
    if delegation is None:
        return [f"{sha}: key {signature.fingerprint} has no delegation record"]
    problems = []
    if delegation.principal is not None and signature.principal != delegation.principal:
        problems.append(
            f"{sha}: principal {signature.principal} does not match delegation "
            f"{delegation.principal}"
        )
    if not delegation.valid_from <= commit_date <= delegation.valid_until:
        problems.append(
            f"{sha}: committed {commit_date}, outside the delegation of {delegation.fingerprint} "
            f"({delegation.valid_from}..{delegation.valid_until})"
        )
    if delegation.operator not in trailers:
        problems.append(
            f"{sha}: Signed-off-by {list(trailers)} does not name the delegating operator "
            f"{delegation.operator!r}"
        )
    return problems


def ended_delegation(
    sha: str, signature: Signature | None, policy: Policy, checked_on: dt.date
) -> list[str]:
    """Return a finding when the signing key's delegation ended before ``checked_on``.

    The committer date that ``check_commit`` tests can be backdated; the date of the CI run cannot.
    """
    if sha.lower() in policy.grandfathered or signature is None:
        return []
    delegation = policy.find(signature.fingerprint)
    if delegation is None or checked_on <= delegation.valid_until:
        return []
    return [
        f"{sha}: delegation of {delegation.fingerprint} ended {delegation.valid_until}; "
        f"checked on {checked_on} (a committer date cannot extend it)"
    ]


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--registry", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--base-sha", required=True)
    parser.add_argument("--head-sha", required=True)
    args = parser.parse_args(argv)

    policy = Policy.load(args.registry / "delegations.toml")
    commits = _git(
        args.repo, "rev-list", "--no-merges", f"{args.base_sha}..{args.head_sha}"
    ).stdout.split()
    findings: list[str] = []
    today = dt.datetime.now(dt.UTC).date()
    with tempfile.TemporaryDirectory() as home:
        gnupg_home = Path(home)
        gnupg_home.chmod(0o700)
        for key in sorted((args.registry / "keys").glob("*.asc")):
            subprocess.run(  # noqa: S603
                ["gpg", "--batch", "--quiet", "--import", str(key)],  # noqa: S607
                env={**os.environ, "GNUPGHOME": home},
                check=True,
                capture_output=True,
            )
        for sha in commits:
            stamp = _git(args.repo, "log", "-1", "--format=%cs", sha).stdout.strip()
            signature = verify_signature(args.repo, sha, args.registry, gnupg_home)
            findings += check_commit(
                sha, signature, sign_offs(args.repo, sha), dt.date.fromisoformat(stamp), policy
            )
            findings += ended_delegation(sha, signature, policy, today)
    tag = "error" if policy.mode == "enforce" else "warning"
    for finding in findings:
        print(f"::{tag}::{finding} (ORG-008)")
    print(
        f"verify_signatures: {len(commits)} commit(s), {len(findings)} finding(s), "
        f"mode={policy.mode}"
    )
    return 1 if findings and policy.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
