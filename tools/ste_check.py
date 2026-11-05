import re, sys

FLAG = """ability able acceptable achieve acquire action activity additional adequate affect allow already
alternatively amount analyze another any appear appropriate approve arise ask assess assign assist assume
attempt avoid aware begin beginning big both bring build capability capable carry certain choose complete
completely complicated comply comprise confirm consider consist construct convert create critical decide
define deliver delivery depend describe desired detect determine difficult display due early either eliminate
enable enough ensure entire establish evaluate every exact except exist explain extra fail feasible final finish
following function further gain generate great handle happen however identical immediate implement improve
including indicate inform initiate insert instead job kind later locate main maintain major may minimize
modify need never normal normally now obtain often once option original otherwise people perform permit
place poor prior priority process produce proper properly provide reason reduce relevant remain repeat
request require retain return reuse review run save search several shall should significant similar
simultaneously situation size specific state submit substitute successful suitable switch therefore thorough
track transfer trigger true twice unable upon usage utilize valid various verify via whether whole within
would wrong yet currently""".split()
OK_ING = {"lighting", "opening", "routing", "servicing", "mating", "missing", "remaining", "something", "during"}
LATIN = re.compile(r"\b(e[.·]g[.·]|i[.·]e[.·]|etc[.·]|cf[.·]|vs[.·])", re.I)
PASSIVE = re.compile(r"\b(is|are|was|were|be|been|being)\s+(\w+ly\s+)?(\w+ed|built|done|made|given|sent|run|set|kept|found|shown|written)\b", re.I)
PERFECT = re.compile(r"\b(has|have|had)\s+(\w+ed|been|done|made|given|sent|run|set|kept|found|shown|written)\b", re.I)

def words(sentence):
    s = re.sub(r"\([^)]*\)", " X ", sentence)      # rule 8.5
    s = re.sub(r"\"[^\"]*\"|“[^”]*”", " X ", s)     # rule 8.6 quoted text
    return [w for w in re.split(r"\s+", s) if re.search(r"\w", w)]

text = open(sys.argv[1], encoding="utf-8").read()
for n, line in enumerate(text.splitlines(), 1):
    item = line.strip().lstrip("-*•0123456789.() ").strip()
    if not item:
        continue
    item = LATIN.sub(lambda m: m.group(0).replace(".", "·"), item)  # keep e.g. in one sentence
    for sent in re.split(r"(?<=[.!?:])\s+", item):
        w = words(sent)
        issues = []
        if len(w) > 25:
            issues.append(f"{len(w)} words (max 20 procedural / 25 descriptive)")
        elif len(w) > 20:
            issues.append(f"{len(w)} words (OK only if descriptive)")
        if ";" in sent: issues.append("semicolon")
        if re.search(r"\b\w+n't\b|\b\w+'(re|ve|ll|d)\b|\b(it|that|there|here|what|who)'s\b", sent, re.I): issues.append("contraction")
        if LATIN.search(sent): issues.append("Latin abbreviation")
        if PASSIVE.search(sent): issues.append("possible passive: " + PASSIVE.search(sent).group(0))
        if PERFECT.search(sent): issues.append("perfect tense: " + PERFECT.search(sent).group(0))
        ing = [x for x in re.findall(r"\b[a-z]+ing\b", sent) if x not in OK_ING and len(x) > 4]
        if ing: issues.append("-ing: " + ", ".join(sorted(set(ing))))
        bad = sorted({x for x in re.findall(r"[a-z]+", sent.lower()) if x in FLAG or re.sub(r"(es|s|ed|d)$", "", x) in FLAG})
        if bad: issues.append("check words: " + ", ".join(bad))
        if issues:
            print(f"line {n}: {sent[:70]}…\n   - " + "\n   - ".join(issues))
