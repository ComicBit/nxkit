//! Texture and sampler pools: arrays of 33 or more handles.
//!
//! The front end sizes the array of a handle it cannot trace to one slot by the whole ID range of the driver pool
//! (`<stage>_t..._cb0_40000000`), or by the handles of one guest uniform buffer. Such an array is not declared as
//! `array<texture2d<float>, n>` in the argument buffer. The member is a pointer to a table the host keeps. The first
//! words of the table are the highest ID the host filled and the address of the bitmap of the IDs a shader reads. A
//! read clamps the index to the highest ID. A texture read also sets the bit of its ID, so the host makes only the
//! textures the shaders read.

use super::abi::{self, Kind};
use super::args::{Res, SET_NAME};
use super::handles::{Decoded, handle, ubo_handle};
use super::usage::Usage;
use crate::driver::TexInfo;

/// Arrays at least this long are pools.
pub const POOL_MIN: i32 = 33;

/// The helper that sets the bit of a texture ID. A bit is set once, so most reads cost a load and no atomic.
pub const MARK: &str = "template<typename P> static inline __attribute__((always_inline)) uint dm_pool_mark(constant P* p, uint i)
{
    device atomic_uint* w = reinterpret_cast<device atomic_uint*>(p->seen) + (i >> 5);
    uint b = 1u << (i & 31u);
    if (!(atomic_load_explicit(w, memory_order_relaxed) & b)) atomic_fetch_or_explicit(w, b, memory_order_relaxed);
    return i;
}

";

/// The kind of a pool member and the facts of the kind, or `None` if the array is not a pool.
///
/// A handle that no slot explains (`cbuf_slot` 0 and the ID `0x40000000`) is indexed by the texture or sampler ID.
/// A handle in a guest uniform buffer gives a table of that buffer's handles.
pub fn decode(t: &TexInfo, sampler: bool, len: i32) -> Option<Decoded> {
    if len < POOL_MIN {
        return None;
    }
    let h = handle(t);
    if t.cbuf_slot == 0 && t.handle == 0x4000_0000 {
        let kind = if sampler {
            Kind::SampPool
        } else {
            Kind::TexPool
        };
        return Some(Decoded {
            kind,
            reg: None,
            off: None,
            soff: None,
            sreg: None,
        });
    }
    if !ubo_handle(&h) {
        return None;
    }
    let kind = if sampler {
        Kind::SampHtab
    } else {
        Kind::TexHtab
    };
    let (soff, sreg) = if h.ty == 1 && h.sb == 2 {
        (Some((h.o1 * 4) as u32), Some(abi::SREG_C2))
    } else {
        (None, None)
    };
    Some(Decoded {
        kind,
        reg: Some(h.tb - 3),
        off: Some((h.o0 * 4) as u32),
        soff,
        sreg,
    })
}

/// The struct of the table of one pool.
pub fn strukt(name: &str, elem: &str) -> String {
    format!(
        "struct spvPool_{name}\n{{\n    uint n;\n    ulong seen;\n    array<{elem}, 1> e;\n}};\n\n"
    )
}

/// The type of the pointer member.
pub fn member_type(name: &str) -> String {
    format!("constant spvPool_{name}*")
}

/// The read of one element. A texture read marks its ID.
pub fn element(name: &str, index: &str, mark: bool) -> String {
    let base = format!("{SET_NAME}.{name}");
    let at = format!("min(uint({index}), {base}->n)");
    if mark {
        format!("(&{base}->e[0])[dm_pool_mark({base}, {at})]")
    } else {
        format!("(&{base}->e[0])[{at}]")
    }
}

/// A fragment shader whose texture pool marks the IDs tests depth early, so the mark does not change which pixels
/// the shader covers. A shader that discards, writes depth or writes memory keeps the late test.
pub fn early(res: &Res, usage: &Usage) -> bool {
    let marks = res
        .bindings
        .iter()
        .any(|b| b.kind == Kind::TexPool || b.kind == Kind::TexHtab);
    let writes =
        res.sbs.values().any(|b| b.write) || usage.img.values().any(|u| u.write || u.atomic);
    marks && !writes && !usage.discard && !usage.frag_depth
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
            array_length: 64,
            separate: false,
            flags: 0,
        }
    }

    #[test]
    fn short_arrays_are_no_pools() {
        assert!(decode(&info(0, 0x4000_0000), false, 32).is_none());
        assert!(decode(&info(0, 0x4000_0000), false, 33).is_some());
    }

    #[test]
    fn the_pool_of_the_driver_has_no_facts() {
        let d = decode(&info(0, 0x4000_0000), true, 4096).unwrap();
        assert_eq!((d.kind, d.reg, d.off), (Kind::SampPool, None, None));
    }

    #[test]
    fn a_table_of_a_uniform_buffer_names_the_buffer_and_the_word() {
        let d = decode(&info(7, 0x2a), false, 64).unwrap();
        assert_eq!((d.kind, d.reg, d.off), (Kind::TexHtab, Some(4), Some(0xa8)));
        assert_eq!((d.soff, d.sreg), (None, None));
    }

    #[test]
    fn a_driver_table_sampler_goes_to_the_other_half() {
        // Slots 6 | (2 + 1) << 16: texture words in c[6], the sampler word in the driver table.
        let h = (1i64 << 28) | (0x5c << 14) | 0x2a;
        let d = decode(&info(6 | (3 << 16), h), true, 64).unwrap();
        assert_eq!(
            (d.kind, d.reg, d.off),
            (Kind::SampHtab, Some(3), Some(0xa8))
        );
        assert_eq!((d.soff, d.sreg), (Some(0x170), Some(abi::SREG_C2)));
    }

    #[test]
    fn other_handles_are_no_pools() {
        assert!(decode(&info(2, 0x10), false, 64).is_none());
        assert!(decode(&info(-1, 0x10), false, 64).is_none());
    }

    #[test]
    fn a_read_clamps_the_index_and_a_texture_read_marks_it() {
        assert_eq!(
            element("t", "v1", false),
            "(&spvDescriptorSet0.t->e[0])[min(uint(v1), spvDescriptorSet0.t->n)]"
        );
        assert_eq!(
            element("t", "v1", true),
            "(&spvDescriptorSet0.t->e[0])[dm_pool_mark(spvDescriptorSet0.t, min(uint(v1), spvDescriptorSet0.t->n))]"
        );
    }
}
