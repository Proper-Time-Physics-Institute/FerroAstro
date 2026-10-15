# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
#
# Orekit 13.1.9 + Hipparchus 4.0.3 oracle image (REF-006, CI-001; WP17 stage B). Orekit and
# Hipparchus are Apache-2.0 like FerroAstro; the image still follows the oracle conventions:
# built locally, never pushed, no FerroAstro build inside, plain files in and out.
# * Base image by digest; OpenJDK 17 and unzip from Debian by exact version.
# * Jars from Maven Central by URL and SHA-256 (each also matched its .sha1 on Maven Central).
# * orekit-data at git commit 2422a7d06ee2cc3b5a503a47271203b0706f3e82 (2026-10-06, "Updated to
#   October 2026") by archive SHA-256. Its DE-440-ephemerides/lnxp1990.440 is removed so that
#   the only JPL ephemeris Orekit can load is the pinned linux_p1550p2650.440 mounted at run time
#   (tests/data/MANIFEST.toml product de440-linux-binary).
# * StageB.java (FerroAstro, Apache-2.0) is compiled here against the pinned jars; it uses the
#   documented Orekit API only.
FROM python:3.12-slim-bookworm@sha256:34386ef0cb081344d7ec1c103ba398e6e9f64e9ab3a1509accc92a4e24a07258

RUN apt-get update \
 && apt-get install -y --no-install-recommends openjdk-17-jdk-headless=17.0.20.1+1-1~deb12u1 \
      unzip=6.0-28+deb12u1 \
 && rm -rf /var/lib/apt/lists/*

ADD --checksum=sha256:a1dbec163c1119459cf79e2dfe32ff74b446f7a8d457e2d4d9761cd6cd820fbe \
    https://repo1.maven.org/maven2/org/orekit/orekit/13.1.9/orekit-13.1.9.jar /opt/orekit/lib/
ADD --checksum=sha256:5af54dc3c21b8d85e1264157ee91d93fa67e68c825cfb38834dc2bbb9b0f6583 \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-core/4.0.3/hipparchus-core-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:19594d5106ec6a46951e859e2924bfcade02a834c06c56bd3d2130a74f0fbb7e \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-geometry/4.0.3/hipparchus-geometry-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:75f0f0b3fdcd99d13e9c394b7ae809cce5a9c22f1c14b1b90a5109ed951467fe \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-ode/4.0.3/hipparchus-ode-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:3712c3b4f43d9c12bf7b7aad187764f4b3c09cc962504899d8951eb310b1e4be \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-fitting/4.0.3/hipparchus-fitting-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:4bc0dbb5c6d5fefda5db9d92141c6cbb807174a1d491a85df46bcc57a67b8992 \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-optim/4.0.3/hipparchus-optim-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:279eece43411f372bd8340355cb89101def0bf9b1b6824231b76181d1ec63a81 \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-filtering/4.0.3/hipparchus-filtering-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:8b066e3060dc119efcce241fb0720a3b9e59c026d0f5fe9e70abfd5e22d3817b \
    https://repo1.maven.org/maven2/org/hipparchus/hipparchus-stat/4.0.3/hipparchus-stat-4.0.3.jar /opt/orekit/lib/
ADD --checksum=sha256:3f1cdf30d802f5e3cbeaaf22cadf02e6f30c8ba6c0d9ef865c57c83e2532761e \
    https://gitlab.orekit.org/orekit/orekit-data/-/archive/2422a7d06ee2cc3b5a503a47271203b0706f3e82/orekit-data-2422a7d06ee2cc3b5a503a47271203b0706f3e82.zip \
    /tmp/orekit-data.zip
RUN mkdir -p /opt/orekit/data /opt/orekit/classes \
 && unzip -q /tmp/orekit-data.zip -d /tmp/od \
 && mv /tmp/od/orekit-data-2422a7d06ee2cc3b5a503a47271203b0706f3e82/* /opt/orekit/data/ \
 && rm -rf /opt/orekit/data/DE-440-ephemerides /opt/orekit/data/_pyinstaller /tmp/od \
      /tmp/orekit-data.zip \
 && chmod -R a+rX /opt/orekit

COPY StageB.java settings.toml /opt/fa/
RUN javac -encoding UTF-8 -cp '/opt/orekit/lib/*' -d /opt/orekit/classes /opt/fa/StageB.java \
 && java -version 2>&1 | head -1 > /opt/orekit/java-version.txt

COPY oracles/ /opt/fa/tools/oracles/
WORKDIR /opt/fa/tools
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0
USER nobody
ENTRYPOINT ["python", "-m", "oracles.orekit_runner"]
