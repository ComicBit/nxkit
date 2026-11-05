#!/bin/sh
# Speed: the whole corpus at 12 threads, three runs (report the median), and optionally the prototype once.
# usage: tools/bench.sh <work dir> [--python]
# Run it on an otherwise idle machine. Wall times come from /usr/bin/time, per-file times from the JSON `sec` fields.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
work=$1
proto=${MSL_PROTO:?set MSL_PROTO}
target=${CARGO_TARGET_DIR:-target}
[ -f "$work/corpus.txt" ] || python3 -c "import json,sys; print('\n'.join(r['path'] for r in json.load(open(sys.argv[1]))))" "$proto/parse_results.json" > "$work/corpus.txt"
xargs cat < "$work/corpus.txt" > /dev/null   # warm the file cache
for r in 1 2 3; do
    uptime
    /usr/bin/time -p "$target/release/mslcheck" -j 12 --json --stats -l "$work/corpus.txt" > "$work/bench_rs_$r.jsonl"
done
if [ "${2:-}" = "--python" ]; then
    uptime
    /usr/bin/time -p python3 "$proto/msl_parse.py" "$work/corpus.txt" "$work/bench_py.json"
fi
python3 - "$work" <<'PY'
import json, statistics as st, sys, os
w = sys.argv[1]
for r in (1, 2, 3):
    s = [json.loads(l)['sec'] for l in open(f'{w}/bench_rs_{r}.jsonl')]
    print(f'rust run {r}: per file median {st.median(s)*1e3:.3f} ms, mean {st.mean(s)*1e3:.3f} ms')
if os.path.exists(f'{w}/bench_py.json'):
    s = [r['sec'] for r in json.load(open(f'{w}/bench_py.json'))]
    print(f'python: per file median {st.median(s)*1e3:.2f} ms, mean {st.mean(s)*1e3:.2f} ms')
PY
