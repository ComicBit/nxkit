//! The writer of the rule files.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::{item_text, quote, sym_text};
use crate::patches::{FsOp, PatchTable};

const HEADER: &str = "# nxb rewrite rules (generated from the engine's patch files). Private data: instruction words of game code.\n";

fn list(v: &[String]) -> String {
    format!(
        "[{}]",
        v.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", ")
    )
}

impl PatchTable {
    /// The rule files `(file name, text)`: `files.toml` and one file per scope (`sdk.toml`, `<profile>.toml`).
    pub fn to_toml(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut f = String::from(HEADER);
        for (p, h) in &self.files {
            let _ = write!(
                f,
                "\n[[file]]\npath = {}\nsha256 = {}\n",
                quote(p),
                quote(h)
            );
        }
        out.push(("files.toml".to_string(), f));
        let mut scopes: BTreeMap<String, String> = BTreeMap::new();
        let mut all: Vec<&String> = self
            .cuts
            .keys()
            .chain(self.obs.keys())
            .chain(self.caps.keys())
            .chain(self.frame.iter())
            .collect();
        all.sort();
        all.dedup();
        let scope_of = |b: &String| {
            self.build_scope
                .get(b)
                .cloned()
                .unwrap_or_else(|| "unscoped".to_string())
        };
        let mut names: Vec<String> = all.iter().map(|b| scope_of(b)).collect();
        names.extend(self.rules.keys().cloned());
        names.extend(self.gbuild.keys().cloned());
        names.sort();
        names.dedup();
        for n in &names {
            scopes.insert(n.clone(), format!("{HEADER}\nscope = {}\n", quote(n)));
        }
        for (prof, rows) in sorted(&self.gbuild) {
            let t = scopes.entry(prof.clone()).or_default();
            for r in rows {
                let _ = write!(t, "\n[[build]]\nrow = {}\n", list(r));
            }
        }
        for b in all {
            let sc = scope_of(b);
            let t = scopes.entry(sc).or_default();
            for c in self.cuts.get(b).into_iter().flatten() {
                let _ = write!(
                    t,
                    "\n[[rule]]\nbinding = \"build\"\nkind = \"cut\"\nbuild = {}\noffset = {}\nold = {}\nnew = {}\nwhy = {}\n",
                    quote(b),
                    c.off,
                    quote(&c.old),
                    quote(&c.new),
                    quote(&c.why)
                );
            }
            if self.cuts.get(b).is_some_and(Vec::is_empty) {
                let _ = write!(
                    t,
                    "\n[[rule]]\nbinding = \"build\"\nkind = \"cuts-none\"\nbuild = {}\n",
                    quote(b)
                );
            }
            for x in self.obs.get(b).into_iter().flatten() {
                let _ = write!(
                    t,
                    "\n[[rule]]\nbinding = \"build\"\nkind = \"obs\"\nbuild = {}\noffset = {}\nnumber = {}\noriginal = {}\nwhy = {}\n",
                    quote(b),
                    x.off,
                    x.id,
                    quote(&x.word),
                    quote(&x.why)
                );
            }
            if self.obs.get(b).is_some_and(Vec::is_empty) {
                let _ = write!(
                    t,
                    "\n[[rule]]\nbinding = \"build\"\nkind = \"obs-none\"\nbuild = {}\n",
                    quote(b)
                );
            }
            for c in self.caps.get(b).into_iter().flatten() {
                let _ = write!(
                    t,
                    "\n[[rule]]\nbinding = \"build\"\nkind = \"capability\"\nbuild = {}\nname = {}\n",
                    quote(b),
                    quote(c)
                );
            }
            if self.frame.contains(b) {
                let kind = if self.frame_no.contains(b) {
                    "frame-step-unsupported"
                } else {
                    "frame-step"
                };
                let _ = write!(
                    t,
                    "\n[[rule]]\nbinding = \"build\"\nkind = \"{kind}\"\nbuild = {}\nwhy = \"frame step stub of the build\"\n",
                    quote(b)
                );
                for op in self.frame_ops.get(b).into_iter().flatten() {
                    t.push_str("\n[[rule.step]]\n");
                    match op {
                        FsOp::Site(a, h) => {
                            let _ = write!(t, "op = \"site\"\nat = {a}\noriginal = {}\n", quote(h));
                        }
                        FsOp::Word(w) => {
                            let _ = write!(t, "op = \"word\"\nword = {w}\n");
                        }
                        FsOp::Adrp(r, p, tg) => {
                            let _ = write!(
                                t,
                                "op = \"adrp\"\nreg = {r}\npc = {}\ntarget = {}\n",
                                quote(&sym_text(p)),
                                quote(&sym_text(tg))
                            );
                        }
                        FsOp::B(fr, to, m) => {
                            let _ = write!(
                                t,
                                "op = \"b\"\nfrom = {}\nto = {}\nmask = {m}\n",
                                quote(&sym_text(fr)),
                                quote(&sym_text(to))
                            );
                        }
                        FsOp::Label(n) => {
                            let _ = write!(t, "op = \"label\"\nn = {n}\n");
                        }
                        FsOp::Scale => t.push_str("op = \"scale\"\n"),
                        FsOp::Put(a, why, items) => {
                            let it: Vec<String> = items.iter().map(item_text).collect();
                            let _ = write!(
                                t,
                                "op = \"put\"\nat = {a}\nwhy = {}\nitems = {}\n",
                                quote(why),
                                list(&it)
                            );
                        }
                    }
                }
            }
        }
        for (sc, mods) in sorted(&self.rules) {
            let binding = if sc == "sdk" { "sdk" } else { "game" };
            let t = scopes.entry(sc.clone()).or_default();
            for (m, rules) in sorted(mods) {
                for r in rules {
                    let _ = write!(
                        t,
                        "\n[[rule]]\nbinding = \"{binding}\"\nkind = \"match\"\nmodule = {}\nname = {}\nmulti = {}\n",
                        quote(m),
                        quote(&r.name),
                        r.multi
                    );
                    for fnr in &r.fns {
                        let _ = write!(
                            t,
                            "\n[[rule.function]]\nsize = {}\nfingerprints = {}\n",
                            fnr.size,
                            list(&fnr.fps)
                        );
                        for (d, ob, nb, why) in &fnr.cuts {
                            let _ = write!(
                                t,
                                "\n[[rule.function.cut]]\ndelta = {d}\nold = {}\nnew = {}\nwhy = {}\n",
                                quote(ob),
                                quote(nb),
                                quote(why)
                            );
                        }
                    }
                }
            }
        }
        for (n, text) in scopes {
            out.push((format!("{n}.toml"), text));
        }
        out
    }
}

fn sorted<V>(m: &std::collections::HashMap<String, V>) -> Vec<(&String, &V)> {
    let mut v: Vec<(&String, &V)> = m.iter().collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    v
}
