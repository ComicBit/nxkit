//! `<key>_bf`: framebuffer-fetch ("readable tile") variant of a pixel shader (port of `dmsh_fetch.variant` with
//! `_once`, `_proj`, `_frag_pos`). Every regular expression of the Python is reproduced by a matcher below, each says
//! which pattern it stands for.

use nxb_pyjson::Value;
use std::collections::{BTreeMap, BTreeSet};

const NFB: usize = 8;
const BORDER_MEMBER: &str = "dm_border";
const BORDER_TWIN: &str = "_w";
const SCALE: &str = "(*spvDescriptorSet0.support_buffer)._m5[0].x";

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_end(s: &str, i: usize) -> usize {
    s[i..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(s.len(), |(k, _)| i + k)
}

fn digits_end(s: &str, i: usize) -> usize {
    i + s[i..].bytes().take_while(u8::is_ascii_digit).count()
}

/// `_\d+` at `i`: its end.
fn underscore_num(s: &str, i: usize) -> Option<usize> {
    let a = i + usize::from(s[i..].starts_with('_'));
    if a == i {
        return None;
    }
    let b = digits_end(s, a);
    (b > a).then_some(b)
}

/// `re.fullmatch(r'_\d+', s)`.
fn is_unum(s: &str) -> bool {
    underscore_num(s, 0) == Some(s.len())
}

/// `_once(msl, name)`: `len(re.findall(r'(?<![\w.])NAME = ', msl)) == 1`.
fn once(msl: &str, name: &str) -> bool {
    let pat = format!("{name} = ");
    msl.match_indices(&pat)
        .filter(|(i, _)| {
            msl[..*i]
                .chars()
                .next_back()
                .is_none_or(|c| !(is_word(c) || c == '.'))
        })
        .count()
        == 1
}

/// First occurrence of `prefix` for which `tail` accepts the text after it.
fn search<'a, T>(msl: &'a str, prefix: &str, tail: impl Fn(&'a str) -> Option<T>) -> Option<T> {
    msl.match_indices(prefix)
        .find_map(|(i, _)| tail(&msl[i + prefix.len()..]))
}

/// `(_\d+)SEP(_\d+);` at the start of `t` → the two names.
fn two_unums<'a>(t: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    let a = underscore_num(t, 0)?;
    let r = t[a..].strip_prefix(sep)?;
    let b = underscore_num(r, 0)?;
    r[b..].starts_with(';').then(|| (&t[..a], &r[..b]))
}

/// `float N = in\.(m_\d+)\.C;` → `m_X`.
fn in_member<'a>(msl: &'a str, n: &str, c: char) -> Option<&'a str> {
    search(msl, &format!("float {n} = in."), |t| {
        let e = t.strip_prefix("m_").map(|r| digits_end(r, 0) + 2)?;
        (e > 2 && t[e..].starts_with(&format!(".{c};"))).then(|| &t[..e])
    })
}

/// `_proj(msl, var, comp, locn)`.
fn proj<'a>(msl: &'a str, var: &str, comp: char, locn: &BTreeMap<&str, i64>) -> Option<&'a str> {
    let src = |n: &str, c: char| -> Option<&'a str> {
        in_member(msl, n, c).filter(|m| once(msl, n) && locn.contains_key(m))
    };
    if !once(msl, var) {
        return None;
    }
    let prefix = format!("float {var} = ");
    if let Some((x1, x2)) = search(msl, &prefix, |t| two_unums(t, " * ")) {
        for (x, r) in [(x1, x2), (x2, x1)] {
            let d = search(msl, &format!("float {r} = 1.0 / "), |t| {
                let e = underscore_num(t, 0)?;
                t[e..].starts_with(';').then(|| &t[..e])
            });
            if let Some(d) = d
                && once(msl, r)
                && src(x, comp).is_some()
                && src(x, comp) == src(d, 'w')
            {
                return src(x, comp);
            }
        }
        return None;
    }
    let (a, b) = search(msl, &prefix, |t| two_unums(t, " / "))?;
    let sa = src(a, comp);
    (sa.is_some() && sa == src(b, 'w')).then_some(sa).flatten()
}

/// `_UNI.fullmatch(u)`: `\(\*spvDescriptorSet0\.fp_c(\d+)\)\._m0\[(\d+)\]\.([xyzw])` → (slot, vector, component).
fn uniform(u: &str) -> Option<(i64, i64, i64)> {
    let r = u.strip_prefix("(*spvDescriptorSet0.fp_c")?;
    let a = digits_end(r, 0);
    let slot: i64 = r[..a].parse().ok()?;
    let r = r[a..].strip_prefix(")._m0[")?;
    let b = digits_end(r, 0);
    let vec: i64 = r[..b].parse().ok()?;
    let r = r[b..].strip_prefix("].")?;
    let comp = "xyzw".find(r).filter(|_| r.len() == 1)?;
    Some((slot, vec, comp as i64))
}

/// `one(n, pat)` with a literal right side: `float N = LIT;` and `_once(n)`.
fn one_lit(msl: &str, n: &str, lit: &str) -> bool {
    msl.contains(&format!("float {n} = {lit};")) && once(msl, n)
}

/// `_frag_pos(msl, var, comp)`.
fn frag_pos(msl: &str, var: &str, comp: char) -> Option<[i64; 3]> {
    let pixel = |n: &str| -> bool {
        let fc = format!("gl_FragCoord.{comp}");
        if one_lit(msl, n, &fc) {
            return true;
        }
        let q = search(msl, &format!("float {n} = "), |t| two_unums(t, " / "));
        matches!(q, Some((a, b)) if once(msl, n) && one_lit(msl, a, &fc) && one_lit(msl, b, SCALE))
    };
    // one(var, r'(\S+) \* (\S+)'): group 1 the maximal non-space run before " * ", group 2 the run after it up to its
    // last ';'.
    let m = search(msl, &format!("float {var} = "), |t| {
        let e1 = t.find(char::is_whitespace).unwrap_or(t.len());
        if e1 == 0 {
            return None;
        }
        let r = t[e1..].strip_prefix(" * ")?;
        let e2 = r.find(char::is_whitespace).unwrap_or(r.len());
        let run = &r[..e2];
        let semi = run.rfind(';')?;
        (semi > 0).then(|| (&t[..e1], &run[..semi]))
    })
    .filter(|_| once(msl, var))?;
    for (a, u) in [(m.0, m.1), (m.1, m.0)] {
        if let Some((slot, vec, c)) = uniform(u)
            && slot >= 3
            && is_unum(a)
            && pixel(a)
        {
            return Some([slot - 3, vec, c]);
        }
    }
    None
}

/// One match of the sample pattern: (start, end, member, var).
struct Sample<'a> {
    start: usize,
    end: usize,
    member: &'a str,
    sampler: &'a str,
    var: &'a str,
}

/// The sample pattern at `p` without the `dm_bs(` prefix: `spvDescriptorSet0\.(\w+)\.sample\(spvDescriptorSet0\.(\w+)Smplr,
/// select\((_\d+), float2\(0\.0\), isnan\(\4\)\)\)` → (end, member, sampler, var).
fn sample_at(s: &str, p: usize) -> Option<(usize, &str, &str, &str)> {
    let a = p + "spvDescriptorSet0.".len();
    if !s[p..].starts_with("spvDescriptorSet0.") {
        return None;
    }
    let b = word_end(s, a);
    if b == a || !s[b..].starts_with(".sample(spvDescriptorSet0.") {
        return None;
    }
    let c = b + ".sample(spvDescriptorSet0.".len();
    let d = word_end(s, c);
    let w = &s[c..d];
    let sampler = w.strip_suffix("Smplr").filter(|x| !x.is_empty())?;
    let e = d;
    if !s[e..].starts_with(", select(") {
        return None;
    }
    let f = e + ", select(".len();
    let g = underscore_num(s, f)?;
    let var = &s[f..g];
    let tail = format!(", float2(0.0), isnan({var})))");
    s[g..]
        .starts_with(&tail)
        .then(|| (g + tail.len(), &s[a..b], sampler, var))
}

/// The conditional tail after a `dm_bs(`-wrapped sample.
fn border_tail(s: &str, p: usize, member: &str, sampler: &str, var: &str) -> Option<usize> {
    let head = format!(", spvDescriptorSet0.{BORDER_MEMBER}, ");
    let r = s[p..].strip_prefix(&head)?;
    let n = digits_end(r, 0);
    if n == 0 {
        return None;
    }
    let rest = format!(
        "u, spvDescriptorSet0.{member}, spvDescriptorSet0.{sampler}Smplr{BORDER_TWIN}, select({var}, float2(0.0), isnan({var})))"
    );
    r[n..]
        .starts_with(&rest)
        .then(|| p + head.len() + n + rest.len())
}

/// `pat.finditer(msl)`: non-overlapping, left to right.
fn samples(s: &str) -> Vec<Sample<'_>> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < s.len() {
        let Some(off) = s[p..].find(['d', 's']) else {
            break;
        };
        let q = p + off;
        let mut hit = None;
        if s[q..].starts_with("dm_bs(")
            && let Some((e, m, sm, v)) = sample_at(s, q + 6)
            && let Some(end) = border_tail(s, e, m, sm, v)
        {
            hit = Some((end, m, sm, v));
        }
        if hit.is_none() {
            hit = sample_at(s, q);
        }
        match hit {
            Some((end, member, sampler, var)) => {
                out.push(Sample {
                    start: q,
                    end,
                    member,
                    sampler,
                    var,
                });
                p = end;
            }
            None => p = q + 1,
        }
    }
    out
}

/// `(\w+) (m_\d+) \[\[color\((\d+)\)\]\];` matches in `s` → (type, name, color, start, end).
fn color_outputs(s: &str) -> Vec<(&str, &str, i64, usize, usize)> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < s.len() {
        let Some((q, c)) = s[p..]
            .char_indices()
            .find(|(_, c)| is_word(*c))
            .map(|(k, c)| (p + k, c))
        else {
            break;
        };
        let _ = c;
        let e = word_end(s, q);
        let found = (|| {
            let r = s[e..].strip_prefix(" m_")?;
            let d = digits_end(r, 0);
            if d == 0 {
                return None;
            }
            let name_end = e + 3 + d;
            let r = s[name_end..].strip_prefix(" [[color(")?;
            let k = digits_end(r, 0);
            if k == 0 || !r[k..].starts_with(")]];") {
                return None;
            }
            let color: i64 = r[..k].parse().ok()?;
            Some((&s[q..e], &s[e + 1..name_end], color, name_end + 9 + k + 4))
        })();
        match found {
            Some((t, n, c, end)) => {
                out.push((t, n, c, q, end));
                p = end;
            }
            None => p = e.max(q + 1),
        }
    }
    out
}

/// The `main0_out` struct body span: `re.search(r'struct main0_out\s*\{(.*?)\};', body, re.S)` → (group start, end).
fn out_struct(s: &str) -> Option<(usize, usize)> {
    let pat = "struct main0_out";
    let mut from = 0;
    while let Some(off) = s[from..].find(pat) {
        let at = from + off;
        let j = at + pat.len() + s[at + pat.len()..].len() - s[at + pat.len()..].trim_start().len();
        if s[j..].starts_with('{') {
            let gs = j + 1;
            let ge = gs + s[gs..].find("};")?;
            return Some((gs, ge));
        }
        from = at + 1;
    }
    None
}

/// `variant(msl, bindings, om_only_ok)` → (source, metadata) or the reason.
pub fn variant(msl: &str, bindings: &Value, om_only_ok: bool) -> Result<(String, Value), String> {
    let msl = match msl.find("\nstruct main0_out_u\n") {
        Some(cut) => &msl[..=cut],
        None => msl,
    };
    let tex: BTreeSet<&str> = msl
        .match_indices("texture2d<float> ")
        .filter_map(|(i, m)| {
            let a = i + m.len();
            let b = word_end(msl, a);
            let r = msl[b..].strip_prefix(" [[id(")?;
            let d = digits_end(r, 0);
            (b > a && d > 0 && r[d..].starts_with(")]];")).then(|| &msl[a..b])
        })
        .collect();
    let in_text = msl.split_once("struct main0_in").map_or(msl, |(_, r)| r);
    let in_text = in_text.split_once("};").map_or(in_text, |(a, _)| a);
    let mut locn: BTreeMap<&str, i64> = BTreeMap::new();
    for (i, m) in in_text.match_indices("float4 m_") {
        let a = i + m.len();
        let d = digits_end(in_text, a);
        if d == a {
            continue;
        }
        if let Some(r) = in_text[d..].strip_prefix(" [[user(locn") {
            let k = digits_end(r, 0);
            if k > 0
                && r[k..].starts_with(")]];")
                && let Ok(n) = r[..k].parse()
            {
                locn.insert(&in_text[i + 7..d], n);
            }
        }
    }
    let mut reg: BTreeMap<String, i64> = BTreeMap::new();
    for b in bindings.as_list().unwrap_or(&[]) {
        let name = b.get("msl_name").and_then(Value::as_str).unwrap_or("");
        if b.get("kind").and_then(Value::as_str) == Some("tex")
            && !name.is_empty()
            && let Some(r) = b.get("reg").and_then(Value::as_i64)
        {
            reg.insert(name.to_string(), r);
        }
    }
    let mut calls: Vec<Value> = Vec::new();
    let mut conv: BTreeMap<i64, i64> = BTreeMap::new();
    let mut body = String::with_capacity(msl.len() + 4096);
    let mut pos = 0;
    for m in samples(msl) {
        let Some(&r) = reg.get(m.member) else {
            continue;
        };
        if !tex.contains(m.member) || m.member != m.sampler {
            continue;
        }
        let d = search(msl, &format!("float2 {} = float2(", m.var), |t| {
            let (a, rest) = t.split_once(", ")?;
            let b = underscore_num(rest, 0)?;
            (is_unum(a) && rest[b..].starts_with(");")).then(|| (a, &rest[..b]))
        });
        let a = d.and_then(|(x, _)| in_member(msl, x, 'x'));
        let b = d.and_then(|(_, y)| in_member(msl, y, 'y'));
        let how: Vec<(&str, Value)> = match (a, b) {
            (Some(a), Some(b)) if a == b && locn.contains_key(a) => {
                vec![("varying_locn", Value::Int(locn[a]))]
            }
            _ => {
                let (pa, pb) = d.map_or((None, None), |(x, y)| {
                    (proj(msl, x, 'x', &locn), proj(msl, y, 'y', &locn))
                });
                let (fx, fy) = d.map_or((None, None), |(x, y)| {
                    (frag_pos(msl, x, 'x'), frag_pos(msl, y, 'y'))
                });
                match (pa, pb, fx, fy) {
                    (Some(pa), Some(pb), _, _) if pa == pb => {
                        vec![
                            ("varying_locn", Value::Int(locn[pa])),
                            ("proj", Value::Int(1)),
                        ]
                    }
                    (_, _, Some(fx), Some(fy)) => {
                        let l =
                            |v: [i64; 3]| Value::List(v.iter().map(|x| Value::Int(*x)).collect());
                        vec![("frag_pos", Value::List(vec![l(fx), l(fy)]))]
                    }
                    _ => continue,
                }
            }
        };
        let i = calls.len();
        if i >= 32 {
            break;
        }
        let sel: String = (0..NFB)
            .map(|k| format!("FX_{i} == {k} ? fb{k} : "))
            .collect();
        body.push_str(&msl[pos..m.start]);
        body.push_str(&format!("({sel}{})", &msl[m.start..m.end]));
        pos = m.end;
        let mut c = Value::obj([("fc", Value::Int(i as i64)), ("tex_reg", Value::Int(r))]);
        for (k, v) in how {
            c.set(k, v);
        }
        calls.push(c);
        *conv.entry(r).or_default() += 1;
    }
    if calls.is_empty() && !om_only_ok {
        return Err("no rewritable sample".into());
    }
    let mixed: BTreeSet<i64> = reg
        .iter()
        .filter(|(mb, r)| {
            tex.contains(mb.as_str())
                && msl.matches(&format!("spvDescriptorSet0.{mb}.")).count() as i64
                    > conv.get(r).copied().unwrap_or(0)
        })
        .map(|(_, r)| *r)
        .collect();
    body.push_str(&msl[pos..]);
    let Some((gs, ge)) = out_struct(&body) else {
        return Err("no output struct".into());
    };
    let group = &body[gs..ge];
    let outs = color_outputs(group);
    let mut rest = String::new();
    let mut at = 0;
    for o in &outs {
        rest.push_str(&group[at..o.3]);
        at = o.4;
    }
    rest.push_str(&group[at..]);
    if outs.is_empty() || outs.len() > 4 || !crate::side::strip_py(&rest).is_empty() {
        return Err("the output struct is not plain color outputs (or has more than 4)".into());
    }
    let outs: Vec<(String, String, i64)> = outs
        .iter()
        .map(|o| (o.0.to_string(), o.1.to_string(), o.2))
        .collect();
    let mut members = String::new();
    for (j, (t, n, _)) in outs.iter().enumerate() {
        for k in 0..NFB {
            members.push_str(&format!(
                "    {t} {n}_at{k} [[color({k}), function_constant(OM_{j}_{k})]];\n"
            ));
        }
    }
    let mut body = format!("{}\n{members}{}", &body[..gs], &body[ge..]);
    let mut consts = String::new();
    for c in &calls {
        let fc = c.get("fc").and_then(Value::as_i64).unwrap_or(0);
        consts.push_str(&format!(
            "constant int FX_{fc} [[function_constant({fc})]];\n"
        ));
    }
    for k in 0..NFB {
        consts.push_str(&format!(
            "constant bool FB_{k} [[function_constant({})]];\n",
            32 + k
        ));
    }
    for j in 0..outs.len() {
        for k in 0..NFB {
            consts.push_str(&format!(
                "constant bool OM_{j}_{k} [[function_constant({})]];\n",
                64 + 8 * j + k
            ));
        }
    }
    body = body.replacen(
        "using namespace metal;\n",
        &format!("using namespace metal;\n\n{consts}"),
        1,
    );
    let i0 = body.find("fragment main0_out main0(");
    let i1 = i0.and_then(|i| body[i..].find(")\n{").map(|k| i + k));
    let Some(i1) = i1 else {
        return Err("main function signature not recognized".into());
    };
    let mut fbs: String = (0..NFB)
        .map(|k| format!(", float4 fb{k} [[color({k}), function_constant(FB_{k})]]"))
        .collect();
    if body[..i1].ends_with('(') {
        fbs.drain(..2);
    }
    body.insert_str(i1, &fbs);
    let decl: String = outs
        .iter()
        .map(|(t, n, _)| format!("    {t} spv_o_{n} = {t}(0);\n"))
        .collect();
    body = body.replacen(
        "    main0_out out = {};\n",
        &format!("    main0_out out = {{}};\n{decl}"),
        1,
    );
    for (_, n, _) in &outs {
        body = body.replace(&format!("out.{n}"), &format!("spv_o_{n}"));
    }
    let mut put = String::new();
    for (j, (_, n, _)) in outs.iter().enumerate() {
        for k in 0..NFB {
            put.push_str(&format!("    if (OM_{j}_{k}) out.{n}_at{k} = spv_o_{n};\n"));
        }
    }
    if !body.contains("    return out;\n") {
        return Err("return out not found".into());
    }
    body = body.replace("    return out;\n", &format!("{put}    return out;\n"));
    let meta = Value::obj([
        ("calls", Value::List(calls)),
        (
            "mixed_regs",
            Value::List(mixed.into_iter().map(Value::Int).collect()),
        ),
        (
            "outputs",
            Value::List(
                outs.iter()
                    .map(|(t, _, c)| {
                        Value::obj([("color", Value::Int(*c)), ("type", Value::str(t.clone()))])
                    })
                    .collect(),
            ),
        ),
    ]);
    Ok((body, meta))
}
