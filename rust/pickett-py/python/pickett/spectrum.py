"""Analysis of measured spectra: baseline, smoothing, peaks, line fits,
and assignment to an SPCAT prediction.

    from pickett import spectrum as sp
    f, y = sp.read("scan.txt")
    flat = y - sp.asls(y, lam=1e8, p=0.002)
    peaks = sp.find_peaks(f, flat, min_snr=6, min_distance=3)
    fits = sp.fit_peaks(f, flat, peaks)
    m = sp.match_lines(fits["center"], prediction, tolerance=1.0)
    lin = sp.to_lin(prediction, m, err=fits["center_err"][m["observed"]])

Frequencies can be in any unit (MHz for SPFIT/SPCAT); intensities are linear.
"""

from __future__ import annotations

from typing import Optional, Tuple, Union

import numpy as np

from . import Prediction, _native, read_text

__all__ = [
    "read", "asls", "savgol", "noise_level", "find_peaks", "fit_peaks", "match_lines",
    "to_lin", "synthesize", "catalog_spectrum", "profile_area",
]


def _f64(a) -> np.ndarray:
    return np.ascontiguousarray(a, dtype=np.float64)


def read(src, fcol: int = 0, icol: int = 1) -> Tuple[np.ndarray, np.ndarray]:
    """Frequency and intensity from a text file (path or text): columns
    `fcol` and `icol`, separated by spaces, tabs, commas or semicolons;
    comment and header lines are skipped; sorted by frequency."""
    return _native.read_spectrum(read_text(src), fcol, icol)


def asls(y, lam: float = 1e6, p: float = 0.01, iterations: int = 10) -> np.ndarray:
    """Asymmetric least-squares baseline (Eilers & Boelens). Larger `lam`
    gives a stiffer baseline; `p` is the weight of points above it."""
    return _native.asls(_f64(y), lam, p, iterations)


def savgol(y, half_window: int, order: int) -> np.ndarray:
    """Savitzky–Golay smoothing over ``2*half_window + 1`` points."""
    return _native.savgol(_f64(y), half_window, order)


def noise_level(y) -> float:
    """Robust noise standard deviation (MAD of point differences)."""
    return _native.noise_level(_f64(y))


def find_peaks(freq, y, min_snr: float = 5.0, min_height: Optional[float] = None,
               min_prominence: Optional[float] = None, min_distance: int = 1,
               noise: Optional[float] = None) -> dict:
    """Peaks of a baseline-free spectrum: dict of arrays ``index``, ``freq``
    (sub-point), ``height``, ``prominence``, ``snr``, ``fwhm``."""
    return _native.find_peaks(_f64(freq), _f64(y), min_snr, min_height, min_prominence, min_distance, noise)


def fit_peaks(freq, y, peaks: Union[dict, None] = None, center=None, height=None, fwhm=None,
              shape: str = "gaussian", baseline: str = "linear", window: float = 3.0) -> dict:
    """Fits each line (``shape`` gaussian/lorentzian/voigt, local baseline
    none/constant/linear) within `window` FWHM. Starting values come from
    `peaks` (a :func:`find_peaks` result) or `center`/`height`/`fwhm`.
    Returns dict of arrays ``center``, ``center_err``, ``amplitude``,
    ``amplitude_err``, ``fwhm``, ``rms`` and list ``converged``; failed fits are NaN."""
    if peaks is not None:
        center, height, fwhm = peaks["freq"], peaks["height"], peaks["fwhm"]
    if center is None or height is None or fwhm is None:
        raise ValueError("give peaks= or center=, height= and fwhm=")
    out = _native.fit_peaks(_f64(freq), _f64(y), _f64(center), _f64(height), _f64(fwhm), shape, baseline, window)
    out["converged"] = np.asarray(out["converged"], dtype=bool)
    return out


def _catalog(pred) -> str:
    return pred.catalog if isinstance(pred, Prediction) else read_text(pred)


def match_lines(observed, prediction, tolerance: float = 1.0, min_lgint: float = -np.inf,
                drop_ambiguous: bool = True) -> dict:
    """Matches measured frequencies to predicted lines (closest first, each
    used once). `prediction` is a :class:`pickett.Prediction` or a ``.cat``
    text/path. Returns dict of arrays ``observed`` (index into `observed`),
    ``predicted`` (index into the catalog), ``obs``, ``calc``, ``residual``,
    ``ambiguous``, ``upper``, ``lower``. Ambiguous matches (another predicted
    line within the tolerance) are dropped unless `drop_ambiguous` is False."""
    m = _native.match_lines(_f64(observed), _catalog(prediction), tolerance, min_lgint)
    m["ambiguous"] = np.asarray(m["ambiguous"], dtype=bool)
    if drop_ambiguous:
        keep = ~m["ambiguous"]
        m = {k: v[keep] for k, v in m.items()}
    return m


def to_lin(prediction, matches: dict, err) -> str:
    """A ``.lin`` text for SPFIT from :func:`match_lines` results; `err` is
    one value or one per match (e.g. fitted centre errors)."""
    n = len(matches["obs"])
    err = np.broadcast_to(_f64(err), (n,)).copy()
    return _native.to_lin(_catalog(prediction), [int(i) for i in matches["predicted"]], _f64(matches["obs"]), err)


def synthesize(freq, amplitude, grid, shape: str = "gaussian", fwhm: float = 1.0,
               fwhm_lorentz: float = 0.0, cutoff: float = 20.0) -> np.ndarray:
    """Lines of peak height `amplitude` at `freq`, drawn on the ascending
    `grid` out to `cutoff` FWHM (for voigt, `fwhm` is the Gaussian width)."""
    return _native.synthesize(_f64(freq), _f64(amplitude), _f64(grid), shape, fwhm, fwhm_lorentz, cutoff)


def profile_area(shape: str = "gaussian", fwhm: float = 1.0, fwhm_lorentz: float = 0.0) -> float:
    """Integral of a line of height 1."""
    return _native.profile_area(shape, fwhm, fwhm_lorentz)


def catalog_spectrum(prediction: Prediction, fmin: float, fmax: float, step: float,
                     shape: str = "gaussian", fwhm: float = 1.0, fwhm_lorentz: float = 0.0
                     ) -> Tuple[np.ndarray, np.ndarray]:
    """A simulated spectrum of the predicted lines: heights
    ``10**lgint / profile_area`` so the integrated intensities match."""
    grid = np.arange(fmin, fmax + 0.5 * step, step)
    f, lg = prediction.lines["freq"], prediction.lines["lgint"]
    reach = 30 * fwhm
    sel = (f >= fmin - reach) & (f <= fmax + reach)
    amp = 10.0 ** lg[sel] / profile_area(shape, fwhm, fwhm_lorentz)
    return grid, synthesize(f[sel], amp, grid, shape, fwhm, fwhm_lorentz, 30.0)
