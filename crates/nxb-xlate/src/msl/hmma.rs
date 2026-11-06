//! The Ampere HMMA.16816 helper with an f16 accumulator as `simdgroup_matrix` code, as an emitter choice.
//!
//! The tensor pass of the front end makes `dmsh_hmma16816_f16(lane, block, A0..A3, B0, B1, C0, C1, out D1) -> D0` and
//! gives it a body that computes D lane by lane from the subgroup's 256 word block of a scratch shared memory. The
//! emitter writes another body over the same block: A row-major (halves 0..255, 16 a row), B transposed (256..383, row n
//! of 16 halves of k, loaded transposed) and C and D row-major (384..511, 8 a row). On an M4 Pro one HMMA takes 1.08 ns
//! a SIMD-group, against 5.4 ns for the shuffle emulation.

use super::emit::{Em, R};
use super::types::{msl_type, scalar};
use crate::ir::StorageKind;
use crate::structured::ast::{A, Ast, Kind};
use crate::structured::types::AggType;

/// The name the front end gives the helper.
pub const HELPER: &str = "dmsh_hmma16816_f16";

/// The id of the scratch shared memory the body of function `main` reads.
fn scratch(ast: &Ast, main: A) -> Option<i32> {
    let mut found = None;
    visit(ast, main, &mut found);
    found
}

fn visit(ast: &Ast, a: A, found: &mut Option<i32>) {
    match ast.kind(a) {
        Kind::Block(_) => {
            for c in ast.children(a) {
                visit(ast, c, found);
            }
        }
        Kind::Assign { src, .. } => visit(ast, *src, found),
        Kind::Operation(o) => {
            if found.is_none() && o.storage_kind == StorageKind::SharedMemory {
                *found = o
                    .sources
                    .first()
                    .and_then(|s| super::usage::const_of(ast, *s));
            }
            for s in &o.sources {
                visit(ast, *s, found);
            }
        }
        _ => {}
    }
}

/// An unsigned word from an argument of any 32 bit type.
fn word(n: usize, t: AggType) -> String {
    if scalar(t) == AggType::FP32 {
        format!("as_type<uint>(a{n})")
    } else {
        format!("uint(a{n})")
    }
}

/// The body of function `i` when it is the HMMA helper.
pub(super) fn body(em: &Em<'_>, i: usize) -> R<Option<String>> {
    let f = &em.info.functions[i];
    if i == 0 || f.name != HELPER {
        return Ok(None);
    }
    if f.in_arguments.len() != 10 || f.out_arguments.len() != 1 {
        return Err(format!("unported: {HELPER} with unexpected parameters"));
    }
    let id = scratch(&em.info.ast, f.main_block).ok_or("HMMA helper without scratch memory")?;
    let space = if em.shared.spill {
        "device"
    } else {
        "threadgroup"
    };
    let w = |n: usize| word(n, f.in_arguments[n]);
    let d1 = msl_type(scalar(f.out_arguments[0]))?;
    let rt = msl_type(scalar(f.return_type))?;
    let flags = if em.shared.spill {
        "mem_flags::mem_device | mem_flags::mem_threadgroup"
    } else {
        "mem_flags::mem_threadgroup"
    };
    let (l, blk) = (w(0), w(1));
    let (a0, a1, a2, a3) = (w(2), w(3), w(4), w(5));
    let (b0, b1, c0, c1) = (w(6), w(7), w(8), w(9));
    let mut t = String::new();
    let mut put = |s: String| {
        t.push_str("    ");
        t.push_str(&s);
        t.push('\n');
    };
    put(format!("{space} uint* s = &sm{id}[{blk}];"));
    put(format!("uint g = {l} >> 2, t = {l} & 3;"));
    put(format!(
        "s[g * 8 + t] = {a0}; s[(g + 8) * 8 + t] = {a1}; s[g * 8 + 4 + t] = {a2}; s[(g + 8) * 8 + 4 + t] = {a3};"
    ));
    put(format!(
        "s[128 + g * 8 + t] = {b0}; s[128 + g * 8 + 4 + t] = {b1};"
    ));
    put(format!(
        "s[192 + g * 4 + t] = {c0}; s[192 + (g + 8) * 4 + t] = {c1};"
    ));
    put(format!("simdgroup_barrier({flags});"));
    put(format!("{space} half* h = ({space} half*)s;"));
    put("simdgroup_half8x8 ma00, ma01, ma10, ma11, mb0, mb1, mc0, mc1;".into());
    put("simdgroup_load(ma00, h, 16, ulong2(0, 0)); simdgroup_load(ma01, h, 16, ulong2(8, 0)); simdgroup_load(ma10, h, 16, ulong2(0, 8)); simdgroup_load(ma11, h, 16, ulong2(8, 8));".into());
    put("simdgroup_load(mb0, h + 256, 16, ulong2(0, 0), true); simdgroup_load(mb1, h + 256, 16, ulong2(8, 0), true);".into());
    put("simdgroup_load(mc0, h + 384, 8, ulong2(0, 0)); simdgroup_load(mc1, h + 384, 8, ulong2(0, 8));".into());
    put("simdgroup_multiply_accumulate(mc0, ma00, mb0, mc0); simdgroup_multiply_accumulate(mc0, ma01, mb1, mc0);".into());
    put("simdgroup_multiply_accumulate(mc1, ma10, mb0, mc1); simdgroup_multiply_accumulate(mc1, ma11, mb1, mc1);".into());
    put("simdgroup_store(mc0, h + 384, 8, ulong2(0, 0)); simdgroup_store(mc1, h + 384, 8, ulong2(0, 8));".into());
    put(format!("simdgroup_barrier({flags});"));
    put("uint d0 = s[192 + g * 4 + t], d1 = s[192 + (g + 8) * 4 + t];".into());
    put(format!("simdgroup_barrier({flags});"));
    put(format!("a10 = as_type<{d1}>(d1);"));
    put(format!("return as_type<{rt}>(d0);"));
    Ok(Some(t))
}
