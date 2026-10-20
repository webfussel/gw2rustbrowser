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
    TextPackManifest => "txtm", "txtm", Binary, ["TextPack Manifests"];
    TextPackVariant => "txtV", "txtv", Binary, ["TextPack Variant"];
    TextPackVoices => "txtv", "txtvoice", Binary, ["TextPack Voices"];
    Bank => "ABNK", "abnk", SoundBank, ["Soundbank"];
    BankIndex => "ABIX", "abix", Binary, ["Soundbank Index"];
    AudioScript => "AMSP", "amsp", Binary, ["Audio Scripts"];
    Model => "MODL", "modl", Model, ["Models", "Other"];
    ModelProp => "MODL prop", "modl", Model, ["Models", "Props"];
    ModelMultiPart => "MODL multi", "modl", Model, ["Models", "Multi-part props"];
    ModelAnimated => "MODL animated", "modl", Model, ["Models", "Animated"];
    ModelEquipment => "MODL equip", "modl", Model, ["Models", "Equipment & body parts"];
    ModelCreature => "MODL creature", "modl", Model, ["Models", "Creatures & NPCs"];
    ModelSkeleton => "MODL skeleton", "modl", Model, ["Models", "Skeletons"];
    ModelAnimationSet => "MODL animset", "modl", Model, ["Models", "Animation sets"];
    ModelEmpty => "MODL empty", "modl", Model, ["Models", "Without geometry"];
    ModelAnimation => "MODL anim", "modl", Animation, ["Models", "Animations"];
    BoneScale => "bone", "bone", Binary, ["Models", "Bone scales"];
    ModelCollisionManifest => "cmaC", "cmac", Binary, ["Model Collision Manifest"];
    DependencyTable => "DEPS", "deps", Binary, ["Dependency Tables"];
    Eula => "eula", "eula", Eula, ["EULA"];
    GameContent => "cntc", "cntc", Content, ["Game Content"];
    GameContentPortalManifest => "prlt", "prlt", Binary, ["Game Content Portal Manifest"];
    MapCollision => "hvkC", "hvkc", Binary, ["Map Collision"];
    MapParam => "mapc", "mapc", Binary, ["Map"];
    MapShadow => "mpsd", "mpsd", Binary, ["Map Shadow"];
    MapMetadata => "mMet", "mmet", Binary, ["Map Metadata"];
    PagedImageTable => "PIMG", "pimg", Binary, ["Paged Image Table"];
    Material => "AMAT", "amat", Binary, ["Materials"];
    Composite => "cmpc", "cmpc", Binary, ["Composite Data"];
    Cinematic => "CINP", "cinp", Binary, ["Cinematics"];
    AnimSequences => "anic", "anic", Binary, ["Animation Sequences"];
    EmoteAnimation => "emoc", "emoc", Binary, ["Emote Animations"];
    ShaderCache => "CDHS", "cdhs", Binary, ["Shader Cache"];
    Config => "locl", "locl", Binary, ["Configuration"];
    BitmapFont => "AFNT", "afnt", BitmapFont, ["Bitmap Font"];
    BitmapFontGlyphs => "AFNT glyphs", "glyphs", Binary, ["Bitmap Font", "Glyphs"];

    Binary => "binary", "bin", Binary, ["Binaries"];
    Dll => "DLL", "dll", Binary, ["Binaries"];
    Exe => "EXE", "exe", Binary, ["Binaries"];

    Strings => "strs", "strs", Strings, ["Strings"];
    Font => "font", "eot", Binary, ["Font"];
    Bink2Video => "Bink2", "bk2", Binary, ["Bink Videos"];
    Arap => "ARAP", "arap", Binary, ["Misc"];
    Utf8Text => "UTF-8", "txt", Text, ["Text"];
    Text => "text", "txt", Text, ["Text"];
}

/// Which viewer/exporter handles a file type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Image,
    Sound,
    SoundBank,
    Model,
    Animation,
    Strings,
    Text,
    Eula,
    Content,
    BitmapFont,
    Binary,
}

impl FileType {
    pub fn from_name(name: &str) -> Option<FileType> {
        FileType::ALL.iter().copied().find(|t| t.name() == name)
    }

    pub fn is_texture(self) -> bool {
        matches!(
            self,
            FileType::Atex | FileType::Attx | FileType::Atec | FileType::Atep | FileType::Ateu | FileType::Atet
        )
    }
}

const SIGNATURES: &[(u32, FileType)] = &[
    (fcc(b"ATEX"), FileType::Atex),
    (fcc(b"ATTX"), FileType::Attx),
    (fcc(b"ATEC"), FileType::Atec),
    (fcc(b"ATEP"), FileType::Atep),
    (fcc(b"ATEU"), FileType::Ateu),
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
        return Type(FileType::Unknown);
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

    match magic & 0xff_ffff {
        0x32_4b42 => return Type(FileType::Bink2Video), // "BK2"
        0x33_4449 => return Type(FileType::Mp3),        // "ID3"
        0xff_d8ff => return Type(FileType::Jpeg),
        0xbf_bbef => return Type(FileType::Utf8Text), // UTF-8 BOM
        _ => {}
    }

    let is_text = data.iter().all(|&c| c.is_ascii_graphic() || matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c));
    Type(if is_text { FileType::Text } else { FileType::Unknown })
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
        _ => {}
    }
    path
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
