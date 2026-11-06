//! The facts of a library that the emitter knows while it writes: driver bank reads, vertex inputs and raster facts.

use super::emit::{DrvReads, Shader};
use super::io::Plan;
use super::refl::{Binding, DrvRead, Raster, VertexInput};

/// The driver bank reads in bank order. A word index is a vec4 index, the offset is in bytes.
pub fn drv_reads(d: &DrvReads) -> Vec<DrvRead> {
    let mut banks: Vec<u32> = d.words.keys().chain(d.dynamic.iter()).copied().collect();
    banks.sort_unstable();
    banks.dedup();
    banks
        .into_iter()
        .map(|bank| DrvRead {
            bank,
            vec4_offsets: d
                .words
                .get(&bank)
                .map(|w| w.iter().map(|i| i * 16).collect())
                .unwrap_or_default(),
            dynamic: d.dynamic.contains(&bank),
        })
        .collect()
}

/// The vertex inputs in location order.
pub fn vertex_inputs(p: &Plan) -> Vec<VertexInput> {
    let mut v: Vec<VertexInput> = p
        .attributes
        .iter()
        .map(|(l, t)| VertexInput {
            location: *l,
            msl_type: t.clone(),
        })
        .collect();
    v.sort_by_key(|x| x.location);
    v
}

/// The raster facts. Only the pixel shader has any besides the default.
pub fn raster(shader: Shader, p: &Plan, discards: bool, bindings: &[Binding]) -> Raster {
    if shader == Shader::Fragment {
        super::facts_ps::raster(p, discards, bindings)
    } else {
        Raster::default()
    }
}

/// The bytes as lower case hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msl::abi::Kind;

    #[test]
    fn drv_reads_are_byte_offsets_in_bank_order() {
        let mut d = DrvReads::default();
        d.words.entry(2).or_default().insert(3);
        d.words.entry(0).or_default().insert(40);
        d.words.entry(0).or_default().insert(1);
        d.dynamic.insert(2);
        let r = drv_reads(&d);
        assert_eq!(r[0].bank, 0);
        assert_eq!(r[0].vec4_offsets, vec![16, 640]);
        assert!(!r[0].dynamic);
        assert_eq!(r[1].vec4_offsets, vec![48]);
        assert!(r[1].dynamic);
    }

    #[test]
    fn pixel_raster_follows_the_plan() {
        let p = Plan {
            colors: vec![1, 0, 1],
            primitive_id: true,
            ..Plan::default()
        };
        let mut img = Binding::new("img", "texture2d<float, access::write>", 1, Kind::Img);
        img.rw = None;
        let r = raster(Shader::Fragment, &p, false, &[img]);
        assert_eq!(r.colors, Some(vec![0, 1]));
        assert_eq!(r.discard, Some(true));
        assert!(r.primitive_id);
        assert_eq!(raster(Shader::Vertex, &p, true, &[]), Raster::default());
    }

    #[test]
    fn hex_is_lower_case() {
        assert_eq!(hex(&[0, 0xab, 0x1]), "00ab01");
    }
}
