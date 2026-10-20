//! Full scan with timing, cache round trip and incremental update.
use std::time::Instant;

use gw2dat::{Archive, Index, ScanProgress};

fn main() -> gw2dat::Result<()> {
    let archive = Archive::open(std::env::args().nth(1).expect("usage: scan <Gw2.dat>"))?;
    let t = Instant::now();
    let index = Index::scan(&archive, &ScanProgress::default());
    println!("scan: {} files, {} categories in {:?}", index.entries.len(), index.categories.len(), t.elapsed());

    let cache = std::env::temp_dir().join("gw2browser-index-test.bin");
    let t = Instant::now();
    index.save(&cache)?;
    println!("save: {:?} ({} KiB)", t.elapsed(), std::fs::metadata(&cache)?.len() / 1024);
    let t = Instant::now();
    let loaded = Index::load(&cache).expect("cache loads");
    println!("load: {:?}", t.elapsed());

    let progress = ScanProgress::default();
    let t = Instant::now();
    let updated = Index::update(&archive, Some(&loaded), &progress);
    println!("update: rescanned {} in {:?}", progress.total.load(std::sync::atomic::Ordering::Relaxed), t.elapsed());

    assert_eq!(updated.entries.len(), index.entries.len());
    for (a, b) in index.entries.iter().zip(&updated.entries) {
        assert_eq!((a.mft_index, a.file_type, a.size), (b.mft_index, b.file_type, b.size));
        assert_eq!(index.category_path(a.category), updated.category_path(b.category));
    }
    println!("cache round trip identical");
    std::fs::remove_file(cache)?;
    Ok(())
}
