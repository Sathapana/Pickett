use std::fmt;

/// A measured or simulated spectrum: intensities on ascending frequencies.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spectrum {
    pub freq: Vec<f64>,
    pub intensity: Vec<f64>,
}

/// A problem with spectrum data. `line` is 1-based (0 when not about a line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectrumError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for SpectrumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            f.write_str(&self.message)
        }
    }
}

impl std::error::Error for SpectrumError {}

fn err(line: usize, message: impl Into<String>) -> SpectrumError {
    SpectrumError { line, message: message.into() }
}

impl Spectrum {
    /// A spectrum from two columns. The points are sorted by frequency if
    /// they are not already; every value must be finite.
    pub fn new(freq: Vec<f64>, intensity: Vec<f64>) -> Result<Spectrum, SpectrumError> {
        if freq.len() != intensity.len() {
            return Err(err(0, format!("{} frequencies but {} intensities", freq.len(), intensity.len())));
        }
        if let Some(i) = freq.iter().zip(&intensity).position(|(f, y)| !f.is_finite() || !y.is_finite()) {
            return Err(err(0, format!("point {} is not finite", i + 1)));
        }
        let mut s = Spectrum { freq, intensity };
        if !s.freq.windows(2).all(|w| w[0] <= w[1]) {
            let mut idx: Vec<usize> = (0..s.freq.len()).collect();
            idx.sort_by(|&a, &b| s.freq[a].total_cmp(&s.freq[b]));
            s = Spectrum {
                freq: idx.iter().map(|&i| s.freq[i]).collect(),
                intensity: idx.iter().map(|&i| s.intensity[i]).collect(),
            };
        }
        Ok(s)
    }

    /// Reads the first two numeric columns of a text file (frequency,
    /// intensity). See [`Spectrum::parse_columns`].
    pub fn parse(text: &str) -> Result<Spectrum, SpectrumError> {
        Self::parse_columns(text, 0, 1)
    }

    /// Reads columns `fcol` (frequency) and `icol` (intensity), counted from 0.
    ///
    /// Columns may be separated by spaces, tabs, commas or semicolons.
    /// Lines that start with `#`, `%`, `!` or `;`, blank lines, and header
    /// lines before the first data line are skipped. After data has started,
    /// a line that cannot be read is an error.
    pub fn parse_columns(text: &str, fcol: usize, icol: usize) -> Result<Spectrum, SpectrumError> {
        let need = fcol.max(icol) + 1;
        let (mut freq, mut intensity) = (Vec::new(), Vec::new());
        let mut started = false;
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(['#', '%', '!', ';']) {
                continue;
            }
            let fields: Vec<&str> = line.split([' ', '\t', ',', ';']).filter(|t| !t.is_empty()).collect();
            let get = |c: usize| fields.get(c).and_then(|t| t.parse::<f64>().ok());
            match (get(fcol), get(icol)) {
                (Some(f), Some(y)) if fields.len() >= need => {
                    freq.push(f);
                    intensity.push(y);
                    started = true;
                }
                _ if !started => continue, // header
                _ => return Err(err(n + 1, format!("cannot read columns {} and {} of {line:?}", fcol + 1, icol + 1))),
            }
        }
        if freq.is_empty() {
            return Err(err(0, "no data lines"));
        }
        Spectrum::new(freq, intensity)
    }

    /// Two columns, frequency and intensity, separated by a space; numbers
    /// use the shortest text that reads back to the same value.
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity(self.len() * 24);
        for (f, y) in self.freq.iter().zip(&self.intensity) {
            s.push_str(&format!("{f} {y}\n"));
        }
        s
    }

    pub fn len(&self) -> usize {
        self.freq.len()
    }

    pub fn is_empty(&self) -> bool {
        self.freq.is_empty()
    }

    /// The median point spacing, or `None` for fewer than two points.
    pub fn step(&self) -> Option<f64> {
        let mut d: Vec<f64> = self.freq.windows(2).map(|w| w[1] - w[0]).collect();
        if d.is_empty() {
            return None;
        }
        d.sort_by(f64::total_cmp);
        Some(d[d.len() / 2])
    }

    /// Index of the point nearest to `f` (`None` when empty).
    pub fn nearest(&self, f: f64) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        let i = self.freq.partition_point(|&x| x < f);
        Some(match i {
            0 => 0,
            i if i >= self.len() => self.len() - 1,
            i if (self.freq[i] - f).abs() < (f - self.freq[i - 1]).abs() => i,
            i => i - 1,
        })
    }

    /// Index range of the points with `lo <= freq <= hi`.
    pub fn index_range(&self, lo: f64, hi: f64) -> std::ops::Range<usize> {
        let a = self.freq.partition_point(|&x| x < lo);
        let b = self.freq.partition_point(|&x| x <= hi);
        a..b.max(a)
    }

    /// A copy of the part with `lo <= freq <= hi`.
    pub fn slice(&self, lo: f64, hi: f64) -> Spectrum {
        let r = self.index_range(lo, hi);
        Spectrum { freq: self.freq[r.clone()].to_vec(), intensity: self.intensity[r].to_vec() }
    }

    /// The same frequencies with `intensity - other` (for example a baseline).
    pub fn minus(&self, other: &[f64]) -> Spectrum {
        assert_eq!(other.len(), self.len(), "length mismatch");
        Spectrum {
            freq: self.freq.clone(),
            intensity: self.intensity.iter().zip(other).map(|(y, b)| y - b).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_csv_with_header_and_comments() {
        let t = "# measured 2026\nfreq,intensity,phase\n100.0,1.5,0\n100.1, 2.5 ,0\n\n% end\n100.2;3.5;0\n";
        let s = Spectrum::parse(t).unwrap();
        assert_eq!(s.freq, vec![100.0, 100.1, 100.2]);
        assert_eq!(s.intensity, vec![1.5, 2.5, 3.5]);
    }

    #[test]
    fn other_columns_and_sorting() {
        let s = Spectrum::parse_columns("1 9 200\n2 8 100\n3 7 150\n", 2, 0).unwrap();
        assert_eq!(s.freq, vec![100.0, 150.0, 200.0]);
        assert_eq!(s.intensity, vec![2.0, 3.0, 1.0]);
    }

    #[test]
    fn bad_line_after_data_is_an_error() {
        let e = Spectrum::parse("1 2\n3 x\n").unwrap_err();
        assert_eq!(e.line, 2);
        assert!(Spectrum::parse("only text\n").is_err());
        assert!(Spectrum::new(vec![1.0], vec![f64::NAN]).is_err());
    }

    #[test]
    fn helpers() {
        let s = Spectrum::new(vec![0.0, 1.0, 2.0, 3.0], vec![0.0, 1.0, 4.0, 9.0]).unwrap();
        assert_eq!(s.step(), Some(1.0));
        assert_eq!(s.nearest(1.4), Some(1));
        assert_eq!(s.nearest(1.6), Some(2));
        assert_eq!(s.nearest(-5.0), Some(0));
        assert_eq!(s.index_range(0.5, 2.0), 1..3);
        assert_eq!(s.slice(0.5, 2.0).intensity, vec![1.0, 4.0]);
        assert_eq!(Spectrum::parse(&s.to_text()).unwrap(), s);
        assert_eq!(s.minus(&[0.0, 1.0, 1.0, 1.0]).intensity, vec![0.0, 0.0, 3.0, 8.0]);
    }
}
