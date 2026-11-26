//! Small dense linear algebra for the fits (matrices of a few rows).

/// Solves `a x = b` in place for a small square matrix (Gaussian
/// elimination with partial pivoting). `a` is row-major `n × n`.
/// Returns `None` when the matrix is singular.
pub(crate) fn solve(a: &mut [f64], b: &mut [f64], n: usize) -> Option<()> {
    for col in 0..n {
        let piv = (col..n).max_by(|&i, &j| a[i * n + col].abs().total_cmp(&a[j * n + col].abs()))?;
        if a[piv * n + col].abs() < 1e-300 {
            return None;
        }
        if piv != col {
            for k in 0..n {
                a.swap(piv * n + k, col * n + k);
            }
            b.swap(piv, col);
        }
        for r in col + 1..n {
            let f = a[r * n + col] / a[col * n + col];
            if f != 0.0 {
                for k in col..n {
                    a[r * n + k] -= f * a[col * n + k];
                }
                b[r] -= f * b[col];
            }
        }
    }
    for col in (0..n).rev() {
        let mut s = b[col];
        for k in col + 1..n {
            s -= a[col * n + k] * b[k];
        }
        b[col] = s / a[col * n + col];
    }
    Some(())
}

/// Inverse of a small square matrix, or `None` if singular.
pub(crate) fn inverse(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut inv = vec![0.0; n * n];
    for j in 0..n {
        let mut m = a.to_vec();
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        solve(&mut m, &mut e, n)?;
        for i in 0..n {
            inv[i * n + j] = e[i];
        }
    }
    Some(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_and_inverts() {
        let a = [2.0, 1.0, 1.0, 3.0];
        let mut m = a;
        let mut b = [3.0, 5.0];
        solve(&mut m, &mut b, 2).unwrap();
        assert!((b[0] - 0.8).abs() < 1e-12 && (b[1] - 1.4).abs() < 1e-12);
        let inv = inverse(&a, 2).unwrap();
        assert!((inv[0] - 0.6).abs() < 1e-12 && (inv[3] - 0.4).abs() < 1e-12);
        assert!(inverse(&[1.0, 2.0, 2.0, 4.0], 2).is_none());
    }
}
