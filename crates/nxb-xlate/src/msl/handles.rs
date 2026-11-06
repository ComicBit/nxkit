//! The binding kind of a texture, image or sampler member, derived from the descriptor of the front end.
//!
//! A descriptor carries the constant buffer slots and the handle word of the guest texture. The handle decodes to the
//! driver table slots the host fills (`c2` texture, image and sampler tables) or to an offset in a guest uniform buffer.

use super::abi::{self, Kind};
use super::refl::Binding;
use crate::driver::TexInfo;

/// The decoded handle of a descriptor.
#[derive(Clone, Copy, Debug)]
struct Handle {
    /// Texture (or image) buffer slot, sampler buffer slot.
    tb: i64,
    sb: i64,
    o0: i64,
    o1: i64,
    ty: i64,
    /// The slots field was not set (`tcb`).
    tcb: bool,
}

fn handle(t: &TexInfo) -> Handle {
    let tcb = t.cbuf_slot < 0;
    let slots = t.cbuf_slot.max(0);
    let tb = if tcb { 2 } else { slots & 0xffff };
    let sb = if !tcb && (slots >> 16) != 0 {
        (slots >> 16) - 1
    } else {
        tb
    };
    let h = t.handle;
    Handle {
        tb,
        sb,
        o0: h & 0x3fff,
        o1: (h >> 14) & 0x3fff,
        ty: h >> 28,
        tcb,
    }
}

fn slot(word: i64, base: u32, cb: i64) -> Option<i64> {
    let (b, row) = (i64::from(base), i64::from(abi::C2_ROW));
    (cb == 2 && word * 4 >= b && (word * 4 - b) % row == 0).then(|| (word * 4 - b) / row)
}

/// The texture slot and the sampler slot of a c2 handle (`decode_tex`).
fn c2_slots(h: &Handle, image: bool) -> (Option<i64>, Option<i64>) {
    let words: Vec<i64> = if h.ty != 0 {
        vec![h.o0, h.o1]
    } else {
        vec![h.o0]
    };
    let (c2_img, c2_sep, c2_smp) = (
        i64::from(abi::C2_IMG),
        i64::from(abi::C2_SEPTEX),
        i64::from(abi::C2_SAMP),
    );
    let row = i64::from(abi::C2_ROW);
    if image {
        let s = words.iter().find_map(|x| {
            (c2_img <= x * 4 && x * 4 < c2_img + row * i64::from(abi::C2_IMG_SLOTS))
                .then(|| slot(*x, abi::C2_IMG, h.tb))
                .flatten()
        });
        return (s, None);
    }
    let tw: Vec<i64> = if h.ty == 3 { vec![h.o0] } else { words.clone() };
    let mut t = tw
        .iter()
        .find_map(|x| slot(*x, abi::C2_TEX, h.tb).filter(|_| x * 4 < c2_sep));
    if t.is_none() {
        t = tw.iter().find_map(|x| {
            slot(*x, abi::C2_SEPTEX, h.tb)
                .filter(|_| x * 4 < c2_smp)
                .map(|s| i64::from(abi::SEPTEX_SLOT0) + s)
        });
    }
    let s = if h.ty != 3 {
        words
            .iter()
            .find_map(|x| (x * 4 >= c2_smp).then(|| slot(*x, abi::C2_SAMP, h.sb)).flatten())
    } else {
        None
    };
    (t, s)
}

fn in_ubo(slot: i64) -> bool {
    (3..=16).contains(&slot)
}

/// What kind of member a texture, image or sampler is, and the facts of the kind.
pub struct Decoded {
    /// The kind.
    pub kind: Kind,
    /// `reg`.
    pub reg: Option<i64>,
    /// `off`.
    pub off: Option<u32>,
    /// `soff`.
    pub soff: Option<u32>,
    /// `sreg`.
    pub sreg: Option<u32>,
}

impl Decoded {
    fn of(kind: Kind, reg: Option<i64>) -> Decoded {
        Decoded {
            kind,
            reg,
            off: None,
            soff: None,
            sreg: None,
        }
    }

    /// Copy the facts to a binding.
    pub fn apply(&self, b: &mut Binding) {
        b.kind = self.kind;
        b.reg = self.reg;
        b.off = self.off;
        b.soff = self.soff;
        b.sreg = self.sreg;
    }
}

/// The member of a texture (`sampler` false) or the sampler of a descriptor.
///
/// `buffer` is a buffer texture, `array` is an array of handles, `dynamic` is a handle traced to no slot.
pub fn decode(t: &TexInfo, image: bool, sampler: bool, buffer: bool, array: bool) -> Decoded {
    let h = handle(t);
    let ubo_ok = !h.tcb
        && in_ubo(h.tb)
        && [0, 1, 3].contains(&h.ty)
        && !(h.ty == 1 && !in_ubo(h.sb));
    if image {
        if ubo_ok && !array {
            let mut d = Decoded::of(Kind::ImgUbo, Some(h.tb - 3));
            d.off = Some((h.o0 * 4) as u32);
            return d;
        }
        return Decoded::of(Kind::Img, c2_slots(&h, true).0);
    }
    if ubo_ok && !array {
        let tex = (h.tb - 3, (h.o0 * 4) as u32);
        let other = match h.ty {
            1 => Some((h.sb - 3, (h.o1 * 4) as u32)),
            _ => None,
        };
        if sampler {
            if h.ty == 3 {
                return Decoded::of(Kind::SampId, Some(h.o1));
            }
            let (r, o) = other.unwrap_or(tex);
            let mut d = Decoded::of(Kind::SampUbo, Some(r));
            d.off = Some(o);
            if other.is_some_and(|x| x != tex) {
                d.soff = Some(tex.1);
                d.sreg = Some(tex.0 as u32);
            }
            return d;
        }
        let mut d = Decoded::of(if buffer { Kind::TbufUbo } else { Kind::TexUbo }, Some(tex.0));
        d.off = Some(tex.1);
        if !buffer && let Some((r, o)) = other.filter(|x| *x != tex) {
            d.soff = Some(o);
            d.sreg = Some(r as u32);
        }
        return d;
    }
    if array && h.tb == 2 && h.ty == 0 && !h.tcb {
        let mut d = Decoded::of(if sampler { Kind::SampC2a } else { Kind::TexC2a }, None);
        d.off = Some((h.o0 * 4) as u32);
        return d;
    }
    if array && h.ty == 1 && !h.tcb && h.tb == 2 && h.sb == 2 {
        let mut d = Decoded::of(if sampler { Kind::SampC2a } else { Kind::TexC2a }, None);
        d.off = Some((h.o0 * 4) as u32);
        d.soff = Some((h.o1 * 4) as u32);
        return d;
    }
    let (ts, ss) = c2_slots(&h, false);
    if sampler {
        return match h.ty {
            3 => Decoded::of(Kind::SampId, Some(h.o1)),
            0 => Decoded::of(Kind::SampTex, ts),
            _ => Decoded::of(Kind::Samp, ss),
        };
    }
    Decoded::of(if buffer { Kind::Tbuf } else { Kind::Tex }, ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(cbuf_slot: i64, handle: i64) -> TexInfo {
        TexInfo {
            binding: 0,
            ty: 3,
            format: 0,
            cbuf_slot,
            handle,
            array_length: 1,
            separate: false,
            flags: 0,
        }
    }

    #[test]
    fn combined_c2_handle_is_a_tex_slot() {
        let word = (i64::from(abi::C2_TEX) + 8 * 5) / 4;
        let d = decode(&info(2, word), false, false, false, false);
        assert_eq!((d.kind, d.reg), (Kind::Tex, Some(5)));
        let s = decode(&info(2, word), false, true, false, false);
        assert_eq!((s.kind, s.reg), (Kind::SampTex, Some(5)));
    }

    #[test]
    fn guest_ubo_handle_is_bindless() {
        let d = decode(&info(4, 7), false, false, false, false);
        assert_eq!((d.kind, d.reg, d.off), (Kind::TexUbo, Some(1), Some(28)));
    }

    #[test]
    fn constant_sampler_has_an_id() {
        let h = (3i64 << 28) | (9 << 14) | 5;
        let s = decode(&info(2, h), false, true, false, false);
        assert_eq!((s.kind, s.reg), (Kind::SampId, Some(9)));
    }
}
