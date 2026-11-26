//! Prints a short summary of an SPCAT catalog: line count, frequency range
//! and the strongest lines.
//! Usage: cargo run --example cat_summary -- <file.cat> [count]
use pickett_io::CatFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: cat_summary <file.cat> [count]")?;
    let count: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(5);

    let cat = CatFile::parse(&std::fs::read_to_string(&path)?)?;
    let (lo, hi) = cat.lines.iter().fold((f64::MAX, f64::MIN), |(lo, hi), l| (lo.min(l.freq), hi.max(l.freq)));
    println!("{} lines, {:.4} - {:.4} MHz", cat.lines.len(), lo, hi);

    let mut strongest: Vec<_> = cat.lines.iter().collect();
    strongest.sort_by(|a, b| b.lgint.total_cmp(&a.lgint));
    println!("{:>14} {:>8} {:>10}  upper <- lower", "freq (MHz)", "log I", "E_low");
    for l in strongest.iter().take(count) {
        println!("{:>14.4} {:>8.4} {:>10.4}  {:?} <- {:?}", l.freq, l.lgint, l.elo, l.upper, l.lower);
    }
    Ok(())
}
