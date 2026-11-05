// gpudiff: GPU differential test of two compiled versions of the same Metal shader.
//
//   gpudiff reflect <lib.metallib> [--spec spec.json]
//   gpudiff run <A.metallib> <B.metallib> <spec.json> [options]
//   gpudiff batch <jobs.jsonl> <results.jsonl> [--workers N] [options]     (job line: {"id","a","b","spec"})
//
// options: --vectors N (3) --seed S (1) --timeout SEC (4) --max-diffs N (8) --no-nondet --verbose --arena-mb N (24)
import Foundation
import Metal

var args = Array(CommandLine.arguments.dropFirst())
var opts = Opts()
var workers = 4
var positional: [String] = []
var specOverride: String? = nil
var i = 0
while i < args.count {
    let a = args[i]
    func val() -> String { i += 1; return i < args.count ? args[i] : "" }
    switch a {
    case "--vectors": opts.vectors = Int(val()) ?? 12
    case "--seed": opts.seed = UInt64(val()) ?? 1
    case "--timeout": opts.timeout = Double(val()) ?? 4
    case "--max-diffs": opts.maxDiffs = Int(val()) ?? 8
    case "--no-nondet": opts.nondetCheck = false
    case "--verbose": opts.verbose = true
    case "--arena-mb": opts.arenaMB = Int(val()) ?? 24
    case "--workers": workers = max(1, Int(val()) ?? 4)
    case "--spec": specOverride = val()
    case "--dump": opts.dump = val()
    case "--dry": opts.dry = true
    case "--budget": opts.budget = Double(val()) ?? 0
    default: positional.append(a)
    }
    i += 1
}

func usage() -> Never {
    eprint("usage: gpudiff reflect <lib.metallib> [--spec spec.json]\n       gpudiff run <A.metallib> <B.metallib> <spec.json> [options]\n       gpudiff batch <jobs.jsonl> <results.jsonl> [--workers N] [options]")
    exit(2)
}

guard let cmd = positional.first else { usage() }
do {
    let h = try Harness(opts: opts)
    switch cmd {
    case "reflect":
        guard positional.count >= 2 else { usage() }
        let spec = try specOverride.map { try loadJSON($0) as? J } ?? nil
        print(jsonString(try h.reflect(positional[1], spec: spec ?? nil), pretty: true))
    case "run":
        guard positional.count >= 4 else { usage() }
        guard let spec = try loadJSON(positional[3]) as? J else { throw HarnessError("spec is not an object") }
        let arena = try Arena(device: h.device, capacity: opts.arenaMB << 20)
        let r = h.runJob(a: positional[1], b: positional[2], spec: spec, arena: arena)
        print(jsonString(r, pretty: true))
        let st = r["status"] as? String ?? "error"
        exit(st == "same" ? 0 : st == "diff" || st == "mismatch" ? 1 : st == "nondet" ? 3 : 4)
    case "batch":
        guard positional.count >= 3 else { usage() }
        let lines = try String(contentsOfFile: positional[1], encoding: .utf8).split(separator: "\n").map(String.init).filter { !$0.isEmpty }
        let jobs: [J] = lines.compactMap { (try? JSONSerialization.jsonObject(with: $0.data(using: .utf8)!)) as? J }
        FileManager.default.createFile(atPath: positional[2], contents: nil)
        guard let out = FileHandle(forWritingAtPath: positional[2]) else { throw HarnessError("cannot write \(positional[2])") }
        let lock = NSLock()
        var next = 0
        var counts: [String: Int] = [:]
        let t0 = now()
        let group = DispatchGroup()
        for w in 0..<min(workers, max(1, jobs.count)) {
            group.enter()
            Thread.detachNewThread {
                defer { group.leave() }
                guard let arena = try? Arena(device: h.device, capacity: opts.arenaMB << 20) else { eprint("worker \(w): no arena"); return }
                while true {
                    lock.lock()
                    if next >= jobs.count || gpuStuck { lock.unlock(); return }
                    let job = jobs[next]; next += 1
                    lock.unlock()
                    autoreleasepool {
                        var r: J
                        if let sp = job.str("spec"), let spec = (try? loadJSON(sp)) as? J {
                            r = h.runJob(a: job.str("a") ?? "", b: job.str("b") ?? "", spec: spec, arena: arena)
                        } else { r = ["status": "error", "error": "spec unreadable: \(job.str("spec") ?? "-")"] }
                        r["id"] = job["id"] ?? NSNull()
                        if let m = job["meta"] { r["meta"] = m }
                        let line = jsonString(r) + "\n"
                        lock.lock()
                        out.write(line.data(using: .utf8)!)
                        let st = r["status"] as? String ?? "error"
                        counts[st, default: 0] += 1
                        let done = counts.values.reduce(0, +)
                        if done % 25 == 0 || done == jobs.count { eprint("[\(done)/\(jobs.count)] \(String(format: "%.1f", now() - t0)) s \(counts)") }
                        lock.unlock()
                    }
                }
            }
        }
        group.wait()
        out.closeFile()
        let summary: J = ["jobs": jobs.count, "counts": counts, "seconds": now() - t0, "per_second": Double(jobs.count) / max(0.001, now() - t0), "gpu_stuck": gpuStuck]
        print(jsonString(summary, pretty: true))
        exit(gpuStuck ? 5 : 0)
    default: usage()
    }
} catch {
    eprint("gpudiff: \(error)")
    exit(4)
}
