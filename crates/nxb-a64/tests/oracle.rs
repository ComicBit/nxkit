//! Compares the decoder with a dump made by Capstone 5 (`dump_insn.py`): every distinct instruction word of some
//! real modules. Skipped without `NXB_A64_ORACLE` (the dump file). Columns: word, mnemonic, op_str, operands,
//! registers read, registers written, write-back.

use std::collections::BTreeMap;

use nxb_a64::{Op, Reg, decode};

fn norm(names: &str, vectors: bool) -> Vec<String> {
    let mut v: Vec<String> = names
        .split(',')
        .filter(|s| !s.is_empty())
        .filter(|s| !matches!(*s, "xzr" | "wzr"))
        .filter(|s| {
            vectors
                || s.starts_with(['x', 'w', 's']) && !s.starts_with(['s'])
                || ["sp", "wsp", "fp", "lr", "nzcv"].contains(s)
        })
        .map(String::from)
        .collect();
    v.sort();
    v.dedup();
    v
}

fn mine(regs: &[Reg], vectors: bool) -> Vec<String> {
    let mut v: Vec<String> = regs
        .iter()
        .filter(|r| vectors || !matches!(r, Reg::F(..)))
        .map(|r| r.name())
        .collect();
    v.sort();
    v.dedup();
    v
}

fn gprs(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let word_start = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if word_start && (b[i] == b'x' || b[i] == b'w') {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1
                && (j == b.len() || !b[j].is_ascii_alphanumeric())
                && let Ok(n) = text[i + 1..j].parse::<u8>()
            {
                out.push(n);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn decoder_equals_capstone() {
    let Some(path) = std::env::var_os("NXB_A64_ORACLE") else {
        eprintln!("skipped: NXB_A64_ORACLE not set");
        return;
    };
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut bad: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let (mut n, mut fails) = (0usize, 0usize);
    let mut note = |key: String, line: String| {
        fails += 1;
        let e = bad.entry(key).or_insert((0, line));
        e.0 += 1;
    };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let Ok(w) = u32::from_str_radix(f[0], 16) else {
            continue;
        };
        n += 1;
        let ins = decode(w, 0x100_0000);
        if f[1] == "-" {
            if ins.is_some() {
                note("accepts what capstone rejects".into(), line.to_string());
            }
            continue;
        }
        let Some(i) = ins else {
            note(format!("rejects {}", f[1]), line.to_string());
            continue;
        };
        let generic = i.simd
            && i.ops.iter().all(|o| matches!(o, Op::Reg(Reg::F(..), _)))
            && !matches!(
                i.mn,
                "scvtf" | "ucvtf" | "fcvtzs" | "fcvtzu" | "smov" | "umov"
            );
        if !generic && i.mn != f[1] {
            note(format!("mnemonic {} vs {}", i.mn, f[1]), line.to_string());
            continue;
        }
        let sys = matches!(
            i.mn,
            "mrs"
                | "msr"
                | "sys"
                | "sysl"
                | "dc"
                | "ic"
                | "at"
                | "tlbi"
                | "dmb"
                | "dsb"
                | "isb"
                | "hint"
                | "prfm"
                | "prfum"
        );
        if !i.simd && !sys && i.op_str() != f[2] {
            note(
                format!("text of {}", i.mn),
                format!("{line}\t=> {}", i.op_str()),
            );
            continue;
        }
        let want = gprs(f[2]);
        let mut got = i.text_gprs();
        got.sort_unstable();
        got.dedup();
        if want != got {
            note(format!("gprs of {}", f[1]), format!("{line}\t=> {got:?}"));
            continue;
        }
        let vec = !i.simd;
        if norm(f[4], vec) != mine(&i.reads, vec) {
            note(
                format!("reads of {}", f[1]),
                format!("{line}\t=> {:?}", mine(&i.reads, vec)),
            );
            continue;
        }
        if norm(f[5], vec) != mine(&i.writes, vec) {
            note(
                format!("writes of {}", f[1]),
                format!("{line}\t=> {:?}", mine(&i.writes, vec)),
            );
            continue;
        }
        if (f[6] == "1") != i.wb {
            note(format!("writeback of {}", f[1]), line.to_string());
            continue;
        }
        let _ = matches!(i.ops.first(), Some(Op::Reg(..)));
    }
    let mut rows: Vec<_> = bad.iter().collect();
    rows.sort_by_key(|(_, (c, _))| std::cmp::Reverse(*c));
    let skip = std::env::var_os("NXB_A64_SKIP_REJECTS").is_some();
    for (k, (c, ex)) in rows
        .iter()
        .filter(|(k, _)| !(skip && k.starts_with("rejects")))
        .take(60)
    {
        eprintln!("{c:7} {k}\n          e.g. {ex}");
    }
    eprintln!("{n} words, {fails} mismatches");
    assert_eq!(fails, 0);
}
