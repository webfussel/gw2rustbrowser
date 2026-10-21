//! Decodes every supported file in an archive and reports successes and
//! failures per type. Run after game updates to catch format changes:
//!
//!     cargo run --release -p gw2dat --example validate -- <Gw2.dat>
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

use gw2dat::formats::{
    animation, audio, audio_script, cinematic, collision, composite, content, emotes, font, image, manifests, map,
    map_data, material, media, model, paged_image, sequences, stars, text, textpack, truetype, web,
};
use gw2dat::{Archive, FileType, Index, Kind, ScanProgress};
use rayon::prelude::*;

#[derive(Default)]
struct Stat {
    ok: usize,
    failed: usize,
    examples: Vec<String>,
}

fn main() -> gw2dat::Result<()> {
    let path = std::env::args().nth(1).expect("usage: validate <Gw2.dat>");
    let archive = Archive::open(&path)?;
    let started = Instant::now();
    let index = Index::scan(&archive, &ScanProgress::default());
    println!("{} files scanned in {:?}", index.entries.len(), started.elapsed());

    let stats: Mutex<BTreeMap<String, Stat>> = Mutex::default();
    let record = |key: &str, result: Result<(), String>, id: u32| {
        let mut stats = stats.lock().unwrap();
        let stat = stats.entry(key.to_string()).or_default();
        match result {
            Ok(()) => stat.ok += 1,
            Err(e) => {
                stat.failed += 1;
                if stat.examples.len() < 2 {
                    stat.examples.push(format!("{id}: {e}"));
                }
            }
        }
    };

    index.entries.par_iter().for_each(|e| {
        let kind = e.file_type.kind();
        if kind == Kind::Binary && e.file_type != FileType::Sound {
            return;
        }
        let data = match archive.read(e.mft_index) {
            Ok(data) => data,
            Err(err) => return record("read", Err(err.to_string()), e.base_id),
        };
        let key = format!("{:?} {}", kind, e.file_type.name());
        let result: Result<(), String> = match kind {
            Kind::Image => image::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Sound | Kind::Binary => audio::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::SoundBank => audio::decode_bank(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Strings => text::decode_strings(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Eula => text::decode_eula(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Content => content::to_xml(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::BitmapFont => font::fonts(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Model => match model::decode(&data) {
                Ok(_) => model::materials(&data).map(|_| ()).map_err(|e| format!("materials: {e}")),
                // Animation, skeleton and collision files carry no geometry.
                Err(err) if err.to_string().contains("no GEOM chunk") => {
                    record("Model without geometry", Ok(()), e.base_id);
                    return;
                }
                Err(err) => Err(err.to_string()),
            },
            Kind::Animation => match animation::info(&data) {
                Ok(a) if !a.name.is_empty() && a.duration.is_finite() && a.duration >= 0.0 && a.time_step > 0.0 => {
                    Ok(())
                }
                Ok(a) => Err(format!("implausible header {a:?}")),
                Err(err) => Err(err.to_string()),
            },
            // MicroType Express-compressed EOT fonts only have their header names read.
            Kind::Font if truetype::is_compressed_eot(&data) => {
                truetype::eot_info(&data).map(|_| ()).map_err(|e| e.to_string())
            }
            Kind::Font => {
                truetype::to_sfnt(&data).and_then(|s| truetype::info(&s)).map(|_| ()).map_err(|e| e.to_string())
            }
            Kind::Video => media::bink(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Flash => media::flash(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::ResourcePack => web::pak(&data)
                .and_then(|p| {
                    p.resources.iter().try_for_each(|r| web::decompress(&data[r.offset..r.offset + r.size]).map(|_| ()))
                })
                .map_err(|e| e.to_string()),
            Kind::IcuData => web::icu(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::V8Snapshot => web::v8_snapshot(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::StarField => stars::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Sequences => sequences::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            // Only scene versions 35 and 36 are decoded.
            Kind::Cinematic => match cinematic::decode(&data) {
                Err(gw2dat::Error::Unsupported(_)) => Ok(()),
                other => other.map(|_| ()).map_err(|e| e.to_string()),
            },
            Kind::Composite => composite::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Collision => collision::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::MapShadow => map_data::shadow(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::MapMetadata => map_data::metadata(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::CollisionManifest => manifests::collision_manifest(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::PortalManifest => manifests::portal_manifest(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Emotes => emotes::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::PagedImage => paged_image::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::AudioScript => audio_script::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::SoundBankIndex => audio::decode_bank_index(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Material => material::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::Map => map::decode(&data).map(|_| ()).map_err(|e| e.to_string()),
            Kind::TextPack => match e.file_type {
                FileType::TextPackManifest => textpack::manifest(&data).map(|_| ()),
                FileType::TextPackVariant => textpack::variants(&data).map(|_| ()),
                _ => textpack::voices(&data).map(|_| ()),
            }
            .map_err(|e| e.to_string()),
            // Glyph files need their character range from the font file; the
            // fonts are checked through `Kind::BitmapFont`.
            Kind::FontGlyphs | Kind::Text => Ok(()),
        };
        record(&key, result, e.base_id);
    });

    println!("{:<34} {:>9} {:>8}", "", "ok", "failed");
    let mut failures = 0;
    for (key, stat) in stats.into_inner().unwrap() {
        failures += stat.failed;
        println!("{key:<34} {:>9} {:>8}  {}", stat.ok, stat.failed, stat.examples.join(" | "));
    }
    println!("done in {:?}, {failures} failures", started.elapsed());
    Ok(())
}
