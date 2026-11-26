//! `predict` and `fit` on the golden cases of `test/golden/`.

use std::fs;
use std::path::{Path, PathBuf};

use pickett::{fit, fit_text, predict, predict_parallel, predict_text, CatFile, Error, Flavor, IntFile, LinFile, ParFile};

fn golden(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/golden").join(rel)
}

fn read(rel: &str) -> String {
    fs::read_to_string(golden(rel)).unwrap().replace("\r\n", "\n")
}

#[test]
fn predict_text_reproduces_golden_catalogs() {
    for (case, base, flavor) in [
        ("spcat_prolate", "prolate", Flavor::Spinv),
        ("spcat_asym", "asym", Flavor::Spinv),
        ("dpcat_pi2", "pi2", Flavor::Dpi),
    ] {
        let p = predict_text(
            flavor,
            &read(&format!("{case}/input/{base}.var")),
            &read(&format!("{case}/input/{base}.int")),
        )
        .unwrap_or_else(|e| panic!("{case}: {e}"));
        let expected = read(&format!("{case}/expected/{base}.cat"));
        assert_eq!(p.catalog.to_text(), expected, "{case}: .cat");
        assert_eq!(p.lines.len(), p.catalog.lines.len(), "{case}");
        assert!(p.lines.windows(2).all(|w| w[0].freq <= w[1].freq), "{case}: sorted");
        // full-precision lines round to the catalog's 4 decimals
        for (r, c) in p.lines.iter().zip(&p.catalog.lines) {
            assert!((r.freq - c.freq).abs() <= 0.5e-4 + 1e-9, "{case}: {} vs {}", r.freq, c.freq);
        }
        assert!(!p.levels.is_empty() && !p.report.is_empty() && !p.log.is_empty(), "{case}");
    }
}

#[test]
fn partition_function_table_and_interpolation() {
    let p = predict_text(
        Flavor::Spinv,
        &read("spcat_prolate/input/prolate.var"),
        &read("spcat_prolate/input/prolate.int"),
    )
    .unwrap();
    assert_eq!(p.partition.len(), 1000);
    assert_eq!(p.partition[0].0, 1000.0);
    let q300 = p.partition_at(300.0).unwrap();
    assert!((q300 - 7820.3911).abs() < 1e-3, "{q300}");
    // between table points the value lies between its neighbours
    let (q299, q301) = (p.partition_at(299.0).unwrap(), p.partition_at(301.0).unwrap());
    let q = p.partition_at(300.5).unwrap();
    assert!(q > q300 && q < q301 && q299 < q300, "{q299} {q300} {q} {q301}");
    assert_eq!(p.partition_at(0.5), None);
    assert_eq!(p.partition_at(2000.0), None);
}

#[test]
fn typed_predict_matches_text_predict() {
    let var = ParFile::parse(&read("dpcat_pi2/input/pi2.var"), Flavor::Dpi).unwrap();
    let int = IntFile::parse(&read("dpcat_pi2/input/pi2.int")).unwrap();
    let typed = predict(Flavor::Dpi, &var, &int).unwrap();
    assert_eq!(typed.catalog.to_text(), read("dpcat_pi2/expected/pi2.cat"));
}

#[test]
fn fit_reproduces_golden_parameters() {
    for (case, base, flavor) in [
        ("spfit_asym", "asym", Flavor::Spinv),
        ("spfit_prolate", "prolate", Flavor::Spinv),
        ("dpfit_pi2", "pi2", Flavor::Dpi),
    ] {
        let par = ParFile::parse(&read(&format!("{case}/input/{base}.par")), flavor).unwrap();
        let lin = LinFile::parse(&read(&format!("{case}/input/{base}.lin"))).unwrap();
        let r = fit(flavor, &par, &lin).unwrap_or_else(|e| panic!("{case}: {e}"));
        let expected = ParFile::parse(&read(&format!("{case}/expected/{base}.var")), flavor).unwrap();
        assert_eq!(r.params.len(), expected.params.len(), "{case}");
        for (p, e) in r.params.iter().zip(&expected.params) {
            assert_eq!(p.id, e.id.to_string(), "{case}");
            let tol = 1e-6 * e.value.abs().max(p.value.abs()) + 1e-12;
            assert!((p.value - e.value).abs() <= tol, "{case} {}: {} vs {}", p.id, p.value, e.value);
        }
        // the typed .var agrees with the full-precision records
        for (p, v) in r.params.iter().zip(&r.var.params) {
            assert!((p.value - v.value).abs() <= 2e-15 * p.value.abs() + 1e-300, "{case} {}", p.id);
        }
        assert!(!r.iterations.is_empty() && r.rms_error().is_some(), "{case}");
        assert!(r.report.contains("END OF ITERATION"), "{case}");
        assert!(r.var.covariance().is_some(), "{case}: covariance");
    }
}

#[test]
fn fit_then_predict_workflow() {
    // fit the asymmetric top from perturbed starting values, then predict
    // with the fitted .var and the spcat_asym intensity settings
    let r = fit_text(Flavor::Spinv, &read("spfit_asym/input/asym.par"), &read("spfit_asym/input/asym.lin")).unwrap();
    let int = IntFile::parse(&read("spcat_asym/input/asym.int")).unwrap();
    let p = predict(Flavor::Spinv, &r.var, &int).unwrap();
    let reference = CatFile::parse(&read("spcat_asym/expected/asym.cat")).unwrap();
    assert_eq!(p.catalog.lines.len(), reference.lines.len());
    // the lines were simulated with ±0.02 MHz noise; the fit recovers the
    // spectrum to well within a few tenths of a MHz
    for (a, b) in p.catalog.lines.iter().zip(&reference.lines) {
        assert_eq!((&a.upper, &a.lower), (&b.upper, &b.lower));
        assert!((a.freq - b.freq).abs() < 0.5, "{} vs {}", a.freq, b.freq);
    }
}

#[test]
fn errors_carry_the_program_message() {
    let int = read("spcat_prolate/input/prolate.int");
    match predict_text(Flavor::Spinv, "title only\n", &int) {
        Err(Error::Run { log, .. }) => assert!(log.contains("Error reading .VAR file"), "log: {log}"),
        other => panic!("expected Error::Run, got {other:?}"),
    }
    // and the library still works afterwards
    predict_text(Flavor::Spinv, &read("spcat_prolate/input/prolate.var"), &int).unwrap();
}

#[test]
fn predict_parallel_matches_sequential_and_keeps_order() {
    // a small scan of rotational constant A around its value
    let base = ParFile::parse(&read("spcat_asym/input/asym.var"), Flavor::Spinv).unwrap();
    let int = IntFile::parse(&read("spcat_asym/input/asym.int")).unwrap();
    let jobs: Vec<(ParFile, IntFile)> = (0..12)
        .map(|k| {
            let mut v = base.clone();
            v.params[0].value += 10.0 * (k as f64 - 6.0); // A in MHz
            (v, int.clone())
        })
        .collect();
    let parallel = predict_parallel(Flavor::Spinv, &jobs, Some(4));
    assert_eq!(parallel.len(), jobs.len());
    let mut first_lines = Vec::new();
    for ((var, int), p) in jobs.iter().zip(&parallel) {
        let p = p.as_ref().unwrap();
        let s = predict(Flavor::Spinv, var, int).unwrap();
        assert!(p.lines == s.lines && p.partition == s.partition, "parallel result differs");
        first_lines.push(p.lines[0].freq);
    }
    // results are in job order: changing A moves the lines, so neighbours differ
    assert!(first_lines.windows(2).all(|w| w[0] != w[1]));
    // all cores by default, and an empty batch
    assert_eq!(predict_parallel(Flavor::Spinv, &jobs[..2], None).len(), 2);
    assert!(predict_parallel(Flavor::Spinv, &[], None).is_empty());
}
