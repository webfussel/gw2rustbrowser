//! Compiled materials (PF `AMAT`).
//!
//! An AMAT file holds two chunks:
//! - `GRMT`: material settings and the texture slots models fill in,
//! - `BGFX`: the shaders and how they are combined. GW2's DirectX 11 renderer
//!   is built on [bgfx](https://github.com/bkaradzic/bgfx); the shaders are
//!   bgfx shader binaries wrapping DXBC bytecode, and render and sampler
//!   states use bgfx's state bits.
//!
//! Effects are grouped into techniques (one per quality level) and passes.
//! Each effect names a pixel shader and one vertex shader per vertex variant
//! (e.g. static or skinned meshes).
//!
//! Names (texture slots, effects) are 64-bit tokens: five bits per letter,
//! `a` = 1, first letter in the lowest bits.

use super::pf::{Cursor, PackFile};
use crate::error::{Error, Result};
use crate::fourcc::fcc;

pub const AMAT: u32 = fcc(b"AMAT");
const GRMT: u32 = fcc(b"GRMT");
const BGFX: u32 = fcc(b"BGFX");

#[derive(Clone, Debug)]
pub struct Material {
    pub settings: Option<Settings>,
    /// `BGFX` chunk version (1–3).
    pub version: u16,
    pub shaders: Vec<Shader>,
    /// bgfx sampler flags per sampler slot.
    pub samplers: Vec<u32>,
    pub techniques: Vec<Technique>,
}

/// `AmatGrV6`.
#[derive(Clone, Debug)]
pub struct Settings {
    pub texture_array_range: u8,
    pub texture_count: u8,
    pub sort_order: u8,
    pub sort_triangles: u8,
    pub procedural_animation: u8,
    pub debug_flags: u32,
    pub flags: u32,
    /// Texture slot names (tokens), in slot order.
    pub textures: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct Shader {
    /// Stage bits: 1 vertex, 16 pixel shader.
    pub flags: u32,
    /// The bgfx shader binary for DirectX.
    pub binary: Vec<u8>,
    /// Constant tokens the material binds.
    pub constants: Vec<u32>,
    pub samplers: Vec<ShaderSampler>,
}

#[derive(Clone, Debug)]
pub struct ShaderSampler {
    pub token: u64,
    pub state_index: u32,
    pub texture_index: u32,
    pub texture_slot: u32,
}

#[derive(Clone, Debug)]
pub struct Technique {
    /// Identifies the technique (one per quality setting); meaning unknown.
    pub quality: u32,
    pub passes: Vec<Vec<Effect>>,
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub token: u64,
    /// bgfx render state bits (blend, depth test, culling, alpha reference).
    pub render_state: u64,
    pub pass_flags: u32,
    pub pixel_shader: u32,
    /// (vertex variant, vertex shader index).
    pub vertex_shaders: Vec<(u32, u32)>,
}

pub fn decode(data: &[u8]) -> Result<Material> {
    let file = PackFile::parse(data)?;
    if file.file_type != AMAT {
        return Err(Error::format("material", "not an AMAT file"));
    }
    let settings = match file.chunk(GRMT) {
        Some(chunk) if chunk.version == 6 => Some(settings(chunk.cursor())?),
        _ => None,
    };
    let chunk = file.require_chunk(BGFX)?;
    if !(1..=3).contains(&chunk.version) {
        return Err(Error::Unsupported(format!("BGFX chunk version {}", chunk.version)));
    }
    let mut c = chunk.cursor();
    let v = chunk.version;
    let (count, mut item) = c.array()?;
    let mut shaders = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        shaders.push(shader(&mut item, v)?);
    }
    let (count, mut item) = c.array()?;
    let samplers = (0..count).map(|_| item.u32()).collect::<Result<_>>()?;
    let (count, mut item) = c.array()?;
    let mut techniques = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let quality = item.u32()?;
        let (passes, mut pass) = item.array()?;
        let mut list = Vec::with_capacity(passes.min(4096));
        for _ in 0..passes {
            let (count, mut e) = pass.array()?;
            list.push((0..count).map(|_| effect(&mut e)).collect::<Result<_>>()?);
        }
        techniques.push(Technique { quality, passes: list });
    }
    Ok(Material { settings, version: v, shaders, samplers, techniques })
}

fn settings(mut c: Cursor) -> Result<Settings> {
    let texture_array_range = c.u8()?;
    let texture_count = c.u8()?;
    let sort_order = c.u8()?;
    let sort_triangles = c.u8()?;
    let procedural_animation = c.u8()?;
    let debug_flags = c.u32()?;
    let flags = c.u32()?;
    let (count, mut item) = c.array()?;
    let textures = (0..count).map(|_| item.u64()).collect::<Result<_>>()?;
    Ok(Settings {
        texture_array_range,
        texture_count,
        sort_order,
        sort_triangles,
        procedural_animation,
        debug_flags,
        flags,
        textures,
    })
}

/// `AmatGfxShaderV1`: a flag, then the DirectX and the (unused) OS X binary.
/// From version 3 on each binary has a fourth array, empty in every file.
fn shader(c: &mut Cursor, version: u16) -> Result<Shader> {
    let flags = c.u32()?;
    let binary = c.byte_array()?.to_vec();
    let (count, mut item) = c.array()?;
    let constants = (0..count).map(|_| item.u32()).collect::<Result<_>>()?;
    let (count, mut item) = c.array()?;
    let mut samplers = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        samplers.push(ShaderSampler {
            token: item.u64()?,
            state_index: item.u32()?,
            texture_index: item.u32()?,
            texture_slot: item.u32()?,
        });
    }
    let arrays = if version >= 3 { 4 } else { 3 };
    if version >= 3 {
        c.array()?;
    }
    // The OS X binary.
    for _ in 0..arrays {
        c.array()?;
    }
    Ok(Shader { flags, binary, constants, samplers })
}

fn effect(c: &mut Cursor) -> Result<Effect> {
    let token = c.u64()?;
    let render_state = c.u64()?;
    let pass_flags = c.u32()?;
    let pixel_shader = c.u32()?;
    let (count, mut item) = c.array()?;
    let vertex_shaders = (0..count).map(|_| Ok((item.u32()?, item.u32()?))).collect::<Result<_>>()?;
    Ok(Effect { token, render_state, pass_flags, pixel_shader, vertex_shaders })
}

/// Name of a 64-bit token: letters (`_` for any other character) and a
/// trailing number in the top four bits, as bone names have.
pub fn token_name(token: u64) -> Option<String> {
    if token == 0 {
        return None;
    }
    let mut name = String::new();
    let mut letters = token & ((1 << 60) - 1);
    while letters != 0 {
        // Characters other than letters are stored as 0 (`mod2x`).
        name.push(match letters & 31 {
            v @ 1..=26 => (b'a' + v as u8 - 1) as char,
            0 => '_',
            _ => return None,
        });
        letters >>= 5;
    }
    if name.is_empty() {
        return None;
    }
    let number = token >> 60;
    if number != 0 {
        name.push_str(&format!("{number:02}"));
    }
    Some(name)
}

const BLEND_FACTORS: [&str; 14] = [
    "-",
    "zero",
    "one",
    "src_color",
    "inv_src_color",
    "src_alpha",
    "inv_src_alpha",
    "dst_alpha",
    "inv_dst_alpha",
    "dst_color",
    "inv_dst_color",
    "src_alpha_sat",
    "factor",
    "inv_factor",
];
const BLEND_EQUATIONS: [&str; 5] = ["add", "sub", "revsub", "min", "max"];
const COMPARISONS: [&str; 9] = ["-", "less", "lequal", "equal", "gequal", "greater", "notequal", "never", "always"];

fn factor(value: u64) -> &'static str {
    BLEND_FACTORS.get(value as usize).copied().unwrap_or("?")
}

/// Readable form of bgfx render state bits; empty for state 0.
pub fn render_state_text(state: u64) -> Vec<String> {
    let mut parts = Vec::new();
    let write: String = [(0, 'R'), (1, 'G'), (2, 'B'), (3, 'A')]
        .iter()
        .filter(|(bit, _)| state & (1 << bit) != 0)
        .map(|(_, c)| *c)
        .chain((state & (1 << 38) != 0).then_some('Z'))
        .collect();
    if !write.is_empty() {
        parts.push(format!("write {write}"));
    }
    let depth = (state >> 4) & 0xf;
    if depth != 0 {
        parts.push(format!("depth test {}", COMPARISONS.get(depth as usize).copied().unwrap_or("?")));
    }
    let blend = (state >> 12) & 0xffff;
    if blend != 0 {
        let (src, dst, src_a, dst_a) = (blend & 0xf, (blend >> 4) & 0xf, (blend >> 8) & 0xf, (blend >> 12) & 0xf);
        let mut text = format!("blend {} × src + {} × dst", factor(src), factor(dst));
        if (src_a, dst_a) != (src, dst) {
            text.push_str(&format!(", alpha {} / {}", factor(src_a), factor(dst_a)));
        }
        parts.push(text);
    }
    let equation = (state >> 28) & 0x3f;
    if equation != 0 {
        let (rgb, alpha) = (equation & 7, equation >> 3);
        let name = |e: u64| BLEND_EQUATIONS.get(e as usize).copied().unwrap_or("?");
        parts.push(if rgb == alpha {
            format!("equation {}", name(rgb))
        } else {
            format!("equation {} / alpha {}", name(rgb), name(alpha))
        });
    }
    if state & (1 << 34) != 0 {
        parts.push("independent blend".into());
    }
    if state & (1 << 35) != 0 {
        parts.push("alpha to coverage".into());
    }
    match (state >> 36) & 3 {
        1 => parts.push("cull cw".into()),
        2 => parts.push("cull ccw".into()),
        _ => {}
    }
    let alpha_ref = (state >> 40) & 0xff;
    if alpha_ref != 0 {
        parts.push(format!("alpha ref {alpha_ref}"));
    }
    let primitive = (state >> 48) & 7;
    if primitive != 0 {
        let names = ["", "tristrip", "lines", "linestrip", "points"];
        parts.push(names.get(primitive as usize).copied().unwrap_or("primitive ?").into());
    }
    if state & (1 << 56) != 0 {
        parts.push("msaa".into());
    }
    let unknown = state & !0x0107_00ff_ffff_f0ff_u64 & !(0xffu64 << 40) & !(7u64 << 48);
    if unknown != 0 {
        parts.push(format!("other bits {unknown:#x}"));
    }
    parts
}

/// Readable form of bgfx sampler flags.
pub fn sampler_text(flags: u32) -> String {
    let address = |shift: u32| ["wrap", "mirror", "clamp", "border"][((flags >> shift) & 3) as usize];
    let filter = |shift: u32| ["linear", "point", "anisotropic", "?"][((flags >> shift) & 3) as usize];
    let mut text = format!(
        "u {} · v {} · w {} · min {} · mag {} · mip {}",
        address(0),
        address(2),
        address(4),
        filter(6),
        filter(8),
        if flags & (1 << 10) != 0 { "point" } else { "linear" }
    );
    let compare = (flags >> 16) & 0xf;
    if compare != 0 {
        text.push_str(&format!(" · compare {}", COMPARISONS.get(compare as usize).copied().unwrap_or("?")));
    }
    let rest = flags & !0x000f_07ff;
    if rest != 0 {
        text.push_str(&format!(" · other bits {rest:#x}"));
    }
    text
}

/// Name of a token for display: its letters, or hex if it has none.
pub fn token_label(token: u64) -> String {
    token_name(token).unwrap_or_else(|| format!("{token:#x}"))
}

/// Shader stage of a record from its flags.
pub fn stage_name(shader: &Shader) -> &'static str {
    match shader.binary.get(..3) {
        Some(b"VSH") => "vertex",
        Some(b"FSH") => "pixel",
        Some(b"CSH") => "compute",
        _ => "unknown",
    }
}

/// Annotation for a disassembled instruction: the uniforms it reads.
pub fn annotate(instruction: &super::shader::Instruction, uniforms: &[super::shader::Uniform]) -> Option<String> {
    let mut names: Vec<&str> = Vec::new();
    for &(buffer, register) in &instruction.constants {
        if buffer != 0 {
            continue;
        }
        for u in uniforms.iter().filter(|u| u.kind != 0) {
            let first = u.register as u32 / 16;
            if (first..first + u.register_count.max(1) as u32).contains(&register) && !names.contains(&u.name.as_str())
            {
                names.push(&u.name);
            }
        }
    }
    // Texture and sampler registers.
    for word in instruction.text.split([' ', ',']) {
        let Some(rest) = word.strip_prefix('t').or_else(|| word.strip_prefix('s')) else { continue };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || !rest[digits.len()..].is_empty() && !rest[digits.len()..].starts_with('.') {
            continue;
        }
        let stage: u16 = digits.parse().unwrap_or(u16::MAX);
        for u in uniforms.iter().filter(|u| u.kind == 0 && u.register == stage) {
            if !names.contains(&u.name.as_str()) {
                names.push(&u.name);
            }
        }
    }
    (!names.is_empty()).then(|| names.join(", "))
}

/// The whole material as text: settings, techniques, samplers and every
/// shader with its disassembly.
pub fn to_text(material: &Material) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "AMAT material, BGFX chunk version {}", material.version);
    if let Some(s) = &material.settings {
        let slots: Vec<String> = s.textures.iter().map(|&t| token_label(t)).collect();
        let _ = writeln!(
            out,
            "texture slots: {}\nsort order {}, flags {:#x}, debug flags {:#x}",
            slots.join(", "),
            s.sort_order,
            s.flags,
            s.debug_flags
        );
    }
    for (i, t) in material.techniques.iter().enumerate() {
        let _ = writeln!(out, "\ntechnique {i} ({:#010x})", t.quality);
        for (p, effects) in t.passes.iter().enumerate() {
            let _ = writeln!(out, "  pass {p}");
            for e in effects {
                let vs: Vec<String> = e.vertex_shaders.iter().map(|(v, s)| format!("{v}:{s}")).collect();
                let state = render_state_text(e.render_state);
                let _ = writeln!(
                    out,
                    "    {:<14} pixel shader {:>3}, vertex shaders [{}]{}",
                    token_label(e.token),
                    e.pixel_shader,
                    vs.join(" "),
                    if state.is_empty() { String::new() } else { format!(", {}", state.join(", ")) }
                );
            }
        }
    }
    let _ = writeln!(out, "\nsamplers");
    for (i, &s) in material.samplers.iter().enumerate() {
        let _ = writeln!(out, "  {i}: {}", sampler_text(s));
    }
    for (i, s) in material.shaders.iter().enumerate() {
        let _ = writeln!(out, "\n==== shader {i}: {} ====", stage_name(s));
        let Ok(bgfx) = super::shader::bgfx(&s.binary) else {
            let _ = writeln!(out, "(not a bgfx shader)");
            continue;
        };
        for u in &bgfx.uniforms {
            let _ = writeln!(out, "// {} {} @ {}", u.type_name(), u.name, u.register);
        }
        match super::shader::dxbc(bgfx.code).ok().and_then(|d| d.disassemble()) {
            Some(Ok(program)) => {
                let _ = writeln!(out, "{}", program.profile);
                for instruction in &program.instructions {
                    let line = format!("{}{}", "  ".repeat(instruction.depth), instruction.text);
                    match annotate(instruction, &bgfx.uniforms) {
                        Some(note) => {
                            let _ = writeln!(out, "{line:<60} // {note}");
                        }
                        None => {
                            let _ = writeln!(out, "{line}");
                        }
                    }
                }
            }
            Some(Err(e)) => {
                let _ = writeln!(out, "(disassembly failed: {e})");
            }
            None => {
                let _ = writeln!(out, "(no program)");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_names() {
        assert_eq!(token_name(0x836a2).as_deref(), Some("bump"));
        assert_eq!(token_name(0x2cc22).as_deref(), Some("base"));
        assert_eq!(token_name(0x1081).as_deref(), Some("add"));
        assert_eq!(token_name(0x2000_0000_0057_2613).as_deref(), Some("spine02"));
        assert_eq!(token_name(0x18011ed).as_deref(), Some("mod_x"));
        assert_eq!(token_name(0x1f), None);
    }

    #[test]
    fn states() {
        assert_eq!(
            render_state_text(0x0000_0100_0656_5000),
            ["blend src_alpha × src + inv_src_alpha × dst", "alpha ref 1"]
        );
        assert_eq!(render_state_text(0x1_2222_2000), ["blend one × src + one × dst", "equation revsub"]);
        assert!(render_state_text(0).is_empty());
        assert_eq!(sampler_text(0x280), "u wrap · v wrap · w wrap · min anisotropic · mag anisotropic · mip linear");
        assert_eq!(sampler_text(0x54a), "u clamp · v clamp · w wrap · min point · mag point · mip point");
    }
}
