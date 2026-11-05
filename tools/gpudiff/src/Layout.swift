// gpudiff: type layout taken from Metal pipeline reflection, path resolution for fill rules, JSON dump
import Metal

/// Scalar component kind: f float, h half, i signed int, u unsigned int, b bool
struct Scalar { let comps: Int; let size: Int; let kind: Character }

func scalarInfo(_ dt: MTLDataType) -> Scalar? {
    switch dt {
    case .float: return Scalar(comps: 1, size: 4, kind: "f")
    case .float2: return Scalar(comps: 2, size: 4, kind: "f")
    case .float3: return Scalar(comps: 3, size: 4, kind: "f")
    case .float4: return Scalar(comps: 4, size: 4, kind: "f")
    case .half: return Scalar(comps: 1, size: 2, kind: "h")
    case .half2: return Scalar(comps: 2, size: 2, kind: "h")
    case .half3: return Scalar(comps: 3, size: 2, kind: "h")
    case .half4: return Scalar(comps: 4, size: 2, kind: "h")
    case .int: return Scalar(comps: 1, size: 4, kind: "i")
    case .int2: return Scalar(comps: 2, size: 4, kind: "i")
    case .int3: return Scalar(comps: 3, size: 4, kind: "i")
    case .int4: return Scalar(comps: 4, size: 4, kind: "i")
    case .uint: return Scalar(comps: 1, size: 4, kind: "u")
    case .uint2: return Scalar(comps: 2, size: 4, kind: "u")
    case .uint3: return Scalar(comps: 3, size: 4, kind: "u")
    case .uint4: return Scalar(comps: 4, size: 4, kind: "u")
    case .short: return Scalar(comps: 1, size: 2, kind: "i")
    case .short2: return Scalar(comps: 2, size: 2, kind: "i")
    case .short3: return Scalar(comps: 3, size: 2, kind: "i")
    case .short4: return Scalar(comps: 4, size: 2, kind: "i")
    case .ushort: return Scalar(comps: 1, size: 2, kind: "u")
    case .ushort2: return Scalar(comps: 2, size: 2, kind: "u")
    case .ushort3: return Scalar(comps: 3, size: 2, kind: "u")
    case .ushort4: return Scalar(comps: 4, size: 2, kind: "u")
    case .char: return Scalar(comps: 1, size: 1, kind: "i")
    case .char2: return Scalar(comps: 2, size: 1, kind: "i")
    case .char3: return Scalar(comps: 3, size: 1, kind: "i")
    case .char4: return Scalar(comps: 4, size: 1, kind: "i")
    case .uchar: return Scalar(comps: 1, size: 1, kind: "u")
    case .uchar2: return Scalar(comps: 2, size: 1, kind: "u")
    case .uchar3: return Scalar(comps: 3, size: 1, kind: "u")
    case .uchar4: return Scalar(comps: 4, size: 1, kind: "u")
    case .bool: return Scalar(comps: 1, size: 1, kind: "b")
    case .bool2: return Scalar(comps: 2, size: 1, kind: "b")
    case .bool3: return Scalar(comps: 3, size: 1, kind: "b")
    case .bool4: return Scalar(comps: 4, size: 1, kind: "b")
    case .long: return Scalar(comps: 1, size: 8, kind: "i")
    case .ulong: return Scalar(comps: 1, size: 8, kind: "u")
    default: return nil
    }
}

func dataTypeName(_ dt: MTLDataType) -> String {
    switch dt {
    case .struct: return "struct"; case .array: return "array"; case .pointer: return "pointer"
    case .texture: return "texture"; case .sampler: return "sampler"
    default:
        if let s = scalarInfo(dt) {
            let base: String = ["f": "float", "h": "half", "i": s.size == 2 ? "short" : s.size == 1 ? "char" : s.size == 8 ? "long" : "int",
                                "u": s.size == 2 ? "ushort" : s.size == 1 ? "uchar" : s.size == 8 ? "ulong" : "uint", "b": "bool"][s.kind]!
            return s.comps == 1 ? base : base + String(s.comps)
        }
        return "dt\(dt.rawValue)"
    }
}

/// A node of the reflected type tree
final class TNode {
    enum Kind { case scalar, structT, array, pointer, texture, sampler, other }
    var kind: Kind = .other
    var dt: MTLDataType = .none
    var scalar: Scalar? = nil
    var members: [(name: String, offset: Int, node: TNode, argIndex: Int)] = []
    var elem: TNode? = nil
    var count = 0
    var stride = 0
    var size = 0
    var pointee: TNode? = nil
    var pointeeSize = 0
    var access: MTLBindingAccess = .readOnly
    var texType: MTLTextureType = .type2D
    var texData: MTLDataType = .float
    var texDepth = false
    var texAccess: MTLBindingAccess = .readOnly

    static func from(dt: MTLDataType, st: MTLStructType?, at: MTLArrayType?, pt: MTLPointerType?, tt: MTLTextureReferenceType?, fallbackSize: Int = 0) -> TNode {
        let n = TNode(); n.dt = dt
        switch dt {
        case .struct:
            n.kind = .structT
            if let st = st { for m in st.members { n.members.append((m.name, m.offset, TNode.member(m), m.argumentIndex)) } }
            n.size = n.members.map { $0.offset + max($0.node.size, 1) }.max() ?? 0
            if fallbackSize > n.size { n.size = fallbackSize }
        case .array:
            n.kind = .array
            if let at = at {
                n.count = at.arrayLength; n.stride = at.stride
                n.elem = TNode.from(dt: at.elementType, st: at.elementStructType(), at: at.element(), pt: at.elementPointerType(), tt: at.elementTextureReferenceType(), fallbackSize: at.stride)
                n.size = n.count * n.stride
            }
        case .pointer:
            n.kind = .pointer; n.size = 8
            if let pt = pt {
                n.access = pt.access; n.pointeeSize = pt.dataSize
                n.pointee = TNode.from(dt: pt.elementType, st: pt.elementStructType(), at: pt.elementArrayType(), pt: nil, tt: nil, fallbackSize: pt.dataSize)
            }
        case .texture:
            n.kind = .texture; n.size = 8
            if let tt = tt { n.texType = tt.textureType; n.texData = tt.textureDataType; n.texDepth = tt.isDepthTexture; n.texAccess = tt.access }
        case .sampler:
            n.kind = .sampler; n.size = 8
        default:
            if let s = scalarInfo(dt) {
                n.kind = .scalar; n.scalar = s
                n.size = s.comps == 3 ? 4 * s.size : s.comps * s.size   // MSL vec3 occupies the room of a vec4
                if fallbackSize > 0 && fallbackSize < n.size { n.size = fallbackSize }
            } else { n.size = max(fallbackSize, 0) }
        }
        return n
    }
    static func member(_ m: MTLStructMember) -> TNode {
        return TNode.from(dt: m.dataType, st: m.structType(), at: m.arrayType(), pt: m.pointerType(), tt: m.textureReferenceType())
    }

    /// A stable text form, used to compare the reference's and the candidate's interfaces
    func signature() -> String {
        switch kind {
        case .scalar: return dataTypeName(dt)
        case .structT: return "{" + members.map { "\($0.name)@\($0.offset)#\($0.argIndex):\($0.node.signature())" }.joined(separator: ",") + "}"
        case .array: return "[\(count)x\(stride)]" + (elem?.signature() ?? "?")
        case .pointer: return "*\(access.rawValue)/\(pointeeSize)" + (pointee?.signature() ?? "?")
        case .texture: return "tex(\(texType.rawValue),\(texData.rawValue),\(texDepth),\(texAccess.rawValue))"
        case .sampler: return "sampler"
        case .other: return "other\(dt.rawValue)/\(size)"
        }
    }

    func dump(depth: Int = 0) -> J {
        var o: J = ["type": dataTypeName(dt), "size": size]
        switch kind {
        case .structT:
            o["members"] = members.map { (m) -> J in
                var d = m.node.dump(depth: depth + 1); d["name"] = m.name; d["offset"] = m.offset
                if m.argIndex >= 0 { d["id"] = m.argIndex }
                return d
            }
        case .array:
            o["count"] = count; o["stride"] = stride
            if let e = elem { o["elem"] = e.dump(depth: depth + 1) }
        case .pointer:
            o["access"] = access == .readOnly ? "read" : access == .writeOnly ? "write" : "read_write"
            o["pointee_size"] = pointeeSize
            if let p = pointee, depth < 6 { o["pointee"] = p.dump(depth: depth + 1) }
        case .texture:
            o["texture_type"] = texTypeName(texType); o["data"] = dataTypeName(texData); o["depth"] = texDepth
            o["access"] = texAccess == .readOnly ? "read" : texAccess == .writeOnly ? "write" : "read_write"
        default: break
        }
        return o
    }

    /// Resolves a rule path ("_m0[14].x", "k[*]", "s[*].stride", "" for the whole node) to a list of scalar slots
    func resolve(_ path: String) -> [(off: Int, sc: Scalar)]? {
        var toks: [String] = []
        var cur = ""
        for ch in path {
            if ch == "." { if !cur.isEmpty { toks.append(cur) }; cur = "" }
            else if ch == "[" { if !cur.isEmpty { toks.append(cur) }; cur = "[" }
            else if ch == "]" { cur += "]"; toks.append(cur); cur = "" }
            else { cur.append(ch) }
        }
        if !cur.isEmpty { toks.append(cur) }
        var states: [(Int, TNode, Int?)] = [(0, self, nil)]   // offset, node, selected component of a vector
        for t in toks {
            var next: [(Int, TNode, Int?)] = []
            for (off, n, comp) in states {
                if t.hasPrefix("[") {
                    let inner = String(t.dropFirst().dropLast())
                    if n.kind == .array, let e = n.elem {
                        if inner == "*" { for i in 0..<n.count { next.append((off + i * n.stride, e, nil)) } }
                        else if let i = Int(inner), i >= 0, i < n.count { next.append((off + i * n.stride, e, nil)) }
                        else { return nil }
                    } else if n.kind == .scalar, let s = n.scalar, comp == nil {
                        if inner == "*" { for c in 0..<s.comps { next.append((off, n, c)) } }
                        else if let c = Int(inner), c < s.comps { next.append((off, n, c)) } else { return nil }
                    } else { return nil }
                } else if n.kind == .structT {
                    guard let m = n.members.first(where: { $0.name == t }) else { return nil }
                    next.append((off + m.offset, m.node, nil))
                } else if n.kind == .scalar, let s = n.scalar, comp == nil {
                    var cs: [Int] = []
                    for ch in t {
                        switch ch { case "x", "r": cs.append(0); case "y", "g": cs.append(1); case "z", "b": cs.append(2); case "w", "a": cs.append(3); default: return nil }
                    }
                    for c in cs where c < s.comps { next.append((off, n, c)) }
                } else { return nil }
            }
            states = next
        }
        var out: [(off: Int, sc: Scalar)] = []
        for (off, n, comp) in states { n.slots(base: off, comp: comp, into: &out) }
        return out
    }

    func slots(base: Int, comp: Int? = nil, into out: inout [(off: Int, sc: Scalar)]) {
        switch kind {
        case .scalar:
            guard let s = scalar else { return }
            let one = Scalar(comps: 1, size: s.size, kind: s.kind)
            if let c = comp { out.append((base + c * s.size, one)) }
            else { for c in 0..<s.comps { out.append((base + c * s.size, one)) } }
        case .structT: for m in members { m.node.slots(base: base + m.offset, into: &out) }
        case .array: if let e = elem { for i in 0..<count { e.slots(base: base + i * stride, into: &out) } }
        default: break
        }
    }
}

func texTypeName(_ t: MTLTextureType) -> String {
    switch t {
    case .type1D: return "1d"; case .type1DArray: return "1d_array"; case .type2D: return "2d"; case .type2DArray: return "2d_array"
    case .type2DMultisample: return "2d_ms"; case .typeCube: return "cube"; case .typeCubeArray: return "cube_array"; case .type3D: return "3d"
    case .type2DMultisampleArray: return "2d_ms_array"; case .typeTextureBuffer: return "buffer"
    @unknown default: return "t\(t.rawValue)"
    }
}

/// One pipeline binding the harness has to supply
struct BindingPlan {
    let stage: String       // "vertex", "fragment", "kernel"
    let name: String
    let index: Int
    let type: MTLBindingType
    let node: TNode?        // buffer: the pointee type (struct or scalar)
    let size: Int
    let isArgbuf: Bool
    let used: Bool
}

func planBindings(_ bindings: [MTLBinding], stage: String) -> [BindingPlan] {
    var out: [BindingPlan] = []
    for b in bindings {
        switch b.type {
        case .buffer:
            guard let bb = b as? MTLBufferBinding else { continue }
            var node: TNode? = nil
            if let st = bb.bufferStructType {
                node = TNode.from(dt: .struct, st: st, at: nil, pt: nil, tt: nil, fallbackSize: bb.bufferDataSize)
            } else if let pt = bb.bufferPointerType, let es = pt.elementStructType() {
                node = TNode.from(dt: .struct, st: es, at: nil, pt: nil, tt: nil, fallbackSize: bb.bufferDataSize)
            } else {
                node = TNode.from(dt: bb.bufferDataType, st: nil, at: nil, pt: nil, tt: nil, fallbackSize: bb.bufferDataSize)
            }
            let isArg = b.name == "spvDescriptorSet0" || (node?.members.contains(where: { $0.node.kind == .texture || $0.node.kind == .sampler }) ?? false)
            out.append(BindingPlan(stage: stage, name: b.name, index: b.index, type: .buffer, node: node, size: max(bb.bufferDataSize, node?.size ?? 0), isArgbuf: isArg, used: b.isUsed))
        case .texture:
            var node: TNode? = nil
            if let tb = b as? MTLTextureBinding {
                let n = TNode(); n.kind = .texture; n.texType = tb.textureType; n.texData = tb.textureDataType; n.texDepth = tb.isDepthTexture; n.texAccess = tb.access
                node = n
            }
            out.append(BindingPlan(stage: stage, name: b.name, index: b.index, type: b.type, node: node, size: 0, isArgbuf: false, used: b.isUsed))
        case .sampler:
            out.append(BindingPlan(stage: stage, name: b.name, index: b.index, type: b.type, node: nil, size: 0, isArgbuf: false, used: b.isUsed))
        default:
            out.append(BindingPlan(stage: stage, name: b.name, index: b.index, type: b.type, node: nil, size: 0, isArgbuf: false, used: b.isUsed))
        }
    }
    return out.sorted { ($0.stage, $0.index, $0.name) < ($1.stage, $1.index, $1.name) }
}

func planSignature(_ p: [BindingPlan]) -> String {
    p.map { "\($0.stage):\($0.index):\($0.name):\($0.type.rawValue):\($0.node?.signature() ?? "-")" }.joined(separator: "|")
}

func planDump(_ p: [BindingPlan]) -> [J] {
    p.map { b in
        var o: J = ["stage": b.stage, "name": b.name, "index": b.index, "used": b.used,
                    "binding": b.type == .buffer ? "buffer" : b.type == .texture ? "texture" : b.type == .sampler ? "sampler" : "other\(b.type.rawValue)"]
        if let n = b.node { o["layout"] = n.dump(); o["argbuf"] = b.isArgbuf; o["size"] = b.size }
        return o
    }
}
