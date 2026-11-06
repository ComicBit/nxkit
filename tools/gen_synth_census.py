#!/usr/bin/env python3
"""Synthetic modules for the rewrite-site census (development tool, not part of a build).

Usage: gen_synth_census.py <engine folder holding dm/> <python with capstone 5> <out dir>
Writes <out dir>/<case>.nso, runs the reference script `dm/tools/insn_census.py <case>.nso` in <out dir> and keeps its
output as <out dir>/<case>.txt. The test `crates/nxb-rewrite/tests/census.rs` compares it with the Rust census.
The sites file (`--sites`) is not ported: it prints the operands of Advanced SIMD instructions, which the decoder does not model. Commit <out dir> as
crates/nxb-rewrite/tests/data/census.
The modules have a text of 0x4000 bytes with one function from the .eh_frame table that covers the code. `sites` holds one
of each kind of site and a data block with the same words. `plain` has the code and some undecodable words in the
function. `nothing` has nop words only."""
import hashlib, os, struct, subprocess, sys

root, python, out = sys.argv[1], os.path.abspath(sys.argv[2]), os.path.abspath(sys.argv[3])
os.makedirs(out, exist_ok=True)

NOP, RET, BAD = 0xD503201F, 0xD65F03C0, 0xFFFFFFFF
TEXT = 0x4000
CIE, FDE = 0x100, 0x200       # in ro

def svc(n): return 0xD4000001 | n << 5
def mrs(rt, reg): return 0xD5300000 | reg << 5 | rt
def msr(reg, rt): return 0xD5100000 | reg << 5 | rt
TPIDR, TPIDRRO, CNTFRQ, CNTPCT, CNTVCT = 0xBD040 >> 5, 0xBD060 >> 5, 0xBE000 >> 5, 0xBE020 >> 5, 0xBE040 >> 5

SITES = [
    svc(0x1D), svc(5), svc(0x1D), svc(0x7B),
    mrs(0, TPIDR), msr(TPIDR, 1), mrs(2, TPIDRRO), mrs(3, CNTPCT), mrs(4, CNTVCT), mrs(5, CNTFRQ), mrs(6, TPIDR), mrs(7, 0xBE0A0 >> 5), msr(0xBE0C0 >> 5, 8),
    0xF9400012,   # ldr x18, [x0]
    0x2A0103F2,   # mov w18, w1
    0x91006240,   # add x0, x18, #0x18
    0xD2800300,   # mov x0, #0x18
    0x58001012,   # ldr x18, #0x1000 (a literal)
    0x3D800240,   # str q0, [x18]
    0xCB2963FF,   # sub sp, sp, x9
    0xCB2969FF,   # sub sp, sp, x9, lsl #2
    0x910043FF,   # add sp, sp, #0x10
    0xAA0103E0,   # mov x0, x1
    RET,
]

def module(words, data_words=()):
    """an uncompressed NSO0: text (TEXT), ro and data. words: the code from 0x100, data_words: the words of 0x2000.. (the data block)"""
    text = bytearray(struct.pack('<I', NOP) * (TEXT // 4))
    struct.pack_into('<II', text, 0, 0x14000002, TEXT)   # b #8, the offset of MOD0
    for k, w in enumerate(words): struct.pack_into('<I', text, 0x100 + 4 * k, w)
    for k in range(0x2000 // 4 if data_words else 0): struct.pack_into('<I', text, 0x2000 + 4 * k, data_words[k % len(data_words)])
    ro_at = TEXT
    ro = bytearray(0x1000)
    ehs = 0x40
    fde_end = 0x2000 if data_words else TEXT
    struct.pack_into('<4sIIIIII', ro, 0, b'MOD0', 0x1000, 0, 0, ehs, ehs + 12 + 8, 0)
    struct.pack_into('<BBBBII', ro, ehs, 1, 0x03, 0x03, 0x3B, 0, 1)
    struct.pack_into('<ii', ro, ehs + 12, 0x100 - (ro_at + ehs), ro_at + FDE - (ro_at + ehs))
    # a CIE with the augmentation "zR" (pointer encoding pcrel sdata4) and one FDE for [0x100, fde_end)
    struct.pack_into('<IIB3sBBBBB', ro, CIE, 16, 0, 1, b'zR\0', 1, 0x7C, 30, 1, 0x1B)
    struct.pack_into('<IIii', ro, FDE, 16, (ro_at + FDE + 4) - (ro_at + CIE), 0x100 - (ro_at + FDE + 8), fde_end - 0x100)
    ro[0x280:0x280 + 8] = b'\0' * 8
    data = bytearray(0x1000)
    segs = [bytes(text), bytes(ro), bytes(data)]
    bid = hashlib.sha256(bytes(text)).digest()[:16]
    hdr = bytearray(0x100)
    hdr[0:4] = b'NSO0'
    struct.pack_into('<I', hdr, 0xC, 0x38)
    fo = 0x100
    off = 0
    for i, s in enumerate(segs):
        struct.pack_into('<IIII', hdr, 0x10 + i * 0x10, fo, off, len(s), 0)
        struct.pack_into('<I', hdr, 0x60 + i * 4, len(s))
        hdr[0xA0 + 0x20 * i:0xC0 + 0x20 * i] = hashlib.sha256(s).digest()
        fo += len(s)
        off += len(s)
    struct.pack_into('<I', hdr, 0x3C, 0x1000)
    hdr[0x40:0x50] = bid
    return bytes(hdr) + b''.join(segs)

DATA = [BAD, BAD, 0xF9400012, BAD, svc(7), BAD, BAD, BAD]
cases = {
    'sites': module(SITES, DATA),
    'plain': module(SITES[:10] + [BAD, BAD, 0x91006240, BAD] + SITES[10:]),
    'nothing': module([RET]),
}
for name, data in sorted(cases.items()):
    open(os.path.join(out, name + '.nso'), 'wb').write(data)
    r = subprocess.run([python, os.path.join(root, 'dm', 'tools', 'insn_census.py'), name + '.nso'],
                       capture_output=True, text=True, cwd=out)
    if r.returncode: sys.exit(f'{name}: insn_census.py failed\n{r.stdout}{r.stderr}')
    open(os.path.join(out, name + '.txt'), 'w').write(r.stdout)
print(len(cases), 'modules')
