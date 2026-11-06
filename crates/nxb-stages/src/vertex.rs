//! The vertex path of `dmsh_build.finish`: translation (with the integer-input re-translation), spirv-cross, the
//! passes, the depth, capture, discard and layer entry points, the reflection and the `vin` / `pull` fields.

use crate::front::{PsInfo, VsExtra, ps_reflection};
use nxb_pyjson::{Value, dumps};
use nxb_shader::msl::{common_passes, cross_args, vertex as vx};
use nxb_shader::reflect::reflect;
use nxb_shader::variants::{pull, vin};
use nxb_tool::{Translator, XlateJob};
use std::path::Path;

/// spirv-cross: (module, arguments) → MSL.
pub type CrossFn<'a> = dyn Fn(&[u8], &[String]) -> Result<String, String> + 'a;

/// What the vertex path produced.
pub struct VsOut {
    /// The full source (`<key>_b.metal`).
    pub full: String,
    /// The reflection up to the compiler fields.
    pub refl: Value,
}

/// `spv_single_lane` is not ported: refuse a module with subgroup operations or builtins it would rewrite.
pub fn single_lane_guard(spv: &[u8]) -> Result<(), String> {
    let w: Vec<u32> = spv
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    let mut i = 5;
    const OPS: [u32; 11] = [333, 334, 335, 336, 337, 338, 339, 345, 346, 347, 348];
    const BUILTINS: [u32; 7] = [36, 41, 4416, 4417, 4418, 4419, 4420];
    while i < w.len() {
        let (n, op) = ((w[i] >> 16) as usize, w[i] & 0xffff);
        if OPS.contains(&op)
            || (op == 71
                && n > 3
                && w.get(i + 2) == Some(&11)
                && w.get(i + 3).is_some_and(|b| BUILTINS.contains(b)))
        {
            return Err(
                "spv_single_lane: subgroup operations in a vertex shader are not ported yet".into(),
            );
        }
        if n == 0 {
            break;
        }
        i += n;
    }
    Ok(())
}

/// One vertex shader from its job. `cross(spv) -> MSL` runs spirv-cross.`pixel_sources` are the paired pixel
/// shaders' final sources (`vs_pad`).
#[allow(clippy::too_many_arguments, reason = "stage wiring; one call site")]
pub fn vertex(
    key: &str,
    job: &Value,
    tr: &dyn Translator,
    workdir: &Path,
    cross: &CrossFn<'_>,
    pixel_sources: &[String],
    info: (&str, String, &str, String),
    chain: &str,
    first: Option<(Vec<u8>, Value)>,
) -> Result<VsOut, (String, String)> {
    let fail = |r: &str, d: String| (r.to_string(), d);
    if job.get("tf").is_some() {
        return Err(fail("not ported", "transform feedback (vs_tf)".into()));
    }
    let run = |j: &Value| -> Result<(Vec<u8>, Value), (String, String)> {
        let r = tr
            .translate(&[XlateJob {
                key: key.to_string(),
                job_json: dumps(j),
            }])
            .map_err(|e| fail("translation failed", e.to_string()))?;
        let o = r
            .into_iter()
            .next()
            .ok_or_else(|| fail("translation failed", "no result".into()))?;
        let o = o
            .outcome
            .map_err(|f| fail("translation failed", f.detail))?;
        let res = nxb_pyjson::parse(&String::from_utf8_lossy(&o.resources_json))
            .map_err(|e| fail("translation failed", e.to_string()))?;
        Ok((o.spirv, res))
    };
    let (mut spv, mut res) = match first {
        Some(f) => f,
        None => run(job)?,
    };
    single_lane_guard(&spv).map_err(|e| fail("not ported", e))?;
    let mut src = cross(&spv, &cross_args(&spv).map_err(|e| fail("MSL failed", e))?)
        .map_err(|e| fail("MSL failed", e))?;
    let attr = job.get("attr").cloned().unwrap_or(Value::Object(vec![]));
    let extra: Vec<(String, String)> = vx::int_inputs(&src)
        .into_iter()
        .filter(|(l, _)| attr.get(l).is_none())
        .collect();
    if !extra.is_empty() {
        let mut j2 = job.clone();
        let mut a = attr.clone();
        for (l, t) in extra {
            a.set(&l, Value::str(t));
        }
        j2.set("attr", a);
        let (s2, r2) = run(&j2).map_err(|(_, d)| {
            fail(
                "translation failed",
                format!("integer-input retranslation: {d}"),
            )
        })?;
        spv = s2;
        res = r2;
        single_lane_guard(&spv).map_err(|e| fail("not ported", e))?;
        src = cross(&spv, &cross_args(&spv).map_err(|e| fail("MSL failed", e))?)
            .map_err(|e| fail("MSL failed", e))?;
    }
    let _ = workdir;
    let src = common_passes(&src)?;
    let layered = vx::vs_layer(&vx::vs_init(&src)).map_err(|e| fail("vertex failed", e))?;
    let src = vx::vs_base_instance(&layered);
    let (src, pad) = if pixel_sources.is_empty() {
        (src, vec![])
    } else {
        vx::vs_pad(&src, pixel_sources)
    };
    let capture = nxb_shader::msl::capture(&src).map_err(|e| fail("geometry failed", e))?;
    let (single, mut full) = vx::vs_depth(&src).map_err(|e| fail("vertex failed", e))?;
    full.push_str(&capture);
    let binds = reflect(key, "vs", &single, &res).map_err(|e| fail("reflection failed", e))?;
    let discard = vx::vs_writes(&single, &binds);
    if discard {
        full.push_str(&vx::vs_discard(&single).map_err(|e| fail("vertex failed", e))?);
    }
    let (full, layer) = vx::vs_layer_entry(&full).map_err(|e| fail("vertex failed", e))?;
    let (name, k, pc, c1_hex) = info;
    let pi = PsInfo {
        key,
        chain,
        source: format!("{name}#{k}"),
        pc_paired: pc,
        stage: "vs",
        src: &single,
        fu: None,
        res: &res,
        job,
        c1_hex,
        vs: Some(VsExtra {
            discard,
            layer,
            pad,
        }),
    };
    let mut refl = ps_reflection(&pi).map_err(|e| fail("reflection failed", e))?;
    if let Ok((_, shadows)) = vin::variant(&full) {
        refl.set(
            "vin",
            Value::obj([
                ("entry", Value::str("main0")),
                ("entry_z", Value::str("main0_z")),
                ("shadow", shadows),
            ]),
        );
    }
    if let Ok((_, info)) = pull::variant(&full) {
        let kept = match info {
            Value::Object(m) => {
                Value::Object(m.into_iter().filter(|(_, v)| *v != Value::Null).collect())
            }
            v => v,
        };
        refl.set("pull", kept);
    }
    Ok(VsOut { full, refl })
}
