//! Predict and fit rotational spectra with Pickett's SPCAT/SPFIT, in memory.
//!
//! [`predict`] runs SPCAT (or DPCAT) and [`fit`] runs SPFIT (or DPFIT) on
//! data you pass in. Results come back as full-precision values, the program
//! report and the files the program would have written. No files are created
//! and nothing is printed.
//!
//! ```no_run
//! use pickett::{fit, predict, Flavor, IntFile, LinFile, ParFile};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let par = ParFile::parse(&std::fs::read_to_string("mol.par")?, Flavor::Spinv)?;
//! let lin = LinFile::parse(&std::fs::read_to_string("mol.lin")?)?;
//! let int = IntFile::parse(&std::fs::read_to_string("mol.int")?)?;
//!
//! let result = fit(Flavor::Spinv, &par, &lin)?;
//! for p in &result.params {
//!     println!("{:>8} {:.10} ± {:.2e} {}", p.id, p.value, p.err_fit, p.label);
//! }
//! let spectrum = predict(Flavor::Spinv, &result.var, &int)?;
//! for l in spectrum.lines.iter().filter(|l| l.lgint > -5.0) {
//!     println!("{:.6} MHz  {:?} <- {:?}", l.freq, l.upper, l.lower);
//! }
//! println!("Q(300 K) = {:?}", spectrum.partition_at(300.0));
//! # Ok(())
//! # }
//! ```
//!
//! Calls from different threads run in parallel: the C library keeps its
//! state per thread. [`predict_parallel`] runs a batch of predictions on all
//! cores.

use std::collections::BTreeMap;
use std::fmt;

pub use pickett_io::{CatFile, EgyFile, Flavor, IntFile, LinFile, ParFile, ParseError};
pub use pickett_sys::{
    CatLineRecord as Line, FitIterationRecord as FitIteration, FitParamRecord as FitParam,
    LevelRecord as Level, Program,
};
use pickett_sys::{run_memory, MemoryRun, RunError};

/// Base name of the memory files (`pk.int`, `pk.var`, …).
const BASE: &str = "pk";

/// Why [`predict`] or [`fit`] failed.
#[derive(Debug, Clone)]
pub enum Error {
    /// The program stopped with an error; `log` has its message.
    Run { program: Program, status: i32, log: String },
    /// The program finished but did not write an output it should have
    /// (for example SPFIT writes no `.par` when it does no iteration).
    NoOutput { program: Program, file: String, log: String },
    /// An output file could not be read back.
    Parse { file: String, error: ParseError },
    /// An argument could not be passed to the C code.
    BadArgument(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Run { program, status, log } => {
                write!(f, "{program} failed with status {status}")?;
                if let Some(last) = log.lines().rev().find(|l| !l.trim().is_empty()) {
                    write!(f, ": {}", last.trim())?;
                }
                Ok(())
            }
            Error::NoOutput { program, file, .. } => write!(f, "{program} did not write {file}"),
            Error::Parse { file, error } => write!(f, "cannot read {file}: {error}"),
            Error::BadArgument(a) => write!(f, "bad argument {a:?}"),
        }
    }
}

impl std::error::Error for Error {}

/// The result of [`predict`].
#[derive(Debug, Clone)]
pub struct Prediction {
    /// Catalog lines with full precision, sorted by frequency.
    pub lines: Vec<Line>,
    /// Energy levels with full precision, in calculation order.
    pub levels: Vec<Level>,
    /// `(temperature K, Q)`, from 1000 K down to 1 K (see the manual).
    pub partition: Vec<(f64, f64)>,
    /// The `.cat` file as SPCAT wrote it (rounded, JPL/CDMS format).
    pub catalog: CatFile,
    /// The `.out` report.
    pub report: String,
    /// The program's messages.
    pub log: String,
    /// Every file of the run by name (`pk.cat`, `pk.out`, `pk.egy`, …).
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Prediction {
    /// The partition function at `temp` K. Exact at table points;
    /// in between, `ln Q` is interpolated linearly in `ln T`.
    /// `None` outside the table.
    pub fn partition_at(&self, temp: f64) -> Option<f64> {
        let mut pts: Vec<(f64, f64)> = self.partition.iter().copied().filter(|&(t, q)| t > 0.0 && q > 0.0).collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some(&(_, q)) = pts.iter().find(|p| p.0 == temp) {
            return Some(q);
        }
        let i = pts.iter().position(|p| p.0 > temp)?;
        if i == 0 {
            return None;
        }
        let ((t0, q0), (t1, q1)) = (pts[i - 1], pts[i]);
        let x = (temp.ln() - t0.ln()) / (t1.ln() - t0.ln());
        Some((q0.ln() + x * (q1.ln() - q0.ln())).exp())
    }
}

/// The result of [`fit`].
#[derive(Debug, Clone)]
pub struct FitResult {
    /// Fitted parameters with full precision, in file order.
    pub params: Vec<FitParam>,
    /// Statistics of each iteration.
    pub iterations: Vec<FitIteration>,
    /// The new `.par` (input for the next fit).
    pub par: ParFile,
    /// The `.var` (parameters + covariance, input for [`predict`]).
    pub var: ParFile,
    /// The `.fit` report.
    pub report: String,
    /// The program's messages.
    pub log: String,
    /// Every file of the run by name (`pk.par`, `pk.var`, `pk.fit`, `pk.bak`, `pk.lin`).
    pub files: BTreeMap<String, Vec<u8>>,
}

impl FitResult {
    /// Weighted RMS error (obs−calc)/err after the last iteration.
    pub fn rms_error(&self) -> Option<f64> {
        self.iterations.last().map(|i| i.rms_error_new)
    }
}

/// Predicts a spectrum: runs SPCAT (`Flavor::Spinv`) or DPCAT
/// (`Flavor::Dpi`) with parameters `var` and intensity settings `int`.
pub fn predict(flavor: Flavor, var: &ParFile, int: &IntFile) -> Result<Prediction, Error> {
    predict_text(flavor, &var.to_text(), &int.to_text())
}

/// Like [`predict`], with the `.var` and `.int` contents as text.
pub fn predict_text(flavor: Flavor, var: &str, int: &str) -> Result<Prediction, Error> {
    let program = match flavor {
        Flavor::Spinv => Program::Spcat,
        Flavor::Dpi => Program::Dpcat,
    };
    let run = run_memory(program, BASE, &[("pk.var", var.as_bytes()), ("pk.int", int.as_bytes())]);
    let run = check(program, run)?;
    let catalog = CatFile::parse(&need_text(program, &run, "cat")?)
        .map_err(|error| Error::Parse { file: "pk.cat".into(), error })?;
    let report = need_text(program, &run, "out")?;
    let MemoryRun { log, files, records, .. } = run;
    let mut lines = records.cat_lines;
    lines.sort_by(|a, b| a.freq.total_cmp(&b.freq)); // stable: ties keep calculation order
    Ok(Prediction { lines, levels: records.levels, partition: records.partition, catalog, report, log, files })
}

/// Runs many predictions at once on `threads` threads (all CPU cores when
/// `None`). Each prediction then runs on one thread (see
/// [`pickett_sys::set_threads`]). Results are in the order of `jobs` and are the same as calling
/// [`predict`] for each job. Useful for parameter scans or Monte-Carlo
/// estimates of prediction uncertainty.
pub fn predict_parallel(
    flavor: Flavor,
    jobs: &[(ParFile, IntFile)],
    threads: Option<usize>,
) -> Vec<Result<Prediction, Error>> {
    let n = threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .clamp(1, jobs.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut done: Vec<(usize, Result<Prediction, Error>)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..n)
            .map(|_| {
                s.spawn(|| {
                    // every core already runs a whole prediction
                    pickett_sys::set_threads(1);
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((var, int)) = jobs.get(i) else { break };
                        out.push((i, predict(flavor, var, int)));
                    }
                    out
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().expect("prediction thread panicked")).collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// Fits parameters to assigned lines: runs SPFIT (`Flavor::Spinv`) or DPFIT
/// (`Flavor::Dpi`) with starting parameters `par` and lines `lin`.
pub fn fit(flavor: Flavor, par: &ParFile, lin: &LinFile) -> Result<FitResult, Error> {
    fit_text(flavor, &par.to_text(), &lin.to_text())
}

/// Like [`fit`], with the `.par` and `.lin` contents as text.
pub fn fit_text(flavor: Flavor, par: &str, lin: &str) -> Result<FitResult, Error> {
    let program = match flavor {
        Flavor::Spinv => Program::Spfit,
        Flavor::Dpi => Program::Dpfit,
    };
    let run = run_memory(program, BASE, &[("pk.par", par.as_bytes()), ("pk.lin", lin.as_bytes())]);
    let run = check(program, run)?;
    // With no iteration SPFIT writes nothing (and `.par` is still the input),
    // so `.var` shows whether there is a result.
    need_text(program, &run, "var")?;
    let read_par = |ext: &str| -> Result<ParFile, Error> {
        let text = need_text(program, &run, ext)?;
        ParFile::parse(&text, flavor).map_err(|error| Error::Parse { file: format!("pk.{ext}"), error })
    };
    let par = read_par("par")?;
    let var = read_par("var")?;
    let report = need_text(program, &run, "fit")?;
    let MemoryRun { log, files, records, .. } = run;
    Ok(FitResult { params: records.fit_params, iterations: records.fit_iterations, par, var, report, log, files })
}

fn check(program: Program, run: MemoryRun) -> Result<MemoryRun, Error> {
    match &run.status {
        Ok(()) => Ok(run),
        Err(RunError::Failed { status, .. }) => Err(Error::Run { program, status: *status, log: run.log }),
        Err(RunError::BadArgument(a)) => Err(Error::BadArgument(a.clone())),
    }
}

/// An output file the program must have written.
fn need_text(program: Program, run: &MemoryRun, ext: &str) -> Result<String, Error> {
    let name = format!("{BASE}.{ext}");
    run.file_text(&name).ok_or_else(|| Error::NoOutput { program, file: name, log: run.log.clone() })
}
