//! `profile_guess.py`: what a check boot's host log and address region table say about a profile (`heap_signals`,
//! `depth_signal`, `region_findings`, `stdalloc_room`). Pure text in, keys and findings out: the boot that writes the
//! log is a run of the host and is not ported.

use super::pyfmt::{int0, splitlines};
use std::collections::BTreeMap;

const MIB: u64 = 1 << 20;
/// The room the host leaves after a StandardAllocator region.
pub const STDALLOC_LATER: i128 = 0x1000_0000;
/// The SDK's node area at the start of the alias region.
pub const NODE_AREA: i128 = 0x3e0_0000;

/// A profile's keys as `game.py` gives them (name to text).
pub type Keys = BTreeMap<String, String>;
/// `{key: (value, reason)}`.
pub type Signals = Vec<(String, (String, String))>;

fn after<'a>(log: &'a str, pat: &str) -> Vec<&'a str> {
    let mut v = Vec::new();
    let mut from = 0;
    while let Some(i) = log[from..].find(pat) {
        v.push(&log[from + i + pat.len()..]);
        from += i + pat.len();
    }
    v
}

fn hex_prefix(s: &str) -> Option<(u64, usize)> {
    let r = s.strip_prefix("0x")?;
    let n = r
        .bytes()
        .take_while(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
        .count();
    (n > 0).then(|| (u64::from_str_radix(&r[..n], 16).unwrap_or(u64::MAX), 2 + n))
}

fn num(k: &Keys, key: &str, default: Option<&str>) -> Result<i128, String> {
    let v = k
        .get(key)
        .map(String::as_str)
        .or(default)
        .ok_or(format!("the profile has no {key}"))?;
    int0(v)
}

/// `depth_signal(log, g)`.
pub fn depth_signal(log: &str, g: &Keys) -> Signals {
    let mut last = None;
    for r in after(log, "[nvn] depth mode near=") {
        let ok = |n: usize| {
            !r[n..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };
        if r.starts_with('0') && ok(1) {
            last = Some("zero");
        } else if r.starts_with("-w") && ok(2) {
            last = Some("gl");
        }
    }
    let Some(seen) = last else { return Vec::new() };
    if seen == g.get("depth_mode").map_or("gl", String::as_str) {
        return Vec::new();
    }
    let near = if seen == "zero" { "0" } else { "-w" };
    vec![(
        "depth_mode".into(),
        (
            seen.into(),
            format!(
                "the game calls nvnDeviceSetDepthMode with near = {near} in the check boot: the pipeline bases made before it runs are made for that mode"
            ),
        ),
    )]
}

/// `heap_signals(log, g)`.
pub fn heap_signals(log: &str, g: &Keys) -> Result<Signals, String> {
    let mut out = Signals::new();
    let req: Vec<u64> = after(log, "SetHeapSize OOM: req ")
        .iter()
        .filter_map(|r| hex_prefix(r).map(|h| h.0))
        .collect();
    let gib = 2f64.powi(30);
    if let Some(&mx) = req.iter().max() {
        let (reserve, stack) = (num(g, "mem_reserve", None)?, num(g, "mem_stack", None)?);
        let heap = i128::from(mx.div_ceil(0x800_0000) * 0x800_0000);
        let alias = reserve - 0x1000_0000 - stack - heap;
        if alias >= 0x4000_0000 {
            out.push((
                "mem_alias".into(),
                (
                    format!("{alias:#x}"),
                    format!(
                        "the game asked for a {:.2} GB heap: a {:.3} GB heap region",
                        mx as f64 / gib,
                        heap as f64 / gib
                    ),
                ),
            ));
        } else {
            out.push((
                "mem_reserve".into(),
                (
                    format!("{:#x}", 0x1000_0000 + 0x4000_0000 + stack + heap),
                    format!("the game asked for a {:.2} GB heap", mx as f64 / gib),
                ),
            ));
        }
    }
    let top: Vec<f64> = after(log, "[mem] alias region mapped up to +")
        .iter()
        .filter_map(|r| {
            let n = r
                .bytes()
                .take_while(|c| c.is_ascii_digit() || *c == b'.')
                .count();
            (n > 0 && r[n..].starts_with(" GB"))
                .then(|| r[..n].parse::<f64>().ok())
                .flatten()
        })
        .collect();
    let alias = num(g, "mem_alias", None)? as f64 / gib;
    let mx = top.iter().copied().fold(f64::MIN, f64::max);
    if req.is_empty()
        && !log.contains("[mem] heap ")
        && num(g, "sys_resource", Some("0"))? != 0
        && !top.is_empty()
        && mx > alias - 0.25
        && num(g, "app_pool", None)? > 0x4000_0000
    {
        out.push(("app_pool".into(), ("0x40000000".into(), format!("no heap set and the alias region used up to +{mx:.2} GB of {alias:.2} GB in the heap check: a 1 GB pool, so StandardAllocator's region leaves the game room for its own"))));
    }
    // `\[mem\] SetHeapSize (0x[0-9a-f]+) \+ modules (0x[0-9a-f]+) > app pool`
    let pre = "[mem] SetHeapSize ";
    let mut from = 0;
    while let Some(i) = log[from..].find(pre) {
        let s = from + i + pre.len();
        from = s;
        let Some((_, n1)) = hex_prefix(&log[s..]) else {
            continue;
        };
        let Some(r) = log[s + n1..].strip_prefix(" + modules ") else {
            continue;
        };
        let Some((m, n2)) = hex_prefix(r) else {
            continue;
        };
        let end = s + n1 + " + modules ".len() + n2;
        if !log[end..].starts_with(" > app pool") {
            continue;
        }
        let end = end + " > app pool".len();
        if !log[end..].contains("[mem] heap ") {
            let need = (i128::from(m) + 0x1F_FFFF) & !0x1F_FFFF;
            if need > num(g, "mem_used", Some("0x4000000"))? {
                out.push(("mem_used".into(), (format!("{:#x}", need + i128::from(4 * MIB)), format!("the heap check failed for good: {:.0} MB of modules, more than the used memory reported", need as f64 / MIB as f64))));
            }
        }
        break;
    }
    Ok(out)
}

/// One row of the host's address region table.
#[derive(Clone, Debug)]
pub struct Region {
    /// Offset in the alias region.
    pub off: i128,
    /// Size.
    pub size: i128,
    /// Who asked (the return address).
    pub who: String,
    /// Bytes mapped.
    pub mapped: i128,
    /// How far it was opened up to.
    pub end: Option<i128>,
    /// The return address of the request.
    pub via: i128,
    /// The SDK API the request came through.
    pub api: Option<String>,
}

/// `demangle(name)`.
pub fn demangle(name: &str) -> String {
    let Some(r) = name.strip_prefix("_ZN") else {
        return name.to_string();
    };
    let Some(e) = r.find('E') else {
        return name.to_string();
    };
    let (mut t, mut parts) = (&r[..e], Vec::new());
    loop {
        let d = t.bytes().take_while(u8::is_ascii_digit).count();
        if d == 0 {
            break;
        }
        let n: usize = t[..d].parse().unwrap_or(0);
        let rest = &t[d..];
        let cut = rest.char_indices().nth(n).map_or(rest.len(), |x| x.0);
        parts.push(&rest[..cut]);
        t = &rest[cut..];
    }
    if parts.is_empty() {
        name.to_string()
    } else {
        parts.join("::")
    }
}

/// `region_rows(table, sdk_syms)`.
pub fn region_rows(table: &str, sdk_syms: &[(String, (u64, u64))]) -> Result<Vec<Region>, String> {
    let mut rows = Vec::new();
    for l in splitlines(table) {
        let c: Vec<&str> = l.split('\t').collect();
        if l.starts_with('#') || c.len() != 7 {
            continue;
        }
        rows.push(Region {
            off: int0(c[0])?,
            size: int0(c[1])?,
            who: c[2].to_string(),
            mapped: int0(c[3])?,
            end: if c[5] == "-" { None } else { Some(int0(c[5])?) },
            via: int0(c[6])?,
            api: None,
        });
    }
    for r in &mut rows {
        r.api = if r.via == 0 {
            None
        } else {
            sdk_syms
                .iter()
                .find(|(_, (a, n))| {
                    i128::from(*a) <= r.via && r.via < i128::from(*a) + i128::from((*n).max(4))
                })
                .map(|(name, _)| demangle(name))
        };
    }
    Ok(rows)
}

/// `stdalloc_room(rows, r, alias)`: (the regions made after it that fit by themselves, the cap that leaves them room).
pub fn stdalloc_room(rows: &[Region], r: &Region, alias: i128) -> (i128, Option<i128>) {
    let later: i128 = rows
        .iter()
        .filter(|x| x.off > r.off && x.size <= alias - NODE_AREA)
        .map(|x| x.size)
        .sum();
    let room = (alias - r.off - later - STDALLOC_LATER) & !0x1ff_ffff;
    (later, (room > r.mapped.max(0x200_0000)).then_some(room))
}

fn is_std(r: &Region) -> bool {
    r.api
        .as_ref()
        .is_some_and(|a| a.contains("StandardAllocator"))
}

/// `stdalloc_cap_needed(rows, alias)`.
pub fn stdalloc_cap_needed(rows: &[Region], alias: i128) -> Option<i128> {
    rows.iter()
        .filter(|r| is_std(r) && r.off + r.size > alias)
        .map(|r| stdalloc_room(rows, r, alias))
        .filter(|(later, n)| *later > STDALLOC_LATER && n.is_some())
        .filter_map(|(_, n)| n)
        .min()
}

fn gb(n: i128) -> String {
    format!("{:.2} GB", n as f64 / 2f64.powi(30))
}

/// `region_findings(table, alias, sdk_syms, secs, cap)`: `(red, text)` lines.
pub fn region_findings(
    table: &str,
    alias: i128,
    sdk_syms: &[(String, (u64, u64))],
    secs: u32,
    cap: i128,
) -> Result<Vec<(bool, String)>, String> {
    let rows = region_rows(table, sdk_syms)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let (mut out, mut red, mut own): (Vec<(bool, String)>, Vec<String>, Vec<&Region>) =
        (Vec::new(), Vec::new(), Vec::new());
    let mut big: Vec<&Region> = rows.iter().filter(|r| r.size >= 0x1000_0000).collect();
    big.sort_by_key(|r| r.off);
    for r in big {
        let use_ = match r.end {
            None => "not used yet".to_string(),
            Some(e) => format!("opened up to +{}", gb(e)),
        };
        let line = format!(
            "{} at alias +{}{}, returning to {}: {} mapped, {use_}",
            gb(r.size),
            gb(r.off),
            r.api
                .as_ref()
                .map_or(String::new(), |a| format!(" through {a}")),
            r.who,
            gb(r.mapped)
        );
        if r.off + r.size <= alias {
            out.push((false, line));
            continue;
        }
        if is_std(r) {
            let (later, need) = stdalloc_room(&rows, r, alias);
            let mut note = String::new();
            let needs = need.map_or(String::new(), |n| {
                format!(": stdalloc_cap {n:#x} ({})", gb(n))
            });
            if cap != 0 && r.off + cap + later <= alias {
                note = format!(
                    "; the profile's stdalloc_cap gives it {}, which leaves the {} of regions made after it room",
                    gb(cap),
                    gb(later)
                );
            } else if cap != 0 {
                red.push(format!("the profile's stdalloc_cap {cap:#x} leaves the {} of regions made after the one returning to {} too little room{needs}", gb(later), r.who));
            } else if later <= STDALLOC_LATER {
                note = format!(
                    "; on the iPhone layout the host gives it {}, what the alias region has room for",
                    gb((alias - r.off - STDALLOC_LATER) & !0x1ff_ffff)
                );
            } else if let Some(n) = need {
                red.push(format!("the regions made after the one returning to {} take {}, more than the {} the host leaves them: stdalloc_cap {n:#x} ({})", r.who, gb(later), gb(STDALLOC_LATER), gb(n)));
            } else {
                red.push(format!("the regions made after the one returning to {} take {}, which leaves it no room", r.who, gb(later)));
            }
            out.push((false, format!("{line}{note}")));
        } else {
            out.push((false, line));
            if r.size > alias - NODE_AREA {
                own.push(r);
            }
        }
    }
    for r in own {
        red.push(format!(
            "the request returning to {} takes {}{}: a smaller one works only if its allocator places nothing by fixed offsets",
            r.who,
            gb(r.size),
            r.api.as_ref().map_or(String::new(), |a| format!(" through {a}"))
        ));
    }
    let reach = rows.iter().map(|r| r.off + r.size).max().unwrap_or(0);
    let where_ = format!(
        "in its first {secs} s in nnMain the game's regions reach alias +{}",
        gb(reach)
    );
    let capped = rows.iter().any(|r| is_std(r) && r.off + r.size > alias);
    if red.is_empty() && reach > alias && !capped {
        red.push("no single region is too big, but together they need more room than the alias region has".into());
    }
    if !red.is_empty() {
        out.push((true, format!("{where_}, past the profile's {} alias region, the iPhone layout: {} (docs/reference/memory.md, Virtual address memory and the alias region)", gb(alias), red.join("; "))));
    } else if reach > alias {
        out.push((false, format!("{where_}, past the profile's {} alias region only by StandardAllocator regions, which the host makes fit (what it takes later in play is not measured here)", gb(alias))));
    } else {
        out.push((false, format!("{where_}, within the profile's {} alias region (what it takes later in play is not measured here)", gb(alias))));
    }
    Ok(out)
}
