//! The gate for `dmsh_build.first_store` (not ported): whether the pass would change a kernel.

use nxb_pyjson::Value;

/// Statement templates of `FIRST_STORE` and `FIRST_STORE_2D`: `{n}` captures `\w+`, `{=n}` repeats capture n,
/// `{#n}` captures digits, statements are separated by `\s*`.
const FS1: &[&str] = &[
    "float {1} = as_type<float>(gl_WorkGroupID.x);",
    "float {2} = as_type<float>(gl_LocalInvocationID.x);",
    "int {3} = as_type<int>({=1}) << {#4};",
    "int {5} = ~as_type<int>({=2});",
    "int {6} = {=5} + 1;",
    "bool {7} = {=6} != {=3};",
];
const FS2: &[&str] = &[
    "float {1} = as_type<float>(gl_WorkGroupID.y);",
    "float {2} = as_type<float>(gl_LocalInvocationID.y);",
    "float {3} = as_type<float>(gl_LocalInvocationID.x);",
    "float {4} = as_type<float>(gl_WorkGroupID.x);",
    "float {5} = as_type<float>(gl_WorkGroupID.z);",
    "int {6} = as_type<int>({=1}) << {#7};",
    "int {8} = ~as_type<int>({=2});",
    "int {9} = {=8} + 1;",
    "bool {10} = {=9} != {=6};",
    "int {11} = as_type<int>({=4}) << {#12};",
    "int {13} = ~as_type<int>({=3});",
    "int {14} = {=13} + 1;",
    "bool {15} = {=14} == {=11};",
    "bool {16} = !{=10};",
    "bool {17} = {=15} && {=16};",
    "bool {18} = as_type<int>({=5}) == 0;|bool {18} = 0 == as_type<int>({=5});",
    "bool {19} = {=18} && {=17};",
];

/// Match one statement template at the start of `t` → bytes consumed.
fn stmt(tpl: &str, t: &str, caps: &mut std::collections::BTreeMap<u32, String>) -> Option<usize> {
    let mut p = 0;
    let mut rest = tpl;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('{') {
            let close = r.find('}')?;
            let spec = &r[..close];
            rest = &r[close + 1..];
            if let Some(n) = spec.strip_prefix('=') {
                let v = caps.get(&n.parse().ok()?)?.clone();
                t[p..].starts_with(&v).then_some(())?;
                p += v.len();
            } else {
                let digits = spec.starts_with('#');
                let n: u32 = spec.trim_start_matches('#').parse().ok()?;
                let e = t[p..]
                    .find(|c: char| {
                        if digits {
                            !c.is_ascii_digit()
                        } else {
                            !(c.is_alphanumeric() || c == '_')
                        }
                    })
                    .unwrap_or(t.len() - p);
                if e == 0 {
                    return None;
                }
                caps.insert(n, t[p..p + e].to_string());
                p += e;
            }
        } else {
            let k = rest.find('{').unwrap_or(rest.len());
            t[p..].starts_with(&rest[..k]).then_some(())?;
            p += k;
            rest = &rest[k..];
        }
    }
    Some(p)
}

/// The first position where the whole template sequence matches → captures.
fn sequence(src: &str, tpls: &[&str]) -> Option<std::collections::BTreeMap<u32, String>> {
    'start: for (i, _) in src.match_indices("float ") {
        let mut caps = std::collections::BTreeMap::new();
        let mut p = i;
        for tpl in tpls {
            let mut ok = None;
            for alt in tpl.split('|') {
                let mut c2 = caps.clone();
                if let Some(n) = stmt(alt, &src[p..], &mut c2) {
                    ok = Some((n, c2));
                    break;
                }
            }
            let Some((n, c2)) = ok else { continue 'start };
            caps = c2;
            p += n;
            p += src[p..].len() - src[p..].trim_start().len();
        }
        return Some(caps);
    }
    None
}

/// Whether `dmsh_build.first_store` would change this kernel (`local[2:] == [1]`, the whole `FIRST_STORE` or
/// `FIRST_STORE_2D` pattern and its size checks). The rest of the pass is not ported: such a kernel is refused.
pub fn first_store_could_apply(src: &str, local: Option<&Value>) -> bool {
    let l: Vec<i64> = local
        .and_then(Value::as_list)
        .map(|l| l.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();
    if l.len() != 3 || l[2] != 1 {
        return false;
    }
    let pow = |caps: &std::collections::BTreeMap<u32, String>, n: u32| {
        caps.get(&n)
            .and_then(|d| d.parse::<u32>().ok())
            .and_then(|d| 1i64.checked_shl(d))
    };
    if let Some(c) = sequence(src, FS1) {
        return l[1] == 1 && pow(&c, 4) == Some(l[0]);
    }
    if let Some(c) = sequence(src, FS2) {
        return pow(&c, 7) == Some(l[1]) && pow(&c, 12) == Some(l[0]);
    }
    false
}
