//! `run_memory`: golden cases without files, and full-precision records
//! that agree with the text files written in the same run.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use pickett_io::{CatFile, EgyFile, Flavor, ParFile};
use pickett_sys::{run_memory, MemoryRun, Program, RunError};

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/golden")
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).replace("\r\n", "\n")
}

/// (program, base, flavor) for the SPCAT/SPFIT/DPCAT/DPFIT golden cases.
fn cases() -> Vec<(PathBuf, Program, String, Flavor)> {
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(golden()).unwrap().filter_map(|e| e.ok().map(|e| e.path())).collect();
    dirs.sort();
    for d in dirs {
        let name = d.file_name().unwrap().to_string_lossy().into_owned();
        let (program, flavor, key) = match name.split('_').next().unwrap() {
            "spcat" => (Program::Spcat, Flavor::Spinv, "int"),
            "spfit" => (Program::Spfit, Flavor::Spinv, "par"),
            "dpcat" => (Program::Dpcat, Flavor::Dpi, "int"),
            "dpfit" => (Program::Dpfit, Flavor::Dpi, "par"),
            _ => continue,
        };
        let base = fs::read_dir(d.join("input"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|x| x == key))
            .unwrap()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        out.push((d, program, base, flavor));
    }
    out
}

fn inputs(case: &Path) -> Vec<(String, Vec<u8>)> {
    fs::read_dir(case.join("input"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()))
        .collect()
}

fn run_case(case: &Path, program: Program, base: &str) -> MemoryRun {
    let owned = inputs(case);
    let refs: Vec<(&str, &[u8])> = owned.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    run_memory(program, base, &refs)
}

fn close(a: f64, b: f64, rtol: f64, atol: f64) -> bool {
    (a - b).abs() <= rtol * a.abs().max(b.abs()) + atol
}

#[test]
fn golden_cases_in_memory_match_expected_files() {
    let mut problems = Vec::new();
    for (case, program, base, flavor) in cases() {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let run = run_case(&case, program, &base);
        if let Err(e) = &run.status {
            problems.push(format!("{name}: {e}\n{}", run.log));
            continue;
        }
        for e in fs::read_dir(case.join("expected")).unwrap() {
            let exp = e.unwrap().path();
            let fname = exp.file_name().unwrap().to_string_lossy().into_owned();
            let Some(act) = run.files.get(&fname) else {
                problems.push(format!("{name}/{fname}: not produced"));
                continue;
            };
            let ext = exp.extension().unwrap().to_string_lossy().into_owned();
            match ext.as_str() {
                "cat" | "egy" => {
                    if text(&fs::read(&exp).unwrap()) != text(act) {
                        problems.push(format!("{name}/{fname}: differs"));
                    }
                }
                "par" | "var" | "bak" => {
                    let e = ParFile::parse(&text(&fs::read(&exp).unwrap()), flavor).unwrap();
                    let a = ParFile::parse(&text(act), flavor).unwrap();
                    let same = e.params.len() == a.params.len()
                        && e.params.iter().zip(&a.params).all(|(x, y)| {
                            x.id == y.id && close(x.value, y.value, 1e-6, 1e-12)
                        });
                    if !same {
                        problems.push(format!("{name}/{fname}: parameters differ"));
                    }
                }
                _ => {}
            }
        }
        assert!(!run.log.is_empty(), "{name}: messages were not captured");
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_named_files_are_written() {
    let (case, program, base, _) = cases().into_iter().find(|c| c.1 == Program::Spcat).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let before: Vec<_> = fs::read_dir(&cwd).unwrap().filter_map(|e| e.ok().map(|e| e.file_name())).collect();
    let run = run_case(&case, program, &base);
    run.status.unwrap();
    let after: Vec<_> = fs::read_dir(&cwd).unwrap().filter_map(|e| e.ok().map(|e| e.file_name())).collect();
    assert_eq!(before.len(), after.len(), "files appeared in {}", cwd.display());
    assert!(run.files.contains_key(&format!("{base}.cat")));
    assert!(run.files.contains_key(&format!("{base}.out")));
}

#[test]
fn catalog_records_match_cat_file() {
    for (case, program, base, _) in cases().into_iter().filter(|c| matches!(c.1, Program::Spcat | Program::Dpcat)) {
        let run = run_case(&case, program, &base);
        run.status.as_ref().unwrap();
        let cat = CatFile::parse(&text(&run.files[&format!("{base}.cat")])).unwrap();
        let recs = &run.records.cat_lines;
        assert_eq!(recs.len(), cat.lines.len(), "{base}: line count");

        // The .cat file is sorted and rounded to 4 decimals: match each file
        // line to a record by its rounded text fields.
        let key = |f: f64, e: f64, u: &[i32], l: &[i32]| format!("{f:.4} {e:.4} {u:?} {l:?}");
        let mut pool: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, r) in recs.iter().enumerate() {
            pool.entry(key(r.freq, r.err, &r.upper, &r.lower)).or_default().push(i);
        }
        for l in &cat.lines {
            let k = key(l.freq, l.err, &l.upper, &l.lower);
            let i = pool.get_mut(&k).and_then(|v| v.pop()).unwrap_or_else(|| panic!("{base}: no record for {k}"));
            let r = &recs[i];
            assert!((r.lgint - l.lgint).abs() <= 0.5e-4 + 1e-9, "{base} {k}: lgint {} vs {}", r.lgint, l.lgint);
            assert!((r.elow - l.elo).abs() <= 0.5e-4 + 1e-9, "{base} {k}: elow");
            assert_eq!((r.gup as u32, r.tag, r.qnfmt, r.dr), (l.gup, l.tag, l.qnfmt, l.dr), "{base} {k}");
        }
        // records carry more digits than the file
        assert!(recs.iter().any(|r| (r.freq * 1e4).fract().abs() > 1e-6), "{base}: no extra precision");
    }
}

#[test]
fn level_and_partition_records_match_files() {
    let (case, program, base, _) = cases().into_iter().find(|c| c.2 == "prolate" && c.1 == Program::Spcat).unwrap();
    let run = run_case(&case, program, &base);
    run.status.as_ref().unwrap();

    let egy = EgyFile::parse(&text(&run.files[&format!("{base}.egy")])).unwrap();
    let levels: Vec<_> = egy.levels().collect();
    assert_eq!(levels.len(), run.records.levels.len());
    for (f, r) in levels.iter().zip(&run.records.levels) {
        assert_eq!((f.block, f.index, f.degeneracy), (r.block as i64, r.index as i64, r.degeneracy as i64));
        assert!((f.energy - r.energy).abs() <= 0.5e-6 + 1e-12);
        assert_eq!(f.qn, r.qn);
    }

    // partition table at the end of .out: "   1000.000     47744.6549    4.6789"
    let out = text(&run.files[&format!("{base}.out")]);
    let table: Vec<(f64, f64)> = out
        .lines()
        .skip_while(|l| !l.starts_with("TEMPERATURE"))
        .skip(1)
        .map_while(|l| {
            let v: Vec<f64> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| (v[0], v[1]))
        })
        .collect();
    assert_eq!(table.len(), 1000);
    assert_eq!(table.len(), run.records.partition.len());
    for ((t, q), (rt, rq)) in table.iter().zip(&run.records.partition) {
        assert!((t - rt).abs() < 1e-9 && (q - rq).abs() <= 0.5e-4 + 1e-9, "T={t}: {q} vs {rq}");
    }
    assert_eq!(run.records.partition[700].0, 300.0);
}

#[test]
fn fit_records_match_var_file() {
    for (case, program, base, flavor) in cases().into_iter().filter(|c| matches!(c.1, Program::Spfit | Program::Dpfit)) {
        let run = run_case(&case, program, &base);
        run.status.as_ref().unwrap();
        let var = ParFile::parse(&text(&run.files[&format!("{base}.var")]), flavor).unwrap();
        let recs = &run.records.fit_params;
        assert_eq!(recs.len(), var.params.len(), "{base}");
        for (r, p) in recs.iter().zip(&var.params) {
            assert_eq!(r.id, p.id.to_string());
            assert_eq!(r.label, p.label);
            // .var has 16 significant digits
            assert!(close(r.value, p.value, 2e-15, 1e-300), "{base} {}: {} vs {}", r.id, r.value, p.value);
            assert!(close(r.err_fit, p.error.unwrap(), 1e-8, 1e-300), "{base} {} error", r.id);
        }
        let its = &run.records.fit_iterations;
        assert!(!its.is_empty(), "{base}: no iterations");
        assert!(its.windows(2).all(|w| w[1].iteration == w[0].iteration + 1));
        let fit = text(&run.files[&format!("{base}.fit")]);
        let printed = fit.lines().filter(|l| l.contains("END OF ITERATION")).count();
        assert_eq!(printed, its.len(), "{base}: iterations in .fit");
    }
}

#[test]
fn failure_message_is_in_the_log() {
    let run = run_memory(Program::Spcat, "nothing", &[]);
    assert!(matches!(run.status, Err(RunError::Failed { .. })), "{:?}", run.status);
    assert!(run.log.contains("Trouble opening"), "log: {}", run.log);
}

#[test]
fn bad_base_name_is_rejected() {
    for base in ["", "a.b", "dir/x"] {
        let run = run_memory(Program::Spcat, base, &[]);
        assert!(matches!(run.status, Err(RunError::BadArgument(_))), "{base:?}");
    }
}
