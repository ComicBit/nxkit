#!/usr/bin/env python3
"""Synthetic main modules for the finder `ue_region` (development tool, not part of a build).

Usage: gen_synth_region.py <engine folder holding dm/> <python with capstone 5> <out dir>
Writes <out dir>/<case>.nso for small modules that hold the forms the finder knows (and the forms it refuses), runs
the reference script `dm/tools/patch_nso.py` on each, and writes the expected results for the test
`crates/nxb-rewrite/tests/region.rs`: <out dir>/expected.tsv (case, output file, length, SHA-256) and
<out dir>/<case>.log (the log lines of the script). Commit <out dir> as crates/nxb-rewrite/tests/data/region.
The modules have a text of nop words, one engine function with the reservation, a PLT stub and the tables a module needs
for its dynamic symbols and one jump slot relocation."""
import hashlib, os, shutil, struct, subprocess, sys

root, python, out = sys.argv[1], os.path.abspath(sys.argv[2]), os.path.abspath(sys.argv[3])
os.makedirs(out, exist_ok=True)

NOP, RET = 0xD503201F, 0xD65F03C0
STP_FRAME, LDP_FRAME = 0xA9BE7BFD, 0xA8C27BFD   # stp x29, x30, [sp, #-32]! and ldp x29, x30, [sp], #32
OPTION = b'SwitchVirtualAddressSpaceSize'
IMPORT = b'_ZN2nn2os21AllocateAddressRegionEPmm'
NINIT, ENGINE, STUB = 0x100, 0x200, 0x400
GOT, VAR = 0x2800, 0x2900            # the jump slot, and the region's base variable (its size is the word after it)

def adrp(rd, target, pc):
    imm = (target - (pc & ~0xFFF)) >> 12
    return 0x90000000 | (imm & 3) << 29 | ((imm >> 2) & 0x7FFFF) << 5 | rd
def add(rd, rn, imm): return 0x91000000 | imm << 10 | rn << 5 | rd
def movz(rd, imm16, hw): return 0xD2800000 | hw << 21 | imm16 << 5 | rd
def mov(rd, rm): return 0xAA0003E0 | rm << 16 | rd
def str_(rt, rn, off): return 0xF9000000 | off // 8 << 10 | rn << 5 | rt
def ldr(rt, rn, off): return 0xF9400000 | off // 8 << 10 | rn << 5 | rt
def b(frm, to): return 0x14000000 | ((to - frm) // 4) & 0x3FFFFFF
def bl(frm, to): return 0x94000000 | ((to - frm) // 4) & 0x3FFFFFF
def beq(frm, to): return 0x54000000 | (((to - frm) // 4) & 0x7FFFF) << 5

def pad(x): return x + bytes(-len(x) % 0x1000)

def module(code, sym=b'nninitStartup', option=OPTION, import_slot=True):
    """an uncompressed NSO0: text (0x1000), ro (0x1000) and data (0x1000) as laid out in the constants above.
    code: {address: [words]} in text"""
    text = bytearray(struct.pack('<I', NOP) * 0x400)
    struct.pack_into('<II', text, 0, 0x14000002, 0x1000)   # b #8, the offset of MOD0
    for at, words in code.items():
        for k, w in enumerate(words): struct.pack_into('<I', text, at + 4 * k, w)
    ro = bytearray(0x1000)
    ehs = 0x40
    struct.pack_into('<4sIIIIII', ro, 0, b'MOD0', 0x1000, 0, 0, ehs, 0, 0)   # the dynamic section is the start of data
    struct.pack_into('<BBBBII', ro, ehs, 1, 0x03, 0x03, 0x3B, 0, 1)         # eh_frame_hdr: one function table entry
    struct.pack_into('<ii', ro, ehs + 12, -ehs, 0)
    ro[0x100:0x100 + len(option) + 1] = option + b'\0'
    strtab = b'\0' + sym + b'\0' + IMPORT + b'\0'
    s0, s1 = 1, 2 + len(sym)
    symtab = bytes(24) + struct.pack('<IBBHQQ', s0, 0x12, 0, 1, NINIT, 4) + struct.pack('<IBBHQQ', s1, 0x12, 0, 0, 0, 0)
    ro[0x140:0x140 + len(symtab)] = symtab
    ro[0x1C0:0x1C0 + len(strtab)] = strtab
    struct.pack_into('<QQq', ro, 0x280, GOT, 2 << 32 | 1026, 0)                # R_AARCH64_JUMP_SLOT against the import
    struct.pack_into('<III', ro, 0x300, 1, 3, 0)                              # DT_HASH: nbucket 1, nchain 3
    data = bytearray(0x1000)
    tags = [(4, 0x1300), (2, 24 if import_slot else 0), (5, 0x11C0), (6, 0x1140), (20, 7), (23, 0x1280)]
    for k, (t, v) in enumerate(tags): struct.pack_into('<qQ', data, 16 * k, t, v)
    segs = [pad(bytes(text)), pad(bytes(ro)), pad(bytes(data))]
    bid = hashlib.sha256(bytes(text) + sym).digest()[:16]
    hdr = bytearray(0x100)
    hdr[0:4] = b'NSO0'
    struct.pack_into('<I', hdr, 0xC, 0x38)
    fo = 0x100
    for i, s in enumerate(segs):
        struct.pack_into('<IIII', hdr, 0x10 + i * 0x10, fo, i * 0x1000, len(s), 0)
        struct.pack_into('<I', hdr, 0x60 + i * 4, len(s))
        hdr[0xA0 + 0x20 * i:0xC0 + 0x20 * i] = hashlib.sha256(s).digest()
        fo += len(s)
    struct.pack_into('<I', hdr, 0x3C, 0x1000)
    hdr[0x40:0x50] = bid
    return bytes(hdr) + b''.join(segs)

def stub():
    # adrp x16, GOT page / ldr x17, [x16, #lo] / add x16, x16, #lo / br x17
    return {STUB: [adrp(16, GOT, STUB), ldr(17, 16, GOT & 0xFFF), add(16, 16, GOT & 0xFFF), 0xD61F0220]}

def engine(body, ninit=None):
    """the engine function at ENGINE: the frame, the base variable (adrp x19, add x0, x19), the body, the epilogue.
    A `bl` of the body is written as the string 'bl' and gets the stub as its target."""
    words = [STP_FRAME, adrp(19, VAR, ENGINE + 4), add(0, 19, VAR & 0xFFF)] if body[0] != 'bare' else [STP_FRAME]
    words += [w for w in body if w not in ('bare',)]
    at = ENGINE
    for k, w in enumerate(words):
        if w == 'bl': words[k] = bl(at + 4 * k, STUB)
        elif w == 'beq': words[k] = beq(at + 4 * k, at + 4 * k + 8)
    words += [LDP_FRAME, RET]
    code = {ENGINE: words}
    code[NINIT] = ninit or [b(NINIT, ENGINE)]
    code.update(stub())
    return code

C = lambda r, v: movz(r, v, 1)                  # mov xr, #(v << 16)
SZ = VAR - 0x2000 + 8                           # the size word, from the page of the data segment
cases = {
    'str-x1': engine([C(1, 0x3E00), str_(1, 19, SZ), 'bl']),
    'str-x8': engine([C(8, 0x4000), C(1, 0x4000), str_(8, 19, SZ), 'bl']),
    'x1-after': engine([C(8, 0x4000), str_(8, 19, SZ), C(1, 0x4000), 'bl']),
    'via-call': engine([C(1, 0x3E00), str_(1, 19, SZ), 'bl'], ninit=[bl(NINIT, ENGINE), RET]),
    'two-calls': engine([C(1, 0x3E00), str_(1, 19, SZ), 'bl', adrp(19, VAR, ENGINE + 0x14), add(0, 19, VAR & 0xFFF), C(1, 0x3E00), str_(1, 19, SZ), 'bl']),
    'x1-differs': engine([C(8, 0x4000), C(1, 0x2000), str_(8, 19, SZ), 'bl']),
    'x8-rewritten': engine([C(8, 0x4000), str_(8, 19, SZ), C(1, 0x4000), C(8, 0), 'bl']),
    'xzr': engine([C(1, 0x3E00), str_(31, 19, SZ), 'bl']),
    'wrong-word': engine([C(1, 0x3E00), str_(1, 19, SZ + 8), 'bl']),
    'branch-before': engine(['bare', adrp(19, VAR, ENGINE + 4), add(0, 19, VAR & 0xFFF), 'beq', NOP, C(1, 0x3E00), str_(1, 19, SZ), 'bl']),
}
variants = {
    'no-symbol': lambda: module(cases['str-x1'], sym=b'nninitStartux'),
    'no-option': lambda: module(cases['str-x1'], option=b'SwitchVirtualAddressSpaceSizX'),
    'no-import': lambda: module(cases['str-x1'], import_slot=False),
}
nsos = {k: module(v) for k, v in cases.items()}
nsos.update({k: f() for k, f in variants.items()})

expected = []
for name, data in sorted(nsos.items()):
    nso = os.path.join(out, name + '.nso')
    open(nso, 'wb').write(data)
    ref = os.path.join(out, 'ref-' + name)
    shutil.rmtree(ref, ignore_errors=True)
    os.makedirs(ref)
    r = subprocess.run([python, os.path.join(root, 'dm', 'tools', 'patch_nso.py'), nso, ref, 'main'], capture_output=True, text=True, env={**os.environ, 'DM_REWRITE_J': '1'})
    if r.returncode: sys.exit(f'{name}: patch_nso.py failed\n{r.stdout}{r.stderr}')
    open(os.path.join(out, name + '.log'), 'w').write(r.stdout)
    for f in sorted(os.listdir(ref)):
        if f in ('log.txt', 'main.build_src.sha256', 'build_src.sha256'): continue
        d = open(os.path.join(ref, f), 'rb').read()
        expected.append(f'{name}\t{f}\t{len(d)}\t{hashlib.sha256(d).hexdigest()}')
    shutil.rmtree(ref)
open(os.path.join(out, 'expected.tsv'), 'w').write('\n'.join(expected) + '\n')
print(len(nsos), 'modules')
