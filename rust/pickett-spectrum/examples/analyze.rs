//! One assignment pass on a measured spectrum:
//! baseline → peaks → line fits → SPCAT prediction from `<mol.par>` →
//! matching → `.lin`, and optionally an SPFIT fit of that `.lin`.
//!
//! Usage:
//!   cargo run --release -p pickett-spectrum --example analyze -- <spectrum.txt> <mol.par> <mol.int> <out.lin>
//!       [--tol MHz] [--min-lgint x] [--snr x] [--err-floor MHz] [--lambda x] [--fit] [--dpi]
//!
//! Repeat with the new `.par` (written next to `out.lin` with `--fit`) and a
//! smaller `--tol` as the constants improve.
use pickett::{fit, predict, Flavor, IntFile, ParFile};
use pickett_spectrum::{
    assign::{match_lines, to_lin, MatchOptions},
    baseline::{asls, AslsOptions},
    fit::{fit_peak, FitOptions},
    peaks::{find_peaks, noise_level, PeakOptions},
    Spectrum,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pos: Vec<&String> = args.iter().filter(|s| !s.starts_with("--")).collect();
    let opt = |name: &str, default: f64| -> Result<f64, Box<dyn std::error::Error>> {
        match args.iter().position(|s| s == name) {
            Some(i) => Ok(args.get(i + 1).ok_or(format!("{name} needs a value"))?.parse()?),
            None => Ok(default),
        }
    };
    let flag = |name: &str| args.iter().any(|s| s == name);
    // values of options are not positional arguments
    let values: Vec<&String> = ["--tol", "--min-lgint", "--snr", "--err-floor", "--lambda"]
        .iter()
        .filter_map(|n| args.iter().position(|s| s == n).and_then(|i| args.get(i + 1)))
        .collect();
    let pos: Vec<&String> = pos.into_iter().filter(|p| !values.iter().any(|v| std::ptr::eq(*v, *p))).collect();
    if pos.len() < 4 {
        return Err("usage: analyze <spectrum.txt> <mol.par> <mol.int> <out.lin> [--tol MHz] [--min-lgint x] [--snr x] [--err-floor MHz] [--lambda x] [--fit] [--dpi]".into());
    }
    let flavor = if flag("--dpi") { Flavor::Dpi } else { Flavor::Spinv };
    let (tol, min_lgint, snr, err_floor, lambda) =
        (opt("--tol", 1.0)?, opt("--min-lgint", f64::NEG_INFINITY)?, opt("--snr", 6.0)?, opt("--err-floor", 0.0)?, opt("--lambda", 1e8)?);

    let spectrum = Spectrum::parse(&std::fs::read_to_string(pos[0])?)?;
    let par = ParFile::parse(&std::fs::read_to_string(pos[1])?, flavor)?;
    let int = IntFile::parse(&std::fs::read_to_string(pos[2])?)?;

    let baseline = asls(&spectrum.intensity, &AslsOptions { lambda, p: 0.002, iterations: 15 });
    let flat = spectrum.minus(&baseline);
    let noise = noise_level(&flat.intensity);
    let peaks = find_peaks(&flat, &PeakOptions { min_snr: snr, min_distance: 3, noise: Some(noise), ..Default::default() });
    let fits: Vec<_> = peaks.iter().filter_map(|p| fit_peak(&flat, p, &FitOptions::default())).filter(|f| f.converged).collect();
    println!("{} points, noise {noise:.3e}, {} peaks, {} line fits", spectrum.len(), peaks.len(), fits.len());

    let prediction = predict(flavor, &par, &int)?;
    let centres: Vec<f64> = fits.iter().map(|f| f.center).collect();
    let all = match_lines(&centres, &prediction.catalog.lines, &MatchOptions { tolerance: tol, min_lgint });
    let (good, ambiguous): (Vec<_>, Vec<_>) = all.into_iter().partition(|a| !a.ambiguous);
    let errs: Vec<f64> = good.iter().map(|a| fits[a.observed].center_err.max(err_floor)).collect();
    let lin = to_lin(&good, &prediction.catalog.lines, 0.0, Some(&errs));
    std::fs::write(pos[3], lin.to_text())?;
    println!("{} assigned within {tol} MHz ({} ambiguous left out), written to {}", good.len(), ambiguous.len(), pos[3]);
    println!("{:>16} {:>12} {:>10}  upper <- lower", "measured", "obs-calc", "error");
    for (a, e) in good.iter().zip(&errs).take(10) {
        let p = &prediction.catalog.lines[a.predicted];
        println!("{:>16.4} {:>12.4} {:>10.4}  {:?} <- {:?}", a.obs, a.residual, e, p.upper, p.lower);
    }
    if good.len() > 10 {
        println!("{:>16}", "...");
    }

    if flag("--fit") {
        let result = fit(flavor, &par, &lin)?;
        let out_par = std::path::Path::new(pos[3]).with_extension("par");
        std::fs::write(&out_par, result.par.to_text())?;
        println!("SPFIT: weighted RMS {:.4}, new parameters in {}", result.rms_error().unwrap_or(f64::NAN), out_par.display());
        for p in &result.params {
            println!("  {:>8} {:>22.10} ± {:.3e}  {}", p.id, p.value, p.err_fit, p.label);
        }
    }
    Ok(())
}
