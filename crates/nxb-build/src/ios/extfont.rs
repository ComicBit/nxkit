//! `make_ext_font.py` in Rust: the stand-in for the "Nintendo extension" shared font (controller-button icons at the
//! original private-use code points). The reference uses fontTools, and so does its arithmetic: floats until the
//! glyph is built, `otRound` at the end, the contour reversal of `ReverseContourPen`, the quadratic bounds of
//! `BoundsPen`. `SOURCE_DATE_EPOCH` sets the font's time as it does for fontTools (the reference stamps `now` without
//! it), so the output is byte-identical to the script's for the same time.
#![allow(clippy::unwrap_used)]

use super::fontpen::{
    CY, Pen, Pt, R, area, bounds_of, circle, contour, cross, rect, reversed, rot, rrect, transform,
};
use super::ttf::{Font, Glyph};
use super::ttf_read::{Contour, P, Reader};
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

/// The script's default letter font.
pub const LETTER_FONT: &str = "/System/Library/Fonts/Supplemental/Arial Rounded Bold.ttf";

struct Letters {
    f: Reader,
}

impl Letters {
    fn knock(&self, pen: &mut Pen, text: &str, cx: f64, cy: f64, h: f64) -> Result<(), String> {
        let mut x = 0i32;
        let mut parts = vec![];
        for ch in text.chars() {
            let g = self.f.gid(ch)?;
            parts.push((self.f.draw(g)?, x));
            x += self.f.advance(g)?;
        }
        let mut bc: Vec<Contour> = vec![];
        for (cs, dx) in &parts {
            for c in cs {
                bc.push(transform(c, (1.0, 0.0, 0.0, 1.0, *dx as f64, 0.0)));
            }
        }
        let (x0, y0, x1, y1) = bounds_of(&bc).ok_or("empty letters")?;
        let s = h / (y1 - y0);
        let (ox, oy) = (cx - (x0 + x1) / 2.0 * s, cy - (y0 + y1) / 2.0 * s);
        let mut rec: Vec<Contour> = vec![];
        for (cs, dx) in &parts {
            for c in cs {
                rec.push(transform(c, (s, 0.0, 0.0, s, ox + *dx as f64 * s, oy)));
            }
        }
        let rings: Vec<Vec<P>> = rec
            .iter()
            .map(|c| {
                c.iter()
                    .flat_map(|s| s.pts.iter().flatten().copied())
                    .collect()
            })
            .collect();
        let mut big = &rings[0];
        for r in &rings[1..] {
            if area(r).abs() > area(big).abs() {
                big = r;
            }
        }
        let rev = area(big) < 0.0;
        for c in rec {
            let c = if rev { reversed(c)? } else { c };
            pen.replay(&c);
        }
        Ok(())
    }
}

struct Builder {
    glyphs: BTreeMap<String, Glyph>,
    adv: BTreeMap<String, i32>,
    cmap: BTreeMap<u32, String>,
}

type Draw<'a> = Box<dyn Fn(&mut Pen, f64) -> Result<(), String> + 'a>;

impl Builder {
    fn glyph(&mut self, name: &str, codes: &[u32], width: i32, draw: Draw) -> Result<(), String> {
        let mut pen = Pen::default();
        draw(&mut pen, width as f64 / 2.0)?;
        self.glyphs.insert(name.into(), pen.glyph());
        self.adv.insert(name.into(), width);
        for c in codes {
            self.cmap.insert(*c, name.into());
        }
        Ok(())
    }
}

/// Build the font file's bytes (`build(out, letter_font)` of the script).
pub fn build(letter_font: &Path, time: u64) -> Result<Vec<u8>, String> {
    let data = std::fs::read(letter_font).map_err(|e| format!("{}: {e}", letter_font.display()))?;
    let letters = Rc::new(Letters {
        f: Reader::new(data)?,
    });
    let mut b = Builder {
        glyphs: BTreeMap::new(),
        adv: BTreeMap::new(),
        cmap: BTreeMap::new(),
    };
    let mk = || letters.clone();
    for (i, t) in ["A", "B", "X", "Y"].into_iter().enumerate() {
        let l = mk();
        b.glyph(
            &format!("btn{t}"),
            &[0xE0A0 + i as u32, 0xE0E0 + i as u32],
            1000,
            Box::new(move |pen, cx| {
                contour(pen, &circle(cx, CY, R), false);
                l.knock(pen, t, cx, CY, 470.0)
            }),
        )?;
    }
    for (i, (t, w)) in [
        ("L", 900),
        ("R", 900),
        ("ZL", 1200),
        ("ZR", 1200),
        ("SL", 1200),
        ("SR", 1200),
    ]
    .into_iter()
    .enumerate()
    {
        let l = mk();
        b.glyph(
            &format!("btn{t}"),
            &[0xE0A4 + i as u32, 0xE0E4 + i as u32],
            w + 100,
            Box::new(move |pen, cx| {
                let (w, h) = (w as f64, 620.0);
                contour(
                    pen,
                    &rrect(
                        cx - w / 2.0,
                        CY - h / 2.0,
                        cx + w / 2.0,
                        CY + h / 2.0,
                        220.0,
                    ),
                    false,
                );
                l.knock(pen, t, cx, CY, 400.0)
            }),
        )?;
    }
    for (name, code, minus) in [
        ("btnPlus", [0xE0B5, 0xE0EF], false),
        ("btnMinus", [0xE0B6, 0xE0F0], true),
    ] {
        b.glyph(
            name,
            &code,
            1000,
            Box::new(move |pen, cx| {
                contour(pen, &circle(cx, CY, R), false);
                let (t, l) = (70.0, 250.0);
                if minus {
                    contour(pen, &rect(cx - l, CY - t, cx + l, CY + t), true);
                } else {
                    contour(pen, &cross(cx, CY, l, 2.0 * t), true);
                }
                Ok(())
            }),
        )?;
    }
    let dpad = |dir: Option<&'static str>| -> Draw<'static> {
        Box::new(move |pen, cx| {
            contour(pen, &cross(cx, CY, R, 320.0), false);
            let Some(d) = dir else { return Ok(()) };
            contour(pen, &cross(cx, CY, R - 55.0, 320.0 - 110.0), true);
            let arm = rect(cx - 105.0, CY + 105.0, cx + 105.0, CY + R - 55.0);
            let deg = match d {
                "up" => 0.0,
                "left" => 90.0,
                "down" => 180.0,
                _ => 270.0,
            };
            contour(pen, &rot(&arm, cx, CY, deg), false);
            Ok(())
        })
    };
    b.glyph("dpad", &[0xE0EA], 1000, dpad(None))?;
    for (c, d) in [
        (0xE0ABu32, "right"),
        (0xE0AC, "down"),
        (0xE0AD, "up"),
        (0xE0AE, "left"),
    ] {
        b.glyph(&format!("dpad_{d}"), &[c, c + 0x40], 1000, dpad(Some(d)))?;
    }
    let arrows = [
        ("right", 0.0),
        ("left", 180.0),
        ("up", 90.0),
        ("down", 270.0),
        ("rightup", 45.0),
        ("rightdown", 315.0),
        ("leftdown", 225.0),
        ("leftup", 135.0),
    ];
    for (c, (nm, deg)) in (0xE090u32..).zip(arrows) {
        b.glyph(
            &format!("arrow_{nm}"),
            &[c],
            1000,
            Box::new(move |pen, cx| {
                let p = [
                    (-380.0, 105.0),
                    (60.0, 105.0),
                    (60.0, 310.0),
                    (420.0, 0.0),
                    (60.0, -310.0),
                    (60.0, -105.0),
                    (-380.0, -105.0),
                ];
                let v: Vec<Pt> = p.iter().map(|&(x, y)| (cx + x, CY + y, true)).collect();
                contour(pen, &rot(&v, cx, CY, deg), false);
                Ok(())
            }),
        )?;
    }
    for (name, codes, t) in [
        ("stick", vec![0xE100u32], ""),
        ("stickL", vec![0xE101, 0xE0C1], "L"),
        ("stickR", vec![0xE102, 0xE0C2], "R"),
    ] {
        let l = mk();
        b.glyph(
            name,
            &codes,
            1000,
            Box::new(move |pen, cx| {
                contour(pen, &circle(cx, CY, R), false);
                contour(pen, &circle(cx, CY, R - 70.0), true);
                contour(pen, &circle(cx, CY, 300.0), false);
                if !t.is_empty() {
                    l.knock(pen, t, cx, CY, 300.0)?;
                }
                Ok(())
            }),
        )?;
    }
    let rest: Vec<u32> = (0xE000..0xE200)
        .filter(|c| !b.cmap.contains_key(c))
        .collect();
    b.glyph(
        "icon",
        &rest,
        1000,
        Box::new(|pen, cx| {
            contour(pen, &circle(cx, CY, R), false);
            contour(pen, &circle(cx, CY, R - 70.0), true);
            Ok(())
        }),
    )?;
    let mut order = vec![".notdef".to_string()];
    order.extend(b.glyphs.keys().cloned());
    let idx: BTreeMap<&String, usize> = order.iter().enumerate().map(|(i, n)| (n, i)).collect();
    let mut glyphs = vec![Glyph::default()];
    glyphs.extend(b.glyphs.values().cloned());
    let mut adv = vec![500];
    adv.extend(b.glyphs.keys().map(|k| b.adv[k]));
    let cmap = b.cmap.iter().map(|(c, n)| (*c, idx[n])).collect();
    Font {
        order,
        glyphs,
        adv,
        cmap,
        family: "DM Nintendo Extension Stand-in".into(),
        style: "Regular".into(),
        em: 1000,
        asc: 880,
        desc: -120,
        time,
    }
    .compile()
}

/// `make_ext_font.py out.ttf [letter-font.ttf]`.
pub fn make(out: &Path, letter_font: Option<&Path>) -> Result<(), String> {
    let time = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(v) => v
            .trim()
            .parse::<u64>()
            .map_err(|_| "SOURCE_DATE_EPOCH".to_string())?,
        Err(_) => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs(),
    };
    let bytes = build(letter_font.unwrap_or(Path::new(LETTER_FONT)), time)?;
    std::fs::write(out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: `NXB_ORACLE_ENGINE` (the engine folder) and `NXB_ORACLE_PYTHON` (a Python with fontTools). Both run
    /// with `SOURCE_DATE_EPOCH` fixed, so the creation time and the checksum adjustment agree.
    #[test]
    fn font_equals_the_reference_script() {
        let (Some(eng), Some(py)) = (
            std::env::var_os("NXB_ORACLE_ENGINE"),
            std::env::var_os("NXB_ORACLE_PYTHON"),
        ) else {
            return;
        };
        let out = std::env::temp_dir().join(format!("nxb-extfont-{}.ttf", std::process::id()));
        let st = std::process::Command::new(py)
            .arg(Path::new(&eng).join("dm/tools/make_ext_font.py"))
            .arg(&out)
            .env("SOURCE_DATE_EPOCH", "1000000000")
            .output()
            .unwrap();
        assert!(st.status.success());
        let want = std::fs::read(&out).unwrap();
        let _ = std::fs::remove_file(&out);
        let got = build(Path::new(LETTER_FONT), 1_000_000_000).unwrap();
        assert!(
            got == want,
            "font differs ({} vs {} bytes)",
            got.len(),
            want.len()
        );
    }
}
