//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AlSize(pub i32);
impl AlSize {
    pub const _32: AlSize = AlSize(0);
    pub const _64: AlSize = AlSize(1);
    pub const _96: AlSize = AlSize(2);
    pub const _128: AlSize = AlSize(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AtomSize(pub i32);
impl AtomSize {
    pub const U32: AtomSize = AtomSize(0);
    pub const S32: AtomSize = AtomSize(1);
    pub const U64: AtomSize = AtomSize(2);
    pub const F32FtzRn: AtomSize = AtomSize(3);
    pub const F16x2FtzRn: AtomSize = AtomSize(4);
    pub const S64: AtomSize = AtomSize(5);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AtomOp(pub i32);
impl AtomOp {
    pub const Add: AtomOp = AtomOp(0);
    pub const Min: AtomOp = AtomOp(1);
    pub const Max: AtomOp = AtomOp(2);
    pub const Inc: AtomOp = AtomOp(3);
    pub const Dec: AtomOp = AtomOp(4);
    pub const And: AtomOp = AtomOp(5);
    pub const Or: AtomOp = AtomOp(6);
    pub const Xor: AtomOp = AtomOp(7);
    pub const Exch: AtomOp = AtomOp(8);
    pub const Safeadd: AtomOp = AtomOp(10);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AtomsSize(pub i32);
impl AtomsSize {
    pub const U32: AtomsSize = AtomsSize(0);
    pub const S32: AtomsSize = AtomsSize(1);
    pub const U64: AtomsSize = AtomsSize(2);
    pub const S64: AtomsSize = AtomsSize(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BarMode(pub i32);
impl BarMode {
    pub const Bar: BarMode = BarMode(0);
    pub const Result: BarMode = BarMode(1);
    pub const Warp: BarMode = BarMode(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BarOp(pub i32);
impl BarOp {
    pub const Sync: BarOp = BarOp(0);
    pub const Arv: BarOp = BarOp(1);
    pub const Red: BarOp = BarOp(2);
    pub const Scan: BarOp = BarOp(3);
    pub const SyncAll: BarOp = BarOp(4);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BarRedOp(pub i32);
impl BarRedOp {
    pub const Popc: BarRedOp = BarRedOp(0);
    pub const And: BarRedOp = BarRedOp(1);
    pub const Or: BarRedOp = BarRedOp(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bpt(pub i32);
impl Bpt {
    pub const DrainIllegal: Bpt = Bpt(0);
    pub const Cal: Bpt = Bpt(1);
    pub const Pause: Bpt = Bpt(2);
    pub const Trap: Bpt = Bpt(3);
    pub const Int: Bpt = Bpt(4);
    pub const Drain: Bpt = Bpt(5);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ccc(pub i32);
impl Ccc {
    pub const F: Ccc = Ccc(0);
    pub const Lt: Ccc = Ccc(1);
    pub const Eq: Ccc = Ccc(2);
    pub const Le: Ccc = Ccc(3);
    pub const Gt: Ccc = Ccc(4);
    pub const Ne: Ccc = Ccc(5);
    pub const Ge: Ccc = Ccc(6);
    pub const Num: Ccc = Ccc(7);
    pub const Nan: Ccc = Ccc(8);
    pub const Ltu: Ccc = Ccc(9);
    pub const Equ: Ccc = Ccc(10);
    pub const Leu: Ccc = Ccc(11);
    pub const Gtu: Ccc = Ccc(12);
    pub const Neu: Ccc = Ccc(13);
    pub const Geu: Ccc = Ccc(14);
    pub const T: Ccc = Ccc(15);
    pub const Off: Ccc = Ccc(16);
    pub const Lo: Ccc = Ccc(17);
    pub const Sff: Ccc = Ccc(18);
    pub const Ls: Ccc = Ccc(19);
    pub const Hi: Ccc = Ccc(20);
    pub const Sft: Ccc = Ccc(21);
    pub const Hs: Ccc = Ccc(22);
    pub const Oft: Ccc = Ccc(23);
    pub const CsmTa: Ccc = Ccc(24);
    pub const CsmTr: Ccc = Ccc(25);
    pub const CsmMx: Ccc = Ccc(26);
    pub const FcsmTa: Ccc = Ccc(27);
    pub const FcsmTr: Ccc = Ccc(28);
    pub const FcsmMx: Ccc = Ccc(29);
    pub const Rle: Ccc = Ccc(30);
    pub const Rgt: Ccc = Ccc(31);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CacheType(pub i32);
impl CacheType {
    pub const U: CacheType = CacheType(1);
    pub const C: CacheType = CacheType(2);
    pub const I: CacheType = CacheType(3);
    pub const Crs: CacheType = CacheType(4);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CctlOp(pub i32);
impl CctlOp {
    pub const Pf1: CctlOp = CctlOp(1);
    pub const Pf1_5: CctlOp = CctlOp(2);
    pub const Pf2: CctlOp = CctlOp(3);
    pub const Wb: CctlOp = CctlOp(4);
    pub const Iv: CctlOp = CctlOp(5);
    pub const Ivall: CctlOp = CctlOp(6);
    pub const Rs: CctlOp = CctlOp(7);
    pub const Rslb: CctlOp = CctlOp(9);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CctltOp(pub i32);
impl CctltOp {
    pub const Ivth: CctltOp = CctltOp(1);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoolOp(pub i32);
impl BoolOp {
    pub const And: BoolOp = BoolOp(0);
    pub const Or: BoolOp = BoolOp(1);
    pub const Xor: BoolOp = BoolOp(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SReg(pub i32);
impl SReg {
    pub const LaneId: SReg = SReg(0);
    pub const Clock: SReg = SReg(1);
    pub const VirtCfg: SReg = SReg(2);
    pub const VirtId: SReg = SReg(3);
    pub const Pm0: SReg = SReg(4);
    pub const Pm1: SReg = SReg(5);
    pub const Pm2: SReg = SReg(6);
    pub const Pm3: SReg = SReg(7);
    pub const Pm4: SReg = SReg(8);
    pub const Pm5: SReg = SReg(9);
    pub const Pm6: SReg = SReg(10);
    pub const Pm7: SReg = SReg(11);
    pub const OrderingTicket: SReg = SReg(15);
    pub const PrimType: SReg = SReg(16);
    pub const InvocationId: SReg = SReg(17);
    pub const YDirection: SReg = SReg(18);
    pub const ThreadKill: SReg = SReg(19);
    pub const ShaderType: SReg = SReg(20);
    pub const DirectCbeWriteAddressLow: SReg = SReg(21);
    pub const DirectCbeWriteAddressHigh: SReg = SReg(22);
    pub const DirectCbeWriteEnabled: SReg = SReg(23);
    pub const MachineId0: SReg = SReg(24);
    pub const MachineId1: SReg = SReg(25);
    pub const MachineId2: SReg = SReg(26);
    pub const MachineId3: SReg = SReg(27);
    pub const Affinity: SReg = SReg(28);
    pub const InvocationInfo: SReg = SReg(29);
    pub const WScaleFactorXY: SReg = SReg(30);
    pub const WScaleFactorZ: SReg = SReg(31);
    pub const TId: SReg = SReg(32);
    pub const TIdX: SReg = SReg(33);
    pub const TIdY: SReg = SReg(34);
    pub const TIdZ: SReg = SReg(35);
    pub const CtaParam: SReg = SReg(36);
    pub const CtaIdX: SReg = SReg(37);
    pub const CtaIdY: SReg = SReg(38);
    pub const CtaIdZ: SReg = SReg(39);
    pub const Ntid: SReg = SReg(40);
    pub const CirQueueIncrMinusOne: SReg = SReg(41);
    pub const Nlatc: SReg = SReg(42);
    pub const Swinlo: SReg = SReg(48);
    pub const Swinsz: SReg = SReg(49);
    pub const Smemsz: SReg = SReg(50);
    pub const Smembanks: SReg = SReg(51);
    pub const LWinLo: SReg = SReg(52);
    pub const LWinSz: SReg = SReg(53);
    pub const LMemLoSz: SReg = SReg(54);
    pub const LMemHiOff: SReg = SReg(55);
    pub const EqMask: SReg = SReg(56);
    pub const LtMask: SReg = SReg(57);
    pub const LeMask: SReg = SReg(58);
    pub const GtMask: SReg = SReg(59);
    pub const GeMask: SReg = SReg(60);
    pub const RegAlloc: SReg = SReg(61);
    pub const CtxAddr: SReg = SReg(62);
    pub const GlobalErrorStatus: SReg = SReg(64);
    pub const WarpErrorStatus: SReg = SReg(66);
    pub const WarpErrorStatusClear: SReg = SReg(67);
    pub const PmHi0: SReg = SReg(72);
    pub const PmHi1: SReg = SReg(73);
    pub const PmHi2: SReg = SReg(74);
    pub const PmHi3: SReg = SReg(75);
    pub const PmHi4: SReg = SReg(76);
    pub const PmHi5: SReg = SReg(77);
    pub const PmHi6: SReg = SReg(78);
    pub const PmHi7: SReg = SReg(79);
    pub const ClockLo: SReg = SReg(80);
    pub const ClockHi: SReg = SReg(81);
    pub const GlobalTimerLo: SReg = SReg(82);
    pub const GlobalTimerHi: SReg = SReg(83);
    pub const HwTaskId: SReg = SReg(96);
    pub const CircularQueueEntryIndex: SReg = SReg(97);
    pub const CircularQueueEntryAddressLow: SReg = SReg(98);
    pub const CircularQueueEntryAddressHigh: SReg = SReg(99);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoundMode(pub i32);
impl RoundMode {
    pub const Rn: RoundMode = RoundMode(0);
    pub const Rm: RoundMode = RoundMode(1);
    pub const Rp: RoundMode = RoundMode(2);
    pub const Rz: RoundMode = RoundMode(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FComp(pub i32);
impl FComp {
    pub const F: FComp = FComp(0);
    pub const Lt: FComp = FComp(1);
    pub const Eq: FComp = FComp(2);
    pub const Le: FComp = FComp(3);
    pub const Gt: FComp = FComp(4);
    pub const Ne: FComp = FComp(5);
    pub const Ge: FComp = FComp(6);
    pub const Num: FComp = FComp(7);
    pub const Nan: FComp = FComp(8);
    pub const Ltu: FComp = FComp(9);
    pub const Equ: FComp = FComp(10);
    pub const Leu: FComp = FComp(11);
    pub const Gtu: FComp = FComp(12);
    pub const Neu: FComp = FComp(13);
    pub const Geu: FComp = FComp(14);
    pub const T: FComp = FComp(15);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntegerRound(pub i32);
impl IntegerRound {
    pub const Pass: IntegerRound = IntegerRound(1);
    pub const Round: IntegerRound = IntegerRound(4);
    pub const Floor: IntegerRound = IntegerRound(5);
    pub const Ceil: IntegerRound = IntegerRound(6);
    pub const Trunc: IntegerRound = IntegerRound(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IDstFmt(pub i32);
impl IDstFmt {
    pub const U16: IDstFmt = IDstFmt(1);
    pub const U32: IDstFmt = IDstFmt(2);
    pub const U64: IDstFmt = IDstFmt(3);
    pub const S16: IDstFmt = IDstFmt(5);
    pub const S32: IDstFmt = IDstFmt(6);
    pub const S64: IDstFmt = IDstFmt(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ISrcFmt(pub i32);
impl ISrcFmt {
    pub const U8: ISrcFmt = ISrcFmt(0);
    pub const U16: ISrcFmt = ISrcFmt(1);
    pub const U32: ISrcFmt = ISrcFmt(2);
    pub const U64: ISrcFmt = ISrcFmt(3);
    pub const S8: ISrcFmt = ISrcFmt(4);
    pub const S16: ISrcFmt = ISrcFmt(5);
    pub const S32: ISrcFmt = ISrcFmt(6);
    pub const S64: ISrcFmt = ISrcFmt(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ISrcDstFmt(pub i32);
impl ISrcDstFmt {
    pub const U8: ISrcDstFmt = ISrcDstFmt(0);
    pub const U16: ISrcDstFmt = ISrcDstFmt(1);
    pub const U32: ISrcDstFmt = ISrcDstFmt(2);
    pub const S8: ISrcDstFmt = ISrcDstFmt(4);
    pub const S16: ISrcDstFmt = ISrcDstFmt(5);
    pub const S32: ISrcDstFmt = ISrcDstFmt(6);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RoundMode2(pub i32);
impl RoundMode2 {
    pub const Round: RoundMode2 = RoundMode2(0);
    pub const Floor: RoundMode2 = RoundMode2(1);
    pub const Ceil: RoundMode2 = RoundMode2(2);
    pub const Trunc: RoundMode2 = RoundMode2(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChkModeF(pub i32);
impl ChkModeF {
    pub const Divide: ChkModeF = ChkModeF(0);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fmz(pub i32);
impl Fmz {
    pub const Ftz: Fmz = Fmz(1);
    pub const Fmz: Fmz = Fmz(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultiplyScale(pub i32);
impl MultiplyScale {
    pub const NoScale: MultiplyScale = MultiplyScale(0);
    pub const D2: MultiplyScale = MultiplyScale(1);
    pub const D4: MultiplyScale = MultiplyScale(2);
    pub const D8: MultiplyScale = MultiplyScale(3);
    pub const M8: MultiplyScale = MultiplyScale(4);
    pub const M4: MultiplyScale = MultiplyScale(5);
    pub const M2: MultiplyScale = MultiplyScale(6);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OFmt(pub i32);
impl OFmt {
    pub const F16: OFmt = OFmt(0);
    pub const F32: OFmt = OFmt(1);
    pub const MrgH0: OFmt = OFmt(2);
    pub const MrgH1: OFmt = OFmt(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HalfSwizzle(pub i32);
impl HalfSwizzle {
    pub const F16: HalfSwizzle = HalfSwizzle(0);
    pub const F32: HalfSwizzle = HalfSwizzle(1);
    pub const H0H0: HalfSwizzle = HalfSwizzle(2);
    pub const H1H1: HalfSwizzle = HalfSwizzle(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ByteSel(pub i32);
impl ByteSel {
    pub const B0: ByteSel = ByteSel(0);
    pub const B1: ByteSel = ByteSel(1);
    pub const B2: ByteSel = ByteSel(2);
    pub const B3: ByteSel = ByteSel(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DstFmt(pub i32);
impl DstFmt {
    pub const F16: DstFmt = DstFmt(1);
    pub const F32: DstFmt = DstFmt(2);
    pub const F64: DstFmt = DstFmt(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AvgMode(pub i32);
impl AvgMode {
    pub const NoNeg: AvgMode = AvgMode(0);
    pub const NegB: AvgMode = AvgMode(1);
    pub const NegA: AvgMode = AvgMode(2);
    pub const PlusOne: AvgMode = AvgMode(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Lrs(pub i32);
impl Lrs {
    pub const None: Lrs = Lrs(0);
    pub const RightShift: Lrs = Lrs(1);
    pub const LeftShift: Lrs = Lrs(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HalfSelect(pub i32);
impl HalfSelect {
    pub const B32: HalfSelect = HalfSelect(0);
    pub const H0: HalfSelect = HalfSelect(1);
    pub const H1: HalfSelect = HalfSelect(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IComp(pub i32);
impl IComp {
    pub const F: IComp = IComp(0);
    pub const Lt: IComp = IComp(1);
    pub const Eq: IComp = IComp(2);
    pub const Le: IComp = IComp(3);
    pub const Gt: IComp = IComp(4);
    pub const Ne: IComp = IComp(5);
    pub const Ge: IComp = IComp(6);
    pub const T: IComp = IComp(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct XMode(pub i32);
impl XMode {
    pub const Xlo: XMode = XMode(1);
    pub const Xmed: XMode = XMode(2);
    pub const Xhi: XMode = XMode(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IpaOp(pub i32);
impl IpaOp {
    pub const Pass: IpaOp = IpaOp(0);
    pub const Multiply: IpaOp = IpaOp(1);
    pub const Constant: IpaOp = IpaOp(2);
    pub const Sc: IpaOp = IpaOp(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IBase(pub i32);
impl IBase {
    pub const Patch: IBase = IBase(1);
    pub const Prim: IBase = IBase(2);
    pub const Attr: IBase = IBase(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CacheOpLd(pub i32);
impl CacheOpLd {
    pub const Ca: CacheOpLd = CacheOpLd(0);
    pub const Cg: CacheOpLd = CacheOpLd(1);
    pub const Ci: CacheOpLd = CacheOpLd(2);
    pub const Cv: CacheOpLd = CacheOpLd(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CacheOpSt(pub i32);
impl CacheOpSt {
    pub const Wb: CacheOpSt = CacheOpSt(0);
    pub const Cg: CacheOpSt = CacheOpSt(1);
    pub const Ci: CacheOpSt = CacheOpSt(2);
    pub const Wt: CacheOpSt = CacheOpSt(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LsSize(pub i32);
impl LsSize {
    pub const U8: LsSize = LsSize(0);
    pub const S8: LsSize = LsSize(1);
    pub const U16: LsSize = LsSize(2);
    pub const S16: LsSize = LsSize(3);
    pub const B32: LsSize = LsSize(4);
    pub const B64: LsSize = LsSize(5);
    pub const B128: LsSize = LsSize(6);
    pub const UB128: LsSize = LsSize(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LsSize2(pub i32);
impl LsSize2 {
    pub const U8: LsSize2 = LsSize2(0);
    pub const S8: LsSize2 = LsSize2(1);
    pub const U16: LsSize2 = LsSize2(2);
    pub const S16: LsSize2 = LsSize2(3);
    pub const B32: LsSize2 = LsSize2(4);
    pub const B64: LsSize2 = LsSize2(5);
    pub const B128: LsSize2 = LsSize2(6);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddressMode(pub i32);
impl AddressMode {
    pub const Il: AddressMode = AddressMode(1);
    pub const Is: AddressMode = AddressMode(2);
    pub const Isl: AddressMode = AddressMode(3);
}
