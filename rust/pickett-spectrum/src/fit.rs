//! Fitting one line shape (plus a local baseline) to a piece of spectrum
//! by Levenberg–Marquardt least squares, for accurate line centres with
//! standard errors.

use crate::linalg::{inverse, solve};
use crate::peaks::Peak;
use crate::profile::Profile;
use crate::Spectrum;

/// Which line shape to fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Gaussian,
    Lorentzian,
    /// Pseudo-Voigt with separate Gaussian and Lorentzian widths.
    Voigt,
}

/// Local baseline under the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Baseline {
    None,
    Constant,
    Linear,
}

/// Settings for [`fit_peak`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitOptions {
    pub shape: Shape,
    pub baseline: Baseline,
    /// Half-width of the fitted window, in units of the starting FWHM.
    pub window: f64,
    pub max_iterations: usize,
}

impl Default for FitOptions {
    fn default() -> Self {
        FitOptions { shape: Shape::Gaussian, baseline: Baseline::Linear, window: 3.0, max_iterations: 100 }
    }
}

/// The result of [`fit_peak`]. Errors are one standard deviation, scaled
/// by the reduced χ² of the fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeakFit {
    pub center: f64,
    pub center_err: f64,
    /// Peak height above the local baseline.
    pub amplitude: f64,
    pub amplitude_err: f64,
    /// The fitted profile (its widths).
    pub profile: Profile,
    /// Baseline `b0 + b1·(f − center_start)` under the line.
    pub baseline: (f64, f64),
    /// RMS of the residuals in the window.
    pub rms: f64,
    pub points: usize,
    pub iterations: usize,
    pub converged: bool,
}

/// Starting values for [`fit_peak`]: centre, height and FWHM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guess {
    pub center: f64,
    pub height: f64,
    pub fwhm: f64,
}

impl From<&Peak> for Guess {
    fn from(p: &Peak) -> Guess {
        Guess { center: p.freq, height: p.height, fwhm: p.fwhm }
    }
}

/// Fits one line near `guess` within `opts.window` FWHM on either side.
/// Returns `None` when the window has too few points, the starting width is
/// not usable, or the fit is singular.
///
/// Parameters: centre offset, height, width(s) and the baseline terms. The
/// centre is fitted relative to the starting centre, which keeps the problem
/// well conditioned at GHz frequencies.
pub fn fit_peak(spec: &Spectrum, guess: impl Into<Guess>, opts: &FitOptions) -> Option<PeakFit> {
    let g = guess.into();
    if !(g.fwhm.is_finite() && g.fwhm > 0.0) {
        return None;
    }
    let half = opts.window * g.fwhm;
    let range = spec.index_range(g.center - half, g.center + half);
    let x: Vec<f64> = spec.freq[range.clone()].iter().map(|f| f - g.center).collect();
    let y = &spec.intensity[range];

    // parameter vector: [offset, height, width(s)..., baseline...]
    let mut p = vec![0.0, g.height];
    match opts.shape {
        Shape::Gaussian | Shape::Lorentzian => p.push(g.fwhm),
        Shape::Voigt => {
            p.push(g.fwhm * 0.7);
            p.push(g.fwhm * 0.5);
        }
    }
    let nb = match opts.baseline {
        Baseline::None => 0,
        Baseline::Constant => 1,
        Baseline::Linear => 2,
    };
    let edge = y.first().zip(y.last()).map_or(0.0, |(a, b)| 0.5 * (a + b));
    if nb >= 1 {
        p.push(edge);
        p[1] -= edge;
    }
    if nb == 2 {
        p.push(0.0);
    }
    let np = p.len();
    if x.len() < np + 2 {
        return None;
    }

    let shape = opts.shape;
    let profile_of = |p: &[f64]| -> Profile {
        match shape {
            Shape::Gaussian => Profile::Gaussian { fwhm: p[2].abs() },
            Shape::Lorentzian => Profile::Lorentzian { fwhm: p[2].abs() },
            Shape::Voigt => Profile::Voigt { fwhm_gauss: p[2].abs(), fwhm_lorentz: p[3].abs() },
        }
    };
    let wcount = if shape == Shape::Voigt { 2 } else { 1 };
    let model = |p: &[f64], xi: f64| -> f64 {
        let mut v = p[1] * profile_of(p).value(xi - p[0]);
        if nb >= 1 {
            v += p[2 + wcount];
        }
        if nb == 2 {
            v += p[3 + wcount] * xi;
        }
        v
    };
    let chi2 = |p: &[f64]| -> f64 { x.iter().zip(y).map(|(&xi, &yi)| (yi - model(p, xi)).powi(2)).sum() };
    // finite-difference steps scaled to each parameter's natural size
    let steps = |p: &[f64]| -> Vec<f64> {
        let w = profile_of(p).fwhm().max(1e-300);
        let ysc = p[1].abs().max(1e-300);
        (0..np)
            .map(|j| match j {
                0 => 1e-6 * w,
                1 => 1e-6 * ysc,
                j if j < 2 + wcount => 1e-6 * p[j].abs().max(1e-3 * w),
                j if j == 2 + wcount => 1e-6 * ysc,
                _ => 1e-6 * ysc / w,
            })
            .collect()
    };
    let jacobian = |p: &[f64]| -> Vec<f64> {
        let h = steps(p);
        let mut jac = vec![0.0; x.len() * np];
        let mut q = p.to_vec();
        for j in 0..np {
            q[j] = p[j] + h[j];
            let up: Vec<f64> = x.iter().map(|&xi| model(&q, xi)).collect();
            q[j] = p[j] - h[j];
            for (i, &xi) in x.iter().enumerate() {
                jac[i * np + j] = (up[i] - model(&q, xi)) / (2.0 * h[j]);
            }
            q[j] = p[j];
        }
        jac
    };
    let normal = |jac: &[f64], p: &[f64]| -> (Vec<f64>, Vec<f64>) {
        let mut jtj = vec![0.0; np * np];
        let mut jtr = vec![0.0; np];
        for (i, (&xi, &yi)) in x.iter().zip(y).enumerate() {
            let r = yi - model(p, xi);
            let row = &jac[i * np..(i + 1) * np];
            for a in 0..np {
                jtr[a] += row[a] * r;
                for b in 0..np {
                    jtj[a * np + b] += row[a] * row[b];
                }
            }
        }
        (jtj, jtr)
    };

    let mut lambda = 1e-3;
    let mut cur = chi2(&p);
    let mut converged = false;
    let mut iterations = 0;
    for it in 1..=opts.max_iterations {
        iterations = it;
        let jac = jacobian(&p);
        let (jtj, jtr) = normal(&jac, &p);
        let mut improved = false;
        for _ in 0..30 {
            let mut a = jtj.clone();
            for k in 0..np {
                a[k * np + k] += lambda * jtj[k * np + k].max(1e-300);
            }
            let mut delta = jtr.clone();
            if solve(&mut a, &mut delta, np).is_none() {
                lambda *= 10.0;
                continue;
            }
            let trial: Vec<f64> = p.iter().zip(&delta).map(|(a, d)| a + d).collect();
            let c = chi2(&trial);
            if c.is_finite() && c <= cur {
                let rel = (cur - c) / cur.max(1e-300);
                p = trial;
                cur = c;
                lambda = (lambda / 10.0).max(1e-12);
                improved = true;
                if rel < 1e-12 {
                    converged = true;
                }
                break;
            }
            lambda *= 10.0;
        }
        if !improved || converged {
            converged = true;
            break;
        }
    }

    let jac = jacobian(&p);
    let (jtj, _) = normal(&jac, &p);
    let cov = inverse(&jtj, np)?;
    let dof = (x.len() - np) as f64;
    let s2 = cur / dof;
    let err = |k: usize| (cov[k * np + k] * s2).max(0.0).sqrt();
    let (b0, b1) = match nb {
        0 => (0.0, 0.0),
        1 => (p[2 + wcount], 0.0),
        _ => (p[2 + wcount], p[3 + wcount]),
    };
    Some(PeakFit {
        center: g.center + p[0],
        center_err: err(0),
        amplitude: p[1],
        amplitude_err: err(1),
        profile: profile_of(&p),
        baseline: (b0, b1),
        rms: (cur / x.len() as f64).sqrt(),
        points: x.len(),
        iterations,
        converged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth;

    fn noisy(profile: Profile, centre: f64, amp: f64, sd: f64, slope: f64) -> Spectrum {
        let g = synth::grid(99.0, 101.0, 0.004);
        let mut y = synth::synthesize([(centre, amp)], &g, profile, 50.0);
        let mut s: u64 = 99;
        let mut u = || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 11) as f64 / (1u64 << 53) as f64 };
        for (i, v) in y.iter_mut().enumerate() {
            let n = (-2.0 * u().max(1e-300).ln()).sqrt() * (std::f64::consts::TAU * u()).cos();
            *v += 0.2 + slope * (g[i] - 100.0) + sd * n;
        }
        Spectrum::new(g, y).unwrap()
    }

    #[test]
    fn recovers_each_shape_within_errors() {
        for (shape, profile) in [
            (Shape::Gaussian, Profile::Gaussian { fwhm: 0.12 }),
            (Shape::Lorentzian, Profile::Lorentzian { fwhm: 0.12 }),
            (Shape::Voigt, Profile::Voigt { fwhm_gauss: 0.1, fwhm_lorentz: 0.05 }),
        ] {
            let s = noisy(profile, 100.0123, 1.0, 0.01, 0.05);
            let guess = Guess { center: 100.0, height: 1.0, fwhm: 0.15 };
            let fit = fit_peak(&s, guess, &FitOptions { shape, window: 5.0, ..Default::default() }).unwrap();
            assert!(fit.converged, "{shape:?}");
            let dc = (fit.center - 100.0123).abs();
            assert!(dc < 4.0 * fit.center_err + 1e-6, "{shape:?}: centre off by {dc}, err {}", fit.center_err);
            assert!(fit.center_err < 1e-3, "{shape:?}: err {}", fit.center_err);
            assert!((fit.amplitude - 1.0).abs() < 0.03, "{shape:?}: amp {}", fit.amplitude);
            assert!((fit.profile.fwhm() - profile.fwhm()).abs() < 0.01, "{shape:?}: fwhm {}", fit.profile.fwhm());
            assert!((fit.rms - 0.01).abs() < 0.002, "{shape:?}: rms {}", fit.rms);
            assert!((fit.baseline.0 - 0.2).abs() < 0.01 && (fit.baseline.1 - 0.05).abs() < 0.05, "{shape:?}: {:?}", fit.baseline);
        }
    }

    #[test]
    fn unusable_inputs() {
        let s = noisy(Profile::Gaussian { fwhm: 0.1 }, 100.0, 1.0, 0.01, 0.0);
        assert!(fit_peak(&s, Guess { center: 100.0, height: 1.0, fwhm: f64::NAN }, &FitOptions::default()).is_none());
        assert!(fit_peak(&s, Guess { center: 100.0, height: 1.0, fwhm: 0.001 }, &FitOptions::default()).is_none());
    }
}
