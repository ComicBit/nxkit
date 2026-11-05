//! The registered patches of the engine (`CUTS`, `OBS`, `MATCH_CUTS`, `CAPABILITIES`, `FRAME_STEP` of
//! `sdk_patches.py` and every `games/*/patches.py`) as a table.
//!
//! The reference script executes those Python files, which build their rows with helper functions, so the
//! rows reach this crate as a TSV snapshot made by `tools/gen_rewrite_patches.py` of the same engine. The file
//! list at the top of the snapshot holds the SHA-256 of every source file, the caller refuses a snapshot whose
//! hashes differ from the engine's files.
//!
//! Lines (tab separated, `\t` `\n` `\\` escaped inside fields):
//! `file path sha256`, `cuts build off old new why`, `obs build off number word why`, `cap build NAME`,
//! `bscope build scope` (the patch file that registered a build), `frame build`, `fs build op ...` (a recorded stub step), `gbuild profile col...` (a `build` row of a profile's `game.tsv`),
//! `rule scope module name ruleno fnno size fingerprints(comma)` and
//! `rcut scope module name ruleno fnno delta old new why`, scope is `sdk` or a profile folder name,
//! a rule with `fnno` 0 only is a single-function rule, with `fnno` 1.. a rule of several functions.

use std::collections::{HashMap, HashSet};

use crate::error::{RewriteError, fail};

/// A registered cut: bytes at a module offset, asserted and replaced.
#[derive(Clone, Debug)]
pub struct Cut {
    /// The offset in the module image.
    pub off: usize,
    /// The original bytes as hexadecimal text.
    pub old: String,
    /// The new bytes as hexadecimal text.
    pub new: String,
    /// The reason.
    pub why: String,
}

/// A registered observation gate.
#[derive(Clone, Debug)]
pub struct Obs {
    /// The offset of the instruction.
    pub off: usize,
    /// The observation number.
    pub id: u64,
    /// The original instruction word as 8 hexadecimal digits.
    pub word: String,
    /// The reason.
    pub why: String,
}

/// One function of a matching rule.
#[derive(Clone, Debug, Default)]
pub struct FnRule {
    /// The function's size in bytes.
    pub size: usize,
    /// The accepted fingerprints.
    pub fps: Vec<String>,
    /// `(delta, old, new, reason)`.
    pub cuts: Vec<(usize, String, String, String)>,
}

/// A function-matching rule.
#[derive(Clone, Debug)]
pub struct Rule {
    /// The patch name (rules of the same name are alternatives).
    pub name: String,
    /// One function, or several that apply together.
    pub fns: Vec<FnRule>,
    /// Written as `functions: [...]`.
    pub multi: bool,
}

/// An address of a recorded stub: a constant, a label of the stub's emission point plus a constant, or the frame
/// state page plus a constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sym {
    /// A module offset.
    Abs(i64),
    /// Label `n` plus a constant.
    Label(usize, i64),
    /// The frame state page plus a constant.
    State(i64),
}

impl Sym {
    /// Parses `123`, `L0+4` or `S+0`.
    pub fn parse(s: &str) -> Result<Sym, RewriteError> {
        let bad = || fail(format!("patch table: bad address {s:?}"));
        let split = |t: &str| -> Option<(String, i64)> {
            let i = t.rfind(['+', '-'])?;
            Some((t[..i].to_string(), t[i..].parse::<i64>().ok()?))
        };
        if let Some(r) = s.strip_prefix('S') {
            return match split(&format!("S{r}")) {
                Some((k, d)) if k == "S" => Ok(Sym::State(d)),
                _ => bad(),
            };
        }
        if let Some(r) = s.strip_prefix('L') {
            return match split(r).and_then(|(n, d)| Some((n.parse::<usize>().ok()?, d))) {
                Some((n, d)) => Ok(Sym::Label(n, d)),
                None => bad(),
            };
        }
        s.parse::<i64>().map(Sym::Abs).or_else(|_| bad())
    }
}

/// One step of a recorded `FRAME_STEP` stub.
#[derive(Clone, Debug)]
pub enum FsOp {
    /// Assert the original bytes at an offset (hexadecimal) and that no rewrite site lies there.
    Site(usize, String),
    /// Emit a literal word.
    Word(u32),
    /// Emit `adrp reg` at `pc` to the page of `target`: `(reg, pc, target)`.
    Adrp(u32, Sym, Sym),
    /// Emit `b to` as encoded at `frm`, or-ed with a mask (`bl` is `0x80000000`): `(frm, to, mask)`.
    B(Sym, Sym, u32),
    /// Name the current emission point.
    Label(usize),
    /// Overwrite code at an offset with literal words or branches: `(offset, reason, items)`.
    Put(usize, String, Vec<FsItem>),
    /// The build has a render-scale stub.
    Scale,
}

/// A word of a `Put`.
#[derive(Clone, Debug)]
pub enum FsItem {
    /// A literal word.
    Word(u32),
    /// A branch `(frm, to, mask)` as in [`FsOp::B`].
    B(Sym, Sym, u32),
}

/// All registered patches.
#[derive(Default, Debug)]
pub struct PatchTable {
    /// `(path relative to dm/tools, sha256)` of every source file the snapshot was made from.
    pub files: Vec<(String, String)>,
    /// `CUTS` by build ID.
    pub cuts: HashMap<String, Vec<Cut>>,
    /// `OBS` by build ID.
    pub obs: HashMap<String, Vec<Obs>>,
    /// `CAPABILITIES` by build ID.
    pub caps: HashMap<String, Vec<String>>,
    /// Build IDs that have a `FRAME_STEP`.
    pub frame: HashSet<String>,
    /// Builds whose stub could not be recorded.
    pub frame_no: HashSet<String>,
    /// The recorded stubs of the builds in `frame`.
    pub frame_ops: HashMap<String, Vec<FsOp>>,
    /// The scope (`sdk` or a profile folder name) of the patch file that registered a build ID.
    pub build_scope: HashMap<String, String>,
    /// `game.tsv` build rows by profile.
    pub gbuild: HashMap<String, Vec<Vec<String>>>,
    /// Matching rules: scope, then module name.
    pub rules: HashMap<String, HashMap<String, Vec<Rule>>>,
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn num(s: &str) -> Result<usize, RewriteError> {
    s.parse::<usize>()
        .or_else(|_| fail(format!("patch table: bad number {s:?}")))
}

impl PatchTable {
    /// Parses a snapshot.
    pub fn parse(text: &str) -> Result<PatchTable, RewriteError> {
        let mut t = PatchTable::default();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<String> = line.split('\t').map(unescape).collect();
            let g = |i: usize| f.get(i).map(String::as_str).unwrap_or("");
            match g(0) {
                "file" => t.files.push((g(1).into(), g(2).into())),
                "cuts" => t.cuts.entry(g(1).into()).or_default().push(Cut {
                    off: num(g(2))?,
                    old: g(3).into(),
                    new: g(4).into(),
                    why: g(5).into(),
                }),
                "obs" => t.obs.entry(g(1).into()).or_default().push(Obs {
                    off: num(g(2))?,
                    id: num(g(3))? as u64,
                    word: g(4).into(),
                    why: g(5).into(),
                }),
                "cutsempty" => {
                    t.cuts.entry(g(1).into()).or_default();
                }
                "obsempty" => {
                    t.obs.entry(g(1).into()).or_default();
                }
                "cap" => t.caps.entry(g(1).into()).or_default().push(g(2).into()),
                "bscope" => {
                    t.build_scope.insert(g(1).into(), g(2).into());
                }
                "frame" => {
                    t.frame.insert(g(1).into());
                }
                "fs" => {
                    let op = match g(2) {
                        "site" => FsOp::Site(num(g(3))?, g(4).into()),
                        "word" => FsOp::Word(num(g(3))? as u32),
                        "adrp" => {
                            FsOp::Adrp(num(g(3))? as u32, Sym::parse(g(4))?, Sym::parse(g(5))?)
                        }
                        "b" => FsOp::B(Sym::parse(g(3))?, Sym::parse(g(4))?, num(g(5))? as u32),
                        "label" => FsOp::Label(num(g(3))?),
                        "scale" => FsOp::Scale,
                        "put" => FsOp::Put(
                            num(g(3))?,
                            g(4).into(),
                            g(5).split(';')
                                .map(|i| {
                                    let p: Vec<&str> = i.split(':').collect();
                                    match p.as_slice() {
                                        ["w", v] => num(v).map(|v| FsItem::Word(v as u32)),
                                        ["b", f, t, m] => Ok(FsItem::B(
                                            Sym::parse(f)?,
                                            Sym::parse(t)?,
                                            num(m)? as u32,
                                        )),
                                        _ => fail("patch table: bad put item"),
                                    }
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                        ),
                        o => return fail(format!("patch table: unknown fs op {o:?}")),
                    };
                    t.frame_ops.entry(g(1).into()).or_default().push(op);
                }
                "fsno" => {
                    t.frame_ops.remove(g(1));
                    t.frame_no.insert(g(1).into());
                }
                "gbuild" => t
                    .gbuild
                    .entry(g(1).into())
                    .or_default()
                    .push(f[2..].to_vec()),
                "rule" => {
                    let (ruleno, fnno) = (num(g(4))?, num(g(5))?);
                    let list = t
                        .rules
                        .entry(g(1).into())
                        .or_default()
                        .entry(g(2).into())
                        .or_default();
                    if list.len() <= ruleno {
                        list.push(Rule {
                            name: g(3).into(),
                            fns: Vec::new(),
                            multi: fnno > 0,
                        });
                    }
                    let r = &mut list[ruleno];
                    r.fns.push(FnRule {
                        size: num(g(6))?,
                        fps: g(7).split(',').map(String::from).collect(),
                        cuts: Vec::new(),
                    });
                }
                "rcut" => {
                    let ruleno = num(g(4))?;
                    let list = t
                        .rules
                        .get_mut(g(1))
                        .and_then(|m| m.get_mut(g(2)))
                        .ok_or_else(|| {
                            RewriteError::Failed("patch table: rcut before rule".into())
                        })?;
                    let r = list
                        .get_mut(ruleno)
                        .and_then(|r| r.fns.last_mut())
                        .ok_or_else(|| {
                            RewriteError::Failed("patch table: rcut before rule".into())
                        })?;
                    r.cuts
                        .push((num(g(6))?, g(7).into(), g(8).into(), g(9).into()));
                }
                other => return fail(format!("patch table: unknown line kind {other:?}")),
            }
        }
        Ok(t)
    }
}
