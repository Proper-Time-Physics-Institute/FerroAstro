# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Mercury6 oracle image (REF-008, CI-001): the github.com/smirik/mercury mirror (labelled GPL-3.0;
# the original Chambers licence is UNVERIFIED, OI-25), pinned to commit
# aee9e0f6b8e4d359a9ed3607ee12e34a2e2dafac (2023-02-18) by the SHA-256 of its source tarball.
# Treated as GPL and isolated:
# * built and kept locally or in a CI cache only, NEVER pushed to any registry (LIC-002, red-team
#   L3; maintainer decision 2026-10-14); the source is fetched at build time, never vendored;
# * contains no FerroAstro build (red-team L4): the build context staged by
#   tools/oracles/run_oracles.sh holds only this directory and tools/oracles/*.py;
# * receives a scenario and header.440 through a read-only mount and returns plain files (LIC-001).
# Built with `make build` as the mirror's README documents; no Fortran source is read by FerroAstro
# contributors (DEP-008). Newtonian only: its GR is never used (REF-008).
FROM python:3.12-slim-bookworm@sha256:34386ef0cb081344d7ec1c103ba398e6e9f64e9ab3a1509accc92a4e24a07258

RUN apt-get update \
 && apt-get install -y --no-install-recommends gfortran=4:12.2.0-3 make=4.3-4.1 \
      libc6-dev=2.36-9+deb12u14 \
 && rm -rf /var/lib/apt/lists/*

ADD --checksum=sha256:9ef3b2457c5c9e1d87fe3103daec7ce8647372689555a199e6138b2cccfbd609 \
    https://github.com/smirik/mercury/archive/aee9e0f6b8e4d359a9ed3607ee12e34a2e2dafac.tar.gz \
    /tmp/mercury.tar.gz
RUN mkdir -p /opt/mercury6/src /opt/mercury6/bin /opt/mercury6/share \
 && tar xzf /tmp/mercury.tar.gz -C /opt/mercury6/src --strip-components=1 \
 && cd /opt/mercury6/src && make build \
 && cp mercury6 element6 /opt/mercury6/bin/ \
 && cp message.in.sample files.in.sample LICENSE README.md mercury6.man /opt/mercury6/share/ \
 && gfortran --version | head -1 > /opt/mercury6/share/compiler.txt \
 && cd / && rm -rf /opt/mercury6/src /tmp/mercury.tar.gz

COPY settings.toml /opt/fa/
COPY oracles/ /opt/fa/tools/oracles/
WORKDIR /opt/fa/tools
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0
USER nobody
ENTRYPOINT ["python", "-m", "oracles.mercury6_runner"]
