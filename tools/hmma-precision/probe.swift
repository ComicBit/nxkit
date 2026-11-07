// Runs probe.metal on edge cases and prints the half product, the product with a float accumulator and the exact value.
import Foundation
import Metal

let dev = MTLCreateSystemDefaultDevice()!
print(dev.name)
let src = try! String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let lib = try! dev.makeLibrary(source: src, options: nil)
let pso = try! dev.makeComputePipelineState(function: lib.makeFunction(name: "mma")!)

func mat(_ f: (Int, Int) -> Float) -> [Float] { (0..<64).map { f($0 / 8, $0 % 8) } }
func row0(_ v: [Float]) -> [Float] { mat { r, k in r == 0 ? v[k] : 0 } }
let ones = mat { _, n in n == 0 ? 1 : 0 }
let zero = mat { _, _ in 0 }
let tests: [(String, [Float], [Float], [Float])] = [
    ("partial sum above 65504", row0([40000, 40000, -40000, -40000, 0, 0, 0, 0]), ones, zero),
    ("rounding of each step", row0([2048, 1, 1, 1, 1, 1, 1, 1]), ones, zero),
    ("C near the limit", row0([16, -16, 0, 0, 0, 0, 0, 0]), ones, mat { r, n in r == 0 && n == 0 ? 65504 : 0 }),
    ("product above 65504", row0([300, 300, 0, 0, 0, 0, 0, 0]), mat { k, n in n == 0 ? [300, -300, 0, 0, 0, 0, 0, 0][k] : 0 }, zero),
    ("products below the half range", row0([Float](repeating: 0.0001, count: 8)), mat { _, n in n == 0 ? 0.0001 : 0 }, zero),
    ("cancellation", row0([0.5, 4096, -4096, 0.25, 0, 0, 0, 0]), ones, zero),
]
let n = tests.count
func buf(_ sel: (Int) -> [Float]) -> MTLBuffer {
    let h = (0..<n).flatMap { sel($0).map { Float16($0) } }
    return dev.makeBuffer(bytes: h, length: h.count * 2)!
}
let a = buf { tests[$0].1 }, b = buf { tests[$0].2 }, c = buf { tests[$0].3 }
let d16 = dev.makeBuffer(length: n * 128)!, d32 = dev.makeBuffer(length: n * 256)!
let cb = dev.makeCommandQueue()!.makeCommandBuffer()!
let e = cb.makeComputeCommandEncoder()!
e.setComputePipelineState(pso)
for (i, x) in [a, b, c, d16, d32].enumerated() { e.setBuffer(x, offset: 0, index: i) }
e.dispatchThreadgroups(MTLSize(width: n, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 32, height: 1, depth: 1))
e.endEncoding()
cb.commit()
cb.waitUntilCompleted()
let h = d16.contents().bindMemory(to: Float16.self, capacity: n * 64)
let f = d32.contents().bindMemory(to: Float.self, capacity: n * 64)
for (i, t) in tests.enumerated() {
    var exact = Double(Float16(t.3[0]))
    for k in 0..<8 { exact += Double(Float16(t.1[k])) * Double(Float16(t.2[k * 8])) }
    print("\(t.0): half \(h[i * 64]), float accumulator \(f[i * 64]), exact \(Float16(Float(exact))) (\(exact))")
}
