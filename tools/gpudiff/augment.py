#!/usr/bin/env python3
"""Adds an exact capture twin to every vertex entry point that returns its outputs (main0, main0_z, main0_l, ...):

    vertex void <name>_hcap(<same parameters>, device uint* dm_hcap [[buffer(K)]], [vertex_id], [instance_id])

with the same body, where every `return out;` first stores each output word (position, point size, every varying) as
raw bits at dm_hcap[(instance * 128 + vertex) * S + k]. A rasterized vertex shader only shows what lands on pixels;
the twin shows every output word of every vertex, bit for bit, like the engine's own main0_gs capture variant.
Both front ends compile the augmented source, so reference and candidate stay comparable.

usage: augment.py <in.metal> <out.metal>     prints JSON {"entries": {name: {"words": S, "buffer": K}}}
"""
import sys, os, re, json
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import msl_info as M

MAXV = 128


def words_of(t):
    t = t.strip()
    m = re.fullmatch(r'(float|int|uint|half|short|ushort|bool)([234]?)', t)
    if not m: return None
    return m.group(1), int(m.group(2) or 1)


def store_code(members, stride):
    lines, k = [], 0
    comps = 'xyzw'
    for mem in members:
        wt = words_of(mem['type'])
        if not wt: continue
        base, n = wt
        cnt = mem['array'] or 1
        for a in range(cnt):
            ref = 'out.%s%s' % (mem['name'], '[%d]' % a if mem['array'] else '')
            for c in range(n):
                v = ref + ('.' + comps[c] if n > 1 else '')
                if base in ('float', 'int', 'uint'): e = 'as_type<uint>(%s)' % v
                elif base == 'half': e = 'uint(as_type<ushort>(%s))' % v
                elif base in ('short', 'ushort'): e = 'uint(as_type<ushort>(%s))' % v
                else: e = 'uint(%s)' % v
                lines.append('dm_hp[%d] = %s;' % (k, e)); k += 1
    return ' '.join(lines), k


def augment(src):
    info = M.analyse(src)
    structs = info['structs']
    used = set()
    for e in info['entries']:
        for p in e['params']:
            b = M.attr_val(p['attrs'], 'buffer')
            if b not in (None, ''): used.add(int(b))
    free = [i for i in range(30, 0, -1) if i not in used]
    K = free[1] if len(free) > 1 else free[0]          # free[0] stays for the vertex attribute buffer
    out_src, meta = src, {}
    for e in info['entries']:
        if e['kind'] != 'vertex' or e['ret'] == 'void': continue
        members = structs.get(e['ret'], [])
        stride = sum((words_of(m['type'])[1] if words_of(m['type']) else 0) * (m['array'] or 1) for m in members)
        if stride == 0: continue
        # the function text
        m = re.search(r'^(\[\[[^\n]*?\]\]\s*)?vertex\s+' + re.escape(e['ret']) + r'\s+' + re.escape(e['name']) + r'\((?P<params>[^\n]*)\)\s*$', out_src, re.M)
        if not m: continue
        i = out_src.index('{', m.end()); d = 1; j = i + 1
        while d:
            if out_src[j] == '{': d += 1
            elif out_src[j] == '}': d -= 1
            j += 1
        body = out_src[i:j]
        params = m.group('params')
        vid = next((p['name'] for p in e['params'] if 'vertex_id' in p['attrs']), None)
        iid = next((p['name'] for p in e['params'] if 'instance_id' in p['attrs']), None)
        extra = ', device uint* dm_hcap [[buffer(%d)]]' % K
        if not vid: vid = 'dm_hvid'; extra += ', uint dm_hvid [[vertex_id]]'
        if not iid: iid = 'dm_hiid'; extra += ', uint dm_hiid [[instance_id]]'
        code, n = store_code(members, stride)
        store = '{ device uint* dm_hp = dm_hcap + (ulong(%s) * %du + ulong(%s)) * %du; %s return; }' % (iid, MAXV, vid, n, code)
        nbody = re.sub(r'\breturn\s+out\s*;', store, body)
        fn = '\n\nvertex void %s_hcap(%s%s)\n%s\n' % (e['name'], params, extra, nbody)
        out_src = out_src + fn
        meta[e['name'] + '_hcap'] = {'words': n, 'buffer': K, 'of': e['name']}
    return out_src, meta


if __name__ == '__main__':
    s, meta = augment(open(sys.argv[1], errors='replace').read())
    open(sys.argv[2], 'w').write(s)
    print(json.dumps({'entries': meta}))
