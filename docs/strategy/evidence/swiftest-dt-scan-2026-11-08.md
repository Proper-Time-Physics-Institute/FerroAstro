<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Swiftest stage A step-size scan (2026-11-08)

Evidence for `ORACLE-STAGE-A-SWIFTEST-MAX-DR` (REF-007) and the maintainer decision of 2026-11-08 on PR #13: stage A runs at dt = 1/16 d with a 2 km bound for every body.

**Method.** Scenario `ci/scenarios/stage-a-sun-8planets-newton-100yr.toml`: Sun and 8 planets (EMB), Newtonian, header.440 initial conditions, 100 Julian years, yearly output. The run used image `fa-oracle-swiftest:local-20261021` and Swiftest 2026.7.1 with WHM, one OpenMP thread. The only change between runs was `dt_days`; dt values had to be exact in binary64 so that a yearly output is a whole number of steps. The reference is the committed REBOUND IAS15 golden, which agrees with FerroAstro to below 1 m. Positions were made heliocentric by subtracting REBOUND's Sun. The table gives the largest |dr| over the 101 outputs, per body. Energy change is Swiftest's own `relative_energy_change` from its manifest.

| dt (d) | worst | Mercury | Venus | EMB | Mars | Jupiter | Saturn | Uranus | Neptune | rel. energy change |
|---|---|---|---|---|---|---|---|---|---|---|
| 0.25 | 7.916 | 7.361 | 2.222 | 4.517 | 7.916 | 0.021 | 0.097 | 0.292 | 0.174 | -1.6796812515533701e-12 |
| 0.125 | 1.969 | 1.851 | 0.563 | 1.140 | 1.969 | 0.044 | 0.660 | 0.524 | 0.355 | -5.8938375201248564e-12 |
| 0.0625 | 1.464 | 0.493 | 0.139 | 0.297 | 0.481 | 1.443 | 1.464 | 1.035 | 0.710 | -4.6540086237033645e-11 |
| 0.03125 | 4.110 | 0.150 | 0.036 | 0.451 | 0.147 | 4.110 | 2.965 | 2.066 | 1.416 | -1.2381479006082075e-10 |
| 0.015625 | 8.338 | 0.063 | 0.578 | 4.050 | 0.427 | 8.338 | 5.929 | 4.091 | 2.870 | -2.5253782498408517e-10 |
| 0.0078125 | 28.676 | 0.377 | 22.518 | 16.970 | 28.676 | 16.705 | 11.821 | 8.256 | 5.757 | -5.043150116866754e-10 |

The two terms:
- WHM truncation falls as dt² and dominates Mercury.
- Swiftest's own energy drift grows with the number of steps below 1/8 d, which marks f64 round-off. The outer-planet errors grow with it, but a simple drift model does not fit them body by body.

No dt meets 1 km for every body. The minimum is 1.464 km (Saturn) at dt = 1/16 d.

**Reproduce.** Per dt, run the container with a settings file where `dt_days` is changed, then compare it with the REBOUND golden:

```sh
docker run --rm --network none -e OMP_NUM_THREADS=1 -e FA_ORACLE_IMAGE_DIGEST=... -e FA_ORACLE_RUNNER_SHA=dt-scan \
  -v "$SCENARIO:/in/scen.toml:ro" -v "$FA_DATA_DIR/header.440:/in/header.440:ro" \
  -v "$PWD/settings.toml:/in/settings.toml:ro" -v "$PWD/out:/out" fa-oracle-swiftest:<tag> \
  --scenario /in/scen.toml --header /in/header.440 --out-dir /out --settings /in/settings.toml
```

```python
# Per-body max heliocentric |dr| (km) between a Swiftest CSV and the REBOUND golden.
import csv, math, sys
from collections import defaultdict
def load(p):
    rows = [r for r in csv.reader(l for l in open(p) if not l.startswith('#'))][1:]
    st = defaultdict(dict)
    for r in rows:
        st[float(r[0])][int(r[1])] = tuple(map(float, r[2:5]))
    return st
sw, rb = load(sys.argv[1]), load(sys.argv[2])
worst = defaultdict(float)
for t, bodies in sw.items():
    sun = rb[t].get(10, (0.0, 0.0, 0.0))
    for n, p in bodies.items():
        if n != 10 and n in rb[t]:
            worst[n] = max(worst[n], math.dist(p, tuple(a - b for a, b in zip(rb[t][n], sun))))
print(max(worst.values()), dict(sorted(worst.items())))
```
