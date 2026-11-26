//! `.par`, `.var` and `.bak` files (parameters for SPFIT/SPCAT/DPFIT/DPCAT),
//! following `calfit.c` (line 2), `setopt()` in `spinv.c`/`dpi.c` (option
//! lines) and `getpar()`/`getvar()`/`putvar()` in `ulib.c`.

use crate::cfmt;
use crate::error::ParseError;
use crate::param_id::ParamId;
use crate::pcard::pcard;

/// Which Hamiltonian module the file is for. It decides how many option
/// lines the file has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flavor {
    /// SPFIT/SPCAT (`spinv.c`). Option lines continue while `VSYM` is negative.
    #[default]
    Spinv,
    /// DPFIT/DPCAT (`dpi.c`). Exactly one option line.
    Dpi,
}

/// Line 2 of the file: `NPAR NLINE NITR NXPAR THRESH ERRTST FRAC CAL`.
/// Missing fields get SPFIT's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Control {
    /// Maximum number of parameters.
    pub npar: i64,
    /// Maximum number of lines. Negative allows up to 10 quanta per state.
    pub nline: i64,
    /// Number of iterations. Negative turns on assignment diagnostics.
    pub nitr: i64,
    /// Number of parameters to exclude from the end of the list for special lines.
    pub nxpar: i64,
    /// Initial Marquardt–Levenberg parameter.
    pub thresh: f64,
    /// Maximum |obs−calc|/error before a line is rejected.
    pub errtst: f64,
    /// Parameter-error scaling (negative: standard-error convention).
    pub frac: f64,
    /// Frequency scaling for infrared lines.
    pub cal: f64,
}

impl Default for Control {
    fn default() -> Self {
        Control {
            npar: 100,
            nline: 32767,
            nitr: 1,
            nxpar: 0,
            thresh: 0.0,
            errtst: 1e6,
            frac: 1.0,
            cal: 1.0,
        }
    }
}

/// One parameter line: `IDPAR VALUE ERROR /LABEL`.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub id: ParamId,
    /// The value as written in the file. For a tied parameter (negative
    /// `id`) SPFIT writes the absolute value, not the ratio.
    pub value: f64,
    /// A-priori error. `None` when the line has no error field; SPFIT then
    /// uses 1e37 (floated) and SPCAT 1e-37.
    pub error: Option<f64>,
    /// Text after `/`, without trailing blanks. Empty when there is none.
    /// (SPFIT itself keeps only the first 10 characters.)
    pub label: String,
}

/// One SPFIT/SPCAT option line, decoded with the defaults of `setopt()` in
/// `spinv.c`: `CHR SPIND NVIB KNMIN KNMAX IXX IAX WTPL WTMN VSYM EWT DIAG XOPT`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpinvOption {
    /// Leading letter in column 1 or 2 (for example `a` or `s`), if any.
    pub chr: Option<char>,
    /// Spin degeneracy code (`SPIND`); `None` when the line has no number.
    pub spind: Option<ParamId>,
    pub nvib: i64,
    pub knmin: i64,
    pub knmax: i64,
    pub ixx: i64,
    pub iax: i64,
    pub wtpl: i64,
    pub wtmn: i64,
    pub vsym: f64,
    pub ewt: i64,
    pub diag: i64,
    pub xopt: i64,
}

impl SpinvOption {
    /// Decodes one option line.
    pub fn parse(line: &str) -> SpinvOption {
        let b = line.as_bytes();
        let chr = [b.first(), b.get(1)]
            .into_iter()
            .flatten()
            .find(|c| c.is_ascii_alphabetic())
            .map(|&c| c as char);
        let (spind, rest) = match ParamId::parse_prefix(line) {
            Some((id, _, next)) => (Some(id), &line[next..]),
            None => (None, ""),
        };
        let mut v = [1.0, 0.0, 359.0, 0.0, 1.0, 1.0, 1.0, 0.0, 99.0, 0.0, 0.0];
        pcard(rest, &mut v);
        SpinvOption {
            chr,
            spind,
            nvib: v[0] as i64,
            knmin: v[1] as i64,
            knmax: v[2] as i64,
            ixx: v[3] as i64,
            iax: v[4] as i64,
            wtpl: v[5] as i64,
            wtmn: v[6] as i64,
            vsym: v[7],
            ewt: v[8] as i64,
            diag: v[9] as i64,
            xopt: v[10] as i64,
        }
    }
}

/// A parsed `.par`, `.var` or `.bak` file.
#[derive(Debug, Clone, PartialEq)]
pub struct ParFile {
    /// Line 1, as written (SPFIT appends a time stamp).
    pub title: String,
    /// Line 2.
    pub control: Control,
    /// Option lines, kept as text. Use [`ParFile::spinv_options`] to decode them.
    pub options: Vec<String>,
    /// Parameter lines.
    pub params: Vec<Parameter>,
    /// Lines after the parameters, kept as text: the packed covariance in a
    /// `.var` file (see [`ParFile::covariance`]), or extra lines in a `.par`.
    pub trailer: Vec<String>,
    /// Width of the `IDPAR` field when writing (13 for SPFIT, 5 for DPFIT).
    pub id_width: usize,
    /// Exponent digits used when writing `%E` numbers (2 or 3, see [`cfmt`]).
    pub exp_digits: usize,
}

impl ParFile {
    /// Parses a file for the given Hamiltonian module.
    pub fn parse(text: &str, flavor: Flavor) -> Result<ParFile, ParseError> {
        let lines: Vec<&str> = text.lines().collect();
        let title = lines
            .first()
            .ok_or_else(|| ParseError::new(1, "empty file"))?
            .to_string();

        let line2 = lines.get(1).ok_or_else(|| ParseError::new(2, "missing line 2"))?;
        let d = Control::default();
        let mut v = [d.npar as f64, d.nline as f64, d.nitr as f64, d.nxpar as f64,
                     d.thresh, d.errtst, d.frac, d.cal];
        if pcard(line2, &mut v) == 0 {
            return Err(ParseError::new(2, "unable to read line 2 (NPAR NLINE NITR ...)"));
        }
        let control = Control {
            npar: v[0] as i64,
            nline: v[1] as i64,
            nitr: v[2] as i64,
            nxpar: v[3] as i64,
            thresh: v[4],
            errtst: v[5],
            frac: v[6],
            cal: v[7],
        };

        let mut idx = 2;
        let mut options = Vec::new();
        loop {
            let line = lines
                .get(idx)
                .ok_or_else(|| ParseError::new(idx + 1, "missing option line"))?;
            options.push(line.to_string());
            idx += 1;
            let more = match flavor {
                Flavor::Spinv => SpinvOption::parse(line).vsym < -0.5,
                Flavor::Dpi => false,
            };
            if !more {
                break;
            }
        }

        let mut params = Vec::new();
        let mut id_width = 0;
        while (params.len() as i64) < control.npar && idx < lines.len() {
            let line = lines[idx];
            let Some((id, digit_end, next)) = ParamId::parse_prefix(line) else { break };
            let mut v = [0.0, f64::NAN, 2.0];
            let k = pcard(&line[next..], &mut v);
            if k == 0 || v[2] < 1.01 {
                break; // not a parameter line: covariance or end of list
            }
            let error = if k >= 2 && !v[1].is_nan() { Some(v[1]) } else { None };
            let label = match line.find('/') {
                Some(p) => line[p + 1..].trim_end().to_string(),
                None => String::new(),
            };
            id_width = id_width.max(digit_end);
            params.push(Parameter { id, value: v[0], error, label });
            idx += 1;
        }

        let trailer = lines[idx..].iter().map(|s| s.to_string()).collect();
        Ok(ParFile {
            title,
            control,
            options,
            params,
            trailer,
            id_width: if id_width == 0 { 13 } else { id_width },
            exp_digits: cfmt::detect_exp_digits(text),
        })
    }

    /// Writes the file in SPFIT's format.
    pub fn to_text(&self) -> String {
        let x = self.exp_digits;
        let c = &self.control;
        let mut out = String::new();
        out.push_str(&self.title);
        out.push('\n');
        out.push_str(&format!(
            "{} {} {} {} {} {} {} {}\n",
            cfmt::d(c.npar, 4),
            cfmt::d(c.nline, 4),
            cfmt::d(c.nitr, 4),
            cfmt::d(c.nxpar, 4),
            cfmt::e(c.thresh, 14, 4, x),
            cfmt::e(c.errtst, 14, 4, x),
            cfmt::e(c.frac, 14, 4, x),
            cfmt::f(c.cal, 12, 10),
        ));
        for o in &self.options {
            out.push_str(o);
            out.push('\n');
        }
        for p in &self.params {
            out.push_str(&p.id.to_field(self.id_width));
            out.push(' ');
            out.push_str(&cfmt::e(p.value, 23, 15, x));
            if let Some(err) = p.error {
                out.push(' ');
                out.push_str(&cfmt::e(err, 14, 8, x));
            }
            out.push(' ');
            if !p.label.is_empty() {
                out.push('/');
                out.push_str(&p.label);
            }
            out.push('\n');
        }
        for t in &self.trailer {
            out.push_str(t);
            out.push('\n');
        }
        out
    }

    /// Decodes the option lines of an SPFIT/SPCAT file.
    pub fn spinv_options(&self) -> Vec<SpinvOption> {
        self.options.iter().map(|o| SpinvOption::parse(o)).collect()
    }

    /// Number of independent (non-tied) parameters.
    pub fn n_independent(&self) -> usize {
        self.params.iter().filter(|p| !p.id.is_negative()).count()
    }

    /// The packed upper-triangular matrix that SPFIT writes after the
    /// parameters of a `.var` file (`putvar()`): column by column, element
    /// `(i, j)` with `i <= j` is at index `j*(j+1)/2 + i`, over the
    /// independent parameters. SPFIT stores it divided by the parameter
    /// errors. Returns `None` when the trailer does not hold the full matrix.
    pub fn covariance(&self) -> Option<Vec<f64>> {
        let n = self.n_independent();
        let need = n * (n + 1) / 2;
        let mut vals = Vec::with_capacity(need);
        'lines: for line in &self.trailer {
            let mut v = [f64::NAN; 8];
            let k = pcard(line, &mut v);
            if k == 0 {
                break;
            }
            for &x in &v[..k] {
                if x.is_nan() || x.abs() >= 1.01 {
                    break 'lines;
                }
                vals.push(x);
                if vals.len() == need {
                    break 'lines;
                }
            }
        }
        (vals.len() == need && need > 0).then_some(vals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASYM: &str = "\
SO2-like asymmetric top
   8  500   10    0    0.0000E+000    1.0000E+003    1.0000E+000 1.0000000000
a   1    1   0   50   0   2   1   0     0    1   0
        10000  6.077000000000000E+004 1.00000000E+001 /A
        -1122  1.774079636037830E-001 1.00000000E-037 /-DJK
";

    #[test]
    fn parses_header_options_and_params() {
        let p = ParFile::parse(ASYM, Flavor::Spinv).unwrap();
        assert_eq!(p.control.npar, 8);
        assert_eq!(p.control.errtst, 1000.0);
        assert_eq!(p.options.len(), 1);
        let o = &p.spinv_options()[0];
        assert_eq!(o.chr, Some('a'));
        assert_eq!((o.nvib, o.knmax, o.iax, o.wtpl, o.wtmn, o.ewt), (1, 50, 2, 1, 0, 1));
        assert_eq!(p.params.len(), 2);
        assert_eq!(p.params[0].label, "A");
        assert_eq!(p.params[0].error, Some(10.0));
        assert!(p.params[1].id.is_negative());
        assert_eq!(p.n_independent(), 1);
        assert_eq!(p.id_width, 13);
        assert_eq!(p.exp_digits, 3);
    }

    #[test]
    fn round_trips_spfit_format() {
        let p = ParFile::parse(ASYM, Flavor::Spinv).unwrap();
        assert_eq!(p.to_text(), ASYM);
    }

    #[test]
    fn negative_vsym_continues_option_lines() {
        let text = "t\n 5 10 1\na 1 2 0 10 0 1 1 1 -1\na 1 2 0 10 0 1 1 1 0\n 10000 1.0 1E37 /A\n";
        let p = ParFile::parse(text, Flavor::Spinv).unwrap();
        assert_eq!(p.options.len(), 2);
        assert_eq!(p.params.len(), 1);
    }

    #[test]
    fn dpi_has_one_option_line() {
        let text = "t\n 5 10 1\n    3    1\n    1  3.6916E+06 1E-35 /A\n";
        let p = ParFile::parse(text, Flavor::Dpi).unwrap();
        assert_eq!(p.options, vec!["    3    1"]);
        assert_eq!(p.params[0].id.digits(), "1");
        assert_eq!(p.id_width, 5);
    }

    #[test]
    fn reads_packed_covariance() {
        let text = "t\n 2 10 1\na 1 1\n 10000 1.0 1E37 /A\n 20000 2.0 1E37 /B\n 0.5000000-0.2000000 0.9000000\n";
        let p = ParFile::parse(text, Flavor::Spinv).unwrap();
        assert_eq!(p.params.len(), 2);
        assert_eq!(p.trailer.len(), 1);
        assert_eq!(p.covariance(), Some(vec![0.5, -0.2, 0.9]));
    }

    #[test]
    fn missing_line_2_is_an_error() {
        let err = ParFile::parse("title only\n", Flavor::Spinv).unwrap_err();
        assert_eq!(err.line, 2);
    }
}
