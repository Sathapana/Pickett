//! Runs SPCAT, SPFIT, DPCAT and DPFIT inside the current process.
//!
//! The C code of the CALPGM suite is compiled by `build.rs` and linked into
//! this crate. [`run`] calls a program the way the command line would, and
//! [`run_in`] runs it on the files `<dir>/<base>.<ext>`:
//!
//! ```no_run
//! use pickett_sys::{run_in, Program};
//! use std::path::Path;
//!
//! // Reads mol.int and mol.var, writes mol.cat, mol.out, ... in data/
//! run_in(Program::Spcat, Path::new("data"), "mol")?;
//! # Ok::<(), pickett_sys::RunError>(())
//! ```
//!
//! [`run_memory`] runs a program without files or console output: the inputs
//! are byte buffers, and the result holds the output files, the captured
//! messages and full-precision records of the results (catalog lines,
//! energy levels, partition function, fitted parameters).
//!
//! Threads: the C library keeps its state per thread (see `pktls.h`), so
//! runs on different threads proceed in parallel and give the same results
//! as on one thread. If it was built with `PK_NO_TLS`, [`thread_safe`] is
//! false and runs are serialized with a global lock instead.
//!
//! Limits of this layer:
//! * A fatal error returns [`RunError`] instead of ending the process, and
//!   the files the program opened are closed. Memory it had allocated is
//!   not freed, so a long-running host should not rely on many failed runs.

use std::ffi::{c_char, c_int, CString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

mod memory;
pub use memory::{run_memory, CatLineRecord, FitIterationRecord, FitParamRecord, LevelRecord, MemoryRun, Records};

/// Raw entry points (see `pickett.h` in the repository root).
pub mod ffi {
    use std::ffi::{c_char, c_int, c_long, c_short, c_void};

    /// `PK_SINK` in `pickett.h`.
    #[repr(C)]
    pub struct PkSink {
        pub user: *mut c_void,
        pub cat_line: Option<
            unsafe extern "C" fn(
                user: *mut c_void, freq: f64, err: f64, lgint: f64, str_: f64, dr: c_int, elow: f64,
                gup: c_int, tag: c_long, qnfmt: c_int, nqn: c_int, upper: *const c_short,
                lower: *const c_short,
            ),
        >,
        pub level: Option<
            unsafe extern "C" fn(
                user: *mut c_void, block: c_int, index: c_int, energy: f64, err: f64, mixing: f64,
                degeneracy: c_int, ktsp: c_int, nqn: c_int, qn: *const c_short,
            ),
        >,
        pub partition: Option<unsafe extern "C" fn(user: *mut c_void, temp: f64, q: f64)>,
        pub fit_iteration: Option<
            unsafe extern "C" fn(
                user: *mut c_void, iteration: c_int, rms_error_old: f64, rms_error_new: f64,
                rms_mw: f64, rms_ir: f64, avg_mw: f64, avg_ir: f64, nlines_used: c_int,
                nlines_rejected: c_int,
            ),
        >,
        pub fit_param: Option<
            unsafe extern "C" fn(
                user: *mut c_void, index: c_int, id: *const c_char, value: f64, err_fit: f64,
                err_par: f64, label: *const c_char,
            ),
        >,
    }

    pub const PK_LOG_STDOUT: c_int = 0;
    pub const PK_LOG_CAPTURE: c_int = 1;
    pub const PK_LOG_DISCARD: c_int = 2;

    extern "C" {
        pub fn pk_spcat_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_spfit_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_dpcat_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_dpfit_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_library_mode(on: c_int);
        pub fn pk_thread_safe() -> c_int;
        pub fn pk_set_threads(n: c_int);
        pub fn pk_get_threads() -> c_int;

        pub fn pk_vfs_begin();
        pub fn pk_vfs_end();
        pub fn pk_vfs_put(name: *const c_char, data: *const c_char, len: usize) -> c_int;
        pub fn pk_vfs_get(name: *const c_char, data: *mut *const c_char, len: *mut usize) -> c_int;
        pub fn pk_vfs_count() -> c_int;
        pub fn pk_vfs_name(k: c_int) -> *const c_char;

        pub fn pk_log_mode(mode: c_int);
        pub fn pk_log_text(len: *mut usize) -> *const c_char;

        pub fn pk_set_sink(sink: *const PkSink);
    }
}

/// One of the four main programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Program {
    /// Prediction with the general Hamiltonian (`spinv.c`).
    Spcat,
    /// Fitting with the general Hamiltonian.
    Spfit,
    /// Prediction for doublet-Π states (`dpi.c`).
    Dpcat,
    /// Fitting for doublet-Π states.
    Dpfit,
}

impl Program {
    /// The program's name on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Program::Spcat => "spcat",
            Program::Spfit => "spfit",
            Program::Dpcat => "dpcat",
            Program::Dpfit => "dpfit",
        }
    }

    /// File extensions the program uses, in the order it expects them.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Program::Spcat | Program::Dpcat => &["int", "var", "out", "cat", "str", "egy"],
            Program::Spfit | Program::Dpfit => &["par", "lin", "fit", "bak", "var"],
        }
    }

    fn entry(self) -> unsafe extern "C" fn(c_int, *mut *mut c_char) -> c_int {
        match self {
            Program::Spcat => ffi::pk_spcat_run,
            Program::Spfit => ffi::pk_spfit_run,
            Program::Dpcat => ffi::pk_dpcat_run,
            Program::Dpfit => ffi::pk_dpfit_run,
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a run failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// The program stopped with this status. Its message is on stdout, or
    /// in [`MemoryRun::log`] for [`run_memory`].
    Failed { program: Program, status: i32 },
    /// An argument could not be passed to C (contains a NUL byte or is not
    /// valid UTF-8).
    BadArgument(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Failed { program, status } => {
                write!(f, "{program} failed with status {status} (see the program's messages)")
            }
            RunError::BadArgument(a) => write!(f, "argument cannot be passed to C: {a:?}"),
        }
    }
}

impl std::error::Error for RunError {}

/// True when the C library keeps its state per thread, so runs on
/// different threads proceed in parallel. When false, runs are serialized.
pub fn thread_safe() -> bool {
    // SAFETY: a pure query of a compile-time setting.
    unsafe { ffi::pk_thread_safe() != 0 }
}

/// Sets how many threads one run on the *calling* thread may use for its
/// own work (currently the partition-function table of SPCAT/DPCAT).
/// `0` means one per CPU core (the default); `1` means no extra threads,
/// which is right when many runs already run in parallel. Results do not
/// depend on this setting.
pub fn set_threads(n: usize) {
    // SAFETY: sets a per-thread integer in the C library.
    unsafe { ffi::pk_set_threads(n.min(c_int::MAX as usize) as c_int) }
}

/// The number of threads a run on the calling thread would use now.
pub fn threads() -> usize {
    // SAFETY: reads a per-thread integer in the C library.
    unsafe { ffi::pk_get_threads().max(1) as usize }
}

static LOCK: Mutex<()> = Mutex::new(());

/// Serializes runs when the C library is not thread-safe; otherwise every
/// thread uses its own C state and no lock is needed.
pub(crate) fn lock() -> Option<std::sync::MutexGuard<'static, ()>> {
    (!thread_safe()).then(|| LOCK.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Calls the program's run function. The caller must hold [`lock`].
pub(crate) fn call_locked<S: AsRef<str>>(program: Program, args: &[S]) -> Result<(), RunError> {
    let mut owned = Vec::with_capacity(args.len() + 1);
    owned.push(CString::new(program.name()).unwrap());
    for a in args {
        let a = a.as_ref();
        owned.push(CString::new(a).map_err(|_| RunError::BadArgument(a.to_string()))?);
    }
    // The C code may write into argv strings it does not own, so pass
    // private, mutable copies.
    let mut raw: Vec<*mut c_char> = owned.into_iter().map(CString::into_raw).collect();
    let argc = raw.len() as c_int;
    raw.push(std::ptr::null_mut());
    // SAFETY: argv holds argc valid NUL-terminated strings followed by a
    // null pointer, all alive until after the call; the caller holds
    // [`lock`]: the C state is this thread's own, or no other run is active.
    let status = unsafe {
        ffi::pk_library_mode(1);
        (program.entry())(argc, raw.as_mut_ptr())
    };
    for p in raw.into_iter().filter(|p| !p.is_null()) {
        // SAFETY: each pointer came from CString::into_raw above.
        drop(unsafe { CString::from_raw(p) });
    }
    if status == 0 {
        Ok(())
    } else {
        Err(RunError::Failed { program, status })
    }
}

/// Runs `program` with command-line arguments `args` (without the program
/// name), for example `["mol"]` or explicit file names. Relative names are
/// relative to the process's current directory. Messages go to stdout.
pub fn run<S: AsRef<str>>(program: Program, args: &[S]) -> Result<(), RunError> {
    let _guard = lock();
    call_locked(program, args)
}

/// Runs `program` on the files `<dir>/<base>.<ext>` for every extension it
/// uses (see [`Program::extensions`]). Every file name is passed
/// explicitly, so directory names containing dots are safe.
pub fn run_in(program: Program, dir: &Path, base: &str) -> Result<(), RunError> {
    let names = file_names(program, dir, base)?;
    run(program, &names)
}

/// The paths [`run_in`] passes for `program`.
pub fn file_paths(program: Program, dir: &Path, base: &str) -> Vec<PathBuf> {
    program.extensions().iter().map(|e| dir.join(format!("{base}.{e}"))).collect()
}

fn file_names(program: Program, dir: &Path, base: &str) -> Result<Vec<String>, RunError> {
    file_paths(program, dir, base)
        .into_iter()
        .map(|p| p.to_str().map(str::to_string).ok_or_else(|| RunError::BadArgument(p.display().to_string())))
        .collect()
}
