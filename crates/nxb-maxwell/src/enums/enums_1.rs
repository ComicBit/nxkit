//! Produced by tools/gen_xlate_decoders.py from InstDecoders.cs: do not edit.
#![allow(non_upper_case_globals, non_camel_case_types, dead_code, unused_parens, clippy::all)]

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CacheOp2(pub i32);
impl CacheOp2 {
    pub const Lu: CacheOp2 = CacheOp2(1);
    pub const Ci: CacheOp2 = CacheOp2(2);
    pub const Cv: CacheOp2 = CacheOp2(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PredicateOp(pub i32);
impl PredicateOp {
    pub const F: PredicateOp = PredicateOp(0);
    pub const T: PredicateOp = PredicateOp(1);
    pub const Z: PredicateOp = PredicateOp(2);
    pub const Nz: PredicateOp = PredicateOp(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogicOp(pub i32);
impl LogicOp {
    pub const And: LogicOp = LogicOp(0);
    pub const Or: LogicOp = LogicOp(1);
    pub const Xor: LogicOp = LogicOp(2);
    pub const PassB: LogicOp = LogicOp(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Membar(pub i32);
impl Membar {
    pub const Cta: Membar = Membar(0);
    pub const Gl: Membar = Membar(1);
    pub const Sys: Membar = Membar(2);
    pub const Vc: Membar = Membar(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ivall(pub i32);
impl Ivall {
    pub const Ivalld: Ivall = Ivall(1);
    pub const Ivallt: Ivall = Ivall(2);
    pub const Ivalltd: Ivall = Ivall(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MufuOp(pub i32);
impl MufuOp {
    pub const Cos: MufuOp = MufuOp(0);
    pub const Sin: MufuOp = MufuOp(1);
    pub const Ex2: MufuOp = MufuOp(2);
    pub const Lg2: MufuOp = MufuOp(3);
    pub const Rcp: MufuOp = MufuOp(4);
    pub const Rsq: MufuOp = MufuOp(5);
    pub const Rcp64h: MufuOp = MufuOp(6);
    pub const Rsq64h: MufuOp = MufuOp(7);
    pub const Sqrt: MufuOp = MufuOp(8);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutType(pub i32);
impl OutType {
    pub const Emit: OutType = OutType(1);
    pub const Cut: OutType = OutType(2);
    pub const EmitThenCut: OutType = OutType(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixMode(pub i32);
impl PixMode {
    pub const Covmask: PixMode = PixMode(1);
    pub const Covered: PixMode = PixMode(2);
    pub const Offset: PixMode = PixMode(3);
    pub const CentroidOffset: PixMode = PixMode(4);
    pub const MyIndex: PixMode = PixMode(5);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PMode(pub i32);
impl PMode {
    pub const F4e: PMode = PMode(1);
    pub const B4e: PMode = PMode(2);
    pub const Rc8: PMode = PMode(3);
    pub const Ecl: PMode = PMode(4);
    pub const Ecr: PMode = PMode(5);
    pub const Rc16: PMode = PMode(6);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RedOp(pub i32);
impl RedOp {
    pub const Add: RedOp = RedOp(0);
    pub const Min: RedOp = RedOp(1);
    pub const Max: RedOp = RedOp(2);
    pub const Inc: RedOp = RedOp(3);
    pub const Dec: RedOp = RedOp(4);
    pub const And: RedOp = RedOp(5);
    pub const Or: RedOp = RedOp(6);
    pub const Xor: RedOp = RedOp(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct XModeShf(pub i32);
impl XModeShf {
    pub const Hi: XModeShf = XModeShf(1);
    pub const X: XModeShf = XModeShf(2);
    pub const Xhi: XModeShf = XModeShf(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaxShift(pub i32);
impl MaxShift {
    pub const U64: MaxShift = MaxShift(2);
    pub const S64: MaxShift = MaxShift(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShflMode(pub i32);
impl ShflMode {
    pub const Idx: ShflMode = ShflMode(0);
    pub const Up: ShflMode = ShflMode(1);
    pub const Down: ShflMode = ShflMode(2);
    pub const Bfly: ShflMode = ShflMode(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Clamp(pub i32);
impl Clamp {
    pub const Ign: Clamp = Clamp(0);
    pub const Trap: Clamp = Clamp(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuatomSize(pub i32);
impl SuatomSize {
    pub const U32: SuatomSize = SuatomSize(0);
    pub const S32: SuatomSize = SuatomSize(1);
    pub const U64: SuatomSize = SuatomSize(2);
    pub const F32FtzRn: SuatomSize = SuatomSize(3);
    pub const F16x2FtzRn: SuatomSize = SuatomSize(4);
    pub const S64: SuatomSize = SuatomSize(5);
    pub const Sd32: SuatomSize = SuatomSize(6);
    pub const Sd64: SuatomSize = SuatomSize(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuDim(pub i32);
impl SuDim {
    pub const _1d: SuDim = SuDim(0);
    pub const _1dBuffer: SuDim = SuDim(1);
    pub const _1dArray: SuDim = SuDim(2);
    pub const _2d: SuDim = SuDim(3);
    pub const _2dArray: SuDim = SuDim(4);
    pub const _3d: SuDim = SuDim(5);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuatomOp(pub i32);
impl SuatomOp {
    pub const Add: SuatomOp = SuatomOp(0);
    pub const Min: SuatomOp = SuatomOp(1);
    pub const Max: SuatomOp = SuatomOp(2);
    pub const Inc: SuatomOp = SuatomOp(3);
    pub const Dec: SuatomOp = SuatomOp(4);
    pub const And: SuatomOp = SuatomOp(5);
    pub const Or: SuatomOp = SuatomOp(6);
    pub const Xor: SuatomOp = SuatomOp(7);
    pub const Exch: SuatomOp = SuatomOp(8);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuSize(pub i32);
impl SuSize {
    pub const U8: SuSize = SuSize(0);
    pub const S8: SuSize = SuSize(1);
    pub const U16: SuSize = SuSize(2);
    pub const S16: SuSize = SuSize(3);
    pub const B32: SuSize = SuSize(4);
    pub const B64: SuSize = SuSize(5);
    pub const B128: SuSize = SuSize(6);
    pub const UB128: SuSize = SuSize(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuRgba(pub i32);
impl SuRgba {
    pub const R: SuRgba = SuRgba(1);
    pub const G: SuRgba = SuRgba(2);
    pub const Rg: SuRgba = SuRgba(3);
    pub const B: SuRgba = SuRgba(4);
    pub const Rb: SuRgba = SuRgba(5);
    pub const Gb: SuRgba = SuRgba(6);
    pub const Rgb: SuRgba = SuRgba(7);
    pub const A: SuRgba = SuRgba(8);
    pub const Ra: SuRgba = SuRgba(9);
    pub const Ga: SuRgba = SuRgba(10);
    pub const Rga: SuRgba = SuRgba(11);
    pub const Ba: SuRgba = SuRgba(12);
    pub const Rba: SuRgba = SuRgba(13);
    pub const Gba: SuRgba = SuRgba(14);
    pub const Rgba: SuRgba = SuRgba(15);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Lod(pub i32);
impl Lod {
    pub const Lz: Lod = Lod(1);
    pub const Lb: Lod = Lod(2);
    pub const Ll: Lod = Lod(3);
    pub const Lba: Lod = Lod(6);
    pub const Lla: Lod = Lod(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexDim(pub i32);
impl TexDim {
    pub const _1d: TexDim = TexDim(0);
    pub const Array1d: TexDim = TexDim(1);
    pub const _2d: TexDim = TexDim(2);
    pub const Array2d: TexDim = TexDim(3);
    pub const _3d: TexDim = TexDim(4);
    pub const Array3d: TexDim = TexDim(5);
    pub const Cube: TexDim = TexDim(6);
    pub const ArrayCube: TexDim = TexDim(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexsTarget(pub i32);
impl TexsTarget {
    pub const Texture1DLodZero: TexsTarget = TexsTarget(0);
    pub const Texture2D: TexsTarget = TexsTarget(1);
    pub const Texture2DLodZero: TexsTarget = TexsTarget(2);
    pub const Texture2DLodLevel: TexsTarget = TexsTarget(3);
    pub const Texture2DDepthCompare: TexsTarget = TexsTarget(4);
    pub const Texture2DLodLevelDepthCompare: TexsTarget = TexsTarget(5);
    pub const Texture2DLodZeroDepthCompare: TexsTarget = TexsTarget(6);
    pub const Texture2DArray: TexsTarget = TexsTarget(7);
    pub const Texture2DArrayLodZero: TexsTarget = TexsTarget(8);
    pub const Texture2DArrayLodZeroDepthCompare: TexsTarget = TexsTarget(9);
    pub const Texture3D: TexsTarget = TexsTarget(10);
    pub const Texture3DLodZero: TexsTarget = TexsTarget(11);
    pub const TextureCube: TexsTarget = TexsTarget(12);
    pub const TextureCubeLodLevel: TexsTarget = TexsTarget(13);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TldsTarget(pub i32);
impl TldsTarget {
    pub const Texture1DLodZero: TldsTarget = TldsTarget(0);
    pub const Texture1DLodLevel: TldsTarget = TldsTarget(1);
    pub const Texture2DLodZero: TldsTarget = TldsTarget(2);
    pub const Texture2DLodZeroOffset: TldsTarget = TldsTarget(4);
    pub const Texture2DLodLevel: TldsTarget = TldsTarget(5);
    pub const Texture2DLodZeroMultisample: TldsTarget = TldsTarget(6);
    pub const Texture3DLodZero: TldsTarget = TldsTarget(7);
    pub const Texture2DArrayLodZero: TldsTarget = TldsTarget(8);
    pub const Texture2DLodLevelOffset: TldsTarget = TldsTarget(12);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexComp(pub i32);
impl TexComp {
    pub const R: TexComp = TexComp(0);
    pub const G: TexComp = TexComp(1);
    pub const B: TexComp = TexComp(2);
    pub const A: TexComp = TexComp(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexOffset(pub i32);
impl TexOffset {
    pub const None: TexOffset = TexOffset(0);
    pub const Aoffi: TexOffset = TexOffset(1);
    pub const Ptp: TexOffset = TexOffset(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexQuery(pub i32);
impl TexQuery {
    pub const TexHeaderDimension: TexQuery = TexQuery(1);
    pub const TexHeaderTextureType: TexQuery = TexQuery(2);
    pub const TexHeaderSamplerPos: TexQuery = TexQuery(5);
    pub const TexSamplerFilter: TexQuery = TexQuery(16);
    pub const TexSamplerLod: TexQuery = TexQuery(18);
    pub const TexSamplerWrap: TexQuery = TexQuery(20);
    pub const TexSamplerBorderColor: TexQuery = TexQuery(22);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VectorSelect(pub i32);
impl VectorSelect {
    pub const U8B0: VectorSelect = VectorSelect(0);
    pub const U8B1: VectorSelect = VectorSelect(1);
    pub const U8B2: VectorSelect = VectorSelect(2);
    pub const U8B3: VectorSelect = VectorSelect(3);
    pub const U16H0: VectorSelect = VectorSelect(4);
    pub const U16H1: VectorSelect = VectorSelect(5);
    pub const U32: VectorSelect = VectorSelect(6);
    pub const S8B0: VectorSelect = VectorSelect(8);
    pub const S8B1: VectorSelect = VectorSelect(9);
    pub const S8B2: VectorSelect = VectorSelect(10);
    pub const S8B3: VectorSelect = VectorSelect(11);
    pub const S16H0: VectorSelect = VectorSelect(12);
    pub const S16H1: VectorSelect = VectorSelect(13);
    pub const S32: VectorSelect = VectorSelect(14);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VideoOp(pub i32);
impl VideoOp {
    pub const Mrg16h: VideoOp = VideoOp(0);
    pub const Mrg16l: VideoOp = VideoOp(1);
    pub const Mrg8b0: VideoOp = VideoOp(2);
    pub const Mrg8b2: VideoOp = VideoOp(3);
    pub const Acc: VideoOp = VideoOp(4);
    pub const Min: VideoOp = VideoOp(5);
    pub const Max: VideoOp = VideoOp(6);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VideoRed(pub i32);
impl VideoRed {
    pub const Acc: VideoRed = VideoRed(1);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LaneMask4(pub i32);
impl LaneMask4 {
    pub const Z: LaneMask4 = LaneMask4(1);
    pub const W: LaneMask4 = LaneMask4(2);
    pub const Zw: LaneMask4 = LaneMask4(3);
    pub const X: LaneMask4 = LaneMask4(4);
    pub const Xz: LaneMask4 = LaneMask4(5);
    pub const Xw: LaneMask4 = LaneMask4(6);
    pub const Xzw: LaneMask4 = LaneMask4(7);
    pub const Y: LaneMask4 = LaneMask4(8);
    pub const Yz: LaneMask4 = LaneMask4(9);
    pub const Yw: LaneMask4 = LaneMask4(10);
    pub const Yzw: LaneMask4 = LaneMask4(11);
    pub const Xy: LaneMask4 = LaneMask4(12);
    pub const Xyz: LaneMask4 = LaneMask4(13);
    pub const Xyw: LaneMask4 = LaneMask4(14);
    pub const Xyzw: LaneMask4 = LaneMask4(15);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ASelect4(pub i32);
impl ASelect4 {
    pub const _0000: ASelect4 = ASelect4(0);
    pub const _1111: ASelect4 = ASelect4(1);
    pub const _2222: ASelect4 = ASelect4(2);
    pub const _3333: ASelect4 = ASelect4(3);
    pub const _3210: ASelect4 = ASelect4(4);
    pub const _5432: ASelect4 = ASelect4(6);
    pub const _6543: ASelect4 = ASelect4(7);
    pub const _3201: ASelect4 = ASelect4(8);
    pub const _3012: ASelect4 = ASelect4(9);
    pub const _0213: ASelect4 = ASelect4(10);
    pub const _3120: ASelect4 = ASelect4(11);
    pub const _1230: ASelect4 = ASelect4(12);
    pub const _2310: ASelect4 = ASelect4(13);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BSelect4(pub i32);
impl BSelect4 {
    pub const _4444: BSelect4 = BSelect4(0);
    pub const _5555: BSelect4 = BSelect4(1);
    pub const _6666: BSelect4 = BSelect4(2);
    pub const _7777: BSelect4 = BSelect4(3);
    pub const _7654: BSelect4 = BSelect4(4);
    pub const _5432: BSelect4 = BSelect4(6);
    pub const _4321: BSelect4 = BSelect4(7);
    pub const _4567: BSelect4 = BSelect4(8);
    pub const _6745: BSelect4 = BSelect4(9);
    pub const _5476: BSelect4 = BSelect4(10);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VideoScale(pub i32);
impl VideoScale {
    pub const Shr7: VideoScale = VideoScale(1);
    pub const Shr15: VideoScale = VideoScale(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoteMode(pub i32);
impl VoteMode {
    pub const All: VoteMode = VoteMode(0);
    pub const Any: VoteMode = VoteMode(1);
    pub const Eq: VoteMode = VoteMode(2);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct XmadCop(pub i32);
impl XmadCop {
    pub const Cfull: XmadCop = XmadCop(0);
    pub const Clo: XmadCop = XmadCop(1);
    pub const Chi: XmadCop = XmadCop(2);
    pub const Csfu: XmadCop = XmadCop(3);
    pub const Cbcc: XmadCop = XmadCop(4);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct XmadCop2(pub i32);
impl XmadCop2 {
    pub const Cfull: XmadCop2 = XmadCop2(0);
    pub const Clo: XmadCop2 = XmadCop2(1);
    pub const Chi: XmadCop2 = XmadCop2(2);
    pub const Csfu: XmadCop2 = XmadCop2(3);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImadspASelect(pub i32);
impl ImadspASelect {
    pub const U32: ImadspASelect = ImadspASelect(0);
    pub const S32: ImadspASelect = ImadspASelect(1);
    pub const U24: ImadspASelect = ImadspASelect(2);
    pub const S24: ImadspASelect = ImadspASelect(3);
    pub const U16h0: ImadspASelect = ImadspASelect(4);
    pub const S16h0: ImadspASelect = ImadspASelect(5);
    pub const U16h1: ImadspASelect = ImadspASelect(6);
    pub const S16h1: ImadspASelect = ImadspASelect(7);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImadspBSelect(pub i32);
impl ImadspBSelect {
    pub const U24: ImadspBSelect = ImadspBSelect(0);
    pub const S24: ImadspBSelect = ImadspBSelect(1);
    pub const U16h0: ImadspBSelect = ImadspBSelect(2);
    pub const S16h0: ImadspBSelect = ImadspBSelect(3);
}
