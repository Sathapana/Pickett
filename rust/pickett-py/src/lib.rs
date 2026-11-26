//! Python bindings (module `pickett._native`). The public Python API is in
//! `python/pickett/`; this module returns plain dicts of numpy arrays and
//! strings, which the Python layer wraps in classes.
//!
//! Every SPCAT/SPFIT run releases the GIL, so Python threads run them in
//! parallel (the C library keeps its state per thread).

use numpy::{IntoPyArray, PyArray2, PyReadonlyArray1};
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

use pickett::{Error, FitResult, Flavor, Prediction};
use pickett_io::CatFile;
use pickett_spectrum::{assign, baseline, fit, peaks, profile::Profile, smooth, synth, Spectrum};

create_exception!(_native, PickettError, PyException, "SPFIT/SPCAT failed; `log` holds the program's messages.");

fn flavor(name: &str) -> PyResult<Flavor> {
    match name.to_ascii_lowercase().as_str() {
        "spinv" | "sp" | "spcat" | "spfit" => Ok(Flavor::Spinv),
        "dpi" | "dp" | "dpcat" | "dpfit" => Ok(Flavor::Dpi),
        other => Err(PyValueError::new_err(format!("flavor must be 'spinv' or 'dpi', not {other:?}"))),
    }
}

/// Turns a library error into `PickettError(message)` with `.log`.
fn py_err(py: Python<'_>, e: Error) -> PyErr {
    let log = match &e {
        Error::Run { log, .. } | Error::NoOutput { log, .. } => log.clone(),
        _ => String::new(),
    };
    let err = PickettError::new_err(e.to_string());
    // setting an attribute on a fresh exception object cannot fail in practice
    let _ = err.value(py).setattr("log", log);
    err
}

fn i32_matrix<'py>(py: Python<'py>, rows: Vec<Vec<i32>>) -> PyResult<Bound<'py, PyArray2<i32>>> {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let padded: Vec<Vec<i32>> = rows.into_iter().map(|mut r| { r.resize(width, 0); r }).collect();
    if padded.is_empty() {
        return Ok(PyArray2::zeros(py, [0, 0], false));
    }
    PyArray2::from_vec2(py, &padded).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn prediction_dict<'py>(py: Python<'py>, p: Prediction) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    let l = &p.lines;
    let lines = PyDict::new(py);
    lines.set_item("freq", l.iter().map(|x| x.freq).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("err", l.iter().map(|x| x.err).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("lgint", l.iter().map(|x| x.lgint).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("strength", l.iter().map(|x| x.strength).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("dr", l.iter().map(|x| x.dr).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("elow", l.iter().map(|x| x.elow).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("gup", l.iter().map(|x| x.gup).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("tag", l.iter().map(|x| x.tag).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("qnfmt", l.iter().map(|x| x.qnfmt).collect::<Vec<_>>().into_pyarray(py))?;
    lines.set_item("upper", i32_matrix(py, l.iter().map(|x| x.upper.clone()).collect())?)?;
    lines.set_item("lower", i32_matrix(py, l.iter().map(|x| x.lower.clone()).collect())?)?;
    d.set_item("lines", lines)?;

    let v = &p.levels;
    let levels = PyDict::new(py);
    levels.set_item("block", v.iter().map(|x| x.block).collect::<Vec<_>>().into_pyarray(py))?;
    levels.set_item("index", v.iter().map(|x| x.index).collect::<Vec<_>>().into_pyarray(py))?;
    levels.set_item("energy", v.iter().map(|x| x.energy).collect::<Vec<_>>().into_pyarray(py))?;
    levels.set_item("err", v.iter().map(|x| x.err).collect::<Vec<_>>().into_pyarray(py))?;
    levels.set_item("mixing", v.iter().map(|x| x.mixing).collect::<Vec<_>>().into_pyarray(py))?;
    levels.set_item("degeneracy", v.iter().map(|x| x.degeneracy).collect::<Vec<_>>().into_pyarray(py))?;
    levels.set_item("qn", i32_matrix(py, v.iter().map(|x| x.qn.clone()).collect())?)?;
    d.set_item("levels", levels)?;

    let t: Vec<f64> = p.partition.iter().map(|x| x.0).collect();
    let q: Vec<f64> = p.partition.iter().map(|x| x.1).collect();
    d.set_item("partition_t", t.into_pyarray(py))?;
    d.set_item("partition_q", q.into_pyarray(py))?;
    d.set_item("catalog", p.catalog.to_text())?;
    d.set_item("report", p.report)?;
    d.set_item("log", p.log)?;
    let files = PyDict::new(py);
    for (name, bytes) in p.files {
        files.set_item(name, PyBytes::new(py, &bytes))?;
    }
    d.set_item("files", files)?;
    Ok(d)
}

fn fit_dict<'py>(py: Python<'py>, r: FitResult) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    let params = PyDict::new(py);
    let p = &r.params;
    params.set_item("id", p.iter().map(|x| x.id.clone()).collect::<Vec<_>>())?;
    params.set_item("label", p.iter().map(|x| x.label.clone()).collect::<Vec<_>>())?;
    params.set_item("value", p.iter().map(|x| x.value).collect::<Vec<_>>().into_pyarray(py))?;
    params.set_item("err_fit", p.iter().map(|x| x.err_fit).collect::<Vec<_>>().into_pyarray(py))?;
    params.set_item("err_par", p.iter().map(|x| x.err_par).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("params", params)?;
    let it = &r.iterations;
    let iters = PyDict::new(py);
    iters.set_item("iteration", it.iter().map(|x| x.iteration).collect::<Vec<_>>().into_pyarray(py))?;
    iters.set_item("rms_error_old", it.iter().map(|x| x.rms_error_old).collect::<Vec<_>>().into_pyarray(py))?;
    iters.set_item("rms_error_new", it.iter().map(|x| x.rms_error_new).collect::<Vec<_>>().into_pyarray(py))?;
    iters.set_item("rms_mw", it.iter().map(|x| x.rms_mw).collect::<Vec<_>>().into_pyarray(py))?;
    iters.set_item("rms_ir", it.iter().map(|x| x.rms_ir).collect::<Vec<_>>().into_pyarray(py))?;
    iters.set_item("lines_used", it.iter().map(|x| x.lines_used).collect::<Vec<_>>().into_pyarray(py))?;
    iters.set_item("lines_rejected", it.iter().map(|x| x.lines_rejected).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("iterations", iters)?;
    d.set_item("par", r.par.to_text())?;
    d.set_item("var", r.var.to_text())?;
    d.set_item("report", r.report)?;
    d.set_item("log", r.log)?;
    Ok(d)
}

/// predict(var, int, flavor="spinv", threads=None) -> dict
///
/// SPCAT (or DPCAT) on the texts of a .var and an .int file.
#[pyfunction]
#[pyo3(signature = (var, int, flavor = "spinv", threads = None))]
fn predict<'py>(py: Python<'py>, var: String, int: String, flavor: &str, threads: Option<usize>) -> PyResult<Bound<'py, PyDict>> {
    let fl = self::flavor(flavor)?;
    let result = py.detach(move || {
        if let Some(n) = threads {
            pickett_sys::set_threads(n);
        }
        pickett::predict_text(fl, &var, &int)
    });
    prediction_dict(py, result.map_err(|e| py_err(py, e))?)
}

/// predict_many(jobs, flavor="spinv", threads=None) -> list[dict]
///
/// Many predictions on all cores; jobs is a list of (var, int) texts.
#[pyfunction]
#[pyo3(signature = (jobs, flavor = "spinv", threads = None))]
fn predict_many<'py>(py: Python<'py>, jobs: Vec<(String, String)>, flavor: &str, threads: Option<usize>) -> PyResult<Bound<'py, PyList>> {
    let fl = self::flavor(flavor)?;
    let results = py.detach(move || {
        let parsed: Result<Vec<_>, String> = jobs
            .iter()
            .enumerate()
            .map(|(i, (v, t))| {
                let v = pickett::ParFile::parse(v, fl).map_err(|e| format!("job {i}: .var: {e}"))?;
                let t = pickett::IntFile::parse(t).map_err(|e| format!("job {i}: .int: {e}"))?;
                Ok((v, t))
            })
            .collect();
        parsed.map(|p| pickett::predict_parallel(fl, &p, threads))
    });
    let results = results.map_err(PickettError::new_err)?;
    let out = PyList::empty(py);
    for (i, r) in results.into_iter().enumerate() {
        match r {
            Ok(p) => out.append(prediction_dict(py, p)?)?,
            Err(e) => {
                let err = py_err(py, e);
                return Err(PickettError::new_err(format!("job {i}: {}", err.value(py))));
            }
        }
    }
    Ok(out)
}

/// fit(par, lin, flavor="spinv") -> dict
///
/// SPFIT (or DPFIT) on the texts of a .par and a .lin file.
#[pyfunction]
#[pyo3(signature = (par, lin, flavor = "spinv"))]
fn fit_lines<'py>(py: Python<'py>, par: String, lin: String, flavor: &str) -> PyResult<Bound<'py, PyDict>> {
    let fl = self::flavor(flavor)?;
    let result = py.detach(move || pickett::fit_text(fl, &par, &lin));
    fit_dict(py, result.map_err(|e| py_err(py, e))?)
}

/// thread_safe() -> bool: whether runs on different threads go in parallel.
#[pyfunction]
fn thread_safe() -> bool {
    pickett_sys::thread_safe()
}

/// set_threads(n): threads one run on the calling thread may use (0 = all cores).
#[pyfunction]
fn set_threads(n: usize) {
    pickett_sys::set_threads(n)
}

// ------------------------------------------------------------ spectra

fn slice<'a>(a: &'a PyReadonlyArray1<'_, f64>, name: &str) -> PyResult<&'a [f64]> {
    a.as_slice().map_err(|_| PyValueError::new_err(format!("{name} must be a contiguous 1-D float64 array")))
}

/// read_spectrum(text, fcol=0, icol=1) -> (freq, intensity)
#[pyfunction]
#[pyo3(signature = (text, fcol = 0, icol = 1))]
fn read_spectrum<'py>(py: Python<'py>, text: &str, fcol: usize, icol: usize) -> PyResult<(Bound<'py, numpy::PyArray1<f64>>, Bound<'py, numpy::PyArray1<f64>>)> {
    let s = Spectrum::parse_columns(text, fcol, icol).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok((s.freq.into_pyarray(py), s.intensity.into_pyarray(py)))
}

/// asls(y, lam=1e6, p=0.01, iterations=10) -> baseline
#[pyfunction]
#[pyo3(signature = (y, lam = 1e6, p = 0.01, iterations = 10))]
fn asls<'py>(py: Python<'py>, y: PyReadonlyArray1<'py, f64>, lam: f64, p: f64, iterations: usize) -> PyResult<Bound<'py, numpy::PyArray1<f64>>> {
    let y = slice(&y, "y")?.to_vec();
    let z = py.detach(move || baseline::asls(&y, &baseline::AslsOptions { lambda: lam, p, iterations }));
    Ok(z.into_pyarray(py))
}

/// savgol(y, half_window, order) -> smoothed
#[pyfunction]
fn savgol<'py>(py: Python<'py>, y: PyReadonlyArray1<'py, f64>, half_window: usize, order: usize) -> PyResult<Bound<'py, numpy::PyArray1<f64>>> {
    if order >= 2 * half_window + 1 {
        return Err(PyValueError::new_err("order must be smaller than the window (2*half_window + 1)"));
    }
    Ok(smooth::savitzky_golay(slice(&y, "y")?, half_window, order).into_pyarray(py))
}

/// noise_level(y) -> float
#[pyfunction]
fn noise_level(y: PyReadonlyArray1<'_, f64>) -> PyResult<f64> {
    Ok(peaks::noise_level(slice(&y, "y")?))
}

fn spectrum(freq: &PyReadonlyArray1<'_, f64>, y: &PyReadonlyArray1<'_, f64>) -> PyResult<Spectrum> {
    Spectrum::new(slice(freq, "freq")?.to_vec(), slice(y, "intensity")?.to_vec()).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// find_peaks(freq, y, min_snr=5, min_height=None, min_prominence=None, min_distance=1, noise=None) -> dict
#[pyfunction]
#[pyo3(signature = (freq, y, min_snr = 5.0, min_height = None, min_prominence = None, min_distance = 1, noise = None))]
#[allow(clippy::too_many_arguments)]
fn find_peaks<'py>(
    py: Python<'py>, freq: PyReadonlyArray1<'py, f64>, y: PyReadonlyArray1<'py, f64>, min_snr: f64,
    min_height: Option<f64>, min_prominence: Option<f64>, min_distance: usize, noise: Option<f64>,
) -> PyResult<Bound<'py, PyDict>> {
    let s = spectrum(&freq, &y)?;
    let opts = peaks::PeakOptions { min_snr, min_height, min_prominence, min_distance, noise };
    let found = py.detach(move || peaks::find_peaks(&s, &opts));
    let d = PyDict::new(py);
    d.set_item("index", found.iter().map(|p| p.index).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("freq", found.iter().map(|p| p.freq).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("height", found.iter().map(|p| p.height).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("prominence", found.iter().map(|p| p.prominence).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("snr", found.iter().map(|p| p.snr).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("fwhm", found.iter().map(|p| p.fwhm).collect::<Vec<_>>().into_pyarray(py))?;
    Ok(d)
}

fn shape(name: &str) -> PyResult<fit::Shape> {
    match name.to_ascii_lowercase().as_str() {
        "gaussian" | "gauss" => Ok(fit::Shape::Gaussian),
        "lorentzian" | "lorentz" => Ok(fit::Shape::Lorentzian),
        "voigt" => Ok(fit::Shape::Voigt),
        o => Err(PyValueError::new_err(format!("shape must be 'gaussian', 'lorentzian' or 'voigt', not {o:?}"))),
    }
}

/// fit_peaks(freq, y, center, height, fwhm, shape="gaussian", baseline="linear", window=3.0) -> dict
///
/// Fits each line independently; failed fits give NaN and converged=False.
#[pyfunction]
#[pyo3(signature = (freq, y, center, height, fwhm, shape = "gaussian", baseline = "linear", window = 3.0))]
#[allow(clippy::too_many_arguments)]
fn fit_peaks<'py>(
    py: Python<'py>, freq: PyReadonlyArray1<'py, f64>, y: PyReadonlyArray1<'py, f64>,
    center: PyReadonlyArray1<'py, f64>, height: PyReadonlyArray1<'py, f64>, fwhm: PyReadonlyArray1<'py, f64>,
    shape: &str, baseline: &str, window: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let s = spectrum(&freq, &y)?;
    let (c, h, w) = (slice(&center, "center")?.to_vec(), slice(&height, "height")?.to_vec(), slice(&fwhm, "fwhm")?.to_vec());
    if c.len() != h.len() || c.len() != w.len() {
        return Err(PyValueError::new_err("center, height and fwhm must have the same length"));
    }
    let b = match baseline.to_ascii_lowercase().as_str() {
        "none" => fit::Baseline::None,
        "constant" => fit::Baseline::Constant,
        "linear" => fit::Baseline::Linear,
        o => return Err(PyValueError::new_err(format!("baseline must be 'none', 'constant' or 'linear', not {o:?}"))),
    };
    let opts = fit::FitOptions { shape: self::shape(shape)?, baseline: b, window, ..Default::default() };
    let fits: Vec<Option<fit::PeakFit>> = py.detach(move || {
        (0..c.len()).map(|i| fit::fit_peak(&s, fit::Guess { center: c[i], height: h[i], fwhm: w[i] }, &opts)).collect()
    });
    let col = |f: &dyn Fn(&fit::PeakFit) -> f64| -> Vec<f64> { fits.iter().map(|x| x.as_ref().map_or(f64::NAN, f)).collect() };
    let d = PyDict::new(py);
    d.set_item("center", col(&|p| p.center).into_pyarray(py))?;
    d.set_item("center_err", col(&|p| p.center_err).into_pyarray(py))?;
    d.set_item("amplitude", col(&|p| p.amplitude).into_pyarray(py))?;
    d.set_item("amplitude_err", col(&|p| p.amplitude_err).into_pyarray(py))?;
    d.set_item("fwhm", col(&|p| p.profile.fwhm()).into_pyarray(py))?;
    d.set_item("rms", col(&|p| p.rms).into_pyarray(py))?;
    d.set_item("converged", fits.iter().map(|x| x.as_ref().is_some_and(|p| p.converged)).collect::<Vec<_>>())?;
    Ok(d)
}

/// match_lines(observed, catalog, tolerance=1.0, min_lgint=-inf) -> dict
///
/// `catalog` is the text of a .cat file (for example Prediction.catalog).
#[pyfunction]
#[pyo3(signature = (observed, catalog, tolerance = 1.0, min_lgint = f64::NEG_INFINITY))]
fn match_lines<'py>(py: Python<'py>, observed: PyReadonlyArray1<'py, f64>, catalog: &str, tolerance: f64, min_lgint: f64) -> PyResult<Bound<'py, PyDict>> {
    let cat = CatFile::parse(catalog).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let m = assign::match_lines(slice(&observed, "observed")?, &cat.lines, &assign::MatchOptions { tolerance, min_lgint });
    let d = PyDict::new(py);
    d.set_item("observed", m.iter().map(|a| a.observed).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("predicted", m.iter().map(|a| a.predicted).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("obs", m.iter().map(|a| a.obs).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("calc", m.iter().map(|a| a.calc).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("residual", m.iter().map(|a| a.residual).collect::<Vec<_>>().into_pyarray(py))?;
    d.set_item("ambiguous", m.iter().map(|a| a.ambiguous).collect::<Vec<_>>())?;
    d.set_item("upper", i32_matrix(py, m.iter().map(|a| cat.lines[a.predicted].upper.clone()).collect())?)?;
    d.set_item("lower", i32_matrix(py, m.iter().map(|a| cat.lines[a.predicted].lower.clone()).collect())?)?;
    Ok(d)
}

/// to_lin(catalog, predicted, obs, err) -> str
///
/// A .lin file: line `predicted[i]` of the catalog measured at `obs[i]` with error `err[i]`.
#[pyfunction]
fn to_lin(catalog: &str, predicted: Vec<usize>, obs: PyReadonlyArray1<'_, f64>, err: PyReadonlyArray1<'_, f64>) -> PyResult<String> {
    let cat = CatFile::parse(catalog).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let (obs, err) = (slice(&obs, "obs")?, slice(&err, "err")?);
    if predicted.len() != obs.len() || obs.len() != err.len() {
        return Err(PyValueError::new_err("predicted, obs and err must have the same length"));
    }
    if let Some(&bad) = predicted.iter().find(|&&i| i >= cat.lines.len()) {
        return Err(PyValueError::new_err(format!("predicted index {bad} is beyond the {} catalog lines", cat.lines.len())));
    }
    let a: Vec<assign::Assignment> = predicted
        .iter()
        .zip(obs)
        .enumerate()
        .map(|(k, (&p, &o))| assign::Assignment {
            observed: k, predicted: p, obs: o, calc: cat.lines[p].freq, residual: o - cat.lines[p].freq, ambiguous: false,
        })
        .collect();
    Ok(assign::to_lin(&a, &cat.lines, 0.0, Some(err)).to_text())
}

fn profile(shape: &str, fwhm: f64, fwhm_lorentz: f64) -> PyResult<Profile> {
    Ok(match self::shape(shape)? {
        fit::Shape::Gaussian => Profile::Gaussian { fwhm },
        fit::Shape::Lorentzian => Profile::Lorentzian { fwhm },
        fit::Shape::Voigt => Profile::Voigt { fwhm_gauss: fwhm, fwhm_lorentz },
    })
}

/// synthesize(freq, amplitude, grid, shape="gaussian", fwhm=1.0, fwhm_lorentz=0.0, cutoff=20.0) -> y
#[pyfunction]
#[pyo3(signature = (freq, amplitude, grid, shape = "gaussian", fwhm = 1.0, fwhm_lorentz = 0.0, cutoff = 20.0))]
#[allow(clippy::too_many_arguments)]
fn synthesize<'py>(
    py: Python<'py>, freq: PyReadonlyArray1<'py, f64>, amplitude: PyReadonlyArray1<'py, f64>, grid: PyReadonlyArray1<'py, f64>,
    shape: &str, fwhm: f64, fwhm_lorentz: f64, cutoff: f64,
) -> PyResult<Bound<'py, numpy::PyArray1<f64>>> {
    let (f, a, g) = (slice(&freq, "freq")?, slice(&amplitude, "amplitude")?, slice(&grid, "grid")?);
    if f.len() != a.len() {
        return Err(PyValueError::new_err("freq and amplitude must have the same length"));
    }
    if !g.windows(2).all(|w| w[0] <= w[1]) {
        return Err(PyValueError::new_err("grid must be ascending"));
    }
    let p = profile(shape, fwhm, fwhm_lorentz)?;
    Ok(synth::synthesize(f.iter().copied().zip(a.iter().copied()), g, p, cutoff).into_pyarray(py))
}

/// profile_area(shape="gaussian", fwhm=1.0, fwhm_lorentz=0.0) -> area of a line of height 1
#[pyfunction]
#[pyo3(signature = (shape = "gaussian", fwhm = 1.0, fwhm_lorentz = 0.0))]
fn profile_area(shape: &str, fwhm: f64, fwhm_lorentz: f64) -> PyResult<f64> {
    Ok(profile(shape, fwhm, fwhm_lorentz)?.area())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("PickettError", m.py().get_type::<PickettError>())?;
    m.add_function(wrap_pyfunction!(predict, m)?)?;
    m.add_function(wrap_pyfunction!(predict_many, m)?)?;
    m.add_function(wrap_pyfunction!(fit_lines, m)?)?;
    m.add_function(wrap_pyfunction!(thread_safe, m)?)?;
    m.add_function(wrap_pyfunction!(set_threads, m)?)?;
    m.add_function(wrap_pyfunction!(read_spectrum, m)?)?;
    m.add_function(wrap_pyfunction!(asls, m)?)?;
    m.add_function(wrap_pyfunction!(savgol, m)?)?;
    m.add_function(wrap_pyfunction!(noise_level, m)?)?;
    m.add_function(wrap_pyfunction!(find_peaks, m)?)?;
    m.add_function(wrap_pyfunction!(fit_peaks, m)?)?;
    m.add_function(wrap_pyfunction!(match_lines, m)?)?;
    m.add_function(wrap_pyfunction!(to_lin, m)?)?;
    m.add_function(wrap_pyfunction!(synthesize, m)?)?;
    m.add_function(wrap_pyfunction!(profile_area, m)?)?;
    Ok(())
}
