#!/usr/bin/env python3
"""Token dump of the prototype lexer (msl_lex.py), for the byte-by-byte comparison with the Rust lexer.

usage: lex_dump.py <file list> <out dir>
Writes <out dir>/<n>.tok for the n-th file of the list: one line per token, `kind<TAB>line<TAB>text`, with backslash,
newline and tab in the text escaped. A file the lexer rejects gets the single line `LEXERROR<TAB><message>`.
The file is read the way msl_parse.py reads it: UTF-8 with errors='replace', universal newlines."""
import os, sys
PROTO = os.environ.get('MSL_PROTO', '')
sys.path.insert(0, PROTO)
from msl_lex import tokens, LexError


def esc(s):
    return s.replace('\\', '\\\\').replace('\n', '\\n').replace('\t', '\\t')


def dump(path):
    src = open(path, encoding='utf-8', errors='replace').read()
    try:
        toks = tokens(src)
    except LexError as e:
        return f'LEXERROR\t{e}\n'
    return ''.join(f'{k}\t{line}\t{esc(t)}\n' for k, t, line in toks)


if __name__ == '__main__':
    files = [l.strip() for l in open(sys.argv[1]) if l.strip()]
    out = sys.argv[2]
    os.makedirs(out, exist_ok=True)
    for n, f in enumerate(files):
        with open(os.path.join(out, f'{n}.tok'), 'w', encoding='utf-8', newline='\n') as fh:
            fh.write(dump(f))
