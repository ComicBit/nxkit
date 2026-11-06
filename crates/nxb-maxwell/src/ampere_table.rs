//! The Ampere (sm_86) opcode table of `InstEmitAmpere.cs`: opcode to name, props and emitter id. ALU instructions are
//! looked up by their low 9 opcode bits, the others by all 12, which is looked up first.

use super::ampere::{AmpOp, opc};
use super::names::InstName;
use super::table::props;
use std::sync::OnceLock;

/// Emitter ids (the C# `InstEmit.A*` methods).`nxb-xlate` maps them to emitters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AEmit {
    ABra,
    ABrx,
    AExit,
    AKill,
    ARet,
    ACall,
    ANop,
    AUnsupported,
    AFadd,
    AFmul,
    AFfma,
    AFmnmx,
    AFset,
    AFsetp,
    AFsel,
    AMufu,
    AF2i,
    AI2f,
    AF2i64,
    AI2f64,
    AI2fp,
    AFrnd,
    AF2f,
    AF2fp,
    AF2ip,
    AHadd2,
    AHmul2,
    AHfma2,
    AMov,
    ASel,
    AIsetp,
    AIadd3,
    AImad,
    AImadWide,
    AImadHi,
    ALop3,
    AShf,
    ALea,
    AImnmx,
    AIabs,
    APrmt,
    APopc,
    AFlo,
    ABrev,
    ASgxt,
    ABmsk,
    AGuardCull,
    APixelCovered,
    AUMov,
    AUp2ur,
    ACs2r,
    AS2r,
    AR2ur,
    AUldc,
    ALdc,
    AFswzadd,
    AR2p,
    AP2r,
    APlop3,
    AVote,
    AShfl,
    AIpa,
    AAld,
    AAst,
    ALdg,
    AAtoms,
    AStg,
    ARed,
    AAtomg,
    ALdLocalShared,
    AStLocalShared,
    AAtomsCas,
    AMembar,
    AErrbar,
    ATex,
    ATxq,
    ASurface,
    ASuatom,
    ASured,
    ABar,
    AHmma,
    ALdsm,
    AMovm,
    AMatch,
    ARedux,
    AHsetp2,
    AHset2,
    AHmnmx2,
    AOut,
    AIsberd,
    AStageOut,
    ALepc,
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: InstName,
    pub emit: AEmit,
    pub props: u16,
    /// a uniform-datapath instruction: its guard names a uniform predicate
    pub uniform: bool,
    pub writes_rd: bool,
}

struct Tables {
    exact: Vec<Option<Entry>>,
    alu: Vec<Option<Entry>>,
}

type T = Tables;

fn x(
    t: &mut T,
    op: usize,
    name: InstName,
    emit: AEmit,
    props: u16,
    uniform: bool,
    writes_rd: bool,
) {
    t.exact[op] = Some(Entry {
        name,
        emit,
        props,
        uniform,
        writes_rd,
    });
}

fn a(
    t: &mut T,
    op: usize,
    name: InstName,
    emit: AEmit,
    _props: u16,
    uniform: bool,
    writes_rd: bool,
) {
    t.alu[op] = Some(Entry {
        name,
        emit,
        props: 0,
        uniform,
        writes_rd,
    });
}

fn build() -> Tables {
    let mut t = Tables {
        exact: vec![None; 4096],
        alu: vec![None; 512],
    };
    let t = &mut t;
    let b = props::BRA;
    x(t, opc::BRA, InstName::Bra, AEmit::ABra, b, false, false);
    x(t, opc::BRX, InstName::Brx, AEmit::ABrx, b, false, false);
    x(t, opc::BRXU, InstName::Brx, AEmit::ABrx, b, false, false);
    x(t, opc::EXIT, InstName::Exit, AEmit::AExit, b, false, false);
    x(t, opc::KILL, InstName::Kil, AEmit::AKill, b, false, false);
    x(t, opc::RET, InstName::Ret, AEmit::ARet, b, false, false);
    x(t, opc::CALL, InstName::Cal, AEmit::ACall, b, false, false);
    for op in [
        0x918, 0x91a, 0x945, 0x941, 0x942, 0x948, 0x9ab, 0xf55, 0xf56,
    ] {
        x(t, op, InstName::Nop, AEmit::ANop, 0, false, false);
    }
    a(t, 0x021, InstName::Fadd, AEmit::AFadd, 0, false, true);
    a(t, 0x020, InstName::Fmul, AEmit::AFmul, 0, false, true);
    a(t, 0x023, InstName::Ffma, AEmit::AFfma, 0, false, true);
    a(t, 0x009, InstName::Fmnmx, AEmit::AFmnmx, 0, false, true);
    a(t, 0x00a, InstName::Fset, AEmit::AFset, 0, false, true);
    a(t, 0x00b, InstName::Fsetp, AEmit::AFsetp, 0, false, false);
    a(t, 0x008, InstName::Sel, AEmit::AFsel, 0, false, true);
    a(t, 0x108, InstName::Mufu, AEmit::AMufu, 0, false, true);
    a(t, 0x105, InstName::F2i, AEmit::AF2i, 0, false, true);
    a(t, 0x106, InstName::I2f, AEmit::AI2f, 0, false, true);
    a(t, 0x111, InstName::F2i, AEmit::AF2i64, 0, false, true);
    a(t, 0x112, InstName::I2f, AEmit::AI2f64, 0, false, true);
    a(t, 0x045, InstName::I2f, AEmit::AI2fp, 0, false, true);
    a(t, 0x107, InstName::F2f, AEmit::AFrnd, 0, false, true);
    a(t, 0x104, InstName::F2f, AEmit::AF2f, 0, false, true);
    a(t, 0x03e, InstName::F2f, AEmit::AF2fp, 0, false, true);
    a(t, 0x043, InstName::F2i, AEmit::AF2ip, 0, false, true);
    a(t, 0x030, InstName::Hadd2, AEmit::AHadd2, 0, false, true);
    a(t, 0x032, InstName::Hmul2, AEmit::AHmul2, 0, false, true);
    a(t, 0x031, InstName::Hfma2, AEmit::AHfma2, 0, false, true);
    a(t, 0x002, InstName::Mov, AEmit::AMov, 0, false, true);
    a(t, 0x007, InstName::Sel, AEmit::ASel, 0, false, true);
    a(t, 0x087, InstName::Sel, AEmit::ASel, 0, true, true);
    a(t, 0x00c, InstName::Isetp, AEmit::AIsetp, 0, false, false);
    a(t, 0x08c, InstName::Isetp, AEmit::AIsetp, 0, true, false);
    a(t, 0x010, InstName::Iadd3, AEmit::AIadd3, 0, false, true);
    a(t, 0x090, InstName::Iadd3, AEmit::AIadd3, 0, true, true);
    a(t, 0x024, InstName::Imad, AEmit::AImad, 0, false, true);
    a(t, 0x0a4, InstName::Imad, AEmit::AImad, 0, true, true);
    a(t, 0x025, InstName::Imad, AEmit::AImadWide, 0, false, true);
    a(t, 0x027, InstName::Imad, AEmit::AImadHi, 0, false, true);
    a(t, 0x0a5, InstName::Imad, AEmit::AImadWide, 0, true, true);
    a(t, 0x012, InstName::Lop3, AEmit::ALop3, 0, false, true);
    a(t, 0x092, InstName::Lop3, AEmit::ALop3, 0, true, true);
    a(t, 0x019, InstName::Shf, AEmit::AShf, 0, false, true);
    a(t, 0x099, InstName::Shf, AEmit::AShf, 0, true, true);
    a(t, 0x011, InstName::Lea, AEmit::ALea, 0, false, true);
    a(t, 0x091, InstName::Lea, AEmit::ALea, 0, true, true);
    a(t, 0x017, InstName::Imnmx, AEmit::AImnmx, 0, false, true);
    a(t, 0x085, InstName::Imnmx, AEmit::AImnmx, 0, true, true);
    a(t, 0x013, InstName::I2i, AEmit::AIabs, 0, false, true);
    a(t, 0x016, InstName::Prmt, AEmit::APrmt, 0, false, true);
    a(t, 0x096, InstName::Prmt, AEmit::APrmt, 0, true, true);
    a(t, 0x109, InstName::Popc, AEmit::APopc, 0, false, true);
    a(t, 0x0bf, InstName::Popc, AEmit::APopc, 0, true, true);
    a(t, 0x100, InstName::Flo, AEmit::AFlo, 0, false, true);
    a(t, 0x0bd, InstName::Flo, AEmit::AFlo, 0, true, true);
    a(t, 0x101, InstName::Flo, AEmit::ABrev, 0, false, true);
    a(t, 0x0be, InstName::Flo, AEmit::ABrev, 0, true, true);
    a(t, 0x01a, InstName::Bfe, AEmit::ASgxt, 0, false, true);
    a(t, 0x09a, InstName::Bfe, AEmit::ASgxt, 0, true, true);
    a(t, 0x01b, InstName::Bfe, AEmit::ABmsk, 0, false, true);
    x(t, 0x882, InstName::Mov, AEmit::AUMov, 0, true, true);
    x(t, 0x883, InstName::P2r, AEmit::AUp2ur, 0, true, true);
    x(t, 0xc82, InstName::Mov, AEmit::AUMov, 0, true, true);
    x(t, 0x805, InstName::Cs2r, AEmit::ACs2r, 0, false, true);
    x(
        t,
        0x925,
        InstName::Pixld,
        AEmit::APixelCovered,
        0,
        false,
        true,
    );
    x(t, 0x919, InstName::S2r, AEmit::AS2r, 0, false, true);
    x(t, 0x9c3, InstName::S2r, AEmit::AS2r, 0, true, true);
    x(t, 0x3c2, InstName::Mov, AEmit::AR2ur, 0, false, true);
    x(t, 0xab9, InstName::Ldc, AEmit::AUldc, 0, true, true);
    x(t, 0xabb, InstName::Ldc, AEmit::AUldc, 0, true, true);
    x(t, 0xb82, InstName::Ldc, AEmit::ALdc, 0, false, true);
    x(t, 0x822, InstName::Fswzadd, AEmit::AFswzadd, 0, false, true);
    x(t, 0x804, InstName::R2p, AEmit::AR2p, 0, false, false);
    x(t, 0x204, InstName::R2p, AEmit::AR2p, 0, false, false);
    x(t, 0x803, InstName::P2r, AEmit::AP2r, 0, false, true);
    x(t, 0x203, InstName::P2r, AEmit::AP2r, 0, false, true);
    x(t, 0x81c, InstName::Psetp, AEmit::APlop3, 0, false, false);
    x(t, 0x89c, InstName::Psetp, AEmit::APlop3, 0, true, false);
    x(
        t,
        0x80d,
        InstName::Isetp,
        AEmit::AGuardCull,
        0,
        false,
        false,
    );
    x(t, 0x806, InstName::Vote, AEmit::AVote, 0, false, true);
    x(t, 0x886, InstName::Vote, AEmit::AVote, 0, false, true);
    x(t, 0x389, InstName::Shfl, AEmit::AShfl, 0, false, true);
    x(t, 0x589, InstName::Shfl, AEmit::AShfl, 0, false, true);
    x(t, 0x989, InstName::Shfl, AEmit::AShfl, 0, false, true);
    x(t, 0xf89, InstName::Shfl, AEmit::AShfl, 0, false, true);
    x(t, 0x326, InstName::Ipa, AEmit::AIpa, 0, false, true);
    x(t, 0x526, InstName::Ipa, AEmit::AIpa, 0, false, true);
    x(t, 0x321, InstName::Ald, AEmit::AAld, 0, false, true);
    x(t, 0x322, InstName::Ast, AEmit::AAst, 0, false, false);
    x(t, 0x981, InstName::Ldg, AEmit::ALdg, 0, false, true);
    x(t, 0x381, InstName::Ldg, AEmit::ALdg, 0, false, true);
    x(t, 0xf8c, InstName::Atoms, AEmit::AAtoms, 0, false, true);
    x(t, 0x986, InstName::Stg, AEmit::AStg, 0, false, false);
    x(t, 0x386, InstName::Stg, AEmit::AStg, 0, false, false);
    x(t, 0x98e, InstName::Red, AEmit::ARed, 0, false, false);
    x(t, 0x9a8, InstName::Atom, AEmit::AAtomg, 0, false, true);
    x(t, 0x3a8, InstName::Atom, AEmit::AAtomg, 0, false, true);
    x(
        t,
        0x983,
        InstName::Ldl,
        AEmit::ALdLocalShared,
        0,
        false,
        true,
    );
    x(
        t,
        0x984,
        InstName::Lds,
        AEmit::ALdLocalShared,
        0,
        false,
        true,
    );
    x(
        t,
        0x387,
        InstName::Stl,
        AEmit::AStLocalShared,
        0,
        false,
        false,
    );
    x(
        t,
        0x987,
        InstName::Stl,
        AEmit::AStLocalShared,
        0,
        false,
        false,
    );
    x(
        t,
        0x388,
        InstName::Sts,
        AEmit::AStLocalShared,
        0,
        false,
        false,
    );
    x(
        t,
        0x988,
        InstName::Sts,
        AEmit::AStLocalShared,
        0,
        false,
        false,
    );
    x(t, 0x38c, InstName::Atoms, AEmit::AAtoms, 0, false, true);
    x(t, 0x38d, InstName::Atoms, AEmit::AAtomsCas, 0, false, true);
    x(t, 0x992, InstName::Membar, AEmit::AMembar, 0, false, false);
    x(t, 0x9ab, InstName::Membar, AEmit::AErrbar, 0, false, false);
    x(
        t,
        0xb60,
        InstName::Tex,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0x361,
        InstName::Tex,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0x364,
        InstName::Tld4,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0x367,
        InstName::Tld,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xf60,
        InstName::Tex,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xf63,
        InstName::Tld4,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xf6c,
        InstName::Txd,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xf69,
        InstName::Tmml,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0x36a,
        InstName::Tmml,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xb66,
        InstName::Tld,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xb63,
        InstName::Tld4,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xb69,
        InstName::Tmml,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xb6c,
        InstName::Txd,
        AEmit::ATex,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0xb6f,
        InstName::Txq,
        AEmit::ATxq,
        props::TEX,
        false,
        true,
    );
    x(
        t,
        0x370,
        InstName::Txq,
        AEmit::ATxq,
        props::TEX,
        false,
        true,
    );
    x(t, 0xb97, InstName::Suld, AEmit::ASurface, 0, false, true);
    x(t, 0xb99, InstName::Suld, AEmit::ASurface, 0, false, true);
    x(t, 0xb9b, InstName::Sust, AEmit::ASurface, 0, false, false);
    x(t, 0xb9d, InstName::Sust, AEmit::ASurface, 0, false, false);
    x(t, 0x593, InstName::Suatom, AEmit::ASuatom, 0, false, true);
    x(t, 0x59f, InstName::Sured, AEmit::ASured, 0, false, false);
    x(t, 0xb1d, InstName::Bar, AEmit::ABar, 0, false, false);
    x(t, 0x23c, InstName::Hfma2, AEmit::AHmma, 0, false, true);
    x(t, 0x83b, InstName::Lds, AEmit::ALdsm, 0, false, true);
    x(t, 0x23a, InstName::Mov, AEmit::AMovm, 0, false, true);
    x(t, 0x3a1, InstName::Vote, AEmit::AMatch, 0, false, true);
    x(t, 0x3c4, InstName::Vote, AEmit::ARedux, 0, false, false);
    a(t, 0x034, InstName::Hsetp2, AEmit::AHsetp2, 0, false, false);
    a(t, 0x033, InstName::Hset2, AEmit::AHset2, 0, false, true);
    a(t, 0x040, InstName::Hadd2, AEmit::AHmnmx2, 0, false, true);
    a(t, 0x124, InstName::Out, AEmit::AOut, 0, false, true);
    x(t, 0x923, InstName::Isberd, AEmit::AIsberd, 0, false, true);
    x(t, 0x927, InstName::Ast, AEmit::AStageOut, 0, false, false);
    x(t, 0x34e, InstName::Lepc, AEmit::ALepc, 0, false, true);
    x(t, 0x357, InstName::Nop, AEmit::ANop, 0, false, false);
    x(t, 0x957, InstName::Nop, AEmit::ANop, 0, false, false);
    Tables {
        exact: std::mem::take(&mut t.exact),
        alu: std::mem::take(&mut t.alu),
    }
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(build)
}

/// `TryGet`.
pub fn lookup(opcode: usize) -> Option<Entry> {
    let t = tables();
    t.exact[opcode & 0xfff].or(t.alu[opcode & 0x1ff])
}

/// `HasUniformGuard`.
pub fn has_uniform_guard(opcode: usize) -> bool {
    lookup(opcode).is_some_and(|e| e.uniform)
}

/// `ReadsConstantBank`: the instruction reads a constant buffer word with its bank in bits 54..59 (LDC, ULDC, or an ALU
/// operation in form 3 or 5, a uniform one has neither).
pub fn reads_constant_bank(x: &AmpOp) -> bool {
    let op = x.opcode();
    if matches!(op, 0xb82 | 0xab9 | 0xabb) {
        return true;
    }
    let t = tables();
    t.exact[op & 0xfff].is_none()
        && t.alu[op & 0x1ff].is_some_and(|e| !e.uniform)
        && matches!(x.form(), 3 | 5)
}

/// `WritesRd`.
pub fn writes_rd(opcode: usize) -> bool {
    lookup(opcode).is_none_or(|e| e.writes_rd)
}
