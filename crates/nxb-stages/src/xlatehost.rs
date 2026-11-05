//! Choosing the translator bridge of a build: the persistent host by default when its binary is found, the process
//! per batch otherwise or when switched off.

use nxb_tool::{
    EnvPolicy, HostConfig, HostTranslator, ProcessRegistry, ProcessTranslator, ToolId, XlateConfig,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The translator bridge of a build.
pub(crate) enum Xlate {
    /// Process per batch.
    Batch(ProcessTranslator),
    /// Persistent hosts.
    Host(Box<nxb_tool::HostTranslator>),
    /// In process Rust, the other variants take refused keys.
    Rust(Box<nxb_tool::RustTranslator>),
}

impl nxb_tool::Translator for Xlate {
    fn identity(&self) -> &nxb_tool::ToolId {
        match self {
            Xlate::Batch(t) => t.identity(),
            Xlate::Host(t) => t.identity(),
            Xlate::Rust(t) => t.identity(),
        }
    }

    fn translate(
        &self,
        b: &[nxb_tool::XlateJob],
    ) -> Result<Vec<nxb_tool::XlateResult>, nxb_error::ToolError> {
        match self {
            Xlate::Batch(t) => t.translate(b),
            Xlate::Host(t) => t.translate(b),
            Xlate::Rust(t) => t.translate(b),
        }
    }
}

impl Xlate {
    /// Host restarts (0 for the process bridge).
    pub(crate) fn restarts(&self) -> u32 {
        match self {
            Xlate::Batch(_) | Xlate::Rust(_) => 0,
            Xlate::Host(t) => t.restarts(),
        }
    }

    /// Keys sent to the process bridge after a host failure, by reason.
    pub(crate) fn fallbacks(&self) -> Vec<(String, u64)> {
        match self {
            Xlate::Batch(_) => Vec::new(),
            Xlate::Host(t) => t.fallbacks(),
            Xlate::Rust(t) => t.fallbacks(),
        }
    }

    /// Translator processes started.
    pub(crate) fn runs(&self) -> u64 {
        match self {
            Xlate::Batch(t) => t.runs(),
            Xlate::Host(t) => t.starts(),
            Xlate::Rust(_) => 0,
        }
    }
}

/// The host program and its arguments. `explicit` is `--xlate-host` (else the env var `NXB_XLATE_HOST`): `off` selects
/// the process bridge, anything else is the program and its arguments. With neither set, `dmsh_xlate_host` is looked
/// for next to `xlate` (`dmsh_xlate`) and in its parent folder. `None`: no host.
pub fn resolve(explicit: Option<&str>, xlate: &Path) -> Option<Vec<String>> {
    let env = std::env::var("NXB_XLATE_HOST").ok();
    match explicit.or(env.as_deref()).map(str::trim) {
        Some("off") => return None,
        Some(v) if !v.is_empty() => {
            return Some(v.split_whitespace().map(str::to_string).collect());
        }
        _ => {}
    }
    let dir = xlate.parent()?;
    let found: Option<PathBuf> = [
        dir.join("dmsh_xlate_host"),
        dir.join("../dmsh_xlate_host_bin/dmsh_xlate_host"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    found.map(|p| vec![p.to_string_lossy().into_owned()])
}

/// The Rust translator switch: `--xlate-rust on|off`, else env `NXB_XLATE_RUST` (`0`, `off` or `false` turn it off).
/// On by default: the output tree equals the C# path and each key the Rust core refuses goes to the C# bridge.
pub fn resolve_rust(explicit: Option<&str>) -> bool {
    let env = std::env::var(nxb_tool::XLATE_RUST_ENV).ok();
    !matches!(
        explicit.or(env.as_deref()).map(str::trim),
        Some("0" | "off" | "false")
    )
}

/// The bridge: a host with the process bridge as fallback, or the process bridge alone.
pub(crate) fn open(
    id: ToolId,
    registry: &Arc<ProcessRegistry>,
    scratch: &Path,
    env: EnvPolicy,
    host: Option<&[String]>,
    rust: bool,
) -> std::io::Result<Xlate> {
    let inner = open_cs(id.clone(), registry, scratch, env, host)?;
    if !rust {
        return Ok(inner);
    }
    let rid = ToolId {
        name: "nxb-xlate".into(),
        path: id.path,
        version: String::new(),
    };
    Ok(Xlate::Rust(Box::new(nxb_tool::RustTranslator::new(
        rid,
        Some(Box::new(inner)),
    ))))
}

fn open_cs(
    id: ToolId,
    registry: &Arc<ProcessRegistry>,
    scratch: &Path,
    env: EnvPolicy,
    host: Option<&[String]>,
) -> std::io::Result<Xlate> {
    let batch = ProcessTranslator::new(
        id.clone(),
        registry.clone(),
        XlateConfig::new(scratch.join("x"), env.clone()),
    );
    let Some([program, args @ ..]) = host else {
        return Ok(Xlate::Batch(batch));
    };
    let dir = scratch.join("h");
    std::fs::create_dir_all(&dir)?;
    let cfg = HostConfig::new(program.into(), args.to_vec(), env, dir);
    Ok(Xlate::Host(Box::new(HostTranslator::new(
        id,
        cfg,
        registry.clone(),
        batch,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_and_discovery() {
        let d = std::env::temp_dir().join(format!("nxb-disc-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap_or_default();
        let x = d.join("dmsh_xlate");
        assert_eq!(resolve(Some("off"), &x), None);
        assert_eq!(
            resolve(Some("/h --threads 4"), &x),
            Some(vec!["/h".into(), "--threads".into(), "4".into()])
        );
        if std::env::var_os("NXB_XLATE_HOST").is_none() {
            assert_eq!(resolve(None, &x), None);
            std::fs::write(d.join("dmsh_xlate_host"), "").unwrap_or_default();
            assert_eq!(
                resolve(None, &x),
                Some(vec![
                    d.join("dmsh_xlate_host").to_string_lossy().into_owned()
                ])
            );
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
