//! LLVM bitstream writer: fixed and VBR fields, blocks with backpatched lengths, unabbreviated
//! records and one blob abbreviation (for the string table).

/// A bitstream being written.
#[derive(Default)]
pub struct Bits {
    out: Vec<u8>,
    cur: u64,
    nbits: u32,
    /// (abbrev width, word index of the length field) of each open block.
    blocks: Vec<(u32, usize)>,
    width: u32,
}

impl Bits {
    /// A stream with abbreviation width 2 (the top level).
    pub fn new() -> Bits {
        Bits {
            width: 2,
            ..Default::default()
        }
    }

    /// Writes `n` low bits of `v`.
    pub fn emit(&mut self, v: u64, n: u32) {
        if n == 0 {
            return;
        }
        let v = if n == 64 { v } else { v & ((1u64 << n) - 1) };
        self.cur |= v << self.nbits;
        self.nbits += n;
        while self.nbits >= 8 {
            self.out.push(self.cur as u8);
            self.cur >>= 8;
            self.nbits -= 8;
        }
        if n > 56 {
            // the bits shifted out above 64 when nbits was > 0
            let lost = (n + self.nbits).saturating_sub(64);
            let _ = lost;
        }
    }

    /// Writes `v` as a variable-width integer with `n`-bit chunks.
    pub fn vbr(&mut self, mut v: u64, n: u32) {
        let hi = 1u64 << (n - 1);
        while v >= hi {
            self.emit((v & (hi - 1)) | hi, n);
            v >>= n - 1;
        }
        self.emit(v, n);
    }

    /// Pads with zero bits to a 32-bit boundary.
    pub fn align32(&mut self) {
        let pos = self.out.len() * 8 + self.nbits as usize;
        let pad = (32 - pos % 32) % 32;
        let mut p = pad as u32;
        while p > 0 {
            let k = p.min(32);
            self.emit(0, k);
            p -= k;
        }
    }

    /// Bytes written so far (the stream must be 32-bit aligned).
    pub fn words(&self) -> usize {
        self.out.len() / 4
    }

    /// ENTER_SUBBLOCK with a new abbreviation width.
    pub fn enter(&mut self, id: u32, width: u32) {
        self.emit(1, self.width);
        self.vbr(id as u64, 8);
        self.vbr(width as u64, 4);
        self.align32();
        let at = self.out.len();
        self.emit(0, 32);
        self.blocks.push((self.width, at));
        self.width = width;
    }

    /// END_BLOCK, backpatching the block length.
    pub fn exit(&mut self) {
        self.emit(0, self.width);
        self.align32();
        let (w, at) = self.blocks.pop().expect("open block");
        let len = ((self.out.len() - at) / 4 - 1) as u32;
        self.out[at..at + 4].copy_from_slice(&len.to_le_bytes());
        self.width = w;
    }

    /// An unabbreviated record.
    pub fn record(&mut self, code: u32, ops: &[u64]) {
        self.emit(3, self.width);
        self.vbr(code as u64, 6);
        self.vbr(ops.len() as u64, 6);
        for &o in ops {
            self.vbr(o, 6);
        }
    }

    /// A record of `code` with one string operand array.
    pub fn record_str(&mut self, code: u32, prefix: &[u64], s: &str) {
        let mut ops = prefix.to_vec();
        ops.extend(s.bytes().map(|b| b as u64));
        self.record(code, &ops);
    }

    /// Defines abbreviation 4 in the current block as [literal code, blob] and writes one blob record.
    pub fn blob_record(&mut self, code: u32, blob: &[u8]) {
        // DEFINE_ABBREV: numops 2, op0 literal(code), op1 encoding blob (5)
        self.emit(2, self.width);
        self.vbr(2, 5);
        self.emit(1, 1);
        self.vbr(code as u64, 8);
        self.emit(0, 1);
        self.emit(5, 3);
        // the record with abbreviation 4
        self.emit(4, self.width);
        self.vbr(blob.len() as u64, 6);
        self.align32();
        for &b in blob {
            self.emit(b as u64, 8);
        }
        self.align32();
    }

    /// The finished bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.align32();
        self.out
    }

    /// Current position in bits.
    pub fn bit_pos(&self) -> u64 {
        self.out.len() as u64 * 8 + self.nbits as u64
    }
}

/// Signed VBR operand encoding (LLVM's emitSignedInt64).
pub fn signed(v: i64) -> u64 {
    if v >= 0 {
        (v as u64) << 1
    } else {
        ((v.unsigned_abs()) << 1) | 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vbr_and_fixed() {
        let mut b = Bits::new();
        b.emit(0b101, 3);
        b.vbr(100, 6);
        b.align32();
        let o = b.finish();
        assert_eq!(o.len(), 4);
        // 100 in vbr6: chunks of 5 bits: 100 = 0b11_00100 -> (00100|100000) then 00011
        assert_eq!(o[0] & 0b111, 0b101);
    }
}
