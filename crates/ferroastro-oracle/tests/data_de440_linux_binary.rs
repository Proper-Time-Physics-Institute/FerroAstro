// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Data tier (WP17, REF-006, OI-27): the JPL binary DE440 file that the Orekit stage B oracle reads
//! (`Linux/de440/linux_p1550p2650.440`, product `de440-linux-binary`) is fetched against its pinned
//! SHA-256, and its header records carry the same DE440 constants as `header.440`, bit for bit.
//!
//! JPL binary layout (little-endian, records of NCOEFF = 1018 doubles = 8144 bytes): record 1 holds
//! the title (3 × 84 characters), the first 400 constant names (6 characters each), the coverage
//! (start, end, step; 3 doubles), NCON (i32), AU and EMRAT; record 2 holds the constant values in
//! name order. Only the first 400 names live in record 1; the test compares those.

use ferroastro_constants::header440::Header;

const RECORD_BYTES: usize = 1018 * 8; // lint: allow-literal(NCOEFF of DE440 x 8 bytes, file layout)
const TITLE_BYTES: usize = 3 * 84; // lint: allow-literal(three 84-character title lines, file layout)
const NAMES_IN_RECORD_1: usize = 400; // lint: allow-literal(constant names held in record 1, file layout)
const NAME_BYTES: usize = 6; // lint: allow-literal(characters per constant name, file layout)

fn f64_at(b: &[u8], at: usize) -> f64 {
    let mut w = [0u8; 8];
    if let Some(s) = b.get(at..at + 8) {
        w.copy_from_slice(s);
    }
    // Outside the file: NaN, which no header.440 value equals.
    if b.len() < at + 8 {
        f64::NAN
    } else {
        f64::from_le_bytes(w)
    }
}

/// Verifies: REF-006, REF-001
#[test]
#[ignore = "data: downloads the 102 MB JPL binary DE440 file (cargo nextest run --profile data --run-ignored only)"]
fn data_de440_linux_binary_constants_equal_header440() {
    let bin = ferroastro_testdata::fetch_product("de440-linux-binary").expect("fetch");
    println!("de440-linux-binary: {}", bin.provenance());
    let hdr = ferroastro_testdata::fetch_product("de440-header").expect("fetch");
    println!("de440-header: {}", hdr.provenance());
    let header = Header::parse(&std::fs::read_to_string(&hdr.path).expect("read")).expect("parse");
    let bytes = std::fs::read(&bin.path).expect("read");
    let title = String::from_utf8_lossy(bytes.get(..84).expect("title"));
    println!("title: {}", title.trim());
    assert!(title.contains("DE440"));
    let names_at = TITLE_BYTES;
    let ss_at = names_at + NAMES_IN_RECORD_1 * NAME_BYTES;
    let (start, end) = (f64_at(&bytes, ss_at), f64_at(&bytes, ss_at + 8));
    let mut ncon = [0u8; 4];
    ncon.copy_from_slice(bytes.get(ss_at + 24..ss_at + 28).expect("NCON"));
    let ncon = usize::try_from(i32::from_le_bytes(ncon)).expect("NCON >= 0");
    println!("coverage JD {start} .. {end} TDB; {ncon} constants");
    assert_eq!(ncon, header.len());
    let mut compared = 0;
    for i in 0..NAMES_IN_RECORD_1.min(ncon) {
        let raw = bytes
            .get(names_at + i * NAME_BYTES..names_at + (i + 1) * NAME_BYTES)
            .expect("name");
        let name = String::from_utf8_lossy(raw).trim().to_owned();
        let value = f64_at(&bytes, RECORD_BYTES + 8 * i);
        let expected = header.value(&name).expect("the name exists in header.440");
        assert_eq!(
            value.to_bits(),
            expected.to_bits(),
            "{name}: binary file {value:e}, header.440 {expected:e}"
        );
        compared += 1;
    }
    println!(
        "{compared} constants bitwise equal to header.440 (AU {}, EMRAT {}, GMS {}, GM1 {})",
        header.value("AU").expect("AU"),
        header.value("EMRAT").expect("EMRAT"),
        header.value("GMS").expect("GMS"),
        header.value("GM1").expect("GM1"),
    );
    assert_eq!(compared, NAMES_IN_RECORD_1);
}
