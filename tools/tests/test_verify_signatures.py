# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for tools/verify_signatures.py: signed commits and delegation records (ORG-008)."""

from __future__ import annotations

import datetime as dt
import subprocess
from pathlib import Path

import pytest

import verify_signatures as vs

ROOT = Path(__file__).resolve().parents[2]
OPERATOR = "Op Erator <op@example.org>"
TODAY = dt.datetime.now(dt.UTC).date()


def policy(mode: str = "enforce", fpr: str = "SHA256:abc", **kw: str) -> vs.Policy:
    raw = {
        "fingerprint": fpr,
        "principal": "agent@fa",
        "operator": OPERATOR,
        "agent": "builder",
        "valid_from": str(TODAY - dt.timedelta(days=1)),
        "valid_until": str(TODAY + dt.timedelta(days=1)),
        "record": "issue #12",
    }
    raw.update(kw)
    return vs.Policy(mode, frozenset({"f" * 40}), (vs.Delegation.parse(raw),))


# Verifies: ORG-008
def test_check_commit_rules() -> None:
    sig = vs.Signature("SHA256:abc", "agent@fa")
    p = policy()
    assert vs.check_commit("c1", sig, [OPERATOR], TODAY, p) == []
    assert "not signed" in vs.check_commit("c1", None, [OPERATOR], TODAY, p)[0]
    assert vs.check_commit("f" * 40, None, [], TODAY, p) == []  # grandfathered
    other = vs.Signature("SHA256:zzz", "agent@fa")
    assert "no delegation record" in vs.check_commit("c1", other, [OPERATOR], TODAY, p)[0]
    assert "does not name" in vs.check_commit("c1", sig, ["Someone <s@x>"], TODAY, p)[0]
    late = TODAY + dt.timedelta(days=5)
    assert "outside the delegation" in vs.check_commit("c1", sig, [OPERATOR], late, p)[0]
    # A backdated committer date does not extend an ended delegation (checked on the CI date).
    after = TODAY + dt.timedelta(days=2)
    (found,) = vs.ended_delegation("c1", sig, p, after)
    assert "ended" in found
    assert vs.ended_delegation("c1", sig, p, TODAY) == []
    assert vs.ended_delegation("f" * 40, sig, p, after) == []  # grandfathered
    wrong = vs.Signature("SHA256:abc", "other@fa")
    assert "principal" in vs.check_commit("c1", wrong, [OPERATOR], TODAY, p)[0]


# Verifies: ORG-008
def test_incomplete_delegation_and_bad_mode_are_rejected(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="lacks record"):
        policy(record="")
    bad = tmp_path / "d.toml"
    bad.write_text('mode = "off"\n', encoding="utf-8")
    with pytest.raises(ValueError, match="mode"):
        vs.Policy.load(bad)


# Verifies: ORG-008
def test_committed_registry_parses() -> None:
    p = vs.Policy.load(ROOT / ".github" / "signing" / "delegations.toml")
    assert p.mode in ("report", "enforce")


# Verifies: ORG-008
def test_gpg_status_parsing() -> None:
    fpr = "0123456789ABCDEF0123456789ABCDEF01234567"
    good = f"[GNUPG:] GOODSIG 89ABCDEF01234567 x\n[GNUPG:] VALIDSIG {fpr} 2026-11-14 1\n"
    assert vs.parse_gpg_status(good) == vs.Signature(fpr, None)
    assert vs.parse_gpg_status(f"[GNUPG:] VALIDSIG {fpr} x\n") is None


def git(repo: Path, *args: str) -> str:
    return subprocess.run(  # noqa: S603
        ["git", "-c", "user.name=T", "-c", "user.email=t@x", *args],  # noqa: S607
        cwd=repo,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


# Verifies: ORG-008
def test_end_to_end_ssh(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    key = tmp_path / "k"
    subprocess.run(  # noqa: S603
        ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", "", "-f", str(key)],  # noqa: S607
        check=True,
    )
    fpr = subprocess.run(  # noqa: S603
        ["ssh-keygen", "-lf", f"{key}.pub"],  # noqa: S607
        check=True,
        capture_output=True,
        text=True,
    ).stdout.split()[1]
    reg = tmp_path / "reg"
    reg.mkdir()
    pub = (tmp_path / "k.pub").read_text(encoding="utf-8").strip()
    (reg / "allowed_signers").write_text(f'agent@fa namespaces="git" {pub}\n', encoding="utf-8")
    (reg / "delegations.toml").write_text(
        f'mode = "enforce"\n[[delegation]]\nfingerprint = "{fpr}"\nprincipal = "agent@fa"\n'
        f'operator = "{OPERATOR}"\nagent = "builder"\n'
        f'valid_from = "{TODAY - dt.timedelta(days=1)}"\n'
        f'valid_until = "{TODAY + dt.timedelta(days=1)}"\nrecord = "test"\n',
        encoding="utf-8",
    )
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "-q")
    git(repo, "commit", "-q", "--allow-empty", "-m", "base")
    base = git(repo, "rev-parse", "HEAD")
    sign = ["-c", "gpg.format=ssh", "-c", f"user.signingkey={key}"]
    git(
        repo,
        *sign,
        "commit",
        "-q",
        "-S",
        "--allow-empty",
        "-m",
        "ok",
        "-m",
        f"Signed-off-by: {OPERATOR}",
    )
    args = ["--registry", str(reg), "--repo", str(repo), "--base-sha", base, "--head-sha", "HEAD"]
    assert vs.main(args) == 0
    git(repo, "commit", "-q", "--allow-empty", "-m", "unsigned", "-m", f"Signed-off-by: {OPERATOR}")
    assert vs.main(args) == 1
    assert "not signed by a key registered" in capsys.readouterr().out
