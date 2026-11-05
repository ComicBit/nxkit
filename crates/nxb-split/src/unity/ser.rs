//! Serialized files: type trees, the object reader and the known layouts (`unityfs.py`: `_tables`, `_read`,
//! `serialized_objects`, `text_assets`, `tree_from_layout`).

use std::collections::HashMap;
use std::sync::Arc;

use crate::SplitError;
use crate::rd::{red, sl};

type R<T> = Result<T, SplitError>;

/// A node of a type tree.
#[derive(Debug, Default)]
pub struct Node {
    /// Type name.
    pub typ: String,
    /// Field name.
    pub name: String,
    /// Meta flags (0x4000: aligned after the field).
    pub flags: i32,
    /// Children.
    pub kids: Vec<Node>,
}

/// Type trees by (class ID, type hash).
pub type Trees = HashMap<(i32, [u8; 16]), Arc<Node>>;

/// A decoded object field.
#[derive(Debug, Clone)]
pub enum Val {
    /// An integer or boolean.
    Int(i128),
    /// A float.
    Float(f64),
    /// Bytes (a string that is not UTF-8, byte arrays).
    Bytes(Vec<u8>),
    /// A string.
    Str(String),
    /// An array.
    List(Vec<Val>),
    /// A struct, in field order (a repeated name keeps its first place and the last value).
    Dict(Vec<(String, Val)>),
}

impl Val {
    /// A field of a struct.
    pub fn get(&self, k: &str) -> Option<&Val> {
        match self {
            Val::Dict(v) => v.iter().find(|x| x.0 == k).map(|x| &x.1),
            _ => None,
        }
    }
    /// The elements of an array (empty for anything else).
    pub fn list(&self) -> &[Val] {
        match self {
            Val::List(v) => v,
            _ => &[],
        }
    }
    /// An integer.
    pub fn int(&self) -> Option<i128> {
        match self {
            Val::Int(v) => Some(*v),
            _ => None,
        }
    }
    /// Bytes (a byte array).
    pub fn bytes(&self) -> &[u8] {
        match self {
            Val::Bytes(v) => v,
            _ => &[],
        }
    }
}

struct Rd<'a> {
    d: &'a [u8],
    p: usize,
    le: bool,
}

impl Rd<'_> {
    fn n(&mut self, n: usize) -> R<u64> {
        let b = self.d.get(self.p..self.p.wrapping_add(n)).ok_or_else(|| {
            SplitError::Red("struct.error: unpack_from requires a buffer of more bytes".into())
        })?;
        self.p += n;
        Ok(if self.le {
            b.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x))
        } else {
            b.iter().fold(0u64, |a, &x| a << 8 | u64::from(x))
        })
    }
    fn i32(&mut self) -> R<i32> {
        Ok(self.n(4)? as u32 as i32)
    }
    fn align(&mut self) {
        self.p = (self.p + 3) & !3;
    }
}

fn prim(t: &str) -> Option<(usize, char)> {
    Some(match t {
        "SInt8" => (1, 'b'),
        "UInt8" | "char" => (1, 'B'),
        "bool" => (1, '?'),
        "SInt16" | "short" => (2, 'h'),
        "UInt16" | "unsigned short" => (2, 'H'),
        "SInt32" | "int" | "Type*" => (4, 'i'),
        "UInt32" | "unsigned int" => (4, 'I'),
        "SInt64" | "long long" => (8, 'q'),
        "UInt64" | "unsigned long long" | "FileSize" => (8, 'Q'),
        "float" => (4, 'f'),
        "double" => (8, 'd'),
        _ => return None,
    })
}

fn one(r: &mut Rd<'_>, w: usize, c: char) -> R<Val> {
    let v = r.n(w)?;
    Ok(match c {
        'f' => Val::Float(f64::from(f32::from_bits(v as u32))),
        'd' => Val::Float(f64::from_bits(v)),
        'b' | 'h' | 'i' | 'q' => Val::Int(
            i128::from(v)
                - if v >> (8 * w - 1) & 1 == 1 {
                    1i128 << (8 * w)
                } else {
                    0
                },
        ),
        _ => Val::Int(i128::from(v)),
    })
}

fn read(r: &mut Rd<'_>, n: &Node) -> R<Val> {
    let v = if let (Some((w, c)), true) = (prim(&n.typ), n.kids.is_empty()) {
        one(r, w, c)?
    } else if n.typ == "string" {
        let ln = r.i32()?;
        let b = sl(r.d, r.p, r.p.saturating_add(ln.max(0) as usize)).to_vec();
        r.p = r.p.saturating_add(ln.max(0) as usize);
        if n.kids.first().is_some_and(|k| k.flags & 0x4000 != 0) {
            r.align();
        }
        match String::from_utf8(b) {
            Ok(s) => Val::Str(s),
            Err(e) => Val::Bytes(e.into_bytes()),
        }
    } else if n.kids.first().is_some_and(|k| k.typ == "Array") {
        let arr = &n.kids[0];
        let cnt = r.i32()?;
        let el = arr
            .kids
            .get(1)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?;
        let v = if matches!(el.typ.as_str(), "UInt8" | "char" | "SInt8") && el.kids.is_empty() {
            let c = cnt.max(0) as usize;
            let b = sl(r.d, r.p, r.p.saturating_add(c)).to_vec();
            r.p = r.p.saturating_add(c);
            Val::Bytes(b)
        } else if let (Some((w, c)), true) = (prim(&el.typ), el.kids.is_empty()) {
            let cnt = cnt.max(0) as usize;
            if cnt
                .checked_mul(w)
                .is_none_or(|x| x > r.d.len().saturating_sub(r.p))
            {
                return red("struct.error: unpack_from requires a buffer of more bytes");
            }
            Val::List((0..cnt).map(|_| one(r, w, c)).collect::<R<_>>()?)
        } else {
            let mut v = Vec::new();
            for _ in 0..cnt.max(0) {
                v.push(read(r, el)?);
            }
            Val::List(v)
        };
        if arr.flags & 0x4000 != 0 {
            r.align();
        }
        v
    } else if n.typ == "TypelessData" {
        let cnt = r.i32()?.max(0) as usize;
        let b = sl(r.d, r.p, r.p.saturating_add(cnt)).to_vec();
        r.p = r.p.saturating_add(cnt);
        Val::Bytes(b)
    } else {
        let mut d: Vec<(String, Val)> = Vec::new();
        for k in &n.kids {
            let v = read(r, k)?;
            match d.iter_mut().find(|x| x.0 == k.name) {
                Some(slot) => slot.1 = v,
                None => d.push((k.name.clone(), v)),
            }
        }
        Val::Dict(d)
    };
    if n.flags & 0x4000 != 0 {
        r.align();
    }
    Ok(v)
}

/// A serialized file's tables: (little-endian, data offset, [(class, type hash, tree)], [(path ID, start, size, type index)]).
type Tables = (
    bool,
    usize,
    Vec<(i32, [u8; 16], Option<Arc<Node>>)>,
    Vec<(i64, usize, usize, usize)>,
);

fn cstr(d: &[u8], p: usize) -> R<(String, usize)> {
    let z = d
        .get(p..)
        .and_then(|x| x.iter().position(|&b| b == 0))
        .ok_or_else(|| SplitError::Red("ValueError: substring not found".into()))?;
    Ok((
        String::from_utf8_lossy(&d[p..p + z]).into_owned(),
        p + z + 1,
    ))
}

fn tables(
    d: &[u8],
    common: &HashMap<u32, String>,
    want: Option<&[i32]>,
    mut trees: Option<&mut Trees>,
) -> R<Tables> {
    let g = |o: usize| -> R<u64> {
        let b = d
            .get(o..o + 4)
            .ok_or_else(|| SplitError::Red("struct.error".into()))?;
        Ok(u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
    };
    let ver = g(8)?;
    if !(17..=22).contains(&ver) {
        return red(format!(
            "ValueError: serialized file format {ver} is not supported"
        ));
    }
    let mut doff = g(12)? as usize;
    let mut p = 16;
    let le = *d
        .get(p)
        .ok_or_else(|| SplitError::Red("IndexError".into()))?
        == 0;
    p += 4;
    if ver >= 22 {
        let q = |o: usize| -> R<usize> {
            Ok(u64::from_be_bytes(
                d.get(o..o + 8)
                    .ok_or_else(|| SplitError::Red("struct.error".into()))?
                    .try_into()
                    .unwrap_or([0; 8]),
            ) as usize)
        };
        doff = q(p + 12)?;
        p += 28;
    }
    let mut r = Rd { d, p, le };
    let (_, np) = cstr(d, r.p)?;
    r.p = np;
    r.i32()?;
    let tt = r.n(1)? != 0;
    let mut tlist = Vec::new();
    for _ in 0..r.i32()? {
        let cid = r.i32()?;
        r.n(1)?;
        r.n(2)?;
        if cid == 114 {
            r.p += 16;
        }
        let mut h = [0u8; 16];
        h.copy_from_slice(
            sl(d, r.p, r.p + 16)
                .get(..16)
                .ok_or_else(|| SplitError::Red("ValueError: type hash".into()))?,
        );
        r.p += 16;
        let mut root = None;
        if tt {
            let (nn, sb) = (r.i32()?, r.i32()?);
            let mut raw = Vec::new();
            for _ in 0..nn.max(0) {
                let (_v, lv, _tf) = (r.n(2)?, r.n(1)?, r.n(1)?);
                let (to, no, bs, _ix, mf) =
                    (r.n(4)? as u32, r.n(4)? as u32, r.i32()?, r.i32()?, r.i32()?);
                if ver >= 19 {
                    r.n(8)?;
                }
                raw.push((lv as usize, to, no, bs, mf));
            }
            let buf = sl(d, r.p, r.p + sb.max(0) as usize);
            r.p += sb.max(0) as usize;
            let st = |o: u32| -> R<String> {
                if o & 0x8000_0000 != 0 {
                    return Ok(common
                        .get(&(o & 0x7fff_ffff))
                        .cloned()
                        .unwrap_or_else(|| format!("?{}", o & 0x7fff_ffff)));
                }
                Ok(cstr(buf, o as usize)?.0)
            };
            // an arena of nodes with their parents, then the owned tree
            let mut arena: Vec<(Node, Vec<usize>)> = Vec::new();
            let mut stack: Vec<usize> = Vec::new();
            for &(lv, to, no, _bs, mf) in &raw {
                arena.push((
                    Node {
                        typ: st(to)?,
                        name: st(no)?,
                        flags: mf,
                        kids: Vec::new(),
                    },
                    Vec::new(),
                ));
                let i = arena.len() - 1;
                stack.truncate(lv);
                if let Some(&par) = stack.last() {
                    arena[par].1.push(i);
                }
                stack.push(i);
            }
            fn build(arena: &mut [(Node, Vec<usize>)], i: usize) -> Node {
                let kids: Vec<usize> = std::mem::take(&mut arena[i].1);
                let mut n = std::mem::take(&mut arena[i].0);
                n.kids = kids.into_iter().map(|k| build(arena, k)).collect();
                n
            }
            if !arena.is_empty() {
                root = Some(Arc::new(build(&mut arena, 0)));
            }
            if ver >= 21 {
                let n = r.i32()?;
                r.p += 4 * n.max(0) as usize;
            }
            if let (Some(t), Some(rt)) = (trees.as_deref_mut(), &root)
                && want.is_none_or(|w| w.contains(&cid))
            {
                t.entry((cid, h)).or_insert_with(|| rt.clone());
            }
        } else if let Some(t) = trees.as_deref() {
            root = t.get(&(cid, h)).cloned();
        }
        tlist.push((cid, h, root));
    }
    let mut objs = Vec::new();
    for _ in 0..r.i32()? {
        r.align();
        let pid = r.n(8)? as i64;
        let start = if ver >= 22 { r.n(8)? } else { r.n(4)? } as usize;
        let (size, ti) = (r.n(4)? as usize, r.i32()?);
        objs.push((pid, start, size, ti.max(0) as usize));
    }
    Ok((le, doff, tlist, objs))
}

/// `serialized_objects`: (class ID, path ID, object) of the wanted classes.`NoTree` (a `Red` naming it) when a class has no tree.
pub fn objects(
    d: &[u8],
    common: &HashMap<u32, String>,
    want: &[i32],
    trees: Option<&mut Trees>,
) -> R<Vec<(i32, i64, Val)>> {
    let (le, doff, tlist, objs) = tables(d, common, Some(want), trees)?;
    let mut out = Vec::new();
    for (pid, start, size, ti) in objs {
        let (cid, h, root) = tlist
            .get(ti)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?;
        if !want.contains(cid) {
            continue;
        }
        let Some(root) = root else {
            return Err(no_tree(*cid, h));
        };
        let mut o = Rd {
            d: sl(d, doff + start, doff + start + size),
            p: 0,
            le,
        };
        out.push((*cid, pid, read(&mut o, root)?));
    }
    Ok(out)
}

/// The marker of `unityfs.NoTree`.
pub const NO_TREE: &str = "NoTree: ";

/// `NoTree`: the class has no type tree and none to borrow.
pub fn no_tree(cid: i32, h: &[u8; 16]) -> SplitError {
    let hx: String = h.iter().map(|b| format!("{b:02x}")).collect();
    SplitError::Red(format!(
        "{NO_TREE}class {cid} (type hash {hx}) has no type tree available"
    ))
}

/// Whether an error is `NoTree`.
pub fn is_no_tree(e: &SplitError) -> bool {
    matches!(e, SplitError::Red(m) if m.starts_with(NO_TREE))
}

/// `text_assets`: (name, script bytes) of the TextAsset objects, read without a type tree.
pub fn text_assets(d: &[u8], common: &HashMap<u32, String>) -> R<Vec<(String, Vec<u8>)>> {
    let (le, doff, tlist, objs) = tables(d, common, Some(&[49]), None)?;
    let mut out = Vec::new();
    for (_pid, start, size, ti) in objs {
        if tlist
            .get(ti)
            .ok_or_else(|| SplitError::Red("IndexError".into()))?
            .0
            != 49
        {
            continue;
        }
        let b = sl(d, doff + start, doff + start + size);
        let mut r = Rd { d: b, p: 0, le };
        if b.len() < 8 {
            continue;
        }
        let ln = r.i32()?;
        if ln < 0 || ln as usize > b.len() - 8 {
            continue;
        }
        let name = String::from_utf8_lossy(sl(b, r.p, r.p + ln as usize)).into_owned();
        r.p += ln as usize;
        r.align();
        if r.p + 4 > b.len() {
            continue;
        }
        let ln = r.i32()?;
        if ln < 0 || ln as usize > b.len() - r.p {
            continue;
        }
        out.push((name, sl(b, r.p, r.p + ln as usize).to_vec()));
    }
    Ok(out)
}

fn split_top(s: &str, sep: char) -> Vec<String> {
    let (mut out, mut depth, mut cur) = (Vec::new(), 0i32, String::new());
    for ch in s.chars() {
        if ch == '<' || ch == '(' {
            depth += 1;
        } else if ch == '>' || ch == ')' {
            depth -= 1;
        }
        if ch == sep && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out
}

/// `tree_from_layout`.
pub fn tree_from_layout(root: &str, layout: &str) -> R<Node> {
    let mut structs: HashMap<&str, Vec<(String, String)>> = HashMap::new();
    for line in layout.trim().split('\n') {
        let (name, body) = line
            .split_once(": ")
            .ok_or_else(|| SplitError::Red("ValueError: layout line".into()))?;
        structs.insert(
            name,
            body.split("; ")
                .filter_map(|f| {
                    f.rsplit_once(' ')
                        .map(|(a, b)| (a.to_string(), b.to_string()))
                })
                .collect(),
        );
    }
    fn node(t: &str, name: &str, structs: &HashMap<&str, Vec<(String, String)>>) -> R<Node> {
        let (mut t, mut flags) = (t, 0);
        if let Some(s) = t.strip_suffix('!') {
            (t, flags) = (s, 0x4000);
        }
        let mut arr_align = 0x4000;
        if let Some(s) = t.strip_suffix('~') {
            (t, arr_align) = (s, 0);
        }
        let array = |el: Node| Node {
            typ: "Array".into(),
            name: "Array".into(),
            flags: arr_align,
            kids: vec![
                Node {
                    typ: "int".into(),
                    name: "size".into(),
                    flags: 0,
                    kids: vec![],
                },
                el,
            ],
        };
        let leaf = |t: &str, n: &str| Node {
            typ: t.into(),
            name: n.into(),
            flags: 0,
            kids: vec![],
        };
        Ok(if t == "string" {
            Node {
                typ: "string".into(),
                name: name.into(),
                flags,
                kids: vec![array(leaf("char", "data"))],
            }
        } else if t.starts_with("vector<")
            || t.starts_with("map<")
            || t.starts_with("staticvector<")
        {
            let (kind, inner) = t[..t.len() - 1].split_once('<').unwrap_or((t, ""));
            let (en, et) = inner
                .split_once(':')
                .ok_or_else(|| SplitError::Red("ValueError: layout element".into()))?;
            Node {
                typ: kind.into(),
                name: name.into(),
                flags,
                kids: vec![array(node(et, en, structs)?)],
            }
        } else if let Some(p) = t.strip_prefix("pair(") {
            let parts = split_top(&p[..p.len() - 1], ',');
            Node {
                typ: "pair".into(),
                name: name.into(),
                flags,
                kids: vec![
                    node(&parts[0], "first", structs)?,
                    node(&parts[1], "second", structs)?,
                ],
            }
        } else if let Some(fields) = structs.get(t) {
            Node {
                typ: t.into(),
                name: name.into(),
                flags,
                kids: fields
                    .iter()
                    .map(|(ft, fnm)| node(ft, fnm, structs))
                    .collect::<R<_>>()?,
            }
        } else if prim(t).is_some() {
            Node {
                typ: t.into(),
                name: name.into(),
                flags,
                kids: vec![],
            }
        } else {
            return red(format!("ValueError: type tree layout: unknown type {t}"));
        })
    }
    node(root, "Base", &structs)
}

/// `unityfs.KNOWN_TREES` as trees.
pub fn known_trees() -> R<Trees> {
    let mut t = Trees::new();
    for (cid, h, root, layout) in super::trees::KNOWN {
        t.insert((*cid, *h), Arc::new(tree_from_layout(root, layout)?));
    }
    Ok(t)
}
