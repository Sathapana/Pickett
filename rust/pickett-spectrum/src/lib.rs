//! Analysis of experimental rotational spectra, for use with SPFIT/SPCAT.
//!
//! The usual path from a measured spectrum to a fit:
//!
//! 1. read the spectrum ([`Spectrum::parse`]),
//! 2. optionally smooth it ([`smooth::savitzky_golay`]) and remove the
//!    baseline ([`baseline::asls`]),
//! 3. find the peaks ([`peaks::find_peaks`]) and fit their line shapes
//!    for accurate centres and errors ([`fit::fit_peak`]),
//! 4. match the measured centres to the lines SPCAT predicts
//!    ([`assign::match_lines`]) and write the assignments as a `.lin` file
//!    ([`assign::to_lin`]) for SPFIT.
//!
//! [`synth`] goes the other way and draws a spectrum from a catalog, for
//! comparison with a measurement or to test the steps above.
//!
//! Frequencies can be in any unit (MHz in SPFIT/SPCAT files); intensities
//! are linear.
//!
//! ```
//! use pickett_spectrum::{peaks, profile::Profile, synth, Spectrum};
//!
//! // two Gaussian lines on a grid, then find them again
//! let grid = synth::grid(1000.0, 1010.0, 0.01);
//! let y = synth::synthesize([(1003.0, 1.0), (1007.5, 0.5)], &grid, Profile::Gaussian { fwhm: 0.1 }, 20.0);
//! let spec = Spectrum::new(grid, y).unwrap();
//! let found = peaks::find_peaks(&spec, &peaks::PeakOptions { noise: Some(1e-3), ..Default::default() });
//! assert_eq!(found.len(), 2);
//! assert!((found[0].freq - 1003.0).abs() < 1e-3);
//! ```

pub mod assign;
pub mod baseline;
pub mod fit;
pub mod peaks;
pub mod profile;
pub mod smooth;
pub mod synth;

mod linalg;
mod spectrum;

pub use spectrum::{Spectrum, SpectrumError};
