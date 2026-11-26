//! Closed loop: simulate a measured spectrum from known constants, then
//! recover the constants with the analysis tools and SPFIT, starting from
//! wrong constants.
//!
//! truth constants --SPCAT--> lines --synth + baseline + noise--> spectrum
//! spectrum --AsLS--> peaks --line fits--> measured centres
//! perturbed constants --SPCAT--> prediction --match--> .lin --SPFIT--> ...

use std::path::Path;

use pickett::{fit, predict, predict_text, Flavor, IntFile, ParFile};
use pickett_spectrum::{
    assign::{match_lines, to_lin, MatchOptions},
    baseline::{asls, AslsOptions},
    fit::{fit_peak, FitOptions, Shape},
    peaks::{find_peaks, noise_level, PeakOptions},
    profile::Profile,
    synth, Spectrum,
};

fn read(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/golden").join(rel);
    std::fs::read_to_string(p).unwrap().replace("\r\n", "\n")
}

/// Deterministic Gaussian noise (xorshift + Box–Muller).
struct Noise(u64);
impl Noise {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> f64 {
        let (a, b) = (self.uniform().max(1e-300), self.uniform());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

#[test]
fn recover_constants_from_a_simulated_spectrum() {
    let (fmin, fmax, step, fwhm) = (50_000.0, 350_000.0, 0.2, 1.0);
    let int_text = read("spcat_asym/input/asym.int");
    let int = IntFile::parse(&int_text).unwrap();

    // 1. the "true" spectrum
    let truth_var = ParFile::parse(&read("spcat_asym/input/asym.var"), Flavor::Spinv).unwrap();
    let truth = predict_text(Flavor::Spinv, &read("spcat_asym/input/asym.var"), &int_text).unwrap();
    let profile = Profile::Gaussian { fwhm };
    let grid = synth::grid(fmin, fmax, step);
    let lines: Vec<(f64, f64)> = truth
        .lines
        .iter()
        .filter(|l| l.freq > fmin && l.freq < fmax)
        .map(|l| (l.freq, 10f64.powf(l.lgint) / profile.area()))
        .collect();
    let mut y = synth::synthesize(lines.iter().copied(), &grid, profile, 30.0);
    let noise_sd = 2e-7;
    let mut rng = Noise(0x9E37_79B9_7F4A_7C15);
    for (i, v) in y.iter_mut().enumerate() {
        let f = grid[i];
        let base = 4e-6 + 1e-11 * (f - fmin) + 2e-6 * (f / 9000.0).sin();
        *v += base + noise_sd * rng.gauss();
    }
    let measured = Spectrum::new(grid, y).unwrap();

    // 2. baseline, peaks, line fits
    let base = asls(&measured.intensity, &AslsOptions { lambda: 1e8, p: 0.002, iterations: 15 });
    let flat = measured.minus(&base);
    let noise = noise_level(&flat.intensity);
    assert!((noise - noise_sd).abs() < 0.2 * noise_sd, "noise estimate {noise}");
    let peaks = find_peaks(&flat, &PeakOptions { min_snr: 8.0, min_distance: 3, ..Default::default() });
    let fits: Vec<_> = peaks
        .iter()
        .filter_map(|p| fit_peak(&flat, p, &FitOptions { shape: Shape::Gaussian, ..Default::default() }))
        .filter(|f| f.converged && f.center_err.is_finite() && f.center_err > 0.0)
        .collect();
    let centres: Vec<f64> = fits.iter().map(|f| f.center).collect();
    let errors: Vec<f64> = fits.iter().map(|f| f.center_err.max(1e-4)).collect();
    let visible = lines.iter().filter(|(_, h)| h / noise_sd > 8.0).count();
    assert!(fits.len() as f64 >= 0.9 * visible as f64, "{} fitted of {visible} visible lines", fits.len());

    // measured centres agree with the true frequencies within their errors
    let true_freqs: Vec<f64> = lines.iter().map(|l| l.0).collect();
    let mut pulls = Vec::new();
    for f in &fits {
        let nearest = true_freqs.iter().copied().min_by(|a, b| (a - f.center).abs().total_cmp(&(b - f.center).abs())).unwrap();
        pulls.push((f.center - nearest) / f.center_err);
    }
    let rms_pull = (pulls.iter().map(|p| p * p).sum::<f64>() / pulls.len() as f64).sqrt();
    assert!(rms_pull < 2.0, "centre errors are not consistent: rms pull {rms_pull}");

    // 3. assign and fit, starting from perturbed constants
    let mut par = ParFile::parse(&read("spfit_asym/input/asym.par"), Flavor::Spinv).unwrap();
    let start_a = par.params[0].value;
    let mut last = None;
    // Early passes give the lines errors that match the tolerance, so SPFIT
    // does not reject them while the constants are still far off (|o-c|/err
    // must stay below ERRTST = 1000); the last pass uses the measured errors.
    for (tolerance, min_lgint, err_floor) in [(60.0, -4.0, 3.0), (10.0, -5.5, 0.5), (2.0, -7.0, 0.05), (0.5, -7.0, 0.0)] {
        let prediction = predict(Flavor::Spinv, &par, &int).unwrap();
        let m: Vec<_> = match_lines(&centres, &prediction.catalog.lines, &MatchOptions { tolerance, min_lgint })
            .into_iter()
            .filter(|a| !a.ambiguous)
            .collect();
        assert!(m.len() >= 8, "only {} assignments at tolerance {tolerance}", m.len());
        let errs: Vec<f64> = m.iter().map(|a| errors[a.observed].max(err_floor)).collect();
        let lin = to_lin(&m, &prediction.catalog.lines, 0.0, Some(&errs));
        let result = fit(Flavor::Spinv, &par, &lin).unwrap();
        let wrong = m.iter().filter(|a| {
            let p = &prediction.catalog.lines[a.predicted];
            let t = truth.lines.iter().min_by(|x, y| (x.freq - a.obs).abs().total_cmp(&(y.freq - a.obs).abs())).unwrap();
            (&p.upper, &p.lower) != (&t.upper, &t.lower)
        }).count();
        eprintln!("tol {tolerance:5}: {} assigned ({wrong} wrong), A = {:.4}, RMS {:.3}, max |o-c| {:.3}",
            m.len(), result.params[0].value, result.rms_error().unwrap(),
            m.iter().map(|a| a.residual.abs()).fold(0.0, f64::max));
        par = result.par.clone();
        last = Some((result, m, prediction));
    }
    let (result, assignments, prediction) = last.unwrap();

    // 4. every assignment has the quanta of the true line at that frequency
    for a in &assignments {
        let p = &prediction.catalog.lines[a.predicted];
        let t = truth
            .lines
            .iter()
            .min_by(|x, y| (x.freq - a.obs).abs().total_cmp(&(y.freq - a.obs).abs()))
            .unwrap();
        assert_eq!((&p.upper, &p.lower), (&t.upper, &t.lower), "wrong assignment at {}", a.obs);
    }
    assert!(assignments.len() as f64 >= 0.9 * fits.len() as f64, "{} of {} assigned", assignments.len(), fits.len());

    // 5. the fitted constants agree with the truth within their errors
    for (p, t) in result.params.iter().zip(&truth_var.params) {
        let pull = (p.value - t.value) / p.err_fit;
        assert!(pull.abs() < 5.0, "{}: fitted {} ± {:.2e}, true {} (pull {pull:.1})", p.label, p.value, p.err_fit, t.value);
    }
    let a = &result.params[0];
    assert!((a.value - truth_var.params[0].value).abs() < 0.01 * (start_a - truth_var.params[0].value).abs(),
        "A moved from {start_a} to {}, truth {}", a.value, truth_var.params[0].value);
    eprintln!(
        "closed loop: {} peaks, {} fitted, {} assigned; A = {:.5} ± {:.5} (true {:.5}, start {start_a}); RMS {:.4}",
        peaks.len(), fits.len(), assignments.len(), a.value, a.err_fit, truth_var.params[0].value,
        result.rms_error().unwrap()
    );
}
