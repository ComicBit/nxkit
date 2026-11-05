//! The registered patches as `rules/*.toml` (design 2.2 "Rules are data"): one file per scope (`sdk` or a profile
//! folder), `files.toml` for the source hashes. Each `[[rule]]` has a `binding` (`host`, `sdk`, `engine`, `game` or
//! `build`), a matcher (a build ID and an offset, or function fingerprints), a rewrite and a `why`. A `build` rule
//! carries the original bytes, which the rewriter asserts. The files hold instruction words of game code: they stay
//! outside the repository (a private data folder named by the caller). The writer is a 1:1 image of the snapshot
//! (`patches.rs`) and the reader returns the same table.

mod read;
mod toml;
mod write;

use std::fmt::Write as _;

use crate::patches::{FsItem, FsOp, PatchTable, Sym};

pub(crate) use toml::quote;

/// The bindings a rule may name.
pub const BINDINGS: [&str; 5] = ["host", "sdk", "engine", "game", "build"];

fn sym_text(s: &Sym) -> String {
    match s {
        Sym::Abs(a) => a.to_string(),
        Sym::Label(n, d) => format!("L{n}{d:+}"),
        Sym::State(d) => format!("S{d:+}"),
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn item_text(i: &FsItem) -> String {
    match i {
        FsItem::Word(w) => format!("w:{w}"),
        FsItem::B(f, t, m) => format!("b:{}:{}:{m}", sym_text(f), sym_text(t)),
    }
}

impl PatchTable {
    /// The table as snapshot lines in a fixed order (builds and scopes sorted): equal tables give equal text.
    pub fn to_snapshot(&self) -> String {
        let mut o = String::new();
        let mut row = |c: &[String]| {
            let _ = writeln!(
                o,
                "{}",
                c.iter().map(|x| esc(x)).collect::<Vec<_>>().join("\t")
            );
        };
        for (p, h) in &self.files {
            row(&["file".into(), p.clone(), h.clone()]);
        }
        let mut builds: Vec<&String> = self
            .cuts
            .keys()
            .chain(self.obs.keys())
            .chain(self.caps.keys())
            .chain(self.frame.iter())
            .collect();
        builds.sort();
        builds.dedup();
        for b in builds {
            if let Some(s) = self.build_scope.get(b) {
                row(&["bscope".into(), b.clone(), s.clone()]);
            }
            if let Some(cs) = self.cuts.get(b) {
                for c in cs {
                    row(&[
                        "cuts".into(),
                        b.clone(),
                        c.off.to_string(),
                        c.old.clone(),
                        c.new.clone(),
                        c.why.clone(),
                    ]);
                }
                if cs.is_empty() {
                    row(&["cutsempty".into(), b.clone()]);
                }
            }
            if let Some(os) = self.obs.get(b) {
                for x in os {
                    row(&[
                        "obs".into(),
                        b.clone(),
                        x.off.to_string(),
                        x.id.to_string(),
                        x.word.clone(),
                        x.why.clone(),
                    ]);
                }
                if os.is_empty() {
                    row(&["obsempty".into(), b.clone()]);
                }
            }
            for c in self.caps.get(b).into_iter().flatten() {
                row(&["cap".into(), b.clone(), c.clone()]);
            }
            if self.frame.contains(b) {
                row(&["frame".into(), b.clone()]);
                for op in self.frame_ops.get(b).into_iter().flatten() {
                    let mut c = vec!["fs".to_string(), b.clone()];
                    match op {
                        FsOp::Site(a, h) => c.extend(["site".into(), a.to_string(), h.clone()]),
                        FsOp::Word(w) => c.extend(["word".into(), w.to_string()]),
                        FsOp::Adrp(r, p, t) => {
                            c.extend(["adrp".into(), r.to_string(), sym_text(p), sym_text(t)]);
                        }
                        FsOp::B(f, t, m) => {
                            c.extend(["b".into(), sym_text(f), sym_text(t), m.to_string()]);
                        }
                        FsOp::Label(n) => c.extend(["label".into(), n.to_string()]),
                        FsOp::Scale => c.push("scale".into()),
                        FsOp::Put(a, why, items) => c.extend([
                            "put".into(),
                            a.to_string(),
                            why.clone(),
                            items.iter().map(item_text).collect::<Vec<_>>().join(";"),
                        ]),
                    }
                    row(&c);
                }
                if self.frame_no.contains(b) {
                    row(&["fsno".into(), b.clone(), String::new()]);
                }
            }
        }
        let mut scopes: Vec<&String> = self.rules.keys().collect();
        scopes.sort();
        for sc in scopes {
            let mut mods: Vec<&String> = self.rules[sc].keys().collect();
            mods.sort();
            for m in mods {
                for (ri, r) in self.rules[sc][m].iter().enumerate() {
                    for (fi, f) in r.fns.iter().enumerate() {
                        let fno = if r.multi { fi + 1 } else { 0 };
                        row(&[
                            "rule".into(),
                            sc.clone(),
                            m.clone(),
                            r.name.clone(),
                            ri.to_string(),
                            fno.to_string(),
                            f.size.to_string(),
                            f.fps.join(","),
                        ]);
                        for (d, ob, nb, why) in &f.cuts {
                            row(&[
                                "rcut".into(),
                                sc.clone(),
                                m.clone(),
                                r.name.clone(),
                                ri.to_string(),
                                fno.to_string(),
                                d.to_string(),
                                ob.clone(),
                                nb.clone(),
                                why.clone(),
                            ]);
                        }
                    }
                }
            }
        }
        let mut profs: Vec<&String> = self.gbuild.keys().collect();
        profs.sort();
        for p in profs {
            for r in &self.gbuild[p] {
                let mut c = vec!["gbuild".to_string(), p.clone()];
                c.extend(r.iter().cloned());
                row(&c);
            }
        }
        o
    }
}

#[cfg(test)]
mod tests;
