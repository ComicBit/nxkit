#!/usr/bin/env python3
"""Synthetic NSOs for the rewriter's oracle runs (development tool, not part of a build).

Usage: gen_synth_nso.py <patch snapshot (tools/gen_rewrite_patches.py)> <out dir>
Writes <out dir>/<name>.nso for modules that no local title provides, and <out dir>/cases.tsv (profile, file, name,
reference folder) for the oracle runs of patch_nso.py and of nxb-rewrite:
  * main modules of builds whose registered patches (cuts, observation gates, frame step sites) never ran on a module
    here (three frame step builds and one build with registered cuts): text of nop words with the
    original bytes the snapshot asserts, at their offsets, and the build ID in the header. They exercise every stub of
    a frame step and every build bound cut against Python, but hold no game code other than the asserted words.
  * layouts: a text segment of 125.3 MB (the size of a large title: one patch area before the base), 185.6 MB (the size of a larger
    title: two patch areas) and 100 MB, with `mrs x0, tpidr_el0` and `svc` sites spread over the whole text.
    The `-rerun` modules have so many sites that the front area is too small at its first size, so the rewriter runs
    again with the size it needs (DM_PATCH_FRONT in the script).
The words are synthetic: only the layout code and the registered rows are checked, not the rules on real code."""
import hashlib, os, struct, sys

snap, out = sys.argv[1], os.path.abspath(sys.argv[2])
os.makedirs(out, exist_ok=True)
NOP = 0xd503201f
rows = [l.rstrip('\n').split('\t') for l in open(snap, encoding='utf-8')]
def unesc(s): return s.replace('\\t', '\t').replace('\\n', '\n').replace('\\\\', '\\')

def build_asserts(bid):
    """(offset, bytes) the registered patches of a build assert"""
    a = []
    for r in rows:
        if r[0] == 'cuts' and r[1] == bid: a.append((int(r[2]), bytes.fromhex(r[3])))
        elif r[0] == 'obs' and r[1] == bid: a.append((int(r[2]), bytes.fromhex(r[4])))
        elif r[0] == 'fs' and r[1] == bid and r[2] == 'site': a.append((int(r[3]), bytes.fromhex(r[4])))
    return a

def make_nso(text, ro, data, bss, bid, fn_every=0x1000):
    """an uncompressed NSO0 with the check-hash flags set. The text starts with `b #8` and the offset of the MOD0
    block, which holds the dynamic section's offset (an empty one in data) and an eh_frame_hdr with a function every
    fn_every bytes: what the script reads function boundaries from"""
    def pad(b): return b + bytes(-len(b) % 0x1000)
    text = bytearray(text)
    tlen = (len(text) + 0xfff) & ~0xfff
    m0 = tlen                                   # MOD0 at the start of ro
    ehs_rel = 0x40
    nf = tlen // fn_every
    hdr_eh = struct.pack('<BBBB', 1, 0x03, 0x03, 0x3B) + struct.pack('<II', 0, nf)
    ehs = m0 + ehs_rel
    table = b''.join(struct.pack('<ii', i * fn_every - ehs, 0) for i in range(nf))
    ro_len = (ehs_rel + len(hdr_eh) + len(table) + len(ro) + 0xfff) & ~0xfff
    dyn_rel = tlen + ro_len - m0   # data starts right after ro: an all-zero (empty) dynamic section
    modb = struct.pack('<4sIIIIII', b'MOD0', dyn_rel, 0, 0, ehs_rel, 0, 0).ljust(ehs_rel, b'\0') + hdr_eh + table
    ro = modb + bytes(ro)
    struct.pack_into('<II', text, 0, 0x14000002, m0)
    text, ro, data = pad(bytes(text)), pad(bytes(ro)), pad(bytes(data))
    assert len(text) == tlen
    segs = [text, ro, data]
    hdr = bytearray(0x100)
    hdr[0:4] = b'NSO0'
    struct.pack_into('<I', hdr, 0xc, 0x38)
    fo, mo = 0x100, 0
    for i, s in enumerate(segs):
        struct.pack_into('<IIII', hdr, 0x10 + i * 0x10, fo, mo, len(s), 0)
        struct.pack_into('<I', hdr, 0x60 + i * 4, len(s))
        hdr[0xa0 + 0x20 * i:0xc0 + 0x20 * i] = hashlib.sha256(s).digest()
        fo += len(s); mo += len(s)
    struct.pack_into('<I', hdr, 0x3c, bss)
    hdr[0x40:0x50] = bytes.fromhex(bid)
    return bytes(hdr) + b''.join(segs)

def small_text(bid, extra=0x10000):
    a = build_asserts(bid)
    size = (max(o + len(b) for o, b in a) + extra + 0xfff) & ~0xfff
    text = bytearray(struct.pack('<I', NOP) * (size // 4))
    for o, b in a: text[o:o + len(b)] = b
    return text

cases = []
def emit(name, profile, data):
    p = os.path.join(out, name + '.nso')
    open(p, 'wb').write(data)
    cases.append(f'{profile}\t{p}\tmain\t{out}/ref-{name}')   # every case is a `main` (the frame step stubs and the main-only rules)
    print(name, len(data))

ro = bytes(0x2000); data = bytes(0x2000)
for name, bid, prof in (('stub-a-main', '4b159f0f7a360669738390ee8657b4f0', '01007300020FA000'),
                        ('stub-b-main', '0effe4af1dec3a7966b934d4a7c3d2bf', '0100ECD018EBE000'),
                        ('stub-c-main', 'c3cf52bf2b05d7310884971b50bcccb8', '01002B00111A2000'),
                        ('cuts-main', 'feae83dc9e9fb34c77d0a62f1fa9ae9f', '0100ACD0163D0000')):
    emit(name, prof, make_nso(small_text(bid), ro, data, 0x100000, bid))

def sites_text(mb, every):
    n = int(mb * 1024 * 1024) // 4
    text = bytearray(struct.pack('<I', NOP) * n)
    k = 0
    for w in range(1000, n - 1000, every):
        struct.pack_into('<I', text, w * 4, 0xd53bd040 if k % 3 else 0xd4000021)   # mrs x0, tpidr_el0 / svc #1
        k += 1
    return text, k

for name, mb, every in (('bigtext-main', 125.3, 1200), ('twotext-main', 185.6, 1800), ('text100-main', 100.0, 1500),
                       ('bigtext-rerun-main', 125.3, 300), ('twotext-rerun-main', 185.6, 55)):
    text, k = sites_text(mb, every)
    bid = hashlib.sha256(name.encode()).hexdigest()[:32]
    emit(name, '0100000000000000', make_nso(text, ro, data, 0x200000, bid))
    print('  sites', k)
open(os.path.join(out, 'cases.tsv'), 'w').write('\n'.join(cases) + '\n')
