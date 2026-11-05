//! The own front end in process (design-v2 16): `air_front::compile` turns MSL into AIR bitcode
//! without a process, then Apple's back half (`air-opt`, `metallib`) finishes the library.
//!
//! Off by default. It compiles only shaders the verdict file allows (per-shader verdicts of the emitter project's
//! validation, keyed by the SHA-256 of the MSL), every other shader and every reject goes to Apple's `metal -c`.
//! With only `own-identical` allowed the libraries are byte-identical to Apple's.

use crate::apple::{Io, MetalTools, StepCost};
use crate::frontend::{FrontEnd, FrontEndError, FrontEndOutput};
use crate::tool::ProcessTool;
use nxb_model::{Digest, sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Shaders the own front end may compile: digests of the MSL whose verdict is allowed.
#[derive(Clone, Debug, Default)]
pub struct Verdicts {
    allowed: BTreeSet<Digest>,
    /// Lines per verdict, for the report.
    pub counts: BTreeMap<String, usize>,
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("\"{name}\": \"");
    let start = line.find(&pat)? + pat.len();
    let end = line[start..].find('"')?;
    Some(&line[start..start + end])
}

impl Verdicts {
    /// Parse the JSONL verdict file (`source_sha256`, `verdict` per line).`allow` lists the verdicts that select the
    /// own front end (for example `own-identical`).
    pub fn parse(text: &str, allow: &[&str]) -> Result<Verdicts, String> {
        let mut v = Verdicts::default();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            // JSONL (the front end's validation output) or TSV `<sha256>\t<verdict>` (the shipped data form)
            let tsv = line
                .split_once('\t')
                .filter(|(h, _)| h.len() == 64 && !line.starts_with('{'));
            let (Some(h), Some(verdict)) = (
                tsv.map(|t| t.0).or_else(|| field(line, "source_sha256")),
                tsv.map(|t| t.1.trim()).or_else(|| field(line, "verdict")),
            ) else {
                return Err(format!("verdict line {}: no source_sha256/verdict", n + 1));
            };
            *v.counts.entry(verdict.to_string()).or_default() += 1;
            if allow.contains(&verdict) {
                v.allowed.insert(
                    Digest::from_hex(h).map_err(|e| format!("verdict line {}: {e}", n + 1))?,
                );
            }
        }
        Ok(v)
    }

    /// Read a verdict file.
    pub fn load(path: &Path, allow: &[&str]) -> Result<Verdicts, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text, allow)
    }

    /// True when the shader with this MSL goes to the own front end.
    pub fn allows(&self, msl: &[u8]) -> bool {
        self.allowed.contains(&sha256(msl))
    }

    /// Number of allowed shaders.
    pub fn len(&self) -> usize {
        self.allowed.len()
    }

    /// True when nothing is allowed.
    pub fn is_empty(&self) -> bool {
        self.allowed.is_empty()
    }
}

/// `air_front::id()`: the front end's identity (crate version and output format generation).
pub fn front_id() -> String {
    air_front::id()
}

/// `air_front` in process plus Apple's back half.
#[derive(Debug)]
pub struct AirFrontEnd {
    tools: Arc<MetalTools>,
    air_opt: ProcessTool,
    target: air_front::Target,
    id: String,
    /// Shaders that went through `air-opt`'s text path (two calls).
    pub text_ir: AtomicU64,
}

impl AirFrontEnd {
    /// The own front end for macOS (`ios` false) or iOS, with `air-opt` from the same toolchain as `tools`.
    pub fn new(tools: Arc<MetalTools>, air_opt: ProcessTool, ios: bool) -> Self {
        AirFrontEnd {
            tools,
            air_opt,
            target: if ios {
                air_front::Target::ios()
            } else {
                air_front::Target::macos()
            },
            id: air_front::id(),
            text_ir: AtomicU64::new(0),
        }
    }
}

impl FrontEnd for AirFrontEnd {
    fn id(&self) -> &str {
        &self.id
    }

    fn compile(&self, msl: &[u8]) -> Result<FrontEndOutput, FrontEndError> {
        let ir = air_front::compile(msl, &self.target).map_err(|r| FrontEndError::Reject(r.0))?;
        if matches!(ir, air_front::Ir::Text(..)) {
            self.text_ir.fetch_add(1, Ordering::Relaxed);
        }
        let mut steps: Vec<StepCost> = Vec::new();
        let mut bytes = ir.bytes().to_vec();
        for args in ir.back_half() {
            let (out, c) = self
                .tools
                .filter(&self.air_opt, args, bytes)
                .map_err(FrontEndError::Tool)?;
            steps.push(c);
            bytes = out;
        }
        let (lib, l) = self
            .tools
            .link(&bytes, Io::Pipes)
            .map_err(FrontEndError::Tool)?;
        steps.push(l);
        Ok(FrontEndOutput {
            metallib: lib,
            frontend: self.id.clone(),
            steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts_select_allowed() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let text = format!(
            "{{\"i\": 0, \"source_sha256\": \"{a}\", \"verdict\": \"own-identical\"}}\n{{\"source_sha256\": \"{b}\", \"verdict\": \"apple\"}}\n"
        );
        let v = Verdicts::parse(&text, &["own-identical"]).expect("parses");
        assert_eq!(v.len(), 1);
        assert_eq!(v.counts.get("apple"), Some(&1));
        assert!(Verdicts::parse("{\"x\": 1}", &[]).is_err());
        let t = Verdicts::parse(
            &format!("{a}\town-identical\n{b}\tapple\n"),
            &["own-identical"],
        )
        .expect("tsv");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn verdicts_match_msl_digest() {
        let msl = b"fragment float4 f() { return 0; }";
        let line = format!(
            "{{\"source_sha256\": \"{}\", \"verdict\": \"own-validated\"}}",
            sha256(msl).to_hex()
        );
        let v = Verdicts::parse(&line, &["own-validated"]).expect("parses");
        assert!(v.allows(msl));
        assert!(!v.allows(b"other"));
    }
}
