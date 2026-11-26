//! Reads a file and writes it back to stdout in the C programs' format.
//! Usage: cargo run --example roundtrip -- <file> [dpi]
use pickett_io::{CatFile, EgyFile, Flavor, IntFile, LinFile, ParFile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: roundtrip <file> [dpi]")?;
    let flavor = if args.next().as_deref() == Some("dpi") { Flavor::Dpi } else { Flavor::Spinv };
    let text = std::fs::read_to_string(&path)?;
    let out = match path.rsplit('.').next().unwrap_or("") {
        "par" | "var" | "bak" => ParFile::parse(&text, flavor)?.to_text(),
        "lin" => LinFile::parse(&text)?.to_text(),
        "cat" | "mrg" => CatFile::parse(&text)?.to_text(),
        "egy" => EgyFile::parse(&text)?.to_text(),
        "int" => IntFile::parse(&text)?.to_text(),
        other => return Err(format!("unknown extension .{other}").into()),
    };
    print!("{out}");
    Ok(())
}
