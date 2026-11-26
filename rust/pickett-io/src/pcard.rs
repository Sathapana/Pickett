//! Free-format number reading, ported from `pcard()` in `catutil.c`.
//!
//! The rules, which differ from a plain whitespace split:
//!
//! * Numbers may run together when a sign starts the next one:
//!   `0.31-0.07` is two numbers, `0.31` and `-0.07`. The `.var` covariance
//!   lines depend on this.
//! * A second decimal point starts a new number: `1.5.3` is `1.5` and `0.3`.
//! * `E`, `e`, `D` or `d` right after a number starts an exponent.
//! * A comma separates fields. Two commas in a row (spaces between are
//!   allowed) skip a field, which then keeps its default value.
//! * `/` ends the scan, so text after it (a label) is never read as numbers.
//! * Any other character is ignored.
//! * As in the C code, numbers with more than 37 integer-digit powers are
//!   clamped to `1e37`, and numbers below `1e-38` become `0`.
//!
//! The value of each number is the correctly rounded `f64` of its decimal
//! text. The C code accumulates digits itself, so in rare cases it can differ
//! from this by one unit in the last place.

/// Reads up to `vals.len()` numbers from `s` into `vals`. Fields that are not
/// found keep the value already in `vals` (the default). Returns the number
/// of fields consumed, counting fields skipped with `,,`.
pub fn pcard(s: &str, vals: &mut [f64]) -> usize {
    let b = s.as_bytes();
    let mut i = 0;
    let mut k = 0;
    let mut comma = false;
    while k < vals.len() {
        let Some(&c) = b.get(i) else { break };
        match c {
            b'0'..=b'9' | b'.' | b'-' | b'+' => {
                let (val, len) = scan_number(&b[i..]);
                vals[k] = val;
                k += 1;
                i += len;
                comma = false;
            }
            b',' => {
                if comma {
                    k += 1;
                }
                comma = true;
                i += 1;
            }
            b'/' => break,
            _ => i += 1,
        }
    }
    k
}

/// Convenience wrapper: reads up to `n` numbers and returns only those found
/// (no defaults, no skipped fields).
pub fn numbers(s: &str, n: usize) -> Vec<f64> {
    let mut v = vec![f64::NAN; n];
    let k = pcard(s, &mut v);
    v.truncate(k);
    v.retain(|x| !x.is_nan());
    v
}

/// Scans one number starting at `b[0]` (a digit, `.`, `-` or `+`).
/// Returns the value and the number of bytes used.
fn scan_number(b: &[u8]) -> (f64, usize) {
    let mut i = 0;
    let mut neg = false;
    if b[0] == b'-' || b[0] == b'+' {
        neg = b[0] == b'-';
        i = 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_part = &b[int_start..i];
    let mut frac_part: &[u8] = &[];
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_part = &b[frac_start..i];
    }
    let mut exp: i64 = 0;
    if i < b.len() && matches!(b[i], b'E' | b'e' | b'D' | b'd') {
        i += 1;
        let mut exp_neg = false;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            exp_neg = b[i] == b'-';
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let digits = std::str::from_utf8(&b[exp_start..i]).unwrap_or("0");
        exp = digits.parse::<i64>().unwrap_or(0);
        if exp_neg {
            exp = -exp;
        }
    }
    // C: npwr = integer digits typed + exponent; clamp outside 1e-38..1e37.
    let npwr = int_part.len() as i64 + exp;
    let mut val = if npwr < -38 {
        0.0
    } else if npwr > 37 {
        1e37
    } else {
        let text = format!(
            "{}.{}e{}",
            if int_part.is_empty() { "0" } else { std::str::from_utf8(int_part).unwrap() },
            if frac_part.is_empty() { "0" } else { std::str::from_utf8(frac_part).unwrap() },
            exp
        );
        text.parse::<f64>().unwrap_or(0.0)
    };
    if neg {
        val = -val;
    }
    (val, i.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(s: &str, n: usize) -> Vec<f64> {
        let mut v = vec![-1.0; n];
        let k = pcard(s, &mut v);
        v.truncate(k);
        v
    }

    #[test]
    fn plain_numbers() {
        assert_eq!(read("  1  41505    7820.3911  -11.0 1000. 300.", 6),
                   vec![1.0, 41505.0, 7820.3911, -11.0, 1000.0, 300.0]);
    }

    #[test]
    fn run_together_signs() {
        assert_eq!(read(" 0.3114888-0.0695831-0.1432090 0.4176056", 8),
                   vec![0.3114888, -0.0695831, -0.1432090, 0.4176056]);
    }

    #[test]
    fn exponents() {
        assert_eq!(read("1.0E+037 1.0E-35 2.5d3 3e", 4), vec![1e37, 1e-35, 2500.0, 3.0]);
    }

    #[test]
    fn clamps_like_c() {
        assert_eq!(read("5E37 1E-40", 2), vec![1e37, 0.0]);
    }

    #[test]
    fn slash_ends_scan() {
        assert_eq!(read("  1.5  2.5 /A-B 99", 4), vec![1.5, 2.5]);
    }

    #[test]
    fn double_comma_skips_field() {
        assert_eq!(read("1,,3", 3), vec![1.0, -1.0, 3.0]);
        assert_eq!(read("1, ,3", 3), vec![1.0, -1.0, 3.0]);
        assert_eq!(read("1,3", 3), vec![1.0, 3.0]);
    }

    #[test]
    fn second_decimal_point_starts_new_number() {
        assert_eq!(read("1.5.3", 2), vec![1.5, 0.3]);
    }

    #[test]
    fn bare_sign_is_zero() {
        assert_eq!(read("- 5", 2), vec![-0.0, 5.0]);
    }
}
