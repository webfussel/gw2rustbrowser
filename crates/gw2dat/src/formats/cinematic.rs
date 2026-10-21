//! Cinematic scenes (PF `CINP`, chunk `CSCN`).
//!
//! A scene holds sequences (shots played in a map) made of track groups
//! (actors, cameras, lights, sounds) whose tracks carry animation curves,
//! flag keys and trigger keys; plus resources: ambient lights, referenced
//! files, scripts, the dialogue texts with their voice ids, and species.
//! The layout follows gw2formats' `SceneDataV36`; versions 35 and 36 (938 of
//! 1,457 files) are decoded. Older versions lay out sequences, track groups
//! and resources differently.

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const CINP: u32 = fcc(b"CINP");

#[derive(Clone, Debug)]
pub struct Scene {
    pub version: u16,
    pub starting_sequence: u64,
    pub sequences: Vec<Sequence>,
    pub ambient_lights: Vec<AmbientLight>,
    /// (name, file id).
    pub files: Vec<(u64, Option<u32>)>,
    /// (name, byte code size).
    pub scripts: Vec<(u64, usize)>,
    pub texts: Vec<TextResource>,
    pub species: Vec<Species>,
    pub track_groups: Option<TrackGroup>,
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub name: u64,
    pub play_script: u64,
    pub update_script: u64,
    pub environment_map: Option<u32>,
    pub map: String,
    pub client_map: String,
    /// Seconds.
    pub length: f32,
    pub flags: u32,
    pub track_groups: Vec<TrackGroup>,
}

#[derive(Clone, Debug)]
pub struct TrackGroup {
    pub name: u64,
    pub flags: u32,
    pub properties: Vec<Property>,
    pub tracks: Vec<Track>,
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct Property {
    pub value: u64,
    pub file: Option<u32>,
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub name: u64,
    /// (time, value, in tangent, out tangent).
    pub curve_keys: Vec<[f32; 4]>,
    /// (time, value).
    pub flag_keys: Vec<[f32; 2]>,
    pub trigger_keys: Vec<TriggerKey>,
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct TriggerKey {
    pub time: f32,
    pub flags: [u8; 4],
    pub token1: u64,
    pub token2: u64,
    pub values: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct AmbientLight {
    pub name: u64,
    /// ground, sky, fill, hemispherical: (intensity, RGB).
    pub colors: [(f32, [u8; 3]); 4],
}

/// A line of dialogue or other text, per language.
#[derive(Clone, Debug)]
pub struct TextResource {
    pub name: u64,
    pub index: u32,
    pub voice: u32,
    /// (language, text).
    pub entries: Vec<(u8, String)>,
}

#[derive(Clone, Debug)]
pub struct Species {
    pub id: [u8; 16],
    pub name: u64,
    pub model: u64,
    pub variant: u64,
}

pub fn decode(data: &[u8]) -> Result<Scene> {
    let file = PackFile::parse(data)?;
    if file.file_type != CINP {
        return Err(Error::format("cinematic", "not a CINP file"));
    }
    let chunk = file.require_chunk(fcc(b"CSCN"))?;
    if !matches!(chunk.version, 35 | 36) {
        return Err(Error::Unsupported(format!(
            "cinematic scene version {} (versions 35 and 36 are decoded)",
            chunk.version
        )));
    }
    let mut c = chunk.cursor();
    let starting_sequence = c.u64()?;
    let sequences = list(&mut c, sequence)?;
    let ambient_lights = list(&mut c, |a| {
        let mut color = || -> Result<(f32, [u8; 3])> { Ok((a.f32()?, [a.u8()?, a.u8()?, a.u8()?])) };
        let colors = [color()?, color()?, color()?, color()?];
        Ok(AmbientLight { colors, name: a.u64()? })
    })?;
    let files = list(&mut c, |f| Ok((f.u64()?, f.file_ref()?)))?;
    let scripts = list(&mut c, |s| Ok((s.u64()?, s.array()?.0)))?;
    let texts = list(&mut c, |t| {
        Ok(TextResource {
            name: t.u64()?,
            index: t.u32()?,
            voice: t.u32()?,
            entries: list(t, |e| {
                let text = e.wstring()?;
                Ok((e.u8()?, text))
            })?,
        })
    })?;
    let species = list(&mut c, |s| {
        Ok(Species { id: s.take(16)?.try_into().unwrap(), name: s.u64()?, model: s.u64()?, variant: s.u64()? })
    })?;
    let track_groups = c.ptr()?.map(|mut g| track_group(&mut g)).transpose()?;
    Ok(Scene {
        version: chunk.version,
        starting_sequence,
        sequences,
        ambient_lights,
        files,
        scripts,
        texts,
        species,
        track_groups,
    })
}

fn list<T>(c: &mut Cursor, mut item: impl FnMut(&mut Cursor) -> Result<T>) -> Result<Vec<T>> {
    let (count, mut at) = c.array()?;
    let mut items = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        items.push(item(&mut at)?);
    }
    Ok(items)
}

fn sequence(c: &mut Cursor) -> Result<Sequence> {
    Ok(Sequence {
        name: c.u64()?,
        play_script: c.u64()?,
        update_script: c.u64()?,
        environment_map: c.file_ref()?,
        map: c.wstring()?,
        client_map: c.wstring()?,
        length: c.f32()?,
        flags: c.u32()?,
        track_groups: list(c, track_group)?,
    })
}

fn track_group(c: &mut Cursor) -> Result<TrackGroup> {
    Ok(TrackGroup {
        name: c.u64()?,
        flags: c.u32()?,
        properties: list(c, |p| Ok(Property { value: p.u64()?, file: p.file_ref()?, kind: p.u8()? }))?,
        tracks: list(c, |t| {
            Ok(Track {
                name: t.u64()?,
                curve_keys: list(t, |k| Ok([k.f32()?, k.f32()?, k.f32()?, k.f32()?]))?,
                flag_keys: list(t, |k| Ok([k.f32()?, k.f32()?]))?,
                trigger_keys: list(t, |k| {
                    Ok(TriggerKey {
                        time: k.f32()?,
                        flags: [k.u8()?, k.u8()?, k.u8()?, k.u8()?],
                        token1: k.u64()?,
                        token2: k.u64()?,
                        values: [k.f32()?, k.f32()?, k.f32()?, k.f32()?],
                    })
                })?,
                kind: t.u8()?,
            })
        })?,
        kind: c.u8()?,
    })
}

/// What a track group drives, from its type number (named after the groups'
/// own names, e.g. `cameraa`, `audioa`).
pub fn group_kind_name(kind: u8) -> &'static str {
    match kind {
        0 => "ambient light",
        1 => "audio",
        2 => "camera",
        3 => "camera cuts",
        5 => "colour filter",
        6 => "directional light",
        7 => "framing",
        8 => "image",
        9 => "model",
        10 => "screen effect",
        11 => "script",
        12 => "text",
        14 => "video",
        15 => "volume",
        _ => "other",
    }
}

/// The scene's dialogue as text: one line per text resource.
pub fn to_text(scene: &Scene) -> String {
    use super::material::token_label;
    let mut out = String::new();
    for s in &scene.sequences {
        out.push_str(&format!(
            "sequence {} ({:.2} s){}\n",
            token_label(s.name),
            s.length,
            if s.map.is_empty() { String::new() } else { format!(", map {}", s.map) }
        ));
    }
    for t in &scene.texts {
        for (_, text) in t.entries.iter().filter(|(_, x)| !x.is_empty()) {
            out.push_str(&format!("{}\tvoice {}\t{}\n", token_label(t.name), t.voice, text));
        }
    }
    out
}
