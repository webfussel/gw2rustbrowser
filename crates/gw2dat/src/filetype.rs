//! File type detection from the first bytes of an entry, and the category tree
//! the browser sorts files into.

use crate::fourcc::fcc;

macro_rules! file_types {
    ($($variant:ident => $name:literal, $ext:literal, $kind:ident, [$($category:literal),+];)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum FileType { $($variant),+ }

        impl FileType {
            pub const ALL: &[FileType] = &[$(FileType::$variant),+];

            /// Short, stable name (also used in the index cache and the UI).
            pub fn name(self) -> &'static str {
                match self { $(FileType::$variant => $name),+ }
            }

            /// File extension used when exporting the raw (decompressed) data.
            pub fn extension(self) -> &'static str {
                match self { $(FileType::$variant => $ext),+ }
            }

            pub fn kind(self) -> Kind {
                match self { $(FileType::$variant => Kind::$kind),+ }
            }

            /// Fixed part of the category path; [`categorize`] may add levels.
            pub fn category(self) -> &'static [&'static str] {
                match self { $(FileType::$variant => &[$($category),+]),+ }
            }
        }
    };
}

file_types! {
    Unknown => "unknown", "bin", Binary, ["Unknown"];

    Atex => "ATEX", "atex", Image, ["Textures", "Generic Textures"];
    Attx => "ATTX", "attx", Image, ["Textures", "Terrain Textures"];
    Atec => "ATEC", "atec", Image, ["Textures", "ATEC"];
    Atep => "ATEP", "atep", Image, ["Textures", "Map Textures"];
    Ateu => "ATEU", "ateu", Image, ["Textures", "UI Textures"];
    Cteu => "CTEU", "cteu", Image, ["Textures", "CTEU"];
    Atet => "ATET", "atet", Image, ["Textures", "ATET"];
    Ctex => "CTEX", "ctex", Binary, ["Textures", "CTEX"];
    Dds => "DDS", "dds", Image, ["Textures", "DDS"];
    Jpeg => "JPEG", "jpg", Image, ["Textures", "JPEG"];
    Webp => "WebP", "webp", Image, ["Textures", "WebP"];
    Png => "PNG", "png", Image, ["Textures", "PNG"];

    Sound => "sound", "bin", Binary, ["Sounds"];
    AsndMp3 => "asndMP3", "asnd", Sound, ["Sounds", "asndMP3"];
    AsndOgg => "asndOgg", "asnd", Sound, ["Sounds", "asndOgg"];
    PackedMp3 => "PackedMP3", "asnd", Sound, ["Sounds", "PackedMP3"];
    PackedOgg => "PackedOgg", "asnd", Sound, ["Sounds", "PackedOgg"];
    Ogg => "Ogg", "ogg", Sound, ["Sounds", "Ogg"];
    Mp3 => "MP3", "mp3", Sound, ["Sounds", "MP3"];

    Riff => "RIFF", "riff", Binary, ["Misc", "RIFF"];

    Pf => "PF", "pf", Binary, ["Misc"];
    Manifest => "ARMF", "armf", Binary, ["Manifests"];
    TextPackManifest => "txtm", "txtm", TextPack, ["Manifests", "TextPack"];
    TextPackVariant => "txtV", "txtv", TextPack, ["Texts", "TextPack", "Variant"];
    TextPackVoices => "txtv", "txtvoice", TextPack, ["Texts", "TextPack", "Voices"];
    Bank => "ABNK", "abnk", SoundBank, ["Sounds", "Soundbanks"];
    BankIndex => "ABIX", "abix", SoundBankIndex, ["Sounds", "Soundbank indexes"];
    AudioScript => "AMSP", "amsp", AudioScript, ["Sounds", "Audio scripts"];
    Model => "MODL", "modl", Model, ["Models", "Meshes", "Other"];
    ModelAnimated => "MODL animated", "modl", Model, ["Models", "Meshes", "Models with animation"];
    ModelStatic => "MODL static", "modl", Model, ["Models", "Meshes", "Models without animation"];
    ModelSkeleton => "MODL skeleton", "modl", Model, ["Models", "Animation", "Skeletons"];
    ModelAnimationSet => "MODL animset", "modl", Model, ["Models", "Animation", "Animation sets"];
    ModelMaterials => "MODL materials", "modl", Model, ["Models", "Meshes", "Only materials"];
    ModelEmpty => "MODL empty", "modl", Model, ["Models", "Meshes", "Empty"];
    ModelAnimation => "MODL anim", "modl", Animation, ["Models", "Animation", "Animations"];
    BoneScale => "bone", "bone", Binary, ["Models", "Animation", "Bone scales"];
    StarField => "STAR", "star", StarField, ["Star fields"];
    ModelCollisionManifest => "cmaC", "cmac", CollisionManifest, ["Manifests", "Model Collision"];
    DependencyTable => "DEPS", "deps", Binary, ["Dependency Tables"];
    Eula => "eula", "eula", Eula, ["Texts", "EULA"];
    GameContent => "cntc", "cntc", Content, ["Texts", "Game Content"];
    GameContentPortalManifest => "prlt", "prlt", PortalManifest, ["Manifests", "Game Content Portal"];
    MapCollision => "hvkC", "hvkc", Collision, ["Maps", "Collision"];
    MapParam => "mapc", "mapc", Map, ["Maps"];
    MapShadow => "mpsd", "mpsd", MapShadow, ["Maps", "Shadows"];
    MapMetadata => "mMet", "mmet", MapMetadata, ["Maps", "Metadata"];
    PagedImageTable => "PIMG", "pimg", PagedImage, ["Maps", "World Map Tiles"];
    Material => "AMAT", "amat", Material, ["Materials"];
    Composite => "cmpc", "cmpc", Composite, ["Composite Data"];
    Cinematic => "CINP", "cinp", Cinematic, ["Cinematics"];
    AnimSequences => "anic", "anic", Sequences, ["Models", "Animation", "Animation sequences"];
    EmoteAnimation => "emoc", "emoc", Emotes, ["Models", "Animation", "Emote animations"];
    ShaderCache => "CDHS", "cdhs", Binary, ["Shader Cache"];
    Config => "locl", "locl", Binary, ["Configuration"];
    BitmapFont => "AFNT", "afnt", BitmapFont, ["Fonts", "Bitmap"];
    BitmapFontGlyphs => "AFNT glyphs", "glyphs", FontGlyphs, ["Fonts", "Bitmap", "Glyphs"];

    Binary => "binary", "bin", Binary, ["Binaries"];
    Dll => "DLL", "dll", Binary, ["Binaries"];
    Exe => "EXE", "exe", Binary, ["Binaries"];

    Strings => "strs", "strs", Strings, ["Texts", "Strings"];
    Font => "font", "ttf", Font, ["Fonts", "TrueType"];
    Woff => "WOFF", "woff", Font, ["Fonts", "WOFF"];
    Eot => "EOT", "eot", Font, ["Fonts", "EOT"];
    Bink2Video => "Bink2", "bk2", Video, ["Bink Videos"];
    Flash => "SWF", "swf", Flash, ["Web browser", "Flash"];
    ChromiumPak => "PAK", "pak", ResourcePack, ["Web browser", "Resource packs"];
    IcuData => "ICU data", "dat", IcuData, ["Web browser", "ICU data"];
    V8Snapshot => "V8 snapshot", "bin", V8Snapshot, ["Web browser", "V8 snapshots"];
    Arap => "ARAP", "arap", Binary, ["Misc"];
    Utf8Text => "UTF-8", "txt", Text, ["Texts", "Text"];
    Text => "text", "txt", Text, ["Texts", "Text"];
}

/// Which viewer/exporter handles a file type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Image,
    Sound,
    SoundBank,
    /// Which sound bank holds each bank number, per language.
    SoundBankIndex,
    /// Sound events, triggers and (in one file) the music playlists.
    AudioScript,
    /// Large image (world and region maps) assembled from texture pages.
    PagedImage,
    /// How each emote plays: animation files and part lengths.
    Emotes,
    /// Asset paths of models with their collision files.
    CollisionManifest,
    /// Map start points and the models loaded there.
    PortalManifest,
    /// Havok collision meshes of a model.
    Collision,
    /// Sun shadow over a map.
    MapShadow,
    /// Map ids and types.
    MapMetadata,
    /// How player characters are composed: races, parts, variants.
    Composite,
    /// Cinematic scene: sequences, tracks, dialogue.
    Cinematic,
    /// Bink video (header only: the codec is proprietary).
    Video,
    /// Flash movie (header only).
    Flash,
    /// Chromium resource pack of the in-game browser.
    ResourcePack,
    /// ICU (Unicode) data package of the in-game browser.
    IcuData,
    /// V8 JavaScript engine startup snapshot of the in-game browser.
    V8Snapshot,
    Model,
    Animation,
    Strings,
    Text,
    Eula,
    Content,
    BitmapFont,
    /// Glyph images of one character range of a bitmap font.
    FontGlyphs,
    /// TrueType font.
    Font,
    /// Stars of the night sky.
    StarField,
    /// How game actions play their animations.
    Sequences,
    /// Text index, text variants and voice lines.
    TextPack,
    /// Terrain, props and water of a map.
    Map,
    /// Compiled material: settings, techniques and shaders.
    Material,
    Binary,
}

impl FileType {
    pub fn from_name(name: &str) -> Option<FileType> {
        FileType::ALL.iter().copied().find(|t| t.name() == name)
    }

    pub fn is_texture(self) -> bool {
        matches!(
            self,
            FileType::Atex
                | FileType::Attx
                | FileType::Atec
                | FileType::Atep
                | FileType::Ateu
                | FileType::Atet
                | FileType::Cteu
        )
    }
}

const SIGNATURES: &[(u32, FileType)] = &[
    (fcc(b"ATEX"), FileType::Atex),
    (fcc(b"ATTX"), FileType::Attx),
    (fcc(b"ATEC"), FileType::Atec),
    (fcc(b"ATEP"), FileType::Atep),
    (fcc(b"ATEU"), FileType::Ateu),
    (fcc(b"CTEU"), FileType::Cteu),
    (fcc(b"wOFF"), FileType::Woff),
    (fcc(b"ATET"), FileType::Atet),
    (fcc(b"CTEX"), FileType::Ctex),
    (fcc(b"DDS "), FileType::Dds),
    (fcc(b"strs"), FileType::Strings),
    (fcc(b"OggS"), FileType::Ogg),
    (fcc(b"ARAP"), FileType::Arap),
    (fcc(b"\x89PNG"), FileType::Png),
    // Embedded OpenType fonts carrying a TrueType header.
    (0x0000_0100, FileType::Font),
];

const PF_TYPES: &[(u32, FileType)] = &[
    (fcc(b"ARMF"), FileType::Manifest),
    (fcc(b"txtm"), FileType::TextPackManifest),
    (fcc(b"txtV"), FileType::TextPackVariant),
    (fcc(b"txtv"), FileType::TextPackVoices),
    (fcc(b"ABNK"), FileType::Bank),
    (fcc(b"ABIX"), FileType::BankIndex),
    (fcc(b"AMSP"), FileType::AudioScript),
    (fcc(b"MODL"), FileType::Model),
    (fcc(b"cmaC"), FileType::ModelCollisionManifest),
    (fcc(b"DEPS"), FileType::DependencyTable),
    (fcc(b"eula"), FileType::Eula),
    (fcc(b"cntc"), FileType::GameContent),
    (fcc(b"prlt"), FileType::GameContentPortalManifest),
    (fcc(b"hvkC"), FileType::MapCollision),
    (fcc(b"mapc"), FileType::MapParam),
    (fcc(b"mpsd"), FileType::MapShadow),
    (fcc(b"mMet"), FileType::MapMetadata),
    (fcc(b"PIMG"), FileType::PagedImageTable),
    (fcc(b"AMAT"), FileType::Material),
    (fcc(b"cmpc"), FileType::Composite),
    (fcc(b"anic"), FileType::AnimSequences),
    (fcc(b"emoc"), FileType::EmoteAnimation),
    (fcc(b"CINP"), FileType::Cinematic),
    (fcc(b"CDHS"), FileType::ShaderCache),
    (fcc(b"locl"), FileType::Config),
    (fcc(b"AFNT"), FileType::BitmapFont),
    (fcc(b"bone"), FileType::BoneScale),
    (fcc(b"STAR"), FileType::StarField),
];

/// Bytes read from the start of every entry while scanning. Enough for all
/// types except PE executables, which may ask for more.
pub const PEEK_SIZE: usize = 128;

pub enum Identified {
    Type(FileType),
    /// The type can only be decided with at least this many bytes.
    NeedMore(usize),
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?))
}

/// Identifies a file from its first bytes. `complete` tells whether `data`
/// holds the whole file (then `NeedMore` is never returned).
pub fn identify(data: &[u8], complete: bool) -> Identified {
    use Identified::*;
    let need = |size: usize, fallback: FileType| if complete { Type(fallback) } else { NeedMore(size) };

    let Some(magic) = u32_at(data, 0) else {
        // Files of under four bytes: text if they are text.
        return Type(if !data.is_empty() && is_text(data) { FileType::Text } else { FileType::Unknown });
    };
    if let Some(&(_, file_type)) = SIGNATURES.iter().find(|(m, _)| *m == magic) {
        return Type(file_type);
    }

    if magic == fcc(b"asnd") {
        return match data.get(8) {
            Some(1) => Type(FileType::AsndMp3),
            Some(2) => Type(FileType::AsndOgg),
            Some(_) => Type(FileType::Sound),
            None => need(12, FileType::Sound),
        };
    }

    if magic == fcc(b"RIFF") {
        return match u32_at(data, 8) {
            Some(m) if m == fcc(b"WEBP") => Type(FileType::Webp),
            Some(_) => Type(FileType::Riff),
            None => need(12, FileType::Riff),
        };
    }

    if magic & 0xffff == u32::from_le_bytes(*b"PF\0\0") {
        let Some(pf_type) = u32_at(data, 8) else {
            return need(12, FileType::Pf);
        };
        if pf_type == fcc(b"ASND") {
            // The codec byte lives in the waveform chunk; its offset depends on
            // the pointer width of the file.
            let wide = u16::from_le_bytes([data[2], data[3]]) & 0x4 != 0;
            let offset = crate::formats::audio::packed_codec_offset(wide);
            return match data.get(offset) {
                Some(1) => Type(FileType::PackedMp3),
                Some(2) => Type(FileType::PackedOgg),
                Some(_) => Type(FileType::Sound),
                None => need(offset + 1, FileType::Sound),
            };
        }
        let file_type = PF_TYPES.iter().find(|(m, _)| *m == pf_type).map_or(FileType::Pf, |&(_, t)| t);
        return Type(file_type);
    }

    if magic & 0xffff == u32::from_le_bytes(*b"MZ\0\0") {
        let Some(pe_offset) = u32_at(data, 0x3c) else {
            return need(0x140, FileType::Binary);
        };
        let flags_offset = pe_offset as usize + 0x16;
        return match data.get(flags_offset..flags_offset + 2) {
            Some(flags) if u16::from_le_bytes([flags[0], flags[1]]) & 0x2000 != 0 => Type(FileType::Dll),
            Some(_) => Type(FileType::Exe),
            None => need(flags_offset + 2, FileType::Binary),
        };
    }

    // Chromium resource pack (version 5): the first resource starts right
    // after the header, the entry table and the alias table.
    if magic == 5 && u32_at(data, 4).is_some_and(|encoding| encoding <= 2) {
        let u16_at = |at: usize| data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
        match (u16_at(8), u16_at(10), u32_at(data, 14)) {
            (Some(resources), Some(aliases), Some(first)) if resources > 0 => {
                if first as usize == 12 + (resources + 1) * 6 + aliases * 4 {
                    return Type(FileType::ChromiumPak);
                }
            }
            (Some(_), Some(_), None) | (None, _, _) | (_, None, _) => return need(18, FileType::Unknown),
            _ => {}
        }
    }
    // V8 startup snapshot: context count, rehashability flag, checksum, then
    // the V8 version string (e.g. `10.3.174.18`).
    if magic < 64 && u32_at(data, 4).is_some_and(|flag| flag <= 1) {
        if let Some(version) = data.get(12..76) {
            let end = version.iter().position(|&b| b == 0).unwrap_or(0);
            let text = &version[..end];
            if end >= 5
                && text.iter().all(|b| b.is_ascii_digit() || *b == b'.')
                && text.iter().filter(|&&b| b == b'.').count() == 3
            {
                return Type(FileType::V8Snapshot);
            }
        } else if !complete {
            return NeedMore(76);
        }
    }
    // ICU data package: `0xDA27` after the header size, format `CmnD`.
    if data.get(2..4) == Some(&[0xda, 0x27]) && data.get(12..16) == Some(b"CmnD") {
        return Type(FileType::IcuData);
    }
    // Embedded OpenType font: known version and the `LP` magic at 34.
    if matches!(u32_at(data, 8), Some(0x0001_0000 | 0x0002_0001 | 0x0002_0002)) {
        match data.get(34..36) {
            Some([0x4c, 0x50]) => return Type(FileType::Eot),
            None => return need(36, FileType::Unknown),
            _ => {}
        }
    }

    match magic & 0xff_ffff {
        0x32_424b => return Type(FileType::Bink2Video), // "KB2" + revision letter
        0x53_5743 | 0x53_5746 | 0x53_575a => return Type(FileType::Flash), // "CWS", "FWS", "ZWS"
        0x33_4449 => return Type(FileType::Mp3),        // "ID3"
        0xff_d8ff => return Type(FileType::Jpeg),
        0xbf_bbef => return Type(FileType::Utf8Text), // UTF-8 BOM
        _ => {}
    }

    if is_mpeg_audio_frame(data) {
        return Type(FileType::Mp3);
    }
    Type(if is_text(data) { FileType::Text } else { FileType::Unknown })
}

/// An MPEG audio frame header (MP3 without ID3 tag): sync bits and valid
/// version, layer, bitrate and sample rate fields.
fn is_mpeg_audio_frame(data: &[u8]) -> bool {
    let [0xff, b1, b2, ..] = *data else { return false };
    b1 & 0xe0 == 0xe0 && (b1 >> 3) & 3 != 1 && (b1 >> 1) & 3 != 0 && !matches!(b2 >> 4, 0 | 15) && (b2 >> 2) & 3 != 3
}

/// Printable text: ASCII or UTF-8 (a character cut off at the end of `data`
/// is allowed, since only the start of a file is checked).
fn is_text(data: &[u8]) -> bool {
    let valid = match std::str::from_utf8(data) {
        Ok(text) => text,
        Err(e) if e.error_len().is_none() => std::str::from_utf8(&data[..e.valid_up_to()]).unwrap(),
        Err(_) => return false,
    };
    !valid.is_empty()
        && valid.chars().all(|c| !c.is_control() || matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0b}' | '\u{0c}'))
}

/// Refines a type using the decompressed file size: model files consisting of
/// a single `ANIM` chunk are animation clips, not models.
pub fn refine(file_type: FileType, head: &[u8], size: u32) -> FileType {
    if file_type == FileType::Model && crate::formats::animation::is_animation_only(head, size) {
        FileType::ModelAnimation
    } else {
        file_type
    }
}

/// Full category path of a file. `head` holds the first bytes of the file,
/// `tail` (only needed for strings files) its last two bytes.
pub fn categorize(file_type: FileType, head: &[u8], tail: Option<[u8; 2]>) -> Vec<String> {
    let mut path: Vec<String> = file_type.category().iter().map(|s| s.to_string()).collect();
    let u16_at = |offset: usize| head.get(offset..offset + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));

    match file_type {
        t if t.is_texture() => {
            if let (Some(width), Some(height)) = (u16_at(8), u16_at(10)) {
                path.push(format!("{width}x{height}"));
            }
        }
        FileType::Dds => {
            if let (Some(height), Some(width)) = (u32_at(head, 12), u32_at(head, 16)) {
                path.push(format!("{width}x{height}"));
            }
        }
        FileType::Strings => {
            let language = tail.map(|t| t[0]);
            path.push(match language {
                Some(0) => "English".into(),
                Some(1) => "Korean".into(),
                Some(2) => "French".into(),
                Some(3) => "German".into(),
                Some(4) => "Spanish".into(),
                Some(5) => "Chinese".into(),
                Some(other) => format!("Unknown ({other})"),
                None => "Unknown".into(),
            });
        }
        FileType::Pf => {
            if let Some(pf_type) = u32_at(head, 8) {
                path.push(crate::fourcc::display(pf_type));
            }
        }
        FileType::Text | FileType::Utf8Text => path.push(text_category(head).into()),
        _ => {}
    }
    path
}

/// How much of a text file [`text_category`] needs.
pub const TEXT_PEEK_SIZE: usize = 4096;

const HTML_TAGS: &[&str] = &[
    "html", "head", "body", "div", "script", "link", "meta", "p", "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5", "h6",
    "table", "span", "style", "section", "form", "iframe", "a", "img", "br", "input", "button", "template", "label",
    "select", "textarea", "title", "nav", "header", "footer", "main", "article", "b", "i", "em", "strong", "pre",
    "code",
];
const LICENSE_STARTS: &[&str] = &[
    "apache license",
    "software license agreement",
    "the mit license",
    "mit license",
    "permission is hereby granted",
    "bsd license",
    "gnu general public license",
    "gnu lesser general public license",
    "licensed under",
    "copyright (c)",
    "copyright ©",
];
const LICENSE_SIGNALS: &[&str] = &[
    "copyright",
    "all rights reserved",
    "redistribution and use",
    "permission is hereby granted",
    "license",
    "warranty",
];

/// Content category of a plain text file from its first [`TEXT_PEEK_SIZE`]
/// bytes: the language of web files and scripts, licenses, documentation.
pub fn text_category(head: &[u8]) -> &'static str {
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}');
    let s = text.trim_start();
    let low = s.to_lowercase();
    if low.starts_with("<?php") {
        return "PHP";
    }
    if low.starts_with("<?xml") || low.starts_with("<svg") {
        return "XML";
    }
    let html_tag = low.strip_prefix('<').is_some_and(|rest| {
        HTML_TAGS.iter().any(|tag| {
            rest.strip_prefix(tag)
                .is_some_and(|after| after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/'))
        })
    });
    if low.starts_with("<!doctype html")
        || html_tag
        || low.starts_with("<!--") && (low.contains("<!doctype html") || low.contains("<html"))
    {
        return "HTML";
    }
    if low.starts_with("#!") {
        let first = low.lines().next().unwrap_or("");
        return if first.contains("node") {
            "JavaScript"
        } else if first.contains("python") {
            "Python"
        } else {
            "Shell scripts"
        };
    }
    let license = is_license(&low);
    if is_documentation(s, &low) && !license {
        return "Documentation";
    }
    if license {
        return "Licenses";
    }
    let after_bracket = s.strip_prefix(['{', '[']).map(str::trim_start);
    if after_bracket.is_some_and(|rest| rest.is_empty() || rest.starts_with(['"', '[', '{', ']', '}'])) {
        return "JSON";
    }
    // `var` as a keyword, not the CSS class `.var`.
    let script = has_word(text, "function")
        || text.match_indices("var").any(|(at, _)| {
            !text[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.')
                && text[at + 3..].starts_with(char::is_whitespace)
        })
        || text.contains("=>");
    if is_css(s) && !script {
        return "CSS";
    }
    if low.starts_with("package") && low.contains("import flash.") {
        return "ActionScript";
    }
    let markers = ["YUI.add", "YUI(", "require(", "module.exports", "=>", "use strict", "console."];
    if ["function", "var", "let", "const", "return", "typeof"].iter().any(|w| has_word(text, w))
        || markers.iter().any(|m| text.contains(m))
    {
        return "JavaScript";
    }
    "Plain text"
}

/// `word` occurs with no letter, digit or `_` directly around it.
fn has_word(text: &str, word: &str) -> bool {
    let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    text.match_indices(word)
        .any(|(at, _)| !ident(text[..at].chars().next_back()) && !ident(text[at + word.len()..].chars().next()))
}

fn is_license(low: &str) -> bool {
    if LICENSE_STARTS.iter().any(|p| low.starts_with(p)) {
        return true;
    }
    // A plain text document (not code) full of license wording.
    if !low.starts_with(|c: char| c.is_alphanumeric()) {
        return false;
    }
    let mut end = low.len().min(2000);
    while !low.is_char_boundary(end) {
        end -= 1;
    }
    LICENSE_SIGNALS.iter().filter(|s| low[..end].contains(*s)).count() >= 3
}

/// Change logs, readme files and Markdown documents.
fn is_documentation(s: &str, low: &str) -> bool {
    let mut lines = s.lines();
    let title = lines.next().unwrap_or("").trim();
    let underline = lines.next().unwrap_or("").trim();
    let setext = !title.is_empty()
        && underline.len() >= 3
        && (underline.chars().all(|c| c == '=') || underline.chars().all(|c| c == '-'));
    let hashes = s.len() - s.trim_start_matches('#').len();
    let heading = (1..=6).contains(&hashes) && s[hashes..].starts_with(' ');
    low.get(..200).unwrap_or(low).contains("change history") || setext || heading || low.starts_with("readme")
}

/// Starts (after comments) with a selector, `{` and a property or `}`.
fn is_css(s: &str) -> bool {
    let mut rest = s;
    loop {
        rest = rest.trim_start();
        if let Some(comment) = rest.strip_prefix("/*") {
            let Some(end) = comment.find("*/") else { return false };
            rest = &comment[end + 2..];
        } else if rest.starts_with("//") {
            rest = rest.find('\n').map_or("", |end| &rest[end + 1..]);
        } else {
            break;
        }
    }
    if !rest.starts_with(|c: char| c.is_ascii_alphabetic() || "@.#*:[".contains(c)) {
        return false;
    }
    let Some(open) = rest.find(['{', ';', '}']) else { return false };
    if !rest[open..].starts_with('{') {
        return false;
    }
    let mut body = rest[open + 1..].trim_start();
    while let Some(comment) = body.strip_prefix("/*") {
        let Some(end) = comment.find("*/") else { return false };
        body = comment[end + 2..].trim_start();
    }
    if body.starts_with('}') {
        return true;
    }
    let name = body.len() - body.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == '-').len();
    name > 0 && body[name..].trim_start().starts_with(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identified(data: &[u8]) -> FileType {
        match identify(data, true) {
            Identified::Type(t) => t,
            Identified::NeedMore(_) => panic!("needs more data"),
        }
    }

    fn packed_sound(flags: u16, codec: u8) -> Vec<u8> {
        let mut data = vec![0u8; 96];
        data[..2].copy_from_slice(b"PF");
        data[2..4].copy_from_slice(&flags.to_le_bytes());
        data[8..12].copy_from_slice(b"ASND");
        let wide = flags & 4 != 0;
        data[crate::formats::audio::packed_codec_offset(wide)] = codec;
        data
    }

    #[test]
    fn packed_sound_codec_depends_on_pointer_width() {
        assert_eq!(identified(&packed_sound(1, 1)), FileType::PackedMp3);
        assert_eq!(identified(&packed_sound(5, 1)), FileType::PackedMp3);
        assert_eq!(identified(&packed_sound(5, 2)), FileType::PackedOgg);
        assert_eq!(crate::formats::audio::packed_codec_offset(false), 68);
        assert_eq!(crate::formats::audio::packed_codec_offset(true), 72);
    }

    #[test]
    fn signatures() {
        assert_eq!(identified(b"ATEXDXT5\x40\x00\x40\x00"), FileType::Atex);
        assert_eq!(identified(b"\xff\xd8\xff\xe0rest"), FileType::Jpeg);
        assert_eq!(identified(b"RIFF\0\0\0\0WEBPVP8 "), FileType::Webp);
        assert_eq!(identified(b"plain text\n"), FileType::Text);
        assert_eq!(identified(b"\r\n"), FileType::Text);
        assert_eq!(identified("caf\u{e9} \u{2014} \u{8bed}".as_bytes()), FileType::Text);
        assert_eq!(identified(&[0xff, 0xfb, 0x54, 0x64, 0, 0]), FileType::Mp3);
        assert_eq!(identified(b"KB2i\0\0\0\0"), FileType::Bink2Video);
        assert_eq!(identified(b"CWS\x0e\0\0\0\0"), FileType::Flash);
    }

    #[test]
    fn text_categories() {
        let cases: &[(&str, &str)] = &[
            ("<!DOCTYPE html>\n<html>", "HTML"),
            ("<!--\n/*! normalize.css */\n-->\n<!DOCTYPE html>", "HTML"),
            ("  <div class=\"a\">x</div>", "HTML"),
            ("<?php\necho 1;", "PHP"),
            ("<?xml version=\"1.0\"?><a/>", "XML"),
            ("#!/usr/bin/env node\nvar a;", "JavaScript"),
            ("YUI.add('datatable', function (Y) {", "JavaScript"),
            ("console.log(\"a.js\");", "JavaScript"),
            (".comboLine {\n    /*margin-top: 15px;*/\n    width: 346px;\n}", "CSS"),
            ("/* reset */\n.authDetails input[type=\"text\"] {\n    width: 150px;\n}", "CSS"),
            (".pln{color:#000}.dec,.var{color:#606}", "CSS"),
            ("{\"views\":[{\"height\":1}]}", "JSON"),
            ("Anim Change History\r\n===================\r\n\r\n3.11.0\r\n------\r\n* function names", "Documentation"),
            ("# Title\n\ntext", "Documentation"),
            ("                                 Apache License\n  Version 2.0", "Licenses"),
            (
                "\u{feff}THIRDPARTYSOFTWAREREADME.TXT FOR GUILD WARS 2\n\nLZ4 Library\nCopyright (c) 2011-2020, Yann Collet\nAll rights reserved.\n\nRedistribution and use in source and binary forms",
                "Licenses",
            ),
            ("package {\n\timport flash.display.Sprite;", "ActionScript"),
            ("SUCCESS!\n", "Plain text"),
            ("undefined", "Plain text"),
        ];
        for (text, category) in cases {
            assert_eq!(text_category(text.as_bytes()), *category, "{text:?}");
        }
        assert_eq!(identified(&[0, 1, 2, 3, 4]), FileType::Unknown);
    }

    #[test]
    fn short_headers_ask_for_more() {
        assert!(matches!(identify(b"RIFF\0\0", false), Identified::NeedMore(12)));
        assert!(matches!(identify(b"RIFF\0\0", true), Identified::Type(FileType::Riff)));
    }

    #[test]
    fn names_round_trip() {
        for &t in FileType::ALL {
            assert_eq!(FileType::from_name(t.name()), Some(t), "{t:?}");
        }
    }
}
