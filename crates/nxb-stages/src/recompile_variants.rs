//! Variant libraries of [`crate::recompile`]: `<key>_bp` and `<key>_bv` of vertex shaders, `<key>_bf` of pixel shaders,
//! made from `<key>_b.metal` (and the reflection's bindings for `_bf`) as `dmsh_build.pull_finish`, `vin_finish` and
//! `fetch_finish` do, compiled with the plain convention, compared with Python's library and reflection field.

use crate::recompile::{Outcome, Row, Shared, push, row};
use crate::recompile_build::{Built, build};
use nxb_model::ContentKey;
use nxb_pyjson::{Value, dumps};
use nxb_sched::Claim;
use nxb_shader::target::MAC;
use nxb_shader::variants::{fetch, pull, vin};
use std::fs;
use std::sync::Arc;
use std::time::Instant;

/// One variant kind.
#[derive(Clone, Copy)]
enum Kind {
    Pull,
    Vin,
    Fetch,
}

impl Kind {
    fn suffix(self) -> &'static str {
        match self {
            Kind::Pull => "_bp",
            Kind::Vin => "_bv",
            Kind::Fetch => "_bf",
        }
    }

    fn field(self) -> &'static str {
        match self {
            Kind::Pull => "pull",
            Kind::Vin => "vin",
            Kind::Fetch => "fetch",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Pull => "bp",
            Kind::Vin => "bv",
            Kind::Fetch => "bf",
        }
    }
}

/// The variant source and the reflection field Python writes for it.
fn make(kind: Kind, msl: &str, refl: &Value) -> Option<(String, Value)> {
    let empty = Value::List(Vec::new());
    match kind {
        Kind::Pull => pull::variant(msl).ok().map(|(src, info)| {
            // pull_finish keeps the members that are not None.
            let kept = match info {
                Value::Object(m) => {
                    Value::Object(m.into_iter().filter(|(_, v)| *v != Value::Null).collect())
                }
                v => v,
            };
            (src, kept)
        }),
        Kind::Vin => vin::variant(msl).ok().map(|(src, shadows)| {
            let field = Value::obj([
                ("entry", Value::str("main0")),
                ("entry_z", Value::str("main0_z")),
                ("shadow", shadows),
            ]);
            (src, field)
        }),
        Kind::Fetch => {
            let bindings = refl.get("bindings").unwrap_or(&empty);
            fetch::variant(msl, bindings, false).ok()
        }
    }
}

/// Build and compare the variants that apply to this shader's stage.
pub(crate) fn variant_tasks(s: &Arc<Shared>, key: String) {
    let t = Instant::now();
    let read = |suffix: &str| fs::read_to_string(s.cfg.b_dir.join(format!("{key}{suffix}")));
    let (Ok(msl), Ok(json)) = (read("_b.metal"), read("_b.json")) else {
        return;
    };
    let Ok(refl) = nxb_pyjson::parse(&json) else {
        return push(
            s,
            row(
                key,
                Outcome::Failed("unreadable _b.json".into()),
                "compiled",
                t,
            ),
        );
    };
    let kinds: &[Kind] = match refl.get("stage").and_then(Value::as_str) {
        Some("vs") => &[Kind::Pull, Kind::Vin],
        Some("ps") => &[Kind::Fetch],
        _ => &[],
    };
    for &kind in kinds {
        one(s, &key, kind, &msl, &refl, t);
    }
}

fn one(s: &Arc<Shared>, key: &str, kind: Kind, msl: &str, refl: &Value, t: Instant) {
    let reference = s.cfg.b_dir.join(format!("{key}{}.metallib", kind.suffix()));
    let ref_field = refl.get(kind.field()).map(dumps);
    let Some((src, field)) = make(kind, msl, refl) else {
        // No variant: Python has no library and no reflection field either.
        let same = !reference.exists() && ref_field.is_none();
        let mut r = row(
            key.to_string(),
            if same {
                Outcome::Identical
            } else {
                Outcome::Different
            },
            "none",
            t,
        );
        r.kind = kind.label();
        return push(s, r);
    };
    let mut kb = ContentKey::builder("nxb.metallib.v2")
        .field_str("record", &MAC.record())
        .field_str("compiler", &s.compiler_id)
        .field("msl", src.as_bytes());
    if let Some(o) = &s.own {
        kb = kb.field_str("frontend", o.key_field(src.as_bytes()));
    }
    let ckey = kb.finish();
    let field_text = dumps(&field);
    let check = move |s: &Shared, key: &str, built: &Built| -> Outcome {
        let lib = match built {
            Ok(l) => l.b,
            // Python drops library and field when the variant does not compile.
            Err(_) => {
                return if !reference.exists() && ref_field.is_none() {
                    Outcome::Identical
                } else {
                    Outcome::Different
                };
            }
        };
        let _ = (s, key);
        match nxb_io::hash_file(&reference) {
            Ok(d) if d == lib && ref_field.as_deref() == Some(field_text.as_str()) => {
                Outcome::Identical
            }
            Ok(_) => Outcome::Different,
            Err(_) => Outcome::NoReference,
        }
    };
    let check = Arc::new(check);
    let follower = {
        let (s2, key2, check2) = (s.clone(), key.to_string(), check.clone());
        move |b: Option<&Built>| {
            let out = b.map_or(Outcome::Failed("leader abandoned".into()), |b| {
                check2(&s2, &key2, b)
            });
            let mut r = row(key2, out, "joined", t);
            r.kind = kind.label();
            push(&s2, r);
        }
    };
    let Claim::Leader(leader) = s.flights.claim(ckey, follower) else {
        return;
    };
    let (built, mut r): (Built, Row) = build(s, ckey, &src, None, key, t);
    let tc = Instant::now();
    r.outcome = check(s, key, &built);
    r.compare = tc.elapsed();
    r.kind = kind.label();
    leader.complete(built);
    r.task_wall = t.elapsed();
    push(s, r);
}
