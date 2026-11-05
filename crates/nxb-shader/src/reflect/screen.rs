//! `dmsh_fetch.screen_varyings` with its `_lin` and `_entry_screen`.

use super::is_word;
use std::collections::{BTreeMap, BTreeSet};

type Lin = BTreeMap<String, f64>;

pub(super) fn word_end(s: &str, i: usize) -> usize {
    s[i..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(s.len(), |(k, _)| i + k)
}

/// `_TOK`: `\s*(?:(out\.gl_Position\.[xyzw])|(_\d+)|(\d+\.\d*(?:e[+-]?\d+)?|\d+)f?|(fma)|([-+*/(),]))`.
fn tokens(e: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < e.len() {
        let q = p + (e[p..].len() - e[p..].trim_start().len());
        let r = &e[q..];
        if r.is_empty() {
            return None;
        }
        if let Some(x) = r.strip_prefix("out.gl_Position.")
            && x.starts_with(['x', 'y', 'z', 'w'])
        {
            out.push(r[..17].to_string());
            p = q + 17;
            continue;
        }
        if let Some(after) = r.strip_prefix('_') {
            let d = after.bytes().take_while(u8::is_ascii_digit).count();
            if d > 0 {
                out.push(r[..1 + d].to_string());
                p = q + 1 + d;
                continue;
            }
        }
        let d = r.bytes().take_while(u8::is_ascii_digit).count();
        if d > 0 {
            let mut n = d;
            if r[n..].starts_with('.') {
                n += 1 + r[n + 1..].bytes().take_while(u8::is_ascii_digit).count();
                if r[n..].starts_with('e') {
                    let s = n + 1 + usize::from(r[n + 1..].starts_with(['+', '-']));
                    let x = r[s..].bytes().take_while(u8::is_ascii_digit).count();
                    if x > 0 {
                        n = s + x;
                    }
                }
            }
            out.push(format!("#{}", &r[..n]));
            p = q + n + usize::from(r[n..].starts_with('f'));
            continue;
        }
        if r.starts_with("fma") {
            out.push("fma".into());
            p = q + 3;
            continue;
        }
        let c = r.chars().next()?;
        if "-+*/(),".contains(c) {
            out.push(c.to_string());
            p = q + 1;
            continue;
        }
        return None;
    }
    Some(out)
}

fn add(a: &Lin, b: &Lin, s: f64) -> Lin {
    let mut r = a.clone();
    for (k, v) in b {
        *r.entry(k.clone()).or_insert(0.0) += s * v;
    }
    r.into_iter().filter(|(_, v)| *v != 0.0).collect()
}

fn is_const(a: &Lin) -> bool {
    a.keys().all(|k| k == "1")
}

fn scale(a: &Lin, c: f64) -> Lin {
    a.iter()
        .filter(|(_, v)| **v * c != 0.0)
        .map(|(k, v)| (k.clone(), v * c))
        .collect()
}

struct P<'a, 'e> {
    t: Vec<String>,
    i: usize,
    env: &'a mut dyn FnMut(&str) -> Option<Lin>,
    _e: std::marker::PhantomData<&'e ()>,
}

impl P<'_, '_> {
    fn peek(&self) -> Option<&str> {
        self.t.get(self.i).map(String::as_str)
    }
    fn take(&mut self, want: Option<&str>) -> Option<String> {
        let x = self.t.get(self.i)?.clone();
        if want.is_some_and(|w| w != x) {
            return None;
        }
        self.i += 1;
        Some(x)
    }
    fn mul(a: &Lin, b: &Lin) -> Option<Lin> {
        if is_const(a) {
            return Some(scale(b, a.get("1").copied().unwrap_or(0.0)));
        }
        if is_const(b) {
            return Some(scale(a, b.get("1").copied().unwrap_or(0.0)));
        }
        None
    }
    fn atom(&mut self) -> Option<Lin> {
        let t = self.take(None)?;
        if t == "-" {
            return Some(scale(&self.atom()?, -1.0));
        }
        if t == "(" {
            let v = self.sum()?;
            self.take(Some(")"))?;
            return Some(v);
        }
        if t == "fma" {
            self.take(Some("("))?;
            let a = self.sum()?;
            self.take(Some(","))?;
            let b = self.sum()?;
            self.take(Some(","))?;
            let c = self.sum()?;
            self.take(Some(")"))?;
            return Some(add(&Self::mul(&a, &b)?, &c, 1.0));
        }
        if let Some(n) = t.strip_prefix('#') {
            let v: f64 = n.parse().ok()?;
            let mut m = Lin::new();
            if v != 0.0 {
                m.insert("1".into(), v);
            }
            return Some(m);
        }
        (self.env)(&t)
    }
    fn product(&mut self) -> Option<Lin> {
        let mut v = self.atom()?;
        while matches!(self.peek(), Some("*" | "/")) {
            let op = self.take(None)?;
            let w = self.atom()?;
            if op == "*" {
                v = Self::mul(&v, &w)?;
            } else {
                let c = w.get("1").copied().filter(|c| *c != 0.0 && is_const(&w))?;
                v = scale(&v, 1.0 / c);
            }
        }
        Some(v)
    }
    fn sum(&mut self) -> Option<Lin> {
        let mut v = self.product()?;
        while matches!(self.peek(), Some("+" | "-")) {
            let op = self.take(None)?;
            let w = self.product()?;
            v = add(&v, &w, if op == "+" { 1.0 } else { -1.0 });
        }
        Some(v)
    }
}

/// `_lin(expr, env)`.
fn lin(e: &str, env: &mut dyn FnMut(&str) -> Option<Lin>) -> Option<Lin> {
    let t = tokens(e)?;
    let mut p = P {
        t,
        i: 0,
        env,
        _e: std::marker::PhantomData,
    };
    let v = p.sum()?;
    p.peek().is_none().then_some(v)
}

/// `(?:float|int|uint|bool|half) (_\d+) = (.+);` fullmatch.
fn decl(s: &str) -> Option<(&str, &str)> {
    let r = ["float ", "int ", "uint ", "bool ", "half "]
        .iter()
        .find_map(|p| s.strip_prefix(p))?;
    let d = r
        .strip_prefix('_')
        .map(|x| x.bytes().take_while(u8::is_ascii_digit).count())?;
    if d == 0 {
        return None;
    }
    let e = r[1 + d..].strip_prefix(" = ")?.strip_suffix(';')?;
    (!e.is_empty()).then(|| (&r[..1 + d], e))
}

type Last = BTreeMap<(String, char), (usize, i64, Option<String>)>;

fn entry_screen(body: &str, locn: &[(String, i64)]) -> BTreeSet<i64> {
    let none = BTreeSet::new();
    let returns = body.match_indices("return").filter(|(i, _)| {
        !body[..*i].chars().next_back().is_some_and(is_word)
            && !body[i + 6..].chars().next().is_some_and(is_word)
    });
    if returns.count() != 1 || !(format!("{body}\n")).ends_with("\n    return out;\n") {
        return none;
    }
    let lines: Vec<&str> = body.split('\n').map(str::trim).collect();
    for (n, s) in lines.iter().enumerate() {
        let w = word_end(s, 0);
        let kw = &s[..w];
        if ["if", "else", "for", "while", "do", "switch"].contains(&kw)
            && (n + 1 >= lines.len() || lines[n + 1] != "{")
        {
            return none;
        }
    }
    let mut depth = 0i64;
    let mut stmts = Vec::new();
    for s in &lines {
        match *s {
            "{" => depth += 1,
            "}" => depth -= 1,
            "" => {}
            _ => stmts.push((depth, *s)),
        }
    }
    let mut decls: BTreeMap<String, (usize, i64, String)> = BTreeMap::new();
    let mut last: Last = BTreeMap::new();
    let mut nassign: BTreeMap<String, usize> = BTreeMap::new();
    for (i, _) in body.match_indices('_') {
        if i > 0
            && body[..i]
                .chars()
                .next_back()
                .is_some_and(|c| is_word(c) || c == '.')
        {
            continue;
        }
        let d = body[i + 1..].bytes().take_while(u8::is_ascii_digit).count();
        if d > 0 && body[i + 1 + d..].starts_with(" = ") {
            *nassign.entry(body[i..i + 1 + d].to_string()).or_default() += 1;
        }
    }
    for (n, (dp, s)) in stmts.iter().enumerate() {
        if let Some((v, e)) = decl(s) {
            decls.insert(v.to_string(), (n, *dp, e.to_string()));
            continue;
        }
        if let Some(r) = s.strip_prefix("out.") {
            let e = word_end(r, 0);
            let mem = &r[..e];
            let okm = mem == "gl_Position"
                || (mem.starts_with("m_")
                    && mem.len() > 2
                    && mem[2..].bytes().all(|c| c.is_ascii_digit()));
            if !okm {
                continue;
            }
            let t = &r[e..];
            if let Some(x) = t.strip_prefix('.')
                && let Some(c) = x.chars().next()
                && "xyzw".contains(c)
                && let Some(v) = x[1..].strip_prefix(" = ").and_then(|v| v.strip_suffix(';'))
                && !v.is_empty()
            {
                last.insert((mem.to_string(), c), (n, *dp, Some(v.to_string())));
                continue;
            }
            if let Some(v) = t
                .strip_prefix(" = float4(")
                .and_then(|v| v.strip_suffix(");"))
                && !v.is_empty()
            {
                let parts: Vec<String> = v.split(',').map(|p| p.trim().to_string()).collect();
                for (k, c) in "xyzw".chars().enumerate() {
                    let p = if parts.len() == 4 {
                        Some(parts[k].clone())
                    } else {
                        None
                    };
                    last.insert((mem.to_string(), c), (n, *dp, p));
                }
            }
        }
    }
    let mut pos: BTreeMap<char, (usize, i64, String)> = BTreeMap::new();
    for c in ['x', 'y', 'w'] {
        match last.get(&("gl_Position".to_string(), c)) {
            Some((n, 0, Some(e))) => {
                pos.insert(c, (*n, 0, e.clone()));
            }
            _ => return none,
        }
    }
    let mut sym: BTreeMap<String, String> = BTreeMap::new();
    for (c, p) in &pos {
        let e = &p.2;
        if e.len() > 1 && e.starts_with('_') && e[1..].bytes().all(|b| b.is_ascii_digit()) {
            sym.insert(e.clone(), format!("P{c}"));
        }
    }
    fn env_at(
        at: usize,
        name: &str,
        pos: &BTreeMap<char, (usize, i64, String)>,
        sym: &BTreeMap<String, String>,
        decls: &BTreeMap<String, (usize, i64, String)>,
        nassign: &BTreeMap<String, usize>,
        memo: &mut BTreeMap<(usize, String), Option<Lin>>,
    ) -> Option<Lin> {
        if let Some(c) = name
            .strip_prefix("out.gl_Position.")
            .and_then(|x| x.chars().next())
        {
            let p = pos.get(&c)?;
            return (p.0 < at).then(|| [(format!("P{c}"), 1.0)].into_iter().collect());
        }
        if let Some(s) = sym.get(name) {
            return Some([(s.clone(), 1.0)].into_iter().collect());
        }
        let key = (at, name.to_string());
        if let Some(v) = memo.get(&key) {
            return v.clone();
        }
        let d = decls.get(name)?;
        if d.0 >= at || nassign.get(name).copied().unwrap_or(0) != 1 {
            return None;
        }
        memo.insert(key.clone(), None);
        let (dn, expr) = (d.0, d.2.clone());
        let v = lin(&expr, &mut |n: &str| {
            env_at(dn, n, pos, sym, decls, nassign, memo)
        });
        memo.insert(key, v.clone());
        v
    }
    let want: [(char, Lin); 3] = [
        (
            'x',
            [("Px".to_string(), 0.5), ("Pw".to_string(), 0.5)]
                .into_iter()
                .collect(),
        ),
        (
            'y',
            [("Py".to_string(), -0.5), ("Pw".to_string(), 0.5)]
                .into_iter()
                .collect(),
        ),
        ('w', [("Pw".to_string(), 1.0)].into_iter().collect()),
    ];
    let mut got = BTreeSet::new();
    for (mem, k) in locn {
        let mut ok = true;
        for (c, w) in &want {
            let Some((n, 0, Some(e))) = last.get(&(mem.clone(), *c)).cloned() else {
                ok = false;
                break;
            };
            let mut memo = BTreeMap::new();
            let v = lin(&e, &mut |nm: &str| {
                env_at(n, nm, &pos, &sym, &decls, &nassign, &mut memo)
            });
            if v.as_ref() != Some(w) {
                ok = false;
                break;
            }
        }
        if ok {
            got.insert(*k);
        }
    }
    got
}

/// `dmsh_fetch.screen_varyings(msl)`.
pub fn screen_varyings(msl: &str) -> Vec<i64> {
    let ost = crate::msl::out_group(msl).map_or("", |(a, b)| &msl[a..b]);
    let mut locn: Vec<(String, i64)> = Vec::new();
    for (i, m) in ost.match_indices("float4 m_") {
        let r = &ost[i + 7..];
        let e = word_end(r, 0);
        if !r[..e][2..].bytes().all(|c| c.is_ascii_digit()) || e <= 2 {
            continue;
        }
        let _ = m;
        if let Some(x) = r[e..].strip_prefix(" [[user(locn") {
            let d = x.bytes().take_while(u8::is_ascii_digit).count();
            if d > 0 && x[d..].starts_with(")]]") {
                let n = r[..e].to_string();
                let l = x[..d].parse().unwrap_or(0);
                match locn.iter_mut().find(|(a, _)| *a == n) {
                    Some(s) => s.1 = l,
                    None => locn.push((n, l)),
                }
            }
        }
    }
    // \nvertex main0_out \w+\(.*?\)\n\{\n(.*?)\n\}\n (re.S), non-overlapping.
    let mut bodies = Vec::new();
    let mut from = 0;
    while let Some(off) = msl[from..].find("\nvertex main0_out ") {
        let i = from + off + "\nvertex main0_out ".len();
        let e = word_end(msl, i);
        from = i;
        if e == i || !msl[e..].starts_with('(') {
            continue;
        }
        let Some(h) = msl[e..].find(")\n{\n").map(|k| e + k + 4) else {
            continue;
        };
        let Some(b) = msl[h..].find("\n}\n").map(|k| h + k) else {
            continue;
        };
        bodies.push(&msl[h..b]);
        from = b + 3;
    }
    if locn.is_empty() || bodies.is_empty() {
        return Vec::new();
    }
    let mut got: BTreeSet<i64> = locn.iter().map(|x| x.1).collect();
    for b in bodies {
        let s = entry_screen(b, &locn);
        got = got.intersection(&s).copied().collect();
    }
    got.into_iter().collect()
}
