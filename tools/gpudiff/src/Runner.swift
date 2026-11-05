// gpudiff: pipelines, helper stages, execution and bitwise comparison
import Metal
import Foundation

let RT_W = 64, RT_H = 64, GRID = 8          // fragment tests: 8x8 cells of 8x8 pixels, two triangles per cell
var gpuStuck = false                         // set when a command buffer did not finish in time: the batch stops

struct Opts {
    var vectors = 12
    var seed: UInt64 = 1
    var maxDiffs = 8
    var timeout = 4.0
    var nondetCheck = true
    var arenaMB = 24
    var verbose = false
    var dump: String? = nil
    var dry = false
    var budget = 0.0          // seconds per job; 0: none. Cases after the budget are reported as "budget"
}

/// What one run of one pipeline produced
struct Outputs {
    var arena: [UInt8] = []
    var textures: [(String, [UInt8])] = []
    var gpuMs = 0.0
}

final class HelperCache {
    let device: MTLDevice
    var libs: [String: MTLLibrary] = [:]
    let lock = NSLock()
    init(device: MTLDevice) { self.device = device }
    func function(_ src: String, _ name: String) throws -> MTLFunction {
        lock.lock(); defer { lock.unlock() }
        if let l = libs[src], let f = l.makeFunction(name: name) { return f }
        let o = MTLCompileOptions()
        o.mathMode = .safe
        let l = try device.makeLibrary(source: src, options: o)
        libs[src] = l
        guard let f = l.makeFunction(name: name) else { throw HarnessError("helper function \(name) missing") }
        return f
    }
}

// MARK: helper stage sources ---------------------------------------------------------------------

/// Words a type occupies when passed through as raw bits, and the MSL expression that rebuilds it from uint words
func typeWords(_ t: String) -> (words: Int, build: (String) -> String, pack: (String) -> String, isInt: Bool)? {
    let base = t.trimmingCharacters(in: .whitespaces)
    let scalars = ["float": 4, "int": 4, "uint": 4, "half": 2, "short": 2, "ushort": 2, "bool": 1]
    var name = base, n = 1
    if let last = base.last, let d = Int(String(last)), d >= 2 && d <= 4 { name = String(base.dropLast()); n = d }
    guard let sz = scalars[name] else { return nil }
    let isInt = name != "float" && name != "half"
    if sz == 4 {
        let ut = n == 1 ? "uint" : "uint\(n)"
        let build: (String) -> String = { p in
            let parts = (0..<n).map { "\(p)[\($0)]" }.joined(separator: ", ")
            return "as_type<\(base)>(\(ut)(\(parts)))"
        }
        let pack: (String) -> String = { v in
            let u = "as_type<\(ut)>(\(v))"
            switch n { case 1: return "uint4(\(u), 0u, 0u, 0u)"; case 2: return "uint4(\(u), 0u, 0u)"; case 3: return "uint4(\(u), 0u)"; default: return u }
        }
        return (n, build, pack, isInt)
    }
    if sz == 2 {
        // half / short: n * 16 bits
        let words = (n + 1) / 2
        let build: (String) -> String = { p in
            if n == 1 { return "as_type<\(base)>(ushort(\(p)[0] & 0xffffu))" }
            if n == 2 { return "as_type<\(base)>(\(p)[0])" }
            if n == 3 { return "\(base)(as_type<\(name)4>(uint2(\(p)[0], \(p)[1])).xyz)" }
            return "as_type<\(base)>(uint2(\(p)[0], \(p)[1]))"
        }
        let pack: (String) -> String = { v in
            if n == 1 { return "uint4(uint(as_type<ushort>(\(v))), 0u, 0u, 0u)" }
            if n == 2 { return "uint4(as_type<uint>(\(v)), 0u, 0u, 0u)" }
            if n == 3 { return "uint4(as_type<uint2>(\(name)4(\(v), 0)), 0u, 0u)" }
            return "uint4(as_type<uint2>(\(v)), 0u, 0u)"
        }
        return (words, build, pack, isInt)
    }
    // bool
    let build: (String) -> String = { p in n == 1 ? "bool(\(p)[0] & 1u)" : "\(base)(" + (0..<n).map { "bool(\(p)[\($0)] & 1u)" }.joined(separator: ", ") + ")" }
    let pack: (String) -> String = { v in n == 1 ? "uint4(uint(\(v)), 0u, 0u, 0u)" : "uint4(uint\(n)(\(v))" + String(repeating: ", 0u", count: 4 - n) + ")" }
    return (n, build, pack, true)
}

/// Vertex stage that feeds a fragment shader under test: position and every varying straight from a buffer (bit exact)
func helperVSSource(stageIn: [J], points: Bool) throws -> (String, Int) {
    var members = ["    float4 pos [[position]];"]
    var body = ["    HVOut o;", "    o.pos = as_type<float4>(uint4(p[0], p[1], p[2], p[3]));"]
    var w = 4
    for (i, v) in stageIn.enumerated() {
        guard let t = v.str("type"), let tw = typeWords(t) else { throw HarnessError("varying type \(v.str("type") ?? "?") not supported") }
        let attr = v.str("attr") ?? "user(locn\(i))"
        members.append("    \(t) v\(i) [[\(attr)]];")
        body.append("    o.v\(i) = \(tw.build("(p + \(w))"));")
        w += 4
    }
    if points { members.append("    float psize [[point_size]];"); body.append("    o.psize = \(Float(RT_W / GRID));") }
    let src = """
    #include <metal_stdlib>
    using namespace metal;
    struct HVOut {
    \(members.joined(separator: "\n"))
    };
    vertex HVOut hvs(uint vid [[vertex_id]], const device uint* d [[buffer(0)]]) {
        const device uint* p = d + vid * \(w)u;
    \(body.joined(separator: "\n"))
        return o;
    }
    """
    return (src, w)
}

/// Fragment stage that captures a vertex shader's position and varyings into rgba32Uint targets (pass k: 7 varyings)
func helperFSSource(vout: [J], pass: Int) throws -> (String, Int) {
    let users = vout.filter { ($0.str("attr") ?? "").hasPrefix("user(") }
    let slice = Array(users.dropFirst(pass * 7).prefix(7))
    var members = ["    float4 pos [[position]];"]
    var outs = ["    uint4 c0 [[color(0)]];"]
    var body = ["    HFOut o;", "    o.c0 = as_type<uint4>(in.pos);"]
    for (i, v) in slice.enumerated() {
        guard let t = v.str("type"), let tw = typeWords(t) else { throw HarnessError("vertex output type \(v.str("type") ?? "?") not supported") }
        members.append("    \(t) v\(i) [[\(v.str("attr")!)\(tw.isInt ? ", flat" : "")]];")
        outs.append("    uint4 c\(i + 1) [[color(\(i + 1))]];")
        body.append("    o.c\(i + 1) = \(tw.pack("in.v\(i)"));")
    }
    let src = """
    #include <metal_stdlib>
    using namespace metal;
    struct HFIn {
    \(members.joined(separator: "\n"))
    };
    struct HFOut {
    \(outs.joined(separator: "\n"))
    };
    fragment HFOut hfs(HFIn in [[stage_in]]) {
    \(body.joined(separator: "\n"))
        return o;
    }
    """
    return (src, 1 + slice.count)
}

func vertexFormat(_ dt: MTLDataType) -> MTLVertexFormat? {
    switch dt {
    case .float: return .float; case .float2: return .float2; case .float3: return .float3; case .float4: return .float4
    case .int: return .int; case .int2: return .int2; case .int3: return .int3; case .int4: return .int4
    case .uint: return .uint; case .uint2: return .uint2; case .uint3: return .uint3; case .uint4: return .uint4
    case .half: return .half; case .half2: return .half2; case .half3: return .half3; case .half4: return .half4
    case .short: return .short; case .short2: return .short2; case .short3: return .short3; case .short4: return .short4
    case .ushort: return .ushort; case .ushort2: return .ushort2; case .ushort3: return .ushort3; case .ushort4: return .ushort4
    case .char: return .char; case .char2: return .char2; case .char3: return .char3; case .char4: return .char4
    case .uchar: return .uchar; case .uchar2: return .uchar2; case .uchar3: return .uchar3; case .uchar4: return .uchar4
    default: return nil
    }
}

func targetFormat(_ t: String) -> MTLPixelFormat {
    let b = t.trimmingCharacters(in: .whitespaces)
    if b.hasPrefix("uint") || b.hasPrefix("ushort") || b.hasPrefix("uchar") { return .rgba32Uint }
    if b.hasPrefix("int") || b.hasPrefix("short") || b.hasPrefix("char") { return .rgba32Sint }
    return .rgba32Float
}

// MARK: test case ----------------------------------------------------------------------------------

/// A built pipeline of one library (reference or candidate)
struct Built {
    var compute: MTLComputePipelineState? = nil
    var render: [MTLRenderPipelineState] = []      // raster probe: one per capture pass
    var plan: [BindingPlan] = []
    var compileMs = 0.0
}

final class Case {
    let h: Harness
    let entry: J
    let mode: String
    let fc: J
    let spec: J
    let arena: Arena
    var regions: [String: (off: Int, node: TNode?)] = [:]
    var ptrSlots: [(at: Int, target: String)] = []               // arena offsets that hold a pointer to a region
    var argSlots: [(at: Int, kind: String, name: String)] = []   // argument buffer texture / sampler slots
    var bufBindings: [(stage: String, index: Int, region: String)] = []
    var texBindings: [(stage: String, index: Int, name: String)] = []
    var smpBindings: [(stage: String, index: Int, name: String)] = []
    var textures: [String: TexInput] = [:]
    var samplerNames: [String] = []
    var samplerStates: [String: MTLSamplerState] = [:]
    var targets: [(index: Int, ti: TexInput)] = []
    var depthTarget: TexInput? = nil
    var vertexData: MTLBuffer? = nil
    var vertexStride = 0
    var vertexCount = 0
    var instanceCount = 1
    var attrBuffer: MTLBuffer? = nil
    var attrIndex = 30
    var attrStride = 0
    var snapshot: [UInt8] = []
    var hints: [String: [Rule]] = [:]
    var tailRegions: Set<String> = []
    var perCell: [String: Int] = [:]      // region -> stride of its per-cell copies
    func hasPointer(_ n: TNode) -> Bool {
        switch n.kind {
        case .pointer, .texture, .sampler: return true
        case .structT: return n.members.contains { hasPointer($0.node) }
        case .array: return n.elem.map(hasPointer) ?? false
        default: return false
        }
    }

    init(h: Harness, spec: J, entry: J, fc: J, arena: Arena) {
        self.h = h; self.spec = spec; self.entry = entry; self.fc = fc; self.arena = arena
        mode = entry.str("mode") ?? "compute"
        for (k, v) in spec.dict("hints") ?? [:] { hints[k] = ((v as? [Any]) ?? []).compactMap { $0 as? J }.map(Rule.init) }
        attrIndex = entry.int("attr_buffer") ?? 30
        tailRegions = Set((spec.arr("tail_regions") as? [String]) ?? [])
    }

    // allocate a region for a node, recursing into pointers found in its layout
    func allocNode(_ name: String, _ node: TNode?, size: Int, depth: Int = 0) throws {
        let len = max(16, (size + 15) / 16 * 16)
        let off = try arena.alloc(len, name: name)
        regions[name] = (off, node)
        guard let node = node, depth < 3 else { return }
        try allocPointers(name, node, base: off, depth: depth)
    }
    func allocPointers(_ name: String, _ node: TNode, base: Int, depth: Int) throws {
        switch node.kind {
        case .structT:
            for m in node.members {
                if m.node.kind == .pointer { try allocPointer("\(name).\(m.name)", m.node, at: base + m.offset, depth: depth) }
                else if m.node.kind == .structT || m.node.kind == .array { try allocPointers("\(name).\(m.name)", m.node, base: base + m.offset, depth: depth) }
            }
        case .array:
            if let e = node.elem, e.kind == .pointer || e.kind == .structT {
                for i in 0..<min(node.count, 64) {
                    if e.kind == .pointer { try allocPointer("\(name)[\(i)]", e, at: base + i * node.stride, depth: depth) }
                    else { try allocPointers("\(name)[\(i)]", e, base: base + i * node.stride, depth: depth) }
                }
            }
        default: break
        }
    }
    func pointerSize(_ name: String, _ p: TNode) -> Int {
        if let s = (spec.dict("pointer_sizes") ?? [:])[name] as? NSNumber { return s.intValue }
        let writable = p.access != .readOnly
        if writable { return max(p.pointeeSize, 65536) }
        if p.pointeeSize <= 16 { return 16384 }          // a byte pointer (vertex stream base): vertices x stride + offsets
        return max(p.pointeeSize, 256)
    }
    var pending: [(name: String, node: TNode?, size: Int, depth: Int)] = []
    func allocPointer(_ name: String, _ p: TNode, at: Int, depth: Int) throws {
        let sz = pointerSize(name, p)
        ptrSlots.append((at, name))
        // regions whose dynamically indexed contents control a loop go to the end of the arena: their out-of-bounds reads
        // then land in small-integer garbage (or past the arena: zeros), never in another region's float data
        if tailRegions.contains(name) { pending.append((name, p.pointee, sz, depth + 1)); return }
        try allocNode(name, p.pointee, size: sz, depth: depth + 1)
    }

    /// Resources from the reference pipeline's reflection
    func build(plan: [BindingPlan]) throws {
        arena.reset()
        for b in plan {
            switch b.type {
            case .buffer:
                if b.stage == "vertex" && b.index == attrIndex && (entry.dicts("vertex_attrs").count > 0) { continue }
                if b.stage == "vertex" && mode == "fragment" { continue }      // the helper vertex stage's own data
                guard let node = b.node else { continue }
                let rname = "\(b.isArgbuf ? "argbuf" : "buf"):\(b.stage):\(b.name)"
                if b.isArgbuf {
                    let len = max(16, (node.size + 15) / 16 * 16)
                    let off = try arena.alloc(len, name: rname)
                    regions[rname] = (off, node)
                    for m in node.members where m.argIndex >= 0 {
                        switch m.node.kind {
                        case .pointer: try allocPointer("arg:\(m.name)", m.node, at: off + m.offset, depth: 0)
                        case .texture:
                            if textures["arg:\(m.name)"] == nil {
                                let w = m.node.texAccess != .readOnly
                                textures["arg:\(m.name)"] = try makeTexture(device: h.device, node: m.node, name: m.name, writable: w)
                            }
                            argSlots.append((off + m.offset, "texture", "arg:\(m.name)"))
                        case .sampler:
                            if !samplerNames.contains("arg:\(m.name)") { samplerNames.append("arg:\(m.name)") }
                            argSlots.append((off + m.offset, "sampler", "arg:\(m.name)"))
                        default: break
                        }
                    }
                } else if mode == "fragment" && b.stage == "fragment" && node.size <= 1024 && !hasPointer(node) {
                    // small engine state (dm_alpha_state, dm_logic_state): one copy per cell, bound per cell draw, so one
                    // vector covers every alpha function and logic operation instead of one
                    let stride = (max(16, node.size) + 255) / 256 * 256
                    let off = try arena.alloc(stride * GRID * GRID, name: rname)
                    regions[rname] = (off, node)
                    perCell[rname] = stride
                } else {
                    let sz = (spec.dict("buffer_sizes") ?? [:])[b.name] as? NSNumber
                    try allocNode(rname, node, size: max(sz?.intValue ?? 0, b.size, 16))
                }
                bufBindings.append((b.stage, b.index, rname))
            case .texture:
                guard let node = b.node else { continue }
                textures["tex:\(b.name)"] = try makeTexture(device: h.device, node: node, name: b.name, writable: node.texAccess != .readOnly)
                texBindings.append((b.stage, b.index, "tex:\(b.name)"))
            case .sampler:
                samplerNames.append("smp:\(b.name)")
                smpBindings.append((b.stage, b.index, "smp:\(b.name)"))
            default: break
            }
        }
        for p in pending { try allocNode(p.name, p.node, size: p.size, depth: p.depth) }
        pending.removeAll()
        // stage inputs and render targets
        if mode == "fragment" {
            let stageIn = entry.dicts("stage_in")
            let points = entry.bool("points") ?? false
            vertexStride = 4 + 4 * stageIn.count
            vertexCount = points ? GRID * GRID : GRID * GRID * 3
            vertexData = h.device.makeBuffer(length: vertexCount * vertexStride * 4, options: .storageModeShared)
            for t in entry.dicts("targets") {
                let idx = t.int("index") ?? 0
                let n = TNode(); n.kind = .texture; n.texType = .type2D
                let fmt = targetFormat(t.str("type") ?? "float4")
                n.texData = fmt == .rgba32Uint ? .uint : fmt == .rgba32Sint ? .int : .float
                targets.append((idx, try makeRT(fmt, name: "rt\(idx)")))
            }
            if entry.dict("depth") != nil { depthTarget = try makeRT(.depth32Float, name: "depth") }
        }
        if mode == "raster" {
            vertexCount = entry.dict("draw")?.int("verts") ?? 48
            let npass = max(1, (entry.dicts("vout").filter { ($0.str("attr") ?? "").hasPrefix("user(") }.count + 6) / 7)
            for p in 0..<npass {
                for i in 0..<8 { targets.append((p * 8 + i, try makeRT(.rgba32Uint, name: "pass\(p).c\(i)"))) }
            }
            depthTarget = try makeRT(.depth32Float, name: "depth")
        }
        if mode == "capture" || mode == "noraster" {
            vertexCount = entry.dict("draw")?.int("verts") ?? 48
            instanceCount = entry.dict("draw")?.int("instances") ?? 2
        }
        let attrs = entry.dicts("vertex_attrs")
        if !attrs.isEmpty {
            attrStride = 16 * ((attrs.compactMap { $0.int("index") }.max() ?? 0) + 1)
            attrBuffer = h.device.makeBuffer(length: max(16, attrStride * vertexCount * instanceCount), options: .storageModeShared)
        }
    }

    func makeRT(_ fmt: MTLPixelFormat, name: String) throws -> TexInput {
        let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: fmt, width: RT_W, height: RT_H, mipmapped: false)
        d.usage = [.renderTarget, .shaderRead]
        d.storageMode = .shared
        guard let t = h.device.makeTexture(descriptor: d) else { throw HarnessError("render target \(name) failed") }
        let ti = TexInput(name: name, tex: t, writable: true, bpp: fmt == .depth32Float ? 4 : 16)
        ti.regions = [(0, 0, RT_W, RT_H, 1)]
        return ti
    }

    /// Fills every input for one vector and snapshots the arena
    func fill(vector: Int, seed: UInt64) throws {
        let prof = vector % 4      // 0 sane, 1 mixed, 2 edge-heavy, 3 degenerate (exact 0, 1, 0.5)
        arena.garbage(seed: seedFor(seed, "garbage", "\(vector)"))
        // regions: default by layout, then the spec's rules
        for (name, reg0) in regions.sorted(by: { $0.key < $1.key }) {
          let copies = perCell[name] != nil ? GRID * GRID : 1
          for copy in 0..<copies {
            let reg = (off: reg0.off + copy * (perCell[name] ?? 0), node: reg0.node)
            var r = RNG(seedFor(seed, name, "\(vector)", copy == 0 ? "" : "cell\(copy)"), profile: prof)
            guard let node = reg.node else { continue }
            var slots: [(off: Int, sc: Scalar)] = []
            node.slots(base: 0, into: &slots)
            let defaultRule: Rule? = nil
            for s in slots { writeSlot(arena, reg.off + s.off, s.sc, ruleValue(defaultRule, s.sc, &r, iota: 0)) }
            // a scalar or runtime-sized pointee larger than its type: fill the rest as float data
            let covered = node.size
            let len = perCell[name] ?? (arena.regions.first(where: { $0.name == name })?.len ?? covered)
            if covered < len {
                let p = arena.base.advanced(by: reg.off).bindMemory(to: UInt32.self, capacity: len / 4)
                for i in (covered / 4)..<(len / 4) { p[i] = genF(&r) }
            }
            let key = name.hasPrefix("arg:") ? name : name.hasPrefix("buf:") ? "buf:" + (name.split(separator: ":").last.map(String.init) ?? name) : name
            for rule in hints[key] ?? [] {
                if rule.cls == "region" {   // raw word fill of the whole allocation
                    let p = arena.base.advanced(by: reg.off).bindMemory(to: UInt32.self, capacity: len / 4)
                    let sc = Scalar(comps: 1, size: 4, kind: "u")
                    let inner = Rule(rule.raw.dict("of") ?? ["class": "f"])
                    for i in 0..<(len / 4) { p[i] = UInt32(truncatingIfNeeded: ruleValue(inner, sc, &r, iota: i)) }
                    continue
                }
                guard let ss = node.resolve(rule.path) else { if h.opts.verbose { eprint("rule path \(rule.path) not found in \(name)") }; continue }
                for (i, s) in ss.enumerated() { writeSlot(arena, reg.off + s.off, s.sc, ruleValue(rule, s.sc, &r, iota: i)) }
            }
          }
        }
        for p in ptrSlots { arena.w64(p.at, arena.gpuAddress + UInt64(regions[p.target]!.off)) }
        // textures
        for (name, ti) in textures.sorted(by: { $0.key < $1.key }) {
            var r = RNG(seedFor(seed, name, "\(vector)", "tex"), profile: prof)
            let cls = ((spec.dict("textures") ?? [:])[name] as? J)?.str("class") ?? "texel"
            fillTexture(ti, &r, cls: cls)
            uploadTexture(ti)
        }
        // samplers: a twin ("_w") shares its partner's state except the border colour, like the engine's twin samplers
        var cfgs: [String: SamplerPool.Cfg] = [:]
        let sinfo = spec.dict("samplers") ?? [:]
        for name in samplerNames.sorted() {
            var r = RNG(seedFor(seed, name, "\(vector)", "smp"), profile: prof)
            let info = sinfo[name] as? J ?? [:]
            var c = h.samplers(arena).random(&r, compare: info.bool("compare") ?? false)
            if let twin = info.str("twin_of"), let base = cfgs[twin] { c = base; c.border = (base.border + 1 + r.below(2)) % 3 }
            cfgs[name] = c
            samplerStates[name] = try h.samplers(arena).state(c)
        }
        for s in argSlots {
            if s.kind == "texture", let t = textures[s.name] { arena.w64(s.at, t.tex.gpuResourceID._impl) }
            if s.kind == "sampler", let st = samplerStates[s.name] { arena.w64(s.at, st.gpuResourceID._impl) }
        }
        // render targets and depth: random initial contents (kept where a fragment is discarded or not covered)
        for (i, t) in targets.enumerated() {
            // the raster probe's capture targets only need some initial contents: once per case
            if mode == "raster" && !t.ti.data.isEmpty { continue }
            var r = RNG(seedFor(seed, "rt\(i)", "\(vector)"), profile: prof)
            fillTexture(t.ti, &r, cls: "texel")
        }
        if let d = depthTarget {
            var r = RNG(seedFor(seed, "depth", "\(vector)"), profile: prof)
            var w = [UInt32](repeating: 0, count: RT_W * RT_H)
            for i in 0..<w.count { w[i] = r.below(8) == 0 ? Float(1).bitPattern : r.unit().bitPattern }
            d.data = [w.withUnsafeBytes { Array($0) }]
        }
        // vertex data for the helper vertex stage (fragment tests)
        if mode == "fragment", let vb = vertexData {
            var r = RNG(seedFor(seed, "varyings", "\(vector)"), profile: prof)
            let stageIn = entry.dicts("stage_in")
            let points = entry.bool("points") ?? false
            let p = vb.contents().bindMemory(to: UInt32.self, capacity: vertexCount * vertexStride)
            var v = 0
            for cy in 0..<GRID {
                for cx in 0..<GRID {
                    // one triangle per cell, twice the cell's size and cut to the cell by a scissor: every 2x2 pixel quad
                    // belongs to one primitive, so helper lanes never read a framebuffer pixel another primitive writes
                    // (with framebuffer fetch that made derivatives depend on timing). Three corners, each with its own w
                    // (perspective-correct interpolation) and its own varyings.
                    var corners: [[UInt32]] = []
                    let flip = r.below(2) == 0
                    let cs = 2 / Float(GRID)
                    for k in 0..<3 {
                        let px = Float(cx) * cs - 1 + (k == 1 ? 2 * cs : 0), py = Float(cy) * cs - 1 + (k == 2 ? 2 * cs : 0)
                        let w: Float = r.below(4) == 0 ? 1 : r.range(0.5, 2)
                        let z = r.unit()
                        var rec: [UInt32] = [(px * w).bitPattern, (py * w).bitPattern, (z * w).bitPattern, w.bitPattern]
                        if points { rec = [(Float(cx) * cs - 1 + cs / 2).bitPattern, (Float(cy) * cs - 1 + cs / 2).bitPattern, z.bitPattern, Float(1).bitPattern] }
                        for si in stageIn {
                            let cls = si.str("class") ?? "vary"
                            let rule = Rule(["class": cls, "lo": si.int("lo") ?? 0, "hi": si.int("hi") ?? 8, "repr": si.str("repr") ?? "f"])
                            let isInt = typeWords(si.str("type") ?? "float4")?.isInt ?? false
                            for _ in 0..<4 {
                                if isInt && cls == "vary" { rec.append(r.below(4) == 0 ? r.u32() : UInt32(r.below(16))) }
                                else { rec.append(UInt32(truncatingIfNeeded: ruleValue(rule, Scalar(comps: 1, size: 4, kind: "f"), &r, iota: 0))) }
                            }
                        }
                        corners.append(rec)
                    }
                    let order = points ? [0] : (flip ? [0, 2, 1] : [0, 1, 2])
                    for k in order {
                        for (j, x) in corners[k].enumerated() { p[v * vertexStride + j] = x }
                        v += 1
                    }
                }
            }
        }
        // vertex attributes for a vertex shader with a stage_in
        if let ab = attrBuffer {
            var r = RNG(seedFor(seed, "attrs", "\(vector)"), profile: prof)
            let p = ab.contents().bindMemory(to: UInt32.self, capacity: ab.length / 4)
            let attrs = entry.dicts("vertex_attrs")
            for vtx in 0..<(vertexCount * instanceCount) {
                for a in attrs {
                    let idx = a.int("index") ?? 0
                    let ty = a.str("type") ?? "float4"
                    let isInt = !(ty.hasPrefix("float") || ty.hasPrefix("half"))
                    let cls = a.str("class") ?? (isInt ? "i" : "vary")
                    let rule = Rule(["class": cls, "lo": a.int("lo") ?? 0, "hi": a.int("hi") ?? 8, "repr": isInt ? "i" : "f"])
                    for c in 0..<4 {
                        var x = UInt32(truncatingIfNeeded: ruleValue(rule, Scalar(comps: 1, size: 4, kind: isInt ? "i" : "f"), &r, iota: 0))
                        if ty.hasPrefix("half") { x = UInt32(Float16(Float(bitPattern: x)).bitPattern) | (UInt32(Float16(Float(bitPattern: genVary(&r))).bitPattern) << 16) }
                        if isInt && (ty.hasPrefix("short") || ty.hasPrefix("ushort")) { x = (x & 0xffff) | (UInt32(r.below(16)) << 16) }
                        p[(vtx * attrStride + idx * 16) / 4 + c] = x
                    }
                }
            }
        }
        snapshot = Array(UnsafeRawBufferPointer(start: arena.base, count: arena.extent))
    }

    /// One run of one library's pipeline on the current inputs
    func run(_ b: Built) throws -> Outputs {
        if h.opts.dry { return initialOutputs() }
        snapshot.withUnsafeBytes { arena.base.copyMemory(from: $0.baseAddress!, byteCount: snapshot.count) }
        for (_, t) in textures where t.writable { uploadTexture(t) }
        for t in targets { uploadTexture(t.ti) }
        if let d = depthTarget { uploadTexture(d) }
        guard let cb = h.queue.makeCommandBuffer() else { throw HarnessError("no command buffer") }
        let usage: MTLResourceUsage = [.read, .write]
        switch mode {
        case "compute":
            guard let ps = b.compute, let enc = cb.makeComputeCommandEncoder() else { throw HarnessError("compute encoder") }
            enc.setComputePipelineState(ps)
            enc.useResource(arena.buf, usage: usage)
            for (_, t) in textures { enc.useResource(t.tex, usage: t.writable ? usage : .read) }
            for bb in bufBindings { enc.setBuffer(arena.buf, offset: regions[bb.region]!.off, index: bb.index) }
            for tb in texBindings { enc.setTexture(textures[tb.name]!.tex, index: tb.index) }
            for sb in smpBindings { enc.setSamplerState(samplerStates[sb.name], index: sb.index) }
            let tg = (entry.arr("tg") as? [NSNumber])?.map { $0.intValue } ?? [1, 1, 1]
            let groups = (entry.arr("groups") as? [NSNumber])?.map { $0.intValue } ?? [2, 1, 1]
            if tg.reduce(1, *) > ps.maxTotalThreadsPerThreadgroup { throw HarnessError("threadgroup \(tg) exceeds \(ps.maxTotalThreadsPerThreadgroup)") }
            enc.dispatchThreadgroups(MTLSize(width: groups[0], height: groups[1], depth: groups[2]), threadsPerThreadgroup: MTLSize(width: tg[0], height: tg[1], depth: tg[2]))
            enc.endEncoding()
        default:
            let npass = max(1, b.render.count)
            for pass in 0..<npass {
                let rp = MTLRenderPassDescriptor()
                let passTargets = mode == "raster" ? targets.filter { $0.index / 8 == pass } : targets
                for t in passTargets {
                    let ca = rp.colorAttachments[t.index % 8]!
                    ca.texture = t.ti.tex; ca.loadAction = .load; ca.storeAction = .store
                }
                if let d = depthTarget {
                    rp.depthAttachment.texture = d.tex; rp.depthAttachment.loadAction = .load; rp.depthAttachment.storeAction = .store
                }
                if mode == "capture" || mode == "noraster" {
                    rp.renderTargetWidth = RT_W; rp.renderTargetHeight = RT_H; rp.defaultRasterSampleCount = 1
                }
                guard let enc = cb.makeRenderCommandEncoder(descriptor: rp) else { throw HarnessError("render encoder") }
                enc.setRenderPipelineState(b.render[pass])
                if let ds = h.depthState, depthTarget != nil { enc.setDepthStencilState(ds) }
                enc.setCullMode(.none)
                enc.useResource(arena.buf, usage: usage, stages: [.vertex, .fragment])
                for (_, t) in textures { enc.useResource(t.tex, usage: t.writable ? usage : .read, stages: [.vertex, .fragment]) }
                for bb in bufBindings {
                    if bb.stage == "vertex" { enc.setVertexBuffer(arena.buf, offset: regions[bb.region]!.off, index: bb.index) }
                    else { enc.setFragmentBuffer(arena.buf, offset: regions[bb.region]!.off, index: bb.index) }
                }
                for tb in texBindings {
                    if tb.stage == "vertex" { enc.setVertexTexture(textures[tb.name]!.tex, index: tb.index) } else { enc.setFragmentTexture(textures[tb.name]!.tex, index: tb.index) }
                }
                for sb in smpBindings {
                    if sb.stage == "vertex" { enc.setVertexSamplerState(samplerStates[sb.name], index: sb.index) } else { enc.setFragmentSamplerState(samplerStates[sb.name], index: sb.index) }
                }
                if let vd = vertexData { enc.setVertexBuffer(vd, offset: 0, index: 0) }
                if let ab = attrBuffer { enc.setVertexBuffer(ab, offset: 0, index: attrIndex) }
                switch mode {
                case "fragment":
                    let pts = entry.bool("points") ?? false
                    let per = pts ? 1 : 3, cell = RT_W / GRID
                    for i in 0..<(GRID * GRID) {
                        let cx = i % GRID, cy = i / GRID
                        for bb in bufBindings where bb.stage == "fragment" {
                            if let st = perCell[bb.region] { enc.setFragmentBufferOffset(regions[bb.region]!.off + i * st, index: bb.index) }
                        }
                        enc.setScissorRect(MTLScissorRect(x: cx * cell, y: (GRID - 1 - cy) * cell, width: cell, height: cell))
                        enc.drawPrimitives(type: pts ? .point : .triangle, vertexStart: i * per, vertexCount: per)
                    }
                case "raster":
                    enc.drawPrimitives(type: (entry.bool("points") ?? false) ? .point : .triangle, vertexStart: 0, vertexCount: vertexCount)
                default:
                    enc.drawPrimitives(type: .point, vertexStart: 0, vertexCount: vertexCount, instanceCount: instanceCount)
                }
                enc.endEncoding()
            }
        }
        let sem = DispatchSemaphore(value: 0)
        cb.addCompletedHandler { _ in sem.signal() }
        cb.commit()
        if sem.wait(timeout: .now() + h.opts.timeout) == .timedOut {
            gpuStuck = true
            throw HarnessError("gpu timeout: command buffer did not finish in \(h.opts.timeout) s")
        }
        if let e = cb.error {
            let msg = e.localizedDescription
            if msg.contains("Hang") || msg.contains("SubmissionsIgnored") || msg.contains("Timeout") { gpuStuck = true }
            throw HarnessError("gpu error: \(msg)")
        }
        arena.dirty = arena.capacity      // a run may write out of bounds anywhere in the arena: clean it all on reset
        var o = Outputs()
        o.gpuMs = (cb.gpuEndTime - cb.gpuStartTime) * 1000
        o.arena = Array(UnsafeRawBufferPointer(start: arena.base, count: arena.extent))
        for (name, t) in textures.sorted(by: { $0.key < $1.key }) where t.writable { o.textures.append((name, readTexture(t))) }
        for t in targets { o.textures.append((t.ti.name, readTexture(t.ti))) }
        if let d = depthTarget { o.textures.append(("depth", readTexture(d))) }
        return o
    }

    /// The outputs as they were before the run: compared with a run's outputs, tells how much the shader wrote
    func initialOutputs() -> Outputs {
        var o = Outputs()
        o.arena = snapshot
        for (name, t) in textures.sorted(by: { $0.key < $1.key }) where t.writable { o.textures.append((name, t.data.flatMap { $0 })) }
        for t in targets { o.textures.append((t.ti.name, t.ti.data.flatMap { $0 })) }
        if let d = depthTarget { o.textures.append(("depth", d.data.flatMap { $0 })) }
        return o
    }

    /// x, y, w of every captured vertex: capture {"region", "stride" (words per vertex), "pos" (word of position.x),
    /// "verts", "instances", "rows" (records per instance)}
    func positions(_ o: Outputs, _ cap: J) -> [UInt32] {
        guard let reg = regions[cap.str("region") ?? ""] else { return [] }
        let stride = cap.int("stride") ?? 144, pos = cap.int("pos") ?? 0
        let verts = cap.int("verts") ?? vertexCount, inst = cap.int("instances") ?? instanceCount, rows = cap.int("rows") ?? verts
        var out: [UInt32] = []
        o.arena.withUnsafeBytes { p in
            for i in 0..<inst {
                for vtx in 0..<verts {
                    let w = reg.off / 4 + (i * rows + vtx) * stride + pos
                    if (w + 4) * 4 > o.arena.count { continue }
                    for c in [0, 1, 3] { out.append(p.load(fromByteOffset: (w + c) * 4, as: UInt32.self)) }
                }
            }
        }
        return out
    }

    func regionName(_ off: Int) -> String {
        for r in arena.regions where off >= r.off && off < r.off + r.len { return "\(r.name)+\(off - r.off)" }
        return "arena+\(off) (guard)"
    }

    /// Word-by-word comparison, returns (number of differing 32-bit words, first diffs)
    /// Word indices that differ in any of the pairs, per output (0: arena, k: k-th texture)
    func diffMask(_ pairs: [(Outputs, Outputs)]) -> [Int: Set<Int>] {
        var m: [Int: Set<Int>] = [:]
        func add(_ k: Int, _ x: [UInt8], _ y: [UInt8]) {
            if x == y { return }
            x.withUnsafeBytes { px in y.withUnsafeBytes { py in
                let wx = px.bindMemory(to: UInt32.self), wy = py.bindMemory(to: UInt32.self)
                for i in 0..<(min(x.count, y.count) / 4) where wx[i] != wy[i] { m[k, default: []].insert(i) }
            } }
        }
        for (a, b) in pairs {
            add(0, a.arena, b.arena)
            for (i, t) in a.textures.enumerated() where i < b.textures.count { add(i + 1, t.1, b.textures[i].1) }
        }
        return m
    }

    func compare(_ a: Outputs, _ b: Outputs, limit: Int, mask: [Int: Set<Int>] = [:]) -> (Int, [J]) {
        var n = 0
        var first: [J] = []
        var key = 0
        func cmp(_ x: [UInt8], _ y: [UInt8], _ label: (Int) -> String) {
            defer { key += 1 }
            if x == y { return }
            let cnt = min(x.count, y.count) / 4
            let mk = mask[key] ?? []
            x.withUnsafeBytes { px in y.withUnsafeBytes { py in
                let wx = px.bindMemory(to: UInt32.self), wy = py.bindMemory(to: UInt32.self)
                for i in 0..<cnt where wx[i] != wy[i] && !mk.contains(i) {
                    n += 1
                    if first.count < limit {
                        var d: J = ["where": label(i * 4), "a": hex32(wx[i]), "b": hex32(wy[i]),
                                    "a_f": Double(Float(bitPattern: wx[i])).description, "b_f": Double(Float(bitPattern: wy[i])).description]
                        // distance in units in the last place when both are finite floats (1: a rounding difference)
                        let fa = Float(bitPattern: wx[i]), fb = Float(bitPattern: wy[i])
                        if fa.isFinite && fb.isFinite {
                            func ord(_ u: UInt32) -> Int64 { u & 0x80000000 != 0 ? -Int64(u & 0x7fffffff) : Int64(u) }
                            d["ulp"] = abs(ord(wx[i]) - ord(wy[i]))
                        }
                        first.append(d)
                    }
                }
            } }
            if x.count != y.count { n += 1; first.append(["where": "size", "a": x.count, "b": y.count]) }
        }
        cmp(a.arena, b.arena) { self.regionName($0) }
        for (i, t) in a.textures.enumerated() where i < b.textures.count {
            let name = t.0
            cmp(t.1, b.textures[i].1) { off in
                if name.hasPrefix("rt") || name.hasPrefix("pass") || name == "depth" {
                    let bpp = name == "depth" ? 4 : 16
                    let px = off / bpp
                    return "\(name) x=\(px % RT_W) y=\(px / RT_W) c=\((off % bpp) / 4)"
                }
                return "\(name)+\(off)"
            }
        }
        return (n, first)
    }
}

// MARK: harness ------------------------------------------------------------------------------------

final class Harness {
    let device: MTLDevice
    let queue: MTLCommandQueue
    let helpers: HelperCache
    var opts: Opts
    var depthState: MTLDepthStencilState?
    var pools: [ObjectIdentifier: SamplerPool] = [:]
    let poolLock = NSLock()

    init(opts: Opts) throws {
        guard let d = MTLCreateSystemDefaultDevice() else { throw HarnessError("no Metal device") }
        device = d
        guard let q = d.makeCommandQueue() else { throw HarnessError("no command queue") }
        queue = q
        helpers = HelperCache(device: d)
        self.opts = opts
        let dd = MTLDepthStencilDescriptor()
        dd.depthCompareFunction = .lessEqual
        dd.isDepthWriteEnabled = true
        depthState = d.makeDepthStencilState(descriptor: dd)
    }
    func samplers(_ key: Arena) -> SamplerPool {
        poolLock.lock(); defer { poolLock.unlock() }
        let k = ObjectIdentifier(key)
        if let p = pools[k] { return p }
        let p = SamplerPool(device: device); pools[k] = p; return p
    }

    func constants(_ f: MTLFunction, _ fc: J) throws -> MTLFunctionConstantValues? {
        let decl = f.functionConstantsDictionary
        if decl.isEmpty { return nil }
        let v = MTLFunctionConstantValues()
        for (name, c) in decl {
            let given = fc[name]
            switch c.type {
            case .bool:
                var b: Bool = (given as? NSNumber)?.boolValue ?? false
                v.setConstantValue(&b, type: .bool, withName: name)
            case .int:
                var i = Int32((given as? NSNumber)?.intValue ?? 0)
                v.setConstantValue(&i, type: .int, withName: name)
            case .uint:
                var u = UInt32((given as? NSNumber)?.intValue ?? 0)
                v.setConstantValue(&u, type: .uint, withName: name)
            case .float:
                var x = Float((given as? NSNumber)?.doubleValue ?? 0)
                v.setConstantValue(&x, type: .float, withName: name)
            case .short:
                var s = Int16((given as? NSNumber)?.intValue ?? 0)
                v.setConstantValue(&s, type: .short, withName: name)
            case .ushort:
                var s = UInt16((given as? NSNumber)?.intValue ?? 0)
                v.setConstantValue(&s, type: .ushort, withName: name)
            default:
                if c.required { throw HarnessError("function constant \(name) of type \(c.type.rawValue) not supported") }
            }
        }
        return v
    }

    func function(_ lib: MTLLibrary, _ name: String, _ fc: J) throws -> MTLFunction {
        guard let plain = lib.makeFunction(name: name) else { throw HarnessError("function \(name) missing") }
        if let cv = try constants(plain, fc) { return try lib.makeFunction(name: name, constantValues: cv) }
        return plain
    }

    func vertexDescriptor(_ entry: J) -> MTLVertexDescriptor? {
        let attrs = entry.dicts("vertex_attrs")
        if attrs.isEmpty { return nil }
        let vd = MTLVertexDescriptor()
        let bi = entry.int("attr_buffer") ?? 30
        let stride = 16 * ((attrs.compactMap { $0.int("index") }.max() ?? 0) + 1)
        for a in attrs {
            let i = a.int("index") ?? 0
            let dt = MTLDataType(rawValue: UInt(a.int("dt") ?? Int(MTLDataType.float4.rawValue))) ?? .float4
            vd.attributes[i].format = vertexFormat(dt) ?? .float4
            vd.attributes[i].offset = i * 16
            vd.attributes[i].bufferIndex = bi
        }
        vd.layouts[bi].stride = stride
        vd.layouts[bi].stepFunction = .perVertex
        return vd
    }

    /// Builds the pipeline(s) of one entry for one library
    func buildPipelines(_ lib: MTLLibrary, entry inEntry: J, fc: J) throws -> Built {
        var b = Built()
        let t0 = now()
        var entry = inEntry
        let name = entry.str("name")!
        let mode = entry.str("mode") ?? "compute"
        let f = try function(lib, name, fc)
        // a vertex function with a stage_in and no attributes in the spec yet (reflection pass): take them from Metal
        if entry["vertex_attrs"] == nil, let va = f.vertexAttributes, !va.isEmpty {
            entry["vertex_attrs"] = va.map { ["index": $0.attributeIndex, "dt": Int($0.attributeType.rawValue), "type": dataTypeName($0.attributeType)] as J }
        }
        let opt: MTLPipelineOption = [.bindingInfo, .bufferTypeInfo]
        switch mode {
        case "compute":
            let d = MTLComputePipelineDescriptor()
            d.computeFunction = f
            let (ps, refl) = try device.makeComputePipelineState(descriptor: d, options: opt)
            b.compute = ps
            b.plan = planBindings(refl?.bindings ?? [], stage: "kernel")
        case "fragment":
            let points = entry.bool("points") ?? false
            let (src, _) = try helperVSSource(stageIn: entry.dicts("stage_in"), points: points)
            let vs = try helpers.function(src, "hvs")
            let d = MTLRenderPipelineDescriptor()
            d.vertexFunction = vs; d.fragmentFunction = f
            for t in entry.dicts("targets") { d.colorAttachments[t.int("index") ?? 0].pixelFormat = targetFormat(t.str("type") ?? "float4") }
            if entry.dict("depth") != nil { d.depthAttachmentPixelFormat = .depth32Float }
            d.inputPrimitiveTopology = points ? .point : .triangle
            let (ps, refl) = try device.makeRenderPipelineState(descriptor: d, options: opt)
            b.render = [ps]
            b.plan = planBindings(refl?.fragmentBindings ?? [], stage: "fragment")
        case "raster":
            let vout = entry.dicts("vout")
            let npass = max(1, (vout.filter { ($0.str("attr") ?? "").hasPrefix("user(") }.count + 6) / 7)
            for p in 0..<npass {
                let (src, _) = try helperFSSource(vout: vout, pass: p)
                let fs = try helpers.function(src, "hfs")
                let d = MTLRenderPipelineDescriptor()
                d.vertexFunction = f; d.fragmentFunction = fs
                d.vertexDescriptor = vertexDescriptor(entry)
                for i in 0..<8 { d.colorAttachments[i].pixelFormat = .rgba32Uint }
                d.depthAttachmentPixelFormat = .depth32Float
                d.inputPrimitiveTopology = (entry.bool("points") ?? false) ? .point : .unspecified
                let (ps, refl) = try device.makeRenderPipelineState(descriptor: d, options: opt)
                b.render.append(ps)
                if p == 0 { b.plan = planBindings(refl?.vertexBindings ?? [], stage: "vertex") }
            }
        case "capture", "noraster":
            let d = MTLRenderPipelineDescriptor()
            d.vertexFunction = f
            d.isRasterizationEnabled = false
            d.vertexDescriptor = vertexDescriptor(entry)
            d.inputPrimitiveTopology = .point
            let (ps, refl) = try device.makeRenderPipelineState(descriptor: d, options: opt)
            b.render = [ps]
            b.plan = planBindings(refl?.vertexBindings ?? [], stage: "vertex")
        default: throw HarnessError("unknown mode \(mode)")
        }
        b.compileMs = (now() - t0) * 1000
        return b
    }

    /// Compares reference and candidate over one spec. Returns the job result.
    func runJob(a aPath: String, b bPath: String, spec: J, arena: Arena) -> J {
        let t0 = now()
        var result: J = ["a": aPath, "b": bPath]
        var cases: [J] = []
        var status = "same"
        var totalCompile = 0.0, totalGpu = 0.0
        var positions: [String: [Int: ([UInt32], [UInt32])]] = [:]     // entry -> vector -> (A, B) x, y, w words
        do {
            let la = try device.makeLibrary(URL: URL(fileURLWithPath: aPath))
            let lb = try device.makeLibrary(URL: URL(fileURLWithPath: bPath))
            let na = Set(la.functionNames), nb = Set(lb.functionNames)
            if na != nb {
                result["status"] = "mismatch"
                result["error"] = "entry points differ: only A \(na.subtracting(nb).sorted()), only B \(nb.subtracting(na).sorted())"
                result["time_s"] = now() - t0
                return result
            }
            for entry in spec.dicts("entries") {
                let name = entry.str("name") ?? "?"
                if !na.contains(name) { cases.append(["entry": name, "status": "error", "error": "not in library"]); status = "error"; continue }
                let fcs = entry.dicts("fc_sets").isEmpty ? [J()] : entry.dicts("fc_sets")
                for (fi, fc) in fcs.enumerated() {
                    if gpuStuck { throw HarnessError("gpu stuck: stopping") }
                    if opts.budget > 0 && now() - t0 > opts.budget {
                        cases.append(["entry": name, "fc": fi, "status": "budget"]); continue
                    }
                    var c: J = ["entry": name, "mode": entry.str("mode") ?? "?", "fc": fi]
                    do {
                        let ba = try buildPipelines(la, entry: entry, fc: fc)
                        let bb = try buildPipelines(lb, entry: entry, fc: fc)
                        totalCompile += ba.compileMs + bb.compileMs
                        c["compile_ms"] = [round(ba.compileMs), round(bb.compileMs)]
                        if planSignature(ba.plan) != planSignature(bb.plan) {
                            c["status"] = "mismatch"; c["error"] = "interfaces differ (bindings or layouts)"
                            status = "diff"; cases.append(c); continue
                        }
                        let cs = Case(h: self, spec: spec, entry: entry, fc: fc, arena: arena)
                        try cs.build(plan: ba.plan)
                        var diffWords = 0, diffVectors = 0
                        var firsts: [J] = []
                        var gpu = 0.0
                        var written = 0
                        var nondetVectors = 0, suspect = 0
                        var tFill = 0.0, tRun = 0.0
                        let tCase = now()
                        var cstatus = "same"
                        for v in 0..<opts.vectors {
                            // entries of one seed group (a vertex shader's main0, main0_z, main0_gs and their capture
                            // twins) get the same inputs, so their positions can be compared with each other
                            let seed = seedFor(opts.seed, spec.str("name") ?? aPath, entry.str("seed_group") ?? name, "\(fi)")
                            let tf0 = now()
                            try cs.fill(vector: v, seed: seed)
                            let tf1 = now()
                            let oa = try cs.run(ba)
                            let ob = try cs.run(bb)
                            let tf2 = now()
                            tFill += tf1 - tf0; tRun += tf2 - tf1
                            gpu += oa.gpuMs + ob.gpuMs
                            written += cs.compare(cs.initialOutputs(), oa, limit: 0).0
                            if fi == 0, let cap = entry.dict("capture") {
                                positions[name, default: [:]][v] = (cs.positions(oa, cap), cs.positions(ob, cap))
                            }
                            if let dir = opts.dump {
                                let base = "\(dir)/\(name).fc\(fi).v\(v)"
                                try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
                                FileManager.default.createFile(atPath: base + ".arena.bin", contents: Data(oa.arena))
                                FileManager.default.createFile(atPath: base + ".arena0.bin", contents: Data(cs.snapshot))
                                for (tn, tb) in oa.textures { FileManager.default.createFile(atPath: base + ".\(tn).bin", contents: Data(tb)) }
                                let regs = cs.arena.regions.map { ["name": $0.name, "off": $0.off, "len": $0.len] as J }
                                FileManager.default.createFile(atPath: base + ".regions.json", contents: jsonString(regs).data(using: .utf8))
                            }
                            let (n, f) = cs.compare(oa, ob, limit: opts.maxDiffs)
                            if n > 0 {
                                // Some shaders are not deterministic on their own (implicit-LOD sampling inside divergent
                                // control flow reads derivatives of inactive lanes, atomics, ...). Words that differ between
                                // repeated runs of the reference (three runs) or of the candidate (two runs) are masked, only a
                                // difference outside the mask counts.
                                var masked = 0
                                var real = n
                                var shown = f
                                if opts.nondetCheck {
                                    let oa2 = try cs.run(ba), oa3 = try cs.run(ba), ob2 = try cs.run(bb)
                                    var pairs = [(oa, oa2), (oa, oa3), (ob, ob2)]
                                    var mask = cs.diffMask(pairs)
                                    if mask.isEmpty {
                                        // a clean difference: confirm it against more runs before calling it one (nondeterminism
                                        // that shows rarely, in a quad here and there, must not pass as a difference)
                                        for _ in 0..<4 { pairs.append((oa, try cs.run(ba))) }
                                        for _ in 0..<2 { pairs.append((ob, try cs.run(bb))) }
                                        mask = cs.diffMask(pairs)
                                    }
                                    masked = mask.values.reduce(0) { $0 + $1.count }
                                    if masked > 0 {
                                        (real, shown) = cs.compare(oa, ob, limit: opts.maxDiffs, mask: mask)
                                    }
                                }
                                if real == 0 || masked > 0 {
                                    // the shader is not deterministic on these inputs: undecidable here, reported apart,
                                    // with the differences outside the observed variation as suspects
                                    if cstatus == "same" { cstatus = "nondet" }
                                    nondetVectors += 1; suspect += real
                                    if firsts.count < opts.maxDiffs { firsts.append(["vector": v, "nondet_words": masked, "suspect_words": real, "nondet": Array(f.prefix(3)), "suspects": Array(shown.prefix(3))]) }
                                    continue
                                }
                                diffWords += real; diffVectors += 1
                                if firsts.count < opts.maxDiffs { firsts.append(["vector": v, "words": real, "nondet_words": masked, "diffs": shown]) }
                                cstatus = "diff"
                            }
                        }
                        totalGpu += gpu
                        c["ms"] = ["fill": round(tFill * 1000), "run2": round(tRun * 1000), "total": round((now() - tCase) * 1000)]
                        c["status"] = cstatus; c["vectors"] = opts.vectors; c["written_words"] = written; c["gpu_ms"] = (gpu * 1000).rounded() / 1000
                        if cstatus != "same" { c["diff_words"] = diffWords; c["diff_vectors"] = diffVectors; c["nondet_vectors"] = nondetVectors; c["suspect_words"] = suspect; c["first"] = firsts }
                        if cstatus == "diff" { status = "diff" } else if cstatus == "nondet" && status == "same" { status = "nondet" }
                        if opts.verbose { c["plan"] = planDump(ba.plan) }
                    } catch {
                        c["status"] = gpuStuck ? "hang" : "error"; c["error"] = "\(error)"
                        if status == "same" || status == "nondet" { status = "error" }
                        if gpuStuck { cases.append(c); throw error }
                    }
                    cases.append(c)
                }
            }
        } catch {
            result["error"] = "\(error)"
            if status == "same" { status = "error" }
        }
        // invariance inside each library: every capture of the same vertex shader (main0, main0_z, main0_gs, ...) must
        // give the same clip-space x, y and w bits on the same inputs (z differs by design: the depth remap)
        var groups: [String: [String]] = [:]
        for e in spec.dicts("entries") { if let n = e.str("name"), positions[n] != nil { groups[e.dict("capture")?.str("group") ?? "", default: []].append(n) } }
        var invs: [J] = []
        for (_, gnames) in groups.sorted(by: { $0.key < $1.key }) where gnames.count > 1 {
            let names = gnames.sorted()
            var inv: J = ["entries": names]
            for (lib, sel) in [("a", 0), ("b", 1)] {
                var bad = 0
                var first: [J] = []
                for v in positions[names[0]]!.keys.sorted() {
                    guard let ref = positions[names[0]]![v] else { continue }
                    let r0 = sel == 0 ? ref.0 : ref.1
                    for n in names.dropFirst() {
                        guard let o = positions[n]?[v] else { continue }
                        let r1 = sel == 0 ? o.0 : o.1
                        for i in 0..<min(r0.count, r1.count) where r0[i] != r1[i] {
                            bad += 1
                            if first.count < 4 { first.append(["vector": v, "entries": [names[0], n], "vertex_word": i, "bits": [hex32(r0[i]), hex32(r1[i])]]) }
                        }
                    }
                }
                inv[lib + "_mismatch_words"] = bad
                if !first.isEmpty { inv[lib + "_first"] = first }
            }
            invs.append(inv)
        }
        if !invs.isEmpty { result["invariance"] = invs }
        if gpuStuck && cases.contains(where: { ($0["status"] as? String) == "hang" }) { status = "hang" }
        result["status"] = status
        result["cases"] = cases
        result["compile_ms"] = round(totalCompile)
        result["gpu_ms"] = (totalGpu * 1000).rounded() / 1000
        result["time_s"] = ((now() - t0) * 1000).rounded() / 1000
        return result
    }

    /// Reflection of every function of a library: kinds, function constants, vertex attributes, and (when the spec
    /// gives enough to build a pipeline) the bindings with their full layouts, argument buffers included.
    func reflect(_ path: String, spec: J?) throws -> J {
        let lib = try device.makeLibrary(URL: URL(fileURLWithPath: path))
        var fns: [J] = []
        let entries = spec?.dicts("entries") ?? []
        for n in lib.functionNames.sorted() {
            guard let f = lib.makeFunction(name: n) else { continue }
            var o: J = ["name": n, "kind": f.functionType == .vertex ? "vertex" : f.functionType == .fragment ? "fragment" : f.functionType == .kernel ? "kernel" : "other"]
            o["function_constants"] = f.functionConstantsDictionary.map { (k, c) -> J in ["name": k, "index": c.index, "type": dataTypeName(c.type), "required": c.required] }.sorted { ($0["index"] as! Int) < ($1["index"] as! Int) }
            if let va = f.vertexAttributes {
                o["vertex_attributes"] = va.map { ["name": $0.name, "index": $0.attributeIndex, "type": dataTypeName($0.attributeType), "dt": Int($0.attributeType.rawValue), "active": $0.isActive] as J }
            }
            if let e = entries.first(where: { $0.str("name") == n }) {
                do {
                    let fc = e.dicts("fc_sets").first ?? [:]
                    let b = try buildPipelines(lib, entry: e, fc: fc)
                    o["bindings"] = planDump(b.plan)
                } catch { o["bindings_error"] = "\(error)" }
            }
            fns.append(o)
        }
        return ["library": path, "functions": fns]
    }
}
