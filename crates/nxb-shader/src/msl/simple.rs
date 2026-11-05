//! Small passes: `precise`, `dmsh_atomic.check_msl`, and the guards for passes not ported yet.

use super::text::{is_word, struct_group, word_starts};

/// `re.sub(r'\bfast::(min|max|clamp)\(', r'precise::\1(', msl)`.
pub fn precise(msl: &str) -> String {
    let mut out = String::with_capacity(msl.len() + 1024);
    let mut at = 0;
    for i in word_starts(msl, "fast::") {
        if i < at {
            continue;
        }
        let r = &msl[i + 6..];
        if let Some(f) = ["min(", "max(", "clamp("]
            .into_iter()
            .find(|f| r.starts_with(f))
        {
            out.push_str(&msl[at..i]);
            out.push_str("precise::");
            out.push_str(f);
            at = i + 6 + f.len();
        }
    }
    out.push_str(&msl[at..]);
    out
}

/// `dmsh_atomic.check_msl`: fail on SPIRV-Cross's pointer emulation of image atomics
/// (`\bspvImage2DAtomicCoord\b|device\s+atomic_u?int\s*\*\s*\w+_atomic\b`).
pub fn check_msl(src: &str) -> Result<String, String> {
    let bad1 = word_starts(src, "spvImage2DAtomicCoord").any(|i| {
        !src[i + "spvImage2DAtomicCoord".len()..]
            .chars()
            .next()
            .is_some_and(is_word)
    });
    let bad2 = src.match_indices("device").any(|(i, _)| {
        let r = &src[i + 6..];
        let t = r.trim_start();
        if t.len() == r.len() {
            return false;
        }
        let Some(t) = t.strip_prefix("atomic_") else {
            return false;
        };
        let t = t.strip_prefix('u').unwrap_or(t);
        let Some(t) = t.strip_prefix("int") else {
            return false;
        };
        let Some(t) = t.trim_start().strip_prefix('*') else {
            return false;
        };
        let t = t.trim_start();
        let w = t
            .char_indices()
            .find(|(_, c)| !is_word(*c))
            .map_or(t.len(), |(k, _)| k);
        w > "_atomic".len() && t[..w].ends_with("_atomic")
    });
    if bad1 || bad2 {
        return Err("image atomics emulated through a raw pointer; they must be native texture atomics (MSL 3.1)".into());
    }
    Ok(src.to_string())
}

/// `hmma_msl` (Ampere HMMA through simdgroup_matrix) is not ported: refuse a shader that has the helper.
pub fn guard_hmma(src: &str) -> Result<(), String> {
    if src.contains(" dmsh_hmma16816_f16(") {
        return Err("dmsh_hmma16816_f16: the HMMA rewrite is not ported yet".into());
    }
    Ok(())
}

/// `pool_arrays` is not ported: refuse a shader with a pool array (`array<…, N> <stage>_<t|s>_cb0_40000000 [[id(` with
/// N ≥ 33) in its argument buffer.
pub fn guard_pools(src: &str) -> Result<(), String> {
    let Some((gs, ge)) = struct_group(src, "struct spvDescriptorSetBuffer0") else {
        return Ok(());
    };
    for line in src[gs..ge].lines() {
        let t = line.trim_start();
        if let Some(r) = t.strip_prefix("array<")
            && let Some((head, name)) = r.split_once("> ")
            && let Some(n) = head
                .rsplit_once(", ")
                .and_then(|(_, n)| n.parse::<u64>().ok())
            && n >= 33
            && name.split_once(' ').is_some_and(|(nm, rest)| {
                nm.ends_with("_cb0_40000000") && rest.starts_with("[[id(")
            })
        {
            return Err(format!(
                "{}: the pool array rewrite is not ported yet",
                name
            ));
        }
    }
    Ok(())
}
