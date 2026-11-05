//! Early cutoff for the translator (docs/early-cutoff.md): a translation is kept in the store under
//! (translator build id, the job's content: its JSON without the scratch path, and the code bytes it names). A chain
//! change that does not touch the translator translates nothing, one that does re-translates, and every stage after it
//! keys on content again (spirv-cross by the SPIR-V, compiles by the MSL), so equal outputs stop the change there.

use nxb_error::ToolError;
use nxb_model::{ContentKey, Digest, sha256};
use nxb_store::Store;
use nxb_tool::{ToolId, Translator, XlateJob, XlateOutput, XlateResult};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// The translator's build id: SHA-256 over the files of its folder (the apphost and its .NET assemblies), sorted by
/// name.`NXB_TEST_XLATE_SALT` (a test hook) is mixed in.
pub fn translator_id(program: &Path) -> String {
    let mut h = nxb_model::Sha256::new();
    if let Some(dir) = program.parent()
        && let Ok(rd) = std::fs::read_dir(dir)
    {
        let mut files: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for f in files {
            h.update(
                f.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
                    .as_bytes(),
            );
            h.update(&[0]);
            h.update(
                &std::fs::read(&f)
                    .map(|b| sha256(&b).0.to_vec())
                    .unwrap_or_default(),
            );
        }
    }
    h.update(
        std::env::var("NXB_TEST_XLATE_SALT")
            .unwrap_or_default()
            .as_bytes(),
    );
    nxb_model::to_hex(&h.finalize().0)
}

/// The build's translator in front of its store, and spirv-cross's id (SHA-256 of the binary).
pub(crate) fn open<T: Translator>(
    inner: T,
    cfg: &crate::build::BuildConfig,
) -> Result<(CachedTranslator<T>, String), String> {
    let mut store = Store::open(&cfg.store).map_err(|e| e.to_string())?;
    store.set_durability(nxb_store::Durability::Relaxed);
    let sc = std::fs::read(&cfg.spirv_cross)
        .map_err(|e| format!("{}: {e}", cfg.spirv_cross.display()))?;
    Ok((
        CachedTranslator::new(inner, store, translator_id(&cfg.xlate), &cfg.scratch),
        sha256(&sc).to_hex(),
    ))
}

/// A translator in front of the store.
pub struct CachedTranslator<T> {
    inner: T,
    store: Store,
    id: String,
    scratch: String,
    /// Jobs answered from the store.
    pub hits: AtomicU64,
    /// Jobs translated.
    pub misses: AtomicU64,
}

impl<T: Translator> CachedTranslator<T> {
    /// `scratch`: the folder whose path the jobs name (left out of the key).
    pub fn new(inner: T, store: Store, id: String, scratch: &Path) -> Self {
        CachedTranslator {
            inner,
            store,
            id,
            scratch: scratch.to_string_lossy().into_owned(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// The inner translator.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    fn key(&self, j: &XlateJob) -> ContentKey {
        // the code file the job names: its bytes, not its path
        let code: Digest = j
            .job_json
            .find("\"code\": \"")
            .and_then(|a| {
                let rest = &j.job_json[a + 9..];
                rest.find('"').map(|b| &rest[..b])
            })
            .and_then(|p| std::fs::read(p).ok())
            .map_or_else(|| sha256(b""), |b| sha256(&b));
        ContentKey::builder("nxb.xlate.v1")
            .field_str("translator", &self.id)
            .field_str("job", &j.job_json.replace(&self.scratch, "<scratch>"))
            .field_digest("code", &code)
            .finish()
    }
}

impl<T: Translator> Translator for CachedTranslator<T> {
    fn identity(&self) -> &ToolId {
        self.inner.identity()
    }

    fn translate(&self, batch: &[XlateJob]) -> Result<Vec<XlateResult>, ToolError> {
        let keys: Vec<ContentKey> = batch.iter().map(|j| self.key(j)).collect();
        let mut out: Vec<Option<XlateResult>> = vec![None; batch.len()];
        let mut miss = Vec::new();
        for (i, (j, k)) in batch.iter().zip(&keys).enumerate() {
            let hit = self.store.lookup(k).ok().flatten().and_then(|m| {
                let get = |n: &str| m.files.iter().find(|f| f.name == n).map(|f| f.digest);
                let (s, r) = (get("spv")?, get("res.json")?);
                Some(XlateOutput {
                    spirv: self.store.read_blob(&s.sha256).ok()?,
                    resources_json: self.store.read_blob(&r.sha256).ok()?,
                })
            });
            match hit {
                Some(o) => {
                    out[i] = Some(XlateResult {
                        key: j.key.clone(),
                        outcome: Ok(o),
                    })
                }
                None => miss.push(i),
            }
        }
        self.hits
            .fetch_add((batch.len() - miss.len()) as u64, Ordering::Relaxed);
        if !miss.is_empty() {
            self.misses.fetch_add(miss.len() as u64, Ordering::Relaxed);
            let jobs: Vec<XlateJob> = miss.iter().map(|&i| batch[i].clone()).collect();
            let res = self.inner.translate(&jobs)?;
            for (&i, r) in miss.iter().zip(res) {
                if let Ok(o) = &r.outcome {
                    let mut w = self.store.entry(keys[i]);
                    let ok = w.add_bytes("spv", &o.spirv).is_ok()
                        && w.add_bytes("res.json", &o.resources_json).is_ok();
                    if ok {
                        let _ = w.commit();
                    }
                }
                out[i] = Some(r);
            }
        }
        Ok(out.into_iter().flatten().collect())
    }
}
