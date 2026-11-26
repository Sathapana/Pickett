//! Savitzky–Golay smoothing.
//!
//! Each point is replaced by the value at that point of a least-squares
//! polynomial of degree `order` fitted to the `2*half_window + 1` points
//! around it. This keeps line positions and heights much better than a
//! moving average. Near the ends, the polynomial of the first/last full
//! window is evaluated at the point's position, so no padding is invented.
//! The spacing of the points is assumed uniform.

use crate::linalg::solve;

/// Weights that give the value at offset `t` (−m ≤ t ≤ m from the window
/// centre) of the degree-`order` least-squares polynomial through the
/// `2m+1` points of the window.
fn weights(m: usize, order: usize, t: f64) -> Vec<f64> {
    let np = order + 1;
    let xs: Vec<f64> = (0..=2 * m).map(|j| j as f64 - m as f64).collect();
    // normal matrix A^T A with A[j][k] = x_j^k
    let mut ata = vec![0.0; np * np];
    for &x in &xs {
        for r in 0..np {
            for c in 0..np {
                ata[r * np + c] += x.powi((r + c) as i32);
            }
        }
    }
    // solve (A^T A) g = [1, t, t^2, ...]; weights are A g
    let mut g: Vec<f64> = (0..np).map(|k| t.powi(k as i32)).collect();
    solve(&mut ata, &mut g, np).expect("window larger than the polynomial order");
    xs.iter().map(|&x| (0..np).map(|k| g[k] * x.powi(k as i32)).sum()).collect()
}

/// Smooths `y` with a Savitzky–Golay filter of half-width `half_window`
/// (window `2*half_window + 1` points) and polynomial degree `order`.
///
/// Requires `order < 2*half_window + 1`. If `y` is shorter than the
/// window, it is returned unchanged.
pub fn savitzky_golay(y: &[f64], half_window: usize, order: usize) -> Vec<f64> {
    let m = half_window;
    let w = 2 * m + 1;
    assert!(order < w, "polynomial order must be smaller than the window ({w} points)");
    let n = y.len();
    if n < w || m == 0 {
        return y.to_vec();
    }
    let dot = |c: &[f64], start: usize| -> f64 { c.iter().zip(&y[start..start + w]).map(|(a, b)| a * b).sum() };
    let centre = weights(m, order, 0.0);
    let mut out = vec![0.0; n];
    for i in m..n - m {
        out[i] = dot(&centre, i - m);
    }
    for i in 0..m {
        // first window: points 0..w, point i is at offset i - m
        out[i] = dot(&weights(m, order, i as f64 - m as f64), 0);
        // last window: points n-w..n, point n-1-i is at offset m - i
        out[n - 1 - i] = dot(&weights(m, order, m as f64 - i as f64), n - w);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_quadratic_weights() {
        // classic 5-point quadratic/cubic smoothing weights: (-3, 12, 17, 12, -3)/35
        let w = weights(2, 2, 0.0);
        for (a, b) in w.iter().zip([-3.0, 12.0, 17.0, 12.0, -3.0]) {
            assert!((a - b / 35.0).abs() < 1e-12, "{w:?}");
        }
    }

    #[test]
    fn keeps_polynomials_of_its_order_exactly_including_edges() {
        let y: Vec<f64> = (0..40).map(|i| { let x = i as f64; 2.0 - 0.5 * x + 0.03 * x * x - 0.001 * x * x * x }).collect();
        let s = savitzky_golay(&y, 4, 3);
        for (a, b) in s.iter().zip(&y) {
            assert!((a - b).abs() < 1e-9, "{a} vs {b}");
        }
    }

    #[test]
    fn reduces_noise() {
        // deterministic pseudo-noise on a constant
        let y: Vec<f64> = (0..2000).map(|i| (i * 7919 % 1000) as f64 / 1000.0 - 0.5).collect();
        let s = savitzky_golay(&y, 10, 2);
        let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
        assert!(rms(&s[20..1980]) < 0.3 * rms(&y), "{} vs {}", rms(&s), rms(&y));
    }

    #[test]
    fn short_input_is_unchanged() {
        assert_eq!(savitzky_golay(&[1.0, 2.0], 3, 2), vec![1.0, 2.0]);
    }
}
