//! The raw tier of the own front end (first tier, Mac only): a shader whose MSL digest is in the raw allow list
//! (`raw-verified.txt`, every entry passed the GPU differential test) goes through `air_front::build_raw` in process,
//! with no Apple tool. Its cache-key field is `own-raw@<air-front id>`. A raw reject or failure hands the shader to the
//! next tier (own-identical verdicts, then Apple). Shaders not in the list keep their keys and their path unchanged.
//! `NXB_OWN_RAW=0` turns only the raw tier off, `NXB_OWN_FRONT=0` turns every own tier off.

use crate::own_front::{DATA_DIR, front_id};
use std::path::{Path, PathBuf};

/// The raw tier's switch: `NXB_OWN_RAW=0` turns it off, `NXB_OWN_RAW=<file>` selects another allow list.
pub const RAW_SWITCH: &str = "NXB_OWN_RAW";

/// File name of the raw allow list in `<DATA_DIR>/<air-front id>/`.
pub const RAW_LIST: &str = "raw-verified.txt";

/// The raw allow list of this run: `NXB_OWN_RAW` (`0` off, else a file), or the shipped list of this front end.
/// Returns the list file (`None`: tier off) and a note for the run report.
pub fn list_file() -> (Option<PathBuf>, String) {
    let shipped = |id: String| {
        let f = Path::new(DATA_DIR).join(id).join(RAW_LIST);
        if f.is_file() {
            let note = format!("raw tier on ({})", f.display());
            (Some(f), note)
        } else {
            (None, format!("raw tier off ({} missing)", f.display()))
        }
    };
    match std::env::var_os(RAW_SWITCH) {
        Some(v) if v == "0" => (None, format!("raw tier off ({RAW_SWITCH}=0)")),
        Some(v) if !v.is_empty() => {
            let note = format!("raw tier on ({})", Path::new(&v).display());
            (Some(PathBuf::from(v)), note)
        }
        _ => match front_id() {
            Some(id) => shipped(id),
            None => (None, "raw tier off (no air-front feature)".into()),
        },
    }
}

/// The raw tier of a run, with its counters.
#[cfg(feature = "air-front")]
pub struct RawTier {
    fe: nxb_tool::airfront::RawFrontEnd,
    list: nxb_tool::airfront::RawAllowList,
    id: String,
    /// Libraries the raw tier made.
    pub libs: std::sync::atomic::AtomicU64,
    /// Listed shaders the raw tier rejected or failed on (the next tier made them).
    pub fallback: std::sync::atomic::AtomicU64,
}

#[cfg(feature = "air-front")]
impl RawTier {
    /// The tier of this run ([`list_file`]), `None` when it is off. A damaged list is an error.
    pub fn open() -> Result<Option<RawTier>, String> {
        let Some(f) = list_file().0 else {
            return Ok(None);
        };
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        Ok(Some(RawTier {
            fe: nxb_tool::airfront::RawFrontEnd::new(false),
            list: nxb_tool::airfront::RawAllowList::parse(&text)?,
            id: nxb_tool::airfront::raw_id(),
            libs: Default::default(),
            fallback: Default::default(),
        }))
    }

    /// True when the raw tier takes the shader with this MSL.
    pub fn takes(&self, msl: &[u8]) -> bool {
        self.list.allows_digest(&nxb_model::sha256(msl))
    }

    /// The cache-key field of the raw libraries: `own-raw@<air-front id>`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Number of listed shaders.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// True when the list is empty.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The raw library of a listed shader, `None` when the shader is not listed or the build fails.
    pub fn compile(&self, msl: &[u8]) -> Option<(Vec<u8>, nxb_tool::StepCost)> {
        use std::sync::atomic::Ordering::Relaxed;
        if !self.takes(msl) {
            return None;
        }
        match nxb_tool::FrontEnd::compile(&self.fe, msl) {
            Ok(out) => {
                self.libs.fetch_add(1, Relaxed);
                Some((out.metallib, out.steps.first().copied().unwrap_or_default()))
            }
            Err(_) => {
                self.fallback.fetch_add(1, Relaxed);
                None
            }
        }
    }
}

#[cfg(all(test, feature = "air-front"))]
mod tests {
    use super::*;

    #[test]
    fn shipped_list_matches_the_front_end() {
        // The list is bound to the front-end id: a new id needs a new GPU-verified list (no list: tier off).
        let f = Path::new(DATA_DIR)
            .join(front_id().expect("feature on"))
            .join(RAW_LIST);
        if !f.is_file() {
            eprintln!("skipped: no raw list in this tree");
            return;
        }
        let text = std::fs::read_to_string(&f).expect("shipped raw list");
        let list = nxb_tool::airfront::RawAllowList::parse(&text).expect("parses");
        assert!(list.len() > 1000);
    }
}
