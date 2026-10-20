//! Checks against a real Gw2.dat. Skipped unless `GW2_DAT` points to one:
//!
//!     GW2_DAT=/path/to/Gw2.dat cargo test --release -p gw2dat --test real_archive

use std::collections::HashMap;

use gw2dat::formats::{audio, image, model, text};
use gw2dat::{Archive, FileType, Index, Kind, ScanProgress};

fn archive() -> Option<Archive> {
    let path = std::env::var_os("GW2_DAT")?;
    Some(Archive::open(path).expect("GW2_DAT must point to a readable Gw2.dat"))
}

#[test]
fn decodes_a_sample_of_every_kind() {
    let Some(archive) = archive() else {
        eprintln!("GW2_DAT not set, skipping");
        return;
    };
    let index = Index::scan(&archive, &ScanProgress::default());
    assert!(index.entries.len() > 100_000);

    let mut seen: HashMap<FileType, usize> = HashMap::new();
    for entry in &index.entries {
        let count = seen.entry(entry.file_type).or_default();
        if *count >= 25 {
            continue;
        }
        *count += 1;
        let data = archive.read(entry.mft_index).unwrap();
        assert_eq!(data.len() as u32, entry.size, "size of {}", entry.base_id);
        let peek = archive.peek(entry.mft_index, 64).unwrap();
        assert_eq!(&data[..peek.len()], &peek[..], "peek of {}", entry.base_id);

        let id = entry.base_id;
        match entry.file_type.kind() {
            Kind::Image => assert!(image::decode(&data).is_ok(), "image {id}"),
            Kind::Sound => assert!(audio::decode(&data).is_ok(), "sound {id}"),
            Kind::SoundBank => assert!(audio::decode_bank(&data).is_ok(), "bank {id}"),
            Kind::Strings => assert!(text::decode_strings(&data).is_ok(), "strings {id}"),
            Kind::Eula => assert!(text::decode_eula(&data).is_ok(), "eula {id}"),
            Kind::Model => {
                if let Ok(m) = model::decode(&data) {
                    for mesh in &m.meshes {
                        assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()), "model {id}");
                    }
                    assert!(model::materials(&data).is_ok(), "materials {id}");
                }
            }
            _ => {}
        }
    }
    for kind in [FileType::Atex, FileType::PackedMp3, FileType::Model, FileType::Strings] {
        assert!(seen.contains_key(&kind), "no {kind:?} found");
    }
}

#[test]
fn ids_resolve_to_their_entries() {
    let Some(archive) = archive() else { return };
    let index = Index::scan(&archive, &ScanProgress::default());
    for entry in index.entries.iter().filter(|e| e.base_id != 0).step_by(997) {
        assert_eq!(archive.entry_for_id(entry.base_id), Some(entry.mft_index));
        if entry.file_id != 0 {
            assert_eq!(archive.entry_for_id(entry.file_id), Some(entry.mft_index));
        }
    }
}
