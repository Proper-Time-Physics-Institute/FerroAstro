<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Floating-point reproducibility (TST-005, NUM-021, NUM-022)

This page states what FerroAstro guarantees about the bits of its results, and why. It was written for WP26 (M5). The evidence is the tests named below and the WP26 section of `docs/strategy/IMPLEMENTATION-NOTES.md`.

## Guarantees

1. **Same build, same machine, same scenario: the same bits.**
   - Every result is a deterministic function of the scenario, the data products (pinned by SHA-256) and the code. Nothing depends on wall time, thread scheduling or random seeds.
   - Tests: `same_scenario_twice_and_batch_thread_counts_are_bitwise` (WP13) and `batch_thread_counts_are_bitwise_for_both_kernels` (WP26).
2. **Thread count does not change results (NUM-022).**
   - `run_batch` parallelises across runs, never inside a run. Each run is single-threaded and is the same computation as a single `run`, whether it runs on 1, 2, 4 or N threads.
   - Test: `batch_thread_counts_are_bitwise_for_both_kernels`.
3. **The `simd` feature does not change results (NUM-021).**
   - The SoA kernels (`integrator.force_kernel = "soa"`) give the same bits with scalar lanes (feature off) and with `wide` f64x4 lanes (feature on).
   - Tests:
     - `simd_and_scalar_newtonian_kernels_are_bitwise_identical`;
     - `simd_and_scalar_eih_kernels_are_bitwise_identical`;
     - `simd_and_scalar_soa_models_integrate_bitwise` (10 yr of ABM steps);
     - `tools/simd_identity.py` (whole runs from separately built CLIs).
4. **Checkpoint and resume continue bit for bit (CI-010).**

## Why: the arithmetic rules

- **IEEE-754 binary64 with round-to-nearest.** Rust compiles `+ − × ÷ √` on `f64` to single correctly rounded instructions (SSE2/AVX on x86-64, NEON on aarch64, wasm `f64` ops). It never uses x87 extended precision, never reassociates and never contracts `a·b + c` into a fused multiply-add. FerroAstro uses no "fast-math".
- **FMA only where written** (TST-005 note, NUM-021).
  - `mul_add` appears explicitly in some kernels, for example the pairwise kernels' `r²` and the double-double error-free transformations. Rust's `f64::mul_add` is always fused: a hardware FMA when the target has one, otherwise a correctly rounded software routine. Its result is the same either way.
  - The SoA kernels contain **no** FMA, because `wide`'s `mul_add` is fused only when the `fma` target feature is enabled. They also contain no negation, because `wide`'s `Neg` is `0 − x`, which differs from `−x` for `+0`.
- **A fixed reduction order.**
  - Every force sum runs over its sources in storage order.
  - The SoA kernels vectorise across target bodies, so a body's sum stays inside one lane: there is no cross-lane (horizontal) reduction.
  - Compensated sums (Neumaier) are sequential.
- **Exact `1/r³` in the SoA kernels.** A Newton-refined inverse cube built from Dekker two-products is within ½ ulp + O(u²) of the true value. Measured: max 0.500000 ulp over 100 120 inputs, against 2.22 ulp for `1/(r²·√r²)` (WP26 P1).

## Cross-platform caveats

- **Elementary functions** (`sin`, `cos`, `atan2`, `exp`, `ln`, `powf`) come from the platform's libm under `std`, or from the pure-Rust `libm` without it. They are not correctly rounded and can differ in the last bit between platforms and libm versions.
  - The force kernels use none of them.
  - Initial-condition assembly, frame rotations, time scales and the precession estimators do use them. Results that pass through them are reproducible on one platform, and may differ by ulps across platforms.
- **`-C target-cpu=native`.**
  - It changes code generation, including the `wide` lane width and whether `f64::mul_add` is a hardware instruction. It does not change any correctly rounded result.
  - `tools/simd_identity.py --native` reports whether a native build reproduces the baseline digest. A difference there would point to an elementary function or a compiler change, not to the SoA kernels.
- **Different kernels are different arithmetic.** `force_kernel = "soa"` and `"pairwise"` differ at the ulp level (P3: ≤ 3.4e-16 relative in the accelerations). Over long runs that grows like any round-off. The kernel is recorded in the scenario (and its digest) and in the run's model string.
- **Subnormals.** Rust does not set flush-to-zero or denormals-are-zero, so subnormal arithmetic follows IEEE-754 on every target.
