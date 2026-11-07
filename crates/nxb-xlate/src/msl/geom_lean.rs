//! The lean geometry kernels `_bl0`, `_bl1` and `_bl3` (`dmsh_geometry.lean`).
//!
//! A passthrough geometry shader without adjacency input gets a lean kernel for each of these input topologies. The
//! lean kernel is the full kernel `_bg<t>` without its copy of the vertex. It writes only the viewport mask, one word
//! per output vertex, and the indices. It reads only the capture words that it still needs, from a record of that
//! many words, in the order of `capture_reads`. The host draws the game's own vertex shader through the indices.
//!
//! The transform works on the MSL text of the full kernel. The output stores and the capture reads must use the
//! literal offsets that the emitter writes. Other input gives an error, and the shader keeps only the full kernel.

use super::refl::Reflection;
use super::variant::vertex::CAPTURE_WORDS;
use nxb_pyjson::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The word of the viewport mask in an output vertex (`dm_gs_viewport_mask` of the reader).
pub const MASK_WORD: i64 = 5;

const STORE: &str = "(*spvDescriptorSet0.geometry_vb_output)._m0[";
const READ: &str = "(*spvDescriptorSet0.vertex_output)._m0[";
const VOLATILE: &str = "const device volatile vertex_output_block*";
const PLAIN: &str = "const device vertex_output_block*";
const SIDE_EFFECTS: [&str; 10] = [
    "atomic",
    "simd_",
    "Subgroup",
    "quad_",
    "barrier",
    "++",
    "--",
    "dm_pool_mark",
    "dm_body",
    ".write(",
];

/// A lean kernel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lean {
    /// The MSL source.
    pub msl: String,
    /// The capture words the kernel reads, sorted. Word `reads[i]` is at index `i` of a record.
    pub reads: Vec<i64>,
}

/// One line of the kernel body.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    /// `T name;`, `T name[N];` or `T name = <literal>;`.
    Decl(String),
    /// `name = expr;`, `T name = expr;` or `name[K] = expr;`.
    Def {
        lhs: String,
        name: String,
        index: Option<u32>,
        expr: String,
    },
    /// A statement or control flow.
    Other,
    /// A line that the transform removed.
    Removed,
}

/// A local variable of the emitter: `v<n>`, `t<n>` (with a letter suffix) or the local memory `lm<n>`.
fn is_var(s: &str) -> bool {
    let rest = s
        .strip_prefix("lm")
        .or_else(|| s.strip_prefix('v'))
        .or_else(|| s.strip_prefix('t'));
    let Some(rest) = rest else { return false };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && rest[digits..].bytes().all(|b| b.is_ascii_lowercase())
}

/// The variables of a text. An element of a local array in `arrays` at a literal index is the token `name[K]`.
fn tokens(text: &str, arrays: &BTreeSet<String>) -> Vec<String> {
    let b = text.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !word(b[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && word(b[i]) {
            i += 1;
        }
        let name = &text[start..i];
        if b[start].is_ascii_digit() || !is_var(name) {
            continue;
        }
        let element = arrays
            .contains(name)
            .then(|| text[i..].strip_prefix('['))
            .flatten()
            .and_then(|r| r.split_once(']'))
            .filter(|(k, _)| !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()));
        match element {
            Some((k, _)) => out.push(format!("{name}[{k}]")),
            None => out.push(name.to_string()),
        }
    }
    out
}

/// The expression has no side effect: no atomic, no barrier, no call of a function of the shader, no assignment.
fn pure(expr: &str) -> bool {
    if SIDE_EFFECTS.iter().any(|s| expr.contains(s)) {
        return false;
    }
    let b = expr.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| b[j]);
        if c == b'='
            && !matches!(prev, Some(b'=' | b'!' | b'<' | b'>'))
            && b.get(i + 1) != Some(&b'=')
        {
            return false;
        }
    }
    let mut rest = expr;
    while let Some(at) = rest.find("fn") {
        let before = expr.len() - rest.len() + at;
        let free = before == 0 || !expr.as_bytes()[before - 1].is_ascii_alphanumeric();
        let tail = &rest[at + 2..];
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if free && digits > 0 && tail[digits..].starts_with('(') {
            return false;
        }
        rest = tail;
    }
    true
}

fn parse(line: &str) -> Line {
    let t = line.trim();
    let Some(stmt) = t.strip_suffix(';') else {
        return Line::Other;
    };
    let (lhs, expr) = match stmt.split_once(" = ") {
        Some((l, e)) => (l, Some(e)),
        None => (stmt, None),
    };
    let (ty, target) = match lhs.rsplit_once(' ') {
        Some((ty, n)) => (Some(ty), n),
        None => (None, lhs),
    };
    if ty.is_some_and(|ty| !ty.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')) {
        return Line::Other;
    }
    let (name, index) = match target.split_once('[') {
        Some((n, k)) => match k.strip_suffix(']').and_then(|k| k.parse::<u32>().ok()) {
            Some(k) => (n, Some(k)),
            None => return Line::Other,
        },
        None => (target, None),
    };
    if !is_var(name) {
        return Line::Other;
    }
    match (ty, expr) {
        (Some(_), None) => Line::Decl(name.into()),
        (None, None) => Line::Other,
        (Some(_), Some(_)) if index.is_some() => Line::Other,
        (Some(_), Some(e)) if tokens(e, &BTreeSet::new()).is_empty() => Line::Decl(name.into()),
        (_, Some(e)) => Line::Def {
            lhs: lhs.into(),
            name: name.into(),
            index,
            expr: e.into(),
        },
    }
}

/// `(a op b)` without the outer parentheses.
fn bare(e: &str) -> &str {
    e.strip_prefix('(')
        .and_then(|e| e.strip_suffix(')'))
        .unwrap_or(e)
}

struct Kernel {
    lines: Vec<String>,
    parsed: Vec<Line>,
    /// The line of `kernel void main0(`.
    start: usize,
}

impl Kernel {
    fn new(src: &str) -> Result<Kernel, String> {
        let lines: Vec<String> = src.split('\n').map(str::to_string).collect();
        let start = lines
            .iter()
            .position(|l| l.starts_with("kernel void main0("))
            .ok_or("lean geometry: no kernel entry")?;
        let parsed = lines
            .iter()
            .enumerate()
            .map(|(i, l)| if i > start { parse(l) } else { Line::Other })
            .collect();
        Ok(Kernel {
            lines,
            parsed,
            start,
        })
    }

    /// The expression of the single definition of a scalar variable.
    fn single(&self, var: &str) -> Option<(usize, &str)> {
        let mut found = None;
        for (i, p) in self.parsed.iter().enumerate() {
            if let Line::Def {
                name, index, expr, ..
            } = p
                && name == var
            {
                if found.is_some() || index.is_some() {
                    return None;
                }
                found = Some((i, expr.as_str()));
            }
        }
        found
    }

    /// `var = x * stride`: `x`.
    fn base(&self, var: &str, stride: i64) -> Option<String> {
        let (_, e) = self.single(var)?;
        let (x, s) = bare(e).split_once(" * ")?;
        (is_var(x) && s.parse::<i64>().ok() == Some(stride)).then(|| x.to_string())
    }

    /// `var = b + k` with `b = x * stride`, or `var` itself a base: `(b, k)`.
    fn offset(&self, var: &str, stride: i64) -> Option<(String, i64)> {
        if self.base(var, stride).is_some() {
            return Some((var.to_string(), 0));
        }
        let (_, e) = self.single(var)?;
        let (b, k) = bare(e).split_once(" + ")?;
        let k = k.parse::<i64>().ok().filter(|k| *k >= 0)?;
        self.base(b, stride)?;
        Some((b.to_string(), k))
    }

    fn rewrite(&mut self, i: usize, new: String) {
        if let Some(Line::Def { lhs, expr, .. }) = self.parsed.get_mut(i) {
            let indent = &self.lines[i][..self.lines[i].len() - self.lines[i].trim_start().len()];
            self.lines[i] = format!("{indent}{lhs} = {new};");
            *expr = new;
        }
    }

    fn remove(&mut self, i: usize) {
        self.parsed[i] = Line::Removed;
    }

    /// The variables each body line reads. The target of a definition is no read.
    fn reads(&self, i: usize, arrays: &BTreeSet<String>) -> Vec<String> {
        match &self.parsed[i] {
            Line::Def { expr, .. } => tokens(expr, arrays),
            Line::Other if i > self.start => tokens(&self.lines[i], arrays),
            _ => Vec::new(),
        }
    }

    /// How often each variable is read in the body.
    fn uses(&self) -> BTreeMap<String, usize> {
        let mut uses = BTreeMap::new();
        for i in self.start + 1..self.lines.len() {
            for t in self.reads(i, &BTreeSet::new()) {
                *uses.entry(t).or_insert(0) += 1;
            }
        }
        uses
    }
}

/// The lean kernel of the full kernel `src`, whose output vertices are `out_stride` words.
pub fn lean(src: &str, out_stride: i64) -> Result<Lean, String> {
    let mut k = Kernel::new(src)?;
    stores(&mut k, out_stride)?;
    dead_code(&mut k);
    let reads = capture(&mut k)?;
    unused_declarations(&mut k);
    let msl = k
        .lines
        .iter()
        .zip(&k.parsed)
        .filter(|(_, p)| **p != Line::Removed)
        .map(|(l, _)| l.replacen(VOLATILE, PLAIN, 1))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Lean { msl, reads })
}

/// Remove the output stores but the viewport mask. The mask store then writes word `x` for output vertex `x`.
fn stores(k: &mut Kernel, stride: i64) -> Result<(), String> {
    const UNKNOWN: &str = "lean geometry: unknown output store";
    let mut kept: BTreeMap<String, String> = BTreeMap::new();
    let mut gone = Vec::new();
    for (i, l) in k.lines.iter().enumerate() {
        let t = l.trim();
        if !t.contains("geometry_vb_output") || t.starts_with("struct ") || t.starts_with("device ")
        {
            continue;
        }
        let idx = t
            .strip_prefix(STORE)
            .and_then(|r| r.split_once("] = "))
            .and_then(|(idx, v)| Some((idx, v.strip_suffix(';')?)))
            .filter(|(idx, v)| i > k.start && is_var(idx) && pure(v))
            .map(|(idx, _)| idx)
            .ok_or(UNKNOWN)?;
        let (b, w) = k.offset(idx, stride).ok_or(UNKNOWN)?;
        if w == MASK_WORD {
            kept.insert(idx.to_string(), b);
        } else {
            gone.push(i);
        }
    }
    if kept.is_empty() {
        return Err("lean geometry: no viewport mask store".into());
    }
    for i in gone {
        k.remove(i);
    }
    let uses = k.uses();
    for (idx, b) in &kept {
        let prefix = format!("{STORE}{idx}]");
        let stores = k
            .lines
            .iter()
            .zip(&k.parsed)
            .filter(|(l, p)| **p == Line::Other && l.trim().starts_with(&prefix))
            .count();
        if uses.get(idx) != Some(&stores) {
            return Err(UNKNOWN.into());
        }
        let (j, _) = k.single(idx).ok_or(UNKNOWN)?;
        k.rewrite(j, format!("({b} / {stride})"));
    }
    Ok(())
}

/// Remove the definitions nobody reads: pure definitions of variables and the elements of local arrays at a literal
/// index. An array that is used whole or at a computed index keeps every element.
fn dead_code(k: &mut Kernel) {
    let body = k.start + 1..k.lines.len();
    let mut arrays: BTreeSet<String> = BTreeSet::new();
    for i in body.clone() {
        if let Line::Decl(n) = &k.parsed[i]
            && k.lines[i].trim_end().ends_with("];")
        {
            arrays.insert(n.clone());
        }
    }
    let mut whole = BTreeSet::new();
    for i in body.clone() {
        let mut seen = k.reads(i, &arrays);
        if let Line::Def {
            name, index: None, ..
        } = &k.parsed[i]
        {
            seen.push(name.clone());
        }
        whole.extend(seen.into_iter().filter(|t| arrays.contains(t)));
    }
    arrays.retain(|a| !whole.contains(a));
    let mut target: Vec<Option<String>> = vec![None; k.lines.len()];
    let mut rvalues: Vec<Vec<String>> = vec![Vec::new(); k.lines.len()];
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    let mut writers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for i in body {
        let reads = k.reads(i, &arrays);
        if let Line::Def {
            name, index, expr, ..
        } = &k.parsed[i]
            && pure(expr)
        {
            let t = match index {
                None if !arrays.contains(name) => Some(name.clone()),
                Some(x) if arrays.contains(name) => Some(format!("{name}[{x}]")),
                _ => None,
            };
            if let Some(t) = t {
                writers.entry(t.clone()).or_default().push(i);
                target[i] = Some(t);
            }
        }
        for t in &reads {
            *uses.entry(t.clone()).or_insert(0) += 1;
        }
        rvalues[i] = reads;
    }
    let mut work: Vec<usize> = (0..k.lines.len())
        .filter(|i| target[*i].as_ref().is_some_and(|t| !uses.contains_key(t)))
        .collect();
    while let Some(i) = work.pop() {
        if k.parsed[i] == Line::Removed {
            continue;
        }
        k.remove(i);
        for t in &rvalues[i] {
            if let Some(n) = uses.get_mut(t) {
                *n -= 1;
                if *n == 0 {
                    work.extend(writers.get(t).into_iter().flatten());
                }
            }
        }
    }
}

/// Check the capture reads that remain and make them read a record of `reads.len()` words.
fn capture(k: &mut Kernel) -> Result<Vec<i64>, String> {
    let words = i64::from(CAPTURE_WORDS);
    let mut found: Vec<(String, String, i64)> = Vec::new();
    for (i, l) in k.lines.iter().enumerate() {
        let t = l.trim();
        if !t.contains("vertex_output")
            || k.parsed[i] == Line::Removed
            || t.starts_with("struct ")
            || (i < k.start && t.contains("[[id("))
        {
            continue;
        }
        let n = match &k.parsed[i] {
            Line::Def {
                index: None, expr, ..
            } => expr.strip_prefix(READ).and_then(|r| r.strip_suffix(']')),
            _ => None,
        }
        .filter(|n| is_var(n))
        .ok_or("lean geometry: unknown capture read")?;
        let (b, w) = k
            .offset(n, words)
            .ok_or("lean geometry: computed capture read")?;
        found.push((n.to_string(), b, w));
    }
    let reads: Vec<i64> = found
        .iter()
        .map(|f| f.2)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let uses = k.uses();
    let bases: BTreeSet<&String> = found.iter().map(|f| &f.1).collect();
    let offsets: BTreeSet<&String> = found.iter().filter(|f| f.0 != f.1).map(|f| &f.0).collect();
    for v in bases.iter().chain(&offsets) {
        let reads_of = found.iter().filter(|f| &&f.0 == v || &&f.1 == v).count();
        if uses.get(v.as_str()).copied().unwrap_or(0) > reads_of {
            return Err("lean geometry: record base used elsewhere".into());
        }
    }
    let mut edits = Vec::new();
    for n in &offsets {
        let Some((b, w)) = found.iter().find(|f| &&f.0 == n).map(|f| (&f.1, f.2)) else {
            continue;
        };
        let slot = reads.iter().position(|r| *r == w).unwrap_or(0);
        if let Some((j, _)) = k.single(n) {
            edits.push((j, format!("({b} + {slot})")));
        }
    }
    let len = reads.len().max(1);
    for b in &bases {
        if let (Some((j, _)), Some(x)) = (k.single(b), k.base(b, words)) {
            edits.push((j, format!("({x} * {len})")));
        }
    }
    for (j, e) in edits {
        k.rewrite(j, e);
    }
    Ok(reads)
}

/// Remove the declarations of variables that no line uses any more.
fn unused_declarations(k: &mut Kernel) {
    let mut named: BTreeSet<String> = BTreeSet::new();
    for i in k.start + 1..k.lines.len() {
        match &k.parsed[i] {
            Line::Def { name, expr, .. } => {
                named.insert(name.clone());
                named.extend(tokens(expr, &BTreeSet::new()));
            }
            Line::Other => named.extend(tokens(&k.lines[i], &BTreeSet::new())),
            _ => {}
        }
    }
    for i in k.start + 1..k.lines.len() {
        if matches!(&k.parsed[i], Line::Decl(n) if !named.contains(n)) {
            k.remove(i);
        }
    }
}

/// The reflection of the lean kernel `_bl<t>` from the reflection of the full kernel `_bg<t>`.
pub fn reflection(full: &Reflection, l: &Lean, t: i32, key: &str, geometry: Value) -> Reflection {
    let mut r = full.clone();
    r.key = format!("{key}_bl{t}");
    let mut g = geometry;
    if let Value::Object(fields) = &mut g {
        fields.retain(|(k, _)| k != "stride");
        fields.push(("stride".into(), Value::Int(1)));
        fields.push(("lean".into(), Value::Bool(true)));
        fields.push((
            "capture_reads".into(),
            Value::List(l.reads.iter().map(|w| Value::Int(*w)).collect()),
        ));
    }
    r.set_extra("geometry", g);
    r.drv_reads
        .retain(|d| l.msl.contains(&format!("fp_c{})", d.bank)));
    for b in &mut r.bindings {
        if b.msl_type == VOLATILE {
            b.msl_type = PLAIN.into();
        }
    }
    r
}
