//! Baseline estimation by asymmetric least squares (AsLS), after
//! P. H. C. Eilers and H. F. M. Boelens, "Baseline correction with
//! asymmetric least squares smoothing" (2005).
//!
//! The baseline `z` minimises
//! `Σ wᵢ (yᵢ − zᵢ)² + λ Σ (Δ²zᵢ)²`. Points above the current baseline get
//! the small weight `p`, points below get `1 − p`, so the baseline follows
//! the bottom of the spectrum and ignores the lines. A few re-weighting
//! iterations are enough. Each solve is O(n): the system is pentadiagonal
//! and is factorised as `L D Lᵀ`.

/// Settings for [`asls`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AslsOptions {
    /// Smoothness: larger values give a stiffer baseline. Scales roughly
    /// with (baseline wiggle length in points)⁴; 1e4–1e9 is typical.
    pub lambda: f64,
    /// Asymmetry: weight of points above the baseline (0.001–0.05).
    pub p: f64,
    /// Re-weighting iterations.
    pub iterations: usize,
}

impl Default for AslsOptions {
    fn default() -> Self {
        AslsOptions {
            lambda: 1e6,
            p: 0.01,
            iterations: 10,
        }
    }
}

/// The AsLS baseline of `y` (same length). Subtract it with
/// [`crate::Spectrum::minus`].
pub fn asls(y: &[f64], opts: &AslsOptions) -> Vec<f64> {
    let n = y.len();
    if n < 3 {
        return y.to_vec();
    }
    // D^T D for the second-difference matrix D ((n-2) x n), as three bands
    let mut d0 = vec![6.0; n];
    let mut d1 = vec![-4.0; n - 1];
    let d2 = vec![1.0; n - 2];
    d0[0] = 1.0;
    d0[n - 1] = 1.0;
    if n > 3 {
        d0[1] = 5.0;
        d0[n - 2] = 5.0;
    } else {
        d0[1] = 4.0;
    }
    d1[0] = -2.0;
    d1[n - 2] = -2.0;

    let mut w = vec![1.0; n];
    let mut z = y.to_vec();
    for _ in 0..opts.iterations.max(1) {
        let a0: Vec<f64> = (0..n).map(|i| w[i] + opts.lambda * d0[i]).collect();
        let a1: Vec<f64> = d1.iter().map(|v| opts.lambda * v).collect();
        let a2: Vec<f64> = d2.iter().map(|v| opts.lambda * v).collect();
        let b: Vec<f64> = (0..n).map(|i| w[i] * y[i]).collect();
        z = solve_penta(&a0, &a1, &a2, &b);
        let mut changed = false;
        for i in 0..n {
            let nw = if y[i] > z[i] { opts.p } else { 1.0 - opts.p };
            changed |= nw != w[i];
            w[i] = nw;
        }
        if !changed {
            break;
        }
    }
    z
}

/// Solves a symmetric positive-definite pentadiagonal system with
/// diagonal `a0`, first off-diagonal `a1` and second off-diagonal `a2`.
///
/// Accuracy: the AsLS matrix has a condition number of about
/// `6λ/min(w)`, so the baseline is accurate to roughly
/// `1e-16 · 6λ/min(w)` relative (about 1e-8 for λ = 1e6, p = 0.01). That is
/// far below spectral noise; iterative refinement does not improve it.
fn solve_penta(a0: &[f64], a1: &[f64], a2: &[f64], b: &[f64]) -> Vec<f64> {
    factor_penta(a0, a1, a2).solve(b)
}

/// `A = L D Lᵀ` with unit lower bands `l1`, `l2`.
struct Penta {
    d: Vec<f64>,
    l1: Vec<f64>,
    l2: Vec<f64>,
}

fn factor_penta(a0: &[f64], a1: &[f64], a2: &[f64]) -> Penta {
    let n = a0.len();
    let (mut d, mut l1, mut l2) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for i in 0..n {
        let mut di = a0[i];
        if i >= 1 {
            di -= l1[i - 1] * l1[i - 1] * d[i - 1];
        }
        if i >= 2 {
            di -= l2[i - 2] * l2[i - 2] * d[i - 2];
        }
        d[i] = di;
        if i + 1 < n {
            let mut v = a1[i];
            if i >= 1 {
                v -= l2[i - 1] * d[i - 1] * l1[i - 1];
            }
            l1[i] = v / di;
        }
        if i + 2 < n {
            l2[i] = a2[i] / di;
        }
    }
    Penta { d, l1, l2 }
}

impl Penta {
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let (d, l1, l2) = (&self.d, &self.l1, &self.l2);
        let n = d.len();
        let mut x = b.to_vec();
        for i in 0..n {
            if i >= 1 {
                x[i] -= l1[i - 1] * x[i - 1];
            }
            if i >= 2 {
                x[i] -= l2[i - 2] * x[i - 2];
            }
        }
        for i in 0..n {
            x[i] /= d[i];
        }
        for i in (0..n).rev() {
            if i + 1 < n {
                x[i] -= l1[i] * x[i + 1];
            }
            if i + 2 < n {
                x[i] -= l2[i] * x[i + 2];
            }
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pentadiagonal_solver_matches_dense() {
        let n = 7;
        let a0: Vec<f64> = (0..n).map(|i| 10.0 + i as f64).collect();
        let a1: Vec<f64> = (0..n - 1).map(|i| -1.0 - 0.1 * i as f64).collect();
        let a2: Vec<f64> = (0..n - 2).map(|i| 0.5 + 0.05 * i as f64).collect();
        let x_true: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
        let mut b = vec![0.0; n];
        for i in 0..n {
            b[i] += a0[i] * x_true[i];
            if i + 1 < n {
                b[i] += a1[i] * x_true[i + 1];
                b[i + 1] += a1[i] * x_true[i];
            }
            if i + 2 < n {
                b[i] += a2[i] * x_true[i + 2];
                b[i + 2] += a2[i] * x_true[i];
            }
        }
        let x = solve_penta(&a0, &a1, &a2, &b);
        for (a, t) in x.iter().zip(&x_true) {
            assert!((a - t).abs() < 1e-12);
        }
    }

    #[test]
    fn follows_baseline_under_lines() {
        let n = 3000;
        let base = |i: usize| 5.0 + 0.002 * i as f64 + 0.5 * (i as f64 / 400.0).sin();
        let mut y: Vec<f64> = (0..n).map(base).collect();
        for c in [500.0, 1200.0, 1210.0, 2500.0] {
            for (i, v) in y.iter_mut().enumerate() {
                *v += 3.0 * (-((i as f64 - c) / 4.0).powi(2)).exp();
            }
        }
        let z = asls(
            &y,
            &AslsOptions {
                lambda: 1e7,
                p: 0.005,
                iterations: 20,
            },
        );
        let worst = (0..n).map(|i| (z[i] - base(i)).abs()).fold(0.0, f64::max);
        assert!(worst < 0.05, "max baseline error {worst}");
    }

    #[test]
    fn tiny_inputs() {
        assert_eq!(asls(&[1.0, 2.0], &AslsOptions::default()), vec![1.0, 2.0]);
        let z = asls(&[1.0, 1.0, 1.0], &AslsOptions::default());
        // conditioning limits the accuracy to ~1e-16 * 6e6 / 0.99 (see solve_penta)
        assert!(z.iter().all(|v| (v - 1.0).abs() < 1e-7), "{z:?}");
    }
}
