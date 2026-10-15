// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-009 / LIC-004: the IAU 2006 precession/ecliptic routines and the vector/matrix helpers
//! ported in WP9 reproduce ERFA 2.0.1's own reference values (`src/t_erfa_c.c`), in `f64` and in
//! double-double.
//!
//! Expected values and tolerances are the vectors `ERFA-T-*` in
//! `tests/vectors/erfa-t_erfa_c-precession.toml`, extracted from `t_erfa_c.c`
//! (`vvd(val, valok, dval)`). The call arguments are repeated here, each line marked with its
//! `t_erfa_c.c` origin.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::inconsistent_digit_grouping,
    clippy::unreadable_literal,
    clippy::excessive_precision,
    reason = "test helpers: a failed unwrap is a test failure with a clear message; ERFA's \
              arguments are kept verbatim, in ERFA's digits and grouping"
)]

use ferroastro_erfa::{
    Matrix3, anp, anpm, c2s, eceq06, ecm06, eqec06, fw2m, ir, obl06, p06e, pfw06, pmat06, rx, rxp,
    rxr, rz, s2c, tr, trxp,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

/// Check `measured` against vector `id`, print the deviation, return it.
fn check(v: &VectorSet, id: &str, backend: &str, measured: f64) -> f64 {
    let diff = v
        .get(id)
        .unwrap()
        .check(measured)
        .unwrap_or_else(|e| panic!("{backend}: {e}"));
    println!("{id:<22} {backend:<14} measured {measured:<24e} |diff| {diff:e}");
    diff
}

fn check_matrix<R: Real>(v: &VectorSet, prefix: &str, m: &Matrix3<R>) -> f64 {
    let mut worst: f64 = 0.0;
    for (i, row) in m.iter().enumerate() {
        for (j, x) in row.iter().enumerate() {
            let id = format!("{prefix}{}{}", i + 1, j + 1);
            worst = worst.max(check(v, &id, R::NAME, x.to_f64()));
        }
    }
    worst
}

fn d<R: Real>(x: f64) -> R {
    R::from_f64(x)
}

/// The test matrix `r` of `t_rx`, `t_rz`, `t_rxr`, `t_tr`, `t_rxp`, `t_trxp` and `t_ir`.
fn erfa_r<R: Real>() -> Matrix3<R> {
    [
        [d(2.0), d(3.0), d(2.0)], // lint: allow-literal(t_erfa_c.c test matrix r)
        [d(3.0), d(2.0), d(3.0)], // lint: allow-literal(t_erfa_c.c test matrix r)
        [d(3.0), d(4.0), d(5.0)], // lint: allow-literal(t_erfa_c.c test matrix r)
    ]
}

fn run_helpers<R: Real>(v: &VectorSet) {
    let b = R::NAME;
    // t_ir: eraIr(r) overwrites r with the identity.
    check_matrix(v, "ERFA-T-IR-", &ir::<R>());
    // t_rx: eraRx(0.3456789, r)
    check_matrix(v, "ERFA-T-RX-", &rx(d::<R>(0.3456789), erfa_r())); // lint: allow-literal(t_erfa_c.c t_rx input)
    // t_rz: eraRz(0.3456789, r)
    check_matrix(v, "ERFA-T-RZ-", &rz(d::<R>(0.3456789), erfa_r())); // lint: allow-literal(t_erfa_c.c t_rz input)
    // t_rxr: eraRxr(a = r, b)
    let bm: Matrix3<R> = [
        [d(1.0), d(2.0), d(2.0)], // lint: allow-literal(t_erfa_c.c t_rxr matrix b)
        [d(4.0), d(1.0), d(1.0)], // lint: allow-literal(t_erfa_c.c t_rxr matrix b)
        [d(3.0), d(0.0), d(1.0)], // lint: allow-literal(t_erfa_c.c t_rxr matrix b)
    ];
    check_matrix(v, "ERFA-T-RXR-", &rxr(&erfa_r(), &bm));
    // t_tr: eraTr(r)
    check_matrix(v, "ERFA-T-TR-", &tr(&erfa_r::<R>()));
    // t_rxp / t_trxp: p = (0.2, 1.5, 0.1)
    let p = [d::<R>(0.2), d(1.5), d(0.1)]; // lint: allow-literal(t_erfa_c.c t_rxp/t_trxp vector p)
    for (k, x) in rxp(&erfa_r(), p).iter().enumerate() {
        check(v, &format!("ERFA-T-RXP-{}", k + 1), b, x.to_f64());
    }
    for (k, x) in trxp(&erfa_r(), p).iter().enumerate() {
        check(v, &format!("ERFA-T-TRXP-{}", k + 1), b, x.to_f64());
    }
    // t_s2c: eraS2c(3.0123, -0.999)
    let c = s2c(d::<R>(3.0123), d(-0.999)); // lint: allow-literal(t_erfa_c.c t_s2c input)
    for (k, x) in c.iter().enumerate() {
        check(v, &format!("ERFA-T-S2C-{}", k + 1), b, x.to_f64());
    }
    // t_c2s: p = (100.0, -50.0, 25.0)
    let (theta, phi) = c2s([d::<R>(100.0), d(-50.0), d(25.0)]); // lint: allow-literal(t_erfa_c.c t_c2s input)
    check(v, "ERFA-T-C2S-THETA", b, theta.to_f64());
    check(v, "ERFA-T-C2S-PHI", b, phi.to_f64());
    // t_anp: eraAnp(-0.1); t_anpm: eraAnpm(-4.0)
    check(v, "ERFA-T-ANP-VALUE", b, anp(d::<R>(-0.1)).to_f64()); // lint: allow-literal(t_erfa_c.c t_anp input)
    check(v, "ERFA-T-ANPM-VALUE", b, anpm(d::<R>(-4.0)).to_f64()); // lint: allow-literal(t_erfa_c.c t_anpm input)
}

fn run_precession<R: Real>(v: &VectorSet) {
    let b = R::NAME;
    // t_obl06: eraObl06(2400000.5, 54388.0)
    let ob = obl06(d::<R>(2_400_000.5), d(54_388.0)); // lint: allow-literal(t_erfa_c.c t_obl06 input)
    check(v, "ERFA-T-OBL06-VALUE", b, ob.to_f64());

    // t_p06e: eraP06e(2400000.5, 52541.0, ...)
    let a = p06e(d::<R>(2_400_000.5), d(52_541.0)); // lint: allow-literal(t_erfa_c.c t_p06e input)
    for (name, x) in [
        ("EPS0", a.eps0),
        ("PSIA", a.psia),
        ("OMA", a.oma),
        ("BPA", a.bpa),
        ("BQA", a.bqa),
        ("PIA", a.pia),
        ("BPIA", a.bpia),
        ("EPSA", a.epsa),
        ("CHIA", a.chia),
        ("ZA", a.za),
        ("ZETAA", a.zetaa),
        ("THETAA", a.thetaa),
        ("PA", a.pa),
        ("GAM", a.gam),
        ("PHI", a.phi),
        ("PSI", a.psi),
    ] {
        check(v, &format!("ERFA-T-P06E-{name}"), b, x.to_f64());
    }

    // t_pfw06: eraPfw06(2400000.5, 50123.9999, ...)
    let f = pfw06(d::<R>(2_400_000.5), d(50_123.9999)); // lint: allow-literal(t_erfa_c.c t_pfw06 input)
    check(v, "ERFA-T-PFW06-GAMB", b, f.gamb.to_f64());
    check(v, "ERFA-T-PFW06-PHIB", b, f.phib.to_f64());
    check(v, "ERFA-T-PFW06-PSIB", b, f.psib.to_f64());
    check(v, "ERFA-T-PFW06-EPSA", b, f.epsa.to_f64());

    // t_fw2m: eraFw2m(gamb, phib, psi, eps) with t_erfa_c.c's literal angles.
    let m = fw2m(
        d::<R>(-0.2243387670997992368e-5), // lint: allow-literal(t_erfa_c.c t_fw2m input gamb)
        d(0.4091014602391312982),          // lint: allow-literal(t_erfa_c.c t_fw2m input phib)
        d(-0.9501954178013015092e-3),      // lint: allow-literal(t_erfa_c.c t_fw2m input psi)
        d(0.4091014316587367472),          // lint: allow-literal(t_erfa_c.c t_fw2m input eps)
    );
    check_matrix(v, "ERFA-T-FW2M-", &m);

    // t_pmat06: eraPmat06(2400000.5, 50123.9999, rbp)
    let m = pmat06(d::<R>(2_400_000.5), d(50_123.9999)); // lint: allow-literal(t_erfa_c.c t_pmat06 input)
    check_matrix(v, "ERFA-T-PMAT06-", &m);

    // t_ecm06: eraEcm06(2456165.5, 0.401182685, rm)
    let m = ecm06(d::<R>(2_456_165.5), d(0.401_182_685)); // lint: allow-literal(t_erfa_c.c t_ecm06 input)
    check_matrix(v, "ERFA-T-ECM06-RM", &m);

    // t_eqec06: eraEqec06(1234.5, 2440000.5, 1.234, 0.987, &dl, &db)
    let (dl, db) = eqec06(d::<R>(1234.5), d(2_440_000.5), d(1.234), d(0.987)); // lint: allow-literal(t_erfa_c.c t_eqec06 input)
    check(v, "ERFA-T-EQEC06-DL", b, dl.to_f64());
    check(v, "ERFA-T-EQEC06-DB", b, db.to_f64());

    // t_eceq06: eraEceq06(2456165.5, 0.401182685, 5.1, -0.9, &dr, &dd)
    let (dr, dd) = eceq06(d::<R>(2_456_165.5), d(0.401_182_685), d(5.1), d(-0.9)); // lint: allow-literal(t_erfa_c.c t_eceq06 input)
    check(v, "ERFA-T-ECEQ06-DR", b, dr.to_f64());
    check(v, "ERFA-T-ECEQ06-DD", b, dd.to_f64());
}

/// Verifies: DEP-009, LIC-004
#[test]
fn vector_and_matrix_helpers_match_t_erfa_c_in_f64_and_dd() {
    let v = vectors();
    run_helpers::<f64>(&v);
    run_helpers::<DoubleDouble>(&v);
}

/// Verifies: DEP-009, LIC-004
#[test]
fn iau2006_precession_and_ecliptic_match_t_erfa_c_in_f64_and_dd() {
    let v = vectors();
    run_precession::<f64>(&v);
    run_precession::<DoubleDouble>(&v);
}
