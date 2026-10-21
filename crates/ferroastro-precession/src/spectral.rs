// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! A radix-2 complex FFT (iterative Cooley–Tukey, decimation in time) and a direct
//! non-uniform DFT power, used to find periodic terms for the Park-2017 fit.

use std::f64::consts::TAU;

/// In-place forward FFT `X_k = Σ_j x_j e^{−2πi jk/N}` of `re + i·im`. The length must be a power
/// of two; other lengths are left unchanged and `false` is returned.
#[must_use]
#[expect(
    clippy::indexing_slicing,
    reason = "indices are below n = re.len() = im.len(), checked on entry"
)]
pub fn fft_in_place(re: &mut [f64], im: &mut [f64]) -> bool {
    let n = re.len();
    if n != im.len() || !n.is_power_of_two() {
        return false;
    }
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        #[expect(clippy::cast_precision_loss, reason = "FFT lengths are far below 2^53")]
        let ang = -TAU / len as f64;
        let (ws, wc) = ang.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0_f64, 0.0_f64);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b].mul_add(cr, -(im[b] * ci));
                let ti = re[b].mul_add(ci, im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let next = cr.mul_add(wc, -(ci * ws));
                ci = cr.mul_add(ws, ci * wc);
                cr = next;
            }
        }
        len <<= 1;
    }
    true
}

/// `|Σ_j y_j e^{−iωt_j}|²`, the direct (non-uniform) DFT power at angular frequency `omega`.
#[must_use]
pub fn dft_power(t: &[f64], y: &[f64], omega: f64) -> f64 {
    let (mut s, mut c) = (0.0, 0.0);
    for (tj, yj) in t.iter().zip(y) {
        let (sn, cs) = (omega * tj).sin_cos();
        s += yj * sn;
        c += yj * cs;
    }
    s.mul_add(s, c * c)
}

/// Maximise [`dft_power`] over `[lo, hi]` by golden-section search (`iters` iterations).
#[must_use]
pub fn refine_peak(t: &[f64], y: &[f64], lo: f64, hi: f64, iters: usize) -> f64 {
    let g = (5.0_f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (lo, hi);
    let mut x1 = b - g * (b - a);
    let mut x2 = a + g * (b - a);
    let (mut f1, mut f2) = (dft_power(t, y, x1), dft_power(t, y, x2));
    for _ in 0..iters {
        if f1 < f2 {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = dft_power(t, y, x2);
        } else {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = dft_power(t, y, x1);
        }
    }
    f64::midpoint(a, b)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::suboptimal_flops, reason = "test fixtures favour readability")]
    use super::*;

    /// The FFT equals a direct DFT, and the golden-section search finds an off-grid tone.
    ///
    /// Verifies: NUM-011
    #[test]
    fn fft_matches_direct_dft_and_peak_refines() {
        let n = 64;
        let x: Vec<f64> = (0..n)
            .map(|j| (f64::from(j) * 0.37).sin() + 0.25 * f64::from(j % 5)) // lint: allow-literal(math or algorithm fixture, not physics)
            .collect(); // lint: allow-literal(arbitrary test signal)
        let (mut re, mut im) = (x.clone(), vec![0.0; x.len()]);
        assert!(fft_in_place(&mut re, &mut im));
        for k in 0..n {
            let (mut sr, mut si) = (0.0, 0.0);
            for (j, xj) in x.iter().enumerate() {
                let a = -TAU * f64::from(k) * f64::from(u32::try_from(j).unwrap()) / f64::from(n);
                sr += xj * a.cos();
                si += xj * a.sin();
            }
            let k = usize::try_from(k).unwrap();
            assert!((re[k] - sr).abs() < 1e-11 && (im[k] - si).abs() < 1e-11); // lint: allow-literal(round-off bound for n = 64)
        }
        let mut odd = vec![0.0; 3];
        assert!(!fft_in_place(&mut odd.clone(), &mut odd));
        // A real cosine's periodogram peak is pulled by its negative-frequency image by
        // O(1/(ω T²)); with T = 2000 that is well below the asserted 1e-6.
        let t: Vec<f64> = (0..20_000).map(|j| f64::from(j) * 0.1).collect(); // lint: allow-literal(math or algorithm fixture, not physics)
        let w0 = 2.345; // lint: allow-literal(arbitrary off-grid test frequency)
        let y: Vec<f64> = t.iter().map(|tj| (w0 * tj + 0.3).cos()).collect(); // lint: allow-literal(arbitrary phase)
        let w = refine_peak(&t, &y, 2.344, 2.346, 80); // lint: allow-literal(bracket around the test frequency)
        assert!((w - w0).abs() < 1e-6, "{w}"); // lint: allow-literal(golden-section convergence)
    }
}
