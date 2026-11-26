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
//! What this layer does not do yet (planned for later phases):
//! * The programs still read and write files, and print their progress on
//!   stdout. Use `pickett-io` to read the results.
//! * The C code keeps state in static variables, so only one run can be
//!   active at a time. Calls are serialized with a global lock, so calling
//!   from several threads is safe but not parallel.
//! * A fatal error returns [`RunError`] instead of ending the process, and
//!   the files the program opened are closed. Memory it had allocated is
//!   not freed, so a long-running host should not rely on many failed runs.

use std::ffi::{c_char, c_int, CString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Raw entry points (see `pickett.h` in the repository root).
pub mod ffi {
    use std::ffi::{c_char, c_int};

    extern "C" {
        pub fn pk_spcat_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_spfit_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_dpcat_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_dpfit_run(argc: c_int, argv: *mut *mut c_char) -> c_int;
        pub fn pk_library_mode(on: c_int);
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
    /// The program stopped with this status. Its message is on stdout.
    Failed { program: Program, status: i32 },
    /// An argument could not be passed to C (contains a NUL byte or is not
    /// valid UTF-8).
    BadArgument(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Failed { program, status } => {
                write!(f, "{program} failed with status {status} (see its message on stdout)")
            }
            RunError::BadArgument(a) => write!(f, "argument cannot be passed to C: {a:?}"),
        }
    }
}

impl std::error::Error for RunError {}

static LOCK: Mutex<()> = Mutex::new(());

/// Runs `program` with command-line arguments `args` (without the program
/// name), for example `["mol"]` or explicit file names. Relative names are
/// relative to the process's current directory.
pub fn run<S: AsRef<str>>(program: Program, args: &[S]) -> Result<(), RunError> {
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

    let status = {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: argv holds argc valid NUL-terminated strings followed by a
        // null pointer, all alive until after the call; the lock ensures no
        // other run is active (the C code is not reentrant).
        unsafe {
            ffi::pk_library_mode(1);
            (program.entry())(argc, raw.as_mut_ptr())
        }
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
