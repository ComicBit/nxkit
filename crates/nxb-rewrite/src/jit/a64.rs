//! The AArch64 forms the JIT image moves: the four-word address sequences, the PC-relative branches, and the
//! `adrp` + `add` pair that replaces an address sequence.

/// `NOP`.
pub(crate) const NOP: u32 = 0xd503201f;

/// A PC-relative form (`branch_target`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `b`, `bl`: 26-bit displacement.
    B26,
    /// `b.cond`, `cbz`, `cbnz`: 19-bit displacement.
    B19,
    /// `tbz`, `tbnz`: 14-bit displacement.
    B14,
    /// `adr`, `adrp` and literal loads: the image does not move them.
    Lit,
}

/// The little-endian words of `code` (a length that is not a multiple of 4 drops the rest).
pub(crate) fn words(code: &[u8]) -> Vec<u32> {
    code.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// `seqs`: (byte position, register, value) of each `movz #, lsl #48; movk lsl #32; movk lsl #16; movk` on one register.
pub(crate) fn seqs(w: &[u32]) -> Vec<(usize, u32, u64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < w.len() {
        let a = w[i];
        if a & 0xffe00000 == 0xd2e00000 {
            let rd = a & 31;
            if w[i + 1] & 0xffe0001f == 0xf2c00000 | rd
                && w[i + 2] & 0xffe0001f == 0xf2a00000 | rd
                && w[i + 3] & 0xffe0001f == 0xf2800000 | rd
            {
                let imm = |x: u32| u64::from((x >> 5) & 0xffff);
                let v = imm(a) << 48 | imm(w[i + 1]) << 32 | imm(w[i + 2]) << 16 | imm(w[i + 3]);
                out.push((i * 4, rd, v));
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn sext(v: u32, bits: u32) -> i128 {
    let sign = 1i128 << (bits - 1);
    (i128::from(v) ^ sign) - sign
}

/// `branch_target`: the form and the target of a PC-relative instruction at `pos`, `None` for other instructions.
/// The target of [`Kind::Lit`] is `None`.
pub(crate) fn branch_target(word: u32, pos: i128) -> Option<(Kind, Option<i128>)> {
    if word & 0x7c000000 == 0x14000000 {
        return Some((Kind::B26, Some(pos + sext(word & 0x3ffffff, 26) * 4)));
    }
    if word & 0xff000010 == 0x54000000 || word & 0x7e000000 == 0x34000000 {
        return Some((Kind::B19, Some(pos + sext((word >> 5) & 0x7ffff, 19) * 4)));
    }
    if word & 0x7e000000 == 0x36000000 {
        return Some((Kind::B14, Some(pos + sext((word >> 5) & 0x3fff, 14) * 4)));
    }
    if word & 0x1f000000 == 0x10000000 || word & 0x3b000000 == 0x18000000 {
        return Some((Kind::Lit, None));
    }
    None
}

/// `set_branch`: `word` with the displacement `disp` (bytes, rounded down to words).
pub(crate) fn set_branch(word: u32, kind: Kind, disp: i128) -> Result<u32, String> {
    let d = disp.div_euclid(4);
    let (bits, keep, shift) = match kind {
        Kind::B26 => (26, 0xfc000000u32, 0),
        Kind::B19 => (19, 0xff00001f, 5),
        Kind::B14 | Kind::Lit => (14, 0xfff8001f, 5),
    };
    let half = 1i128 << (bits - 1);
    if !(-half..half).contains(&d) {
        return Err("branch out of reach".into());
    }
    let field = (d & ((1i128 << bits) - 1)) as u32;
    Ok(word & keep | field << shift)
}

/// `adrp_add`: `adrp xN; add xN, xN, #lo12; nop; nop` at `pos` for the address `target`.
pub(crate) fn adrp_add(rd: u32, pos: i128, target: i128) -> Result<[u32; 4], String> {
    let pages = (target >> 12) - (pos >> 12);
    if !(-(1i128 << 20)..1i128 << 20).contains(&pages) {
        return Err("adrp out of reach".into());
    }
    let adrp =
        0x90000000 | ((pages & 3) as u32) << 29 | (((pages >> 2) & 0x7ffff) as u32) << 5 | rd;
    let add = 0x91000000 | ((target & 0xfff) as u32) << 10 | rd << 5 | rd;
    Ok([adrp, add, NOP, NOP])
}
