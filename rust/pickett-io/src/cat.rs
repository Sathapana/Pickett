//! `.cat` files (JPL/CDMS catalog format), as written by SPCAT (`calcat.c`)
//! and `calmrg`.
//!
//! | Columns | Field | Format |
//! |---|---|---|
//! | 1–13 | `FREQ` | F13.4 (MHz) |
//! | 14–21 | `ERR` | F8.4 |
//! | 22–29 | `LGINT` | F8.4 (log₁₀ nm² MHz) |
//! | 30–31 | `DR` | I2 |
//! | 32–41 | `ELO` | F10.4 (cm⁻¹) |
//! | 42–44 | `GUP` | I3 (hundreds may be a letter) |
//! | 45–51 | `TAG` | I7 (negative: measured frequency) |
//! | 52–55 | `QNFMT` | I4 |
//! | 56– | quanta | 2 columns each: upper state, then lower state |

use crate::cfmt;
use crate::error::ParseError;
use crate::qn;

/// One catalog line.
#[derive(Debug, Clone, PartialEq)]
pub struct CatLine {
    pub freq: f64,
    pub err: f64,
    pub lgint: f64,
    pub dr: i32,
    pub elo: f64,
    pub gup: u32,
    pub tag: i64,
    pub qnfmt: i32,
    /// Upper-state quanta (`QNFMT % 10` of them, 10 when that digit is 0).
    pub upper: Vec<i32>,
    /// Lower-state quanta.
    pub lower: Vec<i32>,
}

impl CatLine {
    /// Number of quanta per state from `QNFMT`.
    pub fn nqn(&self) -> usize {
        nqn_of(self.qnfmt)
    }
}

fn nqn_of(qnfmt: i32) -> usize {
    match qnfmt.rem_euclid(10) {
        0 => 10,
        n => n as usize,
    }
}

/// A parsed `.cat` file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatFile {
    pub lines: Vec<CatLine>,
    /// Infrared format (`FLAGS` ≥ 1000 in `.int`): 6 decimals for frequency
    /// and error. Detected from the first line.
    pub ir: bool,
}

impl CatFile {
    pub fn parse(text: &str) -> Result<CatFile, ParseError> {
        let mut lines = Vec::new();
        let mut ir = None;
        for (i, raw) in text.lines().enumerate() {
            if raw.trim().is_empty() {
                continue;
            }
            let n = i + 1;
            if raw.len() < 55 || !raw.is_ascii() {
                return Err(ParseError::new(n, "catalog line is shorter than 55 columns or not ASCII"));
            }
            let num = |a: usize, b: usize, name: &str| -> Result<f64, ParseError> {
                raw[a..b]
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| ParseError::new(n, format!("bad {name} field {:?}", &raw[a..b])))
            };
            let freq = num(0, 13, "FREQ")?;
            if ir.is_none() {
                let decimals = raw[0..13].trim().split_once('.').map_or(0, |(_, f)| f.len());
                ir = Some(decimals == 6);
            }
            let err = num(13, 21, "ERR")?;
            let lgint = num(21, 29, "LGINT")?;
            let dr = num(29, 31, "DR")? as i32;
            let elo = num(31, 41, "ELO")?;
            let gup = parse_gup(&raw[41..44]).ok_or_else(|| ParseError::new(n, format!("bad GUP field {:?}", &raw[41..44])))?;
            let tag = num(44, 51, "TAG")? as i64;
            let qnfmt = num(51, 55, "QNFMT")? as i32;
            let nqn = nqn_of(qnfmt);
            let slots = nqn.max(6);
            let rest = &raw[55..];
            let upper = qn::decode(rest, nqn);
            let lower = qn::decode(rest.get(2 * slots..).unwrap_or(""), nqn);
            lines.push(CatLine { freq, err, lgint, dr, elo, gup, tag, qnfmt, upper, lower });
        }
        Ok(CatFile { lines, ir: ir.unwrap_or(false) })
    }

    /// Writes the file with SPCAT's formats.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            let head = if self.ir {
                format!("{}{}{}", cfmt::f(l.freq, 13, 6), cfmt::f(l.err, 8, 6), cfmt::f(l.lgint, 8, 4))
            } else if l.freq < 99999999.0 {
                format!("{}{}{}", cfmt::f(l.freq, 13, 4), cfmt::f(l.err, 8, 4), cfmt::f(l.lgint, 8, 4))
            } else {
                format!("{}{}{}", cfmt::f(l.freq, 13, 3), cfmt::f(l.err, 8, 3), cfmt::f(l.lgint, 8, 3))
            };
            out.push_str(&head);
            out.push_str(&cfmt::d(l.dr as i64, 2));
            out.push_str(&cfmt::f(l.elo, 10, 4));
            out.push_str(&format_gup(l.gup));
            out.push_str(&cfmt::d(l.tag, 7));
            out.push_str(&cfmt::d(l.qnfmt as i64, 4));
            let nqn = l.nqn();
            let slots = nqn.max(6);
            for state in [&l.upper, &l.lower] {
                for s in 0..slots {
                    match state.get(s) {
                        Some(&v) if s < nqn => out.push_str(&qn::encode(v)),
                        _ => out.push_str("  "),
                    }
                }
            }
            if slots == 6 {
                out.push(' '); // calcat.c pads the 6-quanta field to 25 characters
            }
            out.push('\n');
        }
        out
    }
}

/// `GUP` with the hundreds digit possibly a letter (`A` = 10), as `gupfmt()`.
fn parse_gup(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    let digit = |c: u8| match c {
        b' ' => Some(0),
        b'0'..=b'9' => Some((c - b'0') as u32),
        _ => None,
    };
    let hundreds = match b[0] {
        b'A'..=b'Z' => (b[0] - b'A') as u32 + 10,
        c => digit(c)?,
    };
    Some(hundreds * 100 + digit(b[1])? * 10 + digit(b[2])?)
}

fn format_gup(g: u32) -> String {
    if g <= 9 {
        return format!("  {g}");
    }
    let units = g % 10;
    let tens = (g / 10) % 10;
    let hundreds = g / 100;
    let h = match hundreds {
        0 => ' ',
        1..=9 => (b'0' + hundreds as u8) as char,
        10..=35 => (b'A' + (hundreds - 10) as u8) as char,
        _ => '*',
    };
    format!("{h}{tens}{units}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_spcat_line() {
        let t = "  150175.9741  0.0000 -5.1285 2    1.6959  6  30999 814 2 1-1 3     1 1 1 2     \n";
        let c = CatFile::parse(t).unwrap();
        assert_eq!(c.lines[0].upper, vec![2, 1, -1, 3]);
        assert_eq!(c.lines[0].lower, vec![1, 1, 1, 2]);
        assert_eq!(c.to_text(), t);
    }

    #[test]
    fn gup_letters() {
        assert_eq!(parse_gup("A23"), Some(1023));
        assert_eq!(format_gup(1023), "A23");
        assert_eq!(format_gup(7), "  7");
        assert_eq!(format_gup(31), " 31");
        assert_eq!(parse_gup("*00"), None);
    }
}
