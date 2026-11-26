//! `printf`-style number formatting that matches the C programs' output.
//!
//! The one platform difference is the exponent of `%E`: the C standard (glibc,
//! macOS) prints at least 2 digits (`1.0E+05`), the old Microsoft runtime used
//! by MinGW prints 3 (`1.0E+005`). Writers take that as `exp_digits`, and
//! [`detect_exp_digits`] finds it from an existing file.

/// `%W.Pf`
pub fn f(x: f64, width: usize, prec: usize) -> String {
    format!("{:>width$.prec$}", x)
}

/// `%Wd`
pub fn d(x: i64, width: usize) -> String {
    format!("{:>width$}", x)
}

/// `%W.PE`, with at least `exp_digits` exponent digits.
pub fn e(x: f64, width: usize, prec: usize, exp_digits: usize) -> String {
    if !x.is_finite() {
        return format!("{:>width$}", x);
    }
    let s = format!("{:.prec$E}", x); // e.g. "6.077854915641401E4"
    let (mant, exp) = s.split_once('E').expect("Rust {:E} output always contains 'E'");
    let exp: i32 = exp.parse().expect("Rust {:E} exponent is an integer");
    let sign = if exp < 0 { '-' } else { '+' };
    let body = format!("{mant}E{sign}{:0>exp_digits$}", exp.unsigned_abs());
    format!("{:>width$}", body)
}

/// Number of exponent digits used by the first `E±ddd` number in `text`
/// (2 or 3). Returns 2, the C standard, when the text has no such number.
pub fn detect_exp_digits(text: &str) -> usize {
    let b = text.as_bytes();
    for i in 1..b.len().saturating_sub(2) {
        if (b[i] == b'E' || b[i] == b'e')
            && b[i - 1].is_ascii_digit()
            && (b[i + 1] == b'+' || b[i + 1] == b'-')
        {
            let n = b[i + 2..].iter().take_while(|c| c.is_ascii_digit()).count();
            if n == 2 || n == 3 {
                return n;
            }
        }
    }
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e_format_matches_c() {
        assert_eq!(e(60778.54915641401, 23, 15, 3), " 6.077854915641401E+004");
        assert_eq!(e(60778.54915641401, 23, 15, 2), "  6.077854915641401E+04");
        assert_eq!(e(1e-35, 14, 8, 3), "1.00000000E-035");
        assert_eq!(e(0.0, 14, 4, 2), "    0.0000E+00");
        assert_eq!(e(-2.5, 10, 3, 2), "-2.500E+00");
    }

    #[test]
    fn f_and_d() {
        assert_eq!(f(18397.783, 13, 4), "   18397.7830");
        assert_eq!(f(-0.0000000123, 10, 7), "-0.0000000");
        assert_eq!(d(-64999, 7), " -64999");
    }

    #[test]
    fn detects_exponent_width() {
        assert_eq!(detect_exp_digits("x 1.0E+005 y"), 3);
        assert_eq!(detect_exp_digits("x 1.0E-05"), 2);
        assert_eq!(detect_exp_digits("no exponent"), 2);
    }
}
