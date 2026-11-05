//! `VertexCounterLowering`: one analysed vertex shader (a visibility b.as_b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()hunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c))ounter) lowered to singleton subgroups.

use nxb_model::sha256;

const INSTANCE_CODE_HASH: &str = "f5f4d23149565acd852f6b7091f64cc8cb3aa019ccb33ba3f88ced62c6d1ff7b";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// True when the SPIR-V declares `OpTypeFloat 64` (the walk of `UsesFloat64`, starting at byte 20).
pub fn uses_float64(spirv: &[u8]) -> bool {
    let mut i = 20usize;
    while i + 12 <= spirv.len() {
        let word = u32::from_le_bytes([spirv[i], spirv[i + 1], spirv[i + 2], spirv[i + 3]]);
        let width = u32::from_le_bytes([spirv[i + 8], spirv[i + 9], spirv[i + 10], spirv[i + 11]]);
        if (word & 0xffff) == 22 && (word >> 16) == 3 && width == 64 {
            return true;
        }
        i += ((word >> 16) as usize).max(1) * 4;
    }
    false
}

/// Apply the lowering when `stage` and `code` are the analysed ones, else return the SPIR-V unchanged.
pub fn apply(
    code: &[u8],
    stage: &str,
    spirv: &[u8],
    notes: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    // the C# compares an upper-case hex string, the digest is the same
    if stage != "vs" || hex(&sha256(code).0) != INSTANCE_CODE_HASH {
        return Ok(spirv.to_vec());
    }
    lower(spirv, notes)
}

fn lower(spirv: &[u8], notes: &mut Vec<String>) -> Result<Vec<u8>, String> {
    let mut w = words(spirv);
    let (mut lane, mut uint_type, mut vector_type, mut true_id) = (0u32, 0u32, 0u32, 0u32);
    let (mut ballots, mut loads) = (0, 0);
    let mut i = 5;
    while i < w.len() {
        let op = w[i] & 0xffff;
        let at = |k: usize| w.get(i + k).copied().unwrap_or(0);
        if op == 71 && at(2) == 11 && at(3) == 41 {
            lane = at(1);
        }
        if op == 21 && at(2) == 32 && at(3) == 0 {
            uint_type = at(1);
        }
        if op == 41 {
            true_id = at(2);
        }
        if op == 339 {
            vector_type = at(1);
            ballots += 1;
            if at(4) != true_id {
                return Err("visibility counter: ballot predicate changed".into());
            }
        }
        i += ((w[i] >> 16) as usize).max(1);
    }
    if lane == 0 || uint_type == 0 || vector_type == 0 || ballots != 1 {
        return Err("visibility counter: SPIR-V shape changed".into());
    }
    let zero = w[3];
    let one = w[3] + 1;
    let mask = w[3] + 2;
    w[3] += 3;
    let mut r: Vec<u32> = w[..5].to_vec();
    let mut constants = false;
    let mut i = 5;
    while i < w.len() {
        let n = ((w[i] >> 16) as usize).max(1);
        let op = w[i] & 0xffff;
        if op == 54 && !constants {
            r.extend([
                (4 << 16) | 43,
                uint_type,
                zero,
                0,
                (4 << 16) | 43,
                uint_type,
                one,
                1,
                (7 << 16) | 44,
                vector_type,
                mask,
                one,
                zero,
                zero,
                zero,
            ]);
            constants = true;
        }
        if op == 61 && w[i + 3] == lane {
            r.extend([(4 << 16) | 83, w[i + 1], w[i + 2], zero]);
            loads += 1;
        } else if op == 339 {
            r.extend([(4 << 16) | 83, w[i + 1], w[i + 2], mask]);
        } else {
            r.extend_from_slice(&w[i..(i + n).min(w.len())]);
        }
        i += n;
    }
    if loads != 1 || !constants {
        return Err("visibility counter: lane id use changed".into());
    }
    notes.push(
        "visibility counter lowered to singleton groups (one atomic add per passing vertex)".into(),
    );
    Ok(r.iter().flat_map(|x| x.to_le_bytes()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float64_walk() {
        let mut b = vec![0u8; 20];
        for w in [(3u32 << 16) | 22, 7, 64] {
            b.extend(w.to_le_bytes());
        }
        assert!(uses_float64(&b));
        b[28] = 32;
        assert!(!uses_float64(&b));
    }
}
