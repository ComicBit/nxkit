//! The reader of the rule files.

use super::BINDINGS;
use super::toml::{Table, parse};
use crate::error::{RewriteError, fail};
use crate::patches::{Cut, FnRule, FsItem, FsOp, Obs, PatchTable, Rule, Sym};

fn num(t: &Table, k: &str) -> Result<usize, RewriteError> {
    usize::try_from(t.int(k)?).or_else(|_| fail(format!("rules: {k:?} is negative")))
}

fn hex_ok(s: &str) -> bool {
    s.len().is_multiple_of(2) && s.bytes().all(|c| c.is_ascii_hexdigit())
}

fn step(t: &Table) -> Result<FsOp, RewriteError> {
    Ok(match t.str("op")? {
        "site" => FsOp::Site(num(t, "at")?, t.str("original")?.into()),
        "word" => FsOp::Word(num(t, "word")? as u32),
        "adrp" => FsOp::Adrp(
            num(t, "reg")? as u32,
            Sym::parse(t.str("pc")?)?,
            Sym::parse(t.str("target")?)?,
        ),
        "b" => FsOp::B(
            Sym::parse(t.str("from")?)?,
            Sym::parse(t.str("to")?)?,
            num(t, "mask")? as u32,
        ),
        "label" => FsOp::Label(num(t, "n")?),
        "scale" => FsOp::Scale,
        "put" => {
            let mut items = Vec::new();
            for i in t.list("items")? {
                let p: Vec<&str> = i.split(':').collect();
                items.push(match p.as_slice() {
                    ["w", v] => {
                        FsItem::Word(v.parse::<u32>().or_else(|_| fail("rules: bad put word"))?)
                    }
                    ["b", f, to, m] => FsItem::B(
                        Sym::parse(f)?,
                        Sym::parse(to)?,
                        m.parse::<u32>().or_else(|_| fail("rules: bad put mask"))?,
                    ),
                    _ => return fail("rules: bad put item"),
                });
            }
            FsOp::Put(num(t, "at")?, t.str("why")?.into(), items)
        }
        o => return fail(format!("rules: unknown step {o:?}")),
    })
}

impl PatchTable {
    /// Reads the rule files `(file name, text)` written by [`PatchTable::to_toml`]. A rule whose binding is not one
    /// of [`BINDINGS`], a `build` rule without its original bytes, and a build claimed twice are errors.
    pub fn from_toml(files: &[(String, String)]) -> Result<PatchTable, RewriteError> {
        let mut t = PatchTable::default();
        let mut sorted: Vec<&(String, String)> = files.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, text) in sorted {
            let root = parse(text).map_err(|e| RewriteError::Failed(format!("{name}: {e}")))?;
            for f in root.tables("file") {
                t.files
                    .push((f.str("path")?.into(), f.str("sha256")?.into()));
            }
            if name == "files.toml" {
                continue;
            }
            let scope = root.str("scope")?.to_string();
            for b in root.tables("build") {
                t.gbuild
                    .entry(scope.clone())
                    .or_default()
                    .push(b.list("row")?.to_vec());
            }
            for r in root.tables("rule") {
                let binding = r.str("binding")?;
                if !BINDINGS.contains(&binding) {
                    return fail(format!("{name}: unknown binding {binding:?}"));
                }
                let kind = r.str("kind")?;
                let bid = r.str("build").unwrap_or("").to_string();
                if !bid.is_empty()
                    && t.build_scope
                        .insert(bid.clone(), scope.clone())
                        .is_some_and(|old| old != scope)
                {
                    return fail(format!("{name}: build {bid} is claimed by two scopes"));
                }
                match kind {
                    "cut" => {
                        let (old, new) = (r.str("old")?, r.str("new")?);
                        if old.is_empty() || !hex_ok(old) || !hex_ok(new) {
                            return fail(format!(
                                "{name}: a build rule needs its original bytes (build {bid})"
                            ));
                        }
                        t.cuts.entry(bid).or_default().push(Cut {
                            off: num(r, "offset")?,
                            old: old.into(),
                            new: new.into(),
                            why: r.str("why")?.into(),
                        });
                    }
                    "cuts-none" => {
                        t.cuts.entry(bid).or_default();
                    }
                    "obs" => {
                        let w = r.str("original")?;
                        if w.len() != 8 || !hex_ok(w) {
                            return fail(format!(
                                "{name}: an observation rule needs its original word (build {bid})"
                            ));
                        }
                        t.obs.entry(bid).or_default().push(Obs {
                            off: num(r, "offset")?,
                            id: num(r, "number")? as u64,
                            word: w.into(),
                            why: r.str("why")?.into(),
                        });
                    }
                    "obs-none" => {
                        t.obs.entry(bid).or_default();
                    }
                    "capability" => t.caps.entry(bid).or_default().push(r.str("name")?.into()),
                    "frame-step" | "frame-step-unsupported" => {
                        t.frame.insert(bid.clone());
                        if kind == "frame-step" {
                            let ops = r
                                .tables("step")
                                .iter()
                                .map(step)
                                .collect::<Result<Vec<_>, _>>()?;
                            if !ops.iter().any(|o| matches!(o, FsOp::Site(..))) {
                                return fail(format!(
                                    "{name}: a frame step rule needs its original bytes (build {bid})"
                                ));
                            }
                            t.frame_ops.insert(bid, ops);
                        } else {
                            t.frame_no.insert(bid);
                        }
                    }
                    "match" => {
                        let mut fns = Vec::new();
                        for f in r.tables("function") {
                            let mut cuts = Vec::new();
                            for c in f.tables("cut") {
                                cuts.push((
                                    num(c, "delta")?,
                                    c.str("old")?.into(),
                                    c.str("new")?.into(),
                                    c.str("why")?.into(),
                                ));
                            }
                            fns.push(FnRule {
                                size: num(f, "size")?,
                                fps: f.list("fingerprints")?.to_vec(),
                                cuts,
                            });
                        }
                        let multi = matches!(r.get("multi"), Some(super::toml::Val::Bool(true)));
                        t.rules
                            .entry(scope.clone())
                            .or_default()
                            .entry(r.str("module")?.into())
                            .or_default()
                            .push(Rule {
                                name: r.str("name")?.into(),
                                fns,
                                multi,
                            });
                    }
                    k => return fail(format!("{name}: unknown rule kind {k:?}")),
                }
            }
        }
        Ok(t)
    }
}
