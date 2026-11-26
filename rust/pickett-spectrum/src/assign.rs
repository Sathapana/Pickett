//! Matching measured line centres to predicted lines (an SPCAT catalog),
//! and writing the matches as a `.lin` file for SPFIT.

use pickett_io::{CatLine, LinFile, LinLine};

/// Settings for [`match_lines`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchOptions {
    /// Largest |measured − predicted| accepted (same unit as the frequencies).
    pub tolerance: f64,
    /// Predicted lines weaker than this log₁₀ intensity are ignored.
    pub min_lgint: f64,
}

impl Default for MatchOptions {
    fn default() -> Self {
        MatchOptions { tolerance: 1.0, min_lgint: f64::NEG_INFINITY }
    }
}

/// One measured line matched to one predicted line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Assignment {
    /// Index into the measured frequencies.
    pub observed: usize,
    /// Index into the predicted lines.
    pub predicted: usize,
    pub obs: f64,
    pub calc: f64,
    /// `obs − calc`.
    pub residual: f64,
    /// Another predicted line, with different quanta, is also within the
    /// tolerance of this measurement. Such matches may be wrong, or the
    /// measurement may be a blend; check them before fitting.
    pub ambiguous: bool,
}

/// Matches measured frequencies to predicted lines.
///
/// Every (measured, predicted) pair closer than the tolerance is a
/// candidate. Candidates are taken closest first, and each measurement and
/// each predicted line is used at most once. The result is sorted by
/// measured frequency. Predicted lines with exactly the same frequency (for
/// example K-degenerate pairs) count as one line, and the first is used.
pub fn match_lines(observed: &[f64], predicted: &[CatLine], opts: &MatchOptions) -> Vec<Assignment> {
    let mut order: Vec<usize> = (0..predicted.len()).filter(|&i| predicted[i].lgint >= opts.min_lgint).collect();
    order.sort_by(|&a, &b| predicted[a].freq.total_cmp(&predicted[b].freq));
    let freqs: Vec<f64> = order.iter().map(|&i| predicted[i].freq).collect();

    struct Cand {
        obs: usize,
        pred: usize,
        dist: f64,
        ambiguous: bool,
    }
    let mut cands = Vec::new();
    for (o, &fo) in observed.iter().enumerate() {
        let a = freqs.partition_point(|&f| f < fo - opts.tolerance);
        let b = freqs.partition_point(|&f| f <= fo + opts.tolerance);
        let mut distinct: Vec<f64> = freqs[a..b].to_vec();
        distinct.dedup();
        let ambiguous = distinct.len() > 1;
        let mut last = f64::NAN;
        for k in a..b {
            if freqs[k] == last {
                continue; // identical frequency: same line for matching
            }
            last = freqs[k];
            cands.push(Cand { obs: o, pred: order[k], dist: (fo - freqs[k]).abs(), ambiguous });
        }
    }
    cands.sort_by(|x, y| x.dist.total_cmp(&y.dist));
    let mut used_obs = vec![false; observed.len()];
    let mut used_pred = vec![false; predicted.len()];
    let mut out = Vec::new();
    for c in cands {
        if used_obs[c.obs] || used_pred[c.pred] {
            continue;
        }
        used_obs[c.obs] = true;
        used_pred[c.pred] = true;
        let (obs, calc) = (observed[c.obs], predicted[c.pred].freq);
        out.push(Assignment { observed: c.obs, predicted: c.pred, obs, calc, residual: obs - calc, ambiguous: c.ambiguous });
    }
    out.sort_by(|a, b| a.obs.total_cmp(&b.obs));
    out
}

/// A `.lin` file of the assignments: the predicted line's quanta (upper,
/// then lower), the measured frequency and `err` for each. Pass a filtered
/// list (for example without [`Assignment::ambiguous`] ones) to leave
/// matches out; `errs`, when given, holds one error per assignment
/// (for example the fitted centre uncertainties).
pub fn to_lin(assignments: &[Assignment], predicted: &[CatLine], err: f64, errs: Option<&[f64]>) -> LinFile {
    let nqn = assignments.iter().map(|a| predicted[a.predicted].nqn()).max().unwrap_or(1);
    let qn_fields = if nqn <= 6 { 12 } else { 2 * nqn };
    let lines = assignments
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let p = &predicted[a.predicted];
            let mut qn: Vec<Option<i32>> = vec![None; qn_fields];
            for (j, &q) in p.upper.iter().enumerate().take(nqn) {
                qn[j] = Some(q);
            }
            for (j, &q) in p.lower.iter().enumerate().take(nqn) {
                qn[nqn + j] = Some(q);
            }
            let e = errs.and_then(|v| v.get(k).copied()).unwrap_or(err);
            LinLine { qn, freq: a.obs, err: e, wt: 1.0 }
        })
        .collect();
    LinFile { lines, qn_fields, exp_digits: 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(freq: f64, lgint: f64, j: i32) -> CatLine {
        CatLine {
            freq, err: 0.0, lgint, dr: 3, elo: 0.0, gup: 3, tag: 1, qnfmt: 303,
            upper: vec![j + 1, 0, j + 1], lower: vec![j, 0, j],
        }
    }

    #[test]
    fn closest_first_and_each_used_once() {
        let pred = vec![line(100.0, -3.0, 1), line(100.6, -3.0, 2), line(200.0, -3.0, 3)];
        // two measurements near 100: the closer one gets line 0, the other line 1
        let m = match_lines(&[100.05, 100.4, 199.2, 300.0], &pred, &MatchOptions { tolerance: 1.0, ..Default::default() });
        assert_eq!(m.len(), 3);
        assert_eq!((m[0].observed, m[0].predicted), (0, 0));
        assert_eq!((m[1].observed, m[1].predicted), (1, 1));
        assert!(m[0].ambiguous && m[1].ambiguous);
        assert_eq!((m[2].predicted, m[2].ambiguous), (2, false));
        assert!((m[2].residual + 0.8).abs() < 1e-12);
    }

    #[test]
    fn weak_lines_and_identical_frequencies() {
        let pred = vec![line(100.0, -8.0, 1), line(100.5, -3.0, 2), line(100.5, -3.0, 9)];
        let m = match_lines(&[100.1], &pred, &MatchOptions { tolerance: 1.0, min_lgint: -5.0 });
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].predicted, 1);
        assert!(!m[0].ambiguous, "same-frequency lines are one line");
    }

    #[test]
    fn writes_lin_with_quanta() {
        let pred = vec![line(100.0, -3.0, 4)];
        let m = match_lines(&[100.02], &pred, &MatchOptions::default());
        let lin = to_lin(&m, &pred, 0.05, None);
        assert_eq!(lin.lines[0].upper(3), vec![5, 0, 5]);
        assert_eq!(lin.lines[0].lower(3), vec![4, 0, 4]);
        assert_eq!((lin.lines[0].freq, lin.lines[0].err), (100.02, 0.05));
        let back = LinFile::parse(&lin.to_text()).unwrap();
        assert_eq!(back.lines[0].upper(3), vec![5, 0, 5]);
        let lin2 = to_lin(&m, &pred, 0.05, Some(&[0.01]));
        assert_eq!(lin2.lines[0].err, 0.01);
    }
}
