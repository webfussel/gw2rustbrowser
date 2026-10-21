//! Audio scripts (PF `AMSP`): how the game plays its sounds.
//!
//! A script defines "meta sounds" (a named sound event: which sound files it
//! picks from, volume, pitch, fades, looping, 3D positioning), event handlers
//! with byte code, trigger keys that fire cues at animation times, references
//! to other scripts and properties. One script (184774) also carries the
//! global audio settings, including every music playlist of the game.
//!
//! The layout follows gw2formats' `ScriptFileDataV30`. Chunk version 33 (all
//! but eight files) adds one dword after `channelMax` in each meta sound.
//! Names are 64-bit tokens (see [`super::material::token_name`]).

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const AMSP: u32 = fcc(b"AMSP");

#[derive(Clone, Debug)]
pub struct AudioScript {
    pub version: u16,
    pub music_cue: u64,
    pub reverb_override: u64,
    pub snapshot: u64,
    pub settings: Option<AudioSettings>,
    pub handlers: Vec<Handler>,
    pub sounds: Vec<MetaSound>,
    /// (name, script file id).
    pub script_refs: Vec<(u64, Option<u32>)>,
    pub triggers: Vec<TriggerKey>,
    pub properties: Vec<Property>,
    pub flags: u32,
    pub sound_pool_count: u32,
    pub fade_in: f32,
    pub sound_pool_delay: f32,
    pub volume: f32,
}

#[derive(Clone, Debug)]
pub struct Handler {
    pub name: u64,
    pub flags: u32,
    pub code: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct TriggerKey {
    pub name: u64,
    pub markers: Vec<TriggerMarker>,
}

#[derive(Clone, Debug)]
pub struct TriggerMarker {
    pub cue: u64,
    pub end: u64,
    pub time: f32,
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct Property {
    pub name: u64,
    pub token: u64,
    pub value: f32,
}

/// One candidate file of a meta sound or playlist.
#[derive(Clone, Debug)]
pub struct SoundFile {
    pub condition: u64,
    pub language: u64,
    pub volume: f32,
    pub weight: f32,
    pub file: Option<u32>,
    pub audio_type: u8,
    pub note_base: u8,
    pub note_min: u8,
    pub note_max: u8,
}

/// A value range; `min_byte` is used by integer ranges (repeat counts).
#[derive(Clone, Copy, Debug, Default)]
pub struct Range {
    pub max: f32,
    pub min: f32,
    pub min_byte: u8,
}

/// A parameter: a fixed value, optionally randomised or driven by an envelope.
#[derive(Clone, Debug)]
pub struct Param {
    pub value: f32,
    pub kind: u8,
    /// Random value range (min, max).
    pub random: Option<(f32, f32)>,
    pub envelope: Option<Envelope>,
}

#[derive(Clone, Debug)]
pub struct Envelope {
    /// Game parameter driving the envelope.
    pub input: u64,
    pub points: Vec<(f32, f32)>,
}

#[derive(Clone, Debug)]
pub struct MetaSound {
    pub name: u64,
    pub category: u64,
    pub end_cue: u64,
    pub offset_bone: u64,
    pub playlist: u64,
    pub dsp_effects: usize,
    pub attenuation: bool,
    pub files: Vec<SoundFile>,
    pub channel_fade_in: f32,
    pub channel_fade_out: f32,
    pub end_cue_offset: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub position_offset: [f32; 3],
    pub channel_max: u32,
    /// Added in version 33; meaning unknown (0 or 1).
    pub unknown: u32,
    pub flags: u32,
    pub loop_count: u32,
    pub depth: Param,
    pub pan: Param,
    pub pitch: Param,
    pub pitch_ms: Param,
    pub volume: Param,
    pub volume_ms: Param,
    pub initial_delay: Range,
    pub play_length: Range,
    pub position_offset_angle: Range,
    pub position_range: Range,
    pub repeat_count: Range,
    pub repeat_time: Range,
    pub replay_delay: Range,
    pub start_time_offset: Range,
    pub channel_mode: u8,
    pub channel_priority: u8,
    pub file_iterate_mode: u8,
    pub loop_mode: u8,
    pub music_priority: u8,
    pub playback_mode: u8,
    pub position_mode: u8,
    pub repeat_time_from: u8,
}

#[derive(Clone, Debug)]
pub struct AudioSettings {
    pub default_snapshot: u64,
    pub effects_buss: u64,
    pub distance_scale: f32,
    pub doppler_scale: f32,
    pub echo_level: f32,
    pub focus_transition: f32,
    pub memory_pool: f32,
    pub reverb_level: f32,
    pub busses: usize,
    pub categories: usize,
    pub materials: usize,
    pub music_conditions: usize,
    pub playlists: Vec<Playlist>,
    pub properties: Vec<Property>,
    pub reverbs: usize,
    pub script_refs: Vec<(u64, Option<u32>)>,
    pub snapshots: usize,
    pub bank_index: Option<u32>,
    pub bank_script: Option<u32>,
    pub music_script: Option<u32>,
    /// (name, external playlist).
    pub music_external: Vec<(u64, String)>,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub name: u64,
    pub category: u64,
    pub primary: u64,
    pub secondary: u64,
    pub files: Vec<SoundFile>,
    pub fade_in: f32,
    pub fade_out: f32,
    pub flags: u32,
    pub initial_silence: Range,
    pub interval_silence: Range,
    pub max_play_length: Range,
    pub volume: Param,
    pub file_iterate_mode: u8,
}

pub fn decode(data: &[u8]) -> Result<AudioScript> {
    let file = PackFile::parse(data)?;
    if file.file_type != AMSP {
        return Err(Error::format("audio script", "not an AMSP file"));
    }
    let chunk = file.require_chunk(AMSP)?;
    let version = chunk.version;
    if !(28..=33).contains(&version) {
        return Err(Error::Unsupported(format!("AMSP chunk version {version}")));
    }
    let mut c = chunk.cursor();
    let music_cue = c.u64()?;
    let reverb_override = c.u64()?;
    let snapshot = c.u64()?;
    let settings = c.ptr()?.map(|mut s| settings(&mut s)).transpose()?;
    let handlers = list(&mut c, |h| Ok(Handler { name: h.u64()?, flags: h.u32()?, code: h.byte_array()?.to_vec() }))?;
    let sounds = list(&mut c, |m| meta_sound(m, version))?;
    let script_refs = list(&mut c, |r| Ok((r.u64()?, r.file_ref()?)))?;
    let triggers = list(&mut c, |t| {
        let name = t.u64()?;
        let markers = list(t, |k| Ok(TriggerMarker { cue: k.u64()?, end: k.u64()?, time: k.f32()?, kind: k.u8()? }))?;
        Ok(TriggerKey { name, markers })
    })?;
    let properties = list(&mut c, property)?;
    Ok(AudioScript {
        version,
        music_cue,
        reverb_override,
        snapshot,
        settings,
        handlers,
        sounds,
        script_refs,
        triggers,
        properties,
        flags: c.u32()?,
        sound_pool_count: c.u32()?,
        fade_in: c.f32()?,
        sound_pool_delay: c.f32()?,
        volume: c.f32()?,
    })
}

/// Reads an `Array<T>` with `item` reading one element.
fn list<T>(c: &mut Cursor, mut item: impl FnMut(&mut Cursor) -> Result<T>) -> Result<Vec<T>> {
    let (count, mut at) = c.array()?;
    let mut items = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        items.push(item(&mut at)?);
    }
    Ok(items)
}

/// Number of elements of an array whose elements are not read.
fn count(c: &mut Cursor) -> Result<usize> {
    Ok(c.array()?.0)
}

fn property(c: &mut Cursor) -> Result<Property> {
    Ok(Property { name: c.u64()?, token: c.u64()?, value: c.f32()? })
}

fn range(c: &mut Cursor) -> Result<Range> {
    Ok(Range { max: c.f32()?, min: c.f32()?, min_byte: c.u8()? })
}

fn param(c: &mut Cursor) -> Result<Param> {
    let envelope = c
        .ptr()?
        .map(|mut e| {
            e.skip(4 + 8)?; // input offset and its property
            let input = e.u64()?;
            e.skip(4 + 8 + 1 + 4 + 8 + 4 + 8)?; // input scale, type, output offset/scale
            let points = list(&mut e, |p| Ok((p.f32()?, p.f32()?)))?;
            Ok::<_, Error>(Envelope { input, points })
        })
        .transpose()?;
    let random = c
        .ptr()?
        .map(|mut r| {
            range(&mut r)?; // time
            let value = range(&mut r)?;
            Ok::<_, Error>((value.min, value.max))
        })
        .transpose()?;
    Ok(Param { value: c.f32()?, kind: c.u8()?, random, envelope })
}

fn sound_file(c: &mut Cursor) -> Result<SoundFile> {
    Ok(SoundFile {
        condition: c.u64()?,
        language: c.u64()?,
        volume: c.f32()?,
        weight: c.f32()?,
        file: c.file_ref()?,
        audio_type: c.u8()?,
        note_base: c.u8()?,
        note_min: c.u8()?,
        note_max: c.u8()?,
    })
}

fn meta_sound(c: &mut Cursor, version: u16) -> Result<MetaSound> {
    let category = c.u64()?;
    let end_cue = c.u64()?;
    let name = c.u64()?;
    let offset_bone = c.u64()?;
    let playlist = c.u64()?;
    let dsp_effects = count(c)?;
    let attenuation = c.ptr()?.is_some();
    let files = list(c, sound_file)?;
    let floats = [c.f32()?, c.f32()?, c.f32()?, c.f32()?, c.f32()?];
    let position_offset = [c.f32()?, c.f32()?, c.f32()?];
    let channel_max = c.u32()?;
    let unknown = if version >= 33 { c.u32()? } else { 0 };
    let flags = c.u32()?;
    let loop_count = c.u32()?;
    let params = [param(c)?, param(c)?, param(c)?, param(c)?, param(c)?, param(c)?];
    let ranges = [range(c)?, range(c)?, range(c)?, range(c)?, range(c)?, range(c)?, range(c)?, range(c)?];
    let bytes = c.take(8)?;
    let [depth, pan, pitch, pitch_ms, volume, volume_ms] = params;
    Ok(MetaSound {
        name,
        category,
        end_cue,
        offset_bone,
        playlist,
        dsp_effects,
        attenuation,
        files,
        channel_fade_in: floats[0],
        channel_fade_out: floats[1],
        end_cue_offset: floats[2],
        fade_in: floats[3],
        fade_out: floats[4],
        position_offset,
        channel_max,
        unknown,
        flags,
        loop_count,
        depth,
        pan,
        pitch,
        pitch_ms,
        volume,
        volume_ms,
        initial_delay: ranges[0],
        play_length: ranges[1],
        position_offset_angle: ranges[2],
        position_range: ranges[3],
        repeat_count: ranges[4],
        repeat_time: ranges[5],
        replay_delay: ranges[6],
        start_time_offset: ranges[7],
        channel_mode: bytes[0],
        channel_priority: bytes[1],
        file_iterate_mode: bytes[2],
        loop_mode: bytes[3],
        music_priority: bytes[4],
        playback_mode: bytes[5],
        position_mode: bytes[6],
        repeat_time_from: bytes[7],
    })
}

fn settings(c: &mut Cursor) -> Result<AudioSettings> {
    let default_snapshot = c.u64()?;
    let effects_buss = c.u64()?;
    let floats = [c.f32()?, c.f32()?, c.f32()?, c.f32()?, c.f32()?, c.f32()?];
    c.skip(8)?; // min/max channels (low quality)
    let busses = count(c)?;
    let categories = count(c)?;
    let materials = count(c)?;
    let music_conditions = count(c)?;
    let playlists = list(c, |p| {
        let category = p.u64()?;
        let name = p.u64()?;
        let primary = p.u64()?;
        let secondary = p.u64()?;
        let files = list(p, sound_file)?;
        Ok(Playlist {
            name,
            category,
            primary,
            secondary,
            files,
            fade_in: p.f32()?,
            fade_out: p.f32()?,
            flags: p.u32()?,
            initial_silence: range(p)?,
            interval_silence: range(p)?,
            max_play_length: range(p)?,
            volume: param(p)?,
            file_iterate_mode: p.u8()?,
        })
    })?;
    let properties = list(c, property)?;
    let reverbs = count(c)?;
    let script_refs = list(c, |r| Ok((r.u64()?, r.file_ref()?)))?;
    let snapshots = count(c)?;
    let bank_index = c.file_ref()?;
    let bank_script = c.file_ref()?;
    let music_script = c.file_ref()?;
    let music_external = list(c, |m| Ok((m.u64()?, m.wstring()?)))?;
    Ok(AudioSettings {
        default_snapshot,
        effects_buss,
        distance_scale: floats[0],
        doppler_scale: floats[1],
        echo_level: floats[2],
        focus_transition: floats[3],
        memory_pool: floats[4],
        reverb_level: floats[5],
        busses,
        categories,
        materials,
        music_conditions,
        playlists,
        properties,
        reverbs,
        script_refs,
        snapshots,
        bank_index,
        bank_script,
        music_script,
        music_external,
    })
}

/// The script as readable text, for export.
pub fn to_text(script: &AudioScript) -> String {
    use super::material::token_label;
    use std::fmt::Write;
    let name = |t: u64| if t == 0 { "-".to_owned() } else { token_label(t) };
    let file = |f: Option<u32>| f.map_or("-".to_owned(), |id| id.to_string());
    let param = |p: &Param| {
        let mut text = format!("{}", p.value);
        if let Some((lo, hi)) = p.random {
            text = format!("random {lo}..{hi}");
        }
        if let Some(e) = &p.envelope {
            text.push_str(&format!(" (envelope on {} with {} points)", name(e.input), e.points.len()));
        }
        text
    };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "AMSP audio script, chunk version {}\nvolume {}, sound pool {} (delay {}), fade in {}, flags {:#x}",
        script.version, script.volume, script.sound_pool_count, script.sound_pool_delay, script.fade_in, script.flags
    );
    for s in &script.sounds {
        let _ = writeln!(
            out,
            "\nsound {} (category {}): volume {}, pitch {}, fade {}/{}, loops {}, flags {:#x}",
            name(s.name),
            name(s.category),
            param(&s.volume),
            param(&s.pitch),
            s.fade_in,
            s.fade_out,
            s.loop_count,
            s.flags
        );
        for f in &s.files {
            let _ = writeln!(out, "  file {} volume {} weight {}", file(f.file), f.volume, f.weight);
        }
    }
    for t in &script.triggers {
        let _ = writeln!(out, "\ntrigger {}", name(t.name));
        for m in &t.markers {
            let _ = writeln!(out, "  {:>8.3} s  cue {} end {} type {}", m.time, name(m.cue), name(m.end), m.kind);
        }
    }
    for h in &script.handlers {
        let _ = writeln!(out, "\nhandler {} flags {:#x}, {} bytes of byte code", name(h.name), h.flags, h.code.len());
    }
    for (n, f) in &script.script_refs {
        let _ = writeln!(out, "script reference {} -> {}", name(*n), file(*f));
    }
    for p in &script.properties {
        let _ = writeln!(out, "property {} = {} / {}", name(p.name), name(p.token), p.value);
    }
    if let Some(s) = &script.settings {
        let _ = writeln!(out, "\nglobal audio settings: {} music playlists", s.playlists.len());
        for p in &s.playlists {
            let _ = writeln!(
                out,
                "\nplaylist {} (category {}, primary {}, secondary {}), volume {}",
                name(p.name),
                name(p.category),
                name(p.primary),
                name(p.secondary),
                param(&p.volume)
            );
            for f in &p.files {
                let _ = writeln!(out, "  file {} volume {} weight {}", file(f.file), f.volume, f.weight);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::skeleton::bone_token;

    /// A 64-bit AMSP file with one meta sound named `saddle` that plays file 6.
    fn script(version: u16) -> Vec<u8> {
        let extra = if version >= 33 { 4 } else { 0 };
        let mut p = vec![0u8; 470 + extra + 6];
        let put = |p: &mut Vec<u8>, at: usize, bytes: &[u8]| p[at..at + bytes.len()].copy_from_slice(bytes);
        let pointer = |p: &mut Vec<u8>, field: usize, target: usize| {
            put(p, field, &((target - field) as u64).to_le_bytes());
        };
        // Script: meta sound array (count 1) at 44, sound pool 4, volume 0.5.
        put(&mut p, 44, &1u32.to_le_bytes());
        pointer(&mut p, 48, 112);
        put(&mut p, 96, &4u32.to_le_bytes());
        put(&mut p, 108, &0.5f32.to_le_bytes());
        // Meta sound at 112: name, one file, fade out, loop count, pitch.
        put(&mut p, 128, &bone_token("saddle").to_le_bytes());
        put(&mut p, 172, &1u32.to_le_bytes());
        let file = 434 + extra; // the meta sound ends here
        pointer(&mut p, 176, file);
        put(&mut p, 200, &0.25f32.to_le_bytes());
        put(&mut p, 224 + extra, &1u32.to_le_bytes());
        put(&mut p, 228 + extra + 2 * 21 + 16, &1.5f32.to_le_bytes());
        // File entry: volume 1, weight 2, reference to file 6.
        put(&mut p, file + 16, &1.0f32.to_le_bytes());
        put(&mut p, file + 20, &2.0f32.to_le_bytes());
        pointer(&mut p, file + 24, file + 36);
        put(&mut p, file + 36, &[0x05, 0x01, 0x00, 0x01, 0, 0]);

        let mut data = b"PF\x05\x00\x00\x00\x0c\x00AMSPAMSP".to_vec();
        data.extend_from_slice(&((p.len() + 8) as u32).to_le_bytes());
        data.extend_from_slice(&version.to_le_bytes());
        data.extend_from_slice(&[16, 0, 0, 0, 0, 0]);
        data.extend_from_slice(&p);
        data
    }

    #[test]
    fn decodes_meta_sounds_of_both_layouts() {
        for version in [30, 33] {
            let s = decode(&script(version)).unwrap();
            assert_eq!(s.version, version);
            assert_eq!((s.sound_pool_count, s.volume), (4, 0.5));
            let [sound] = &s.sounds[..] else { panic!("one sound expected") };
            assert_eq!(super::super::material::token_name(sound.name).as_deref(), Some("saddle"));
            assert_eq!((sound.fade_out, sound.loop_count, sound.pitch.value), (0.25, 1, 1.5));
            let [file] = &sound.files[..] else { panic!("one file expected") };
            assert_eq!((file.file, file.volume, file.weight), (Some(6), 1.0, 2.0));
        }
    }

    #[test]
    fn rejects_other_files() {
        let mut data = script(33);
        data[8..12].copy_from_slice(b"ABNK");
        assert!(decode(&data).is_err());
    }
}
