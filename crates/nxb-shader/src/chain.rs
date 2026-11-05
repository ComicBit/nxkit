//! `dmsh_chain.py`: the chain version V of the shader library (`b-<V>/`) and the digest of a game profile's inputs
//! (`b-<V>/inputs.tsv`). Both are read from the engine's files, so a changed engine gives another V as in Python.

use std::fs;
use std::path::Path;

/// The files V covers, relative to `dm/gfx` (`dmsh_chain.FILES`, same order).
const FILES: &[&str] = &[
    "tools/dmsh_chain.py",
    "tools/dmsh_build.py",
    "tools/dmsh_atomic.py",
    "tools/dmsh_wide.py",
    "tools/dmsh_fetch.py",
    "tools/dmsh_vin.py",
    "tools/dmsh_pull.py",
    "tools/dmsh_pull.metal",
    "tools/dmsh_alpha.py",
    "tools/dmsh_logic.py",
    "tools/dmsh_side.py",
    "tools/dmsh_geometry.py",
    "tools/dmsh_mesh.py",
    "tools/dmsh_meshstage.py",
    "tools/dmsh_facts.py",
    "tools/dmsh_ampere.py",
    "nvn/dmsh_abi.h",
    "tools/dmsh_abi.py",
    "tools/dmsh_xlate/Program.cs",
    "tools/dmsh_xlate/dmsh_xlate.csproj",
    "../ios/tools/ios_metal.py",
    "tools/dmsh_chain.lock",
];

/// The vendored translator sources V also covers, relative to `third_party/ryujinx/src/Ryujinx.Graphics.Shader/`.
const RYUJINX: &[&str] = &[
    "Instructions/InstEmitSurface.cs",
    "IntermediateRepresentation/TextureFlags.cs",
    "Translation/TranslationFlags.cs",
    "Translation/HostCapabilities.cs",
    "Translation/Transforms/GeometryToCompute.cs",
    "Translation/TranslatorContext.cs",
    "Translation/Translator.cs",
    "Translation/EmitterContext.cs",
    "Translation/ShaderDefinitions.cs",
    "CodeGen/Spirv/Instructions.cs",
    "StructuredIr/ShaderProperties.cs",
    "Translation/Optimizations/BindlessElimination.cs",
    "Translation/Optimizations/Optimizer.cs",
    "Translation/Optimizations/GlobalToStorage.cs",
    "CodeGen/Spirv/SpirvGenerator.cs",
];

/// The profile tables of the inputs digest (`dmsh_chain.INPUT_TABLES`, same order).
const INPUT_TABLES: &[&str] = &[
    "shader_facts.tsv",
    "shader_map.tsv",
    "shader_map.tsv.conflicts.tsv",
    "shader_pairs.tsv",
    "pso_pairs.tsv",
    "bf_want.tsv",
    "geo_mesh.tsv",
    "dual_want.tsv",
];

/// The translation options at their defaults (`XLATE_DEBUG=1`, `XLATE_REDUCED=1`): the only ones the host loads.
const OPTIONS: &str = "debug=True reduced=True";

/// `dmsh_chain.version()`: V for the engine at `engine` (the folder holding `dm/` and `third_party/`).
pub fn version(engine: &Path) -> Result<String, String> {
    let gfx = engine.join("dm/gfx");
    let mut h = nxb_model::Sha256::new();
    let rel: Vec<String> = FILES
        .iter()
        .map(|f| (*f).to_string())
        .chain(
            RYUJINX
                .iter()
                .map(|f| format!("../third_party/ryujinx/src/Ryujinx.Graphics.Shader/{f}")),
        )
        .collect();
    for f in &rel {
        let p = gfx.join(f);
        let body = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        h.update(f.as_bytes());
        h.update(&[0]);
        h.update(&body);
        h.update(&[0]);
    }
    h.update(OPTIONS.as_bytes());
    Ok(nxb_model::to_hex(&h.finalize().0)[..12].to_string())
}

/// `dmsh_chain.profile_rows`: the profile's `nvn_*` / `shader_*` rows of `game.tsv`, tab-joined.
pub fn profile_rows(game_tsv: &str) -> String {
    let mut s = String::new();
    for line in game_tsv.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let mut cells = line.split('\t');
        let key = cells.next().unwrap_or("").trim();
        if key.starts_with("nvn_") || key.starts_with("shader_") {
            s.push_str(key);
            for v in cells {
                s.push('\t');
                s.push_str(v.trim());
            }
            s.push('\n');
        }
    }
    s
}

/// `dmsh_chain.inputs`: the digest of a game profile folder's inputs to its shader library.
pub fn inputs(profile: &Path) -> Result<String, String> {
    let mut h = nxb_model::Sha256::new();
    for t in INPUT_TABLES {
        let body = fs::read(profile.join(t)).unwrap_or_else(|_| b"missing".to_vec());
        h.update(format!("game/{t}").as_bytes());
        h.update(&[0]);
        h.update(&body);
        h.update(&[0]);
    }
    let g = profile.join("game.tsv");
    let text = fs::read_to_string(&g).map_err(|e| format!("{}: {e}", g.display()))?;
    h.update(profile_rows(&text).as_bytes());
    Ok(nxb_model::to_hex(&h.finalize().0)[..12].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_take_build_keys_only() {
        let t = "# c\nname\tTitleA\nshader_versions\t v0 \tv1\nnvn_x\t1\n\nother\t2\n";
        assert_eq!(profile_rows(t), "shader_versions\tv0\tv1\nnvn_x\t1\n");
    }

    #[test]
    fn missing_profile_is_an_error() {
        assert!(inputs(Path::new("/nonexistent-profile")).is_err());
        assert!(version(Path::new("/nonexistent-engine")).is_err());
    }
}
