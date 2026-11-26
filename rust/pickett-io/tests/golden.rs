//! Reads every file under `test/golden/` (inputs and SPFIT/SPCAT outputs),
//! writes it back, and checks that:
//!
//! 1. reading the written text gives the same data (all files), and
//! 2. the written text is byte-identical to the original (files written by
//!    the C programs, plus hand-written inputs that already use their format).

use std::fs;
use std::path::{Path, PathBuf};

use pickett_io::{CatFile, EgyFile, Flavor, IntFile, LinFile, ParFile};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/golden")
}

/// All (case name, file path) pairs in `input/` and `expected/`.
fn golden_files() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut cases: Vec<_> = fs::read_dir(golden_dir())
        .expect("test/golden exists")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("case.json").exists())
        .collect();
    cases.sort_by_key(|e| e.file_name());
    for case in cases {
        let name = case.file_name().to_string_lossy().into_owned();
        for sub in ["input", "expected"] {
            let Ok(entries) = fs::read_dir(case.path().join(sub)) else { continue };
            let mut files: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            files.sort();
            out.extend(files.into_iter().map(|p| (name.clone(), p)));
        }
    }
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

enum Outcome {
    /// Written text equals the original byte for byte.
    Identical,
    /// Data round-trips, but the text is formatted differently.
    SameData,
}

/// Parses, writes and re-parses one file. Returns `None` for file types this
/// crate does not handle (.fit, .out, .str).
fn round_trip(case: &str, path: &Path) -> Option<Result<Outcome, String>> {
    let ext = path.extension()?.to_str()?;
    let text = read(path);
    macro_rules! check {
        ($parse:expr) => {{
            let parse = $parse;
            let a = match parse(&text) {
                Ok(a) => a,
                Err(e) => return Some(Err(format!("parse failed: {e}"))),
            };
            let written = a.to_text();
            let b = match parse(&written) {
                Ok(b) => b,
                Err(e) => return Some(Err(format!("re-parse failed: {e}\n{written}"))),
            };
            if a != b {
                return Some(Err(format!("data changed in round trip:\n{a:#?}\n---\n{b:#?}")));
            }
            Some(Ok(if written == text { Outcome::Identical } else { Outcome::SameData }))
        }};
    }
    let flavor = if case.starts_with("dp") { Flavor::Dpi } else { Flavor::Spinv };
    match ext {
        "par" | "var" | "bak" => check!(|t: &str| ParFile::parse(t, flavor)),
        "lin" => check!(LinFile::parse),
        "cat" | "mrg" => check!(CatFile::parse),
        "egy" => check!(EgyFile::parse),
        "int" => check!(IntFile::parse),
        _ => None,
    }
}

/// The only files whose text may change when written back (their data must
/// still round-trip). Every other golden file must come back byte-identical.
fn reformatted_ok(case: &str, path: &Path) -> bool {
    let name = path.file_name().unwrap().to_string_lossy();
    let ext = path.extension().unwrap().to_string_lossy();
    // No C program writes .int files, so there is no format to match.
    ext == "int"
        // Hand-typed `8.799700000000000E+003`; the nearest double prints as
        // `8.799700000000001E+003` when rounded correctly (as glibc does).
        || (case == "spcat_asym" && name == "asym.var")
}

#[test]
fn every_golden_file_round_trips() {
    let files = golden_files();
    assert!(!files.is_empty(), "no golden files found in {}", golden_dir().display());
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, path) in &files {
        let Some(result) = round_trip(case, path) else { continue };
        checked += 1;
        let rel = format!("{case}/{}", path.strip_prefix(golden_dir().join(case)).unwrap().display());
        match result {
            Ok(Outcome::Identical) => {}
            Ok(Outcome::SameData) if reformatted_ok(case, path) => {}
            Ok(Outcome::SameData) => failures.push(format!("{rel}: data OK but text not byte-identical")),
            Err(e) => failures.push(format!("{rel}: {e}")),
        }
    }
    assert!(checked >= 20, "only {checked} files checked");
    assert!(failures.is_empty(), "{} of {checked} files failed:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn spfit_var_has_full_covariance() {
    let text = read(&golden_dir().join("spfit_asym/expected/asym.var"));
    let var = ParFile::parse(&text, Flavor::Spinv).unwrap();
    assert_eq!(var.params.len(), 8);
    let cov = var.covariance().expect("8x8 packed matrix");
    assert_eq!(cov.len(), 36);
    assert_eq!(cov[0], 0.3114888);
    assert_eq!(cov[35], -1.0);
}

#[test]
fn dpfit_var_has_full_covariance() {
    let text = read(&golden_dir().join("dpfit_pi2/expected/pi2.var"));
    let var = ParFile::parse(&text, Flavor::Dpi).unwrap();
    let n = var.n_independent();
    assert_eq!(var.covariance().map(|c| c.len()), Some(n * (n + 1) / 2));
}

#[test]
fn prolate_catalog_matches_known_values() {
    let cat = CatFile::parse(&read(&golden_dir().join("spcat_prolate/expected/prolate.cat"))).unwrap();
    assert_eq!(cat.lines.len(), 2314);
    let first = &cat.lines[0];
    assert_eq!((first.freq, first.lgint, first.gup, first.tag, first.qnfmt), (18397.783, -5.3539, 3, 41505, 202));
    assert_eq!((first.upper.clone(), first.lower.clone()), (vec![1, 0], vec![0, 0]));
    assert!(cat.lines.windows(2).all(|w| w[0].freq <= w[1].freq), "sorted by frequency");
}

#[test]
fn lin_quanta_match_catalog_for_calbak_case() {
    let lin = LinFile::parse(&read(&golden_dir().join("calbak/expected/asym.lin"))).unwrap();
    let mrg = CatFile::parse(&read(&golden_dir().join("calbak/input/asym.mrg"))).unwrap();
    assert_eq!(lin.lines.len(), mrg.lines.len());
    for (l, c) in lin.lines.iter().zip(&mrg.lines) {
        assert_eq!(l.upper(3), c.upper);
        assert_eq!(l.lower(3), c.lower);
        assert!((l.freq - c.freq).abs() < 1e-9);
    }
}
