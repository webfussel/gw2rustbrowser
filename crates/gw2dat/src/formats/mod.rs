//! Decoders for the file formats stored in the archive.

pub mod animation;
pub mod audio;
pub mod audio_script;
pub mod cinematic;
pub mod collision;
pub mod composite;
pub mod content;
pub mod emotes;
pub mod font;
pub mod granny;
pub mod image;
pub mod manifests;
pub mod map;
pub mod map_data;
pub mod material;
pub mod model;
pub mod paged_image;
pub mod pf;
pub mod sequences;
pub mod shader;
pub mod skeleton;
pub mod stars;
pub mod text;
pub mod textpack;
pub mod truetype;

use crate::error::{Error, Result};
use crate::filetype::{FileType, Kind};

/// A file converted to a common format for export.
pub struct Converted {
    pub data: Vec<u8>,
    pub extension: &'static str,
}

/// Converts a file to the most useful common format for its type. Types
/// without a conversion are returned unchanged with their raw extension.
pub fn convert(file_type: FileType, data: Vec<u8>) -> Result<Converted> {
    let converted = match file_type.kind() {
        Kind::Image => Converted { data: image::decode(&data)?.to_png()?, extension: "png" },
        Kind::Sound => {
            let sound = audio::decode(&data)?;
            Converted { extension: sound.codec.extension(), data: sound.data }
        }
        Kind::Strings => {
            Converted { data: text::strings_to_tsv(&text::decode_strings(&data)?).into_bytes(), extension: "tsv" }
        }
        Kind::Eula => {
            let texts = text::decode_eula(&data)?;
            let joined = texts
                .iter()
                .map(|t| format!("===== {} =====\n{}", text::language_name(t.language), t.text))
                .collect::<Vec<_>>()
                .join("\n\n");
            Converted { data: joined.into_bytes(), extension: "txt" }
        }
        Kind::Text => Converted { data, extension: "txt" },
        Kind::Content => Converted { data: content::to_xml(&data)?.into_bytes(), extension: "xml" },
        Kind::Font => Converted { data, extension: "ttf" },
        Kind::Sequences => {
            Converted { data: sequences::to_json(&sequences::decode(&data)?).into_bytes(), extension: "json" }
        }
        Kind::StarField => Converted { data: stars::to_csv(&stars::decode(&data)?).into_bytes(), extension: "csv" },
        Kind::SoundBankIndex => Converted {
            data: audio::bank_index_to_csv(&audio::decode_bank_index(&data)?).into_bytes(),
            extension: "csv",
        },
        Kind::AudioScript => {
            Converted { data: audio_script::to_text(&audio_script::decode(&data)?).into_bytes(), extension: "txt" }
        }
        Kind::Emotes => Converted { data: emotes::to_csv(&emotes::decode(&data)?).into_bytes(), extension: "csv" },
        Kind::CollisionManifest => Converted {
            data: manifests::collision_csv(&manifests::collision_manifest(&data)?).into_bytes(),
            extension: "csv",
        },
        Kind::PortalManifest => Converted {
            data: manifests::portal_csv(&manifests::portal_manifest(&data)?).into_bytes(),
            extension: "csv",
        },
        Kind::Collision => {
            Converted { data: collision::to_obj(&collision::decode(&data)?).into_bytes(), extension: "obj" }
        }
        Kind::MapShadow => {
            Converted { data: map_data::shadow_image(&map_data::shadow(&data)?).to_png()?, extension: "png" }
        }
        Kind::MapMetadata => {
            let rows: String = map_data::metadata(&data)?.iter().map(|(id, kind)| format!("{id},{kind}\n")).collect();
            Converted { data: format!("map_id,map_type\n{rows}").into_bytes(), extension: "csv" }
        }
        // Older scene versions are not decoded: export them unchanged.
        Kind::Cinematic => match cinematic::decode(&data) {
            Ok(scene) => Converted { data: cinematic::to_text(&scene).into_bytes(), extension: "txt" },
            Err(Error::Unsupported(_)) => Converted { data, extension: file_type.extension() },
            Err(e) => return Err(e),
        },
        Kind::Map => Converted { data: map::heightmap_png(&map::decode(&data)?)?, extension: "png" },
        Kind::Material => {
            Converted { data: material::to_text(&material::decode(&data)?).into_bytes(), extension: "txt" }
        }
        Kind::Model => Converted { data: model::to_obj(&model::decode(&data)?, "").into_bytes(), extension: "obj" },
        _ => Converted { data, extension: file_type.extension() },
    };
    Ok(converted)
}

/// Converts a file into one or more output files. Sound banks expand to one
/// file per playable entry, named with a `_<index>` suffix.
pub fn convert_all(file_type: FileType, data: Vec<u8>) -> Result<Vec<(Option<String>, Converted)>> {
    if file_type.kind() == Kind::SoundBank {
        let entries = audio::decode_bank(&data)?;
        return Ok(entries
            .into_iter()
            .filter_map(|e| {
                let sound = e.sound.ok()?;
                Some((
                    Some(format!("_{:03}", e.index)),
                    Converted { extension: sound.codec.extension(), data: sound.data },
                ))
            })
            .collect());
    }
    Ok(vec![(None, convert(file_type, data)?)])
}

/// Extension a converted export of this type will have.
pub fn converted_extension(file_type: FileType) -> &'static str {
    match file_type {
        FileType::AsndMp3 | FileType::PackedMp3 | FileType::Mp3 => "mp3",
        FileType::AsndOgg | FileType::PackedOgg | FileType::Ogg => "ogg",
        _ => match file_type.kind() {
            Kind::Image => "png",
            Kind::Strings => "tsv",
            Kind::Eula | Kind::Text => "txt",
            Kind::Model => "obj",
            Kind::Content => "xml",
            Kind::BitmapFont | Kind::FontGlyphs => "png",
            Kind::Font => "ttf",
            Kind::StarField => "csv",
            Kind::Sequences => "json",
            Kind::Map | Kind::MapShadow => "png",
            Kind::Cinematic => "txt",
            Kind::Collision => "obj",
            Kind::MapMetadata => "csv",
            Kind::CollisionManifest | Kind::PortalManifest => "csv",
            Kind::Emotes => "csv",
            Kind::AudioScript => "txt",
            Kind::SoundBankIndex => "csv",
            Kind::Material => "txt",
            _ => file_type.extension(),
        },
    }
}
