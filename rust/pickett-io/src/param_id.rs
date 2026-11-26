use std::fmt;

/// A parameter or dipole identifier (`IDPAR`, `IDIP`), ported from the BCD
/// handling in `getbcd()` / `putbcd()` (`ulib.c`).
///
/// Identifiers are decimal digit strings that can be longer than a 64-bit
/// integer, so they are kept as text. A leading `-` (`negative`) means the
/// parameter is tied to the previous one; for a dipole, it is grouped with the
/// previous one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParamId {
    negative: bool,
    digits: String,
}

impl ParamId {
    /// Builds an identifier from its digits, for example `ParamId::new(10000, false)`
    /// for rotational constant A of an SPFIT model.
    pub fn new(value: u128, negative: bool) -> Self {
        ParamId { negative, digits: value.to_string() }
    }

    /// True when the identifier was written with a leading `-`.
    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// The digits without sign or leading zeros (`"0"` for zero).
    pub fn digits(&self) -> &str {
        &self.digits
    }

    /// The identifier as an unsigned integer, when it fits in a `u128`.
    pub fn value(&self) -> Option<u128> {
        self.digits.parse().ok()
    }

    /// Right-aligned field as `putbcd` writes it: the sign goes directly
    /// before the first digit. The field grows if `width` is too small.
    pub fn to_field(&self, width: usize) -> String {
        format!("{:>width$}", self.to_string())
    }

    /// Reads an identifier at the start of `line` like `getbcd()`:
    /// characters before the first digit are skipped (a comma there means
    /// "no identifier"), a `-` right before the first digit makes it
    /// negative, and a trailing `.fraction` is ignored.
    ///
    /// Returns the identifier, the index just after its digits (used to find
    /// the field width), and the index where reading of the rest of the line
    /// should continue (after trailing blanks and at most one comma).
    pub fn parse_prefix(line: &str) -> Option<(ParamId, usize, usize)> {
        let b = line.as_bytes();
        let mut kbgn = 0;
        loop {
            match b.get(kbgn) {
                None | Some(b',') => return None,
                Some(c) if c.is_ascii_digit() => break,
                Some(_) => kbgn += 1,
            }
        }
        let negative = kbgn > 0 && b[kbgn - 1] == b'-';
        let mut kend = kbgn;
        while kend < b.len() && b[kend].is_ascii_digit() {
            kend += 1;
        }
        let digit_end = kend;
        let raw = &line[kbgn..kend];
        let trimmed = raw.trim_start_matches('0');
        let digits = if trimmed.is_empty() { "0" } else { trimmed }.to_string();
        if kend < b.len() && b[kend] == b'.' {
            kend += 1;
            while kend < b.len() && b[kend].is_ascii_digit() {
                kend += 1;
            }
        }
        let mut commas = 0;
        while let Some(&c) = b.get(kend) {
            if c == b'-' || c == b'.' || c == b'/' || c.is_ascii_digit() {
                break;
            }
            if c == b',' {
                if commas != 0 {
                    break;
                }
                commas += 1;
            }
            kend += 1;
        }
        Some((ParamId { negative, digits }, digit_end, kend))
    }
}

impl fmt::Display for ParamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `pad` honours width and alignment, so `{:>8}` works.
        if self.negative {
            f.pad(&format!("-{}", self.digits))
        } else {
            f.pad(&self.digits)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_getbcd() {
        let (id, end, next) = ParamId::parse_prefix("        10099  1.48E+05 1E-35 /A-B").unwrap();
        assert_eq!(id.digits(), "10099");
        assert!(!id.is_negative());
        assert_eq!(end, 13);
        assert_eq!(next, 15);

        let (id, _, _) = ParamId::parse_prefix("       -30099  9.19E+03").unwrap();
        assert!(id.is_negative());
        assert_eq!(id.to_string(), "-30099");

        let (id, _, _) = ParamId::parse_prefix("  0010.  5").unwrap();
        assert_eq!(id.digits(), "10");

        assert!(ParamId::parse_prefix("   , 5").is_none());
        assert!(ParamId::parse_prefix("  no digits").is_none());
    }

    #[test]
    fn option_line_prefix_letter_is_skipped() {
        let (id, _, _) = ParamId::parse_prefix("a   1    1   0   50").unwrap();
        assert_eq!(id.digits(), "1");
    }

    #[test]
    fn writes_like_putbcd() {
        assert_eq!(ParamId::new(10000, false).to_field(13), "        10000");
        assert_eq!(ParamId::new(1122, true).to_field(13), "        -1122");
        assert_eq!(ParamId::new(1, false).to_field(5), "    1");
    }

    #[test]
    fn display_honours_width() {
        assert_eq!(format!("{:>8}|", ParamId::new(200, true)), "    -200|");
        assert_eq!(format!("{:<6}|", ParamId::new(10000, false)), "10000 |");
    }
}
