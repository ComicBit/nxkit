//! The reflection `_b.json` of a pixel or compute shader as `dmsh_build.finish` assembles it (fields in Python's order),
//! before the compiler fields `write_refl` adds.

use nxb_pyjson::Value;
use nxb_shader::reflect::{
    drv_reads, entry, raster_ps, raster_vs, reflect, unfillable, vertex_inputs,
};

/// What `finish` knows about one pixel or compute shader.
pub struct PsInfo<'a> {
    /// Shader key.
    pub key: &'a str,
    /// Chain version.
    pub chain: &'a str,
    /// `<package>#<index>`.
    pub source: String,
    /// `pc_of[key]`.
    pub pc_paired: &'a str,
    /// `ps` or `cs`.
    pub stage: &'a str,
    /// The source after the passes, without the extra entry points.
    pub src: &'a str,
    /// `fs_uint` produced `main0_u` (and the `main0_m` mask).
    pub fu: Option<u32>,
    /// The translator's `res.json`.
    pub res: &'a Value,
    /// The job object.
    pub job: &'a Value,
    /// c1 as hex.
    pub c1_hex: String,
    /// Vertex shaders: the extra entry points and padding.
    pub vs: Option<VsExtra>,
}

/// What the vertex path adds to the reflection.
pub struct VsExtra {
    /// `main0_discard` was added.
    pub discard: bool,
    /// `vs_layer_entry`'s `{loc, comp}`.
    pub layer: Option<Value>,
    /// `vs_pad`'s padded locations.
    pub pad: Vec<i64>,
}

/// The reflection dict up to `vs_pad` (side fields, `fetch` and compiler fields are added by the caller).
pub fn ps_reflection(i: &PsInfo<'_>) -> Result<Value, String> {
    let binds = reflect(i.key, i.stage, i.src, i.res)?;
    let (kind, name) = entry(i.src).map_or((Value::Null, "main0".to_string()), |(k, n)| {
        (Value::str(k), n)
    });
    let mut r = Value::obj([
        ("key", Value::str(format!("{}_b", i.key))),
        ("route", Value::str("B")),
        ("chain", Value::str(i.chain)),
        ("dm_ctl", Value::str(i.key)),
        ("source", Value::str(i.source.clone())),
        ("pc_paired", Value::str(i.pc_paired)),
        ("stage", Value::str(i.stage)),
        ("entry", Value::str(name)),
        ("msl_entry_kind", kind),
    ]);
    if let Some(v) = &i.vs {
        r.set("entry_z", Value::str("main0_z"));
        r.set("entry_geometry_capture", Value::str("main0_gs"));
        if v.discard {
            r.set("entry_discard", Value::str("main0_discard"));
        }
        if let Some(l) = &v.layer {
            r.set("entry_layer", Value::str("main0_l"));
            r.set("entry_layer_z", Value::str("main0_lz"));
            r.set("layer_from", l.clone());
        }
    }
    if let Some(mask) = i.fu {
        r.set("entry_u", Value::str("main0_u"));
        if mask != 0 {
            r.set("entry_m", Value::str("main0_m"));
            r.set("uint_mask_m", Value::Int(i64::from(mask)));
        }
    }
    r.set(
        "threadgroup_size",
        i.job.get("local").cloned().unwrap_or(Value::Null),
    );
    r.set("c1", Value::str(i.c1_hex.clone()));
    let mut notes: Vec<Value> = i
        .res
        .get("notes")
        .and_then(Value::as_list)
        .unwrap_or(&[])
        .to_vec();
    if i.job.get("no_facts") == Some(&Value::Bool(true)) {
        notes.push(Value::str("not in the answer table, queries use defaults"));
    }
    r.set("notes", Value::List(notes));
    let dr = drv_reads(i.src);
    let unf = unfillable(&dr, i.stage);
    r.set("drv_reads", dr);
    r.set("drv_unfillable", unf);
    r.set("bindings", binds);
    r.set("vertex_inputs", vertex_inputs(i.src));
    let raster = match i.stage {
        "ps" => raster_ps(i.src),
        "vs" => raster_vs(i.src),
        _ => Value::Object(vec![]),
    };
    r.set("raster", raster);
    let pad = i.vs.as_ref().map_or(Vec::new(), |v| {
        v.pad.iter().map(|x| Value::Int(*x)).collect()
    });
    r.set("vs_pad", Value::List(pad));
    Ok(r)
}
