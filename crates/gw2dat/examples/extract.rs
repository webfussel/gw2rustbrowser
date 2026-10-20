//! Writes one file by base or file id, raw or converted:
//!
//!     cargo run --release -p gw2dat --example extract -- <Gw2.dat> <id> [--raw] [out]
use gw2dat::{Archive, Index, ScanProgress, formats};

fn main() -> gw2dat::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raw = args.iter().any(|a| a == "--raw");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [path, id, rest @ ..] = positional.as_slice() else {
        eprintln!("usage: extract <Gw2.dat> <id> [--raw] [out]");
        std::process::exit(2);
    };
    let archive = Archive::open(path)?;
    let id: u32 = id.parse().expect("numeric id");
    let mft = archive.entry_for_id(id).ok_or(gw2dat::Error::NoSuchId(id))?;
    let index = Index::scan(&archive, &ScanProgress::default());
    let entry = index.entries.iter().find(|e| e.mft_index == mft).expect("indexed entry");
    let data = archive.read(mft)?;
    let (bytes, extension) = if raw {
        (data, entry.file_type.extension())
    } else {
        let converted = formats::convert(entry.file_type, data)?;
        (converted.data, converted.extension)
    };
    let out = rest.first().map(|s| s.to_string()).unwrap_or_else(|| format!("{id}.{extension}"));
    std::fs::write(&out, &bytes)?;
    println!("{} ({}) -> {out}, {} bytes", id, entry.file_type.name(), bytes.len());
    Ok(())
}
