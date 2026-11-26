//! `.lin` files (assigned lines for SPFIT), following `getlin()` in `ulib.c`
//! for reading and `calbak.c` for writing.
//!
//! Each line has fixed-width quantum numbers, 3 columns each, then free-format
//! `FREQ ERR WT`. With up to 6 quanta per state there are 12 fields (columns
//! 1–36): the upper-state quanta first, then the lower-state quanta.

use crate::cfmt;
use crate::error::ParseError;
use crate::pcard::pcard;

/// One assigned line.
#[derive(Debug, Clone, PartialEq)]
pub struct LinLine {
    /// The quantum-number fields in file order. `None` for a blank field.
    pub qn: Vec<Option<i32>>,
    /// Measured frequency (MHz, or cm⁻¹ when `err` is negative).
    pub freq: f64,
    /// Measurement error. Negative means the line is in cm⁻¹.
    pub err: f64,
    /// Relative weight within a blend.
    pub wt: f64,
}

impl LinLine {
    /// Upper-state quanta for `nqn` quanta per state (blank fields are 0).
    pub fn upper(&self, nqn: usize) -> Vec<i32> {
        self.qn.iter().take(nqn).map(|q| q.unwrap_or(0)).collect()
    }

    /// Lower-state quanta for `nqn` quanta per state (blank fields are 0).
    pub fn lower(&self, nqn: usize) -> Vec<i32> {
        self.qn.iter().skip(nqn).take(nqn).map(|q| q.unwrap_or(0)).collect()
    }
}

/// A parsed `.lin` file.
#[derive(Debug, Clone, PartialEq)]
pub struct LinFile {
    pub lines: Vec<LinLine>,
    /// Number of 3-column quantum fields per line (12, or `2*nqn` above 6 quanta).
    pub qn_fields: usize,
    /// Exponent digits for the weight field when writing.
    pub exp_digits: usize,
}

impl LinFile {
    /// Parses a file with up to 6 quanta per state (the usual case).
    pub fn parse(text: &str) -> Result<LinFile, ParseError> {
        Self::parse_with_nqn(text, 6)
    }

    /// Parses a file with `nqn` quanta per state. Above 6 the quantum fields
    /// take `6*nqn` columns, as in SPFIT (needs a negative `NLINE` in `.par`).
    pub fn parse_with_nqn(text: &str, nqn: usize) -> Result<LinFile, ParseError> {
        let qn_fields = if nqn <= 6 { 12 } else { 2 * nqn };
        let width = 3 * qn_fields;
        let mut lines = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            if raw.trim().is_empty() {
                continue;
            }
            if raw.len() < width {
                return Err(ParseError::new(
                    i + 1,
                    format!("line is shorter than the {width} quantum-number columns"),
                ));
            }
            if !raw.is_char_boundary(width) {
                return Err(ParseError::new(i + 1, "non-ASCII text in the quantum-number columns"));
            }
            let qn = (0..qn_fields)
                .map(|f| {
                    let mut v = [f64::NAN];
                    pcard(&raw[3 * f..3 * f + 3], &mut v);
                    (!v[0].is_nan()).then_some(v[0] as i32)
                })
                .collect();
            let mut v = [0.0, 0.01, 1.0];
            if pcard(&raw[width..], &mut v) == 0 {
                return Err(ParseError::new(i + 1, "no frequency after the quantum numbers"));
            }
            lines.push(LinLine { qn, freq: v[0], err: v[1], wt: v[2] });
        }
        Ok(LinFile { lines, qn_fields, exp_digits: cfmt::detect_exp_digits(text) })
    }

    /// Writes the file in the format of `calbak`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            for f in 0..self.qn_fields {
                match l.qn.get(f).copied().flatten() {
                    Some(q) => out.push_str(&cfmt::d(q as i64, 3)),
                    None => out.push_str("   "),
                }
            }
            let wt = cfmt::e(l.wt, 10, 3, self.exp_digits);
            if l.err > 0.0001 {
                out.push_str(&format!("{} {} {}\n", cfmt::f(l.freq, 15, 4), cfmt::f(l.err, 10, 4), wt));
            } else {
                out.push_str(&format!("{} {} {}\n", cfmt::f(l.freq, 15, 6), cfmt::f(l.err, 9, 6), wt));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = " 12  2 10 13  1 13                        8420.3548     0.0500 1.247E-007\n";

    #[test]
    fn parses_quanta_and_values() {
        let f = LinFile::parse(TEXT).unwrap();
        let l = &f.lines[0];
        assert_eq!(l.upper(3), vec![12, 2, 10]);
        assert_eq!(l.lower(3), vec![13, 1, 13]);
        assert_eq!(l.qn[6], None);
        assert_eq!((l.freq, l.err, l.wt), (8420.3548, 0.05, 1.247e-7));
    }

    #[test]
    fn round_trips_calbak_format() {
        assert_eq!(LinFile::parse(TEXT).unwrap().to_text(), TEXT);
    }

    #[test]
    fn short_line_is_an_error() {
        assert_eq!(LinFile::parse("  1  0  0  0 18397.78").unwrap_err().line, 1);
    }

    #[test]
    fn defaults_for_missing_err_and_wt() {
        let f = LinFile::parse("  1  0  0  0                          18397.7830\n").unwrap();
        assert_eq!((f.lines[0].err, f.lines[0].wt), (0.01, 1.0));
    }
}
