# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# REBOUND 5.2.2 + REBOUNDx 5.1.0 oracle image (REF-005, CI-001). GPL-3.0 tools, isolated:
# * built and kept locally or in a CI cache only, NEVER pushed to any registry (LIC-002, red-team
#   L3; maintainer decision 2026-10-14);
# * contains no FerroAstro build, crate or wheel (red-team L4): the build context staged by
#   tools/oracles/run_oracles.sh holds only this directory and tools/oracles/*.py;
# * receives a scenario and header.440 through a read-only mount and returns plain files (LIC-001).
# Base image pinned by digest (python:3.12-slim-bookworm, created 2026-10-06), apt and pip packages
# by exact version, pip packages also by SHA-256.
FROM python:3.12-slim-bookworm@sha256:34386ef0cb081344d7ec1c103ba398e6e9f64e9ab3a1509accc92a4e24a07258

# A C compiler is needed only because reboundx 5.1.0 ships as an sdist.
RUN apt-get update \
 && apt-get install -y --no-install-recommends gcc=4:12.2.0-3 libc6-dev=2.36-9+deb12u14 \
 && rm -rf /var/lib/apt/lists/*

COPY requirements.txt requirements-reboundx.txt /opt/fa/
RUN pip install --no-cache-dir --no-deps --require-hashes -r /opt/fa/requirements.txt \
 && pip install --no-cache-dir --no-deps --require-hashes --no-build-isolation \
      -r /opt/fa/requirements-reboundx.txt \
 && python -c "import rebound, reboundx; print('rebound', rebound.__version__, 'reboundx', reboundx.__version__)"

COPY oracles/ /opt/fa/tools/oracles/
WORKDIR /opt/fa/tools
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0
USER nobody
ENTRYPOINT ["python", "-m", "oracles.rebound_runner"]
