//! Synthetic spectra from line lists or SPCAT catalogs.

use pickett_io::CatLine;

use crate::{profile::Profile, Spectrum};

/// Evenly spaced frequencies from `fmin` to `fmax` (inclusive when it falls
/// on the grid).
pub fn grid(fmin: f64, fmax: f64, step: f64) -> Vec<f64> {
    assert!(step > 0.0 && fmax >= fmin, "need step > 0 and fmax >= fmin");
    let n = ((fmax - fmin) / step + 1e-9).floor() as usize + 1;
    (0..n).map(|i| fmin + i as f64 * step).collect()
}

/// Sums `profile`, scaled to each line's peak intensity, on the ascending
/// frequencies `grid`. Each line is drawn out to `cutoff` FWHM on either
/// side.
pub fn synthesize(
    lines: impl IntoIterator<Item = (f64, f64)>,
    grid: &[f64],
    profile: Profile,
    cutoff: f64,
) -> Vec<f64> {
    let mut y = vec![0.0; grid.len()];
    let reach = cutoff * profile.fwhm();
    for (f0, amp) in lines {
        let a = grid.partition_point(|&x| x < f0 - reach);
        let b = grid.partition_point(|&x| x <= f0 + reach);
        for i in a..b {
            y[i] += amp * profile.value(grid[i] - f0);
        }
    }
    y
}

/// The lines of a catalog between `fmin` and `fmax` as `(frequency,
/// intensity)`, with intensity `10^LGINT` (nm² MHz).
pub fn catalog_lines(lines: &[CatLine], fmin: f64, fmax: f64) -> Vec<(f64, f64)> {
    lines
        .iter()
        .filter(|l| l.freq >= fmin && l.freq <= fmax)
        .map(|l| (l.freq, 10f64.powf(l.lgint)))
        .collect()
}

/// A spectrum of the catalog lines on a grid from `fmin` to `fmax`. With
/// `peak_scale` the integrated intensities are turned into peak heights:
/// each line's height is `10^LGINT / profile.area()`.
pub fn from_catalog(lines: &[CatLine], fmin: f64, fmax: f64, step: f64, profile: Profile, peak_scale: bool) -> Spectrum {
    let g = grid(fmin, fmax, step);
    let scale = if peak_scale { 1.0 / profile.area() } else { 1.0 };
    let reach = 30.0 * profile.fwhm();
    let src = catalog_lines(lines, fmin - reach, fmax + reach).into_iter().map(|(f, a)| (f, a * scale));
    let y = synthesize(src, &g, profile, 30.0);
    Spectrum { freq: g, intensity: y }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_line_height() {
        let g = grid(10.0, 11.0, 0.25);
        assert_eq!(g, vec![10.0, 10.25, 10.5, 10.75, 11.0]);
        let y = synthesize([(10.5, 2.0)], &g, Profile::Lorentzian { fwhm: 0.5 }, 50.0);
        assert!((y[2] - 2.0).abs() < 1e-12);
        assert!((y[1] - 1.0).abs() < 1e-12 && (y[3] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn catalog_scaling_preserves_area() {
        let line = CatLine {
            freq: 100.0, err: 0.0, lgint: -3.0, dr: 3, elo: 0.0, gup: 3, tag: 1, qnfmt: 202,
            upper: vec![1, 0], lower: vec![0, 0],
        };
        let p = Profile::Gaussian { fwhm: 0.2 };
        let s = from_catalog(&[line], 99.0, 101.0, 0.001, p, true);
        let area: f64 = s.intensity.iter().sum::<f64>() * 0.001;
        assert!((area - 1e-3).abs() < 1e-8, "{area}");
    }
}
