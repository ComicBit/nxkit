//! Byte-for-byte comparison with the reference script on real modules. Skipped without `NXB_REWRITE_CASES`:
//! a file of lines `profile<TAB>module file<TAB>module name<TAB>reference output folder`, with
//! `NXB_REWRITE_PATCHES` the patch snapshot (`tools/gen_rewrite_patches.py`) or `NXB_REWRITE_RULES` a folder of rule files.
//! Without both, the rules in `data/rewrite-rules` are used.

use nxb_rewrite::{Options, patches::PatchTable, rewrite};

#[test]
fn rewrite_equals_the_reference_script() {
    let Some(cases) = std::env::var_os("NXB_REWRITE_CASES") else {
        eprintln!("skipped: NXB_REWRITE_CASES not set");
        return;
    };
    // the registered patches: the rule files of NXB_REWRITE_RULES (a folder of *.toml), else the snapshot
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/rewrite-rules");
    let table = if let Some(dir) = std::env::var_os("NXB_REWRITE_RULES").or_else(|| {
        (std::env::var_os("NXB_REWRITE_PATCHES").is_none() && shipped.is_dir())
            .then(|| shipped.clone().into_os_string())
    }) {
        let mut files = Vec::new();
        for e in std::fs::read_dir(dir).expect("rules folder").flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".toml") {
                files.push((n, std::fs::read_to_string(e.path()).expect("rule file")));
            }
        }
        PatchTable::from_toml(&files).expect("rule files")
    } else if let Some(pt) = std::env::var_os("NXB_REWRITE_PATCHES") {
        PatchTable::parse(&std::fs::read_to_string(pt).unwrap_or_default()).unwrap_or_default()
    } else {
        eprintln!("skipped: NXB_REWRITE_RULES or NXB_REWRITE_PATCHES not set");
        return;
    };
    let only = std::env::var("NXB_REWRITE_ONLY").unwrap_or_default();
    let mut bad = 0;
    for line in std::fs::read_to_string(cases).unwrap_or_default().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 || (!only.is_empty() && !line.contains(&only)) {
            continue;
        }
        let nso = std::fs::read(f[1]).unwrap_or_default();
        let opts = Options {
            name: f[2].into(),
            profile: f[0].into(),
            ..Options::default()
        };
        let t = std::time::Instant::now();
        let out = match rewrite(&nso, &opts, &table) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("{} {}: ERROR {e}", f[0], f[2]);
                bad += 1;
                continue;
            }
        };
        if let Some(d) = std::env::var_os("NXB_REWRITE_DUMP") {
            let _ = std::fs::create_dir_all(&d);
            for (name, data) in &out.files {
                if true {
                    let _ = std::fs::write(std::path::Path::new(&d).join(name), data);
                }
            }
        }
        let mut notes = Vec::new();
        for (name, data) in &out.files {
            if name == "build_src.sha256" {
                continue;
            }
            let want = std::fs::read(std::path::Path::new(f[3]).join(name)).ok();
            match want {
                Some(w) if w == *data => {}
                Some(w) => {
                    let at = w
                        .iter()
                        .zip(data.iter())
                        .position(|(a, b)| a != b)
                        .unwrap_or(w.len().min(data.len()));
                    notes.push(format!(
                        "{name} differs at {at:#x} (lengths {} / {})",
                        w.len(),
                        data.len()
                    ));
                }
                None => notes.push(format!("{name} has no reference")),
            }
        }
        // a file of the reference that this port does not write is a difference too
        if let Ok(rd) = std::fs::read_dir(f[3]) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n != "log.txt"
                    && n != "build_src.sha256"
                    && !out.files.iter().any(|(m, _)| *m == n)
                {
                    notes.push(format!("{n} is not written"));
                }
            }
        }
        eprintln!(
            "{} {}: {} in {:.1}s {}",
            f[0],
            f[2],
            if notes.is_empty() {
                "IDENTICAL"
            } else {
                "DIFFERS"
            },
            t.elapsed().as_secs_f32(),
            notes.join("; ")
        );
        for l in &out.log {
            eprintln!("    {l}");
        }
        bad += usize::from(!notes.is_empty());
    }
    assert_eq!(bad, 0);
}
