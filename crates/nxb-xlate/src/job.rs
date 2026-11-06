//! The job file: a JSON array of job objects (written by the shader stage, read here).

use nxb_pyjson::{Value, parse};
use std::collections::BTreeMap;

/// One translation job (`XlateJob::job_json`).
#[derive(Clone, Debug)]
pub struct Job {
    pub key: String,
    pub stage: String,
    pub code_path: String,
    pub arch_sm86: bool,
    pub tex: BTreeMap<i32, String>,
    /// image slot to (type, format)
    pub img: BTreeMap<i32, (Option<String>, Option<String>)>,
    pub attr: Vec<(i32, String)>,
    pub fout: Vec<(i32, String)>,
    pub local: Option<Vec<i64>>,
    pub shared: i64,
    pub c1: Vec<u8>,
    pub nvn2_cs: Option<Vec<i32>>,
    pub stage_memory: Option<Vec<i32>>,
    pub geometry_topology: Option<i32>,
    /// The vertex or geometry shader captures transform feedback (the job has a `tf` layout).
    pub tf: bool,
    pub early_fragment_tests: bool,
    pub reduced: bool,
    /// `debug` is on unless the job says `false`.
    pub debug: bool,
    pub softfloat64: bool,
}

fn int_list(v: &Value) -> Option<Vec<i64>> {
    Some(v.as_list()?.iter().filter_map(Value::as_i64).collect())
}

fn truthy(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Bool(true)))
}

fn decode_c1(v: Option<&Value>) -> Vec<u8> {
    // the job carries c1 as a list of byte values or a hex string
    match v {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|x| x.as_i64().map(|b| b as u8))
            .collect(),
        Some(Value::Str(s)) => (0..s.len() / 2)
            .filter_map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok())
            .collect(),
        _ => Vec::new(),
    }
}

impl Job {
    /// Read one job object.
    pub fn from_value(v: &Value) -> Result<Job, String> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let key = s("key").ok_or("job without key")?;
        let mut tex = BTreeMap::new();
        if let Some(Value::Object(m)) = v.get("tex") {
            for (k, t) in m {
                let slot = k.parse::<i32>().map_err(|e| format!("tex slot {k}: {e}"))?;
                tex.insert(slot, entry_type(t).unwrap_or_default());
            }
        }
        let mut img = BTreeMap::new();
        if let Some(Value::Object(m)) = v.get("img") {
            for (k, t) in m {
                let slot = k.parse::<i32>().map_err(|e| format!("img slot {k}: {e}"))?;
                let fmt = t.get("fmt").and_then(Value::as_str).map(str::to_string);
                img.insert(slot, (entry_type(t), fmt));
            }
        }
        let pairs = |k: &str| -> Result<Vec<(i32, String)>, String> {
            let mut out = Vec::new();
            if let Some(Value::Object(m)) = v.get(k) {
                for (n, t) in m {
                    let n = n.parse::<i32>().map_err(|e| format!("{k} {n}: {e}"))?;
                    out.push((n, t.as_str().unwrap_or("").to_string()));
                }
            }
            Ok(out)
        };
        let i32s = |k: &str| int_list(v.get(k)?).map(|l| l.into_iter().map(|x| x as i32).collect());
        Ok(Job {
            stage: s("stage").unwrap_or_default(),
            code_path: s("code").unwrap_or_default(),
            arch_sm86: s("arch").as_deref() == Some("sm86"),
            tex,
            img,
            attr: pairs("attr")?,
            fout: pairs("fout")?,
            local: v.get("local").and_then(int_list),
            shared: v.get("shared").and_then(Value::as_i64).unwrap_or(0),
            c1: decode_c1(v.get("c1")),
            nvn2_cs: i32s("nvn2_cs"),
            stage_memory: i32s("stage_memory"),
            geometry_topology: v
                .get("geometry_topology")
                .and_then(Value::as_i64)
                .map(|x| x as i32),
            tf: !matches!(v.get("tf"), None | Some(Value::Null))
                && v.get("tf")
                    .and_then(Value::as_list)
                    .is_none_or(|l| !l.is_empty()),
            early_fragment_tests: truthy(v.get("early_fragment_tests")),
            reduced: truthy(v.get("reduced")),
            debug: !matches!(v.get("debug"), Some(Value::Bool(false))),
            softfloat64: truthy(v.get("softfloat64")),
            key,
        })
    }
}

/// `n is JsonObject o ? o["type"] : (string)n`
fn entry_type(t: &Value) -> Option<String> {
    match t {
        Value::Object(_) => t.get("type").and_then(Value::as_str).map(str::to_string),
        Value::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// Parse a job file (a JSON array).
pub fn parse_jobs(text: &str) -> Result<Vec<Job>, String> {
    let v = parse(text).map_err(|e| e.to_string())?;
    v.as_list()
        .ok_or("job file is not an array")?
        .iter()
        .map(Job::from_value)
        .collect()
}
