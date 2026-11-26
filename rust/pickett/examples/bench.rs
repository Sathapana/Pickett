//! Times SPCAT runs: `repeat` runs one after another, then the same number
//! spread over `threads` threads (in parallel when the C library is
//! thread-safe). Usage:
//!   cargo run --release -p pickett --example bench -- <mol.var> <mol.int> [repeat] [threads] [dpi]
use std::time::Instant;

use pickett::{predict_text, Flavor};

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
    let t0 = Instant::now();
    for _ in 0..repeat {
        predict_text(flavor, &var, &int)?;
    }
    let seq = t0.elapsed().as_secs_f64();

    let t1 = Instant::now();
    let per_thread = repeat.div_ceil(threads);
    let same = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| s.spawn(|| (0..per_thread).all(|_| predict_text(flavor, &var, &int).map(|p| p.lines == reference.lines).unwrap_or(false))))
            .collect();
        handles.into_iter().all(|h| h.join().unwrap())
    });
    let par = t1.elapsed().as_secs_f64();

    println!("{} lines per run, thread-safe C library: {}", reference.lines.len(), pickett_sys::thread_safe());
    println!("sequential: {repeat} runs in {seq:.3} s ({:.1} ms/run)", 1e3 * seq / repeat as f64);
    println!(
        "{threads} threads:  {} runs in {par:.3} s ({:.1} ms/run), speed-up {:.2}x, results identical: {same}",
        per_thread * threads,
        1e3 * par / (per_thread * threads) as f64,
        (seq / repeat as f64) / (par / (per_thread * threads) as f64)
    );
    Ok(())
}
