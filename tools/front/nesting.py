#!/usr/bin/env python3
"""Writes synthetic files with one construct nested 1..N deep, to compare where the prototype hits Python's recursion
limit (reason `recursion`) with the Rust port's emulation of it.

usage: nesting.py <out dir>     writes <out dir>/<pattern>_<n>.metal and <out dir>/list.txt"""
import os, sys

HEAD = '#include <metal_stdlib>\nusing namespace metal;\n'
PATTERNS = {
    # name: (max n, function of n -> source)
    'paren': (120, lambda n: HEAD + 'float f(float x) { return ' + '(' * n + 'x' + ')' * n + '; }\n'),
    'paren_num': (120, lambda n: HEAD + 'float f(float x) { return ' + '(' * n + '1.0' + ')' * n + '; }\n'),
    'neg': (200, lambda n: HEAD + 'float f(float x) { return ' + '- ' * n + 'x; }\n'),
    'not': (200, lambda n: HEAD + 'bool f(bool x) { return ' + '! ' * n + 'x; }\n'),
    'ternary': (500, lambda n: HEAD + 'float f(bool c, float x) { return ' + 'c ? x : ' * n + 'x; }\n'),
    'ternary_mid': (200, lambda n: HEAD + 'float f(bool c, float x) { return ' + 'c ? ' * n + 'x' + ' : x' * n + '; }\n'),
    'call': (120, lambda n: HEAD + 'float f(float x) { return ' + 'abs(' * n + 'x' + ')' * n + '; }\n'),
    'ctor': (120, lambda n: HEAD + 'float2 f(float x) { return ' + 'float2(' * n + 'x' + ')' * n + '; }\n'),
    'index': (120, lambda n: HEAD + 'int f(thread int* a) { return ' + 'a[' * n + '0' + ']' * n + '; }\n'),
    'assign': (500, lambda n: HEAD + 'float f(float x) { ' + 'x = ' * n + '1.0; return x; }\n'),
    'block': (600, lambda n: HEAD + 'void f() { ' + '{ ' * n + '}' * n + ' }\n'),
    'if': (400, lambda n: HEAD + 'void f(bool c) { ' + 'if (c) ' * n + 'return; }\n'),
    'if_block': (400, lambda n: HEAD + 'void f(bool c) { ' + 'if (c) { ' * n + '}' * n + ' }\n'),
    'while': (400, lambda n: HEAD + 'void f(bool c) { ' + 'while (c) ' * n + 'break; }\n'),
    'cast': (150, lambda n: HEAD + 'float f(float x) { return ' + '(float)' * n + 'x; }\n'),
    'member': (120, lambda n: HEAD + 'struct S { float a; };\nfloat f(S s) { return ' + 'S{' * n + 's.a' + '}' * 0 + '; }\n'),
    'mul': (120, lambda n: HEAD + 'float f(float x) { return ' + '(x * ' * n + 'x' + ')' * n + '; }\n'),
    'cmp_or': (120, lambda n: HEAD + 'bool f(float x) { return ' + '(x < x || ' * n + 'true' + ')' * n + '; }\n'),
    'init_list': (120, lambda n: HEAD + 'constant float g[1] = { ' + '(' * n + '1.0' + ')' * n + ' };\n'),
}

if __name__ == '__main__':
    out = sys.argv[1]; os.makedirs(out, exist_ok=True)
    names = []
    for name, (mx, gen) in PATTERNS.items():
        for n in range(1, mx + 1):
            p = os.path.join(out, f'{name}_{n}.metal')
            with open(p, 'w') as f: f.write(gen(n))
            names.append(p)
    open(os.path.join(out, 'list.txt'), 'w').write('\n'.join(names) + '\n')
    print(len(names), 'files')
