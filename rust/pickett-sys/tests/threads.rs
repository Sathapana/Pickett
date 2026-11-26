//! Runs on several threads at once: every result must be bit-identical to
//! the same run on one thread, whatever else runs at the same time.

use std::fs;
use std::path::{Path, PathBuf};

use pickett_sys::{run_memory, set_threads, thread_safe, threads, Program, Records};

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/golden")
}

struct Case {
    program: Program,
    base: String,
    inputs: Vec<(String, Vec<u8>)>,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for (dir, program, key) in [
        ("spcat_prolate", Program::Spcat, "int"),
        ("spcat_asym", Program::Spcat, "int"),
        ("dpcat_pi2", Program::Dpcat, "int"),
        ("spfit_asym", Program::Spfit, "par"),
        ("spfit_prolate", Program::Spfit, "par"),
        ("dpfit_pi2", Program::Dpfit, "par"),
    ] {
        let input = golden().join(dir).join("input");
        let mut inputs = Vec::new();
        let mut base = String::new();
        for e in fs::read_dir(&input).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == key) {
                base = p.file_stem().unwrap().to_string_lossy().into_owned();
            }
            inputs.push((p.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()));
        }
        out.push(Case { program, base, inputs });
    }
    out
}

/// The result of one run that must not depend on the thread: the records
/// and every output file except the time-stamped title lines.
fn run(c: &Case) -> (Records, Vec<(String, String)>) {
    let refs: Vec<(&str, &[u8])> = c.inputs.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    let r = run_memory(c.program, &c.base, &refs);
    r.status.as_ref().unwrap_or_else(|e| panic!("{} {}: {e}\n{}", c.program, c.base, r.log));
    let files = r
        .files
        .iter()
        .map(|(n, b)| {
            let t = String::from_utf8_lossy(b);
            // drop lines with a time stamp (title lines of .fit/.out/.par/.var)
            let kept: Vec<&str> = t.lines().filter(|l| !has_clock(l)).collect();
            (n.clone(), kept.join("\n"))
        })
        .collect();
    (r.records, files)
}

/// True for lines with an `hh:mm:ss` time, as in SPFIT/SPCAT title lines.
fn has_clock(line: &str) -> bool {
    line.as_bytes().windows(8).any(|w| {
        let d = |i: usize| w[i].is_ascii_digit();
        d(0) && d(1) && w[2] == b':' && d(3) && d(4) && w[5] == b':' && d(6) && d(7)
    })
}

#[test]
fn library_reports_thread_safe() {
    // PICKETT_NO_TLS at build time turns thread-local state off (runs are
    // then serialized with a lock, and the other tests still pass)
    let expected = option_env!("PICKETT_NO_TLS").is_none();
    assert_eq!(thread_safe(), expected);
}

#[test]
fn parallel_runs_match_sequential_runs_exactly() {
    let cases = cases();
    let reference: Vec<_> = cases.iter().map(run).collect();
    const THREADS: usize = 8;
    std::thread::scope(|s| {
        for t in 0..THREADS {
            let (cases, reference) = (&cases, &reference);
            s.spawn(move || {
                // each thread runs every case twice, starting at a different case
                for k in 0..2 * cases.len() {
                    let i = (t + k) % cases.len();
                    let got = run(&cases[i]);
                    assert!(got.0 == reference[i].0, "thread {t}: records of {} {} differ", cases[i].program, cases[i].base);
                    assert!(got.1 == reference[i].1, "thread {t}: files of {} {} differ", cases[i].program, cases[i].base);
                }
            });
        }
    });
}

#[test]
fn a_failing_thread_does_not_disturb_others() {
    let cases = cases();
    let good = &cases[1]; // spcat_asym
    let reference = run(good);
    std::thread::scope(|s| {
        let failing = s.spawn(|| {
            (0..20).filter(|_| run_memory(Program::Spfit, "missing", &[]).status.is_err()).count()
        });
        let workers: Vec<_> = (0..4).map(|_| s.spawn(|| (0..5).all(|_| run(good) == reference))).collect();
        assert_eq!(failing.join().unwrap(), 20);
        for w in workers {
            assert!(w.join().unwrap());
        }
    });
}

#[test]
fn partition_table_does_not_depend_on_thread_count() {
    let prolate = &cases()[0];
    let mut tables = Vec::new();
    for n in [1, 2, 3, 8, 0] {
        set_threads(n);
        assert_eq!(threads(), if n == 0 { std::thread::available_parallelism().unwrap().get() } else { n });
        let (records, files) = run(prolate);
        assert_eq!(records.partition.len(), 1000);
        tables.push((n, records.partition, files));
    }
    set_threads(0);
    for (n, t, f) in &tables[1..] {
        assert!(*t == tables[0].1, "Q table with {n} threads differs from 1 thread");
        assert!(*f == tables[0].2, "files with {n} threads differ from 1 thread");
    }
}
