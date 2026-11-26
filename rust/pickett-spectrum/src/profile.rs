//! Line shapes, normalised to 1 at the line centre.

use std::f64::consts::LN_2;

/// A line shape with its full width at half maximum (FWHM).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Profile {
    /// Doppler-limited lines.
    Gaussian { fwhm: f64 },
    /// Pressure-broadened lines.
    Lorentzian { fwhm: f64 },
    /// Both together: the pseudo-Voigt approximation of Thompson, Cox and
    /// Hastings (J. Appl. Cryst. 20, 79 (1987)), a mix of a Gaussian and a
    /// Lorentzian of the same total FWHM. It is within about 1% of the true
    /// Voigt profile.
    Voigt { fwhm_gauss: f64, fwhm_lorentz: f64 },
}

impl Profile {
    /// The value at distance `dx` from the centre (1 at `dx = 0`).
    pub fn value(&self, dx: f64) -> f64 {
        match *self {
            Profile::Gaussian { fwhm } => gauss(dx, fwhm),
            Profile::Lorentzian { fwhm } => lorentz(dx, fwhm),
            Profile::Voigt { fwhm_gauss, fwhm_lorentz } => {
                let (f, eta) = tch(fwhm_gauss.abs(), fwhm_lorentz.abs());
                eta * lorentz(dx, f) + (1.0 - eta) * gauss(dx, f)
            }
        }
    }

    /// Total full width at half maximum.
    pub fn fwhm(&self) -> f64 {
        match *self {
            Profile::Gaussian { fwhm } | Profile::Lorentzian { fwhm } => fwhm.abs(),
            Profile::Voigt { fwhm_gauss, fwhm_lorentz } => tch(fwhm_gauss.abs(), fwhm_lorentz.abs()).0,
        }
    }

    /// Integral over all frequencies (for a peak height of 1).
    pub fn area(&self) -> f64 {
        let g = |f: f64| f * (std::f64::consts::PI / (4.0 * LN_2)).sqrt();
        let l = |f: f64| f * std::f64::consts::FRAC_PI_2;
        match *self {
            Profile::Gaussian { fwhm } => g(fwhm.abs()),
            Profile::Lorentzian { fwhm } => l(fwhm.abs()),
            Profile::Voigt { fwhm_gauss, fwhm_lorentz } => {
                let (f, eta) = tch(fwhm_gauss.abs(), fwhm_lorentz.abs());
                eta * l(f) + (1.0 - eta) * g(f)
            }
        }
    }
}

fn gauss(dx: f64, fwhm: f64) -> f64 {
    let f = fwhm.abs().max(f64::MIN_POSITIVE);
    (-4.0 * LN_2 * dx * dx / (f * f)).exp()
}

fn lorentz(dx: f64, fwhm: f64) -> f64 {
    let f = fwhm.abs().max(f64::MIN_POSITIVE);
    1.0 / (1.0 + 4.0 * dx * dx / (f * f))
}

/// Total FWHM and Lorentzian fraction of the Thompson–Cox–Hastings pseudo-Voigt.
fn tch(fg: f64, fl: f64) -> (f64, f64) {
    let f = (fg.powi(5)
        + 2.69269 * fg.powi(4) * fl
        + 2.42843 * fg.powi(3) * fl.powi(2)
        + 4.47163 * fg.powi(2) * fl.powi(3)
        + 0.07842 * fg * fl.powi(4)
        + fl.powi(5))
    .powf(0.2);
    if f <= 0.0 {
        return (0.0, 0.0);
    }
    let r = fl / f;
    (f, (1.36603 * r - 0.47719 * r * r + 0.11116 * r * r * r).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_maximum_at_half_width() {
        for p in [
            Profile::Gaussian { fwhm: 2.0 },
            Profile::Lorentzian { fwhm: 2.0 },
            Profile::Voigt { fwhm_gauss: 1.2, fwhm_lorentz: 1.0 },
        ] {
            assert!((p.value(0.0) - 1.0).abs() < 1e-12);
            let h = p.fwhm() / 2.0;
            assert!((p.value(h) - 0.5).abs() < 1e-9, "{p:?}: {}", p.value(h));
            assert!((p.value(-h) - 0.5).abs() < 1e-9);
        }
    }

    #[test]
    fn voigt_limits_and_area() {
        let g = Profile::Voigt { fwhm_gauss: 1.0, fwhm_lorentz: 0.0 };
        let l = Profile::Voigt { fwhm_gauss: 0.0, fwhm_lorentz: 1.0 };
        for x in [0.1, 0.5, 1.3] {
            assert!((g.value(x) - Profile::Gaussian { fwhm: 1.0 }.value(x)).abs() < 1e-12);
            assert!((l.value(x) - Profile::Lorentzian { fwhm: 1.0 }.value(x)).abs() < 1e-4);
        }
        // numerical area of the Gaussian
        let p = Profile::Gaussian { fwhm: 0.7 };
        let num: f64 = (-20000..=20000).map(|i| p.value(i as f64 * 1e-4) * 1e-4).sum();
        assert!((num - p.area()).abs() < 1e-9);
    }
}
