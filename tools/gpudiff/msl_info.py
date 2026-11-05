#!/usr/bin/env python3
"""Source facts the Metal reflection does not give, read from the engine's machine-generated MSL (spirv-cross style):
entry signatures, stage_in and output structs (types, user() locations, interpolation), colour inputs, depth output,
function constant declarations and the integer values the code compares them with, sampler pairing, and a backward
data-flow slice that finds the buffer words, varyings and textures feeding loop conditions, dynamic array indices and
switch selectors. Those inputs get small integer values so that loops end quickly and indices stay in bounds, the
rest gets the full random mixture (NaN, Inf, denormals, signed zeros, extremes).

usage (debug): msl_info.py <file.metal>   prints the facts as JSON
"""
import re, sys, json, collections

ID = re.compile(r'\b_\d+\b')


def split_top(s, sep=','):
    out, depth, cur, i = [], 0, [], 0
    while i < len(s):
        c = s[i]
        if s.startswith('[[', i): depth += 1; cur.append('[['); i += 2; continue
        if s.startswith(']]', i) and depth > 0: depth -= 1; cur.append(']]'); i += 2; continue
        if c in '([{<': depth += 1
        elif c in ')]}>': depth = max(0, depth - 1)
        if c == sep and depth == 0:
            out.append(''.join(cur).strip()); cur = []
        else:
            cur.append(c)
        i += 1
    if ''.join(cur).strip(): out.append(''.join(cur).strip())
    return out


def parse_decl(d):
    """'float4 m_38 [[color(0), function_constant(OM_0_0)]]' -> (type, name, array, attrs list)"""
    attrs = []
    m = re.search(r'\[\[(.*)\]\]\s*$', d)
    if m:
        attrs = [a.strip() for a in split_top(m.group(1))]
        d = d[:m.start()].strip()
    arr = None
    m = re.search(r'\[(\d+)\]\s*$', d)
    if m: arr = int(m.group(1)); d = d[:m.start()].strip()
    m = re.match(r'(.*?)([A-Za-z_]\w*)$', d.strip())
    if not m: return None
    return m.group(1).strip(), m.group(2), arr, attrs


def strip_comments(src):
    src = re.sub(r'/\*.*?\*/', ' ', src, flags=re.S)
    return re.sub(r'//[^\n]*', '', src)


def parse_structs(src):
    structs = {}
    for m in re.finditer(r'\bstruct\s+(\w+)\s*\{', src):
        i = m.end(); depth = 1
        while i < len(src) and depth:
            if src[i] == '{': depth += 1
            elif src[i] == '}': depth -= 1
            i += 1
        body = src[m.end():i - 1]
        if '(' in body and ')' in body and '{' in body: body = body  # struct with methods (spvUnsafeArray): members still parse
        mem = []
        for part in body.split(';'):
            part = part.strip()
            if not part or '(' in part.split('[[')[0] or part.startswith(('template', 'static', 'thread', 'constexpr', '#')): continue
            for decl in [part] if ',' not in part.split('[[')[0] else [part]:
                p = parse_decl(decl)
                if p and p[0]: mem.append({'type': p[0], 'name': p[1], 'array': p[2], 'attrs': p[3]})
        structs[m.group(1)] = mem
    return structs


ENTRY = re.compile(r'^(?P<pre>\[\[[^\n]*?\]\]\s*)?(?P<kind>vertex|fragment|kernel)\s+(?P<ret>[\w:<>, ]+?)\s+(?P<name>\w+)\((?P<params>[^\n]*)\)\s*$', re.M)


def parse_entries(src):
    out = []
    for m in ENTRY.finditer(src):
        params = []
        for p in split_top(m.group('params')):
            d = parse_decl(p)
            if d: params.append({'type': d[0], 'name': d[1], 'attrs': d[3]})
        out.append({'kind': m.group('kind'), 'ret': m.group('ret').strip(), 'name': m.group('name'), 'params': params,
                    'early_fragment_tests': 'early_fragment_tests' in (m.group('pre') or '')})
    return out


def parse_fconsts(src):
    fc = []
    for m in re.finditer(r'^constant\s+(\w+)\s+(\w+)\s*\[\[function_constant\((\d+)\)\]\]\s*;', src, re.M):
        name = m.group(2)
        vals = sorted({int(v) for v in re.findall(r'\b' + re.escape(name) + r'\s*[=!]=\s*(-?\d+)', src)})
        fc.append({'type': m.group(1), 'name': name, 'index': int(m.group(3)), 'compared': vals})
    return fc


def attr_val(attrs, key):
    for a in attrs:
        m = re.match(re.escape(key) + r'\s*\(\s*([^)]*)\)', a)
        if m: return m.group(1).strip()
        if a == key: return ''
    return None


# ---------------------------------------------------------------------------------------------------
# data-flow slice

def functions(src):
    """(name, params text, body) of every function definition at brace depth 0"""
    out = []
    depth = 0; i = 0; n = len(src)
    while i < n:
        c = src[i]
        if c == '{':
            if depth == 0:
                head = src[max(0, i - 600):i]
                m = re.search(r'(\w+)\s*\(([^;{}]*)\)\s*(?:const\s*)?$', head)
                start = i; d = 1; j = i + 1
                while j < n and d:
                    if src[j] == '{': d += 1
                    elif src[j] == '}': d -= 1
                    j += 1
                if m and not re.search(r'\bstruct\s+\w+\s*$', head.strip()):
                    out.append((m.group(1), m.group(2), src[start + 1:j - 1]))
                i = j; continue
            depth += 1
        elif c == '}': depth -= 1
        i += 1
    return out


def scan_path(s, i):
    """from s[i] (just after a base), read .member, ->member, [index] steps. Returns (tokens, index exprs, end)"""
    toks, idx = [], []
    while i < len(s):
        if s.startswith('->', i) or s[i] == '.':
            j = i + (2 if s[i] == '-' else 1)
            m = re.match(r'\w+', s[j:])
            if not m: break
            k = j + m.end()
            if k < len(s) and s[k] == '(': break                    # a method call (texture sample)
            toks.append('.' + m.group(0)); i = k
        elif s[i] == '[':
            d = 1; j = i + 1
            while j < len(s) and d:
                if s[j] == '[': d += 1
                elif s[j] == ']': d -= 1
                j += 1
            e = s[i + 1:j - 1].strip()
            lit = re.fullmatch(r'(?:int|uint)?\(?\s*(\d+)[uU]?\s*\)?', e)
            toks.append('[%s]' % (lit.group(1) if lit else '*'))
            if not lit: idx.append(e)
            i = j
        else: break
    return toks, idx, i


class Flow:
    def __init__(self, src, buffer_names, arg_names, tex_names):
        self.src = src
        self.deps = collections.defaultdict(set)      # var -> vars / 'L<n>' loads / 'IN:<member>' stage inputs / 'T:<tex>'
        self.loads = []                                # (base, path tokens)
        self.roots_loop, self.roots_idx = set(), set()
        self.asint = set()                             # vars reinterpreted with as_type<int/uint>
        self.asint_loads = set()                       # loads read directly through as_type<int/uint>
        self.buffer_names, self.arg_names, self.tex_names = set(buffer_names), set(arg_names), set(tex_names)
        self.alias = {}
        self.fns = functions(src)
        self.fnames = {f[0] for f in self.fns}
        self.params = {f[0]: [parse_decl(p)[1] if parse_decl(p) else '' for p in split_top(f[1])] for f in self.fns}
        self._aliases()
        for name, _, body in self.fns: self._body(name, body)

    def _aliases(self):
        # helper parameters that are a buffer passed by reference: support_buffer, s (dm_alpha_state), l, L
        for name, _, body in self.fns:
            for fn in self.fnames:
                for m in re.finditer(r'\b' + re.escape(fn) + r'\(', body):
                    args = self._args(body, m.end())
                    for k, a in enumerate(args):
                        if k >= len(self.params.get(fn, [])): break
                        p = self.params[fn][k]
                        mm = re.fullmatch(r'\(\*spvDescriptorSet0\.(\w+)\)|spvDescriptorSet0\.(\w+)|(\w+)', a.strip())
                        if mm and not ID.fullmatch(p or '_0'):
                            base = mm.group(1) or mm.group(2) or mm.group(3)
                            if base in self.arg_names or base in self.buffer_names or base in self.alias:
                                self.alias[p] = self.alias.get(base, base)

    @staticmethod
    def _args(s, i):
        d = 1; j = i
        while j < len(s) and d:
            if s[j] in '([': d += 1
            elif s[j] in ')]': d -= 1
            j += 1
        return split_top(s[i:j - 1])

    def _refs(self, expr):
        """the vars, loads, stage inputs and textures an expression reads; registers dynamic indices as roots"""
        refs = set()
        for m in re.finditer(r'\(\*spvDescriptorSet0\.(\w+)\)|spvDescriptorSet0\.(\w+)|\b(\w+)\b(?=\s*(?:\.|->|\[))', expr):
            base = m.group(1) or m.group(2) or m.group(3)
            if m.group(3):
                if base in ('in', 'dm_in'):
                    mm = re.match(r'\s*\.(\w+)', expr[m.end():])
                    if mm: refs.add('IN:' + mm.group(1))
                    continue
                base = self.alias.get(base, base)
                if base not in self.buffer_names and base not in self.arg_names: continue
            toks, idx, end = scan_path(expr, m.end())
            if base in self.tex_names and re.match(r'\.(sample|read|gather|sample_compare)\w*\(', expr[end:]):
                refs.add('T:' + base); continue
            if base in self.tex_names: continue
            self.loads.append((base, toks))
            lid = 'L%d' % (len(self.loads) - 1)
            refs.add(lid)
            if re.search(r'as_type<\s*u?int\d?\s*>\(\s*$', expr[:m.start()]): self.asint_loads.add(lid)
            for e in idx:
                for v in self._refs(e): self.roots_idx.add(v)
        for v in ID.findall(re.sub(r'\[\[.*?\]\]', '', expr)): refs.add(v)
        for m in re.finditer(r'as_type<\s*u?int\d?\s*>\(\s*(_\d+)\s*\)', expr): self.asint.add(m.group(1))
        for fn in self.fnames:
            if re.search(r'\b' + re.escape(fn) + r'\(', expr): refs.add('RET:' + fn)
        # dynamic indices into local arrays
        for m in re.finditer(r'\b_\d+\s*((?:\[[^\]]*\])+)', expr):
            for e in re.findall(r'\[([^\]]*)\]', m.group(1)):
                if not re.fullmatch(r'\s*\d+[uU]?\s*', e):
                    self.roots_idx.update(v for v in ID.findall(e))
        return refs

    def _body(self, fname, body):
        params = self.params.get(fname, [])
        for st in re.split(r'[;{}]', body):
            st = st.strip()
            if not st: continue
            m = re.match(r'^(?:while|if)\s*\((.*)\)\s*$', st) or re.match(r'^\}?\s*while\s*\((.*)\)$', st)
            if st.startswith('while') or re.match(r'^for\s*\(', st):
                self.roots_loop.update(self._refs(st))
                continue
            if st.startswith('switch'):
                self.roots_idx.update(self._refs(st)); continue
            if st.startswith('return'):
                self.deps['RET:' + fname].update(self._refs(st[6:])); continue
            if st.startswith('if') or st.startswith('else') or st.startswith('case') or st.startswith('default') or st == 'do':
                if st.startswith('if'): self._refs(st)
                continue
            # calls: map arguments to parameters
            for fn in self.fnames:
                for mm in re.finditer(r'\b' + re.escape(fn) + r'\(', st):
                    args = self._args(st, mm.end())
                    for k, a in enumerate(args):
                        if k < len(self.params.get(fn, [])):
                            p = self.params[fn][k]
                            if p: self.deps[p].update(self._refs(a))
            m = re.match(r'^(?:(?:const\s+)?[A-Za-z_][\w:<>, ]*?\s+)?((?:_\d+|out|dm_r|r|in)(?:\.\w+|\[[^\]]*\])*)\s*([-+*/|&^]?=)\s*(.+)$', st, re.S)
            if m:
                lhs = m.group(1)
                base = re.match(r'_\d+|\w+', lhs).group(0)
                refs = self._refs(m.group(3))
                for e in re.findall(r'\[([^\]]*)\]', lhs):
                    if not re.fullmatch(r'\s*\d+[uU]?\s*', e): self.roots_idx.update(ID.findall(e))
                if ID.fullmatch(base): self.deps[base].update(refs)
            else:
                self._refs(st)

    def closure(self, roots):
        seen, stack = set(), list(roots)
        while stack:
            v = stack.pop()
            if v in seen: continue
            seen.add(v)
            stack.extend(self.deps.get(v, ()))
        return seen


def path_str(toks):
    s = ''.join(toks)
    return s[1:] if s.startswith('.') else s


def analyse(src):
    src = strip_comments(src)
    structs = parse_structs(src)
    entries = parse_entries(src)
    fconsts = parse_fconsts(src)
    argbuf = structs.get('spvDescriptorSetBuffer0', [])
    arg_names = [m['name'] for m in argbuf]
    tex_names = [m['name'] for m in argbuf if m['type'].startswith(('texture', 'depth'))]
    buffer_names = sorted({p['name'] for e in entries for p in e['params'] if attr_val(p['attrs'], 'buffer') is not None and p['name'] != 'spvDescriptorSet0'})
    flow = Flow(src, buffer_names, arg_names, tex_names)
    loop = flow.closure(flow.roots_loop)
    idx = flow.closure(flow.roots_idx)
    # which loads are read as integer bits: the load's var (or the load inside as_type) is reinterpreted
    asint_loads = set(flow.asint_loads)
    for v, ds in flow.deps.items():
        if v in flow.asint:
            for d in ds:
                if d.startswith('L'): asint_loads.add(d)
    for m in re.finditer(r'as_type<\s*u?int\d?\s*>\(\s*\(\*spvDescriptorSet0', src): pass
    hints = collections.defaultdict(dict)        # scope -> path -> rule
    stage_ctl = {}                                # stage input member -> rule
    tex_ctl = set()
    for v in loop | idx:
        inloop = v in loop
        if v.startswith('L'):
            base, toks = flow.loads[int(v[1:])]
            path = path_str(toks)
            dyn = '*' in path
            scope = ('arg:' if base in arg_names else 'buf:') + base
            if inloop: rule = {'class': 'ctl', 'lo': 1, 'hi': 8, 'loop': True}
            elif dyn: rule = {'class': 'mixctl', 'p': 75, 'lo': 0, 'hi': 16}
            else: rule = {'class': 'ctl', 'lo': 0, 'hi': 16}
            if v in asint_loads: rule['repr'] = 'i'
            old = hints[scope].get(path)
            if old is None or (old.get('class') != 'ctl' and rule['class'] == 'ctl') or (inloop and old.get('hi', 99) > 8):
                hints[scope][path] = rule
        elif v.startswith('IN:'):
            stage_ctl[v[3:]] = {'class': 'ctl', 'lo': 1 if inloop else 0, 'hi': 8 if inloop else 16}
        elif v.startswith('T:'):
            tex_ctl.add(v[2:])
    # in a buffer with a dynamically indexed control region, the words the code reads at constant indices for float
    # math keep the full float mixture (rules are applied in order: the region first, then these)
    ctl_loads = {v for v in loop | idx if v.startswith('L')}
    for i, (base, toks) in enumerate(flow.loads):
        scope = ('arg:' if base in arg_names else 'buf:') + base
        if scope not in hints or not any('*' in p for p in hints[scope]): continue
        path = path_str(toks)
        if '*' in path or ('L%d' % i) in ctl_loads or path in hints[scope]: continue
        hints[scope][path] = {'class': 'f'}
    sample_compare = set(re.findall(r'\.sample_compare\(\s*spvDescriptorSet0\.(\w+)', src))
    sample_compare |= set(re.findall(r'spvDepthCast\w*\([^;]*?spvDescriptorSet0\.(\w+Smplr\w*)', src))
    return {
        'structs': structs, 'entries': entries, 'function_constants': fconsts, 'argbuf': argbuf,
        'buffer_params': buffer_names,
        'hints': {k: [dict(path=p, **r) for p, r in sorted(v.items(), key=lambda kv: ('*' not in kv[0], kv[0]))] for k, v in hints.items()},
        'stage_ctl': stage_ctl, 'texture_ctl': sorted(tex_ctl), 'compare_samplers': sorted(sample_compare),
        'loops': len(flow.roots_loop) > 0, 'n_loop_slice': len(loop), 'n_idx_slice': len(idx),
    }


if __name__ == '__main__':
    print(json.dumps(analyse(open(sys.argv[1]).read()), indent=1)[:200000])
