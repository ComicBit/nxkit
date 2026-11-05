//! The build and compare steps of [`crate::recompile`]: store lookup, plain or side compile, cut, store write, byte
//! comparison with Python's libraries.

use crate::recompile::{Outcome, Row, Shared, row};
use nxb_model::{ContentKey, FileDigest};
use nxb_shader::side::{Unit, cut};
use nxb_shader::target::MAC;
use nxb_tool::StepCost;
use std::fs;
use std::time::Instant;

/// The libraries of one shader: `<key>_b` and, for a side shader, `<key>_bx`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Libs {
    pub(crate) b: FileDigest,
    pub(crate) bx: Option<FileDigest>,
}

/// A shader's libraries, or why there are none.
pub(crate) type Built = Result<Libs, String>;

const B: &str = "b.metallib";
const BX: &str = "bx.metallib";

/// Compile `src` and link it: (library bytes, compile cost, link cost).
fn compile_link(s: &Shared, src: &[u8]) -> Result<(Vec<u8>, StepCost, StepCost), String> {
    crate::own_front::compile_link(
        &s.tools,
        s.pch.as_deref(),
        s.own.as_ref(),
        MAC.compile_flags,
        src,
        s.cfg.io,
    )
}

/// Look the shader up in the store, else build it (side first when there is a unit, plain as fallback) and store it.
pub(crate) fn build(
    s: &Shared,
    ckey: ContentKey,
    msl: &str,
    unit: Option<&Unit>,
    key: &str,
    t: Instant,
) -> (Built, Row) {
    if let Ok(Some(m)) = s.store.lookup(&ckey) {
        let get = |n: &str| m.files.iter().find(|f| f.name == n).map(|f| f.digest);
        if let Some(b) = get(B) {
            let mut r = row(key.to_string(), Outcome::Identical, "store", t);
            let bx = get(BX);
            r.kind = if bx.is_some() { "side" } else { "plain" };
            return (Ok(Libs { b, bx }), r);
        }
    }
    let mut r = row(key.to_string(), Outcome::Identical, "compiled", t);
    let mut pieces: Option<(Vec<u8>, Option<Vec<u8>>)> = None;
    if let Some(u) = unit {
        r.msl_bytes = u.source.len() as u64;
        if let Ok((lib, c, l)) = compile_link(s, u.source.as_bytes()) {
            r.compile = c;
            r.link = l;
            let tc = Instant::now();
            if let Ok((b, bx)) = cut(&lib, &u.side()) {
                pieces = Some((b, Some(bx)));
                r.kind = "side";
            }
            r.side = tc.elapsed();
        }
    }
    if pieces.is_none() {
        r.msl_bytes = msl.len() as u64;
        match compile_link(s, msl.as_bytes()) {
            Ok((lib, c, l)) => {
                r.compile = c;
                r.link = l;
                pieces = Some((lib, None));
            }
            Err(e) => return (Err(e), r),
        }
    }
    let Some((b, bx)) = pieces else {
        return (Err("no library".into()), r);
    };
    let ts = Instant::now();
    let mut w = s.store.entry(ckey);
    let stored = (|| {
        let db = w.add_bytes(B, &b)?;
        let dbx = match &bx {
            Some(x) => Some(w.add_bytes(BX, x)?),
            None => None,
        };
        w.commit()?;
        Ok::<_, nxb_store::StoreError>(Libs { b: db, bx: dbx })
    })();
    r.store = ts.elapsed();
    (stored.map_err(|e| e.to_string()), r)
}

/// Compare with Python's `<key>_b.metallib` and `<key>_bx.metallib` (present exactly when we made one).
pub(crate) fn compare(s: &Shared, key: &str, built: &Built) -> Outcome {
    let libs = match built {
        Ok(l) => l,
        Err(m) => return Outcome::Failed(m.clone()),
    };
    let reference = |suffix: &str| s.cfg.b_dir.join(format!("{key}{suffix}.metallib"));
    let Ok(rb) = nxb_io::hash_file(&reference("_b")) else {
        return Outcome::NoReference;
    };
    let rbx = nxb_io::hash_file(&reference("_bx")).ok();
    let same = rb == libs.b && rbx == libs.bx;
    if !same {
        keep_difference(s, key, libs);
        return Outcome::Different;
    }
    Outcome::Identical
}

/// Keep up to 20 differing libraries for diagnosis.
fn keep_difference(s: &Shared, key: &str, l: &Libs) {
    let dir = s.cfg.work_dir.join("diff");
    let _ = fs::create_dir_all(&dir);
    if fs::read_dir(&dir).map_or(0, Iterator::count) >= 40 {
        return;
    }
    let _ = fs::copy(
        s.store.blob_path(&l.b.sha256),
        dir.join(format!("{key}_b.metallib")),
    );
    if let Some(x) = &l.bx {
        let _ = fs::copy(
            s.store.blob_path(&x.sha256),
            dir.join(format!("{key}_bx.metallib")),
        );
    }
}
