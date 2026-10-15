# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Oracle runners and images without starting a container (WP16).

The refusal paths run before any engine is imported, so they are tested here with no REBOUND,
pyerfa or jplephem installed. The real container runs are recorded in IMPLEMENTATION-NOTES WP16.
"""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path

import pytest

from oracles import common, compare_goldens, erfa_runner, rebound_runner
from oracles.header440 import Header440

ROOT = Path(__file__).resolve().parents[2]
CANARIES = ROOT / "ci" / "oracles" / "canaries"
PC_TOLERANCE = 1e-16  # lint: allow-literal(any IAS15 threshold: the runner refuses the key)
# Round-off bound of the synthetic-header arithmetic (a few ulp).
REL = 1e-14  # lint: allow-literal(round-off bound, synthetic header)
STAGE_A = ROOT / "ci" / "scenarios" / "stage-a-sun-8planets-newton-100yr.toml"


# Verifies: REF-011
def test_rebound_runner_refuses_the_ppn_canary_by_name(capsys: pytest.CaptureFixture[str]) -> None:
    """The REBOUNDx runner exits non-zero naming `ppn` (and `gr_eih`) for the canary."""
    status = rebound_runner.main(
        [
            "--scenario",
            str(CANARIES / "canary-ppn-gr-eih.toml"),
            "--header",
            "/nonexistent/header.440",
            "--out-dir",
            "/nonexistent/out",
        ]
    )
    err = capsys.readouterr().err
    assert status == common.REFUSAL_EXIT
    assert "`ppn`" in err
    assert "`gr_eih`" in err
    assert "REF-011" in err


# Verifies: REF-011
def test_rebound_runner_refuses_every_switch_but_newton() -> None:
    """Each CI-002 switch other than `newton`, and an unknown one, is refused by its name."""
    base = common.load_scenario(STAGE_A)
    rebound_runner.check(base)  # stage A is runnable
    values: dict[str, object] = {"ppn": {"beta": 1.0, "gamma": 1.0}, "asteroids": 1}
    for switch in [*common.SWITCHES[1:], "fifth_force"]:
        s = json.loads(json.dumps(base))
        s["forces"][switch] = values.get(switch, True)
        with pytest.raises(common.Refusal) as exc:
            rebound_runner.check(s)
        assert f"`{switch}`" in str(exc.value)
        assert [i for i, _ in exc.value.items] == [switch]


# Verifies: REF-011
def test_rebound_runner_refuses_unsupported_options_by_name() -> None:
    """Options the runner does not implement are refused by their scenario key."""
    base = common.load_scenario(STAGE_A)
    # (refused item, table or "" for the top level, key, value)
    cases: list[tuple[str, str, str, object]] = [
        ("mode", "", "mode", "test-particle"),
        ("integrator.precision", "integrator", "precision", "dd"),
        ("integrator.name", "integrator", "name", "whfast"),
        ("integrator.pc_tolerance", "integrator", "pc_tolerance", PC_TOLERANCE),
        ("initial_conditions.source", "initial_conditions", "source", "horizons"),
        ("output.perihelion", "output", "perihelion", ["Mercury"]),
        ("span.stop_after_passages", "span", "stop_after_passages", 3),
    ]
    for item, table, key, value in cases:
        s = json.loads(json.dumps(base))
        (s[table] if table else s)[key] = value
        with pytest.raises(common.Refusal) as exc:
            rebound_runner.check(s)
        assert f"`{item}`" in str(exc.value), item


# Verifies: REF-011
def test_erfa_runner_refuses_a_scenario(capsys: pytest.CaptureFixture[str]) -> None:
    """The ERFA job has no force switches; a scenario argument is refused, not ignored."""
    status = erfa_runner.main(["--out-dir", "/nonexistent", "--scenario", str(STAGE_A)])
    assert status == common.REFUSAL_EXIT
    assert "`--scenario`" in capsys.readouterr().err


# Verifies: CI-002
def test_canaries_are_schema_valid() -> None:
    """The canaries are refused for their switches, not for a schema error."""
    import validate_scenarios  # noqa: PLC0415

    schema = validate_scenarios.load_schema()
    paths = sorted(CANARIES.glob("*.toml"))
    assert len(paths) >= 2
    assert validate_scenarios.validate_files([*paths, STAGE_A], schema) == []


# Verifies: CI-003
def test_manifest_round_trip_and_validation(tmp_path: Path) -> None:
    """Complete entries validate; an empty field, a changed byte and an unlisted file do not."""
    out = tmp_path / "golden"
    out.mkdir()
    (out / "a.csv").write_text("t\n1\n", encoding="utf-8")
    entry = common.ManifestEntry(
        file="a.csv",
        tool="t",
        version="1",
        image_digest="sha256:" + "0" * 64,
        runner_sha="abc",
        runner_sha256="1" * 64,
        input_sha256={"in.toml": "2" * 64},
        cpu="cpu",
        wall_clock_s=0.0,
        output_sha256=common.sha256_file(out / "a.csv"),
        units="u",
        frame="f",
        time_scale="TDB",
        scenario="s",
        generated_utc="2026-10-15T00:00:00Z",
        command="c",
    )
    common.write_manifest(out, [entry])
    assert common.validate_manifest(out) == []
    doc = json.loads((out / common.MANIFEST_NAME).read_text(encoding="utf-8"))
    doc["entries"][0]["frame"] = ""
    (out / common.MANIFEST_NAME).write_text(json.dumps(doc), encoding="utf-8")
    assert any("`frame`" in p for p in common.validate_manifest(out))
    common.write_manifest(out, [entry])
    (out / "a.csv").write_text("t\n2\n", encoding="utf-8")
    assert any("output_sha256" in p for p in common.validate_manifest(out))
    (out / "b.csv").write_text("x\n", encoding="utf-8")
    assert any("b.csv: no manifest entry" in p for p in common.validate_manifest(out))


# Verifies: CI-003
def test_runner_identity_is_required(monkeypatch: pytest.MonkeyPatch) -> None:
    """Without the host-supplied digest and commit no golden is written (non-empty fields)."""
    monkeypatch.delenv(common.IMAGE_DIGEST_ENV, raising=False)
    monkeypatch.delenv(common.RUNNER_SHA_ENV, raising=False)
    with pytest.raises(common.SetupError):
        common.runner_identity()
    monkeypatch.setenv(common.IMAGE_DIGEST_ENV, "sha256:x")
    monkeypatch.setenv(common.RUNNER_SHA_ENV, "abc")
    assert common.runner_identity() == ("sha256:x", "abc")


# Verifies: REF-005
def test_header440_reader_on_a_synthetic_header() -> None:
    """GROUP 1040/1041 parsing, Fortran exponents and the Earth/Moon split from B and M."""
    names = ["CENTER", "EMRAT", "GMB", "XB", "YB", "ZB", "XDB", "YDB", "ZDB"]
    names += ["XM", "YM", "ZM", "XDM", "YDM", "ZDM"]
    values = ["0.0D+00", "0.8D+02", "0.81D+00", "0.1D+01", "0.0D+00", "0.0D+00"]
    values += ["0.0D+00", "0.1D-01", "0.0D+00", "0.81D-02", "0.0D+00", "0.0D+00"]
    values += ["0.0D+00", "0.81D-03", "0.0D+00"]
    text = (
        f"GROUP   1040\n {len(names)}\n {' '.join(names)}\n"
        f"GROUP   1041\n {len(values)}\n {' '.join(values)}\nGROUP   1050\n"
    )
    h = Header440.parse(text)
    gmb, emrat = h.value("GMB"), h.value("EMRAT")
    (xb, _, _), (_, ydb, _) = h.block("B")
    (xm, _, _), _ = h.block("M")
    assert emrat == float(values[1].replace("D", "E"))
    gm_e, pe, ve = h.state("Earth")
    gm_m, pm, vm = h.state("Moon")
    assert gm_e + gm_m == pytest.approx(gmb, rel=REL)
    assert gm_e / gm_m == pytest.approx(emrat, rel=REL)
    # The mass-weighted Earth and Moon recombine to the barycentre; the Moon is XM from the Earth.
    assert (gm_e * pe[0] + gm_m * pm[0]) / gmb == pytest.approx(xb, rel=REL)
    assert (gm_e * ve[1] + gm_m * vm[1]) / gmb == pytest.approx(ydb, rel=REL)
    assert pm[0] - pe[0] == pytest.approx(xm, rel=REL)


# Verifies: REF-009
def test_compare_goldens_ignores_only_comment_lines(tmp_path: Path) -> None:
    """Header lines may differ; any data byte may not."""
    a, b = tmp_path / "a", tmp_path / "b"
    a.mkdir()
    b.mkdir()
    (a / "g.csv").write_text("# t: 1\nc\n0.1\n", encoding="utf-8")
    (b / "g.csv").write_text("# t: 2\nc\n0.1\n", encoding="utf-8")
    assert compare_goldens.main([str(a), str(b), "g.csv"]) == 0
    (b / "g.csv").write_text("# t: 2\nc\n0.10000000000000002\n", encoding="utf-8")
    assert compare_goldens.main([str(a), str(b), "g.csv"]) == 1


def _dockerfiles() -> list[Path]:
    paths = sorted((ROOT / "ci" / "oracles").glob("*/Dockerfile"))
    assert {p.parent.name for p in paths} >= {"rebound", "erfa", "jpl"}
    return paths


# Verifies: CI-001
def test_oracle_images_are_fully_pinned() -> None:
    """Every FROM is by digest, apt packages by version, pip packages by == and SHA-256."""
    for dockerfile in _dockerfiles():
        text = dockerfile.read_text(encoding="utf-8")
        froms = re.findall(r"^FROM\s+(\S+)", text, re.MULTILINE)
        assert froms, dockerfile
        assert all(re.search(r"@sha256:[0-9a-f]{64}$", f) for f in froms), (dockerfile, froms)
        for pkg in re.findall(r"apt-get install[^\n]*(?:\\\n[^\n]*)*", text):
            for word in pkg.split()[2:]:
                if not word.startswith("-") and word != "\\":
                    assert "=" in word, (dockerfile, word)
        assert "--require-hashes" in text or "pip install" not in text, dockerfile
        for req in sorted(dockerfile.parent.glob("requirements*.txt")):
            lines = [
                ln
                for ln in req.read_text(encoding="utf-8").splitlines()
                if ln.strip() and not ln.startswith("#")
            ]
            assert lines, req
            for ln in lines:
                assert re.match(r"^[A-Za-z0-9_.-]+==[^ ]+ --hash=sha256:[0-9a-f]{64}$", ln), ln


# Verifies: LIC-001, LIC-002
def test_oracle_images_hold_no_ferroastro_build_and_are_never_pushed() -> None:
    """Red-team L3/L4: no FerroAstro build enters an image; nothing pushes an image."""
    forbidden = re.compile(r"ferroastro[-_]py|maturin|cargo|rustup|\.whl|crates/", re.IGNORECASE)
    for dockerfile in _dockerfiles():
        for f in sorted(dockerfile.parent.iterdir()):
            body = "\n".join(
                ln for ln in f.read_text(encoding="utf-8").splitlines() if not ln.startswith("#")
            )
            assert not forbidden.search(body), f
    for f in [ROOT / "tools" / "oracles" / "run_oracles.sh", *(ROOT / ".github").rglob("*.yml")]:
        code = "\n".join(
            ln
            for ln in f.read_text(encoding="utf-8").splitlines()
            if not ln.lstrip().startswith("#")
        )
        assert not re.search(r"docker\s+(image\s+)?push|buildx[^\n]*--push", code), f


# Verifies: LIC-001
def test_tools_project_never_depends_on_a_gpl_oracle() -> None:
    """Red-team L12: REBOUND/REBOUNDx/ASSIST/Swiftest never enter the tools/ uv project."""
    doc = tomllib.loads((ROOT / "tools" / "pyproject.toml").read_text(encoding="utf-8"))
    deps = list(doc["project"]["dependencies"])
    for group in doc.get("dependency-groups", {}).values():
        deps += [d for d in group if isinstance(d, str)]
    gpl = re.compile(r"^(rebound|reboundx|assist|swiftest)\b", re.IGNORECASE)
    assert not [d for d in deps if gpl.match(d)]
    lock = (ROOT / "tools" / "uv.lock").read_text(encoding="utf-8")
    assert not re.search(r'^name = "(rebound|reboundx|assist|swiftest)"$', lock, re.MULTILINE)
