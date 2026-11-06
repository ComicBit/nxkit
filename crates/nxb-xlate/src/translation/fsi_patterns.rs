//! The pattern trees of `FunctionMatch.PatternTrees`: the fragment shader interlock functions of the games.

use crate::maxwell::enums::{
    AtomSize, AvgMode, BoolOp, CacheOpLd, Ccc, IComp, LogicOp, LsSize, Membar, PredicateOp, RedOp,
    SReg, VoteMode, XmadCop,
};
use crate::maxwell::insts::{
    InstBfiI, InstBra, InstCal, InstFloR, InstIadd32i, InstIaddC, InstIscaddC, InstIscaddR,
    InstIsetpR, InstLdg, InstLop32i, InstLopR, InstMembar, InstPopcR, InstRed, InstRet, InstS2r,
    InstShrI, InstVote, InstXmadR,
};
use crate::maxwell::names::InstName;

/// One use of a pattern node. `index` is -1 for PT and RZ, `-(n + 2)` for the call argument `n`, else the output index.
#[derive(Clone, Copy)]
pub struct PUse {
    pub index: i32,
    pub inverted: bool,
    pub node: Option<usize>,
}

impl PUse {
    fn inv(self) -> PUse {
        PUse {
            inverted: !self.inverted,
            ..self
        }
    }
}

pub struct PNode {
    pub name: InstName,
    pub matches: Box<dyn Fn(u64) -> bool>,
    pub label: bool,
    pub order: u8,
    pub is_imm: bool,
    pub uses: Vec<PUse>,
}

/// The nodes of the pattern trees and the roots of each tree (the nodes nobody uses, in instruction order).
pub struct Patterns {
    pub nodes: Vec<PNode>,
    pub get_address: Vec<usize>,
    pub get_address_v2: Vec<usize>,
    pub is_last_warp_thread: Vec<usize>,
    pub begin: Vec<usize>,
    pub end: Vec<usize>,
}

fn pt() -> PUse {
    PUse {
        index: -1,
        inverted: false,
        node: None,
    }
}

fn call_arg(i: i32) -> PUse {
    PUse {
        index: -(i + 2),
        inverted: false,
        node: None,
    }
}

type M = Box<dyn Fn(u64) -> bool>;

impl Patterns {
    pub fn build() -> Patterns {
        let mut p = Patterns {
            nodes: Vec::new(),
            get_address: Vec::new(),
            get_address_v2: Vec::new(),
            is_last_warp_thread: Vec::new(),
            begin: Vec::new(),
            end: Vec::new(),
        };
        p.get_address = p.build_get_address();
        p.get_address_v2 = p.build_get_address_v2();
        p.is_last_warp_thread = p.build_is_last_warp_thread();
        p.begin = p.build_begin();
        p.end = p.build_end();
        p
    }

    fn add(&mut self, name: InstName, imm: bool, order: u8, uses: &[PUse], m: M) -> usize {
        self.nodes.push(PNode {
            name,
            matches: m,
            label: false,
            order,
            is_imm: imm,
            uses: uses.to_vec(),
        });
        self.nodes.len() - 1
    }

    fn out(&self, n: usize) -> PUse {
        self.out_at(n, 0)
    }

    fn out_at(&self, n: usize, index: i32) -> PUse {
        PUse {
            index,
            inverted: false,
            node: Some(n),
        }
    }

    fn s2r(&mut self, r: SReg) -> PUse {
        let n = self.add(
            InstName::S2r,
            false,
            0,
            &[pt()],
            Box::new(move |raw| InstS2r(raw).s_reg() == r),
        );
        self.out(n)
    }

    fn lop32i(&mut self, op: LogicOp, imm: i32, a: PUse) -> PUse {
        let m: M = Box::new(move |raw| {
            let i = InstLop32i(raw);
            !i.neg_a()
                && !i.neg_b()
                && !i.x()
                && !i.write_cc()
                && i.logic_op() == op
                && i.imm32() == imm
        });
        let n = self.add(InstName::Lop32i, false, 0, &[pt(), a], m);
        self.out(n)
    }

    fn shr_u32_w(&mut self, imm: i32, a: PUse) -> usize {
        let m: M = Box::new(move |raw| {
            let i = InstShrI(raw);
            !i.signed() && !i.brev() && i.m() && i.x_mode().0 == 0 && i.imm20() == imm
        });
        self.add(InstName::Shr, true, 0, &[pt(), a], m)
    }

    fn iadd32i(&mut self, imm: i32, a: PUse) -> usize {
        let m: M = Box::new(move |raw| {
            let i = InstIadd32i(raw);
            !i.sat()
                && !i.write_cc()
                && !i.x()
                && i.avg_mode() == AvgMode::NoNeg
                && i.imm32() == imm
        });
        self.add(InstName::Iadd32i, false, 0, &[pt(), a], m)
    }

    fn iadd_c(&mut self, x: bool, slot: i32, offset: i32) -> usize {
        let m: M = Box::new(move |raw| {
            let i = InstIaddC(raw);
            !i.sat()
                && !i.write_cc()
                && i.x() == x
                && i.avg_mode() == AvgMode::NoNeg
                && i.cbuf_slot() == slot
                && i.cbuf_offset() == offset
        });
        self.add(InstName::Iadd, false, 0, &[pt(), pt()], m)
    }

    fn iscadd_c(&mut self, imm: i32, slot: i32, offset: i32, a: PUse) -> usize {
        let m: M = Box::new(move |raw| {
            let i = InstIscaddC(raw);
            i.write_cc()
                && i.avg_mode() == AvgMode::NoNeg
                && i.imm5() == imm
                && i.cbuf_slot() == slot
                && i.cbuf_offset() == offset
        });
        self.add(InstName::Iscadd, false, 0, &[pt(), a], m)
    }

    fn iscadd_r(&mut self, a: PUse, b: PUse) -> PUse {
        let m: M = Box::new(|raw| {
            let i = InstIscaddR(raw);
            !i.write_cc() && i.avg_mode() == AvgMode::NoNeg && i.imm5() == 8
        });
        let n = self.add(InstName::Iscadd, false, 0, &[pt(), a, b], m);
        self.out(n)
    }

    fn bfi(&mut self, imm: i32, a: PUse, c: PUse) -> PUse {
        let m: M = Box::new(move |raw| {
            let i = InstBfiI(raw);
            !i.write_cc() && i.imm20() == imm
        });
        let n = self.add(InstName::Bfi, true, 0, &[pt(), a, c], m);
        self.out(n)
    }

    fn isetp(&mut self, comp: IComp, a: PUse, b: PUse) -> PUse {
        let m: M = Box::new(move |raw| {
            let i = InstIsetpR(raw);
            !i.signed() && i.i_comp() == comp && i.bop() == BoolOp::And
        });
        let n = self.add(InstName::Isetp, false, 0, &[pt(), pt(), a, b], m);
        self.out(n)
    }

    fn flo_u32(&mut self, a: PUse) -> PUse {
        let m: M = Box::new(|raw| {
            let i = InstFloR(raw);
            !i.signed() && !i.sh() && !i.neg_b() && !i.write_cc()
        });
        let n = self.add(InstName::Flo, false, 0, &[pt(), a], m);
        self.out(n)
    }

    fn vote(&mut self, mode: VoteMode, a: PUse) -> PUse {
        let m: M = Box::new(move |raw| InstVote(raw).vote_mode() == mode);
        let n = self.add(InstName::Vote, false, 0, &[pt(), a], m);
        self.out_at(n, 1)
    }

    fn popc(&mut self, a: PUse) -> PUse {
        let n = self.add(
            InstName::Popc,
            false,
            0,
            &[pt(), a],
            Box::new(|raw| !InstPopcR(raw).neg_b()),
        );
        self.out(n)
    }

    fn ret(&mut self, guard: PUse) -> usize {
        self.add(
            InstName::Ret,
            false,
            0,
            &[guard],
            Box::new(|raw| InstRet(raw).ccc() == Ccc::T),
        )
    }

    fn ldg_e(&mut self, order: u8, a: PUse) -> PUse {
        let m: M = Box::new(|raw| {
            let i = InstLdg(raw);
            i.e() && i.cache_op() == CacheOpLd::Cg && i.ls_size() == LsSize::B32
        });
        let n = self.add(InstName::Ldg, false, order, &[pt(), a], m);
        self.out(n)
    }

    /// `flags` are psl, mrg, hiloA and hiloB.
    fn xmad(&mut self, cop: XmadCop, flags: [bool; 4], uses: &[PUse]) -> PUse {
        let [psl, mrg, hilo_a, hilo_b] = flags;
        let m: M = Box::new(move |raw| {
            let i = InstXmadR(raw);
            i.xmad_cop() == cop
                && i.psl() == psl
                && i.mrg() == mrg
                && i.hilo_a() == hilo_a
                && i.hilo_b() == hilo_b
        });
        let n = self.add(InstName::Xmad, false, 0, uses, m);
        self.out(n)
    }

    fn high_u16_equals(&mut self, x: PUse) -> PUse {
        let shr = self.shr_u32_w(16, x);
        let shr = self.out(shr);
        self.isetp(IComp::Eq, shr, call_arg(3))
    }

    fn build_get_address(&mut self) -> Vec<usize> {
        let affinity = self.s2r(SReg::Affinity);
        let ticket = self.s2r(SReg::OrderingTicket);
        let a = self.lop32i(LogicOp::And, 0xff, affinity);
        let t = self.lop32i(LogicOp::And, 0xff, ticket);
        let inner = self.iscadd_r(a, t);
        let first = self.iscadd_c(2, 0, 404, inner);
        let shr = self.shr_u32_w(16, ticket);
        let hi = self.lop32i(LogicOp::And, 0xfe00, ticket);
        let second = self.iadd32i(0x200, hi);
        let third = self.iadd_c(true, 0, 405);
        let ret = self.ret(pt());
        vec![first, shr, second, third, ret]
    }

    fn build_get_address_v2(&mut self) -> Vec<usize> {
        let affinity = self.s2r(SReg::Affinity);
        let ticket = self.s2r(SReg::OrderingTicket);
        let shr = self.shr_u32_w(16, ticket);
        let hi = self.lop32i(LogicOp::And, 0xfe00, ticket);
        let second = self.iadd32i(0x200, hi);
        let lo = self.lop32i(LogicOp::And, 0xff, ticket);
        let bfi = self.bfi(0x808, affinity, lo);
        let third = self.iscadd_c(2, 0, 404, bfi);
        let fourth = self.iadd_c(true, 0, 405);
        let ret = self.ret(pt());
        vec![shr, second, third, fourth, ret]
    }

    fn build_is_last_warp_thread(&mut self) -> Vec<usize> {
        let thread_kill = self.s2r(SReg::ThreadKill);
        let lane_id = self.s2r(SReg::LaneId);
        let lop: M = Box::new(|raw| {
            let i = InstLopR(raw);
            !i.neg_a()
                && i.neg_b()
                && !i.write_cc()
                && !i.x()
                && i.lop() == LogicOp::PassB
                && i.predicate_op() == PredicateOp::F
        });
        let lop = self.add(InstName::Lop, false, 0, &[pt(), pt(), thread_kill], lop);
        let lop = self.out_at(lop, 1);
        let ne = self.isetp(IComp::Ne, lop, pt());
        let any = self.vote(VoteMode::Any, ne);
        let flo = self.flo_u32(any);
        let eq = self.isetp(IComp::Eq, flo, lane_id);
        let ret = self.ret(pt());
        vec![eq.node.unwrap_or(0), ret]
    }

    fn build_begin(&mut self) -> Vec<usize> {
        let cal = self.cal();
        let ret_guard = self.ret(call_arg(0).inv());
        let ldg = self.ldg_e(0, call_arg(1));
        let eq = self.high_u16_equals(ldg);
        let ret_loaded = self.ret(eq);
        let label = self.label();
        let ldg = self.ldg_e(1, call_arg(1));
        let eq = self.high_u16_equals(ldg).inv();
        let label_use = self.out(label);
        let bra = self.add(
            InstName::Bra,
            false,
            0,
            &[eq, label_use],
            Box::new(|raw| {
                let i = InstBra(raw);
                i.ccc() == Ccc::T && !i.ca()
            }),
        );
        let ret = self.ret(pt());
        vec![cal, ret_guard, ret_loaded, label, bra, ret]
    }

    fn build_end(&mut self) -> Vec<usize> {
        let vote = {
            let m: M = Box::new(|raw| InstVote(raw).vote_mode() == VoteMode::All);
            let n = self.add(InstName::Vote, false, 0, &[pt(), pt()], m);
            self.out_at(n, 1)
        };
        let popc = self.popc(vote);
        let thread_kill = self.s2r(SReg::ThreadKill);
        let lane_id = self.s2r(SReg::LaneId);
        let (address, increment) = (call_arg(1), call_arg(2));
        let cal = self.cal();
        let ret_guard = self.ret(call_arg(0).inv());
        let membar = self.add(
            InstName::Membar,
            false,
            0,
            &[pt()],
            Box::new(|raw| InstMembar(raw).membar() == Membar::Vc),
        );
        let ne = self.isetp(IComp::Ne, thread_kill, pt());
        let ret_killed = self.ret(ne);
        let flo = self.flo_u32(vote);
        let eq = self.isetp(IComp::Eq, flo, lane_id);
        let merged = self.xmad(
            XmadCop::Cfull,
            [false, true, false, true],
            &[pt(), increment, popc, pt()],
        );
        let full = self.xmad(XmadCop::Cfull, [false; 4], &[pt(), increment, popc, pt()]);
        let sum = self.xmad(
            XmadCop::Cbcc,
            [true, false, true, true],
            &[pt(), increment, merged, full],
        );
        let red: M = Box::new(|raw| {
            let i = InstRed(raw);
            i.e() && i.red_op() == RedOp::Add && i.red_size() == AtomSize::U32
        });
        let red = self.add(InstName::Red, false, 0, &[eq, address, sum], red);
        let ret = self.ret(pt());
        vec![cal, ret_guard, membar, ret_killed, red, ret]
    }

    fn cal(&mut self) -> usize {
        self.add(
            InstName::Cal,
            false,
            0,
            &[],
            Box::new(|raw| {
                let i = InstCal(raw);
                !i.ca() && i.inc()
            }),
        )
    }

    fn label(&mut self) -> usize {
        self.nodes.push(PNode {
            name: InstName::Invalid,
            matches: Box::new(|_| true),
            label: true,
            order: 0,
            is_imm: false,
            uses: Vec::new(),
        });
        self.nodes.len() - 1
    }
}
