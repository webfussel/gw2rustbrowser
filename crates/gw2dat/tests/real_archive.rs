//! Checks against a real Gw2.dat. Skipped unless `GW2_DAT` points to one:
//!
//!     GW2_DAT=/path/to/Gw2.dat cargo test --release -p gw2dat --test real_archive

use std::collections::HashMap;

use gw2dat::formats::{
    audio, audio_script, cinematic, collision, composite, emotes, image, manifests, map, map_data, material, media,
    model, paged_image, shader, text, truetype, web,
};
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
            Kind::Video => {
                let b = media::bink(&data).unwrap_or_else(|e| panic!("Bink {id}: {e}"));
                assert!(b.width > 0 && b.height > 0 && b.fps() > 0.0, "Bink header of {id}");
            }
            Kind::Flash => {
                media::flash(&data).unwrap_or_else(|e| panic!("SWF {id}: {e}"));
            }
            Kind::ResourcePack => {
                let pak = web::pak(&data).unwrap_or_else(|e| panic!("PAK {id}: {e}"));
                for r in &pak.resources {
                    web::decompress(&data[r.offset..r.offset + r.size])
                        .unwrap_or_else(|e| panic!("PAK {id}/{}: {e}", r.id));
                }
            }
            Kind::IcuData => {
                let icu = web::icu(&data).unwrap_or_else(|e| panic!("ICU {id}: {e}"));
                assert!(icu.copyright.starts_with("Copyright") && !icu.entries.is_empty(), "ICU {id}");
            }
            Kind::V8Snapshot => {
                let v8 = web::v8_snapshot(&data).unwrap_or_else(|e| panic!("V8 {id}: {e}"));
                assert_eq!(v8.version.split('.').count(), 4, "V8 version of {id}");
            }
            Kind::Font if truetype::is_compressed_eot(&data) => {
                truetype::eot_info(&data).unwrap_or_else(|e| panic!("EOT {id}: {e}"));
            }
            Kind::Font => {
                let sfnt = truetype::to_sfnt(&data).unwrap_or_else(|e| panic!("font {id}: {e}"));
                assert!(truetype::info(&sfnt).is_ok_and(|i| i.glyphs > 0), "font {id}");
            }
            Kind::Cinematic => match cinematic::decode(&data) {
                Ok(_) | Err(gw2dat::Error::Unsupported(_)) => {}
                Err(e) => panic!("CINP {id}: {e}"),
            },
            Kind::Composite => {
                let c = composite::decode(&data).unwrap_or_else(|e| panic!("cmpc {id}: {e}"));
                assert!(c.races.iter().any(|r| r.name == "HumanFemale"), "cmpc {id} races");
            }
            Kind::Collision => {
                collision::decode(&data).unwrap_or_else(|e| panic!("hvkC {id}: {e}"));
            }
            Kind::MapShadow => {
                let s = map_data::shadow(&data).unwrap_or_else(|e| panic!("mpsd {id}: {e}"));
                assert_eq!(
                    s.tiles.len() as u32,
                    s.width.div_ceil(s.tile_width) * s.height.div_ceil(s.tile_height),
                    "mpsd {id}"
                );
            }
            Kind::MapMetadata => {
                assert!(!map_data::metadata(&data).unwrap_or_else(|e| panic!("mMet {id}: {e}")).is_empty());
            }
            Kind::CollisionManifest => {
                let entries = manifests::collision_manifest(&data).unwrap_or_else(|e| panic!("cmaC {id}: {e}"));
                assert!(entries.iter().all(|e| e.model.is_some() && e.collision.is_some()), "cmaC files of {id}");
            }
            Kind::PortalManifest => {
                manifests::portal_manifest(&data).unwrap_or_else(|e| panic!("prlt {id}: {e}"));
            }
            Kind::Emotes => {
                let list = emotes::decode(&data).unwrap_or_else(|e| panic!("emotes {id}: {e}"));
                assert!(list.iter().flat_map(|e| &e.timings).all(|t| t.model.is_some()), "emote models of {id}");
            }
            Kind::PagedImage => {
                let image = paged_image::decode(&data).unwrap_or_else(|e| panic!("paged image {id}: {e}"));
                assert!(image.pages.iter().all(|p| (p.layer as usize) < image.layers.len()), "page layers of {id}");
            }
            Kind::AudioScript => {
                audio_script::decode(&data).unwrap_or_else(|e| panic!("audio script {id}: {e}"));
            }
            Kind::SoundBankIndex => {
                // Most indexes only have empty language slots; one lists every bank.
                audio::decode_bank_index(&data).unwrap_or_else(|e| panic!("bank index {id}: {e}"));
            }
            Kind::Material => {
                let m = material::decode(&data).unwrap_or_else(|e| panic!("material {id}: {e}"));
                for s in &m.shaders {
                    let bgfx = shader::bgfx(&s.binary).unwrap_or_else(|e| panic!("shader of {id}: {e}"));
                    let dxbc = shader::dxbc(bgfx.code).unwrap_or_else(|e| panic!("DXBC of {id}: {e}"));
                    dxbc.disassemble().expect("program").unwrap_or_else(|e| panic!("disassembly of {id}: {e}"));
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
        FileType::Material,
        FileType::BankIndex,
        FileType::AudioScript,
        FileType::PagedImageTable,
        FileType::EmoteAnimation,
        FileType::ModelCollisionManifest,
        FileType::GameContentPortalManifest,
        FileType::MapCollision,
        FileType::MapShadow,
        FileType::MapMetadata,
        FileType::Composite,
        FileType::Cinematic,
        FileType::Bink2Video,
        FileType::Flash,
        FileType::ChromiumPak,
        FileType::IcuData,
        FileType::V8Snapshot,
        FileType::Woff,
        FileType::Eot,
        FileType::Cteu,
        FileType::Mp3,
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
