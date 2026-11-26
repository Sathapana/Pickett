//! Times SPCAT runs three ways:
//!   1. one run at a time, each run on one thread (`set_threads(1)`);
//!   2. one run at a time, each run using all cores for its own work
//!      (the default);
//!   3. `repeat` runs spread over `threads` threads, each run on one thread.
//! Every result is checked against the first run.
//! Usage:
//!   cargo run --release -p pickett --example bench -- <mol.var> <mol.int> [repeat] [threads] [dpi]
use std::time::Instant;

use pickett::{predict_text, Flavor};
use pickett_sys::set_threads;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 2 {
        return Err("usage: bench <mol.var> <mol.int> [repeat] [threads] [dpi]".into());
    }
    let var = std::fs::read_to_string(&a[0])?;
    let int = std::fs::read_to_string(&a[1])?;
    let repeat: usize = a.get(2).map(|s| s.parse()).transpose()?.unwrap_or(8);
    let threads: usize = a.get(3).map(|s| s.parse()).transpose()?.unwrap_or(4);
    let flavor = if a.get(4).map(String::as_str) == Some("dpi") { Flavor::Dpi } else { Flavor::Spinv };

    let reference = predict_text(flavor, &var, &int)?; // warm-up and reference result
    let same = |p: &pickett::Prediction| p.lines == reference.lines && p.partition == reference.partition;
    let ms = |secs: f64, n: usize| 1e3 * secs / n as f64;

    let mut timed = |n_threads: usize| -> Result<(f64, bool), Box<dyn std::error::Error>> {
        set_threads(n_threads);
        let t = Instant::now();
        let mut ok = true;
        for _ in 0..repeat {
            ok &= same(&predict_text(flavor, &var, &int)?);
        }
        Ok((t.elapsed().as_secs_f64(), ok))
    };
    let (one, ok1) = timed(1)?;
    let (all, ok2) = timed(0)?;

    let per_thread = repeat.div_ceil(threads);
    let t = Instant::now();
    let ok3 = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    set_threads(1);
                    (0..per_thread).all(|_| predict_text(flavor, &var, &int).map(|p| same(&p)).unwrap_or(false))
                })
            })
            .collect();
        handles.into_iter().all(|h| h.join().unwrap())
    });
    let batch = t.elapsed().as_secs_f64();

    println!("{} lines, {} levels per run", reference.lines.len(), reference.levels.len());
    println!("one run, 1 thread:        {:7.1} ms/run", ms(one, repeat));
    println!("one run, all {} cores:     {:7.1} ms/run  ({:.2}x)", std::thread::available_parallelism()?.get(), ms(all, repeat), one / all);
    println!("{threads} runs at once (batch): {:7.1} ms/run  ({:.2}x)", ms(batch, per_thread * threads), ms(one, repeat) / ms(batch, per_thread * threads));
    println!("all results identical: {}", ok1 && ok2 && ok3);
    Ok(())
}
