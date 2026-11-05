#!/usr/bin/env python3
"""Differential fuzzing input generator: token-level random edits of real shaders, to compare the Python prototype and the
Rust port on inputs far outside the engine's normal output (odd tokens, cut-off files, swapped operators ...).

usage: fuzz.py <file list> <n> <out dir> [seed]
Writes <out dir>/<i>.metal and <out dir>/list.txt. Only files below 12 KB are used, to keep the Python side cheap.
Run tools/py_check.py and mslcheck --json --stats on list.txt, then tools/compare_results.py."""
import os, re, sys, random

VOCAB = ('{ } ( ) [ ] [[ ]] ; , . -> :: ? : = += -= *= <<= >>= &= |= ^= == != < > <= >= << >> + - * / % & | ^ ~ ! ++ -- # '
         'if else switch case default do while for break continue return discard_fragment struct template typename using '
         'namespace metal constant device thread threadgroup const static inline __attribute__ always_inline vertex fragment '
         'kernel void bool char uchar short ushort int uint long ulong half float size_t float2 float3 float4 half4 int2 '
         'uint4 bool2 packed_float3 packed_float float4x4 half2x2 sampler texture2d texture2d_array texturecube depth2d '
         'depth2d_array texture3d texture2d_ms access read write sample sample_compare gather read get_width level bias '
         'gradient2d component x y z w xy xyzw rgba q true false as_type static_cast reinterpret_cast fma abs min max clamp '
         'select isnan all dot length cross unpack_unorm4x8_to_float extract_bits insert_bits precise fast spvUnsafeArray '
         'dm_bs spvDepthCast spvFAdd main0 _1 _2 _999 S out in 0 1 7u 0x10 0xFFFFFFFF 3000000000 1.5 1.0h 2.5f 1e3 1f 1lu 1ul '
         '.5 1. "s" /*x*/ //c').split()
TOKEN = re.compile(r'\[\[|\]\]|->|::|<<=|>>=|[-+*/%&|^!=<>]=|&&|\|\||<<|>>|\+\+|--|\w+|[^\s\w]')


def mutate(src, rnd):
    for _ in range(rnd.choice((1, 1, 1, 2, 3))):
        toks = list(TOKEN.finditer(src))
        if not toks: break
        m = rnd.choice(toks)
        op = rnd.random()
        if op < 0.25:   # delete
            src = src[:m.start()] + src[m.end():]
        elif op < 0.55:   # replace with a vocabulary token
            src = src[:m.start()] + rnd.choice(VOCAB) + src[m.end():]
        elif op < 0.7:   # insert before
            src = src[:m.start()] + rnd.choice(VOCAB) + ' ' + src[m.start():]
        elif op < 0.8:   # duplicate
            src = src[:m.end()] + ' ' + m.group() + src[m.end():]
        elif op < 0.9:   # replace with another token of the same file
            src = src[:m.start()] + rnd.choice(toks).group() + src[m.end():]
        elif op < 0.95:   # cut the file
            src = src[:m.start()]
        else:   # swap with the next token
            i = toks.index(m)
            if i + 1 < len(toks):
                n = toks[i + 1]
                src = src[:m.start()] + n.group() + src[m.end():n.start()] + m.group() + src[n.end():]
    return src


if __name__ == '__main__':
    files = [l.strip() for l in open(sys.argv[1]) if l.strip()]
    small = [f for f in files if os.path.getsize(f) < 12000]
    n, out = int(sys.argv[2]), sys.argv[3]
    rnd = random.Random(int(sys.argv[4]) if len(sys.argv) > 4 else 1)
    os.makedirs(out, exist_ok=True)
    names = []
    for i in range(n):
        src = open(rnd.choice(small)).read()
        p = os.path.join(out, f'{i}.metal')
        with open(p, 'w', newline='\n') as f: f.write(mutate(src, rnd))
        names.append(p)
    open(os.path.join(out, 'list.txt'), 'w').write('\n'.join(names) + '\n')
    print(len(small), 'small inputs,', n, 'mutants')
