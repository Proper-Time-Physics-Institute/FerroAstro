// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Data tier (network, `#[ignore]`, run with the nextest `data` profile): the SPK reader against
//! independent JPL products.
//!
//! * DE440 ASCII coefficient blocks (`ascp01550/01950/02550.440`, header.440 GROUP 1050): every
//!   coefficient of the 10 SSB-centred bodies equals the SPK's, rendered to 16 digits; the
//!   geocentric Moon agrees numerically.
//! * JPL Horizons (DE441) barycentric Mercury, Earth and Moon: gross-error bound only, replacing
//!   the ANISE cross-check that the maintainer decision in force excludes (DEP-011).
//! * sb441-n373s.bsp: Ceres, Pallas and Vesta against jplephem 2.24 goldens.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure with a clear message; block indices are \
              checked integral and non-negative before the cast; record epochs are computed \
              without FMA, as the SPK writer did"
)]

mod common;

use common::{Max, check, load, vectors};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_ephem::{Ephemeris, NaifId, SECONDS_PER_DAY, Spk, clenshaw_with_derivative};
use ferroastro_testdata::{fetch_product, horizons};

/// Sample points in each 4-day Moon sub-interval, as fractions of it.
const SAMPLE_FRACTIONS: [f64; 3] = [0.0625, 0.5, 0.875]; // lint: allow-literal(sample points in a sub-interval)

fn fetch(name: &str) -> Vec<u8> {
    let f = fetch_product(name).unwrap_or_else(|e| panic!("{e}"));
    println!("{name}: {}", f.provenance());
    std::fs::read(&f.path).unwrap()
}

/// header.440 GROUP 1050: per body `(pointer, coefficients, sub-intervals)` (pointer 1-based into
/// a block that starts with its two Julian dates).
fn group_1050(header: &str) -> Vec<(usize, usize, usize)> {
    let mut it = header
        .split_whitespace()
        .skip_while(|t| *t != "1050")
        .skip(1);
    let mut row = |n: usize| -> Vec<usize> {
        (0..n)
            .map(|_| it.next().unwrap().parse().unwrap())
            .collect()
    };
    let (p, c, s) = (row(15), row(15), row(15));
    (0..15).map(|i| (p[i], c[i], s[i])).collect()
}

/// One ASCII block: the raw tokens (with `D` exponents) of all its words.
fn blocks(text: &str, words_per_block: usize) -> Vec<Vec<String>> {
    let mut tokens = text.split_whitespace();
    let mut out = Vec::new();
    while let Some(_index) = tokens.next() {
        let n: usize = tokens.next().unwrap().parse().unwrap();
        assert_eq!(n, words_per_block);
        let mut b: Vec<String> = (0..n).map(|_| tokens.next().unwrap().to_owned()).collect();
        // Lines hold 3 words; the block's last line holds the remainder (no padding).
        b.shrink_to_fit();
        out.push(b);
    }
    out
}

fn ascii_f64(token: &str) -> f64 {
    token.replace('D', "E").parse().unwrap()
}

/// `(negative, 16 significant digits, decimal exponent of the leading digit)`, or `None` for 0.
fn digits_of_ascii(token: &str) -> Option<(bool, String, i32)> {
    let (neg, t) = token
        .strip_prefix('-')
        .map_or((false, token), |t| (true, t));
    let (mant, exp) = t.split_once('D').unwrap();
    let digits = mant.strip_prefix("0.").unwrap().to_owned();
    if digits.chars().all(|c| c == '0') {
        return None;
    }
    Some((neg, digits, exp.parse::<i32>().unwrap() - 1))
}

fn digits_of_f64(x: f64) -> Option<(bool, String, i32)> {
    if x == 0.0 {
        return None;
    }
    let s = format!("{:.15e}", x.abs());
    let (mant, exp) = s.split_once('e').unwrap();
    Some((x < 0.0, mant.replace('.', ""), exp.parse().unwrap()))
}

/// Verifies: DEP-011, REF-001
#[test]
#[ignore = "data tier: downloads ~90 MB of DE440 ASCII blocks"]
fn data_de440_ascii_coefficients_equal_the_spk() {
    let v = vectors();
    let bsp = fetch("de440-bsp");
    let spk = Spk::parse(&bsp).unwrap();
    let header = String::from_utf8(fetch("de440-header")).unwrap();
    let layout = group_1050(&header);
    let words_per_block = 1018; // lint: allow-literal(ASCII words per block = KSIZE/2, checked below)
    assert!(
        header.contains("KSIZE= 2036"),
        "header.440 block size changed"
    );
    let init_jd = 2_287_184.5; // lint: allow-literal(DE440 start epoch, checked against the SPK below)
    let block_days: f64 = 32.0; // lint: allow-literal(DE440 block length, header GROUP 1030, checked below)
    assert!(header.contains("2287184.50  2688976.50         32."));
    let (mut compared, mut mismatches, mut blocks_seen) = (0usize, Max::default(), 0usize);
    let mut first_mismatch = String::new();
    let (mut moon_r, mut moon_v) = (Max::default(), Max::default());
    for name in ["de440-ascp01550", "de440-ascp01950", "de440-ascp02550"] {
        let text = String::from_utf8(fetch(name)).unwrap();
        for b in blocks(&text, words_per_block) {
            blocks_seen += 1;
            let jd0 = ascii_f64(&b[0]);
            assert_eq!((ascii_f64(&b[1]) - jd0).to_bits(), block_days.to_bits());
            let block_index = (jd0 - init_jd) / block_days;
            assert!(
                block_index >= 0.0 && block_index.fract().to_bits() == 0,
                "{jd0}"
            );
            let block_index = block_index as usize;
            // Bodies 0..=8 are segments 1..=9 wrt SSB, 10 is the Sun (segment 10 wrt SSB).
            for (body, naif) in (0..9)
                .map(|i| (i, i32::try_from(i + 1).unwrap()))
                .chain([(10, 10)])
            {
                let (ptr, nc, ns) = layout[body];
                let seg = spk
                    .segments()
                    .iter()
                    .find(|s| s.target == naif && s.center == 0)
                    .unwrap();
                for j in 0..ns {
                    let rec = block_index * ns + j;
                    let (mid, radius) = seg.record_mid_radius(rec).unwrap();
                    let sub_days = block_days / f64::from(u32::try_from(ns).unwrap());
                    let mid_jd = jd0 + (f64::from(u32::try_from(j).unwrap()) + 0.5) * sub_days;
                    let want_mid = (mid_jd - ferroastro_ephem::J2000_JD) * SECONDS_PER_DAY;
                    assert_eq!(mid.to_bits(), want_mid.to_bits(), "MID of record {rec}");
                    assert_eq!(
                        radius.to_bits(),
                        (sub_days / 2.0 * SECONDS_PER_DAY).to_bits()
                    );
                    for comp in 0..3 {
                        let ours = seg.coefficients(rec, comp).unwrap();
                        for (k, x) in ours.iter().enumerate() {
                            let token = &b[ptr - 1 + nc * (3 * j + comp) + k];
                            compared += 1;
                            if digits_of_f64(*x) != digits_of_ascii(token) {
                                mismatches.value += 1.0;
                                if first_mismatch.is_empty() {
                                    first_mismatch = format!(
                                        "{name} jd0 {jd0} body {body} sub {j} comp {comp} k {k}: spk {x:e} ascii {token}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            // Geocentric Moon (body 9): ASCII series vs SPK (301 wrt 3) − (399 wrt 3).
            let (ptr, nc, ns) = layout[9];
            let sub_days = block_days / f64::from(u32::try_from(ns).unwrap());
            for j in 0..ns {
                for frac in SAMPLE_FRACTIONS {
                    // lint: allow-literal(sample points in a sub-interval)
                    let jdj = jd0 + f64::from(u32::try_from(j).unwrap()) * sub_days;
                    let off = frac * sub_days;
                    let e = Epoch::<Tdb>::from_jd_parts(jdj, off).unwrap();
                    let (s, _) = spk
                        .relative_state_km(NaifId::MOON, NaifId::EARTH, e)
                        .unwrap();
                    let sarg = 2.0 * frac - 1.0;
                    for comp in 0..3 {
                        let c = (0..nc).map(|k| ascii_f64(&b[ptr - 1 + nc * (3 * j + comp) + k]));
                        let (p, d) = clenshaw_with_derivative(c, sarg);
                        let vel = d * 2.0 / (sub_days * SECONDS_PER_DAY);
                        let at = || format!("{name} jd {jdj}+{off} comp {comp}");
                        moon_r.add((s.position[comp] - p).abs(), at);
                        moon_v.add((s.velocity[comp] - vel).abs(), at);
                    }
                }
            }
        }
    }
    println!(
        "{blocks_seen} ASCII blocks, {compared} coefficients compared; first mismatch: {first_mismatch:?}"
    );
    assert!(compared > 1_000_000, "the three files were compared"); // lint: allow-literal(count sanity)
    check(&v, "DE440-ASCII-COEFF-MISMATCHES", "count", &mismatches);
    check(&v, "DE440-ASCII-MOON-GEO-POS-MAX-DIFF", "f64", &moon_r);
    check(&v, "DE440-ASCII-MOON-GEO-VEL-MAX-DIFF", "f64", &moon_v);
}

/// Verifies: DEP-011
#[test]
#[ignore = "data tier: JPL Horizons API"]
fn data_horizons_de441_barycentric_states_vs_de440_gross_bound() {
    let v = vectors();
    let bsp = fetch("de440-bsp");
    let spk = Spk::parse(&bsp).unwrap();
    let (mut pos, mut vel) = (Max::default(), Max::default());
    // 1600-01-01 0h TDB, 21 epochs every 18 262 d (~50 yr) to ~2600.
    for (cmd, naif) in [
        ("199", NaifId::MERCURY),
        ("399", NaifId::EARTH),
        ("301", NaifId::MOON),
    ] {
        let q = horizons::VectorQuery {
            command: cmd.into(),
            center: "500@0".into(),
            ref_plane: horizons::RefPlane::Frame,
            start_jd_tdb: 2_305_447.5, // lint: allow-literal(1600-01-01 TDB, request epoch)
            stop_jd_tdb: 2_670_687.5,  // lint: allow-literal(start + 20 steps, request epoch)
            step_days: 18_262,         // lint: allow-literal(request step)
        };
        let t = horizons::fetch_vectors(&q).unwrap();
        for c in &t.chunks {
            println!("{}", c.line());
        }
        assert_eq!(
            t.sources(),
            vec!["DE441".to_owned()],
            "Horizons source changed"
        );
        let (mut pr, mut pv) = (Max::default(), Max::default());
        for (i, jd) in t.jd_tdb.iter().enumerate() {
            let e = Epoch::<Tdb>::from_jd_parts(*jd, 0.0).unwrap();
            let (s, frame) = spk.relative_state_km(naif, NaifId::SSB, e).unwrap();
            assert_eq!(frame, 1);
            for k in 0..3 {
                let at = || format!("{cmd} JD {jd} comp {k}");
                let dr = (s.position[k] - t.r_km[i][k]).abs();
                let dv = (s.velocity[k] - t.v_km_s[i][k]).abs();
                pr.add(dr, at);
                pv.add(dv, at);
                pos.add(dr, at);
                vel.add(dv, at);
            }
        }
        println!(
            "  {cmd}: {} epochs, DE440 - DE441 max |dr| {:.3e} km ({}), max |dv| {:.3e} km/s",
            t.jd_tdb.len(),
            pr.value,
            pr.at,
            pv.value
        );
    }
    check(&v, "HORIZONS-DE441-VS-DE440-POS-GROSS", "f64", &pos);
    check(&v, "HORIZONS-DE441-VS-DE440-VEL-GROSS", "f64", &vel);
}

/// Verifies: DEP-011, REF-004
#[test]
#[ignore = "data tier: downloads sb441-n373s.bsp (936.6 MB)"]
fn data_sb441_asteroids_match_jplephem() {
    let v = vectors();
    let g = load("jplephem-sb441.csv", &v, "SB441-JPLEPHEM-POS-MAX-DIFF");
    let bytes = fetch("sb441-n373s-bsp");
    assert!(g.meta("source").contains(&common::sha256(&bytes)));
    let spk = Spk::parse(&bytes).unwrap();
    println!(
        "sb441-n373s.bsp: {} segments, internal name {:?}",
        spk.segments().len(),
        spk.daf().internal_name()
    );
    let (mut pos, mut vel) = (Max::default(), Max::default());
    for row in &g.rows {
        let seg = &spk.segments()[usize::try_from(g.i(row, "seg")).unwrap()];
        assert_eq!(
            (seg.target, seg.center),
            (g.i(row, "target"), g.i(row, "center"))
        );
        let s = seg.state(g.epoch(row)).unwrap();
        let at = || format!("{} jd {}+{}", seg.target, g.s(row, "jd1"), g.s(row, "jd2"));
        for (k, c) in ["x", "y", "z"].iter().enumerate() {
            pos.add((s.position[k] - g.f(row, c)).abs(), at);
        }
        for (k, c) in ["vx", "vy", "vz"].iter().enumerate() {
            vel.add((s.velocity[k] - g.f(row, c)).abs(), at);
        }
    }
    check(&v, "SB441-JPLEPHEM-POS-MAX-DIFF", "f64", &pos);
    check(&v, "SB441-JPLEPHEM-VEL-MAX-DIFF", "f64", &vel);
}
