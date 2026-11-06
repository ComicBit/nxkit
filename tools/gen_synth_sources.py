#!/usr/bin/env python3
"""A synthetic RomFS for the source checks of `nxb build` (development tool, not part of a build).

Usage: gen_synth_sources.py <engine folder holding dm/> <python> <out dir>
Writes <out dir>/romfs/ with small Unreal paks, IoStore TOCs, PKZ, GFPAK and Unreal 3 files, Trinity packs, SPIR-V and GLSL
files and a Frostbite layout.toc. Then runs `profile_guess.build_sources` of the reference script on it, with the module
reader and `shader_sources` replaced so that only the counts that the RomFS decides are kept. The result is
<out dir>/expected.txt: one line `<case> <count name> <value>`. Case `vk_egl` has a main that imports Vulkan, EGL and
glslcCompile, case `plain` one that imports none of them. The test `crates/nxb-build/tests/sources.rs`
compares it with the Rust checks. Commit <out dir> as crates/nxb-build/tests/data/sources."""
import os, struct, sys

engine, out = sys.argv[1], os.path.abspath(sys.argv[3])
root = os.path.join(out, 'romfs')
MAGIC = 0x5A6F12E1

def put(rel, data):
    p = os.path.join(root, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, 'wb') as f: f.write(data)

def index(names):
    """A version 11 index: mount point, count, seed, no path hash index, a directory index that holds `names`"""
    idx = struct.pack('<i', 1) + b'\0' + bytes(12) + struct.pack('<ii', 0, 1)
    return idx, names

def pak11(names, enc=0, back=0xc8):
    b = bytearray(16)
    idx, d = index(names)
    ioff = len(b); doff = ioff + len(idx) + 16
    idx += struct.pack('<QQ', doff, len(d))
    b += idx + d
    ft = bytearray(21); ft[20] = enc
    ft += struct.pack('<IIQQ', MAGIC, 11, ioff, len(idx)) + bytes(20)
    ft += bytes(back + 21 - len(ft))
    return bytes(b + ft)

def pak_legacy(ver):
    b = bytearray(64)
    if ver == 4:
        ft = bytearray(1) + struct.pack('<II', MAGIC, 4) + struct.pack('<QQ', 0, 16) + bytes(20)
    else:
        ft = bytearray(21) + struct.pack('<IIQQ', MAGIC, 8, 0, 16) + bytes(20) + bytes(0xc8 - 44)
    return bytes(b + ft)

def toc(version, types):
    b = bytearray(144)
    b[:16] = b'-==--==--==--==-'; b[16] = version
    struct.pack_into('<9I', b, 20, 144, len(types), 0, 12, 0, 0, 0, 0, 1)
    for t in types: b += bytes(11) + bytes([t])
    return bytes(b)

GSC = b'GlobalShaderCache-SF_METAL.bin\0'
put('p/archives.pak', pak11(b'x/.ushaderbytecode\0'))
put('p/cooked.pak', pak11(b'x/m.uexp\0'))
put('p/encrypted.pak', pak11(b'x/m.uexp\0', enc=1))
put('p/v4.pak', pak_legacy(4))
put('p/v8.pak', pak_legacy(8))
put('p/short.pak', bytes(10))
put('p/garbage.pak', bytes(range(256)) * 2)
put('p/no_dir_index.pak', pak11(b'x/y.bin\0'))
put('ioa/g.pak', pak11(GSC)); put('ioa/g.utoc', toc(4, [1, 3]))
put('iob/g.pak', pak11(GSC)); put('iob/g.utoc', toc(4, [1, 8]))
put('ioc/g.pak', pak11(GSC))
put('iod/g.pak', pak11(GSC)); put('iod/g.utoc', toc(3, [2, 8]))
put('ioe/g.pak', pak11(GSC)); put('ioe/g.utoc', toc(4, [3]))
put('iof/g.pak', pak11(GSC)); put('iof/sub/h.utoc', toc(4, [1]))
put('iof/g2.pak', pak11(GSC)); put('iof/bad.utoc', b'not a toc')

pkz = lambda name: b'pkzl' + bytes(12) + struct.pack('<III', 2, 0x40, len(name)) + bytes(0x40 - 0x1c) + bytes(0x40) + name
put('k/oodle.pkz', pkz(b'OodleKraken\0'))
put('k/zstd.pkz', pkz(b'Zstd\0\0\0\0\0\0\0\0'))
put('k/other.pkz', b'abcd' + bytes(60))

def gfpak(comp):
    return b'GFLXPACK' + bytes(8) + struct.pack('<IIQ', 2, 0, 0x20) + (bytes(2) + struct.pack('<H', 1) + bytes(20)) + (bytes(2) + struct.pack('<H', comp) + bytes(20))
put('k/oodle.gfpak', gfpak(3)); put('k/lz4.gfpak', gfpak(1))

TAG = bytes.fromhex('c1832a9e')
def ue3(block):
    h = TAG + struct.pack('<H', 500) + bytes(0x20 - 6) + TAG + struct.pack('<III', 0x20000, 0x40000, 0x40000)
    return h + bytes(8 * 2) + block + bytes(16)
put('u/oodle.bin', ue3(b'\x8c')); put('u/zlib.bin', ue3(b'\x01'))
put('u/tag_only.bin', TAG + struct.pack('<H', 0x9000) + bytes(40))

put('t/a.trpfs', b'ONEPACK\0' + bytes(8)); put('t/b.trpfs', b'ONEPACK!' + bytes(8)); put('t/c.bin', b'ONEPACK\0')
put('s/a.spv', bytes.fromhex('03022307') + bytes(8)); put('s/b.spv', bytes.fromhex('03022308')); put('s/c.bin', bytes.fromhex('0302'))
put('g/ok.frag', b'#version 450\nvoid main() {}\n'); put('g/ok.VERT', b'void main() {}'); put('g/bin.vert', bytes([0, 1, 2, 3]))
put('g/empty.frag', b''); put('g/x.mvgl', b'abc'); put('g/y.glsl', b'abc')
put('Data/layout.toc', bytes.fromhex('00d1ce01') + bytes(8))
put('Other/Data/layout.toc', bytes.fromhex('00d1ce01'))

sys.path.insert(0, os.path.join(engine, 'dm/games')); sys.path.insert(0, os.path.join(engine, 'dm/tools')); sys.path.insert(0, os.path.join(engine, 'dm/gfx/tools'))
import profile_guess as pg
from pathlib import Path
captured = {}
pg.nso = lambda p: (b'',)
pg.glsl_sources = lambda m: 0
pg.shader_sources = lambda *a, **k: (a, k)
lines = []
for case, imports in (('vk_egl', ['vkCreateInstance', 'eglCreateWindowSurface', 'glslcCompile']), ('plain', ['nvnBootstrapLoader'])):
    pg.nso_syms.dynsyms = lambda b, imports=imports: [(s, False, 0) for s in imports]
    a, k = pg.build_sources(Path(out), Path(root), ['main'])
    names = ('imports', 'glsl_src', 'glslc_mods', 'spirv', 'unreal_paks', 'trinity', 'legacy_paks', 'gl_names', 'gl_src', 'gl_files', 'oodle_pkz', 'nro_imports')
    for n, v in list(zip(names, a)) + list(k.items()):
        if n in ('imports', 'glslc_mods', 'nro_imports', 'glsl_src', 'gl_names', 'gl_src'): continue
        lines.append(f'{case} {n} {int(v)}')
with open(os.path.join(out, 'expected.txt'), 'w') as f: f.write('\n'.join(lines) + '\n')
print(open(os.path.join(out, 'expected.txt')).read())
