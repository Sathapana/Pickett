//! Peak finding.
//!
//! A peak is a local maximum that is high enough above the noise (and,
//! optionally, prominent enough and far enough from a higher peak). Remove
//! the baseline first ([`crate::baseline`]): heights are measured from zero.

use crate::Spectrum;

/// Settings for [`find_peaks`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakOptions {
    /// Minimum height / noise. Default 5.
    pub min_snr: f64,
    /// Minimum height (absolute), if any.
    pub min_height: Option<f64>,
    /// Minimum prominence (height above the higher of the two minima that
    /// separate the peak from higher ground), if any.
    pub min_prominence: Option<f64>,
    /// Peaks closer than this many points to a higher peak are dropped.
    pub min_distance: usize,
    /// Noise level to use; estimated with [`noise_level`] when `None`.
    pub noise: Option<f64>,
}

impl Default for PeakOptions {
    fn default() -> Self {
        PeakOptions { min_snr: 5.0, min_height: None, min_prominence: None, min_distance: 1, noise: None }
    }
}

/// A peak found by [`find_peaks`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    /// Index of the highest point.
    pub index: usize,
    /// Centre from a parabola through the highest point and its neighbours
    /// (better than the point spacing). Use [`crate::fit`] for the best value.
    pub freq: f64,
    pub height: f64,
    pub prominence: f64,
    /// `height / noise`.
    pub snr: f64,
    /// Width at half height from linear interpolation; NaN if a side never
    /// drops below half height.
    pub fwhm: f64,
}

/// Robust noise estimate: the median absolute deviation of the
/// point-to-point differences, scaled to a Gaussian standard deviation
/// (`1.4826 · MAD / √2`). Lines barely affect it.
pub fn noise_level(y: &[f64]) -> f64 {
    let mut d: Vec<f64> = y.windows(2).map(|w| w[1] - w[0]).collect();
    if d.is_empty() {
        return 0.0;
    }
    let med = median(&mut d);
    let mut dev: Vec<f64> = d.iter().map(|v| (v - med).abs()).collect();
    1.4826 * median(&mut dev) / std::f64::consts::SQRT_2
}

fn median(v: &mut [f64]) -> f64 {
    let mid = v.len() / 2;
    v.select_nth_unstable_by(mid, f64::total_cmp);
    v[mid]
}

/// Finds the peaks of `spec`, sorted by frequency.
pub fn find_peaks(spec: &Spectrum, opts: &PeakOptions) -> Vec<Peak> {
    let (f, y) = (&spec.freq, &spec.intensity);
    let n = y.len();
    if n < 3 {
        return Vec::new();
    }
    let noise = opts.noise.unwrap_or_else(|| noise_level(y));
    let threshold = (opts.min_snr * noise).max(opts.min_height.unwrap_or(f64::NEG_INFINITY));

    let mut peaks = Vec::new();
    let mut i = 1;
    while i < n - 1 {
        if y[i] > y[i - 1] && y[i] >= y[i + 1] && y[i] > threshold {
            // a flat top counts once: take its middle
            let mut j = i;
            while j + 1 < n && y[j + 1] == y[i] {
                j += 1;
            }
            if j + 1 < n {
                let top = (i + j) / 2;
                let prominence = prominence(y, top);
                if opts.min_prominence.is_none_or(|p| prominence >= p) {
                    peaks.push(Peak {
                        index: top,
                        freq: parabolic_centre(f, y, top),
                        height: y[top],
                        prominence,
                        snr: if noise > 0.0 { y[top] / noise } else { f64::INFINITY },
                        fwhm: half_width(f, y, top),
                    });
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    if opts.min_distance > 1 && peaks.len() > 1 {
        // keep higher peaks first, drop lower ones that are too close
        let mut order: Vec<usize> = (0..peaks.len()).collect();
        order.sort_by(|&a, &b| peaks[b].height.total_cmp(&peaks[a].height));
        let mut keep = vec![true; peaks.len()];
        for (k, &a) in order.iter().enumerate() {
            if !keep[a] {
                continue;
            }
            for &b in &order[k + 1..] {
                if keep[b] && peaks[a].index.abs_diff(peaks[b].index) < opts.min_distance {
                    keep[b] = false;
                }
            }
        }
        peaks = peaks.into_iter().zip(keep).filter_map(|(p, k)| k.then_some(p)).collect();
    }
    peaks
}

/// Height above the higher of the lowest points between the peak and the
/// nearest higher point on each side (or the ends).
fn prominence(y: &[f64], i: usize) -> f64 {
    let h = y[i];
    let mut left_min = h;
    for &v in y[..i].iter().rev() {
        if v > h {
            break;
        }
        left_min = left_min.min(v);
    }
    let mut right_min = h;
    for &v in &y[i + 1..] {
        if v > h {
            break;
        }
        right_min = right_min.min(v);
    }
    h - left_min.max(right_min)
}

fn parabolic_centre(f: &[f64], y: &[f64], i: usize) -> f64 {
    let (a, b, c) = (y[i - 1], y[i], y[i + 1]);
    let den = a - 2.0 * b + c;
    if den == 0.0 {
        return f[i];
    }
    let delta = (0.5 * (a - c) / den).clamp(-0.5, 0.5);
    if delta >= 0.0 {
        f[i] + delta * (f[i + 1] - f[i])
    } else {
        f[i] + delta * (f[i] - f[i - 1])
    }
}

fn half_width(f: &[f64], y: &[f64], i: usize) -> f64 {
    let half = y[i] / 2.0;
    let mut l = i;
    while l > 0 && y[l] > half {
        l -= 1;
    }
    let mut r = i;
    while r + 1 < y.len() && y[r] > half {
        r += 1;
    }
    if y[l] > half || y[r] > half {
        return f64::NAN;
    }
    let cross = |a: usize, b: usize| f[a] + (half - y[a]) * (f[b] - f[a]) / (y[b] - y[a]);
    cross(r - 1, r) - cross(l, l + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{profile::Profile, synth};

    fn spec(lines: &[(f64, f64)], fwhm: f64) -> Spectrum {
        let grid = synth::grid(0.0, 100.0, 0.05);
        let y = synth::synthesize(lines.iter().copied(), &grid, Profile::Gaussian { fwhm }, 30.0);
        Spectrum::new(grid, y).unwrap()
    }

    #[test]
    fn finds_lines_with_sub_bin_accuracy() {
        let s = spec(&[(20.013, 1.0), (50.5, 0.3), (80.031, 2.0)], 0.4);
        let p = find_peaks(&s, &PeakOptions { noise: Some(0.01), ..Default::default() });
        assert_eq!(p.len(), 3);
        for (pk, want) in p.iter().zip([20.013, 50.5, 80.031]) {
            assert!((pk.freq - want).abs() < 0.003, "{} vs {want}", pk.freq);
            assert!((pk.fwhm - 0.4).abs() < 0.02, "fwhm {}", pk.fwhm);
        }
        assert!((p[1].height - 0.3).abs() < 1e-3);
    }

    #[test]
    fn snr_prominence_and_distance_filters() {
        let s = spec(&[(20.0, 1.0), (21.0, 0.6), (60.0, 0.02)], 0.4);
        let all = find_peaks(&s, &PeakOptions { noise: Some(0.001), ..Default::default() });
        assert_eq!(all.len(), 3);
        let strong = find_peaks(&s, &PeakOptions { noise: Some(0.01), ..Default::default() });
        assert_eq!(strong.len(), 2); // 0.02 / 0.01 = SNR 2 < 5
        let spaced = find_peaks(&s, &PeakOptions { noise: Some(0.001), min_distance: 40, ..Default::default() });
        assert_eq!(spaced.iter().map(|p| p.freq.round()).collect::<Vec<_>>(), vec![20.0, 60.0]);
        let prominent = find_peaks(&s, &PeakOptions { noise: Some(0.001), min_prominence: Some(0.1), ..Default::default() });
        assert_eq!(prominent.len(), 2);
    }

    #[test]
    fn noise_estimate_ignores_lines() {
        // pseudo-random noise with sd 0.1 plus a strong line
        let mut x: u64 = 12345;
        let mut y: Vec<f64> = (0..20000)
            .map(|_| {
                let mut u = || { x ^= x << 13; x ^= x >> 7; x ^= x << 17; (x >> 11) as f64 / (1u64 << 53) as f64 };
                let (a, b) = (u(), u());
                0.1 * (-2.0 * a.max(1e-300).ln()).sqrt() * (std::f64::consts::TAU * b).cos()
            })
            .collect();
        for (i, v) in y.iter_mut().enumerate() {
            *v += 50.0 * (-((i as f64 - 9000.0) / 5.0).powi(2)).exp();
        }
        let s = noise_level(&y);
        assert!((s - 0.1).abs() < 0.01, "noise {s}");
    }
}
