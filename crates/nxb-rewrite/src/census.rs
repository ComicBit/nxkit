//! The rewrite-site census (`dm/tools/insn_census.py`): how many instruction sites the rewriter must handle in a
//! module's text. A linear sweep over the text counts `svc`, the thread pointer and counter-timer registers, every use
//! of `x18` and `sub sp, sp, xN`. The data blocks the rewriter leaves alone are left out.

use std::fmt::Write;

use nxb_a64::decode;

use crate::error::RewriteError;
use crate::nso::load;
use crate::scan::data_blocks;

/// The system registers the census counts, as `(o0 op1 CRn CRm op2 field of the word, Capstone's name)`.
const REGS: [(u32, &str); 7] = [
    (0xBD060 >> 5, "tpidrro_el0"),
    (0xBD040 >> 5, "tpidr_el0"),
    (0xBE020 >> 5, "cntpct_el0"),
    (0xBE040 >> 5, "cntvct_el0"),
    (0xBE000 >> 5, "cntfrq_el0"),
    (0xBE0A0 >> 5, "cntpctss_el0"),
    (0xBE0C0 >> 5, "cntvctss_el0"),
];

/// A counter that lists its keys by count, equal counts in the order the keys first came.
#[derive(Default)]
struct Counter(Vec<(String, usize)>);

impl Counter {
    fn add(&mut self, key: &str) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 += 1,
            None => self.0.push((key.to_string(), 1)),
        }
    }

    fn most_common(&self) -> Vec<&(String, usize)> {
        let mut v: Vec<&(String, usize)> = self.0.iter().collect();
        v.sort_by_key(|e| std::cmp::Reverse(e.1));
        v
    }
}

/// `sp, sp, x` and digits only.
fn sub_sp_xn(ops: &str) -> bool {
    ops.strip_prefix("sp, sp, x")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The value of an `svc` operand (`#0x1d` or `#5`), the sort key of the number list.
fn svc_number(text: &str) -> u64 {
    let t = text.trim_start_matches('#');
    match t.strip_prefix("0x") {
        Some(h) => u64::from_str_radix(h, 16).unwrap_or(0),
        None => t.parse().unwrap_or(0),
    }
}

/// The census of one module: the text the script prints.
/// `label` is the module's name in the first line (the script prints the path it was given).
pub fn census(label: &str, nso: &[u8]) -> Result<String, RewriteError> {
    let m = load(nso, label)?;
    let base = m.segments[0].0;
    let size = m.text_end();
    let end = base + size;
    let blocks = data_blocks(&m.image, end as i64)?;
    let text = m.image.get(base..end).unwrap_or(&[]);
    let skip = |off: usize| {
        blocks.iter().any(|&(s, e)| {
            (s - base as i64..e - base as i64).contains(&(off as i64))
                && (off as i64 - (s - base as i64)) % 4 == 0
        })
    };
    let (mut kinds, mut svcs) = (Counter::default(), Counter::default());
    let (mut total, mut undecoded) = (0usize, 0usize);
    for off in (0..text.len().saturating_sub(3)).step_by(4) {
        if skip(off) {
            continue;
        }
        let w = u32::from_le_bytes([text[off], text[off + 1], text[off + 2], text[off + 3]]);
        let addr = (base + off) as u64;
        let Some(i) = decode(w, addr) else {
            undecoded += 1;
            continue;
        };
        total += 1;
        let (mn, ops) = (i.mn, i.op_str());
        let mut kind: Option<String> = None;
        if mn == "svc" {
            kind = Some("svc".into());
            svcs.add(&ops);
        } else if mn == "mrs" || mn == "msr" {
            let field = (w >> 5) & 0x7fff;
            kind = REGS
                .iter()
                .find(|r| r.0 == field)
                .map(|r| format!("{mn} {}", r.1));
        }
        if kind.is_none() && i.has18() {
            kind = Some("x18 use".into());
        }
        if kind.is_none() && mn == "sub" && sub_sp_xn(&ops) {
            kind = Some("sub sp, sp, xN".into());
        }
        if let Some(k) = kind {
            kinds.add(&k);
        }
    }
    let mut out =
        format!("{label}: {total} instructions decoded ({undecoded} undecodable words)\n");
    if !blocks.is_empty() {
        let list: Vec<String> = blocks
            .iter()
            .map(|(s, e)| format!("{s:#x}..{e:#x}"))
            .collect();
        let _ = writeln!(out, "  data blocks left out: {}", list.join(", "));
    }
    for (k, v) in kinds.most_common() {
        let _ = writeln!(out, "  {k:<22} {v}");
    }
    if !svcs.0.is_empty() {
        let mut v: Vec<&(String, usize)> = svcs.0.iter().collect();
        v.sort_by_key(|(k, _)| svc_number(k));
        let list: Vec<String> = v.iter().map(|(k, n)| format!("{k} x{n}")).collect();
        let _ = writeln!(out, "  svc numbers: {}", list.join(", "));
    }
    Ok(out)
}
