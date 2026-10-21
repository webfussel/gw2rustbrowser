//! Decoders for the file formats stored in the archive.

pub mod animation;
pub mod audio;
pub mod content;
pub mod font;
pub mod granny;
pub mod image;
pub mod model;
pub mod pf;
pub mod sequences;
pub mod skeleton;
pub mod stars;
pub mod text;
pub mod truetype;

use crate::error::Result;
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
            _ => file_type.extension(),
        },
    }
}
