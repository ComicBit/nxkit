//! A small TrueType reader and writer: exactly what `make_ext_font.py` needs from fontTools (the letter font's simple
//! glyphs and the `FontBuilder` output for 10 tables, written the way fontTools 4.63 writes them).
#![allow(missing_docs, unreachable_pub, clippy::unwrap_used)]

use std::collections::BTreeMap;

/// `table.recalcBounds` + `compile` input: a glyph in TrueType form (rounded points, `true` = on-curve).
#[derive(Default, Clone)]
pub struct Glyph {
    pub pts: Vec<(i32, i32, bool)>,
    pub ends: Vec<u16>,
}

fn pad(v: &mut Vec<u8>, to: usize) {
    while !v.len().is_multiple_of(to) {
        v.push(0);
    }
}

impl Glyph {
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let mut b = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for p in &self.pts {
            b = (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1));
        }
        if self.pts.is_empty() { (0, 0, 0, 0) } else { b }
    }

    /// `Glyph.compile` with fontTools' greedy flag and delta packing.
    pub fn compile(&self) -> Vec<u8> {
        if self.ends.is_empty() {
            return vec![];
        }
        let b = self.bounds();
        let mut d = vec![];
        d.extend((self.ends.len() as i16).to_be_bytes());
        for v in [b.0, b.1, b.2, b.3] {
            d.extend((v as i16).to_be_bytes());
        }
        for e in &self.ends {
            d.extend(e.to_be_bytes());
        }
        d.extend(0u16.to_be_bytes());
        let (mut fl, mut xs, mut ys) = (vec![], vec![], vec![]);
        let (mut px, mut py) = (0, 0);
        let mut last: Option<u8> = None;
        let mut repeat = 0u8;
        for &(x, y, on) in &self.pts {
            let (dx, dy) = (x - px, y - py);
            (px, py) = (x, y);
            let mut f = on as u8;
            for (v, short, same, out) in [(dx, 2u8, 16u8, &mut xs), (dy, 4, 32, &mut ys)] {
                if v == 0 {
                    f |= same;
                } else if (-255..=255).contains(&v) {
                    f |= short;
                    if v > 0 {
                        f |= same;
                    }
                    out.push(v.unsigned_abs() as u8);
                } else {
                    out.extend((v as i16).to_be_bytes());
                }
            }
            if Some(f) == last && repeat != 255 {
                repeat += 1;
                if repeat == 1 {
                    fl.push(f);
                } else {
                    let n = fl.len();
                    fl[n - 2] = f | 8;
                    fl[n - 1] = repeat;
                }
            } else {
                repeat = 0;
                fl.push(f);
            }
            last = Some(f);
        }
        d.extend(fl);
        d.extend(xs);
        d.extend(ys);
        d
    }
}

pub struct Font {
    pub order: Vec<String>,
    pub glyphs: Vec<Glyph>,
    pub adv: Vec<i32>,
    pub cmap: BTreeMap<u32, usize>,
    pub family: String,
    pub style: String,
    pub em: u16,
    pub asc: i16,
    pub desc: i16,
    pub time: u64,
}

fn checksum(d: &[u8]) -> u32 {
    let mut s = 0u32;
    for c in d.chunks(4) {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        s = s.wrapping_add(u32::from_be_bytes(b));
    }
    s
}

fn cmap4(m: &BTreeMap<u32, usize>) -> Vec<u8> {
    let codes: Vec<u32> = m.keys().copied().collect();
    let gid = |c: u32| m[&c] as i64;
    let (mut start, mut end) = (vec![codes[0]], vec![]);
    let mut last = codes[0];
    let split = |s: u32, e: u32, start: &mut Vec<u32>, end: &mut Vec<u32>| {
        let (a, b) = split_range(s, e, &gid);
        start.extend(a);
        end.extend(b);
    };
    for &c in &codes[1..] {
        if c == last + 1 {
            last = c;
            continue;
        }
        split(*start.last().unwrap(), last, &mut start, &mut end);
        start.push(c);
        last = c;
    }
    split(*start.last().unwrap(), last, &mut start, &mut end);
    start.push(0xFFFF);
    end.push(0xFFFF);
    let (mut delta, mut ro, mut arr) = (vec![], vec![], vec![]);
    for i in 0..end.len() - 1 {
        let idx: Vec<i64> = (start[i]..=end[i]).map(gid).collect();
        if idx.iter().enumerate().all(|(k, &g)| g == idx[0] + k as i64) {
            delta.push(((idx[0] - start[i] as i64).rem_euclid(0x10000)) as u16);
            ro.push(0u16);
        } else {
            delta.push(0);
            ro.push((2 * (end.len() + arr.len() - i)) as u16);
            arr.extend(idx.iter().map(|&g| g as u16));
        }
    }
    delta.push(1);
    ro.push(0);
    let seg = end.len();
    let mut es = 0;
    while (1usize << (es + 1)) <= seg {
        es += 1;
    }
    let search = (1usize << es) * 2;
    let mut d = vec![];
    for v in [
        4u16,
        0,
        0,
        (seg * 2) as u16,
        search as u16,
        es as u16,
        (seg * 2 - search) as u16,
    ] {
        d.extend(v.to_be_bytes());
    }
    for v in end
        .iter()
        .map(|&v| v as u16)
        .chain([0])
        .chain(start.iter().map(|&v| v as u16))
    {
        d.extend(v.to_be_bytes());
    }
    for v in delta.iter().chain(&ro).chain(&arr) {
        d.extend(v.to_be_bytes());
    }
    let len = d.len() as u16;
    d[2..4].copy_from_slice(&len.to_be_bytes());
    d
}

/// `fontTools.ttLib.tables._c_m_a_p.splitRange`.
fn split_range(start: u32, end: u32, g: &dyn Fn(u32) -> i64) -> (Vec<u32>, Vec<u32>) {
    if start == end {
        return (vec![], vec![end]);
    }
    let (mut last_id, mut last_code) = (g(start), start);
    let (mut in_order, mut begin) = (false, 0);
    let mut sub: Vec<(u32, u32)> = vec![];
    for code in start + 1..=end {
        let id = g(code);
        if id - 1 == last_id {
            if !in_order {
                in_order = true;
                begin = last_code;
            }
        } else if in_order {
            in_order = false;
            sub.push((begin, last_code));
        }
        last_id = id;
        last_code = code;
    }
    if in_order {
        sub.push((begin, last_code));
    }
    let mut keep = vec![];
    for &(b, e) in &sub {
        if b == start && e == end {
            break;
        }
        let th = if b == start || e == end { 4 } else { 8 };
        if e - b + 1 > th {
            keep.push((b, e));
        }
    }
    let mut sub = keep;
    if sub.is_empty() {
        return (vec![], vec![end]);
    }
    if sub[0].0 != start {
        sub.insert(0, (start, sub[0].0 - 1));
    }
    if sub.last().unwrap().1 != end {
        let l = sub.last().unwrap().1;
        sub.push((l + 1, end));
    }
    let mut i = 1;
    while i < sub.len() {
        if sub[i - 1].1 + 1 != sub[i].0 {
            sub.insert(i, (sub[i - 1].1 + 1, sub[i].0 - 1));
            i += 1;
        }
        i += 1;
    }
    let mut s: Vec<u32> = sub.iter().map(|x| x.0).collect();
    s.remove(0);
    (s, sub.iter().map(|x| x.1).collect())
}

fn name_table(family: &str, style: &str) -> Vec<u8> {
    let mac = |s: &str| s.as_bytes().to_vec();
    let win = |s: &str| {
        s.encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect::<Vec<u8>>()
    };
    let recs: [(u16, u16, u16, u16, Vec<u8>); 4] = [
        (1, 0, 0, 1, mac(family)),
        (1, 0, 0, 2, mac(style)),
        (3, 1, 0x409, 1, win(family)),
        (3, 1, 0x409, 2, win(style)),
    ];
    let mut store: Vec<u8> = vec![];
    let mut seen: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    let mut d = vec![];
    for v in [0u16, 4, (6 + 12 * 4) as u16] {
        d.extend(v.to_be_bytes());
    }
    for (p, e, l, n, s) in &recs {
        let off = *seen.entry(s.clone()).or_insert_with(|| {
            let o = store.len();
            store.extend(s);
            o
        });
        for v in [*p, *e, *l, *n, s.len() as u16, off as u16] {
            d.extend(v.to_be_bytes());
        }
    }
    d.extend(store);
    d
}

impl Font {
    /// `FontBuilder.save` for this font (glyf, ten tables, fontTools' table order and checksums).
    pub fn compile(&self) -> Result<Vec<u8>, String> {
        let n = self.glyphs.len();
        let mut glyf = vec![];
        let mut data: Vec<Vec<u8>> = self.glyphs.iter().map(Glyph::compile).collect();
        if data.iter().map(Vec::len).sum::<usize>() < 0x20000 {
            for g in &mut data {
                if g.len() % 2 == 1 {
                    g.push(0);
                }
            }
        }
        let mut loca = vec![0u32];
        for g in &data {
            glyf.extend(g);
            loca.push(glyf.len() as u32);
        }
        if glyf.is_empty() {
            glyf.push(0);
        }
        let short = glyf.len() < 0x20000 && loca.iter().all(|l| l % 2 == 0);
        let loca_b: Vec<u8> = if short {
            loca.iter()
                .flat_map(|l| ((l / 2) as u16).to_be_bytes())
                .collect()
        } else {
            loca.iter().flat_map(|l| l.to_be_bytes()).collect()
        };
        let lsb: Vec<i32> = self.glyphs.iter().map(|g| g.bounds().0).collect();
        // hmtx
        let last = self.adv[n - 1];
        let mut nm = n;
        while self.adv[nm - 2] == last {
            nm -= 1;
            if nm <= 1 {
                nm = 1;
                break;
            }
        }
        let mut hmtx = vec![];
        for (i, l) in lsb.iter().enumerate().take(nm) {
            hmtx.extend((self.adv[i] as u16).to_be_bytes());
            hmtx.extend((*l as i16).to_be_bytes());
        }
        for l in &lsb[nm..] {
            hmtx.extend((*l as i16).to_be_bytes());
        }
        // bounds
        let (mut xmin, mut ymin, mut xmax, mut ymax) = (100000, 100000, -100000, -100000);
        let (mut maxp_pts, mut maxc, mut all_lsb) = (0, 0, true);
        let (mut min_lsb, mut min_rsb, mut max_ext) = (i32::MAX, i32::MAX, i32::MIN);
        for (i, g) in self.glyphs.iter().enumerate() {
            if g.ends.is_empty() {
                continue;
            }
            let b = g.bounds();
            all_lsb &= lsb[i] == b.0;
            (xmin, ymin, xmax, ymax) = (xmin.min(b.0), ymin.min(b.1), xmax.max(b.2), ymax.max(b.3));
            maxp_pts = maxp_pts.max(g.pts.len());
            maxc = maxc.max(g.ends.len());
            let w = b.2 - b.0;
            min_lsb = min_lsb.min(lsb[i]);
            min_rsb = min_rsb.min(self.adv[i] - lsb[i] - w);
            max_ext = max_ext.max(lsb[i] + w);
        }
        let flags: u16 = if all_lsb { 3 | 2 } else { 3 & !2 };
        let t = self.time + 2_082_844_800;
        let mut head = vec![];
        head.extend(0x0001_0000u32.to_be_bytes());
        head.extend(0x0001_0000u32.to_be_bytes());
        head.extend(0u32.to_be_bytes());
        head.extend(0x5F0F3CF5u32.to_be_bytes());
        head.extend(flags.to_be_bytes());
        head.extend(self.em.to_be_bytes());
        head.extend(t.to_be_bytes());
        head.extend(t.to_be_bytes());
        for v in [xmin, ymin, xmax, ymax] {
            head.extend((v as i16).to_be_bytes());
        }
        for v in [0u16, 3, 2, if short { 0 } else { 1 }, 0] {
            head.extend(v.to_be_bytes());
        }
        let mut hhea = vec![];
        hhea.extend(0x0001_0000u32.to_be_bytes());
        let adv_max = *self.adv.iter().max().unwrap();
        for v in [
            self.asc,
            self.desc,
            0,
            adv_max as i16,
            min_lsb as i16,
            min_rsb as i16,
            max_ext as i16,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ] {
            hhea.extend(v.to_be_bytes());
        }
        hhea.extend((nm as u16).to_be_bytes());
        let mut maxp = vec![];
        maxp.extend(0x0001_0000u32.to_be_bytes());
        for v in [n, maxp_pts, maxc, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
            maxp.extend((v as u16).to_be_bytes());
        }
        let ws: Vec<i32> = self.adv.iter().copied().filter(|&w| w > 0).collect();
        let avg = (ws.iter().sum::<i32>() as f64 / ws.len() as f64 + 0.5).floor() as i16;
        let lo = *self.cmap.keys().next().unwrap();
        let hi = *self.cmap.keys().last().unwrap();
        if self.cmap.keys().any(|c| !(0xE000..=0xF8FF).contains(c)) {
            return Err("cmap outside the private use area".into());
        }
        let mut os2 = vec![];
        os2.extend(3u16.to_be_bytes());
        os2.extend(avg.to_be_bytes());
        os2.extend([0x01, 0x90, 0, 5, 0, 4]);
        os2.extend([0u8; 20]);
        os2.extend(0u16.to_be_bytes());
        os2.extend([0u8; 10]);
        os2.extend([0u8; 4]);
        os2.extend(0x1000_0000u32.to_be_bytes());
        os2.extend([0u8; 8]);
        os2.extend(b"????");
        os2.extend(0u16.to_be_bytes());
        os2.extend((lo as u16).to_be_bytes());
        os2.extend((hi as u16).to_be_bytes());
        for v in [self.asc, self.desc, 0, self.asc, -self.desc] {
            os2.extend(v.to_be_bytes());
        }
        os2.extend([0u8; 8]);
        for v in [0u16, 0, 0, 32, 0] {
            os2.extend(v.to_be_bytes());
        }
        let c4 = cmap4(&self.cmap);
        let mut cmap = vec![];
        for v in [0u16, 2, 0, 3] {
            cmap.extend(v.to_be_bytes());
        }
        cmap.extend(20u32.to_be_bytes());
        for v in [3u16, 1] {
            cmap.extend(v.to_be_bytes());
        }
        cmap.extend(20u32.to_be_bytes());
        cmap.extend(c4);
        let mut post = vec![];
        post.extend(0x0002_0000u32.to_be_bytes());
        post.extend([0u8; 28]);
        post.extend((n as u16).to_be_bytes());
        for i in 0..n {
            let v = if i == 0 { 0 } else { 257 + i as u16 };
            post.extend(v.to_be_bytes());
        }
        for nm in &self.order[1..] {
            post.push(nm.len() as u8);
            post.extend(nm.as_bytes());
        }
        let tabs: Vec<(&[u8; 4], Vec<u8>)> = vec![
            (b"head", head),
            (b"hhea", hhea),
            (b"maxp", maxp),
            (b"OS/2", os2),
            (b"hmtx", hmtx),
            (b"cmap", cmap),
            (b"loca", loca_b),
            (b"glyf", glyf),
            (b"name", name_table(&self.family, &self.style)),
            (b"post", post),
        ];
        let nt = tabs.len();
        let mut es = 0;
        while (2usize << es) <= nt {
            es += 1;
        }
        let sr = 16 * (1usize << es);
        let mut out = vec![];
        out.extend(0x0001_0000u32.to_be_bytes());
        for v in [nt, sr, es, nt * 16 - sr] {
            out.extend((v as u16).to_be_bytes());
        }
        let mut dir: Vec<([u8; 4], u32, usize, usize)> = vec![];
        let mut off = 12 + 16 * nt;
        let mut body = vec![];
        for (tag, d) in &tabs {
            dir.push((**tag, checksum(d), off, d.len()));
            let mut p = d.clone();
            pad(&mut p, 4);
            off += p.len();
            body.extend(p);
        }
        dir.sort_by_key(|e| e.0);
        for (tag, cs, o, l) in &dir {
            out.extend(tag);
            out.extend(cs.to_be_bytes());
            out.extend((*o as u32).to_be_bytes());
            out.extend((*l as u32).to_be_bytes());
        }
        let hoff = dir.iter().find(|e| &e.0 == b"head").unwrap().2 - (12 + 16 * nt);
        out.extend(body);
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        let p = 12 + 16 * nt + hoff + 8;
        out[p..p + 4].copy_from_slice(&adj.to_be_bytes());
        Ok(out)
    }
}
