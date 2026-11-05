// gpudiff: input resources. One arena buffer holds every buffer (argument buffers, constant/storage buffers,
// capture buffers, structs with embedded pointers), separated by guard zones filled with deterministic garbage, so that
// even an out-of-bounds read sees the same bytes in the reference and the candidate run.
import Metal
import Foundation

final class Arena {
    let buf: MTLBuffer
    let base: UnsafeMutableRawPointer
    let capacity: Int
    var used = 0
    var regions: [(name: String, off: Int, len: Int)] = []
    init(device: MTLDevice, capacity: Int) throws {
        guard let b = device.makeBuffer(length: capacity, options: [.storageModeShared]) else { throw HarnessError("arena allocation failed") }
        buf = b; base = b.contents(); self.capacity = capacity
        initGarbage()
    }
    var gpuAddress: UInt64 { buf.gpuAddress }
    var dirty = 0
    /// Back to the creation state: whatever earlier test cases left beyond the new extent (their float data) must not
    /// be what this case's out-of-bounds reads see
    func reset() {
        if dirty > 0 { base.copyMemory(from: pattern, byteCount: min(capacity, dirty)) }
        dirty = 0
        used = 0; regions.removeAll(keepingCapacity: true)
    }
    /// Allocates len bytes after a guard zone, returns the offset
    func alloc(_ len: Int, name: String, guardBytes: Int = 4096, align: Int = 256) throws -> Int {
        var off = used + guardBytes
        off = (off + align - 1) / align * align
        if off + len + 262144 > capacity { throw HarnessError("arena too small for \(name) (\(len) bytes)") }
        used = off + len
        regions.append((name, off, len))
        return off
    }
    /// The bytes compared after a run: everything allocated plus a tail guard
    var extent: Int { min(capacity, used + (1 << 20)) }
    /// Per vector: small-integer garbage over everything allocated plus 128 KiB. The rest of the arena keeps the small
    /// integers written at creation, and reads past the arena return zeros on this GPU: an out-of-bounds read that ends up
    /// as a loop bound or an index stays harmless, and it is the same for the reference and the candidate.
    func garbage(seed: UInt64) {
        // the creation pattern again (small integers, the same for every vector): cheap, and all the guarantees hold
        base.copyMemory(from: pattern, byteCount: extent)
        dirty = max(dirty, extent)
    }
    var pattern: UnsafeMutableRawPointer! = nil
    func initGarbage() {
        var r = RNG(0x5eed)
        pattern = UnsafeMutableRawPointer.allocate(byteCount: capacity, alignment: 16)
        let q = pattern.bindMemory(to: UInt32.self, capacity: capacity / 4)
        for i in 0..<(capacity / 4) { q[i] = UInt32(r.below(8)) }
        base.copyMemory(from: pattern, byteCount: capacity)
    }
    func w32(_ off: Int, _ v: UInt32) { base.storeBytes(of: v, toByteOffset: off, as: UInt32.self) }
    func w16(_ off: Int, _ v: UInt16) { base.storeBytes(of: v, toByteOffset: off, as: UInt16.self) }
    func w8(_ off: Int, _ v: UInt8) { base.storeBytes(of: v, toByteOffset: off, as: UInt8.self) }
    func w64(_ off: Int, _ v: UInt64) { base.storeBytes(of: v, toByteOffset: off, as: UInt64.self) }
}

// MARK: fill rules ------------------------------------------------------------------------------

/// A rule from the spec: {"path": "_m0[14].x", "class": "ctl", "lo": 0, "hi": 8, "repr": "i"}
struct Rule {
    let path: String
    let cls: String
    let raw: J
    init(_ j: J) { path = j.str("path") ?? ""; cls = j.str("class") ?? "f"; raw = j }
}

func toBits(_ v: Any, repr: String) -> UInt32 {
    if repr == "f" { return Float((v as? NSNumber)?.doubleValue ?? 0).bitPattern }
    if let n = v as? NSNumber {
        let d = n.doubleValue
        if d < 0 { return UInt32(bitPattern: Int32(d)) }
        return UInt32(truncatingIfNeeded: Int64(d))
    }
    if let s = v as? String, s.hasPrefix("0x"), let x = UInt32(s.dropFirst(2), radix: 16) { return x }
    return 0
}

/// Value for one scalar slot under a rule class
func ruleValue(_ rule: Rule?, _ sc: Scalar, _ r: inout RNG, iota: Int) -> UInt64 {
    let cls = rule?.cls ?? "default"
    let raw = rule?.raw ?? [:]
    let repr = raw.str("repr") ?? (sc.kind == "f" || sc.kind == "h" ? "f" : "i")
    var v32: UInt32
    switch cls {
    case "f": v32 = genF(&r)
    case "vary": v32 = genVary(&r)
    case "texel": v32 = genTexel(&r)
    case "i": v32 = genI(&r)
    case "bits": v32 = r.u32()
    case "zero": v32 = 0
    case "iota": v32 = repr == "f" ? Float(iota).bitPattern : UInt32(truncatingIfNeeded: iota)
    case "const": v32 = toBits(raw["value"] ?? 0, repr: repr)
    case "choice":
        let vals = raw.arr("values") ?? [0]
        v32 = toBits(vals[r.below(vals.count)], repr: repr)
    case "ctl", "mixctl":
        let lo = raw.int("lo") ?? 0, hi = raw.int("hi") ?? 8
        if cls == "mixctl" && r.below(100) >= (raw.int("p") ?? 70) { v32 = genF(&r); break }
        let k = lo + r.below(max(1, hi - lo))
        v32 = repr == "f" ? Float(k).bitPattern : UInt32(bitPattern: Int32(k))
    case "frange":
        let lo = Float((raw["lo"] as? NSNumber)?.doubleValue ?? 0), hi = Float((raw["hi"] as? NSNumber)?.doubleValue ?? 1)
        v32 = r.range(lo, hi).bitPattern
    case "fields":
        v32 = 0
        for f in raw.dicts("fields") {
            let shift = f.int("shift") ?? 0, bits = f.int("bits") ?? 1
            var x: UInt32
            if let vals = f.arr("values") { x = toBits(vals[r.below(vals.count)], repr: "i") }
            else { let lo = f.int("lo") ?? 0, hi = f.int("hi") ?? (1 << bits); x = UInt32(lo + r.below(max(1, hi - lo))) }
            let mask: UInt32 = bits >= 32 ? 0xffffffff : ((1 << UInt32(bits)) - 1)
            v32 |= (x & mask) << UInt32(shift)
        }
    default:
        // the type's own default
        switch sc.kind {
        case "f": v32 = genF(&r)
        case "h": v32 = genF(&r)
        case "b": v32 = UInt32(r.below(2))
        default: v32 = sc.size >= 4 ? genI(&r) : (r.below(4) == 0 ? r.u32() : UInt32(r.below(16)))
        }
    }
    if sc.kind == "h" && repr == "f" && cls != "bits" {
        let f = Float(bitPattern: v32)
        return UInt64(Float16(f).bitPattern)
    }
    if sc.size == 8 { return UInt64(v32) | (cls == "bits" ? UInt64(r.u32()) << 32 : 0) }
    return UInt64(v32)
}

func writeSlot(_ arena: Arena, _ off: Int, _ sc: Scalar, _ v: UInt64) {
    switch sc.size {
    case 1: arena.w8(off, UInt8(truncatingIfNeeded: sc.kind == "b" ? (v & 1) : v))
    case 2: arena.w16(off, UInt16(truncatingIfNeeded: v))
    case 8: arena.w64(off, v)
    default: arena.w32(off, UInt32(truncatingIfNeeded: v))
    }
}

/// Textures and samplers of one test case
final class TexInput {
    let name: String
    let tex: MTLTexture
    let writable: Bool
    var data: [[UInt8]] = []          // per (mip, slice)
    var regions: [(mip: Int, slice: Int, w: Int, h: Int, d: Int)] = []
    let bpp: Int
    init(name: String, tex: MTLTexture, writable: Bool, bpp: Int) { self.name = name; self.tex = tex; self.writable = writable; self.bpp = bpp }
}

func makeTexture(device: MTLDevice, node: TNode, name: String, writable: Bool, renderTarget: Bool = false) throws -> TexInput {
    let d = MTLTextureDescriptor()
    d.textureType = node.texType
    let dt = node.texData
    var bpp = 16
    if node.texDepth { d.pixelFormat = .depth32Float; bpp = 4 }
    else if dt == .int || dt == .short { d.pixelFormat = .rgba32Sint }
    else if dt == .uint || dt == .ushort { d.pixelFormat = .rgba32Uint }
    else if dt == .half && !writable { d.pixelFormat = .rgba16Float; bpp = 8 }
    else { d.pixelFormat = .rgba32Float }
    var w = 16, h = 16, depth = 1, arr = 1, mips = 5
    switch node.texType {
    case .type1D: h = 1
    case .type1DArray: h = 1; arr = 4
    case .type2D: break
    case .type2DArray: arr = 4
    case .typeCube: w = 8; h = 8; mips = 4
    case .typeCubeArray: w = 8; h = 8; mips = 4; arr = 2
    case .type3D: w = 8; h = 8; depth = 8; mips = 4
    case .typeTextureBuffer: h = 1; mips = 1; w = 256
    default: throw HarnessError("texture type \(texTypeName(node.texType)) not supported (\(name))")
    }
    if node.texType == .typeTextureBuffer {
        if d.pixelFormat == .depth32Float { d.pixelFormat = .r32Float; bpp = 4 }
    }
    d.width = w; d.height = h; d.depth = depth; d.arrayLength = arr; d.mipmapLevelCount = mips
    d.storageMode = .shared
    d.usage = [.shaderRead]
    if writable { d.usage.insert(.shaderWrite) }
    if renderTarget { d.usage.insert(.renderTarget) }
    guard let t = device.makeTexture(descriptor: d) else { throw HarnessError("texture creation failed for \(name) (\(d.pixelFormat.rawValue))") }
    let ti = TexInput(name: name, tex: t, writable: writable, bpp: bpp)
    let slices = (node.texType == .typeCube ? 6 : node.texType == .typeCubeArray ? 6 * arr : arr)
    for m in 0..<mips {
        let mw = max(1, w >> m), mh = max(1, h >> m), md = max(1, depth >> m)
        for s in 0..<slices { ti.regions.append((m, s, mw, mh, md)) }
    }
    return ti
}

func fillTexture(_ ti: TexInput, _ r: inout RNG, cls: String) {
    ti.data = []
    let fmt = ti.tex.pixelFormat
    for reg in ti.regions {
        let n = reg.w * reg.h * reg.d * ti.bpp / 4
        var words = [UInt32](repeating: 0, count: n)
        for i in 0..<n {
            switch fmt {
            case .depth32Float:
                let c = r.below(100)
                words[i] = c < 70 ? r.unit().bitPattern : c < 80 ? Float(0).bitPattern : c < 92 ? Float(1).bitPattern : r.pick(edgeFloatBits)
            case .rgba32Uint, .rgba32Sint:
                words[i] = cls == "ctl" ? UInt32(r.below(8)) : genI(&r)
            case .rgba16Float:
                let lo = Float16(Float(bitPattern: genTexel(&r))).bitPattern, hi = Float16(Float(bitPattern: genTexel(&r))).bitPattern
                words[i] = UInt32(lo) | (UInt32(hi) << 16)
            default:
                words[i] = genTexel(&r)
            }
        }
        ti.data.append(words.withUnsafeBytes { Array($0) })
    }
}

func uploadTexture(_ ti: TexInput) {
    for (k, reg) in ti.regions.enumerated() {
        let bpr = reg.w * ti.bpp
        ti.data[k].withUnsafeBytes { p in
            if ti.tex.textureType == .typeTextureBuffer {
                ti.tex.replace(region: MTLRegionMake1D(0, reg.w), mipmapLevel: 0, withBytes: p.baseAddress!, bytesPerRow: bpr)
            } else {
                ti.tex.replace(region: MTLRegionMake3D(0, 0, 0, reg.w, reg.h, reg.d), mipmapLevel: reg.mip, slice: reg.slice,
                               withBytes: p.baseAddress!, bytesPerRow: bpr, bytesPerImage: bpr * reg.h)
            }
        }
    }
}

func readTexture(_ ti: TexInput) -> [UInt8] {
    var out: [UInt8] = []
    for reg in ti.regions {
        let bpr = reg.w * ti.bpp
        var b = [UInt8](repeating: 0, count: bpr * reg.h * reg.d)
        b.withUnsafeMutableBytes { p in
            if ti.tex.textureType == .typeTextureBuffer {
                ti.tex.getBytes(p.baseAddress!, bytesPerRow: bpr, from: MTLRegionMake1D(0, reg.w), mipmapLevel: 0)
            } else {
                ti.tex.getBytes(p.baseAddress!, bytesPerRow: bpr, bytesPerImage: bpr * reg.h, from: MTLRegionMake3D(0, 0, 0, reg.w, reg.h, reg.d), mipmapLevel: reg.mip, slice: reg.slice)
            }
        }
        out += b
    }
    return out
}

// MARK: samplers --------------------------------------------------------------------------------

/// Sampler states drawn from the parameter space the engine's samp_by_id produces (nvn_mtl.m): every min/mag/mip filter,
/// the five address modes, LOD clamps and bias, anisotropy, min/max reduction, border colours, and a compare function
/// for the samplers the spec marks as depth-compare samplers.
final class SamplerPool {
    let device: MTLDevice
    var cache: [String: MTLSamplerState] = [:]
    init(device: MTLDevice) { self.device = device }
    struct Cfg { var minF = 0, magF = 0, mip = 0, s = 0, t = 0, rr = 0, lodMin: Float = 0, lodMax: Float = 1000, bias: Float = 0, aniso = 1, reduction = 0, border = 0, cmp = -1 }
    func random(_ r: inout RNG, compare: Bool) -> Cfg {
        var c = Cfg()
        let modes = 5
        c.minF = r.below(2); c.magF = r.below(2); c.mip = r.below(3)
        let same = r.below(3) != 0
        c.s = r.below(modes); c.t = same ? c.s : r.below(modes); c.rr = same ? c.s : r.below(modes)
        switch r.below(5) { case 0: c.lodMin = 0; c.lodMax = 1000; case 1: c.lodMin = 0; c.lodMax = 0; case 2: c.lodMin = Float(r.below(3)); c.lodMax = c.lodMin + Float(r.below(4)); case 3: c.lodMin = 0.5; c.lodMax = 2.25; default: c.lodMin = 0; c.lodMax = 1000 }
        c.bias = r.below(3) == 0 ? r.pick([-1.5, -0.5, 0.5, 1, 2.75] as [Float]) : 0
        c.aniso = r.below(4) == 0 ? r.pick([2, 4, 8, 16]) : 1
        c.reduction = r.below(6) == 0 ? 1 + r.below(2) : 0
        c.border = r.below(3)
        if compare { c.cmp = r.below(8) }
        // diagnosis: GPUDIFF_SAMPLER_FIX=aniso,reduction,bias,lod,mip,filter pins those parameters
        if let fix = ProcessInfo.processInfo.environment["GPUDIFF_SAMPLER_FIX"] {
            if fix.contains("aniso") { c.aniso = 1 }
            if fix.contains("reduction") { c.reduction = 0 }
            if fix.contains("bias") { c.bias = 0 }
            if fix.contains("lod") { c.lodMin = 0; c.lodMax = 1000 }
            if fix.contains("mip") { c.mip = 0 }
            if fix.contains("filter") { c.minF = 0; c.magF = 0 }
        }
        return c
    }
    func state(_ c: Cfg) throws -> MTLSamplerState {
        let key = "\(c.minF)\(c.magF)\(c.mip)\(c.s)\(c.t)\(c.rr)/\(c.lodMin)/\(c.lodMax)/\(c.bias)/\(c.aniso)/\(c.reduction)/\(c.border)/\(c.cmp)"
        if let s = cache[key] { return s }
        let d = MTLSamplerDescriptor()
        d.minFilter = c.minF == 1 ? .linear : .nearest
        d.magFilter = c.magF == 1 ? .linear : .nearest
        d.mipFilter = c.mip == 0 ? .notMipmapped : c.mip == 1 ? .nearest : .linear
        let am: [MTLSamplerAddressMode] = [.clampToEdge, .repeat, .mirrorClampToEdge, .clampToBorderColor, .mirrorRepeat]
        d.sAddressMode = am[c.s]; d.tAddressMode = am[c.t]; d.rAddressMode = am[c.rr]
        d.lodMinClamp = c.lodMin; d.lodMaxClamp = c.lodMax
        d.lodBias = c.bias
        d.maxAnisotropy = c.aniso
        if c.reduction != 0 { d.reductionMode = c.reduction == 1 ? .minimum : .maximum; d.mipFilter = .linear }
        d.borderColor = [.transparentBlack, .opaqueBlack, .opaqueWhite][c.border]
        if c.cmp >= 0 { d.compareFunction = MTLCompareFunction(rawValue: UInt(c.cmp))! }
        d.supportArgumentBuffers = true
        guard let s = device.makeSamplerState(descriptor: d) else { throw HarnessError("sampler creation failed") }
        cache[key] = s
        return s
    }
}
