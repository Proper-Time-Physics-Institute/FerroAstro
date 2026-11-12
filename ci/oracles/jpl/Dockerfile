# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# JPL oracle image (REF-001, CI-001): fetches header.440, testpo.440 and de440.bsp from
# https://ssd.jpl.nasa.gov/ftp/eph/planets/ into a mounted cache at run time (never into the image:
# JPL redistribution terms are unconfirmed, LIC-003/DEP-017) and verifies each against its pinned
# SHA-256 in tests/data/MANIFEST.toml (mounted read-only). Local or CI-cache only; no FerroAstro
# build inside (red-team L4). Base image pinned by digest, pip packages by version and SHA-256.
FROM python:3.12-slim-bookworm@sha256:34386ef0cb081344d7ec1c103ba398e6e9f64e9ab3a1509accc92a4e24a07258

COPY requirements.txt /opt/fa/
RUN pip install --no-cache-dir --no-deps --require-hashes -r /opt/fa/requirements.txt \
 && python -c "import jplephem; print('jplephem', jplephem.__version__)"

COPY oracles/ /opt/fa/tools/oracles/
WORKDIR /opt/fa/tools
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0
USER nobody
ENTRYPOINT ["python", "-m", "oracles.jpl_runner"]
