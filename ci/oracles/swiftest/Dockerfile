# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Swiftest 2026.7.1 oracle image (REF-007, CI-001). GPL-3.0-or-later tool, isolated:
# * built and kept locally or in a CI cache only, NEVER pushed to any registry (LIC-002, red-team
#   L3; maintainer decision 2026-10-14);
# * contains no FerroAstro build, crate or wheel (red-team L4): the build context staged by
#   tools/oracles/run_oracles.sh holds only this directory and tools/oracles/*.py;
# * receives a scenario and header.440 through a read-only mount and returns plain files (LIC-001).
# Swiftest is used through its documented Python API only (DEP-008). Base image pinned by digest,
# pip packages by exact version and SHA-256 (manylinux wheels; no compiler in the image).
FROM python:3.12-slim-bookworm@sha256:34386ef0cb081344d7ec1c103ba398e6e9f64e9ab3a1509accc92a4e24a07258

COPY requirements.txt settings.toml /opt/fa/
RUN pip install --no-cache-dir --no-deps --require-hashes --only-binary :all: \
      -r /opt/fa/requirements.txt \
 && python -c "import importlib.metadata as m; print('swiftest', m.version('swiftest'))"

COPY oracles/ /opt/fa/tools/oracles/
WORKDIR /opt/fa/tools
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 OMP_NUM_THREADS=1 MPLCONFIGDIR=/tmp/mpl HOME=/tmp
USER nobody
ENTRYPOINT ["python", "-m", "oracles.swiftest_runner"]
