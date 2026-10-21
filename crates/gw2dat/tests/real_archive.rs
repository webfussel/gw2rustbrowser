//! Checks against a real Gw2.dat. Skipped unless `GW2_DAT` points to one:
//!
//!     GW2_DAT=/path/to/Gw2.dat cargo test --release -p gw2dat --test real_archive

use std::collections::HashMap;

use gw2dat::formats::{audio, image, map, model, text};
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
            Kind::Map => {
                let m = map::decode(&data).unwrap_or_else(|e| panic!("map {id}: {e}"));
                if let Some(t) = &m.terrain {
                    assert_eq!(t.heights.len(), t.columns * t.rows, "terrain {id}");
                }
                // Props lie on the map (a few decorations reach past its edge).
                let [x0, y0, x1, y1] = m.rect;
                let inside =
                    m.props.iter().filter(|p| (x0..=x1).contains(&p.position[0]) && (y0..=y1).contains(&p.position[1]));
                assert!(inside.count() * 10 >= m.props.len() * 9, "props of map {id}");
            }
            _ => {}
        }
    }
    for kind in [
        FileType::Atex,
        FileType::PackedMp3,
        FileType::ModelStatic,
        FileType::ModelAnimated,
        FileType::ModelAnimation,
        FileType::Strings,
        FileType::MapParam,
    ] {
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
