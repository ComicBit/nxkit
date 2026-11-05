//! `ShaderInterface`: the typed, versioned description of what the host must provide to run one translated shader.
//!
//! It is derived from the job (the facts the shader domain knows: attribute and output types, compute local size) and
//! from the translation's `res.json` (bindings, bindless handle words, scale flags, written render targets), so any
//! translator that writes a `res.json` can have one. `res.json` stays the compatibility file: the translator writes
//! `<key>.iface.json` beside it.

use crate::job::Job;
use crate::res::{Res, Tex, to_json_string};
use nxb_pyjson::Value;

/// The schema version written as `version`. Raise it when a field changes meaning, add fields without raising it only
/// when old readers can ignore them.
pub const VERSION: i64 = 1;

/// `TextureUsageFlags` bits in `res.json` texture `flags`.
const RES_SCALE_UNSUPPORTED: i64 = 1;
const NEEDS_SCALE_VALUE: i64 = 2;
const IMAGE_STORE: i64 = 4;

/// Where a texture or image handle comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleSource {
    /// A word of a constant buffer holds the handle (a bindless handle word of the uniform data).
    ConstBufferWord { slot: i64, word: i64 },
    /// A fixed index into the texture table (no constant buffer read).
    Fixed { index: i64 },
}

/// A texture or image binding.
#[derive(Clone, Debug, PartialEq)]
pub struct TexBinding {
    pub binding: i64,
    /// `SamplerType` value of the translator.
    pub sampler_type: i64,
    /// Image format name (images), `Unknown` for textures.
    pub format: String,
    pub handle: HandleSource,
    /// Array length (1: no array).
    pub array: i64,
    /// The shader reads the render scale for this resource (integer coordinates).
    pub scale_sensitive: bool,
    /// Render scale cannot be applied to it.
    pub scale_unsupported: bool,
    /// The image is stored to.
    pub written: bool,
}

/// A storage buffer: its address comes from two words of a constant buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct StorageBinding {
    pub binding: i64,
    pub slot: i64,
    /// Constant buffer slot and byte offset of the 64-bit address word pair.
    pub cb_slot: i64,
    pub cb_offset: i64,
    pub written: bool,
}

/// A constant word pair or word that holds a handle or address, for the host to fill from its tables.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HandleWord {
    /// `texture`, `image` (handle words) or `buffer` (address pair at a byte offset).
    pub kind: String,
    pub cb_slot: i64,
    /// Word index (`texture`, `image`) or byte offset (`buffer`).
    pub offset: i64,
}

/// A fragment output.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub target: i64,
    /// Bit 0 red .. bit 3 alpha.
    pub components: i64,
    /// `float`, `sint` or `uint` when the job knows it, else `None` (the bound attachment format decides).
    pub ty: Option<String>,
}

/// A vertex input the host must feed as an integer.
#[derive(Clone, Debug, PartialEq)]
pub struct IntInput {
    pub location: i64,
    pub ty: String,
}

/// Compute launch facts.
#[derive(Clone, Debug, PartialEq)]
pub struct Compute {
    pub local_size: [i64; 3],
    pub shared_bytes: i64,
}

/// The whole interface.
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderInterface {
    pub version: i64,
    pub key: String,
    pub stage: String,
    pub cbufs: Vec<(i64, i64)>,
    pub storage: Vec<StorageBinding>,
    pub textures: Vec<TexBinding>,
    pub images: Vec<TexBinding>,
    pub handle_words: Vec<HandleWord>,
    pub outputs: Vec<Output>,
    /// Targets 0 and 1 are both written: the shader could feed dual-source blending (any multiple render target shader looks the same). Only the blend state decides.
    pub dual_source_candidate: bool,
    pub int_inputs: Vec<IntInput>,
    /// The shader reads the fragment position (scaled by the render scale).
    pub frag_coord: bool,
    /// Any binding is scale sensitive, or `frag_coord`.
    pub render_scale_sensitive: bool,
    pub compute: Option<Compute>,
    /// Geometry output size per invocation (0: not a geometry shader).
    pub geometry_stride: i64,
    pub fetch1d: Vec<i64>,
}

fn tex(t: &Tex) -> TexBinding {
    TexBinding {
        binding: t.binding,
        sampler_type: t.ty,
        format: t.format.clone(),
        handle: if t.cbuf_slot >= 0 {
            HandleSource::ConstBufferWord {
                slot: t.cbuf_slot,
                word: t.handle,
            }
        } else {
            HandleSource::Fixed { index: t.handle }
        },
        array: t.array,
        scale_sensitive: t.flags & NEEDS_SCALE_VALUE != 0,
        scale_unsupported: t.flags & RES_SCALE_UNSUPPORTED != 0,
        written: t.flags & IMAGE_STORE != 0,
    }
}

impl ShaderInterface {
    /// Build the interface of one translation.
    pub fn build(job: &Job, res: &Res) -> ShaderInterface {
        let textures: Vec<TexBinding> = res.textures.iter().map(tex).collect();
        let images: Vec<TexBinding> = res.images.iter().map(tex).collect();
        let mut words = Vec::new();
        for (kind, l) in [("texture", &textures), ("image", &images)] {
            for t in l {
                if let HandleSource::ConstBufferWord { slot, word } = t.handle {
                    words.push(HandleWord {
                        kind: kind.into(),
                        cb_slot: slot,
                        offset: word,
                    });
                }
            }
        }
        let storage: Vec<StorageBinding> = res
            .sbufs
            .iter()
            .map(|b| StorageBinding {
                binding: b.binding,
                slot: b.slot,
                cb_slot: b.cb_slot,
                cb_offset: b.cb_offset,
                written: b.flags & 1 != 0,
            })
            .collect();
        for b in &storage {
            if b.cb_slot >= 0 {
                words.push(HandleWord {
                    kind: "buffer".into(),
                    cb_slot: b.cb_slot,
                    offset: b.cb_offset,
                });
            }
        }
        words.sort();
        words.dedup();
        let fout = |rt: i64| {
            job.fout
                .iter()
                .find(|(n, _)| i64::from(*n) == rt)
                .map(|(_, t)| t.clone())
        };
        let outputs: Vec<Output> = if res.stage == "ps" {
            (0..8)
                .filter_map(|rt| {
                    let c = (res.frag_out_map >> (rt * 4)) & 0xf;
                    (c != 0).then(|| Output {
                        target: rt,
                        components: c,
                        ty: fout(rt),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let dual = outputs.iter().any(|o| o.target == 0) && outputs.iter().any(|o| o.target == 1);
        let frag_coord = res.stage == "ps" && res.uses_frag_coord;
        let sensitive = frag_coord
            || textures
                .iter()
                .chain(&images)
                .any(|t| t.scale_sensitive && !t.scale_unsupported);
        ShaderInterface {
            version: VERSION,
            key: res.key.clone(),
            stage: res.stage.clone(),
            cbufs: res.cbufs.iter().map(|b| (b.binding, b.slot)).collect(),
            storage,
            textures,
            images,
            handle_words: words,
            outputs,
            dual_source_candidate: dual,
            int_inputs: job
                .attr
                .iter()
                .filter(|(_, t)| t == "sint" || t == "uint")
                .map(|(n, t)| IntInput {
                    location: i64::from(*n),
                    ty: t.clone(),
                })
                .collect(),
            frag_coord,
            render_scale_sensitive: sensitive,
            compute: (res.stage == "cs").then(|| {
                let l = |i: usize| {
                    job.local
                        .as_ref()
                        .and_then(|a| a.get(i).copied())
                        .unwrap_or(1)
                };
                Compute {
                    local_size: [l(0), l(1), l(2)],
                    shared_bytes: job.shared,
                }
            }),
            geometry_stride: res.geometry_stride,
            fetch1d: res.fetch1d.clone(),
        }
    }

    /// The JSON value, members in a fixed order.
    pub fn to_value(&self) -> Value {
        let i = Value::Int;
        let b = Value::Bool;
        let tex = |t: &TexBinding| {
            let (src, a, c) = match t.handle {
                HandleSource::ConstBufferWord { slot, word } => ("cbuf_word", slot, word),
                HandleSource::Fixed { index } => ("fixed", -1, index),
            };
            Value::obj([
                ("binding", i(t.binding)),
                ("sampler_type", i(t.sampler_type)),
                ("format", Value::str(&t.format)),
                ("handle_source", Value::str(src)),
                ("handle_cbuf", i(a)),
                ("handle_word", i(c)),
                ("array", i(t.array)),
                ("scale_sensitive", b(t.scale_sensitive)),
                ("scale_unsupported", b(t.scale_unsupported)),
                ("written", b(t.written)),
            ])
        };
        let list = |l: Vec<Value>| Value::List(l);
        Value::obj([
            ("version", i(self.version)),
            ("key", Value::str(&self.key)),
            ("stage", Value::str(&self.stage)),
            (
                "cbufs",
                list(
                    self.cbufs
                        .iter()
                        .map(|(bi, s)| Value::obj([("binding", i(*bi)), ("slot", i(*s))]))
                        .collect(),
                ),
            ),
            (
                "storage",
                list(
                    self.storage
                        .iter()
                        .map(|s| {
                            Value::obj([
                                ("binding", i(s.binding)),
                                ("slot", i(s.slot)),
                                ("cb_slot", i(s.cb_slot)),
                                ("cb_offset", i(s.cb_offset)),
                                ("written", b(s.written)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("textures", list(self.textures.iter().map(tex).collect())),
            ("images", list(self.images.iter().map(tex).collect())),
            (
                "handle_words",
                list(
                    self.handle_words
                        .iter()
                        .map(|w| {
                            Value::obj([
                                ("kind", Value::str(&w.kind)),
                                ("cb_slot", i(w.cb_slot)),
                                ("offset", i(w.offset)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "outputs",
                list(
                    self.outputs
                        .iter()
                        .map(|o| {
                            Value::obj([
                                ("target", i(o.target)),
                                ("components", i(o.components)),
                                ("type", o.ty.as_ref().map_or(Value::Null, Value::str)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("dual_source_candidate", b(self.dual_source_candidate)),
            (
                "int_inputs",
                list(
                    self.int_inputs
                        .iter()
                        .map(|n| {
                            Value::obj([("location", i(n.location)), ("type", Value::str(&n.ty))])
                        })
                        .collect(),
                ),
            ),
            ("frag_coord", b(self.frag_coord)),
            ("render_scale_sensitive", b(self.render_scale_sensitive)),
            (
                "compute",
                self.compute.as_ref().map_or(Value::Null, |c| {
                    Value::obj([
                        (
                            "local_size",
                            list(c.local_size.iter().map(|&x| i(x)).collect()),
                        ),
                        ("shared_bytes", i(c.shared_bytes)),
                    ])
                }),
            ),
            ("geometry_stride", i(self.geometry_stride)),
            (
                "fetch1d",
                list(self.fetch1d.iter().map(|&x| i(x)).collect()),
            ),
        ])
    }

    /// The file text (`<key>.iface.json`).
    pub fn to_text(&self) -> String {
        to_json_string(&self.to_value())
    }

    /// Read a file back. `None` when the shape is wrong or the version is not [`VERSION`].
    pub fn from_value(v: &Value) -> Option<ShaderInterface> {
        let n = |x: &Value, k: &str| x.get(k).and_then(Value::as_i64);
        let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
        let flag = |x: &Value, k: &str| matches!(x.get(k), Some(Value::Bool(true)));
        let list = |x: &Value, k: &str| x.get(k).and_then(Value::as_list).map(<[Value]>::to_vec);
        if n(v, "version")? != VERSION {
            return None;
        }
        let tex = |l: Vec<Value>| -> Option<Vec<TexBinding>> {
            l.iter()
                .map(|t| {
                    let (c, w) = (n(t, "handle_cbuf")?, n(t, "handle_word")?);
                    Some(TexBinding {
                        binding: n(t, "binding")?,
                        sampler_type: n(t, "sampler_type")?,
                        format: s(t, "format")?,
                        handle: match s(t, "handle_source")?.as_str() {
                            "cbuf_word" => HandleSource::ConstBufferWord { slot: c, word: w },
                            "fixed" => HandleSource::Fixed { index: w },
                            _ => return None,
                        },
                        array: n(t, "array")?,
                        scale_sensitive: flag(t, "scale_sensitive"),
                        scale_unsupported: flag(t, "scale_unsupported"),
                        written: flag(t, "written"),
                    })
                })
                .collect()
        };
        Some(ShaderInterface {
            version: VERSION,
            key: s(v, "key")?,
            stage: s(v, "stage")?,
            cbufs: list(v, "cbufs")?
                .iter()
                .map(|b| Some((n(b, "binding")?, n(b, "slot")?)))
                .collect::<Option<_>>()?,
            storage: list(v, "storage")?
                .iter()
                .map(|b| {
                    Some(StorageBinding {
                        binding: n(b, "binding")?,
                        slot: n(b, "slot")?,
                        cb_slot: n(b, "cb_slot")?,
                        cb_offset: n(b, "cb_offset")?,
                        written: flag(b, "written"),
                    })
                })
                .collect::<Option<_>>()?,
            textures: tex(list(v, "textures")?)?,
            images: tex(list(v, "images")?)?,
            handle_words: list(v, "handle_words")?
                .iter()
                .map(|w| {
                    Some(HandleWord {
                        kind: s(w, "kind")?,
                        cb_slot: n(w, "cb_slot")?,
                        offset: n(w, "offset")?,
                    })
                })
                .collect::<Option<_>>()?,
            outputs: list(v, "outputs")?
                .iter()
                .map(|o| {
                    Some(Output {
                        target: n(o, "target")?,
                        components: n(o, "components")?,
                        ty: s(o, "type"),
                    })
                })
                .collect::<Option<_>>()?,
            dual_source_candidate: flag(v, "dual_source_candidate"),
            int_inputs: list(v, "int_inputs")?
                .iter()
                .map(|x| {
                    Some(IntInput {
                        location: n(x, "location")?,
                        ty: s(x, "type")?,
                    })
                })
                .collect::<Option<_>>()?,
            frag_coord: flag(v, "frag_coord"),
            render_scale_sensitive: flag(v, "render_scale_sensitive"),
            compute: match v.get("compute") {
                Some(c @ Value::Object(_)) => {
                    let l = c.get("local_size")?.as_list()?;
                    Some(Compute {
                        local_size: [
                            l.first()?.as_i64()?,
                            l.get(1)?.as_i64()?,
                            l.get(2)?.as_i64()?,
                        ],
                        shared_bytes: n(c, "shared_bytes")?,
                    })
                }
                _ => None,
            },
            geometry_stride: n(v, "geometry_stride")?,
            fetch1d: list(v, "fetch1d")?
                .iter()
                .filter_map(Value::as_i64)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(stage: &str) -> Job {
        let t = format!(
            r#"[{{"key": "k", "stage": "{stage}", "code": "x", "attr": {{"2": "sint", "3": "float"}},
               "fout": {{"1": "uint"}}, "local": [8, 4, 1], "shared": 256}}]"#
        );
        crate::job::parse_jobs(&t).unwrap().remove(0)
    }

    fn res(stage: &str) -> Res {
        let t = Tex {
            binding: 1,
            ty: 3,
            format: "Unknown".into(),
            cbuf_slot: 8,
            handle: 42,
            array: 1,
            separate: false,
            flags: NEEDS_SCALE_VALUE,
        };
        Res {
            key: "k".into(),
            stage: stage.into(),
            notes: vec![],
            cbufs: vec![crate::res::CBuf {
                binding: 0,
                slot: 1,
            }],
            sbufs: vec![crate::res::SBuf {
                binding: 3,
                slot: 0,
                cb_slot: 0,
                cb_offset: 16,
                flags: 1,
            }],
            textures: vec![
                t.clone(),
                Tex {
                    cbuf_slot: -1,
                    handle: 5,
                    flags: 0,
                    ..t
                },
            ],
            images: vec![],
            geometry_stride: 0,
            uses_frag_coord: false,
            frag_out_map: 0x1 | (0x3 << 4),
            fetch1d: vec![],
        }
    }

    #[test]
    fn derived_facts() {
        let i = ShaderInterface::build(&job("ps"), &res("ps"));
        assert!(i.dual_source_candidate && i.render_scale_sensitive && !i.frag_coord);
        assert_eq!(i.outputs.len(), 2);
        assert_eq!(i.outputs[1].ty.as_deref(), Some("uint"));
        assert_eq!(i.int_inputs.len(), 1);
        assert_eq!(i.handle_words.len(), 2);
        assert!(i.compute.is_none());
        let c = ShaderInterface::build(&job("cs"), &res("cs"));
        assert_eq!(c.compute.as_ref().unwrap().local_size, [8, 4, 1]);
        assert!(c.outputs.is_empty());
    }

    #[test]
    fn schema_round_trip_and_version() {
        for st in ["ps", "cs"] {
            let i = ShaderInterface::build(&job(st), &res(st));
            let text = i.to_text();
            let v = nxb_pyjson::parse(&text).unwrap();
            assert_eq!(ShaderInterface::from_value(&v), Some(i));
        }
        let mut v = ShaderInterface::build(&job("ps"), &res("ps")).to_value();
        v.set("version", Value::Int(VERSION + 1));
        assert_eq!(ShaderInterface::from_value(&v), None);
    }

    /// Every reference `res.json` of the corpus (`NXB_XLATE_REF`) gives an interface that survives the round trip.
    #[test]
    fn corpus_round_trip() {
        let Some(root) = std::env::var_os("NXB_XLATE_REF") else {
            return;
        };
        let mut dirs = vec![std::path::PathBuf::from(root)];
        let (mut n, mut dual) = (0, 0);
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.to_string_lossy().ends_with(".res.json") {
                    let r = Res::from_value(
                        &nxb_pyjson::parse(&std::fs::read_to_string(&p).unwrap()).unwrap(),
                    )
                    .unwrap();
                    let i = ShaderInterface::build(&job(&r.stage), &r);
                    let v = nxb_pyjson::parse(&i.to_text()).unwrap();
                    assert_eq!(ShaderInterface::from_value(&v), Some(i.clone()));
                    n += 1;
                    dual += usize::from(i.dual_source_candidate);
                }
            }
        }
        eprintln!("interfaces: {n}, dual-source capable: {dual}");
        assert!(n > 0);
    }
}
