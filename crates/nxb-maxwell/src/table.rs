//! `InstTable` and `InstOp`: opcode lookup by the top 14 bits.

use super::names::{Emit, InstName};
use super::table_gen::ROWS;
use std::sync::OnceLock;

pub mod props {
    pub const RD: u16 = 1;
    pub const RD2: u16 = 2;
    pub const RA: u16 = 4;
    pub const RB: u16 = 8;
    pub const IB: u16 = 32;
    pub const RC: u16 = 64;
    pub const TEX: u16 = 1 << 12;
    pub const TEX_B: u16 = 1 << 13;
    pub const BRA: u16 = 1 << 14;
    pub const NO_PRED: u16 = 1 << 15;
}

/// One decoded instruction word.
#[derive(Clone, Copy, Debug)]
pub struct InstOp {
    pub address: u64,
    pub raw: u64,
    /// Ampere: the upper 64 bits of the 128-bit word
    pub raw_hi: u64,
    /// Ampere: the emitter id (`emit` is `None`)
    pub aemit: Option<super::ampere_table::AEmit>,
    pub ampere: bool,
    pub name: InstName,
    pub emit: Option<Emit>,
    pub props: u16,
}

impl InstOp {
    /// An op with no instruction (`default`).
    pub const fn none() -> InstOp {
        InstOp {
            address: 0,
            raw: 0,
            raw_hi: 0,
            aemit: None,
            ampere: false,
            name: InstName::Invalid,
            emit: None,
            props: 0,
        }
    }

    pub fn has(&self, p: u16) -> bool {
        self.props & p == p
    }

    /// `GetAbsoluteAddress`: the branch target.
    pub fn absolute_address(&self) -> u64 {
        ((self.address as i64) + ((((self.raw >> 20) as i32) << 8) >> 8) as i64 + 8) as u64
    }
}

#[derive(Clone, Copy)]
struct Entry {
    name: InstName,
    emit: Option<Emit>,
    props: u16,
    xbits: i32,
}

fn build() -> Vec<Entry> {
    let mut t = vec![
        Entry {
            name: InstName::Invalid,
            emit: None,
            props: 0,
            xbits: 0
        };
        1 << 14
    ];
    for (enc, name, emit, props) in ROWS {
        let (mut value, mut xmask, mut xbits) = (0usize, 0usize, 0);
        let mut xpos = Vec::new();
        for (i, c) in enc.chars().enumerate() {
            let bit = 13 - i;
            if c == '1' {
                value |= 1 << bit;
            } else if c == 'x' {
                xmask |= 1 << bit;
                xpos.push(bit);
                xbits += 1;
            }
        }
        let xmask = !xmask & 0x3fff;
        let entry = Entry {
            name: *name,
            emit: Some(*emit),
            props: *props,
            xbits,
        };
        for index in 0..(1usize << xbits) {
            value &= xmask;
            for (x, p) in xpos.iter().enumerate() {
                value |= ((index >> x) & 1) << p;
            }
            if t[value].emit.is_none() || t[value].xbits > xbits {
                t[value] = entry;
            }
        }
    }
    t
}

/// `InstTable.GetOp`.
pub fn get_op(address: u64, raw: u64) -> InstOp {
    static T: OnceLock<Vec<Entry>> = OnceLock::new();
    let e = T.get_or_init(build)[(raw >> 50) as usize];
    InstOp {
        address,
        raw,
        raw_hi: 0,
        aemit: None,
        ampere: false,
        name: e.name,
        emit: e.emit,
        props: e.props,
    }
}
