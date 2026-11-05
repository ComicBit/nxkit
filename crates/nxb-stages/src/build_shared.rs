//! The state shared by every task of [`crate::build`] and its output helpers: libraries per target (Mac and, when asked
//! for, iOS from the same MSL, `IOSOUT` in `dmsh_build.py`) through single-flight and the store, outputs cloned from
//! the store, spirv-cross.

use crate::build::Span;
use crate::build::fatal;
use crate::build_finish::Gate;
use crate::inputs::ShaderInputs;
use nxb_error::Fatal;
use nxb_model::{ContentKey, FileDigest};
use nxb_pyjson::{Value, dumps_indent};
use nxb_sched::SingleFlight;
use nxb_shader::pch;
use nxb_shader::side::{self, Unit};
use nxb_shader::target::{MAC, TargetConv};
use nxb_store::Store;
use nxb_tool::{Io, MetalTools, ProcessTool, ProcessTranslator, Tool};
use nxb_tool::{ProcessRegistry, Resolver, SdkRoots};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

/// A second target built from the same sources (iOS): its convention, tools, header, identity and output folder.
pub(crate) struct Side {
    pub(crate) conv: TargetConv,
    pub(crate) tools: Arc<MetalTools>,
    pub(crate) pch: Option<PathBuf>,
    pub(crate) compiler_id: String,
    pub(crate) ident: String,
    pub(crate) out: PathBuf,
}

/// One source's libraries: Mac, and iOS when the build makes it.
#[derive(Clone, Debug)]
pub(crate) struct Libs {
    pub(crate) mac: FileDigest,
    pub(crate) ios: Option<FileDigest>,
}

/// The translator bridge of a build.
pub(crate) enum Xlate {
    /// A process per batch.
    Batch(ProcessTranslator),
    /// Persistent hosts.
    Host(nxb_tool::HostTranslator),
}

impl nxb_tool::Translator for Xlate {
    fn identity(&self) -> &nxb_tool::ToolId {
        match self {
            Xlate::Batch(t) => t.identity(),
            Xlate::Host(t) => t.identity(),
        }
    }

    fn translate(
        &self,
        b: &[nxb_tool::XlateJob],
    ) -> Result<Vec<nxb_tool::XlateResult>, nxb_error::ToolError> {
        match self {
            Xlate::Batch(t) => t.translate(b),
            Xlate::Host(t) => t.translate(b),
        }
    }
}

impl Xlate {
    /// Translator processes started.
    pub(crate) fn runs(&self) -> u64 {
        match self {
            Xlate::Batch(t) => t.runs(),
            Xlate::Host(t) => t.starts(),
        }
    }
}

/// State shared by every task of a build.
pub(crate) struct Shared {
    pub(crate) out: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) scratch: PathBuf,
    pub(crate) chain: String,
    pub(crate) ident: String,
    pub(crate) compiler_id: String,
    pub(crate) tools: Arc<MetalTools>,
    pub(crate) own: Option<crate::own_front::OwnFront>,
    pub(crate) pch: Option<PathBuf>,
    pub(crate) io: Io,
    pub(crate) store: Store,
    pub(crate) sc: ProcessTool,
    pub(crate) translator: crate::xlate_cache::CachedTranslator<Xlate>,
    /// spirv-cross's build id (SHA-256 of the binary): part of the MSL cache key.
    pub(crate) sc_id: String,
    pub(crate) inputs: ShaderInputs,
    pub(crate) pairs: BTreeMap<String, BTreeSet<String>>,
    pub(crate) flights: SingleFlight<ContentKey, Result<FileDigest, String>>,
    /// The iOS side (`None`: Mac only).
    pub(crate) ios: Option<Side>,
    /// Paired vertex shaders waiting for their translation and their pixel shaders.
    pub(crate) gates: Mutex<BTreeMap<String, Gate>>,
    /// Pixel shader → the paired vertex shaders waiting for it.
    pub(crate) dependents: Mutex<BTreeMap<String, Vec<String>>>,
    pub(crate) results: Mutex<BTreeMap<String, (String, String)>>,
    pub(crate) on_done: Option<crate::build::OnDone>,
    /// Store keys of the libraries this build used (`_b_cas.txt`).
    pub(crate) used: Mutex<BTreeSet<String>>,
    pub(crate) spans: Mutex<Vec<Span>>,
    pub(crate) t0: Instant,
    pub(crate) n: AtomicU64,
}

impl Shared {
    pub(crate) fn result(&self, key: &str, st: &str, err: &str) {
        let err: String = err.chars().take(160).collect();
        self.results
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.to_string(), (st.to_string(), err));
        if let Some(f) = &self.on_done {
            (f.0)(key, st);
        }
    }

    pub(crate) fn span(&self, what: &'static str, key: &str, t: Instant) {
        self.spans
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Span {
                what,
                key: key.to_string(),
                start: t.duration_since(self.t0),
                wall: t.elapsed(),
            });
    }

    /// spirv-cross on a module: the SPIR-V through a temp file (spirv-cross seeks its input), the MSL from stdout.
    pub(crate) fn cross(&self, spv: &[u8], args: &[String]) -> Result<String, String> {
        // early cutoff: the MSL of a SPIR-V module under (spirv-cross id, arguments, SPIR-V bytes)
        let key = ContentKey::builder("nxb.msl.v1")
            .field_str("spirv-cross", &self.sc_id)
            .field_str("args", &args.join("\0"))
            .field("spv", spv)
            .finish();
        if let Ok(Some(m)) = self.store.lookup(&key)
            && let Some(f) = m.files.first()
            && let Ok(b) = self.store.read_blob(&f.digest.sha256)
            && let Ok(t) = String::from_utf8(b)
        {
            return Ok(t);
        }
        let n = self.n.fetch_add(1, Ordering::Relaxed);
        let dir = self.scratch.join("tmp");
        let sp = dir.join(format!("x{n}.spv"));
        fs::write(&sp, spv).map_err(|e| e.to_string())?;
        let env = nxb_tool::EnvPolicy::spirv_cross(&dir).map_err(|e| e.to_string())?;
        let inv = self.sc.invocation().args(args.iter()).arg(&sp).env(env);
        let r = self.sc.run(&inv);
        let _ = fs::remove_file(&sp);
        let c = r.map_err(|e| e.stderr_head.trim().to_string())?;
        let mut w = self.store.entry(key);
        if w.add_bytes("msl", &c.stdout).is_ok() {
            let _ = w.commit();
        }
        String::from_utf8(c.stdout).map_err(|e| e.to_string())
    }

    /// A source's libraries for every target of the build.
    pub(crate) fn library(&self, src: &str) -> Result<Libs, String> {
        let mac = self.lib_one(None, src)?;
        let ios = match &self.ios {
            Some(t) => Some(self.lib_one(Some(t), src)?),
            None => None,
        };
        Ok(Libs { mac, ios })
    }

    /// The library of a source for one target, through single-flight and the store (the own front end: Mac only).
    fn lib_one(&self, t: Option<&Side>, src: &str) -> Result<FileDigest, String> {
        let (conv, cid, tools, pch) = match t {
            Some(t) => (t.conv, &t.compiler_id, &t.tools, t.pch.as_deref()),
            None => (MAC, &self.compiler_id, &self.tools, self.pch.as_deref()),
        };
        let own = if t.is_none() { self.own.as_ref() } else { None };
        let mut kb = ContentKey::builder("nxb.metallib.v2")
            .field_str("record", &conv.record())
            .field_str("compiler", cid)
            .field_str(
                "test-salt",
                &std::env::var("NXB_TEST_COMPILER_SALT").unwrap_or_default(),
            )
            .field("msl", src.as_bytes());
        if let Some(o) = own {
            kb = kb.field_str("frontend", o.tag());
        }
        // test hook for the chain-delta benchmark: a compiler identity change
        if let Ok(salt) = std::env::var("NXB_TEST_COMPILER_SALT") {
            kb = kb.field_str("test-salt", &salt);
        }
        let key = kb.finish();
        self.used
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.to_hex());
        let d = self.flights.get_or_run(key, || {
            if let Ok(Some(m)) = self.store.lookup(&key)
                && let Some(f) = m.files.first()
            {
                return Ok(f.digest);
            }
            let (lib, _, _) = crate::own_front::compile_link(
                tools,
                pch,
                own,
                conv.compile_flags,
                src.as_bytes(),
                self.io,
            )?;
            let mut w = self.store.entry(key);
            let d = w.add_bytes("b.metallib", &lib).map_err(|e| e.to_string())?;
            w.commit().map_err(|e| e.to_string())?;
            Ok(d)
        });
        (*d).clone()
    }

    fn lib_bytes(&self, d: &FileDigest) -> Result<Vec<u8>, String> {
        self.store.read_blob(&d.sha256).map_err(|e| e.to_string())
    }

    fn clone_to(&self, dir: &Path, name: &str, d: &FileDigest) -> Result<(), String> {
        let dst = dir.join(name);
        let _ = fs::remove_file(&dst);
        nxb_sys::clone::clone_or_copy(&self.store.blob_path(&d.sha256), &dst)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Put a source's libraries at their output paths: clones of the store's blobs (copies across volumes).
    pub(crate) fn place(&self, key: &str, suffix: &str, l: &Libs) -> Result<(), String> {
        let name = format!("{key}{suffix}");
        self.clone_to(&self.out, &name, &l.mac)?;
        if let (Some(t), Some(d)) = (&self.ios, &l.ios) {
            self.clone_to(&t.out, &name, d)?;
        }
        Ok(())
    }

    /// A pixel shader's side libraries (`ps_libraries`): the unit compiled once per target and cut into `_b` and
    /// `_bx`. `Ok(false)`: the unit does not build or cut (the caller builds the plain source).
    pub(crate) fn side(&self, key: &str, u: &Unit) -> Result<bool, String> {
        let Ok(l) = self.library(&u.source) else {
            return Ok(false);
        };
        let names = u.side();
        let mac = self
            .lib_bytes(&l.mac)
            .ok()
            .and_then(|b| side::cut(&b, &names).ok());
        let ios = match (&self.ios, &l.ios) {
            (Some(_), Some(d)) => match self
                .lib_bytes(d)
                .ok()
                .and_then(|b| side::cut(&b, &names).ok())
            {
                Some(c) => Some(Some(c)),
                None => return Ok(false),
            },
            _ => Some(None),
        };
        let (Some((b, x)), Some(ios)) = (mac, ios) else {
            return Ok(false);
        };
        self.write(key, "_b.metallib", &b)?;
        self.write(key, "_bx.metallib", &x)?;
        if let (Some(t), Some((b, x))) = (&self.ios, ios) {
            fs::write(t.out.join(format!("{key}_b.metallib")), b).map_err(|e| e.to_string())?;
            fs::write(t.out.join(format!("{key}_bx.metallib")), x).map_err(|e| e.to_string())?;
        }
        Ok(true)
    }

    /// `<key>_b.json` per target, with that target's `chain_metal` and `chain_compiler` (`MTL.with_compiler`).
    pub(crate) fn write_json(&self, key: &str, mut refl: Value) -> Result<(), String> {
        refl.set("chain_metal", Value::str(MAC.record()));
        refl.set("chain_compiler", Value::str(self.ident.clone()));
        self.write(key, "_b.json", dumps_indent(&refl, 1).as_bytes())?;
        if let Some(t) = &self.ios {
            refl.set("chain_metal", Value::str(t.conv.record()));
            refl.set("chain_compiler", Value::str(t.ident.clone()));
            fs::write(t.out.join(format!("{key}_b.json")), dumps_indent(&refl, 1))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub(crate) fn write(&self, key: &str, suffix: &str, bytes: &[u8]) -> Result<(), String> {
        let p: &Path = &self.out;
        fs::write(p.join(format!("{key}{suffix}")), bytes).map_err(|e| e.to_string())
    }
}

pub(crate) fn pairs(cfg: &crate::build::BuildConfig) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for t in ["shader_pairs.tsv", "pso_pairs.tsv"] {
        let mut p = cfg.profile.join(t);
        if t == "pso_pairs.tsv" && !p.is_file() {
            p = cfg.pkg.parent().map_or(p.clone(), |d| d.join(t));
        }
        for l in fs::read_to_string(&p).unwrap_or_default().lines() {
            if l.trim().is_empty() || l.starts_with('#') {
                continue;
            }
            let mut it = l.split_whitespace();
            if let (Some(v), Some(ps)) = (it.next(), it.next()) {
                out.entry(v.to_string()).or_default().insert(ps.to_string());
            }
        }
    }
    out
}

/// The iOS side: its tools through xcrun, its header, its identity, its output folder.
pub(crate) fn ios_side(
    cfg: &crate::build::BuildConfig,
    dir: &std::path::Path,
    resolver: &Resolver,
    registry: &Arc<ProcessRegistry>,
    home: &std::path::Path,
    tmp: &std::path::Path,
) -> Result<Side, Fatal> {
    use nxb_shader::target::IOS;
    let roots =
        SdkRoots::resolve(resolver, &[IOS.target], registry).map_err(|e| fatal("iOS SDK", e))?;
    let env = roots
        .apple_env(IOS.target, tmp, home)
        .map_err(|e| fatal("iOS environment", e))?;
    let tools = Arc::new(
        MetalTools::resolve(
            resolver,
            IOS.target,
            env,
            tmp.to_path_buf(),
            registry.clone(),
        )
        .map_err(|e| fatal("iOS metal", e))?,
    );
    let (ver, bld) = resolver
        .sdk_version(IOS.target, registry)
        .map_err(|e| fatal("iOS SDK version", e))?;
    let compiler_id = tools
        .compiler_id(&ver, &bld)
        .map_err(|e| fatal("iOS compiler id", e))?;
    fs::create_dir_all(cfg.work.join("pch-ios")).map_err(|e| fatal("iOS header folder", e))?;
    let pch = tools
        .build_pch(
            &IOS.pch_flags(),
            IOS.compile_flags,
            &pch::header(),
            pch::PROBE,
            &cfg.work.join("pch-ios"),
        )
        .map_err(|e| fatal("iOS header", e))?;
    let mut ident = b"dmsh_mtlcache 2\0".to_vec();
    ident.extend_from_slice(IOS.record().as_bytes());
    ident.push(0);
    ident.extend_from_slice(compiler_id.as_bytes());
    let out = dir.join(format!("b-{}", cfg.chain));
    fs::create_dir_all(&out).map_err(|e| fatal("iOS folder", e))?;
    Ok(Side {
        conv: IOS,
        tools,
        pch,
        compiler_id,
        ident: nxb_model::sha256(&ident).to_hex(),
        out,
    })
}
