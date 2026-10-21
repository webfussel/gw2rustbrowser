//! Shaders inside materials: bgfx shader binaries and the DXBC bytecode they
//! wrap, with a disassembler for shader model 4/5 programs.

use crate::error::{Error, Result};

fn bad(reason: impl Into<String>) -> Error {
    Error::format("shader", reason)
}

/// Byte reader for the bgfx container.
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or_else(|| bad("truncated"))?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Clone, Debug)]
pub struct Uniform {
    pub name: String,
    /// bgfx uniform type without flags: 0 sampler, 2 vec4, 3 mat3, 4 mat4.
    pub kind: u8,
    pub fragment: bool,
    pub count: u8,
    /// Byte offset in the constant buffer, or the texture stage of a sampler.
    pub register: u16,
    /// Number of vec4 registers.
    pub register_count: u16,
}

impl Uniform {
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            0 => "sampler",
            2 => "vec4",
            3 => "mat3",
            4 => "mat4",
            _ => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BgfxShader<'a> {
    pub stage: Stage,
    pub version: u8,
    pub hash_in: u32,
    pub hash_out: u32,
    pub uniforms: Vec<Uniform>,
    pub code: &'a [u8],
    /// bgfx attribute ids of the vertex inputs.
    pub attributes: Vec<u16>,
    pub constant_buffer_size: u16,
}

/// Parses a bgfx shader binary (`VSH`/`FSH`/`CSH` + version).
pub fn bgfx(data: &[u8]) -> Result<BgfxShader<'_>> {
    let mut b = Bytes { data, pos: 0 };
    let magic = b.take(4)?;
    let stage = match &magic[..3] {
        b"VSH" => Stage::Vertex,
        b"FSH" => Stage::Fragment,
        b"CSH" => Stage::Compute,
        _ => return Err(bad("not a bgfx shader")),
    };
    let version = magic[3];
    let hash_in = b.u32()?;
    let hash_out = if stage != Stage::Compute && version >= 6 { b.u32()? } else { 0 };
    let count = b.u16()?;
    let mut uniforms = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let length = b.u8()? as usize;
        let name = String::from_utf8_lossy(b.take(length)?).into_owned();
        let kind = b.u8()?;
        let count = b.u8()?;
        let register = b.u16()?;
        let register_count = b.u16()?;
        if version >= 8 {
            b.take(2)?; // texture component and dimension
        }
        if version >= 10 {
            b.take(2)?; // texture format
        }
        uniforms.push(Uniform { name, kind: kind & 0x0f, fragment: kind & 0x10 != 0, count, register, register_count });
    }
    let size = b.u32()? as usize;
    let code = b.take(size)?;
    b.take(1)?; // terminator
    let mut attributes = Vec::new();
    let mut constant_buffer_size = 0;
    if b.pos < data.len() {
        let n = b.u8()?;
        for _ in 0..n {
            attributes.push(b.u16()?);
        }
        constant_buffer_size = b.u16()?;
    }
    Ok(BgfxShader { stage, version, hash_in, hash_out, uniforms, code, attributes, constant_buffer_size })
}

/// bgfx's name of a vertex attribute id.
pub fn attribute_name(id: u16) -> String {
    match id {
        0x0001 => "Position".into(),
        0x0002 => "Normal".into(),
        0x0003 => "Tangent".into(),
        0x0004 => "Bitangent".into(),
        0x0005 => "Color0".into(),
        0x0006 => "Color1".into(),
        0x0018 => "Color2".into(),
        0x0019 => "Color3".into(),
        0x000e => "Indices".into(),
        0x000f => "Weight".into(),
        0x0010..=0x0017 => format!("TexCoord{}", id - 0x10),
        _ => format!("{id:#06x}"),
    }
}

/// A parsed DXBC container.
#[derive(Clone, Debug)]
pub struct Dxbc<'a> {
    pub chunks: Vec<([u8; 4], &'a [u8])>,
}

pub fn dxbc(data: &[u8]) -> Result<Dxbc<'_>> {
    if data.len() < 32 || &data[..4] != b"DXBC" {
        return Err(bad("not DXBC bytecode"));
    }
    let u32_at = |at: usize| -> Result<u32> {
        data.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("truncated DXBC"))
    };
    let count = u32_at(28)? as usize;
    let mut chunks = Vec::with_capacity(count.min(64));
    for i in 0..count.min(64) {
        let offset = u32_at(32 + i * 4)? as usize;
        let size = u32_at(offset + 4)? as usize;
        let body = data.get(offset + 8..offset + 8 + size).ok_or_else(|| bad("DXBC chunk outside the data"))?;
        chunks.push((data[offset..offset + 4].try_into().unwrap(), body));
    }
    Ok(Dxbc { chunks })
}

impl<'a> Dxbc<'a> {
    pub fn chunk(&self, names: &[&[u8; 4]]) -> Option<(&[u8; 4], &'a [u8])> {
        self.chunks.iter().find(|(n, _)| names.contains(&n)).map(|(n, b)| (n, *b))
    }

    pub fn inputs(&self) -> Result<Vec<SignatureElement>> {
        self.chunk(&[b"ISGN", b"ISG1"]).map_or(Ok(Vec::new()), |(n, b)| signature(n, b))
    }

    pub fn outputs(&self) -> Result<Vec<SignatureElement>> {
        self.chunk(&[b"OSGN", b"OSG5", b"OSG1"]).map_or(Ok(Vec::new()), |(n, b)| signature(n, b))
    }

    /// The shader program as text, or `None` if the container has none.
    pub fn disassemble(&self) -> Option<Result<Program>> {
        self.chunk(&[b"SHEX", b"SHDR"]).map(|(_, b)| disassemble(b))
    }
}

#[derive(Clone, Debug)]
pub struct SignatureElement {
    pub semantic: String,
    pub index: u32,
    pub system_value: u32,
    /// 1 uint, 2 int, 3 float.
    pub component_type: u32,
    pub register: u32,
    pub mask: u8,
    pub used: u8,
}

fn signature(name: &[u8; 4], data: &[u8]) -> Result<Vec<SignatureElement>> {
    let u32_at = |at: usize| -> Result<u32> {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| bad("truncated signature"))
    };
    let (stride, lead) = match name {
        b"OSG5" => (28, 4),
        b"ISG1" | b"OSG1" => (32, 4),
        _ => (24, 0),
    };
    let count = u32_at(0)? as usize;
    let start = u32_at(4)? as usize;
    let mut elements = Vec::with_capacity(count.min(64));
    for i in 0..count.min(64) {
        let at = start + i * stride + lead;
        let name_at = u32_at(at)? as usize;
        let rest = data.get(name_at..).ok_or_else(|| bad("signature name outside the chunk"))?;
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        elements.push(SignatureElement {
            semantic: String::from_utf8_lossy(&rest[..end]).into_owned(),
            index: u32_at(at + 4)?,
            system_value: u32_at(at + 8)?,
            component_type: u32_at(at + 12)?,
            register: u32_at(at + 16)?,
            mask: *data.get(at + 20).ok_or_else(|| bad("truncated signature"))?,
            used: *data.get(at + 21).ok_or_else(|| bad("truncated signature"))?,
        });
    }
    Ok(elements)
}

/// A disassembled program.
#[derive(Clone, Debug)]
pub struct Program {
    /// For example `vs_5_0`.
    pub profile: String,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug)]
pub struct Instruction {
    pub text: String,
    /// Constant buffer registers read (buffer, register), for annotations.
    pub constants: Vec<(u32, u32)>,
    /// Nesting depth for indentation.
    pub depth: usize,
}

const OPCODES: &[&str] = &[
    "add",
    "and",
    "break",
    "breakc",
    "call",
    "callc",
    "case",
    "continue",
    "continuec",
    "cut",
    "default",
    "deriv_rtx",
    "deriv_rty",
    "discard",
    "div",
    "dp2",
    "dp3",
    "dp4",
    "else",
    "emit",
    "emitthencut",
    "endif",
    "endloop",
    "endswitch",
    "eq",
    "exp",
    "frc",
    "ftoi",
    "ftou",
    "ge",
    "iadd",
    "if",
    "ieq",
    "ige",
    "ilt",
    "imad",
    "imax",
    "imin",
    "imul",
    "ine",
    "ineg",
    "ishl",
    "ishr",
    "itof",
    "label",
    "ld",
    "ld_ms",
    "log",
    "loop",
    "lt",
    "mad",
    "min",
    "max",
    "customdata",
    "mov",
    "movc",
    "mul",
    "ne",
    "nop",
    "not",
    "or",
    "resinfo",
    "ret",
    "retc",
    "round_ne",
    "round_ni",
    "round_pi",
    "round_z",
    "rsq",
    "sample",
    "sample_c",
    "sample_c_lz",
    "sample_l",
    "sample_d",
    "sample_b",
    "sqrt",
    "switch",
    "sincos",
    "udiv",
    "ult",
    "uge",
    "umul",
    "umad",
    "umax",
    "umin",
    "ushr",
    "utof",
    "xor",
    "dcl_resource",
    "dcl_constantbuffer",
    "dcl_sampler",
    "dcl_indexrange",
    "dcl_outputtopology",
    "dcl_inputprimitive",
    "dcl_maxout",
    "dcl_input",
    "dcl_input_sgv",
    "dcl_input_siv",
    "dcl_input_ps",
    "dcl_input_ps_sgv",
    "dcl_input_ps_siv",
    "dcl_output",
    "dcl_output_sgv",
    "dcl_output_siv",
    "dcl_temps",
    "dcl_indexableTemp",
    "dcl_globalFlags",
    "reserved",
    "lod",
    "gather4",
    "sample_pos",
    "sample_info",
    "reserved",
    "hs_decls",
    "hs_control_point_phase",
    "hs_fork_phase",
    "hs_join_phase",
    "emit_stream",
    "cut_stream",
    "emitthencut_stream",
    "interface_call",
    "bufinfo",
    "deriv_rtx_coarse",
    "deriv_rtx_fine",
    "deriv_rty_coarse",
    "deriv_rty_fine",
    "gather4_c",
    "gather4_po",
    "gather4_po_c",
    "rcp",
    "f32tof16",
    "f16tof32",
    "uaddc",
    "usubb",
    "countbits",
    "firstbit_hi",
    "firstbit_lo",
    "firstbit_shi",
    "ubfe",
    "ibfe",
    "bfi",
    "bfrev",
    "swapc",
    "dcl_stream",
    "dcl_function_body",
    "dcl_function_table",
    "dcl_interface",
    "dcl_input_control_point_count",
    "dcl_output_control_point_count",
    "dcl_tessellator_domain",
    "dcl_tessellator_partitioning",
    "dcl_tessellator_output_primitive",
    "dcl_hs_max_tessfactor",
    "dcl_hs_fork_phase_instance_count",
    "dcl_hs_join_phase_instance_count",
    "dcl_thread_group",
    "dcl_uav_typed",
    "dcl_uav_raw",
    "dcl_uav_structured",
    "dcl_tgsm_raw",
    "dcl_tgsm_structured",
    "dcl_resource_raw",
    "dcl_resource_structured",
    "ld_uav_typed",
    "store_uav_typed",
    "ld_raw",
    "store_raw",
    "ld_structured",
    "store_structured",
    "atomic_and",
    "atomic_or",
    "atomic_xor",
    "atomic_cmp_store",
    "atomic_iadd",
    "atomic_imax",
    "atomic_imin",
    "atomic_umax",
    "atomic_umin",
    "imm_atomic_alloc",
    "imm_atomic_consume",
    "imm_atomic_iadd",
    "imm_atomic_and",
    "imm_atomic_or",
    "imm_atomic_xor",
    "imm_atomic_exch",
    "imm_atomic_cmp_exch",
    "imm_atomic_imax",
    "imm_atomic_imin",
    "imm_atomic_umax",
    "imm_atomic_umin",
    "sync",
    "dadd",
    "dmax",
    "dmin",
    "dmul",
    "deq",
    "dge",
    "dlt",
    "dne",
    "dmov",
    "dmovc",
    "dtof",
    "ftod",
    "eval_snapped",
    "eval_sample_index",
    "eval_centroid",
    "dcl_gsinstances",
    "abort",
    "debug_break",
    "reserved",
    "ddiv",
    "dfma",
    "drcp",
    "msad",
    "dtoi",
    "dtou",
    "itod",
    "utod",
];

const RESOURCE_DIMENSIONS: &[&str] = &[
    "unknown",
    "buffer",
    "texture1d",
    "texture2d",
    "texture2dms",
    "texture3d",
    "texturecube",
    "texture1darray",
    "texture2darray",
    "texture2dmsarray",
    "texturecubearray",
    "raw_buffer",
    "structured_buffer",
];

const RETURN_TYPES: &[&str] =
    &["unknown", "unorm", "snorm", "sint", "uint", "float", "mixed", "double", "continued", "unused"];

const SYSTEM_VALUES: &[&str] = &[
    "undefined",
    "position",
    "clip_distance",
    "cull_distance",
    "rendertarget_array_index",
    "viewport_array_index",
    "vertex_id",
    "primitive_id",
    "instance_id",
    "is_front_face",
    "sampleIndex",
];

const INTERPOLATION: &[&str] = &[
    "undefined",
    "constant",
    "linear",
    "linear centroid",
    "linear noperspective",
    "linear noperspective centroid",
    "linear sample",
    "linear noperspective sample",
];

const COMPONENTS: [char; 4] = ['x', 'y', 'z', 'w'];

fn lookup(table: &[&str], index: u32) -> String {
    table.get(index as usize).map_or_else(|| format!("unknown({index})"), |s| (*s).to_owned())
}

/// A 32-bit immediate: normal floats as floats (very small or large ones in
/// scientific notation), anything else as an integer.
fn immediate(bits: u32) -> String {
    let f = f32::from_bits(bits);
    let exponent = (bits >> 23) & 0xff;
    if bits == 0 {
        "0".into()
    } else if (1..0xff).contains(&exponent) {
        if (1e-4..1e9).contains(&f.abs()) { format!("{f:.6}") } else { format!("{f:e}") }
    } else if (bits as i32) < 0 && (bits as i32) > -65536 {
        (bits as i32).to_string()
    } else if bits < 0x10000 {
        bits.to_string()
    } else {
        format!("0x{bits:08x}")
    }
}

struct Tokens<'a> {
    tokens: &'a [u32],
    pos: usize,
    end: usize,
}

impl Tokens<'_> {
    fn next(&mut self) -> Result<u32> {
        if self.pos >= self.end {
            return Err(bad("instruction ends inside an operand"));
        }
        self.pos += 1;
        Ok(self.tokens[self.pos - 1])
    }

    fn operand(&mut self, constants: &mut Vec<(u32, u32)>) -> Result<String> {
        let token = self.next()?;
        let components = token & 3;
        let selection = (token >> 2) & 3;
        let kind = (token >> 12) & 0xff;
        let dimensions = (token >> 20) & 3;
        let mut modifier = 0;
        let mut extended = token >> 31 != 0;
        while extended {
            let ext = self.next()?;
            if ext & 0x3f == 1 {
                modifier = (ext >> 6) & 0xff;
            }
            extended = ext >> 31 != 0;
        }

        let mut text = String::new();
        if kind == 4 || kind == 5 {
            // Immediate values.
            let n = match components {
                1 => 1,
                2 => 4,
                _ => 0,
            };
            let mut values = Vec::with_capacity(n);
            for _ in 0..n {
                if kind == 4 {
                    values.push(immediate(self.next()?));
                } else {
                    let low = self.next()? as u64;
                    let high = self.next()? as u64;
                    values.push(format!("{:.6}", f64::from_bits(low | high << 32)));
                }
            }
            text = format!("{}({})", if kind == 4 { "l" } else { "d" }, values.join(", "));
        } else {
            let prefix = match kind {
                0 => "r",
                1 => "v",
                2 => "o",
                3 => "x",
                6 => "s",
                7 => "t",
                8 => "cb",
                9 => "icb",
                10 => "label",
                11 => "vPrim",
                12 => "oDepth",
                13 => "null",
                14 => "rasterizer",
                15 => "oMask",
                16 => "m",
                17 => "fb",
                18 => "ft",
                19 => "fp",
                20 => "fi",
                21 => "fo",
                22 => "vOutputControlPointID",
                23 => "vForkInstanceID",
                24 => "vJoinInstanceID",
                25 => "vicp",
                26 => "vocp",
                27 => "vpc",
                28 => "vDomain",
                29 => "this",
                30 => "u",
                31 => "g",
                32 => "vThreadID",
                33 => "vThreadGroupID",
                34 => "vThreadIDInGroup",
                35 => "vCoverage",
                36 => "vThreadIDInGroupFlattened",
                37 => "vGSInstanceID",
                38 => "oDepthGE",
                39 => "oDepthLE",
                40 => "vCycleCounter",
                41 => "oStencilRef",
                42 => "vInnerCoverage",
                _ => "?",
            };
            text.push_str(prefix);
            let mut indices = Vec::new();
            for d in 0..dimensions {
                let representation = (token >> (22 + 3 * d)) & 7;
                let (value, relative) = match representation {
                    0 => (Some(self.next()? as u64), None),
                    1 => {
                        let high = self.next()? as u64;
                        (Some(high << 32 | self.next()? as u64), None)
                    }
                    2 => (None, Some(self.operand(constants)?)),
                    3 => (Some(self.next()? as u64), Some(self.operand(constants)?)),
                    4 => {
                        let high = self.next()? as u64;
                        let value = high << 32 | self.next()? as u64;
                        (Some(value), Some(self.operand(constants)?))
                    }
                    _ => return Err(bad(format!("index representation {representation}"))),
                };
                indices.push((value, relative));
            }
            // Register numbers are written directly after the prefix, further
            // indices in brackets; relative indices always in brackets.
            let direct = matches!(kind, 0..=3 | 6..=8 | 16..=21 | 30 | 31) && dimensions <= 3;
            for (i, (value, relative)) in indices.iter().enumerate() {
                let index = match (value, relative) {
                    (Some(v), None) => v.to_string(),
                    (None, Some(r)) => r.clone(),
                    (Some(v), Some(r)) => format!("{r} + {v}"),
                    (None, None) => unreachable!(),
                };
                if i == 0 && direct && relative.is_none() {
                    text.push_str(&index);
                } else {
                    text.push_str(&format!("[{index}]"));
                }
            }
            if kind == 8
                && indices.len() == 2
                && let ((Some(buffer), _), (Some(register), None)) = (&indices[0], &indices[1])
            {
                constants.push((*buffer as u32, *register as u32));
            }
            if components == 2 {
                match selection {
                    0 => {
                        let mask = (token >> 4) & 0xf;
                        if mask != 0 {
                            text.push('.');
                            text.extend((0..4).filter(|i| mask & (1 << i) != 0).map(|i| COMPONENTS[i]));
                        }
                    }
                    1 => {
                        text.push('.');
                        for i in 0..4 {
                            text.push(COMPONENTS[((token >> (4 + 2 * i)) & 3) as usize]);
                        }
                    }
                    2 => {
                        text.push('.');
                        text.push(COMPONENTS[((token >> 4) & 3) as usize]);
                    }
                    _ => {}
                }
            }
        }
        Ok(match modifier {
            1 => format!("-{text}"),
            2 => format!("|{text}|"),
            3 => format!("-|{text}|"),
            _ => text,
        })
    }
}

fn return_type(token: u32) -> String {
    let parts: Vec<String> = (0..4).map(|i| lookup(RETURN_TYPES, (token >> (4 * i)) & 0xf)).collect();
    format!("({})", parts.join(","))
}

/// Disassembles a `SHDR`/`SHEX` chunk.
pub fn disassemble(chunk: &[u8]) -> Result<Program> {
    let tokens: Vec<u32> = chunk.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b)).collect();
    if tokens.len() < 2 {
        return Err(bad("empty program"));
    }
    let version = tokens[0];
    let stage = match version >> 16 {
        0 => "ps",
        1 => "vs",
        2 => "gs",
        3 => "hs",
        4 => "ds",
        5 => "cs",
        _ => "unknown",
    };
    let profile = format!("{stage}_{}_{}", (version >> 4) & 0xf, version & 0xf);
    let length = (tokens[1] as usize).min(tokens.len());
    let mut instructions = Vec::new();
    let mut pos = 2;
    let mut depth = 0usize;
    while pos < length {
        let token = tokens[pos];
        let opcode = token & 0x7ff;
        let size = if opcode == 53 {
            *tokens.get(pos + 1).ok_or_else(|| bad("truncated customdata"))? as usize
        } else {
            ((token >> 24) & 0x7f) as usize
        };
        if size == 0 || pos + size > length {
            return Err(bad(format!("bad instruction length at token {pos}")));
        }
        let mut t = Tokens { tokens: &tokens, pos: pos + 1, end: pos + size };
        let mut constants = Vec::new();
        let name = OPCODES.get(opcode as usize).copied().unwrap_or("unknown");

        // Control flow nesting for indentation.
        let closes = matches!(opcode, 18 | 21 | 22 | 23 | 6 | 10);
        if closes {
            depth = depth.saturating_sub(1);
        }
        let line_depth = depth;
        if matches!(opcode, 18 | 31 | 48 | 76 | 6 | 10) {
            depth += 1;
        }

        let text = match opcode {
            53 => {
                let class = token >> 11;
                if class == 3 {
                    let values: Vec<String> = tokens[pos + 2..pos + size]
                        .chunks(4)
                        .map(|v| format!("{{ {} }}", v.iter().map(|&b| immediate(b)).collect::<Vec<_>>().join(", ")))
                        .collect();
                    format!("dcl_immediateConstantBuffer {{ {} }}", values.join(",\n                              "))
                } else {
                    format!("customdata class {class}, {} tokens", size - 2)
                }
            }
            106 => {
                let flags = (token >> 11) & 0x1fff;
                let names = [
                    "refactoringAllowed",
                    "enableDoublePrecisionFloatOps",
                    "forceEarlyDepthStencil",
                    "enableRawAndStructuredBuffers",
                    "skipOptimization",
                    "enableMinimumPrecision",
                    "enable11_1DoubleExtensions",
                    "enable11_1ShaderExtensions",
                ];
                let set: Vec<&str> =
                    names.iter().enumerate().filter(|(i, _)| flags & (1 << i) != 0).map(|(_, n)| *n).collect();
                format!("dcl_globalFlags {}", set.join(" | "))
            }
            104 => format!("dcl_temps {}", t.next()?),
            105 => {
                let (register, count, components) = (t.next()?, t.next()?, t.next()?);
                format!("dcl_indexableTemp x{register}[{count}], {components}")
            }
            89 => {
                let op = t.operand(&mut constants)?;
                let op = op.trim_end_matches(".xyzw");
                constants.clear();
                let access = if token & (1 << 11) != 0 { "dynamicIndexed" } else { "immediateIndexed" };
                format!("dcl_constantbuffer {op}, {access}")
            }
            90 => {
                let op = t.operand(&mut constants)?;
                let mode =
                    ["mode_default", "mode_comparison", "mode_mono"].get(((token >> 11) & 0xf) as usize).copied();
                format!("dcl_sampler {op}, {}", mode.unwrap_or("mode_unknown"))
            }
            88 | 156 => {
                let dimension = lookup(RESOURCE_DIMENSIONS, (token >> 11) & 0x1f);
                let op = t.operand(&mut constants)?;
                let returns = return_type(t.next()?);
                let prefix = if opcode == 88 { "dcl_resource" } else { "dcl_uav_typed" };
                format!("{prefix}_{dimension} {returns} {op}")
            }
            96 | 97 | 102 | 103 => {
                let op = t.operand(&mut constants)?;
                format!("{name} {op}, {}", lookup(SYSTEM_VALUES, t.next()?))
            }
            98 => {
                let mode = lookup(INTERPOLATION, (token >> 11) & 0xf);
                format!("dcl_input_ps {mode} {}", t.operand(&mut constants)?)
            }
            99 | 100 => {
                let mode = lookup(INTERPOLATION, (token >> 11) & 0xf);
                let op = t.operand(&mut constants)?;
                format!("{name} {mode} {op}, {}", lookup(SYSTEM_VALUES, t.next()?))
            }
            91 => {
                let op = t.operand(&mut constants)?;
                format!("dcl_indexrange {op} {}", t.next()?)
            }
            94 | 153 | 154 | 206 => format!("{name} {}", t.next()?),
            155 => format!("dcl_thread_group {}, {}, {}", t.next()?, t.next()?, t.next()?),
            152 => format!("dcl_hs_max_tessfactor l({:.6})", f32::from_bits(t.next()?)),
            158 | 162 => {
                let op = t.operand(&mut constants)?;
                format!("{name} {op}, {}", t.next()?)
            }
            159 => {
                let op = t.operand(&mut constants)?;
                format!("{name} {op}, {}", t.next()?)
            }
            160 => {
                let op = t.operand(&mut constants)?;
                format!("{name} {op}, {}, {}", t.next()?, t.next()?)
            }
            92 | 93 | 147..=151 => format!("{name} {}", (token >> 11) & 0x7f),
            _ => {
                let mut suffix = String::new();
                // Extended opcode tokens: sample offsets, resource dimension and return type.
                let mut extended = token >> 31 != 0;
                while extended {
                    let ext = t.next()?;
                    match ext & 0x3f {
                        1 => {
                            let offset = |shift: u32| (((ext >> shift) & 0xf) as i32) << 28 >> 28;
                            suffix.push_str(&format!("_aoffimmi({},{},{})", offset(9), offset(13), offset(17)));
                        }
                        2 => {
                            suffix.push_str(&format!("_indexable({})", lookup(RESOURCE_DIMENSIONS, (ext >> 6) & 0x1f)))
                        }
                        3 => suffix.push_str(&return_type(ext >> 6)),
                        _ => {}
                    }
                    extended = ext >> 31 != 0;
                }
                let mut full = name.to_owned();
                match opcode {
                    3 | 5 | 8 | 13 | 31 | 63 => full.push_str(if token & (1 << 18) != 0 { "_nz" } else { "_z" }),
                    61 => full.push_str(match (token >> 11) & 3 {
                        1 => "_rcpFloat",
                        2 => "_uint",
                        _ => "",
                    }),
                    190 => {
                        for (bit, flag) in [(14, "_uglobal"), (13, "_ugroup"), (12, "_g"), (11, "_t")] {
                            if token & (1 << bit) != 0 {
                                full.push_str(flag);
                            }
                        }
                    }
                    _ if token & (1 << 13) != 0 && !name.starts_with("dcl_") => full.push_str("_sat"),
                    _ => {}
                }
                full.push_str(&suffix);
                let mut operands = Vec::new();
                while t.pos < t.end {
                    operands.push(t.operand(&mut constants)?);
                }
                if operands.is_empty() { full } else { format!("{full} {}", operands.join(", ")) }
            }
        };
        if t.pos < t.end && opcode != 53 {
            return Err(bad(format!("{name}: {} unread tokens", t.end - t.pos)));
        }
        instructions.push(Instruction { text, constants, depth: line_depth });
        pos += size;
    }
    Ok(Program { profile, instructions })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediates() {
        assert_eq!(immediate(0), "0");
        assert_eq!(immediate(1.0f32.to_bits()), "1.000000");
        assert_eq!(immediate((-0.5f32).to_bits()), "-0.500000");
        assert_eq!(immediate(3), "3");
        assert_eq!(immediate(-1i32 as u32), "-1");
        assert_eq!(immediate(0x0208_1cea), "1e-37");
        assert_eq!(immediate(0x007f_ffff), "0x007fffff");
    }

    #[test]
    fn disassembles_a_small_program() {
        // vs_4_0: dcl_temps 1; mov o0.xyzw, v0.xyzw; mul r0.x, v0.x, l(2.0); ret
        let tokens: [u32; 17] = [
            0x0001_0040,
            17,
            0x0200_0068,
            1,
            0x0500_0036,
            0x0010_20f2,
            0,
            0x0010_1e46,
            0,
            0x0700_0038,
            0x0010_0012,
            0,
            0x0010_100a,
            0,
            0x0000_4001,
            2.0f32.to_bits(),
            0x0100_003e,
        ];
        let bytes: Vec<u8> = tokens.iter().flat_map(|t| t.to_le_bytes()).collect();
        let program = disassemble(&bytes).unwrap();
        assert_eq!(program.profile, "vs_4_0");
        let lines: Vec<&str> = program.instructions.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(lines, ["dcl_temps 1", "mov o0.xyzw, v0.xyzw", "mul r0.x, v0.x, l(2.000000)", "ret"]);
    }
}
