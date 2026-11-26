//! Writes a simulated "measured" spectrum: the SPCAT prediction for a
//! `.var` + `.int`, drawn with a line shape, on a sloping and wavy baseline,
//! with Gaussian noise. Useful to try the `analyze` example.
//!
//! Usage:
//!   cargo run --release -p pickett-spectrum --example simulate -- <mol.var> <mol.int> <out.txt>
//!       [fmin fmax step fwhm noise] [dpi]
//! Defaults: 50000 350000 0.2 1.0 2e-7 (MHz, MHz, MHz, MHz, intensity units).
use pickett::{predict, Flavor, IntFile, ParFile};
use pickett_spectrum::{profile::Profile, synth, Spectrum};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        return Err("usage: simulate <mol.var> <mol.int> <out.txt> [fmin fmax step fwhm noise] [dpi]".into());
    }
    let flavor = if a.last().map(String::as_str) == Some("dpi") { Flavor::Dpi } else { Flavor::Spinv };
    let num = |i: usize, d: f64| -> Result<f64, Box<dyn std::error::Error>> {
        Ok(match a.get(i) {
            Some(s) if s != "dpi" => s.parse()?,
            _ => d,
        })
    };
    let (fmin, fmax, step, fwhm, noise) = (num(3, 50_000.0)?, num(4, 350_000.0)?, num(5, 0.2)?, num(6, 1.0)?, num(7, 2e-7)?);

    let var = ParFile::parse(&std::fs::read_to_string(&a[0])?, flavor)?;
    let int = IntFile::parse(&std::fs::read_to_string(&a[1])?)?;
    let prediction = predict(flavor, &var, &int)?;

    let profile = Profile::Gaussian { fwhm };
    let mut s: Spectrum = synth::from_catalog(&prediction.catalog.lines, fmin, fmax, step, profile, true);
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut uniform = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    for (f, y) in s.freq.iter().zip(s.intensity.iter_mut()) {
        let g = (-2.0 * uniform().max(1e-300).ln()).sqrt() * (std::f64::consts::TAU * uniform()).cos();
        *y += 4e-6 + 1e-11 * (f - fmin) + 2e-6 * (f / 9000.0).sin() + noise * g;
    }
    std::fs::write(&a[2], s.to_text())?;
    println!("{} points, {} predicted lines in range, written to {}", s.len(),
        prediction.catalog.lines.iter().filter(|l| l.freq >= fmin && l.freq <= fmax).count(), a[2]);
    Ok(())
}
