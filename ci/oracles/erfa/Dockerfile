# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# ERFA oracle image: pyerfa 2.0.1.5 (ERFA 2.0.1, SOFA 20231011; BSD-3-Clause) (REF-009, CI-001).
# Regenerates the WP8/WP9 time-scale and frame goldens with the committed generators
# (tools/goldens/erfa_time.py, erfa_frames.py). Local or CI-cache only; contains no FerroAstro
# build (red-team L4). Base image pinned by digest, pip packages by version and SHA-256.
FROM python:3.12-slim-bookworm@sha256:34386ef0cb081344d7ec1c103ba398e6e9f64e9ab3a1509accc92a4e24a07258

COPY requirements.txt /opt/fa/
RUN pip install --no-cache-dir --no-deps --require-hashes -r /opt/fa/requirements.txt \
 && python -c "import erfa; print('pyerfa', erfa.__version__, 'ERFA', erfa.version.erfa_version)"

COPY oracles/ /opt/fa/tools/oracles/
COPY goldens/ /opt/fa/tools/goldens/
WORKDIR /opt/fa/tools
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0
USER nobody
ENTRYPOINT ["python", "-m", "oracles.erfa_runner"]
