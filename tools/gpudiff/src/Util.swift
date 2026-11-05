// gpudiff: utilities (deterministic RNG, value generators, JSON helpers)
import Foundation

/// xoshiro256** seeded through splitmix64. Every input byte the harness generates comes from one of these,
/// seeded from (run seed, vector, resource name), so reference and candidate always see identical inputs.
struct RNG {
    var s: (UInt64, UInt64, UInt64, UInt64)
    /// value profile of the vector: 0 sane (no NaN, Inf or denormals: the shader's ordinary paths), 1 mixed, 2 edge-heavy
    var profile = 1
    init(_ seed: UInt64, profile: Int = 1) {
        self.profile = profile
        var x = seed &+ 0x9E3779B97F4A7C15
        func sm(_ x: inout UInt64) -> UInt64 {
            x = x &+ 0x9E3779B97F4A7C15
            var z = x
            z = (z ^ (z >> 30)) &* 0xBF58476D1CE4E5B9
            z = (z ^ (z >> 27)) &* 0x94D049BB133111EB
            return z ^ (z >> 31)
        }
        s = (sm(&x), sm(&x), sm(&x), sm(&x))
        _ = 0
    }
    @inline(__always) static func rotl(_ x: UInt64, _ k: UInt64) -> UInt64 { (x << k) | (x >> (64 - k)) }
    @inline(__always) mutating func next() -> UInt64 {
        let r = RNG.rotl(s.1 &* 5, 7) &* 9
        let t = s.1 << 17
        s.2 ^= s.0; s.3 ^= s.1; s.1 ^= s.2; s.0 ^= s.3; s.2 ^= t; s.3 = RNG.rotl(s.3, 45)
        return r
    }
    @inline(__always) mutating func u32() -> UInt32 { UInt32(truncatingIfNeeded: next() >> 32) }
    @inline(__always) mutating func below(_ n: Int) -> Int { n <= 1 ? 0 : Int(next() % UInt64(n)) }
    @inline(__always) mutating func unit() -> Float { Float(next() >> 40) / Float(1 << 24) }   // [0,1)
    @inline(__always) mutating func range(_ lo: Float, _ hi: Float) -> Float { lo + (hi - lo) * unit() }
    mutating func pick<T>(_ a: [T]) -> T { a[below(a.count)] }
}

func fnv1a(_ s: String) -> UInt64 {
    var h: UInt64 = 0xcbf29ce484222325
    for b in s.utf8 { h ^= UInt64(b); h = h &* 0x100000001b3 }
    return h
}
func seedFor(_ base: UInt64, _ parts: String...) -> UInt64 {
    var h = base ^ 0x5bd1e995
    for p in parts { h = (h &* 0x100000001b3) ^ fnv1a(p) }
    return h
}

// MARK: value classes ---------------------------------------------------------------------------

/// Float bit patterns the harness treats as edge cases: signed zeros, infinities, quiet and signalling NaNs with payloads,
/// denormals, the normal range limits and the neighbours of 1.0.
let edgeFloatBits: [UInt32] = [
    0x00000000, 0x80000000, 0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000, 0x7fa00001, 0x7f800001, 0xffbfffff,
    0x00000001, 0x80000001, 0x007fffff, 0x807fffff, 0x00800000, 0x80800000, 0x7f7fffff, 0xff7fffff,
    0x3f7fffff, 0x3f800001, 0xbf800000, 0x477fe000, 0x4b800000, 0x4b800001, 0xcb800000, 0x33800000, 0x5f000000, 0xdf000000,
]
let niceFloats: [Float] = [0, 1, -1, 0.5, -0.5, 2, -2, 0.25, 3, 4, 8, 16, 0.1, 255, 1.0/3.0, 0.75, 1.5, 100, -100, 0.001, 1024, 65535, 0.9999]

/// "f": the default mixture for float data: mostly moderate values (so comparisons go both ways), plus edge cases,
/// wide exponents, small integer bit patterns (denormals as float, small ints when reinterpreted) and raw bits.
/// profile 3, "degenerate": exact zeros, ones and halves dominate, so comparisons hit their boundaries (x <= 0.0 with x
/// exactly 0) and products vanish exactly
let degenerateFloats: [Float] = [0, 0, 0, 1, 1, -1, 0.5, 2, -0.0]
@inline(__always) func genF(_ r: inout RNG) -> UInt32 {
    if r.profile == 3 { return r.below(4) == 0 ? r.range(-2, 2).bitPattern : r.pick(degenerateFloats).bitPattern }
    if r.profile == 0 {
        let c = r.below(100)
        switch c {
        case 0..<55: return r.range(-2, 2).bitPattern
        case 55..<80: return r.pick(niceFloats).bitPattern
        case 80..<90: return r.unit().bitPattern
        default: return Float(r.below(17)).bitPattern
        }
    }
    if r.profile == 2 {
        let c = r.below(100)
        switch c {
        case 0..<40: return r.pick(edgeFloatBits)
        case 40..<70: return r.range(-2, 2).bitPattern
        case 70..<85: let e = UInt32(127 - 60 + r.below(121)); return (r.u32() & 0x807fffff) | (e << 23)
        default: return r.u32()
        }
    }
    let c = r.below(100)
    switch c {
    case 0..<36: return r.range(-2, 2).bitPattern
    case 36..<50: return r.pick(niceFloats).bitPattern
    case 50..<60:
        let e = UInt32(127 - 30 + r.below(61)); return (r.u32() & 0x807fffff) | (e << 23)
    case 60..<74: return r.pick(edgeFloatBits)
    case 74..<86: return UInt32(r.below(64))
    case 86..<93: return Float(r.below(17)).bitPattern
    default: return r.u32()
    }
}
/// "vary": vertex varyings, more weight on [0,1] style coordinates
@inline(__always) func genVary(_ r: inout RNG) -> UInt32 {
    if r.profile == 3 { return r.below(3) == 0 ? r.range(-0.25, 1.25).bitPattern : r.pick(degenerateFloats).bitPattern }
    if r.profile == 0 { return r.below(4) == 0 ? r.range(-2, 2).bitPattern : r.range(-0.25, 1.25).bitPattern }
    if r.profile == 2 && r.below(3) == 0 { return r.pick(edgeFloatBits) }
    let c = r.below(100)
    switch c {
    case 0..<45: return r.range(-0.25, 1.25).bitPattern
    case 45..<65: return r.range(-4, 4).bitPattern
    case 65..<75: return r.pick(niceFloats).bitPattern
    case 75..<83: return r.pick(edgeFloatBits)
    case 83..<93: let e = UInt32(127 - 20 + r.below(41)); return (r.u32() & 0x807fffff) | (e << 23)
    default: return UInt32(r.below(32))
    }
}
/// "texel": float texture contents, with exact 0, 1 and NaN texels
@inline(__always) func genTexel(_ r: inout RNG) -> UInt32 {
    if r.profile == 3 { return r.below(3) == 0 ? r.unit().bitPattern : r.pick([Float(0), 0, 1, 1, 0.5]).bitPattern }
    if r.profile == 0 {
        let c = r.below(100)
        return c < 70 ? r.unit().bitPattern : c < 80 ? Float(0).bitPattern : c < 90 ? Float(1).bitPattern : r.range(-1, 4).bitPattern
    }
    if r.profile == 2 && r.below(3) == 0 { return r.pick(edgeFloatBits) }
    let c = r.below(100)
    switch c {
    case 0..<40: return r.unit().bitPattern
    case 40..<52: return Float(0).bitPattern
    case 52..<64: return Float(1).bitPattern
    case 64..<70: return 0x7fc00000
    case 70..<84: return r.range(-2, 4).bitPattern
    case 84..<92: return r.pick(edgeFloatBits)
    default: let e = UInt32(127 - 20 + r.below(41)); return (r.u32() & 0x807fffff) | (e << 23)
    }
}
/// "i": default integer fields
@inline(__always) func genI(_ r: inout RNG) -> UInt32 {
    if r.profile == 0 { return UInt32(r.below(16)) }
    if r.profile == 3 { return r.pick([0, 0, 0, 1, 1, 2, 0xffffffff] as [UInt32]) }
    let c = r.below(100)
    switch c {
    case 0..<55: return UInt32(r.below(16))
    case 55..<70: return UInt32(bitPattern: Int32(-1 - r.below(8)))
    case 70..<85: return r.pick([0x7fffffff, 0x80000000, 0xffffffff, 0x10000, 0xff, 0x100, 0xffff, 0x3f800000] as [UInt32])
    default: return r.u32()
    }
}

extension Float { var hexBits: String { String(format: "0x%08x", bitPattern) } }
func hex32(_ v: UInt32) -> String { String(format: "0x%08x", v) }

// MARK: JSON helpers ----------------------------------------------------------------------------

typealias J = [String: Any]
func loadJSON(_ path: String) throws -> Any {
    let d = try Data(contentsOf: URL(fileURLWithPath: path))
    return try JSONSerialization.jsonObject(with: d, options: [])
}
func jsonString(_ o: Any, pretty: Bool = false) -> String {
    let d = try! JSONSerialization.data(withJSONObject: o, options: pretty ? [.prettyPrinted, .sortedKeys] : [.sortedKeys])
    return String(data: d, encoding: .utf8)!
}
extension Dictionary where Key == String, Value == Any {
    func str(_ k: String) -> String? { self[k] as? String }
    func int(_ k: String) -> Int? { (self[k] as? NSNumber)?.intValue }
    func bool(_ k: String) -> Bool? { (self[k] as? NSNumber)?.boolValue }
    func dict(_ k: String) -> J? { self[k] as? J }
    func arr(_ k: String) -> [Any]? { self[k] as? [Any] }
    func dicts(_ k: String) -> [J] { (self[k] as? [Any])?.compactMap { $0 as? J } ?? [] }
}

struct HarnessError: Error, CustomStringConvertible {
    let description: String
    init(_ s: String) { description = s }
}

func now() -> Double { Date().timeIntervalSince1970 }

func eprint(_ s: String) { FileHandle.standardError.write((s + "\n").data(using: .utf8)!) }
