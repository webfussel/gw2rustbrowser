//! Names of files whose purpose is known (from the original browser's
//! `known_files.txt`). The archive itself stores no file names.

const KNOWN: &[(u32, &str)] = &[
    (553489, "bink2w32.dll"),
    (640202, "Coherent.dll"),
    (640203, "Coherent.exe"),
    (640204, "icudt46.dll"),
    (640205, "libEGL.dll"),
    (640206, "libGLESv2.dll"),
    (720546, "D3DCompiler_43.dll"),
    (791411, "ffmpegsumo.dll"),
    (791413, "THIRDPARTYSOFTWAREREADME.TXT"),
    (1147565, "amd_ags_x86.dll"),
    (1147566, "amd_ags_x64.dll"),
    (1247265, "Coherent.dll"),
    (1247266, "Coherent.exe"),
    (1247267, "icudt46.dll"),
    (1247268, "libEGL.dll"),
    (1247269, "libGLESv2.dll"),
    (1247270, "ffmpegsumo.dll"),
    (1247271, "d3dcompiler_43.dll"),
    (1247272, "bink2w64.dll"),
];

/// Original file name for a base id, if known.
pub fn known_name(base_id: u32) -> Option<&'static str> {
    KNOWN.iter().find(|(id, _)| *id == base_id).map(|(_, name)| *name)
}
