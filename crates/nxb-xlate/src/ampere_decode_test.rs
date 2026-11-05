//! Corpus test of the Ampere decoder: every Ampere job decodes (needs `NXB_XLATE_CORPUS`, a folder with `jobs`).
#![cfg(test)]

mod tests {
    use crate::accessor::Accessor;
    use crate::attr::Stage;
    use crate::job::parse_jobs;
    use crate::maxwell::ampere_decode::decode;

    #[test]
    fn corpus_decodes() {
        let Some(dir) = std::env::var_os("NXB_XLATE_CORPUS") else {
            return;
        };
        let Ok(rd) = std::fs::read_dir(std::path::Path::new(&dir).join("jobs")) else {
            return;
        };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        let (mut jobs, mut blocks, mut ops, mut funcs) = (0u64, 0u64, 0u64, 0u64);
        let mut errors = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap_or_default();
            for job in parse_jobs(&text).unwrap_or_default() {
                if !job.arch_sm86 {
                    continue;
                }
                let p = std::path::Path::new(&dir).join("..").join(&job.code_path);
                let Ok(code) = std::fs::read(p) else {
                    continue;
                };
                let mut acc = Accessor::new(&job);
                let (stage, start) = match job.stage.as_str() {
                    "vs" => (Stage::Vertex, 0x80),
                    "ps" => (Stage::Fragment, 0x80),
                    _ => (Stage::Compute, 0),
                };
                jobs += 1;
                match decode(stage, &mut acc, &code, start) {
                    Ok((p, _)) => {
                        funcs += p.functions.len() as u64;
                        for f in &p.functions {
                            blocks += f.blocks.len() as u64;
                            ops += f.blocks.iter().map(|b| b.ops.len() as u64).sum::<u64>();
                        }
                    }
                    Err(e) => errors.push(format!("{}: {e}", job.key)),
                }
            }
        }
        eprintln!(
            "ampere decode: {jobs} jobs, {funcs} functions, {blocks} blocks, {ops} ops, {} errors",
            errors.len()
        );
        assert!(errors.is_empty(), "{:?}", &errors[..errors.len().min(5)]);
    }
}
