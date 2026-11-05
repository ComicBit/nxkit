//! `dmsh_geometry.capture` with `output_words`: the geometry capture entry point `main0_gs`.

use super::text::{is_word, struct_group};
use super::vertex::{prev_word, word_end};

/// `dmsh_geometry.output_words(src)` → (word, expression) in Python's dict order.
fn output_words(src: &str) -> Result<Vec<(u32, String)>, String> {
    let (gs, ge) =
        struct_group(src, "struct main0_out").ok_or("geometry capture: missing output struct")?;
    let mut stores: Vec<(u32, String)> = Vec::new();
    for line in src[gs..ge].lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        let user = (|| {
            let ty = parts.first()?;
            if !["float4", "uint4", "int4"].contains(ty) {
                return None;
            }
            let nm = parts.get(1)?;
            let l = parts
                .get(2)?
                .strip_prefix("[[user(locn")?
                .strip_suffix(")]];")?;
            (nm.chars().all(is_word) && !l.is_empty() && l.bytes().all(|c| c.is_ascii_digit()))
                .then(|| (nm.to_string(), l.parse::<u32>().ok()))
        })();
        if let Some((nm, Some(l))) = user {
            if l >= 32 {
                return Err("geometry capture: attribute outside Maxwell map".into());
            }
            for (c, s) in "xyzw".chars().enumerate() {
                stores.push((16 + l * 4 + c as u32, format!("out.{nm}.{s}")));
            }
            continue;
        }
        if let Some(r) = line.strip_prefix("float4 ") {
            let e = word_end(r, 0);
            let t = &r[e..];
            if e > 0 && (t == " [[position]];" || t == " [[position, invariant]];") {
                for (c, s) in "xyzw".chars().enumerate() {
                    stores.push((c as u32, format!("out.{}.{s}", &r[..e])));
                }
                continue;
            }
        }
        let special = ["float ", "uint ", "int "]
            .iter()
            .find_map(|p| line.strip_prefix(p))
            .and_then(|r| {
                let e = word_end(r, 0);
                let t = r[e..].strip_prefix(" [[")?.strip_suffix("]];")?;
                let off = match t {
                    "point_size" => 4,
                    "render_target_array_index" => 13,
                    "viewport_array_index" => 14,
                    _ => return None,
                };
                (e > 0).then(|| (off, format!("out.{}", &r[..e])))
            });
        if let Some(x) = special {
            stores.push(x);
            continue;
        }
        if let Some(r) = line.strip_prefix("float ") {
            let e = word_end(r, 0);
            let nm = &r[..e];
            let mut t = &r[e..];
            let mut n1 = None;
            if let Some(x) = t.strip_prefix('[')
                && let Some(k) = x.find(']')
            {
                n1 = x[..k].parse::<u32>().ok();
                t = &x[k + 1..];
            }
            if let Some(t2) = t.strip_prefix(" [[clip_distance]]") {
                let n2 = t2
                    .strip_prefix(" [")
                    .and_then(|x| x.strip_suffix("];"))
                    .and_then(|x| x.parse::<u32>().ok());
                let ok_tail = t2 == ";" || n2.is_some();
                if e > 0
                    && ok_tail
                    && let Some(n) = n1.or(n2)
                    && n <= 8
                {
                    for c in 0..n {
                        stores.push((5 + c, format!("out.{nm}[{c}]")));
                    }
                    continue;
                }
            }
            if let Some(d) = t
                .strip_prefix(" [[user(clip")
                .and_then(|x| x.strip_suffix(")]];"))
                && d.len() == 1
                && let Ok(n) = d.parse::<u32>()
            {
                stores.push((5 + n, format!("out.{nm}")));
                continue;
            }
        }
        return Err(format!("geometry capture: unknown output {line}"));
    }
    // dict(stores): last value wins, first position kept.
    let mut out: Vec<(u32, String)> = Vec::new();
    for (k, v) in stores {
        match out.iter_mut().find(|(x, _)| *x == k) {
            Some(slot) => slot.1 = v,
            None => out.push((k, v)),
        }
    }
    Ok(out)
}

/// `dmsh_geometry.capture(src)`.
pub fn capture(src: &str) -> Result<String, String> {
    let m = src
        .find("vertex main0_out main0(")
        .filter(|i| !prev_word(src, *i));
    let Some(m) = m else {
        return Err("geometry capture: missing main0 vertex entry".into());
    };
    let open = m + "vertex main0_out main0(".len();
    // (.*?)\)\s*\{ lazily.
    let mut k = open;
    let mut found = None;
    while let Some(off) = src[k..].find(')') {
        let c = k + off;
        let r = src[c + 1..].trim_start();
        if r.starts_with('{') {
            found = Some((c, src.len() - r.len() + 1));
            break;
        }
        k = c + 1;
    }
    let (close, end) = found.ok_or("geometry capture: missing main0 vertex entry")?;
    let mut params = src[open..close].to_string();
    let body = &src[end..];
    if !body.trim_end().ends_with('}') {
        return Err("geometry capture: unexpected function ending".into());
    }
    let mut pro = String::new();
    for (semantic, name) in [("vertex_id", "dm_vid"), ("instance_id", "dm_iid")] {
        let tail = format!(" [[{semantic}]]");
        let found = params.match_indices("uint ").find_map(|(i, _)| {
            let a = i + 5;
            let e = word_end(&params, a);
            (e > a && params[e..].starts_with(&tail)).then(|| params[a..e].to_string())
        });
        match found {
            Some(n) => pro.push_str(&format!("    uint {name} = {n};\n")),
            None => {
                if !params.is_empty() {
                    params.push_str(", ");
                }
                params.push_str(&format!("uint {name} [[{semantic}]]"));
            }
        }
    }
    let stores = output_words(src)?;
    params.push_str(", constant dm_geometry_capture& dm_gs [[buffer(13)]]");
    pro.push_str("    uint4 dm_ci = dm_gs.info; device uint* dm_capture = dm_gs.dst;\n    if (dm_vid - dm_ci.x >= dm_ci.y || dm_iid - dm_ci.z >= dm_ci.w) return;\n    device uint* dm_p = dm_capture + ((ulong(dm_iid - dm_ci.z) * dm_ci.y + dm_vid - dm_ci.x) * dm_gs.stride);\n");
    let mut words: Vec<String> = (0..144u32)
        .map(|k| {
            if k == 3 || k == 4 || (k >= 19 && k % 4 == 3) {
                "0x3f800000u".to_string()
            } else {
                "0u".to_string()
            }
        })
        .collect();
    for (k, e) in stores {
        if let Some(w) = words.get_mut(k as usize) {
            *w = format!("as_type<uint>({e})");
        }
    }
    let epi: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(k, v)| format!("dm_cw(dm_gs, dm_p, {k}, {v});"))
        .collect();
    let epilogue = format!("{{ {} return; }}", epi.join(" "));
    if !body.contains("return out;") {
        return Err("geometry capture: missing return".into());
    }
    Ok(format!(
        "\nstruct dm_geometry_capture {{ device uint* dst; uint4 info; uint stride; short slot[144]; }};\nstatic inline void dm_cw(constant dm_geometry_capture& g, device uint* p, uint k, uint v) {{ short s = g.slot[k]; if (s >= 0) p[s] = v; }}\nvertex void main0_gs({params})\n{{\n{pro}{}",
        body.replace("return out;", &epilogue)
    ))
}
