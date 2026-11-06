//! `dm/games/profile_guess.py`: the profile keys a game's own files decide. `facts_of` reads the code modules and the
//! RomFS file headers, `guess` turns them into profile keys, each with its reason (the line above it in `game.tsv`).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub use super::guess_facts::*;

/// One MiB.
pub const MIB: u64 = 1 << 20;
/// The application memory pool of a 4 GB Switch.
pub const APP_POOL: u64 = 0xCD50_0000;
/// A Switch 2 game's pool.
pub const OUNCE_POOL: u64 = 0x2_4000_0000;
/// The import that sets a heap.
pub const HEAP_API: &str = "_ZN2nn2os17SetMemoryHeapSizeEm";
/// The SDK components that are GPU drivers.
pub const GPU_DRIVERS: &[&str] = &["NintendoSDK_OpenGL", "NintendoSDK_Vulkan"];

/// `{key: (value, reason)}` in profile order.
#[derive(Clone, Debug, Default)]
pub struct Guess(pub Vec<(String, (String, String))>);

impl Guess {
    /// `G[k] = (v, why)`: a new key goes last, an old key keeps its place.
    pub fn set(&mut self, k: &str, v: &str, why: &str) {
        let e = (v.to_string(), why.to_string());
        match self.0.iter_mut().find(|(x, _)| x == k) {
            Some(i) => i.1 = e,
            None => self.0.push((k.to_string(), e)),
        }
    }
    /// The value and the reason of a key.
    pub fn get(&self, k: &str) -> Option<&(String, String)> {
        self.0.iter().find(|(x, _)| x == k).map(|(_, e)| e)
    }
    /// The value of a key.
    pub fn value(&self, k: &str) -> Option<&str> {
        self.get(k).map(|e| e.0.as_str())
    }
}

/// `load_order(m)`: main, the subsdks by number, sdk.
pub fn load_order(m: &str) -> (bool, bool, u64) {
    let n = m
        .strip_prefix("subsdk")
        .filter(|r| !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|r| r.parse().ok())
        .unwrap_or(0);
    (m != "main", m == "sdk", n)
}

/// Sorts names by [`load_order`] (stable).
pub fn sort_load_order(v: &mut [String]) {
    v.sort_by_key(|m| load_order(m));
}

/// `engine(F)`: keys that follow from the engine.
pub fn engine(f: &Facts) -> Vec<(&'static str, &'static str, &'static str)> {
    if f.romfs.root.contains("hagi.config") {
        vec![
            (
                "op_mode",
                "handheld",
                "Hagi (hagi.config): the Wii pointer is the touch screen in handheld mode",
            ),
            (
                "pad_layout",
                "full",
                "Hagi: a 3D game pad (D-pad, right stick)",
            ),
        ]
    } else {
        Vec::new()
    }
}

/// `guess(F, hooks)`.
pub fn guess(f: &Facts, hooks: &HashSet<String>) -> Result<Guess, String> {
    let mods = &f.mods;
    let main = f.module("main").ok_or("no main module")?;
    let imp = &main.imports;
    let (mut keep, mut drop): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let mut names: Vec<String> = mods
        .iter()
        .map(|m| m.0.clone())
        .filter(|m| m != "rtld")
        .collect();
    sort_load_order(&mut names);
    for name in &names {
        let m = f.module(name).ok_or("module")?;
        let takes = imp.intersection(&m.exports).any(|s| !hooks.contains(s));
        if name == "main" || name == "sdk" || (takes && !GPU_DRIVERS.contains(&m.kind.as_str())) {
            keep.push(name.clone());
        } else {
            drop.push(format!("{name} ({})", m.kind));
        }
    }
    let mut g = Guess::default();
    g.set(
        "modules",
        &keep.join(" "),
        &if drop.is_empty() {
            "every module main imports from".to_string()
        } else {
            format!(
                "not loaded, main takes nothing from them the host does not supply: {}",
                drop.join(", ")
            )
        },
    );
    let giant: u64 = f
        .mods
        .iter()
        .filter(|(n, m)| keep.contains(n) && m.far)
        .map(|(_, m)| m.bss as u64)
        .sum();
    let heap_api = imp.contains(HEAP_API);
    let pool = if f.ounce { OUNCE_POOL } else { APP_POOL };
    let (reserve, alias, why);
    if f.sysres != 0 {
        reserve = 0x1_4000_0000u64;
        alias = 0x1_1800_0000u64;
        why = format!(
            "the NPDM declares {} MB of system resource (virtual address memory) and main {}",
            f.sysres / MIB,
            if heap_api {
                "sets a heap: a 5 GB block with a large alias region; the heap check sizes the heap region from the heap the game asks for"
            } else {
                "never sets a heap: a 5 GB block with a 4.375 GB alias region, where the game maps its memory itself"
            }
        );
    } else {
        let heap: u64 = if heap_api {
            let need = i128::from(pool) - i128::from(64 * MIB) - i128::from(giant);
            let steps = (need + 0xfff_ffff).div_euclid(0x1000_0000);
            (steps * 0x1000_0000).max(0x1000_0000) as u64
        } else {
            0x1000_0000
        };
        reserve = 0x3000_0000 + heap;
        alias = 0x1000_0000;
        why = if heap_api {
            format!(
                "main sets a heap of at most the pool \u{2212} 64 MB{}: a {} MB heap region",
                if giant != 0 {
                    format!(
                        " \u{2212} {} MB of bss (counted as used memory)",
                        giant / MIB
                    )
                } else {
                    String::new()
                },
                heap / MIB
            )
        } else {
            "main never sets a heap (its memory is its own bss): a 256 MB heap region".to_string()
        };
    }
    g.set("mem_reserve", &format!("{reserve:#x}"), &why);
    g.set("mem_alias", &format!("{alias:#x}"), "");
    g.set("mem_stack", "0x10000000", "");
    g.set(
        "app_pool",
        &format!("0x{pool:X}"),
        if f.ounce {
            "the sdk is built for the Switch 2 (spec: Ounce): its 9 GB pool"
        } else {
            ""
        },
    );
    g.set(
        "sys_resource",
        &format!("{:#x}", f.sysres),
        &format!(
            "the NPDM declares {} MB of system resource{}",
            f.sysres / MIB,
            if f.sysres != 0 {
                ""
            } else {
                ": IsVirtualAddressMemoryEnabled is false, as on Horizon (Big Brain Academy: Brain vs. Brain sets a heap only then)"
            }
        ),
    );
    let unreal = (f.main_markers.contains("Unreal") || f.main_markers.contains("Unreal 4"))
        && f.romfs.exts.get(".pak") != 0
        && imp.contains("nvnBootstrapLoader");
    if unreal {
        g.set(
            "shader_groups",
            "ShaderArchive-Global-=global ShaderArchive-=game",
            "the decoded shader code archives: the engine's global shaders and the game's material shaders",
        );
    }
    if unreal && f.sysres != 0 {
        g.set(
            "mem_reserve",
            "0x1028000000",
            "Unreal: nninitStartup reserves a 62 GB address region and its FMallocBinned3 takes 37 GB of it, so a 64 GB alias region for the bring-up on the Mac. The rewriter cuts the allocator in any Unreal 4 / 5 main (dm/tools/ue_malloc.py) and gives the region what the alias region has room for (dm/tools/ue_region.py), so an iPhone takes a smaller guest block, as Octopath Traveler 0's profile (docs/reference/memory.md)",
        );
        g.set("mem_alias", "0x1000000000", "");
    }
    for (k, v, why) in engine(f) {
        g.set(k, v, why);
    }
    Ok(g)
}

/// `mem_used(patched, modules)` from the rewritten modules' `<m>.fixups.tsv` and `<m>.patched.bin`.
pub fn mem_used(patched: &Path, modules: &[String]) -> Result<Option<u64>, String> {
    let (mut total, mut big) = (0i128, 0i128);
    for m in modules {
        let p = patched.join(format!("{m}.fixups.tsv"));
        let t = String::from_utf8_lossy(
            &std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        )
        .into_owned();
        let mut hdr: BTreeMap<&str, &str> = BTreeMap::new();
        for l in t.split('\n') {
            if let Some(r) = l.strip_prefix("# ") {
                let k: String = r
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !k.is_empty()
                    && let Some(v) = r[k.len()..].strip_prefix('\t')
                {
                    let v = v.split(char::is_whitespace).next().unwrap_or("");
                    if !v.is_empty() {
                        hdr.insert(&r[..k.len()], v);
                    }
                }
            }
        }
        let hex = |k: &str, d: Option<&str>| -> Result<i128, String> {
            super::pyfmt::int16(
                hdr.get(k)
                    .copied()
                    .or(d)
                    .ok_or(format!("{m}.fixups.tsv: no {k}"))?,
            )
        };
        let size;
        if hex("PATCH_OFF", Some("0"))? < 0 {
            size = hex("IMAGE_SIZE", None)?;
            big += size - hex("FILE_IMAGE_LEN", None)?;
        } else {
            let b = patched.join(format!("{m}.patched.bin"));
            let len = b
                .metadata()
                .map_err(|e| format!("{}: {e}", b.display()))?
                .len();
            size = (i128::from(len) + 0x3FFF) & !0x3FFF;
        }
        total += size;
    }
    let need = ((total + 0x1F_FFFF) & !0x1F_FFFF) - big;
    Ok((need > i128::from(64 * MIB)).then(|| (need + i128::from(4 * MIB)) as u64))
}
