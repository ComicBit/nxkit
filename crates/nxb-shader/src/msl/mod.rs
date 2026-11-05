//! The MSL rewrite passes between `spirv-cross` and the final `<key>_b.metal` (ports of `dmsh_build.py` steps and the
//! pure helper modules `dmsh_atomic`, `dmsh_wide`). Each pass reproduces its Python regular expressions with hand
//! matchers. Passes that never fire on the games ported so far refuse the shader instead of guessing (`Refused`).

mod border;
mod capture;
mod first_store;
pub use first_store::first_store_could_apply;
mod fsuint;
mod simple;
mod text;
pub mod vertex;
pub use capture::capture;
mod wide;

pub use border::{border_emu, member as border_member};

/// The `spvDescriptorSetBuffer0` struct body span.
pub fn descriptor_group(msl: &str) -> Option<(usize, usize)> {
    text::struct_group(msl, "struct spvDescriptorSetBuffer0")
}

/// The `main0_out` struct body span.
pub fn out_group(msl: &str) -> Option<(usize, usize)> {
    text::struct_group(msl, "struct main0_out")
}
pub use fsuint::fs_uint;
pub use simple::{check_msl, guard_hmma, guard_pools, precise};
pub use wide::wide_loads;

/// The pass chain of `dmsh_build.finish` for a pixel or compute shader up to the stage-specific steps:
/// `check_msl`, `precise`, `hmma_msl`, `pool_arrays`, `wide_loads`, `border_emu`. `Err` is the shader's reason and detail.
pub fn common_passes(src: &str) -> Result<String, (String, String)> {
    let s = check_msl(src).map_err(|e| ("image atomic MSL failed".to_string(), e))?;
    let s = precise(&s);
    guard_hmma(&s).map_err(|e| ("HMMA rewrite failed".to_string(), e))?;
    guard_pools(&s).map_err(|e| ("pool rewrite failed".to_string(), e))?;
    let s = wide_loads(&s);
    Ok(border_emu(&s))
}

/// `dmsh_build.XC`, the spirv-cross flags.
pub const XC: [&str; 6] = [
    "--msl",
    "--msl-version",
    "20300",
    "--msl-argument-buffers",
    "--msl-texture-buffer-native",
    "--msl-pad-fragment-output",
];

/// `dmsh_atomic.uses_image_atomics`: the module has an `OpImageTexelPointer`.
pub fn uses_image_atomics(spv: &[u8]) -> Result<bool, String> {
    let words: Vec<u32> = spv
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    if words.len() < 5 || words[0] != 0x0723_0203 {
        return Err("not a SPIR-V module".into());
    }
    let mut i = 5;
    while i < words.len() {
        let (n, op) = ((words[i] >> 16) as usize, words[i] & 0xffff);
        if op == 60 {
            return Ok(true);
        }
        if n == 0 {
            return Err("malformed SPIR-V instruction".into());
        }
        i += n;
    }
    Ok(false)
}

/// `dmsh_atomic.cross_args(XC, spv)`: MSL 3.1 for modules with image atomics.
pub fn cross_args(spv: &[u8]) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = XC.iter().map(|s| s.to_string()).collect();
    if uses_image_atomics(spv)? {
        a[2] = "30100".into();
    }
    Ok(a)
}
