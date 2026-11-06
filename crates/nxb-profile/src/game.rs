//! The typed [`Profile`] and the engine's `game.tsv`: what each key of one is in the other.
//!
//! `game.tsv` keys that the files of a title decide land in `[computed]` (`app_pool` as `pool_size`, `mem_used`,
//! `shader_groups`, `pad_layout`, `sys_resource` as `address_space`.`modules`, `mem_reserve`, `mem_alias`,
//! `mem_stack` and `sys_resource` as written, in `extra`). `name` and `op_mode` are `[declared]`. Everything else of a
//! `game.tsv` stays in the file: the TSV, not the typed profile, is the lossless form (see [`crate::tsv`]).

use crate::Profile;
use crate::toml::Value;
use crate::tsv::TsvFile;

/// `int(value, 0)` for the numbers a profile writes (`0x...`, decimal).
pub fn parse_int(s: &str) -> Option<u64> {
    let t = s.trim();
    match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16).ok(),
        None => t.parse().ok(),
    }
}

/// The `extra` keys of `[computed]` that are `game.tsv` keys written as they are.
const RAW_KEYS: &[&str] = &[
    "modules",
    "mem_reserve",
    "mem_alias",
    "mem_stack",
    "sys_resource",
];

impl Profile {
    /// The typed profile of a `game.tsv`.
    pub fn from_game_tsv(t: &TsvFile) -> Profile {
        let mut p = Profile {
            title_id: t.get("title_id"),
            ..Profile::default()
        };
        p.declared.title = t.get("name");
        p.declared.op_mode = t.get("op_mode");
        p.computed.pool_size = t.get("app_pool").as_deref().and_then(parse_int);
        p.computed.mem_used = t.get("mem_used").as_deref().and_then(parse_int);
        p.computed.shader_groups = t.words("shader_groups");
        p.computed.pad_layout = t.get("pad_layout");
        if let Some(s) = t.get("sys_resource") {
            let virt = parse_int(&s).is_some_and(|v| v != 0);
            p.computed.address_space = Some(if virt { "virtual" } else { "horizon" }.to_string());
        }
        for k in RAW_KEYS {
            if let Some(v) = t.get(k) {
                p.computed.extra.insert((*k).to_string(), Value::Str(v));
            }
        }
        p
    }

    /// Sets the `game.tsv` keys of `[computed]` (the ones [`Profile::from_game_tsv`] reads) in `t`: a key the profile
    /// has no value for is left as it is.
    pub fn computed_into_game_tsv(&self, t: &mut TsvFile) {
        let c = &self.computed;
        for k in RAW_KEYS {
            if let Some(Value::Str(v)) = c.extra.get(*k) {
                t.set(k, &[v]);
            }
        }
        if let Some(v) = c.pool_size {
            t.set("app_pool", &[&format!("{v:#x}")]);
        }
        if let Some(v) = c.mem_used {
            t.set("mem_used", &[&format!("{v:#x}")]);
        }
        if !c.shader_groups.is_empty() {
            t.set("shader_groups", &[&c.shader_groups.join(" ")]);
        }
        if let Some(v) = &c.pad_layout {
            t.set("pad_layout", &[v]);
        }
    }
}
