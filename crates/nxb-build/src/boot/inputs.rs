//! What the boot steps left for the shader steps of the same work folder, in `<work>/shader/host_inputs.tsv`: the
//! split and the translators run in later stages and read it there.

use std::path::{Path, PathBuf};

/// The record's file.
pub(crate) fn inputs_file(work: &Path) -> PathBuf {
    work.join("shader/host_inputs.tsv")
}

/// What the boot steps left for the shader steps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostInputs {
    /// `DM_NRO_DIR`: the NRO folder of a build that captured an NRO (`captured_nros`).
    pub(crate) nro_dir: Option<PathBuf>,
    /// The capture folder of a Vulkan or GL game (`<extract>/<package>/vk_capture`, `gl_capture`): one more input
    /// folder of its shader translator, after the RomFS and the add-on content.
    pub(crate) capture: Option<PathBuf>,
    /// The game's own decoder in the Mac host for the split (`DM_OODLE_HOST`, `DM_OODLE_MODULES`).
    pub(crate) oodle: Option<(PathBuf, PathBuf)>,
    /// The split reads what a boot made (GLSLC outputs, decoded Unreal archives, captured NROs, the game's decoder):
    /// a split that ran in the extract's pass runs again.
    pub(crate) split_again: bool,
    /// The shader keys an empty GLSLC capture saw submitted: the split's shader map must hold each.
    pub(crate) glslc_observed: std::collections::BTreeSet<String>,
}

impl HostInputs {
    /// The record's text: one `key\tvalue` line per value that is set.
    pub(crate) fn to_text(&self) -> String {
        let mut s = String::new();
        if let Some(d) = &self.nro_dir {
            s += &format!("nro_dir\t{}\n", d.display());
        }
        if let Some(d) = &self.capture {
            s += &format!("capture\t{}\n", d.display());
        }
        if let Some((h, m)) = &self.oodle {
            s += &format!("oodle_host\t{}\noodle_modules\t{}\n", h.display(), m.display());
        }
        if self.split_again {
            s += "split_again\t1\n";
        }
        if !self.glslc_observed.is_empty() {
            let v: Vec<&str> = self.glslc_observed.iter().map(String::as_str).collect();
            s += &format!("glslc_observed\t{}\n", v.join(" "));
        }
        s
    }

    /// Reads [`HostInputs::to_text`].
    pub(crate) fn parse(text: &str) -> HostInputs {
        let mut h = HostInputs::default();
        let (mut oh, mut om) = (None, None);
        for l in text.lines() {
            let Some((k, v)) = l.split_once('\t') else {
                continue;
            };
            match k {
                "nro_dir" => h.nro_dir = Some(v.into()),
                "capture" => h.capture = Some(v.into()),
                "oodle_host" => oh = Some(PathBuf::from(v)),
                "oodle_modules" => om = Some(PathBuf::from(v)),
                "split_again" => h.split_again = v == "1",
                "glslc_observed" => {
                    h.glslc_observed = v.split(' ').filter(|k| !k.is_empty()).map(str::to_string).collect();
                }
                _ => {}
            }
        }
        if let (Some(a), Some(b)) = (oh, om) {
            h.oodle = Some((a, b));
        }
        h
    }

    /// The record of `work`, empty when there is none.
    pub(crate) fn load(work: &Path) -> HostInputs {
        HostInputs::parse(&std::fs::read_to_string(inputs_file(work)).unwrap_or_default())
    }

    /// Writes the record of `work`.
    pub(crate) fn save(&self, work: &Path) -> Result<(), String> {
        let f = inputs_file(work);
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let part = f.with_extension("tsv.partial");
        std::fs::write(&part, self.to_text()).map_err(|e| format!("{}: {e}", part.display()))?;
        std::fs::rename(&part, &f).map_err(|e| format!("{}: {e}", f.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_reads_back() {
        let h = HostInputs {
            nro_dir: Some("/w/nro_v0".into()),
            capture: Some("/w/extract/v0/vk_capture".into()),
            oodle: Some(("/w/tools/dmhost".into(), "/w/patched_v0".into())),
            split_again: true,
            glslc_observed: ["0123456789abcdef".to_string(), "fedcba9876543210".to_string()].into(),
        };
        let t = h.to_text();
        assert_eq!(
            t,
            "nro_dir\t/w/nro_v0\ncapture\t/w/extract/v0/vk_capture\noodle_host\t/w/tools/dmhost\noodle_modules\t/w/patched_v0\nsplit_again\t1\nglslc_observed\t0123456789abcdef fedcba9876543210\n"
        );
        assert_eq!(HostInputs::parse(&t), h);
        assert_eq!(HostInputs::parse(""), HostInputs::default());
        assert_eq!(HostInputs::default().to_text(), "");
    }
}
