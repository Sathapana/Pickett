//! Fit, then predict with the fitted parameters, all in memory.
//! Usage: cargo run -p pickett --example fit_predict -- <mol.par> <mol.lin> <mol.int> [dpi]
use pickett::{fit, predict, Flavor, IntFile, LinFile, ParFile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        return Err("usage: fit_predict <mol.par> <mol.lin> <mol.int> [dpi]".into());
    }
    let flavor = if a.get(3).map(String::as_str) == Some("dpi") { Flavor::Dpi } else { Flavor::Spinv };
    let par = ParFile::parse(&std::fs::read_to_string(&a[0])?, flavor)?;
    let lin = LinFile::parse(&std::fs::read_to_string(&a[1])?)?;
    let int = IntFile::parse(&std::fs::read_to_string(&a[2])?)?;

    let result = fit(flavor, &par, &lin)?;
    println!("fit: {} iterations, weighted RMS {:.5}", result.iterations.len(), result.rms_error().unwrap_or(f64::NAN));
    for p in &result.params {
        println!("  {:>8} {:>24.12} ± {:.3e}  {}", p.id, p.value, p.err_fit, p.label);
    }

    let spectrum = predict(flavor, &result.var, &int)?;
    println!("prediction: {} lines, {} levels", spectrum.lines.len(), spectrum.levels.len());
    if let Some(q) = spectrum.partition_at(300.0) {
        println!("  Q(300 K) = {q:.4}");
    }
    let mut strongest: Vec<_> = spectrum.lines.iter().collect();
    strongest.sort_by(|x, y| y.lgint.total_cmp(&x.lgint));
    for l in strongest.iter().take(3) {
        println!("  {:>16.6} ± {:.6} MHz  log I {:.4}  {:?} <- {:?}", l.freq, l.err, l.lgint, l.upper, l.lower);
    }
    Ok(())
}
