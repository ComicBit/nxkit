#!/usr/bin/env python3
"""Synthetic main modules for the finder `ue_ring` (development tool, not part of a build).

Usage: gen_synth_ring.py <engine folder holding dm/> <python with capstone 5> <out dir>
Writes <out dir>/<case>.nso for small Unreal-like modules that hold the chain the finder follows (the name of
`nvn.NumBufferedFrames`, its registration, the lock's rotation, the constructor's layout and the load of the copy count)
in the forms of Unreal Engine 4.26 and 4.27 (and the forms it refuses), runs the reference script
`dm/tools/patch_nso.py` on each, and writes the expected results for the test `crates/nxb-rewrite/tests/ring.rs`:
<out dir>/expected.tsv (case, output file, length, SHA-256) and <out dir>/<case>.log (the log lines of the script).
Commit <out dir> as crates/nxb-rewrite/tests/data/ring.
The modules have a text of nop words with the chain, and the tables a module needs for its dynamic symbols."""
import hashlib, os, shutil, struct, subprocess, sys

root, python, out = sys.argv[1], os.path.abspath(sys.argv[2]), os.path.abspath(sys.argv[3])
os.makedirs(out, exist_ok=True)

NOP = 0xD503201F
TEXT, RO, DATA = 0x4000, 0x4000, 0x5000        # the segments (text size, ro start, data start)
NAME, VAR, GOT = RO + 0x400, DATA + 0x218, DATA + 0x800
REG, LOCK, CTOR, LOAD = 0x1000, 0x2000, 0x3000, 0x3040
RELA = RO + 0x380
OPTION = b'nvn.NumBufferedFrames'
NINIT = 0x100
STUR_Q18, LOC_XZR = 0x3C818000, 0xF8028C1F | 24 << 5     # stur q0, [x0, #0x18] and str xzr, [x24, #0x28]!
LDR_W8_X8, STR_W8_X20_5C, TBZ_W21_1 = 0xB9400108, 0xB9005E88, 0x36080015 | 3 << 5
# ldr w8, [x21, #0x5c]; cmp w8, #2; b.lo; ldr w9, [x21, #0x60]; add w9, w9, #1; udiv w10, w9, w8;
# msub w8, w10, w8, w9; str w8, [x21, #0x60]; add x8, x21, w8, uxtw #3; ldr x8, [x8, #8]
ROTATION = [0xB9405EA8, 0x7100091F, 0x54000103, 0xB94062A9, 0x11000529, 0x1AC8092A, 0x1B08A548, 0xB90062A8, 0x8B284EA8, 0xF9400508]

def adrp(rd, pc, target):
    imm = ((target & ~0xFFF) - (pc & ~0xFFF)) >> 12
    return 0x90000000 | (imm & 3) << 29 | ((imm >> 2) & 0x7FFFF) << 5 | rd
def addi(rd, rn, imm): return 0x91000000 | imm << 10 | rn << 5 | rd
def ldr_w(rt, rn, addr): return 0xB9400000 | ((addr & 0xFFF) // 4) << 10 | rn << 5 | rt
def ldr_x(rt, rn, addr): return 0xF9400000 | ((addr & 0xFFF) // 8) << 10 | rn << 5 | rt

def pad(x): return x + bytes(-len(x) % 0x1000)

def chain(registered=True, rotation=True, layout=True):
    code = {}
    if registered:
        code[REG] = [adrp(1, REG, NAME), addi(1, 1, NAME & 0xFFF), adrp(2, REG + 8, VAR), addi(2, 2, VAR & 0xFFF)]
    if rotation: code[LOCK] = ROTATION
    if layout: code[CTOR] = [STUR_Q18, LOC_XZR]
    return code

def module(code, value=3, name=True, got=False):
    """an uncompressed NSO0: text (TEXT), ro (0x1000) and data (0x1000). code: {address: [words]} in text"""
    sym = b'nninitStartup'
    text = bytearray(struct.pack('<I', NOP) * (TEXT // 4))
    struct.pack_into('<II', text, 0, 0x14000002, RO)   # b #8, the offset of MOD0
    for at, words in code.items():
        for k, w in enumerate(words): struct.pack_into('<I', text, at + 4 * k, w)
    ro = bytearray(0x1000)
    ehs = 0x40
    struct.pack_into('<4sIIIIII', ro, 0, b'MOD0', 0x1000, 0, 0, ehs, 0, 0)   # the dynamic section is the start of data
    struct.pack_into('<BBBBII', ro, ehs, 1, 0x03, 0x03, 0x3B, 0, 1)         # eh_frame_hdr: one function table entry
    struct.pack_into('<ii', ro, ehs + 12, -ehs, 0)
    if name: ro[0x400:0x400 + 2 * len(OPTION) + 2] = (OPTION.decode() + '\0').encode('utf-16le')
    strtab = b'\0' + sym + b'\0'
    symtab = bytes(24) + struct.pack('<IBBHQQ', 1, 0x12, 0, 1, NINIT, 4)
    ro[0x140:0x140 + len(symtab)] = symtab
    ro[0x1C0:0x1C0 + len(strtab)] = strtab
    struct.pack_into('<III', ro, 0x300, 1, 2, 0)                              # DT_HASH: nbucket 1, nchain 2
    if got: struct.pack_into('<QQq', ro, 0x380, GOT, 1027, VAR)               # R_AARCH64_RELATIVE: the GOT slot holds VAR
    data = bytearray(0x1000)
    tags = [(4, RO + 0x300), (5, RO + 0x1C0), (6, RO + 0x140), (10, len(strtab)), (11, 24)]
    if got: tags += [(7, RELA), (8, 24)]
    for k, (t, v) in enumerate(tags): struct.pack_into('<qQ', data, 16 * k, t, v)
    struct.pack_into('<I', data, VAR - DATA, value)
    segs = [pad(bytes(text)), pad(bytes(ro)), pad(bytes(data))]
    bid = hashlib.sha256(bytes(text) + sym).digest()[:16]
    hdr = bytearray(0x100)
    hdr[0:4] = b'NSO0'
    struct.pack_into('<I', hdr, 0xC, 0x38)
    fo, mo = 0x100, 0
    for i, s in enumerate(segs):
        struct.pack_into('<IIII', hdr, 0x10 + i * 0x10, fo, mo, len(s), 0)
        struct.pack_into('<I', hdr, 0x60 + i * 4, len(s))
        hdr[0xA0 + 0x20 * i:0xC0 + 0x20 * i] = hashlib.sha256(s).digest()
        fo += len(s); mo += len(s)
    struct.pack_into('<I', hdr, 0x3C, 0x1000)
    hdr[0x40:0x50] = bid
    return bytes(hdr) + b''.join(segs)

def direct(rd=8):          # Unreal Engine 4.27
    return {LOAD: [TBZ_W21_1, adrp(rd, LOAD + 4, VAR), ldr_w(rd, rd, VAR), STR_W8_X20_5C]}

def with_chain(extra, **kw):
    code = chain(**kw)
    code.update(extra)
    return code

cases = {
    'direct': lambda: module(with_chain(direct())),
    'got': lambda: module(with_chain({LOAD: [TBZ_W21_1, adrp(8, LOAD + 4, GOT), ldr_x(8, 8, GOT), LDR_W8_X8, STR_W8_X20_5C]}), got=True),
    'base-earlier': lambda: module(with_chain({CTOR + 8: [0xF805CC1F | 8 << 5],
                                              LOAD: [TBZ_W21_1, adrp(10, LOAD + 4, VAR), ldr_w(10, 10, VAR), 0xB900010A]})),
    'base-not-at-count': lambda: module(with_chain({LOAD: [TBZ_W21_1, adrp(10, LOAD + 4, VAR), ldr_w(10, 10, VAR), 0xB900010A]})),
    'no-name': lambda: module(with_chain(direct()), name=False),
    'value-2': lambda: module(with_chain(direct()), value=2),
    'not-registered': lambda: module(with_chain(direct(), registered=False)),
    'no-rotation': lambda: module(with_chain(direct(), rotation=False)),
    'no-layout': lambda: module(with_chain(direct(), layout=False)),
    'two-loads': lambda: module(with_chain({**direct(), CTOR + 0x80: [TBZ_W21_1, adrp(8, CTOR + 0x84, VAR), ldr_w(8, 8, VAR), STR_W8_X20_5C]})),
    'other-variable': lambda: module(with_chain({LOAD: [TBZ_W21_1, adrp(8, LOAD + 4, VAR + 8), ldr_w(8, 8, VAR + 8), STR_W8_X20_5C]})),
}

expected = []
for name, make in sorted(cases.items()):
    data = make()
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
print(len(cases), 'modules')
