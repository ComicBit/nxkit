//! `dm/ios/tools/gen_embed.py`: the assembly that compiles the rewritten modules into the iOS executable, and the
//! link flags (`-segprot` for the writable segments). Each module is cut into 16 KiB aligned pieces: the original text
//! (`__TEXT,__dm<tag><name>x0`), the writable rest (`__DM<tag><name>W,__img`), the patch area (`x1`) and, for a module
//! with a back patch area, `x2`. The NRO modules of `nro.tsv` are packed into three shared sections with a table
//! (`__DATA,__dm<tag>_nro_tab`), and an `nn::jit` image goes into `__TEXT,__dm<tag>_jit`. Text equal to the script's,
//! byte for byte, including its checks (an `AssertionError` there is an `Err` here).

use super::abspath;
use crate::host::pyfmt::{hex, int16, is_space};
use std::path::Path;

const PG: i128 = 0x4000;

fn r16(x: i128) -> i128 {
    (x + PG - 1) & !(PG - 1)
}

/// The lines of a text file as Python's text mode iterates them (universal newlines, each with its `\n`).
fn py_lines(text: &str) -> Vec<String> {
    let t = text.replace("\r\n", "\n").replace('\r', "\n");
    t.split_inclusive('\n').map(str::to_string).collect()
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

/// One module: the absolute path of `<name>.patched.bin`, its size and its pieces as (file start, end).
struct Pieces {
    bin: String,
    size: i128,
    parts: Vec<(i128, i128)>,
}

fn check(ok: bool, what: &str, n: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("gen_embed: assertion failed for {n}: {what}"))
    }
}

fn pieces(out: &str, n: &str) -> Result<Pieces, String> {
    let mut hdr: Vec<(String, String)> = Vec::new();
    for l in py_lines(&read(&format!("{out}/{n}.fixups.tsv"))?) {
        let rest: String = l.chars().skip(2).collect();
        let w: Vec<&str> = rest.split(is_space).filter(|w| !w.is_empty()).collect();
        let [k, v] = w[..] else {
            return Err(format!(
                "gen_embed: {out}/{n}.fixups.tsv: a line that is not '# KEY VALUE': {l:?}"
            ));
        };
        match hdr.iter_mut().find(|(n, _)| n == k) {
            Some(slot) => slot.1 = v.to_string(),
            None => hdr.push((k.to_string(), v.to_string())),
        }
    }
    let get = |k: &str| -> Result<i128, String> {
        let v = hdr
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
            .ok_or_else(|| format!("gen_embed: {out}/{n}.fixups.tsv has no {k}"))?;
        int16(v)
    };
    let (te, po) = (get("TEXT_END")?, get("PATCH_OFF")?);
    let (ko, ks) = (get("KEY_PAGE_OFF")?, get("KEY_PAGE_SIZE")?);
    let bin = abspath(&format!("{out}/{n}.patched.bin"))?;
    let size = std::fs::metadata(&bin)
        .map_err(|e| format!("{bin}: {e}"))?
        .len() as i128;
    let (a, end) = (r16(te), r16(size));
    if po < 0 {
        let fl = get("FILE_IMAGE_LEN")?;
        let bl = if hdr.iter().any(|(k, _)| k == "BACK_LEN") {
            get("BACK_LEN")?
        } else {
            0
        };
        check(
            fl % PG == 0
                && a <= fl
                && ko % PG == 0
                && ks == PG
                && ko >= fl
                && bl % PG == 0
                && (bl == 0 || size % PG == 0),
            "far layout",
            n,
        )?;
        let mut parts = vec![(0, a), (a, fl), (fl, end - bl)];
        if bl != 0 {
            parts.push((end - bl, end));
        }
        return Ok(Pieces { bin, size, parts });
    }
    check(
        po % PG == 0 && a <= ko && ko + ks <= po && ko % PG == 0 && ks == PG,
        "layout",
        n,
    )?;
    Ok(Pieces {
        bin,
        size,
        parts: vec![(0, a), (a, po), (po, end)],
    })
}

/// One piece, 16 KiB aligned at both ends.
fn place(asm: &mut Vec<String>, bin: &str, size: i128, lo: i128, hi: i128) {
    let fill = hi.min(size) - lo;
    asm.push("    .p2align 14".into());
    asm.push(format!("    .incbin \"{bin}\", {}, {}", hex(lo), hex(fill)));
    if hi - lo > fill {
        asm.push(format!("    .space {}", hex(hi - lo - fill)));
    }
    asm.push("    .p2align 14".into());
}

/// The assembly text and the link flags.
#[derive(Clone, Debug, PartialEq)]
pub struct Embed {
    /// The `.S` file's text.
    pub asm: String,
    /// The link flags (the script prints them joined by spaces).
    pub flags: Vec<String>,
}

/// The NRO modules `nro.tsv` in `out` lists (not comments, blank lines or `refused` rows), as the script reads them.
pub fn nro_names(out: &str) -> Result<Vec<String>, String> {
    let tab = format!("{out}/nro.tsv");
    if !Path::new(&tab).exists() {
        return Ok(Vec::new());
    }
    Ok(py_lines(&read(&tab)?)
        .into_iter()
        .filter(|l| {
            !l.trim_matches(is_space).is_empty()
                && !l.starts_with('#')
                && !l.starts_with("refused\t")
        })
        .map(|l| l.split('\t').next().unwrap_or("").to_string())
        .collect())
}

/// `gen_embed.py [--tag <tag>] [--jit <jit.img>] <out> <dst> <names...>` without the writing: `out` is the rewriter's
/// output folder (as given: the script joins it as text), `names` the modules.
pub fn generate(
    out: &str,
    names: &[String],
    tag: &str,
    jit: Option<&str>,
) -> Result<Embed, String> {
    let mut asm = vec!["// Generated by dm/ios/tools/gen_embed.py; do not edit".to_string()];
    let mut flags = Vec::new();
    for n in names {
        let p = pieces(out, n)?;
        let sects = [
            format!("__TEXT,__dm{tag}{n}x0"),
            format!("__DM{tag}{n}W,__img"),
            format!("__TEXT,__dm{tag}{n}x1"),
            format!("__TEXT,__dm{tag}{n}x2"),
        ];
        for (sect, &(lo, hi)) in sects.iter().zip(&p.parts) {
            check(
                sect.split(',').all(|x| x.chars().count() <= 16),
                "section name of at most 16 characters",
                n,
            )?;
            asm.push(format!("    .section {sect}"));
            place(&mut asm, &p.bin, p.size, lo, hi);
        }
        flags.push(format!("-Wl,-segprot,__DM{tag}{n}W,rw,rw"));
    }
    let nros = nro_names(out)?;
    if !nros.is_empty() {
        let mut at = [0i128; 3];
        let mut rows: Vec<(String, Vec<i128>)> = Vec::new();
        let sects = [
            format!("__TEXT,__dm{tag}_nro_x0"),
            format!("__DM{tag}_NRO_W,__img"),
            format!("__TEXT,__dm{tag}_nro_x1"),
        ];
        for (i, sect) in sects.iter().enumerate() {
            asm.push(format!("    .section {sect}"));
            for (k, n) in nros.iter().enumerate() {
                let p = pieces(out, n)?;
                let (lo, hi) = p.parts[i];
                check(
                    p.parts.len() == 3,
                    "an NRO with a back patch area, which the packed NRO sections do not hold",
                    n,
                )?;
                if i == 0 {
                    rows.push((n.clone(), Vec::new()));
                }
                rows[k].1.extend([at[i], hi - lo]);
                at[i] += hi - lo;
                place(&mut asm, &p.bin, p.size, lo, hi);
            }
        }
        asm.push(format!("    .section __DATA,__dm{tag}_nro_tab"));
        asm.push("    .p2align 3".into());
        for (name, vals) in &rows {
            let len = name.chars().count();
            check(len < 32, "an NRO name under 32 characters", name)?;
            asm.push(format!("    .ascii \"{name}\""));
            asm.push(format!("    .space {}", 32 - len));
            let v: Vec<String> = vals.iter().map(|&v| hex(v)).collect();
            asm.push(format!("    .quad {}", v.join(", ")));
        }
        flags.push(format!("-Wl,-segprot,__DM{tag}_NRO_W,rw,rw"));
    }
    if let Some(j) = jit {
        let j = abspath(j)?;
        let size = std::fs::metadata(&j)
            .map_err(|e| format!("{j}: {e}"))?
            .len() as i128;
        asm.push(format!("    .section __TEXT,__dm{tag}_jit"));
        place(&mut asm, &j, size, 0, r16(size));
    }
    Ok(Embed {
        asm: asm.join("\n") + "\n",
        flags,
    })
}

/// [`generate`] and write `dst`; returns the flags line (what `build_ipa.sh` takes from the script's output).
pub fn write(
    out: &str,
    dst: &Path,
    names: &[String],
    tag: &str,
    jit: Option<&str>,
) -> Result<String, String> {
    let e = generate(out, names, tag, jit)?;
    std::fs::write(dst, &e.asm).map_err(|err| format!("{}: {err}", dst.display()))?;
    Ok(e.flags.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ios::oracle;
    use std::path::PathBuf;

    /// A rewriter output folder: `main` (patch area after the image), `far` (patch area before the base, with a back
    /// patch area), `nroa` and `nrob` (NRO modules in `nro.tsv`, beside a comment, a blank line and a refused row).
    fn fixture(d: &Path) -> PathBuf {
        let out = d.join("patched");
        std::fs::create_dir_all(&out).unwrap();
        let module = |n: &str, hdr: &str, size: usize| {
            std::fs::write(out.join(format!("{n}.fixups.tsv")), hdr).unwrap();
            std::fs::write(out.join(format!("{n}.patched.bin")), vec![7u8; size]).unwrap();
        };
        module(
            "main",
            "# PATCH_OFF\t0xc000\n# KEY_PAGE_OFF\t0x8000\n# KEY_PAGE_SIZE\t0x4000\n# TEXT_END\t0x5010\n# BUILD_ID\tab\n",
            0xe123,
        );
        module(
            "far",
            "# PATCH_OFF\t-0x10000\n# KEY_PAGE_OFF\t0x8000\n# KEY_PAGE_SIZE\t0x4000\n# TEXT_END\t0x3000\n# FILE_IMAGE_LEN\t0x8000\n# BACK_LEN\t0x4000\n",
            0x14000,
        );
        for n in ["nroa", "nrob"] {
            module(
                n,
                "# PATCH_OFF\t0x8000\r\n# KEY_PAGE_OFF\t0x4000\r\n# KEY_PAGE_SIZE\t0x4000\r\n# TEXT_END\t0x10\r\n",
                0x9001,
            );
        }
        std::fs::write(
            out.join("nro.tsv"),
            "# module\tsource\nnroa\tx.nro\n\nrefused\tnn_online\nnrob\ty.nro\n",
        )
        .unwrap();
        std::fs::write(d.join("jit.img"), vec![1u8; 0x4001]).unwrap();
        out
    }

    #[test]
    fn layouts_tables_and_flags() {
        let d = oracle::scratch("embed");
        let out = fixture(&d);
        let o = out.to_str().unwrap();
        let e = generate(o, &["main".into(), "far".into()], "p1", None).unwrap();
        assert_eq!(
            e.flags,
            [
                "-Wl,-segprot,__DMp1mainW,rw,rw",
                "-Wl,-segprot,__DMp1farW,rw,rw",
                "-Wl,-segprot,__DMp1_NRO_W,rw,rw"
            ]
        );
        let main = format!("{o}/main.patched.bin");
        assert!(e.asm.contains(&format!(
            "    .section __TEXT,__dmp1mainx1\n    .p2align 14\n    .incbin \"{main}\", 0xc000, 0x2123\n    .space 0x1edd\n"
        )));
        assert!(e.asm.contains("    .section __TEXT,__dmp1farx2\n"));
        assert!(e.asm.contains(
            "    .ascii \"nrob\"\n    .space 28\n    .quad 0x4000, 0x4000, 0x4000, 0x4000, 0x4000, 0x4000\n"
        ));
        // a section name over 16 characters is refused, as the script's assertion does
        assert!(generate(o, &["main".into()], "p1234567890", None).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    fn py(e: &Path, args: &[&str]) -> String {
        let o = oracle::python()
            .arg(e.join("dm/ios/tools/gen_embed.py"))
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim_end().to_string()
    }

    /// Oracle: the engine's `gen_embed.py` on the fixture (with and without a tag and a jit image), and on a real
    /// rewriter output when `NXB_ORACLE_PATCHED` names one (`<folder>:<module> <module>...`): `.S` and flags equal.
    #[test]
    fn equals_the_reference_script() {
        let Some(e) = oracle::engine() else { return };
        let d = oracle::scratch("embed-oracle");
        let out = fixture(&d);
        let o = out.to_str().unwrap().to_string();
        let jit = d.join("jit.img").to_str().unwrap().to_string();
        let mut cases: Vec<(String, Vec<String>, String, Option<String>)> = vec![
            (
                o.clone(),
                vec!["main".into(), "far".into()],
                String::new(),
                None,
            ),
            (o.clone(), vec!["far".into()], "p0".into(), Some(jit)),
        ];
        if let Ok(spec) = std::env::var("NXB_ORACLE_PATCHED") {
            let (dir, mods) = spec.split_once(':').unwrap();
            cases.push((
                dir.into(),
                mods.split(' ').map(String::from).collect(),
                String::new(),
                None,
            ));
        }
        for (i, (out, names, tag, jit)) in cases.iter().enumerate() {
            let (pd, rd) = (d.join(format!("py{i}.S")), d.join(format!("rs{i}.S")));
            let mut a: Vec<&str> = Vec::new();
            if !tag.is_empty() {
                a.extend(["--tag", tag]);
            }
            if let Some(j) = jit {
                a.extend(["--jit", j]);
            }
            a.extend([out.as_str(), pd.to_str().unwrap()]);
            a.extend(names.iter().map(String::as_str));
            let pf = py(&e, &a);
            let rf = write(out, &rd, names, tag, jit.as_deref()).unwrap();
            assert_eq!(rf, pf);
            assert_eq!(std::fs::read(&rd).unwrap(), std::fs::read(&pd).unwrap());
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
