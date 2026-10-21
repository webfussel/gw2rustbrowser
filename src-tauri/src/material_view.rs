//! Material (AMAT) preview: overview and per-shader details.

use gw2dat::formats::{material, shader};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::commands::{CmdResult, blocking, err, session};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDto {
    version: u16,
    settings: Option<SettingsDto>,
    techniques: Vec<TechniqueDto>,
    samplers: Vec<String>,
    shaders: Vec<ShaderSummaryDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsDto {
    textures: Vec<String>,
    texture_count: u8,
    texture_array_range: u8,
    sort_order: u8,
    sort_triangles: u8,
    procedural_animation: u8,
    flags: u32,
    debug_flags: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TechniqueDto {
    id: u32,
    passes: Vec<Vec<EffectDto>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectDto {
    name: String,
    render_state: String,
    state: Vec<String>,
    pass_flags: u32,
    pixel_shader: u32,
    /// (vertex variant, shader index).
    vertex_shaders: Vec<(u32, u32)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShaderSummaryDto {
    stage: &'static str,
    size: usize,
    profile: Option<String>,
    instructions: usize,
    uniforms: usize,
}

#[tauri::command]
pub async fn material_info(app: AppHandle, mft: u32) -> CmdResult<MaterialDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        info(&session.read(mft).map_err(err)?)
    })
    .await
}

fn info(data: &[u8]) -> CmdResult<MaterialDto> {
    let m = material::decode(data).map_err(err)?;
    let shaders = m
        .shaders
        .iter()
        .map(|s| {
            let parsed = shader::bgfx(&s.binary).ok();
            let program = parsed.as_ref().and_then(|b| shader::dxbc(b.code).ok()).and_then(|d| d.disassemble()?.ok());
            ShaderSummaryDto {
                stage: material::stage_name(s),
                size: s.binary.len(),
                // Declarations are not counted as instructions.
                instructions: program
                    .as_ref()
                    .map_or(0, |p| p.instructions.iter().filter(|i| !i.text.starts_with("dcl_")).count()),
                profile: program.map(|p| p.profile),
                uniforms: parsed.map_or(0, |b| b.uniforms.len()),
            }
        })
        .collect();
    Ok(MaterialDto {
        version: m.version,
        settings: m.settings.as_ref().map(|s| SettingsDto {
            textures: s.textures.iter().map(|&t| material::token_label(t)).collect(),
            texture_count: s.texture_count,
            texture_array_range: s.texture_array_range,
            sort_order: s.sort_order,
            sort_triangles: s.sort_triangles,
            procedural_animation: s.procedural_animation,
            flags: s.flags,
            debug_flags: s.debug_flags,
        }),
        techniques: m
            .techniques
            .iter()
            .map(|t| TechniqueDto {
                id: t.quality,
                passes: t
                    .passes
                    .iter()
                    .map(|effects| {
                        effects
                            .iter()
                            .map(|e| EffectDto {
                                name: material::token_label(e.token),
                                render_state: format!("{:#x}", e.render_state),
                                state: material::render_state_text(e.render_state),
                                pass_flags: e.pass_flags,
                                pixel_shader: e.pixel_shader,
                                vertex_shaders: e.vertex_shaders.clone(),
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect(),
        samplers: m.samplers.iter().map(|&s| material::sampler_text(s)).collect(),
        shaders,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderDto {
    stage: &'static str,
    bgfx_version: u8,
    hash_in: u32,
    hash_out: u32,
    uniforms: Vec<UniformDto>,
    attributes: Vec<String>,
    constant_buffer_size: u16,
    inputs: Vec<SignatureDto>,
    outputs: Vec<SignatureDto>,
    profile: Option<String>,
    lines: Vec<LineDto>,
    error: Option<String>,
    /// Material sampler bindings of the shader record.
    samplers: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UniformDto {
    name: String,
    kind: &'static str,
    count: u8,
    register: u16,
    register_count: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignatureDto {
    semantic: String,
    register: u32,
    mask: String,
    component_type: &'static str,
    system_value: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LineDto {
    text: String,
    depth: usize,
    note: Option<String>,
}

fn signature(elements: Vec<shader::SignatureElement>) -> Vec<SignatureDto> {
    elements
        .into_iter()
        .map(|e| SignatureDto {
            semantic: if e.index > 0 { format!("{}{}", e.semantic, e.index) } else { e.semantic },
            register: e.register,
            mask: (0..4).filter(|i| e.mask & (1 << i) != 0).map(|i| ['x', 'y', 'z', 'w'][i]).collect(),
            component_type: match e.component_type {
                1 => "uint",
                2 => "int",
                3 => "float",
                _ => "unknown",
            },
            system_value: e.system_value,
        })
        .collect()
}

#[tauri::command]
pub async fn material_shader(app: AppHandle, mft: u32, index: usize) -> CmdResult<ShaderDto> {
    blocking(move || {
        let session = session(&app.state::<AppState>())?;
        shader_details(&session.read(mft).map_err(err)?, index)
    })
    .await
}

fn shader_details(data: &[u8], index: usize) -> CmdResult<ShaderDto> {
    let m = material::decode(data).map_err(err)?;
    let record = m.shaders.get(index).ok_or("No such shader")?;
    let bgfx = shader::bgfx(&record.binary).map_err(err)?;
    let dxbc = shader::dxbc(bgfx.code).ok();
    let (profile, lines, error) = match dxbc.as_ref().and_then(|d| d.disassemble()) {
        Some(Ok(program)) => (
            Some(program.profile.clone()),
            program
                .instructions
                .iter()
                .map(|i| LineDto { text: i.text.clone(), depth: i.depth, note: material::annotate(i, &bgfx.uniforms) })
                .collect(),
            None,
        ),
        Some(Err(e)) => (None, Vec::new(), Some(e.to_string())),
        None => (None, Vec::new(), Some("The shader has no DXBC program".into())),
    };
    Ok(ShaderDto {
        stage: material::stage_name(record),
        bgfx_version: bgfx.version,
        hash_in: bgfx.hash_in,
        hash_out: bgfx.hash_out,
        uniforms: bgfx
            .uniforms
            .iter()
            .map(|u| UniformDto {
                name: u.name.clone(),
                kind: u.type_name(),
                count: u.count,
                register: u.register,
                register_count: u.register_count,
            })
            .collect(),
        attributes: bgfx.attributes.iter().map(|&a| shader::attribute_name(a)).collect(),
        constant_buffer_size: bgfx.constant_buffer_size,
        inputs: dxbc.as_ref().and_then(|d| d.inputs().ok()).map(signature).unwrap_or_default(),
        outputs: dxbc.as_ref().and_then(|d| d.outputs().ok()).map(signature).unwrap_or_default(),
        profile,
        lines,
        error,
        samplers: record
            .samplers
            .iter()
            .map(|s| {
                let texture = m
                    .settings
                    .as_ref()
                    .and_then(|t| t.textures.get(s.texture_index as usize))
                    .map_or_else(|| format!("texture {}", s.texture_index), |&t| material::token_label(t));
                let state = m.samplers.get(s.state_index as usize).map(|&f| material::sampler_text(f));
                let mut text = format!("t{}: {texture}", s.texture_slot);
                if s.token != 0 {
                    text.push_str(&format!(" ({})", material::token_label(s.token)));
                }
                text.push_str(&format!(" · sampler state {}", s.state_index));
                if let Some(state) = state {
                    text.push_str(&format!(": {state}"));
                }
                text
            })
            .collect(),
    })
}
