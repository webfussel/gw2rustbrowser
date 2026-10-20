//! Shows the MFT entry, type, PF chunks and a hex dump of a file:
//!
//!     cargo run --release -p gw2dat --example inspect -- <Gw2.dat> <id> [bytes]
use gw2dat::formats::pf::PackFile;
use gw2dat::{Archive, filetype, fourcc};

fn main() -> gw2dat::Result<()> {
    let mut args = std::env::args().skip(1);
    let archive = Archive::open(args.next().expect("usage: inspect <Gw2.dat> <id> [bytes]"))?;
    let id: u32 = args.next().expect("id").parse().expect("numeric id");
    let bytes: usize = args.next().map_or(256, |b| b.parse().expect("byte count"));
    let mft = archive.entry_for_id(id).ok_or(gw2dat::Error::NoSuchId(id))?;
    let entry = archive.entry(mft)?;
    let ids = archive.ids(mft);
    let data = archive.read(mft)?;
    let file_type = match filetype::identify(&data, true) {
        filetype::Identified::Type(t) => t.name(),
        filetype::Identified::NeedMore(_) => "?",
    };
    println!(
        "MFT #{mft}: base id {} file id {} offset {:#x} stored {} compressed {}",
        ids.base_id,
        ids.file_id,
        entry.offset,
        entry.size,
        entry.is_compressed()
    );
    println!("type {file_type}, {} bytes", data.len());
    if let Ok(pf) = PackFile::parse(&data) {
        println!(
            "PF {} flags {:#x} ({}-bit pointers)",
            fourcc::display(pf.file_type),
            pf.flags,
            if pf.wide_pointers() { 64 } else { 32 }
        );
        for chunk in pf.chunks() {
            println!(
                "  chunk {} v{} at {:#x}, {} bytes",
                fourcc::display(chunk.magic),
                chunk.version,
                chunk.offset,
                chunk.size
            );
        }
    }
    for (row, chunk) in data[..bytes.min(data.len())].chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect();
        println!("{:08x}  {:<48} {ascii}", row * 16, hex.join(" "));
    }
    Ok(())
}
