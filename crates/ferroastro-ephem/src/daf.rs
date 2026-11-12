// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NAIF Double Precision Array File (DAF) container, parsed from a byte slice.
//!
//! Layout (NAIF "DAF Required Reading"): 1024-byte records numbered from 1. Record 1 is the file
//! record: `LOCIDW` (8 chars, address 0), `ND` (i32, 8), `NI` (i32, 12), `LOCIFN` (60 chars, 16),
//! `FWARD` (i32, 76), `BWARD` (i32, 80), `FREE` (i32, 84), `LOCFMT` (8 chars, 88: `LTL-IEEE` or
//! `BIG-IEEE`), nulls, `FTPSTR` (28 bytes, 699), nulls. Summary records form a doubly linked list
//! starting at `FWARD`; each holds `NEXT`, `PREV`, `NSUM` (as doubles) and `NSUM` summaries of
//! `SS = ND + (NI + 1) / 2` words (`ND` doubles then `NI` packed i32). The record after each
//! summary record holds the array names, `NC = 8·SS` characters each. Array data are addressed
//! by 1-based 8-byte word numbers.
//!
//! Pre-1995 files whose `LOCIDW` is `NAIF/DAF` carry no `LOCFMT`; their byte order is inferred
//! from `ND`, as other readers do. Every read is bounds-checked: a truncated or corrupt file gives
//! an [`EphemError`], never a panic (API-002).

use crate::EphemError;

/// Bytes per DAF record.
pub const RECORD_BYTES: usize = 1024;
/// Bytes per DAF word (one binary64).
pub const WORD_BYTES: usize = 8;
const WORDS_PER_RECORD: usize = RECORD_BYTES / WORD_BYTES;
const FTP_OFFSET: usize = 699;
/// The FTP validation string of the file record (NAIF `ZZFTPSTR`): an ASCII-mode transfer
/// corrupts it, which is how such damage is detected.
pub const FTPSTR: &[u8; 28] = b"FTPSTR:\r:\n:\r\n:\r\x00:\x81:\x10\xce:ENDFTP";

/// Byte order of the binary64 and i32 words of a DAF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// `LTL-IEEE`.
    Little,
    /// `BIG-IEEE`.
    Big,
}

impl Endian {
    /// The `LOCFMT` string of this byte order.
    #[must_use]
    pub const fn locfmt(self) -> &'static str {
        match self {
            Endian::Little => "LTL-IEEE",
            Endian::Big => "BIG-IEEE",
        }
    }
}

/// One array summary: `ND` doubles, `NI` integers and the array's name.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// The double-precision components.
    pub doubles: Vec<f64>,
    /// The integer components (the last two are the array's initial and final word addresses).
    pub ints: Vec<i32>,
    /// The array name (trailing blanks and nulls removed).
    pub name: String,
}

/// A parsed DAF file record over borrowed bytes.
#[derive(Debug, Clone)]
pub struct Daf<'a> {
    bytes: &'a [u8],
    endian: Endian,
    id_word: String,
    nd: usize,
    ni: usize,
    internal_name: String,
    fward: usize,
    bward: usize,
    free: usize,
}

fn format_err(quantity: &'static str, value: String, valid: &'static str) -> EphemError {
    EphemError::Format {
        quantity,
        value,
        valid,
        fix: "check that this is a binary DAF/SPK file transferred in binary mode, and verify its \
              SHA-256",
    }
}

fn to_u64(x: usize) -> u64 {
    u64::try_from(x).unwrap_or(u64::MAX)
}

fn bounds(
    quantity: &'static str,
    value: usize,
    unit: &'static str,
    min: usize,
    max: usize,
) -> EphemError {
    EphemError::Bounds {
        quantity,
        value: to_u64(value),
        unit,
        min: to_u64(min),
        max: to_u64(max),
    }
}

/// Read 8 bytes at `offset` as a binary64 in byte order `endian`.
pub(crate) fn f64_at(bytes: &[u8], offset: usize, endian: Endian) -> Option<f64> {
    let b: [u8; 8] = bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    Some(match endian {
        Endian::Little => f64::from_le_bytes(b),
        Endian::Big => f64::from_be_bytes(b),
    })
}

fn i32_at(bytes: &[u8], offset: usize, endian: Endian) -> Option<i32> {
    let b: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(match endian {
        Endian::Little => i32::from_le_bytes(b),
        Endian::Big => i32::from_be_bytes(b),
    })
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\0', ' '])
        .to_owned()
}

/// A non-negative integral binary64 as `usize` (`None` for NaN, fractions, negatives or > 2⁵³).
pub(crate) fn f64_to_usize(x: f64) -> Option<usize> {
    const TWO_53: f64 = 9_007_199_254_740_992.0;
    if !(0.0..=TWO_53).contains(&x) || x.fract() != 0.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked above: integral and 0 <= x <= 2^53"
    )]
    let n = x as u64;
    usize::try_from(n).ok()
}

impl<'a> Daf<'a> {
    /// Parse the file record of a DAF held in `bytes`.
    ///
    /// # Errors
    /// [`EphemError::Format`] (`FA-E1000`) if the file is shorter than one record, its id word is
    /// not `DAF/…` or `NAIF/DAF`, its `LOCFMT` is unknown, its FTP validation string is damaged,
    /// or `ND`/`NI`/`FWARD`/`BWARD`/`FREE` are out of range.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, EphemError> {
        let rec = bytes.get(..RECORD_BYTES).ok_or_else(|| {
            format_err(
                "file length",
                format!("{} bytes", bytes.len()),
                ">= 1024 bytes (one file record)",
            )
        })?;
        let id_raw = rec.get(..8).unwrap_or_default();
        let id_word = text(id_raw).to_ascii_uppercase();
        let endian = if id_word == "NAIF/DAF" {
            // Pre-LOCFMT files: infer the byte order from ND (which is small and positive).
            let le = i32_at(rec, 8, Endian::Little).unwrap_or(-1);
            if (1..=124).contains(&le) {
                Endian::Little
            } else {
                Endian::Big
            }
        } else if id_word.starts_with("DAF/") {
            let ftp = rec
                .get(FTP_OFFSET..FTP_OFFSET + FTPSTR.len())
                .unwrap_or_default();
            if ftp.iter().any(|&b| b != 0) && ftp != FTPSTR {
                return Err(format_err(
                    "FTPSTR (FTP validation string)",
                    format!("{:?}", String::from_utf8_lossy(ftp)),
                    "the NAIF FTP validation string, or all nulls in pre-FTP-string files",
                ));
            }
            match rec.get(88..96) {
                Some(b"LTL-IEEE") => Endian::Little,
                Some(b"BIG-IEEE") => Endian::Big,
                other => {
                    return Err(format_err(
                        "LOCFMT (binary format)",
                        format!("{:?}", other.map(String::from_utf8_lossy)),
                        "\"LTL-IEEE\" or \"BIG-IEEE\"",
                    ));
                }
            }
        } else {
            return Err(format_err(
                "LOCIDW (id word)",
                format!("{:?}", String::from_utf8_lossy(id_raw)),
                "\"DAF/<type>\" or \"NAIF/DAF\"",
            ));
        };
        let int = |offset: usize,
                   quantity: &'static str,
                   valid: &'static str|
         -> Result<usize, EphemError> {
            let v = i32_at(rec, offset, endian).unwrap_or(-1);
            usize::try_from(v).map_err(|_| format_err(quantity, v.to_string(), valid))
        };
        let nd = int(8, "ND (doubles per summary)", "0..=124")?;
        let ni = int(12, "NI (integers per summary)", "2..=250")?;
        if ni < 2 || nd + ni.div_ceil(2) > 125 {
            return Err(format_err(
                "ND + (NI + 1) / 2 (summary size)",
                format!("ND = {nd}, NI = {ni}"),
                "NI >= 2 and ND + (NI + 1) / 2 <= 125 words",
            ));
        }
        let fward = int(76, "FWARD (first summary record)", ">= 2")?;
        let bward = int(80, "BWARD (last summary record)", ">= 2")?;
        let free = int(84, "FREE (first free address)", ">= 1")?;
        if fward < 2 || bward < 2 || free < 1 {
            return Err(format_err(
                "FWARD/BWARD/FREE",
                format!("{fward}/{bward}/{free}"),
                "summary records >= 2 and first free address >= 1",
            ));
        }
        let internal_name = text(rec.get(16..76).unwrap_or_default());
        Ok(Self {
            bytes,
            endian,
            id_word,
            nd,
            ni,
            internal_name,
            fward,
            bward,
            free,
        })
    }

    /// The byte order.
    #[must_use]
    pub const fn endian(&self) -> Endian {
        self.endian
    }

    /// The id word, e.g. `"DAF/SPK"` (upper case, trailing blanks removed).
    #[must_use]
    pub fn id_word(&self) -> &str {
        &self.id_word
    }

    /// Doubles per summary (`ND`).
    #[must_use]
    pub const fn nd(&self) -> usize {
        self.nd
    }

    /// Integers per summary (`NI`).
    #[must_use]
    pub const fn ni(&self) -> usize {
        self.ni
    }

    /// The internal file name (`LOCIFN`).
    #[must_use]
    pub fn internal_name(&self) -> &str {
        &self.internal_name
    }

    /// `(FWARD, BWARD, FREE)`.
    #[must_use]
    pub const fn pointers(&self) -> (usize, usize, usize) {
        (self.fward, self.bward, self.free)
    }

    /// The whole file.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Number of whole 8-byte words in the file.
    #[must_use]
    pub const fn word_count(&self) -> usize {
        self.bytes.len() / WORD_BYTES
    }

    fn record_count(&self) -> usize {
        self.bytes.len().div_ceil(RECORD_BYTES)
    }

    /// The bytes of words `first..=last` (1-based addresses).
    ///
    /// # Errors
    /// [`EphemError::Bounds`] if the range is empty or not inside the file.
    pub fn words(&self, first: usize, last: usize) -> Result<&'a [u8], EphemError> {
        let max = self.word_count();
        if first < 1 || first > max {
            return Err(bounds("initial array address", first, "word", 1, max));
        }
        if last < first || last > max {
            return Err(bounds("final array address", last, "word", first, max));
        }
        self.bytes
            .get((first - 1) * WORD_BYTES..last * WORD_BYTES)
            .ok_or_else(|| bounds("final array address", last, "word", first, max))
    }

    /// Read word `address` (1-based) as a binary64.
    ///
    /// # Errors
    /// [`EphemError::Bounds`] if the address is outside the file.
    pub fn f64_word(&self, address: usize) -> Result<f64, EphemError> {
        let max = self.word_count();
        if address < 1 {
            return Err(bounds("word address", address, "word", 1, max));
        }
        (address - 1)
            .checked_mul(WORD_BYTES)
            .and_then(|offset| f64_at(self.bytes, offset, self.endian))
            .ok_or_else(|| bounds("word address", address, "word", 1, max))
    }

    fn record(&self, number: usize, quantity: &'static str) -> Result<&'a [u8], EphemError> {
        let max = self.record_count();
        if number < 1 {
            return Err(bounds(quantity, number, "record", 1, max));
        }
        // Checked: a corrupt NEXT pointer can be up to 2^53, which overflows a 32-bit usize
        // (wasm32) when scaled to bytes.
        (number - 1)
            .checked_mul(RECORD_BYTES)
            .and_then(|start| self.bytes.get(start..start.checked_add(RECORD_BYTES)?))
            .ok_or_else(|| bounds(quantity, number, "record", 1, max))
    }

    /// Every array summary, following the summary-record chain from `FWARD`.
    ///
    /// # Errors
    /// [`EphemError::Bounds`] if a summary or name record lies outside the file, and
    /// [`EphemError::Format`] if the chain loops or a record's `NEXT`/`NSUM` is not a valid
    /// count.
    pub fn summaries(&self) -> Result<Vec<Summary>, EphemError> {
        let ss = self.nd + self.ni.div_ceil(2);
        let per_record = (WORDS_PER_RECORD - 3) / ss;
        let nc = WORD_BYTES * ss;
        let mut out = Vec::new();
        let mut next = self.fward;
        let mut visited = 0usize;
        while next != 0 {
            visited += 1;
            if visited > self.record_count() {
                return Err(format_err(
                    "summary record chain",
                    format!("more than {} records", self.record_count()),
                    "a finite chain ending with NEXT = 0",
                ));
            }
            let rec = self.record(next, "summary record number")?;
            let names = self.record(next + 1, "name record number")?;
            let word = |i: usize| f64_at(rec, i * WORD_BYTES, self.endian).unwrap_or(f64::NAN);
            let nsum = f64_to_usize(word(2))
                .filter(|&n| n <= per_record)
                .ok_or_else(|| {
                    format_err(
                        "NSUM (summaries in record)",
                        word(2).to_string(),
                        "an integer 0..=(125 - 3) / SS",
                    )
                })?;
            for k in 0..nsum {
                let base = (3 + k * ss) * WORD_BYTES;
                let doubles = (0..self.nd)
                    .map(|i| f64_at(rec, base + i * WORD_BYTES, self.endian).unwrap_or(f64::NAN))
                    .collect();
                let ints = (0..self.ni)
                    .map(|i| {
                        i32_at(rec, base + self.nd * WORD_BYTES + i * 4, self.endian).unwrap_or(0)
                    })
                    .collect();
                let name = text(names.get(k * nc..(k + 1) * nc).unwrap_or_default());
                out.push(Summary {
                    doubles,
                    ints,
                    name,
                });
            }
            next = f64_to_usize(word(0)).ok_or_else(|| {
                format_err(
                    "NEXT (next summary record)",
                    word(0).to_string(),
                    "a record number >= 0",
                )
            })?;
        }
        Ok(out)
    }
}
