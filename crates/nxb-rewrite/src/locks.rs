//! Two in-place word rewrites that run before the scans (`acquire_lock_sites` and `trinity_completer_sites` of
//! `patch_nso.py`): test-and-set lock acquires get acquire ordering, and the completer of Trinity's per-frame
//! rendezvous gets a `dmb ish`.

use crate::rw::Rw;
use crate::scan::data_blocks;

const DMB_ISH: u32 = 0xD503_3BBF;
const DMB_ISHLD: u32 = 0xD503_39BF;

fn word(img: &[u8], o: usize) -> u32 {
    img.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// `acquire_lock_sites(img, ranges)`: `ldxr rt,[rn]; cbnz rt,<ldxr>; stxr ws,rk,[rn]; cbnz ws,<ldxr>` with no
/// barrier after the loop.
fn acquire_lock_sites(img: &[u8], ranges: &[(i64, i64)]) -> Vec<usize> {
    let mut out = Vec::new();
    for &(s, e) in ranges {
        let mut o = s;
        while o < e - 12 {
            let ou = o as usize;
            let w = word(img, ou);
            if w & 0xBFFF_FC00 == 0x885F_7C00 {
                let (c1, st, c2) = (word(img, ou + 4), word(img, ou + 8), word(img, ou + 12));
                let (rt, rn, rs, rk) = (w & 31, (w >> 5) & 31, (st >> 16) & 31, st & 31);
                let cbnz = |x: u32, r: u32, back: u32| {
                    x & 0x7F00_001F == (0x3500_0000 | r) && (x >> 5) & 0x7FFFF == 0x80000 - back
                };
                let fenced = o + 20 <= e && matches!(word(img, ou + 16), DMB_ISH | DMB_ISHLD);
                if cbnz(c1, rt, 1)
                    && cbnz(c2, rs, 3)
                    && st & 0xBFE0_FC00 == 0x8800_7C00
                    && (st >> 5) & 31 == rn
                    && rk != 31
                    && rs != rn
                    && rs != rk
                    && !fenced
                {
                    out.push(ou);
                }
            }
            o += 4;
        }
    }
    out
}

/// `trinity_completer_sites(img, ranges)`.
fn trinity_completer_sites(img: &[u8], ranges: &[(i64, i64)]) -> Vec<usize> {
    let needle: Vec<u8> = [0x88DF_FD28u32, 0x7100_091F]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let mut out = Vec::new();
    for &(s, e) in ranges {
        let (s, e) = (s as usize, (e as usize).min(img.len()));
        let find = |from: usize| {
            (from <= e)
                .then(|| img[from..e].windows(8).position(|w| w == &needle[..]))
                .flatten()
                .map(|p| from + p)
        };
        let mut at = find(s);
        while let Some(o) = at {
            if o % 4 == 0 && o >= s + 40 && o + 12 <= e {
                let f = o - 40;
                let w: Vec<u32> = (0..13).map(|i| word(img, f + 4 * i)).collect();
                if w[..3] == [0xA9BE_7BFD, 0xA901_4FF4, 0x9100_03FD]
                    && w[3] & 0xFFC0_03FF == 0xF940_0008
                    && w[4] == 0x5280_0029
                    && w[5] & 0xFFC0_03FF == 0x3900_0109
                    && w[6] & 0xFFC0_03FF == 0x3940_0109
                    && (w[6] >> 10) & 0xFFF == ((w[5] >> 10) & 0xFFF) + 1
                    && w[7] & 0xFF00_001F == 0x3400_0009
                    && w[8] & 0xFFC0_03FF == 0xF940_0114
                    && w[9] & 0xFFC0_03FF == 0x9100_0289
                    && w[12] & 0xFF00_001F == 0x5400_0001
                {
                    out.push(f);
                }
            }
            at = find(o + 4);
        }
    }
    out
}

impl Rw {
    /// The word rewrites that come before the scans, in the order of the script.
    pub(crate) fn lock_rewrites(&mut self, log: &mut Vec<String>) -> crate::rw::R<()> {
        let blocks = data_blocks(&self.img, self.text_end)?;
        let mut ranges = Vec::new();
        let mut from = 0;
        for (s, e) in &blocks {
            ranges.push((from, *s));
            from = *e;
        }
        ranges.push((from, self.text_end));
        for o in acquire_lock_sites(&self.img, &ranges) {
            let w = word(&self.img, o) | 0x8000;
            self.img[o..o + 4].copy_from_slice(&w.to_le_bytes());
            self.stat("acquire_lock");
        }
        let t = trinity_completer_sites(&self.img, &ranges);
        if t.len() > 1 {
            let at: Vec<String> = t.iter().map(|o| format!("{o:#x}")).collect();
            log.push(format!(
                "[RED] {}: {} functions have the shape of Trinity's rendezvous completer, so none gets its barrier: {}",
                self.name,
                t.len(),
                at.join(", ")
            ));
        } else if let Some(&f) = t.first() {
            let moved: Vec<u32> = (3..6).map(|i| word(&self.img, f + 4 * i)).collect();
            for (i, w) in moved.iter().chain([&DMB_ISH]).enumerate() {
                self.img[f + 8 + 4 * i..f + 12 + 4 * i].copy_from_slice(&w.to_le_bytes());
            }
            self.stat("trinity_completer");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(w: &[u32]) -> Vec<u8> {
        w.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn a_test_and_set_lock_without_a_barrier_is_found() {
        let lock = [0x885F_7C08, 0x35FF_FFE8, 0x8809_7C0A, 0x35FF_FFA9];
        let mut open = lock.to_vec();
        open.extend([0xD503_201F; 4]);
        assert_eq!(acquire_lock_sites(&bytes(&open), &[(0, 32)]), [0]);
        let mut fenced = lock.to_vec();
        fenced.extend([DMB_ISH, 0xD503_201F, 0xD503_201F, 0xD503_201F]);
        assert!(acquire_lock_sites(&bytes(&fenced), &[(0, 32)]).is_empty());
    }

    #[test]
    fn trinity_completer_needs_the_whole_shape() {
        let mut w = vec![
            0xA9BE_7BFD,
            0xA901_4FF4,
            0x9100_03FD,
            0xF940_0008,
            0x5280_0029,
            0x3900_0109,
            0x3940_0509,
            0x3400_0009,
            0xF940_0114,
            0x9100_0289,
            0x88DF_FD28,
            0x7100_091F,
            0x5400_0001,
        ];
        // the strb at offset A and the ldrb at A + 1
        w[5] = 0x3900_0109;
        w[6] = 0x3940_0509;
        assert_eq!(trinity_completer_sites(&bytes(&w), &[(0, 52)]), [0]);
        w[4] = 0xD503_201F;
        assert!(trinity_completer_sites(&bytes(&w), &[(0, 52)]).is_empty());
    }
}
