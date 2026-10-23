# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Oracle runners (CI-001…CI-005, REF-005, REF-009, REF-011, LIC-001).

Each runner executes inside its own pinned container (``ci/oracles/<tool>/Dockerfile``), reads a
tool-agnostic scenario (``ci/scenarios/*.toml``, CI-002) or a fixed job, and writes plain files
plus ``MANIFEST.json`` entries (CI-003). Runners never import, link or receive a FerroAstro build
(red-team L4): FerroAstro reads the files afterwards, in a separate process and job.

Modules:

``common``
    Scenario loading, switch refusal (REF-011), file hashing and ``MANIFEST.json`` entries.
``header440``
    An independent ``header.440`` reader (GMs and the SSB-centred initial conditions).
``rebound_runner``
    REBOUND/REBOUNDx (GPL-3.0, black-box use of the documented Python API only, DEP-008).
``erfa_runner``
    pyerfa (BSD-3-Clause): regenerates the WP8/WP9 time and frame goldens.
``jpl_runner``
    jplephem (MIT): fetches and verifies the REF-001 JPL products against their pins.
"""
