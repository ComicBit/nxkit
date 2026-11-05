//! `border_emu` (port of `dmsh_build.border_emu`): border colours Metal lacks, through `dm_bs(...)` around the
//! samples of the argument buffer's float textures.

use super::text::{find_matching, is_word, struct_group};

/// `abi.BORDER_SAMPLERS`.
const BORDER_SAMPLERS: usize = 32;

fn helpers() -> String {
    format!(
        "
struct dm_border_t {{ uint mask; float4 k[{BORDER_SAMPLERS}]; }};
static inline __attribute__((always_inline)) float4 dm_bmix(float4 r, float4 w, float4 k) {{ return r + (w - r) * k; }}
static inline __attribute__((always_inline)) float dm_bmix(float r, float w, float4 k) {{ return r + (w - r) * k.x; }}
template<typename V, typename T, typename... A> static inline __attribute__((always_inline)) V dm_bs(V r, constant dm_border_t* b, uint i, T t, sampler w, A... a)
{{
    if (b->mask >> i & 1u) r = dm_bmix(r, t.sample(w, a...), b->k[i]);
    return r;
}}
"
    )
}

/// One argument-buffer member: `^\s*(.+?)\s+(\w+)\s*\[\[id\((\d+)\)(?:\s*,\s*\w+\(\d+\))*\]\];` (re.M).
/// One argument-buffer member line → (type, name, id).
pub fn member(line: &str) -> Option<(String, String, u64)> {
    let l = line.trim_start();
    let b = l.as_bytes();
    // The lazy type: the first whitespace run after which the rest matches.
    let mut i = 1;
    while i < l.len() {
        if b[i].is_ascii_whitespace() && !b[i - 1].is_ascii_whitespace() {
            let ty = &l[..i];
            let p = i + (l[i..].len() - l[i..].trim_start().len());
            if let Some(r) = tail(&l[p..]) {
                return Some((ty.trim().to_string(), r.0, r.1));
            }
        }
        i += 1;
    }
    None
}

/// `(\w+)\s*\[\[id\((\d+)\)(?:\s*,\s*\w+\(\d+\))*\]\];` at the start of `s`.
fn tail(s: &str) -> Option<(String, u64)> {
    let e = s
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(s.len(), |(k, _)| k);
    if e == 0 {
        return None;
    }
    let name = &s[..e];
    let r = s[e..].trim_start().strip_prefix("[[id(")?;
    let d = r.bytes().take_while(u8::is_ascii_digit).count();
    let id: u64 = r[..d].parse().ok()?;
    let mut r = r[d..].strip_prefix(')')?;
    loop {
        if let Some(x) = r.strip_prefix("]];") {
            let _ = x;
            return Some((name.to_string(), id));
        }
        let t = r.trim_start().strip_prefix(',')?.trim_start();
        let w = t
            .char_indices()
            .find(|(_, c)| !is_word(*c))
            .map_or(t.len(), |(k, _)| k);
        if w == 0 {
            return None;
        }
        let t = t[w..].strip_prefix('(')?;
        let d = t.bytes().take_while(u8::is_ascii_digit).count();
        if d == 0 {
            return None;
        }
        r = t[d..].strip_prefix(')')?;
    }
}

/// `re.fullmatch(r'(texture|depth)\w*<float(,.*)?>', ty)`.
fn float_texture(ty: &str) -> bool {
    let Some(r) = ty
        .strip_prefix("texture")
        .or_else(|| ty.strip_prefix("depth"))
    else {
        return false;
    };
    let w = r
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(r.len(), |(k, _)| k);
    let Some(r) = r[w..].strip_prefix("<float") else {
        return false;
    };
    r == ">" || (r.starts_with(',') && r.ends_with('>') && !r.contains('\n'))
}

/// `re.fullmatch(r'array<.*,\s*(\d+)>', ty)` → the length.
pub(crate) fn array_len(ty: &str) -> Option<u64> {
    let r = ty.strip_prefix("array<")?.strip_suffix('>')?;
    let c = r.rfind(',')?;
    let n = r[c + 1..].trim_start();
    (!n.is_empty() && n.bytes().all(|x| x.is_ascii_digit()) && !r[..c].contains('\n'))
        .then(|| n.parse().ok())?
}

/// Calls `spvDescriptorSet0\.(\w+)\.sample\(spvDescriptorSet0\.(\w+), ` → (start, end, texture, sampler).
fn calls(s: &str) -> Vec<(usize, usize, &str, &str)> {
    const P: &str = "spvDescriptorSet0.";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(off) = s[from..].find(P) {
        let i = from + off;
        let a = i + P.len();
        let e = s[a..]
            .char_indices()
            .find(|(_, c)| !is_word(*c))
            .map_or(s.len(), |(k, _)| a + k);
        let r = &s[e..];
        let hit = (|| {
            let r2 = r.strip_prefix(".sample(spvDescriptorSet0.")?;
            let b = e + ".sample(spvDescriptorSet0.".len();
            let w = r2
                .char_indices()
                .find(|(_, c)| !is_word(*c))
                .map_or(r2.len(), |(k, _)| k);
            (w > 0 && r2[w..].starts_with(", ")).then(|| (b + w + 2, &s[b..b + w]))
        })();
        match hit {
            Some((end, sampler)) if e > a => {
                out.push((i, end, &s[a..e], sampler));
                from = end;
            }
            _ => from = i + 1,
        }
    }
    out
}

/// `border_emu(msl)`.
pub fn border_emu(msl: &str) -> String {
    let Some((gs, ge)) = struct_group(msl, "struct spvDescriptorSetBuffer0") else {
        return msl.to_string();
    };
    let members: Vec<(String, String, u64)> = msl[gs..ge].split('\n').filter_map(member).collect();
    let samplers: Vec<&str> = members
        .iter()
        .filter(|m| m.0 == "sampler")
        .map(|m| m.1.as_str())
        .collect();
    let ftex: Vec<&str> = members
        .iter()
        .filter(|m| float_texture(&m.0))
        .map(|m| m.1.as_str())
        .collect();
    let used: Vec<&str> = calls(msl)
        .into_iter()
        .filter(|c| ftex.contains(&c.2))
        .map(|c| c.3)
        .collect();
    let bits: Vec<&str> = samplers
        .iter()
        .copied()
        .filter(|n| used.contains(n))
        .take(BORDER_SAMPLERS)
        .collect();
    if bits.is_empty() {
        return msl.to_string();
    }
    let nid = members
        .iter()
        .map(|(ty, _, id)| id + array_len(ty).unwrap_or(1))
        .max()
        .unwrap_or(0);
    let mut add = String::new();
    for (i, n) in bits.iter().enumerate() {
        add.push_str(&format!("    sampler {n}_w [[id({})]];\n", nid + i as u64));
    }
    add.push_str(&format!(
        "    constant dm_border_t* dm_border [[id({})]];\n",
        nid + bits.len() as u64
    ));
    let nl = if msl[..ge].ends_with('\n') { "" } else { "\n" };
    let mut s = format!("{}{nl}{add}{}", &msl[..ge], &msl[ge..]);
    let found: Vec<(usize, usize, String, String)> = calls(&s)
        .into_iter()
        .filter(|c| bits.contains(&c.3) && ftex.contains(&c.2))
        .map(|c| (c.0, c.1, c.2.to_string(), c.3.to_string()))
        .collect();
    for (start, end, tex, smp) in found.into_iter().rev() {
        let Some(close) = find_matching(&s, end) else {
            continue;
        };
        let args = s[end..close].to_string();
        let bit = bits.iter().position(|b| *b == smp).unwrap_or(0);
        let rep = format!(
            "dm_bs({}, spvDescriptorSet0.dm_border, {bit}u, spvDescriptorSet0.{tex}, spvDescriptorSet0.{smp}_w, {args})",
            &s[start..=close]
        );
        s.replace_range(start..=close, &rep);
    }
    s.replacen(
        "using namespace metal;\n",
        &format!("using namespace metal;\n{}", helpers()),
        1,
    )
}
