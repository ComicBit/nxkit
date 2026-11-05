//! The reader half of the TrueType code: the letter font's simple glyphs, advances and Unicode map, and the pen
//! protocol types shared with the font writer.
#![allow(missing_docs, unreachable_pub, clippy::unwrap_used)]

use std::collections::BTreeMap;

pub type P = (f64, f64);

/// A pen operation: `pts` are the operands, the last of a `Q` may be `None` (a contour without on-curve point).
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub kind: Kind,
    pub pts: Vec<Option<P>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Move,
    Line,
    Q,
}

/// One closed contour as the pen protocol sees it (the closing `closePath` is implied).
pub type Contour = Vec<Seg>;

fn u16_at(d: &[u8], o: usize) -> Result<u16, String> {
    d.get(o..o + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| "font truncated".to_string())
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, String> {
    d.get(o..o + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "font truncated".to_string())
}

/// The parts of the letter font that are read: outlines, advances and the best Unicode map.
pub struct Reader {
    data: Vec<u8>,
    tables: BTreeMap<[u8; 4], (usize, usize)>,
    loca_long: bool,
    cmap: BTreeMap<u32, u16>,
}

impl Reader {
    pub fn new(data: Vec<u8>) -> Result<Reader, String> {
        let n = u16_at(&data, 4)? as usize;
        let mut tables = BTreeMap::new();
        for i in 0..n {
            let o = 12 + 16 * i;
            let tag: [u8; 4] = data
                .get(o..o + 4)
                .ok_or("font truncated")?
                .try_into()
                .unwrap();
            tables.insert(
                tag,
                (
                    u32_at(&data, o + 8)? as usize,
                    u32_at(&data, o + 12)? as usize,
                ),
            );
        }
        let mut r = Reader {
            data,
            tables,
            loca_long: false,
            cmap: BTreeMap::new(),
        };
        r.loca_long = u16_at(r.table(b"head")?, 50)? != 0;
        r.cmap = r.read_cmap()?;
        Ok(r)
    }

    fn table(&self, tag: &[u8; 4]) -> Result<&[u8], String> {
        let (o, l) = *self.tables.get(tag).ok_or("font table missing")?;
        self.data
            .get(o..o + l)
            .ok_or_else(|| "font truncated".into())
    }

    /// `getBestCmap` for the subtable kinds that matter: (3,10), (0,x), (3,1), format 4 and 12.
    fn read_cmap(&self) -> Result<BTreeMap<u32, u16>, String> {
        let t = self.table(b"cmap")?;
        let n = u16_at(t, 2)? as usize;
        let mut subs = vec![];
        for i in 0..n {
            subs.push((
                u16_at(t, 4 + 8 * i)?,
                u16_at(t, 6 + 8 * i)?,
                u32_at(t, 8 + 8 * i)? as usize,
            ));
        }
        let prefs = [
            (3, 10),
            (0, 6),
            (0, 4),
            (3, 1),
            (0, 3),
            (0, 2),
            (0, 1),
            (0, 0),
        ];
        let sub = prefs
            .iter()
            .find_map(|p| subs.iter().find(|s| (s.0, s.1) == *p))
            .ok_or("no Unicode cmap")?;
        let s = t.get(sub.2..).ok_or("font truncated")?;
        let mut m = BTreeMap::new();
        match u16_at(s, 0)? {
            4 => {
                let segs = u16_at(s, 6)? as usize / 2;
                let (e, st) = (14, 16 + 2 * segs);
                let (dl, ro) = (st + 2 * segs, st + 4 * segs);
                for i in 0..segs {
                    let (end, start) = (u16_at(s, e + 2 * i)?, u16_at(s, st + 2 * i)?);
                    let (delta, off) = (u16_at(s, dl + 2 * i)?, u16_at(s, ro + 2 * i)?);
                    for c in start..=end {
                        if c == 0xFFFF {
                            continue;
                        }
                        let g = if off == 0 {
                            c.wrapping_add(delta)
                        } else {
                            let p = ro + 2 * i + off as usize + 2 * (c - start) as usize;
                            let g = u16_at(s, p)?;
                            if g == 0 { 0 } else { g.wrapping_add(delta) }
                        };
                        if g != 0 {
                            m.insert(c as u32, g);
                        }
                    }
                }
            }
            12 => {
                for i in 0..u32_at(s, 12)? as usize {
                    let o = 16 + 12 * i;
                    let (a, b, g) = (u32_at(s, o)?, u32_at(s, o + 4)?, u32_at(s, o + 8)?);
                    for c in a..=b {
                        m.insert(c, (g + (c - a)) as u16);
                    }
                }
            }
            f => return Err(format!("cmap format {f} unsupported")),
        }
        Ok(m)
    }

    pub fn gid(&self, ch: char) -> Result<u16, String> {
        self.cmap
            .get(&(ch as u32))
            .copied()
            .ok_or_else(|| format!("letter font lacks {ch:?}"))
    }

    pub fn advance(&self, gid: u16) -> Result<i32, String> {
        let n = u16_at(self.table(b"hhea")?, 34)? as usize;
        let h = self.table(b"hmtx")?;
        Ok(u16_at(h, 4 * (gid as usize).min(n - 1))? as i32)
    }

    /// `Glyph.draw` of a simple glyph (quadratic outlines): one `Contour` per contour.
    pub fn draw(&self, gid: u16) -> Result<Vec<Contour>, String> {
        let loca = self.table(b"loca")?;
        let at = |i: usize| -> Result<usize, String> {
            Ok(if self.loca_long {
                u32_at(loca, 4 * i)? as usize
            } else {
                2 * u16_at(loca, 2 * i)? as usize
            })
        };
        let (a, b) = (at(gid as usize)?, at(gid as usize + 1)?);
        if a == b {
            return Ok(vec![]);
        }
        let g = self.table(b"glyf")?.get(a..b).ok_or("font truncated")?;
        let nc = u16_at(g, 0)? as i16;
        if nc < 0 {
            return Err("composite glyph in the letter font".into());
        }
        let ends: Vec<usize> = (0..nc as usize)
            .map(|i| u16_at(g, 10 + 2 * i).map(|v| v as usize))
            .collect::<Result<_, _>>()?;
        let np = ends.last().map_or(0, |e| e + 1);
        let il = u16_at(g, 10 + 2 * nc as usize)? as usize;
        let mut o = 12 + 2 * nc as usize + il;
        let mut flags = vec![];
        while flags.len() < np {
            let f = *g.get(o).ok_or("font truncated")?;
            o += 1;
            flags.push(f);
            if f & 8 != 0 {
                let r = *g.get(o).ok_or("font truncated")?;
                o += 1;
                for _ in 0..r {
                    flags.push(f);
                }
            }
        }
        let mut coords = vec![(0i32, 0i32); np];
        for axis in 0..2 {
            let (short, same) = if axis == 0 { (2, 16) } else { (4, 32) };
            let mut v = 0i32;
            for (i, &f) in flags.iter().enumerate() {
                if f & short != 0 {
                    let d = *g.get(o).ok_or("font truncated")? as i32;
                    o += 1;
                    v += if f & same != 0 { d } else { -d };
                } else if f & same == 0 {
                    v += u16_at(g, o)? as i16 as i32;
                    o += 2;
                }
                if axis == 0 {
                    coords[i].0 = v
                } else {
                    coords[i].1 = v
                }
            }
        }
        let mut out = vec![];
        let mut start = 0;
        for end in ends {
            let end = end + 1;
            let pts: Vec<P> = coords[start..end]
                .iter()
                .map(|c| (c.0 as f64, c.1 as f64))
                .collect();
            let on: Vec<bool> = flags[start..end].iter().map(|f| f & 1 != 0).collect();
            start = end;
            out.push(contour_of(&pts, &on));
        }
        Ok(out)
    }
}

/// The quadratic branch of `Glyph.draw`.
fn contour_of(pts: &[P], on: &[bool]) -> Contour {
    let seg = |kind, v: &[P], tail: Option<P>| {
        let mut p: Vec<Option<P>> = v.iter().map(|x| Some(*x)).collect();
        if let Some(t) = tail {
            p.push(Some(t));
        }
        Seg { kind, pts: p }
    };
    let Some(fi) = on.iter().position(|&b| b) else {
        let mut p: Vec<Option<P>> = pts.iter().map(|x| Some(*x)).collect();
        p.push(None);
        return vec![Seg {
            kind: Kind::Q,
            pts: p,
        }];
    };
    let k = fi + 1;
    let mut c: Vec<P> = pts[k..].to_vec();
    c.extend_from_slice(&pts[..k]);
    let mut f: Vec<bool> = on[k..].to_vec();
    f.extend_from_slice(&on[..k]);
    let mut out = vec![seg(Kind::Move, &[*c.last().unwrap()], None)];
    while !c.is_empty() {
        let n = f.iter().position(|&b| b).unwrap() + 1;
        if n == 1 {
            if c.len() > 1 {
                out.push(seg(Kind::Line, &[c[0]], None));
            }
        } else {
            out.push(seg(Kind::Q, &c[..n], None));
        }
        c.drain(..n);
        f.drain(..n);
    }
    out
}
