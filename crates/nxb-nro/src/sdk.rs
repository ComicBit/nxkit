//! `online_sdk`: the online SDK an NRO is, by its exported symbols (`nso_syms.dynsyms`).

/// Exports that mark an online SDK the host never loads.
const ONLINE_SDKS: &[(&str, &str)] = &[("EOS_Platform_Create", "Epic Online Services SDK")];

fn u(b: &[u8], o: usize, n: usize) -> Option<u64> {
    let s = b.get(o..o.checked_add(n)?)?;
    Some(s.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x)))
}

fn exports(img: &[u8]) -> Option<Vec<String>> {
    let mod0 = u(img, 4, 4)? as usize;
    if img.get(mod0..mod0 + 4)? != b"MOD0" {
        return None;
    }
    let dyn_ = (mod0 as i64 + i64::from(u(img, mod0 + 4, 4)? as u32 as i32)) as usize;
    let mut tags = std::collections::HashMap::new();
    for i in 0..4096 {
        let (t, v) = (u(img, dyn_ + i * 16, 8)?, u(img, dyn_ + i * 16 + 8, 8)?);
        if t == 0 {
            break;
        }
        tags.entry(t).or_insert(v);
    }
    let (symtab, strtab) = (*tags.get(&6)? as usize, *tags.get(&5)? as usize);
    let n = match tags.get(&4) {
        Some(&h) => u(img, h as usize + 4, 4)? as usize,
        None => strtab.checked_sub(symtab)? / 24,
    };
    let mut out = Vec::new();
    for i in 1..n {
        let o = symtab + i * 24;
        let (name, shndx) = (u(img, o, 4)? as usize, u(img, o + 6, 2)?);
        let s = img.get(strtab + name..)?;
        let end = s.iter().position(|&c| c == 0)?;
        if shndx != 0 && end > 0 {
            out.push(s[..end].iter().map(|&c| c as char).collect());
        }
    }
    Some(out)
}

/// The online SDK `body` is, or `None`.
pub(crate) fn online(body: &[u8]) -> Option<&'static str> {
    let names = exports(body)?;
    ONLINE_SDKS
        .iter()
        .find(|(sym, _)| names.iter().any(|n| n == sym))
        .map(|(_, what)| *what)
}
