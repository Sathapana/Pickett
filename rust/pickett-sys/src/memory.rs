//! Runs without files or console output (`pk_vfs_*`, `pk_log_*`,
//! `pk_set_sink` in `pickett.h`).

use std::collections::BTreeMap;
use std::ffi::{c_char, c_int, c_long, c_short, c_void, CStr, CString};

use crate::ffi::{self, PkSink};
use crate::{call_locked, lock, Program, RunError};

/// One catalog line from SPCAT/DPCAT, with full precision.
#[derive(Debug, Clone, PartialEq)]
pub struct CatLineRecord {
    /// Frequency (MHz; cm⁻¹ in IR mode).
    pub freq: f64,
    /// Estimated error of the frequency, same unit.
    pub err: f64,
    /// log₁₀ of the intensity (nm² MHz) at the `.int` temperature.
    pub lgint: f64,
    /// Line strength S·μ² (Debye²).
    pub strength: f64,
    /// Degrees of freedom of the rotational partition function.
    pub dr: i32,
    /// Lower-state energy (cm⁻¹).
    pub elow: f64,
    /// Upper-state degeneracy.
    pub gup: i32,
    pub tag: i64,
    pub qnfmt: i32,
    pub upper: Vec<i32>,
    pub lower: Vec<i32>,
}

/// One energy level from SPCAT/DPCAT, with full precision.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelRecord {
    pub block: i32,
    pub index: i32,
    /// Energy (cm⁻¹).
    pub energy: f64,
    pub err: f64,
    pub mixing: f64,
    pub degeneracy: i32,
    /// The `(nnn)` field of `.egy`, if any.
    pub ktsp: Option<i32>,
    pub qn: Vec<i32>,
}

/// Statistics at the end of one SPFIT/DPFIT iteration (as printed in `.fit`).
#[derive(Debug, Clone, PartialEq)]
pub struct FitIterationRecord {
    pub iteration: i32,
    /// Weighted RMS error (obs−calc)/err before and after this iteration.
    pub rms_error_old: f64,
    pub rms_error_new: f64,
    /// RMS of obs−calc for microwave lines (MHz) and IR lines (cm⁻¹).
    pub rms_mw: f64,
    pub rms_ir: f64,
    pub avg_mw: f64,
    pub avg_ir: f64,
    pub lines_used: i32,
    pub lines_rejected: i32,
}

/// One parameter as SPFIT/DPFIT writes it to `.par`/`.var`, with full precision.
#[derive(Debug, Clone, PartialEq)]
pub struct FitParamRecord {
    pub index: i32,
    /// The `IDPAR` digits, with `-` for a tied parameter.
    pub id: String,
    pub value: f64,
    /// Fitted error (the `.var` column).
    pub err_fit: f64,
    /// Error written to the new `.par`.
    pub err_par: f64,
    pub label: String,
}

/// Everything the result callbacks reported during a run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Records {
    /// In calculation order; the `.cat` file is sorted by frequency.
    pub cat_lines: Vec<CatLineRecord>,
    pub levels: Vec<LevelRecord>,
    /// `(temperature K, Q)` pairs, as in the `.out` table.
    pub partition: Vec<(f64, f64)>,
    pub fit_iterations: Vec<FitIterationRecord>,
    pub fit_params: Vec<FitParamRecord>,
}

/// The result of [`run_memory`].
#[derive(Debug, Clone)]
pub struct MemoryRun {
    pub status: Result<(), RunError>,
    /// Every memory file after the run (inputs and outputs) by name, for
    /// example `"mol.cat"`. Line ends are `\n`.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The program's messages (what it would have printed on stdout).
    pub log: String,
    pub records: Records,
}

impl MemoryRun {
    /// A memory file as text (lossy UTF-8), if it exists.
    pub fn file_text(&self, name: &str) -> Option<String> {
        self.files.get(name).map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

/// Runs `program` on memory files. `inputs` are `(name, contents)` pairs
/// named `<base>.<ext>`, for example `("mol.int", …)` and `("mol.var", …)`
/// for SPCAT with `base = "mol"`. Nothing is read from or written to named
/// files (scratch data goes to anonymous temporary files, as in the
/// programs), and nothing is printed.
///
/// `base` must not contain `.`, `/` or `\`.
pub fn run_memory(program: Program, base: &str, inputs: &[(&str, &[u8])]) -> MemoryRun {
    let bad = |s: &str| MemoryRun {
        status: Err(RunError::BadArgument(s.to_string())),
        files: BTreeMap::new(),
        log: String::new(),
        records: Records::default(),
    };
    if base.is_empty() || base.contains(['.', '/', '\\']) {
        return bad(base);
    }
    let mut c_inputs = Vec::with_capacity(inputs.len());
    for (name, data) in inputs {
        match CString::new(*name) {
            Ok(n) => c_inputs.push((n, *data)),
            Err(_) => return bad(name),
        }
    }

    let mut records = Box::new(Records::default());
    let sink = PkSink {
        user: (&mut *records as *mut Records).cast::<c_void>(),
        cat_line: Some(on_cat_line),
        level: Some(on_level),
        partition: Some(on_partition),
        fit_iteration: Some(on_fit_iteration),
        fit_param: Some(on_fit_param),
    };

    let _guard = lock();
    let mut files = BTreeMap::new();
    let (status, log);
    // SAFETY: the lock gives this thread sole use of the C library state.
    // `sink` and `records` outlive the run and are unregistered before they
    // are dropped. All strings passed are NUL-terminated or have lengths.
    unsafe {
        ffi::pk_vfs_begin();
        for (name, data) in &c_inputs {
            ffi::pk_vfs_put(name.as_ptr(), data.as_ptr().cast::<c_char>(), data.len());
        }
        ffi::pk_log_mode(ffi::PK_LOG_CAPTURE);
        ffi::pk_set_sink(&sink);

        status = call_locked(program, &[base]);

        ffi::pk_set_sink(std::ptr::null());
        let mut len = 0usize;
        let text = ffi::pk_log_text(&mut len);
        log = String::from_utf8_lossy(bytes(text, len)).into_owned();
        ffi::pk_log_mode(ffi::PK_LOG_STDOUT);

        for k in 0..ffi::pk_vfs_count() {
            let name_ptr = ffi::pk_vfs_name(k);
            if name_ptr.is_null() {
                continue;
            }
            let mut data: *const c_char = std::ptr::null();
            let mut len = 0usize;
            if ffi::pk_vfs_get(name_ptr, &mut data, &mut len) == 0 {
                let name = CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
                files.insert(name, bytes(data, len).to_vec());
            }
        }
        ffi::pk_vfs_end();
    }
    drop(_guard);
    MemoryRun { status, files, log, records: *records }
}

/// # Safety
/// `p` must point to `len` readable bytes, or `len` must be 0.
unsafe fn bytes<'a>(p: *const c_char, len: usize) -> &'a [u8] {
    if p.is_null() || len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p.cast::<u8>(), len)
    }
}

/// # Safety
/// `p` must point to `n` readable shorts, or `n` must be 0.
unsafe fn quanta(p: *const c_short, n: c_int) -> Vec<i32> {
    if p.is_null() || n <= 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(p, n as usize).iter().map(|&q| q as i32).collect()
    }
}

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().trim_end().to_string()
    }
}

/// # Safety (all callbacks)
/// Called by the C code with `user` = the `Records` registered in
/// [`run_memory`] and pointer arguments valid for the duration of the call.
unsafe extern "C" fn on_cat_line(
    user: *mut c_void, freq: f64, err: f64, lgint: f64, str_: f64, dr: c_int, elow: f64, gup: c_int,
    tag: c_long, qnfmt: c_int, nqn: c_int, upper: *const c_short, lower: *const c_short,
) {
    let r = &mut *user.cast::<Records>();
    r.cat_lines.push(CatLineRecord {
        freq,
        err,
        lgint,
        strength: str_,
        dr,
        elow,
        gup,
        tag: tag as i64,
        qnfmt,
        upper: quanta(upper, nqn),
        lower: quanta(lower, nqn),
    });
}

unsafe extern "C" fn on_level(
    user: *mut c_void, block: c_int, index: c_int, energy: f64, err: f64, mixing: f64, degeneracy: c_int,
    ktsp: c_int, nqn: c_int, qn: *const c_short,
) {
    let r = &mut *user.cast::<Records>();
    r.levels.push(LevelRecord {
        block,
        index,
        energy,
        err,
        mixing,
        degeneracy,
        ktsp: (ktsp >= 0).then_some(ktsp),
        qn: quanta(qn, nqn),
    });
}

unsafe extern "C" fn on_partition(user: *mut c_void, temp: f64, q: f64) {
    (*user.cast::<Records>()).partition.push((temp, q));
}

unsafe extern "C" fn on_fit_iteration(
    user: *mut c_void, iteration: c_int, rms_error_old: f64, rms_error_new: f64, rms_mw: f64, rms_ir: f64,
    avg_mw: f64, avg_ir: f64, lines_used: c_int, lines_rejected: c_int,
) {
    (*user.cast::<Records>()).fit_iterations.push(FitIterationRecord {
        iteration,
        rms_error_old,
        rms_error_new,
        rms_mw,
        rms_ir,
        avg_mw,
        avg_ir,
        lines_used,
        lines_rejected,
    });
}

unsafe extern "C" fn on_fit_param(
    user: *mut c_void, index: c_int, id: *const c_char, value: f64, err_fit: f64, err_par: f64,
    label: *const c_char,
) {
    (*user.cast::<Records>()).fit_params.push(FitParamRecord {
        index,
        id: cstr(id),
        value,
        err_fit,
        err_par,
        label: cstr(label),
    });
}
