//! The Metal 4 pipelines script (`metal4-pipelines-script(5)`, the `.mtl4-json` that `pso_offline.py` writes and
//! `metal-tt` reads): parsed, hashed per base, and cut into per-batch scripts written as `json.dump(indent=1)` does.

use std::collections::{BTreeMap, HashMap};

use nxb_model::{Digest, Sha256};
use nxb_pyjson::Value;

use crate::cut::Base;

/// A parsed script.
pub struct Script {
    /// `libraries`: label → entry (label, path).
    pub libraries: Vec<Value>,
    /// `library_function_descriptors`.
    pub functions: Vec<Value>,
    /// `render_pipeline_descriptors`.
    pub pipelines: Vec<Value>,
}

fn list<'a>(v: &'a Value, path: &[&str]) -> Result<&'a [Value], String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p).ok_or_else(|| format!("script: no {p}"))?;
    }
    cur.as_list()
        .ok_or_else(|| format!("script: {} is not a list", path.join(".")))
}

fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("script: no {k}"))
}

impl Script {
    /// Parses the script text.
    pub fn parse(s: &str) -> Result<Script, String> {
        let v = nxb_pyjson::parse(s).map_err(|e| format!("script: {e:?}"))?;
        Ok(Script {
            libraries: list(&v, &["libraries"])?.to_vec(),
            functions: list(
                &v,
                &["function_descriptors", "library_function_descriptors"],
            )?
            .to_vec(),
            pipelines: list(&v, &["pipeline_descriptors", "render_pipeline_descriptors"])?.to_vec(),
        })
    }

    /// Library paths by label.
    pub fn library_paths(&self) -> Result<BTreeMap<String, String>, String> {
        self.libraries
            .iter()
            .map(|l| Ok((text(l, "label")?.to_string(), text(l, "path")?.to_string())))
            .collect()
    }

    /// The content hash of every base: its two functions (library bytes by digest, entry name) and the descriptor
    /// text (colour state), not the library paths or labels.
    pub fn bases(&self, lib_digest: &HashMap<String, Digest>) -> Result<Vec<Base>, String> {
        let fns: HashMap<&str, &Value> = self
            .functions
            .iter()
            .filter_map(|f| Some((text(f, "label").ok()?, f)))
            .collect();
        let mut out = Vec::with_capacity(self.pipelines.len());
        for (index, p) in self.pipelines.iter().enumerate() {
            let mut h = Sha256::new();
            let mut group = [0u8; 32];
            for k in ["vertex_function_descriptor", "fragment_function_descriptor"] {
                let r = text(p, k)?;
                let f = fns
                    .get(r.trim_start_matches("fnd:"))
                    .ok_or_else(|| format!("script: no function {r}"))?;
                let lib = text(f, "library")?.trim_start_matches("lib:");
                let d = lib_digest
                    .get(lib)
                    .ok_or_else(|| format!("script: no digest of library {lib}"))?;
                h.update(&d.0);
                h.update(text(f, "name")?.as_bytes());
                h.update(&[0]);
                if k == "vertex_function_descriptor" {
                    let mut g = Sha256::new();
                    g.update(&d.0);
                    g.update(text(f, "name")?.as_bytes());
                    group = g.finalize().0;
                }
            }
            let mut rest = p.clone();
            if let Value::Object(m) = &mut rest {
                m.retain(|(k, _)| !k.ends_with("_function_descriptor"));
            }
            h.update(nxb_pyjson::dumps(&rest).as_bytes());
            out.push(Base {
                hash: h.finalize().0,
                group,
                index,
            });
        }
        Ok(out)
    }

    /// The script of one batch, in `pso_offline.py`'s layout (libraries and functions in first-use order).
    pub fn batch_text(&self, batch: &[Base]) -> Result<String, String> {
        let fns: HashMap<&str, &Value> = self
            .functions
            .iter()
            .filter_map(|f| Some((text(f, "label").ok()?, f)))
            .collect();
        let libs: HashMap<&str, &Value> = self
            .libraries
            .iter()
            .filter_map(|l| Some((text(l, "label").ok()?, l)))
            .collect();
        let (mut lo, mut fo, mut po) = (Vec::new(), Vec::new(), Vec::new());
        let (mut seen_l, mut seen_f) = (
            std::collections::HashSet::new(),
            std::collections::HashSet::new(),
        );
        for b in batch {
            let p = &self.pipelines[b.index];
            for k in ["vertex_function_descriptor", "fragment_function_descriptor"] {
                let label = text(p, k)?.trim_start_matches("fnd:");
                let f = *fns
                    .get(label)
                    .ok_or_else(|| format!("script: no function {label}"))?;
                let lib = text(f, "library")?.trim_start_matches("lib:");
                if seen_l.insert(lib.to_string()) {
                    lo.push(
                        (*libs
                            .get(lib)
                            .ok_or_else(|| format!("script: no library {lib}"))?)
                        .clone(),
                    );
                }
                if seen_f.insert(label.to_string()) {
                    fo.push(f.clone());
                }
            }
            po.push(p.clone());
        }
        let v = Value::Object(vec![
            ("libraries".into(), Value::List(lo)),
            (
                "function_descriptors".into(),
                Value::Object(vec![(
                    "library_function_descriptors".into(),
                    Value::List(fo),
                )]),
            ),
            (
                "pipeline_descriptors".into(),
                Value::Object(vec![(
                    "render_pipeline_descriptors".into(),
                    Value::List(po),
                )]),
            ),
        ]);
        Ok(nxb_pyjson::dumps_indent(&v, 1))
    }
}
