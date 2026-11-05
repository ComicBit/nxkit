#!/usr/bin/env python3
"""Compares mslcheck --json --stats output with the prototype's results (parse_results.json, or the output of py_check.py).

usage: compare_results.py <python results .json or .jsonl> <rust .jsonl> [--no-stats]
Per file it compares: ok, reasons (keys and counts), detail, and the construct counters (stats).
Prints every difference (up to 50 per kind) and a summary; exit status 1 when anything differs."""
import json, sys, collections


def load(path):
    if path.endswith('.jsonl'):
        return [json.loads(l) for l in open(path) if l.strip()]
    return json.load(open(path))


def main():
    py = {r['path']: r for r in load(sys.argv[1])}
    rs = {r['path']: r for r in load(sys.argv[2])}
    stats = '--no-stats' not in sys.argv
    diff = collections.defaultdict(list)
    missing = [p for p in py if p not in rs]
    for p, a in py.items():
        b = rs.get(p)
        if b is None: continue
        if a['ok'] != b['ok']: diff['ok'].append((p, a['ok'], b['ok'], a['reasons'], b['reasons']))
        if a['reasons'] != b['reasons']: diff['reasons'].append((p, a['reasons'], b['reasons']))
        if a.get('detail', '') != b.get('detail', ''): diff['detail'].append((p, a.get('detail'), b.get('detail')))
        if stats and a.get('stats', {}) != b.get('stats', {}):
            sa, sb = a.get('stats', {}), b.get('stats', {})
            keys = sorted(k for k in set(sa) | set(sb) if sa.get(k) != sb.get(k))
            diff['stats'].append((p, {k: (sa.get(k), sb.get(k)) for k in keys[:10]}))
    n = len(py)
    acc_py = sum(r['ok'] for r in py.values()); acc_rs = sum(rs[p]['ok'] for p in py if p in rs)
    print(f'files: {n} python, {len(rs)} rust, {len(missing)} missing in rust')
    print(f'accepted: python {acc_py}, rust {acc_rs}')
    for k in ('ok', 'reasons', 'detail', 'stats'):
        print(f'{k} differences: {len(diff[k])}')
        for d in diff[k][:50]: print('   ', d)
    sys.exit(1 if missing or any(diff.values()) else 0)


if __name__ == '__main__':
    main()
