//! Runs the SPCAT/SPFIT/DPCAT/DPFIT golden cases of `test/golden/` in-process
//! and compares the results with the expected files:
//! `.cat` and `.egy` must match exactly; `.par`, `.var` and `.bak` must have
//! the same parameters and covariance within a relative tolerance (the fit
//! may differ in the last digits between compilers). `.fit`, `.out` and
//! `.str` are checked by `python test/run_golden.py --inprocess`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use pickett_io::{Flavor, ParFile};
use pickett_sys::{run_in, Program, RunError};

const RTOL: f64 = 1e-6;

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/golden")
}

/// A fresh, empty temporary directory.
fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "pickett_sys_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn copy_inputs(case: &Path, work: &Path) {
    for e in fs::read_dir(case.join("input")).unwrap() {
        let p = e.unwrap().path();
        fs::copy(&p, work.join(p.file_name().unwrap())).unwrap();
    }
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())).replace("\r\n", "\n")
}

/// Program and base name for a golden case, from its directory name and inputs.
fn case_program(case: &Path) -> Option<(Program, String)> {
    let name = case.file_name()?.to_str()?;
    let program = match name.split('_').next()? {
        "spcat" => Program::Spcat,
        "spfit" => Program::Spfit,
        "dpcat" => Program::Dpcat,
        "dpfit" => Program::Dpfit,
        _ => return None,
    };
    let key_ext = if matches!(program, Program::Spcat | Program::Dpcat) { "int" } else { "par" };
    let base = fs::read_dir(case.join("input"))
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|x| x == key_ext))?
        .file_stem()?
        .to_str()?
        .to_string();
    Some((program, base))
}

fn close(a: f64, b: f64, abs: f64) -> bool {
    (a - b).abs() <= RTOL * a.abs().max(b.abs()) + abs
}

fn compare_par(exp: &Path, act: &Path, flavor: Flavor) -> Result<(), String> {
    let e = ParFile::parse(&read(exp), flavor).map_err(|x| x.to_string())?;
    let a = ParFile::parse(&read(act), flavor).map_err(|x| x.to_string())?;
    if e.params.len() != a.params.len() {
        return Err(format!("{} vs {} parameters", e.params.len(), a.params.len()));
    }
    for (pe, pa) in e.params.iter().zip(&a.params) {
        if pe.id != pa.id || pe.label != pa.label {
            return Err(format!("parameter {} /{} vs {} /{}", pe.id, pe.label, pa.id, pa.label));
        }
        if !close(pe.value, pa.value, 1e-12) {
            return Err(format!("{} value {} vs {}", pe.id, pe.value, pa.value));
        }
        match (pe.error, pa.error) {
            (Some(x), Some(y)) if !close(x, y, 1e-30) => {
                return Err(format!("{} error {x} vs {y}", pe.id));
            }
            (x, y) if x.is_some() != y.is_some() => return Err(format!("{} error {x:?} vs {y:?}", pe.id)),
            _ => {}
        }
    }
    match (e.covariance(), a.covariance()) {
        (Some(ce), Some(ca)) => {
            for (i, (x, y)) in ce.iter().zip(&ca).enumerate() {
                // printed with 7 decimals: allow a couple of units in the last digit
                if (x - y).abs() > 2.5e-7 {
                    return Err(format!("covariance[{i}] {x} vs {y}"));
                }
            }
        }
        (None, None) => {}
        (x, y) => return Err(format!("covariance present: {} vs {}", x.is_some(), y.is_some())),
    }
    Ok(())
}

/// Compares every expected file this test handles; returns the problems.
fn compare(case: &Path, work: &Path, flavor: Flavor) -> Vec<String> {
    let mut problems = Vec::new();
    for e in fs::read_dir(case.join("expected")).unwrap() {
        let exp = e.unwrap().path();
        let fname = exp.file_name().unwrap().to_string_lossy().into_owned();
        let act = work.join(&fname);
        let ext = exp.extension().and_then(|x| x.to_str()).unwrap_or("");
        let result = match ext {
            "cat" | "egy" => {
                if !act.exists() {
                    Err("not produced".to_string())
                } else if read(&exp) == read(&act) {
                    Ok(())
                } else {
                    Err("differs".to_string())
                }
            }
            "par" | "var" | "bak" => {
                if act.exists() { compare_par(&exp, &act, flavor) } else { Err("not produced".to_string()) }
            }
            _ => continue,
        };
        if let Err(m) = result {
            problems.push(format!("{fname}: {m}"));
        }
    }
    problems
}

#[test]
fn golden_cases_in_process_twice() {
    let mut cases: Vec<PathBuf> = fs::read_dir(golden()).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
    cases.sort();
    let mut failures = Vec::new();
    let mut ran = 0;
    for case in &cases {
        let Some((program, base)) = case_program(case) else { continue };
        let flavor = if matches!(program, Program::Dpcat | Program::Dpfit) { Flavor::Dpi } else { Flavor::Spinv };
        let work = temp_dir(&base);
        for pass in 1..=2 {
            copy_inputs(case, &work); // restore inputs: SPFIT rewrites its .par
            if let Err(e) = run_in(program, &work, &base) {
                failures.push(format!("{}: run {pass}: {e}", case.display()));
            }
        }
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        failures.extend(compare(case, &work, flavor).into_iter().map(|p| format!("{name}/{p}")));
        let _ = fs::remove_dir_all(&work);
        ran += 1;
    }
    assert!(ran >= 6, "only {ran} golden cases found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn failure_returns_error_and_next_run_works() {
    let work = temp_dir("fail");
    let err = run_in(Program::Spcat, &work, "missing").unwrap_err();
    assert!(matches!(err, RunError::Failed { program: Program::Spcat, status } if status != 0), "{err:?}");
    let err = run_in(Program::Dpfit, &work, "missing").unwrap_err();
    assert!(matches!(err, RunError::Failed { program: Program::Dpfit, .. }), "{err:?}");

    // the process is still alive and the library still works
    let case = golden().join("spcat_prolate");
    copy_inputs(&case, &work);
    run_in(Program::Spcat, &work, "prolate").unwrap();
    assert!(compare(&case, &work, Flavor::Spinv).is_empty());
    let _ = fs::remove_dir_all(&work);
}

#[test]
fn concurrent_callers_are_serialized() {
    let case = golden().join("spcat_asym");
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let case = case.clone();
            std::thread::spawn(move || {
                let work = temp_dir("thread");
                copy_inputs(&case, &work);
                run_in(Program::Spcat, &work, "asym").unwrap();
                let problems = compare(&case, &work, Flavor::Spinv);
                let _ = fs::remove_dir_all(&work);
                problems
            })
        })
        .collect();
    for h in handles {
        let problems = h.join().unwrap();
        assert!(problems.is_empty(), "{problems:?}");
    }
}

#[test]
fn directory_with_dots_in_name() {
    let work = temp_dir("dots.v1.2");
    let case = golden().join("dpcat_pi2");
    copy_inputs(&case, &work);
    run_in(Program::Dpcat, &work, "pi2").unwrap();
    assert!(compare(&case, &work, Flavor::Dpi).is_empty());
    let _ = fs::remove_dir_all(&work);
}
