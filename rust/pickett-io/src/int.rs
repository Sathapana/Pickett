//! `.int` files (intensity control for SPCAT), following `calcat.c`.
//!
//! ```text
//! title
//! FLAGS TAG QROT FBGN FEND STR0 STR1 FQLIM TEMP MAXV
//! IDIP DIPOLE        (one line per dipole component)
//! ```

use crate::error::ParseError;
use crate::param_id::ParamId;
use crate::pcard::pcard;

/// Line 2 of the file. Missing fields get SPCAT's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct IntControl {
    /// Output flags, read digit by digit (see the manual).
    pub flags: i64,
    /// Species tag written to `.cat`.
    pub tag: i64,
    /// Partition function at `temp`, used for intensities.
    pub qrot: f64,
    /// Lowest quantum number (block) to calculate.
    pub fbgn: i64,
    /// Highest quantum number (block) to calculate.
    pub fend: i64,
    /// Minimum log₁₀ intensity.
    pub str0: f64,
    /// Minimum log₁₀ intensity scaled by (300 GHz / ν)².
    pub str1: f64,
    /// Maximum frequency (GHz, or cm⁻¹ in IR mode).
    pub fqlim: f64,
    /// Temperature (K) for intensities.
    pub temp: f64,
    /// Highest vibrational state (−1: all).
    pub maxv: i64,
}

impl Default for IntControl {
    fn default() -> Self {
        IntControl {
            flags: 0,
            tag: 999,
            qrot: 1000.0,
            fbgn: 0,
            fend: 0,
            str0: -100.0,
            str1: -100.0,
            fqlim: 9999.99,
            temp: 300.0,
            maxv: -1,
        }
    }
}

/// One dipole line: `IDIP VALUE` (Debye).
#[derive(Debug, Clone, PartialEq)]
pub struct Dipole {
    pub id: ParamId,
    pub value: f64,
}

/// A parsed `.int` file.
#[derive(Debug, Clone, PartialEq)]
pub struct IntFile {
    pub title: String,
    pub control: IntControl,
    pub dipoles: Vec<Dipole>,
    /// Lines after the first line that is not a dipole, kept as text
    /// (SPCAT ignores them).
    pub trailer: Vec<String>,
}

impl IntFile {
    pub fn parse(text: &str) -> Result<IntFile, ParseError> {
        let lines: Vec<&str> = text.lines().collect();
        let title = lines.first().ok_or_else(|| ParseError::new(1, "empty file"))?.to_string();
        let line2 = lines.get(1).ok_or_else(|| ParseError::new(2, "missing line 2"))?;
        let d = IntControl::default();
        let mut v = [d.flags as f64, d.tag as f64, d.qrot, d.fbgn as f64, d.fend as f64,
                     d.str0, d.str1, d.fqlim, d.temp, d.maxv as f64];
        if pcard(line2, &mut v) == 0 {
            return Err(ParseError::new(2, "unable to read line 2 (FLAGS TAG QROT ...)"));
        }
        let control = IntControl {
            flags: v[0] as i64,
            tag: v[1] as i64,
            qrot: v[2],
            fbgn: v[3] as i64,
            fend: v[4] as i64,
            str0: v[5],
            str1: v[6],
            fqlim: v[7],
            temp: v[8],
            maxv: v[9] as i64,
        };
        let mut dipoles = Vec::new();
        let mut idx = 2;
        while idx < lines.len() {
            let line = lines[idx];
            let Some((id, _, next)) = ParamId::parse_prefix(line) else { break };
            let mut v = [0.0];
            if pcard(&line[next..], &mut v) == 0 {
                break;
            }
            dipoles.push(Dipole { id, value: v[0] });
            idx += 1;
        }
        let trailer = lines[idx..].iter().map(|s| s.to_string()).collect();
        Ok(IntFile { title, control, dipoles, trailer })
    }

    /// Writes the file. Numbers use the shortest text that reads back to
    /// the same value, so nothing is lost.
    pub fn to_text(&self) -> String {
        let c = &self.control;
        let mut out = format!(
            "{}\n{:>4} {:>6} {:>14} {:>6} {:>6} {:>8} {:>8} {:>9} {:>7} {:>4}\n",
            self.title, c.flags, c.tag, num(c.qrot), c.fbgn, c.fend,
            num(c.str0), num(c.str1), num(c.fqlim), num(c.temp), c.maxv
        );
        for d in &self.dipoles {
            out.push_str(&format!("{}  {}\n", d.id.to_field(5), num(d.value)));
        }
        for t in &self.trailer {
            out.push_str(t);
            out.push('\n');
        }
        out
    }
}

/// Shortest round-trip text, always with a decimal point so it reads as a
/// real number.
fn num(x: f64) -> String {
    let s = format!("{x}");
    if s.contains(['.', 'e', 'E', 'i', 'N']) { s } else { format!("{s}.") }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "CH3CN\n1  41505    7820.3911       0    135   -11.0    -11.0    1000. 300.\n  001  3.92197\n";

    #[test]
    fn parses_control_and_dipoles() {
        let f = IntFile::parse(TEXT).unwrap();
        assert_eq!(f.control.tag, 41505);
        assert_eq!(f.control.qrot, 7820.3911);
        assert_eq!(f.control.fend, 135);
        assert_eq!(f.control.temp, 300.0);
        assert_eq!(f.control.maxv, -1);
        assert_eq!(f.dipoles.len(), 1);
        assert_eq!(f.dipoles[0].id.digits(), "1");
        assert_eq!(f.dipoles[0].value, 3.92197);
    }

    #[test]
    fn semantic_round_trip() {
        let f = IntFile::parse(TEXT).unwrap();
        assert_eq!(IntFile::parse(&f.to_text()).unwrap(), f);
    }
}
