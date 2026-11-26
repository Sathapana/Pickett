//! `.egy` files (energy levels from SPCAT, `calcat.c`).
//!
//! A level line is
//! `BLOCK INDEX ENERGY ERROR MIXING DEGEN: [(KTSP)] QN QN ...`
//! (`%6d %4d %17.6f %17.6f %10.6f %4d:` then `(%3d)` and `%3d` per quantum).
//! SPCAT can also write derivative and eigenvector lines (FLAGS 2–4); those
//! are kept as text.

use crate::cfmt;
use crate::error::ParseError;

/// One energy level.
#[derive(Debug, Clone, PartialEq)]
pub struct EgyLevel {
    pub block: i64,
    pub index: i64,
    /// Energy (cm⁻¹).
    pub energy: f64,
    /// Estimated error (cm⁻¹).
    pub error: f64,
    /// Mixing coefficient: well below 1 means the quantum-number label is uncertain.
    pub mixing: f64,
    pub degeneracy: i64,
    /// The optional `(nnn)` field before the quanta.
    pub ktsp: Option<i64>,
    pub qn: Vec<i32>,
}

/// A line of an `.egy` file.
#[derive(Debug, Clone, PartialEq)]
pub enum EgyEntry {
    Level(EgyLevel),
    /// Any other line (derivatives, eigenvectors), kept as text.
    Other(String),
}

/// A parsed `.egy` file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EgyFile {
    pub entries: Vec<EgyEntry>,
}

impl EgyFile {
    /// Parses a file. Lines that are not level lines become [`EgyEntry::Other`],
    /// so this never fails; the `Result` keeps the API the same as the other files.
    pub fn parse(text: &str) -> Result<EgyFile, ParseError> {
        let entries = text
            .lines()
            .map(|l| match parse_level(l) {
                Some(level) => EgyEntry::Level(level),
                None => EgyEntry::Other(l.to_string()),
            })
            .collect();
        Ok(EgyFile { entries })
    }

    /// The level lines only.
    pub fn levels(&self) -> impl Iterator<Item = &EgyLevel> {
        self.entries.iter().filter_map(|e| match e {
            EgyEntry::Level(l) => Some(l),
            EgyEntry::Other(_) => None,
        })
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            match e {
                EgyEntry::Other(s) => out.push_str(s),
                EgyEntry::Level(l) => {
                    out.push_str(&format!(
                        "{} {} {} {} {} {}:",
                        cfmt::d(l.block, 6),
                        cfmt::d(l.index, 4),
                        cfmt::f(l.energy, 17, 6),
                        cfmt::f(l.error, 17, 6),
                        cfmt::f(l.mixing, 10, 6),
                        cfmt::d(l.degeneracy, 4),
                    ));
                    if let Some(k) = l.ktsp {
                        out.push_str(&format!("({})", cfmt::d(k, 3)));
                    }
                    for &q in &l.qn {
                        out.push_str(&cfmt::d(q as i64, 3));
                    }
                }
            }
            out.push('\n');
        }
        out
    }
}

fn parse_level(line: &str) -> Option<EgyLevel> {
    let (head, tail) = line.split_once(':')?;
    let f: Vec<&str> = head.split_whitespace().collect();
    if f.len() != 6 {
        return None;
    }
    let mut tail = tail;
    let mut ktsp = None;
    if let Some(rest) = tail.strip_prefix('(') {
        let (k, after) = rest.split_once(')')?;
        ktsp = Some(k.trim().parse().ok()?);
        tail = after;
    }
    if !tail.is_ascii() {
        return None;
    }
    let qn = parse_quanta(tail)?;
    Some(EgyLevel {
        block: f[0].parse().ok()?,
        index: f[1].parse().ok()?,
        energy: f[2].parse().ok()?,
        error: f[3].parse().ok()?,
        mixing: f[4].parse().ok()?,
        degeneracy: f[5].parse().ok()?,
        ktsp,
        qn,
    })
}

/// The quanta after the colon, written with `%3d` each. Fixed 3-column
/// fields are used when every field is a clean right-aligned integer; a
/// value of 4+ characters (`-100`) breaks the columns, as in `"100-100"`,
/// and then the text is split at number boundaries like `scanf("%d")`.
fn parse_quanta(tail: &str) -> Option<Vec<i32>> {
    if tail.len() % 3 == 0 {
        let fixed: Option<Vec<i32>> = (0..tail.len() / 3)
            .map(|i| {
                let f = &tail[3 * i..3 * i + 3];
                let t = f.trim_start();
                (!t.is_empty() && !t.contains(' ')).then(|| t.parse::<i32>().ok()).flatten()
            })
            .collect();
        if fixed.is_some() {
            return fixed;
        }
    }
    let b = tail.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b' ' {
            i += 1;
            continue;
        }
        let start = i;
        if b[i] == b'-' {
            i += 1;
        }
        let digits = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == digits {
            return None; // not a number
        }
        out.push(tail[start..i].parse().ok()?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quanta_wider_than_three_columns() {
        let t = "   404   50      39662.146819          0.000000   1.000000  201:100-100\n";
        let f = EgyFile::parse(t).unwrap();
        assert_eq!(f.levels().next().unwrap().qn, vec![100, -100]);
        assert_eq!(f.to_text(), t);
        assert_eq!(parse_quanta("100100"), Some(vec![100, 100]));
        assert_eq!(parse_quanta("  1 -1 12"), Some(vec![1, -1, 12]));
        assert_eq!(parse_quanta(" x "), None);
    }

    #[test]
    fn parses_and_round_trips() {
        let t = "     6    1          5.273508          0.000000   1.000000    3:  1 -1\nsome other line\n";
        let f = EgyFile::parse(t).unwrap();
        let l = f.levels().next().unwrap();
        assert_eq!((l.block, l.index, l.degeneracy), (6, 1, 3));
        assert_eq!(l.energy, 5.273508);
        assert_eq!(l.qn, vec![1, -1]);
        assert!(matches!(f.entries[1], EgyEntry::Other(_)));
        assert_eq!(f.to_text(), t);
    }

    #[test]
    fn ktsp_field() {
        let t = "     1    1          0.000000          0.000000   1.000000    1:(  2)  0  0\n";
        let f = EgyFile::parse(t).unwrap();
        assert_eq!(f.levels().next().unwrap().ktsp, Some(2));
        assert_eq!(f.to_text(), t);
    }
}
