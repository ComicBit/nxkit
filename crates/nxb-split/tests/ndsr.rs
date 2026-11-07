//! The record-start score of an NDSR pack counts only complete 16-byte records, also in a short pack.
//! Synthetic bytes only.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use nxb_split::engines::ndsr_shaders;
use nxb_split::shaders::BahsVar;

/// Where a shader starts, its control section and its code area.
type Made = (usize, Vec<u8>, Vec<u8>);
/// Place, stage, control section and code area.
type Row = (String, &'static str, Vec<u8>, Vec<u8>);

/// A pack with both header layouts possible, a zero record and one shader per SPH word. The records start at `start`.
fn pack(start: usize, stages: &[u32]) -> (Vec<u8>, Vec<Made>) {
    let mut d = vec![0u8; 0x40];
    d[..4].copy_from_slice(b"NDSR");
    let mut shaders = Vec::new();
    for (i, sph) in stages.iter().enumerate() {
        let mut ctl = vec![0u8; 0x48];
        ctl[..8].copy_from_slice(&[0x34, 0x12, 0x76, 0x98, 0xc9, 0, 0, 0]);
        let mut code = sph.to_le_bytes().to_vec();
        code.extend_from_slice(&[0u8; 0x7c]);
        ctl[0x44..0x48].copy_from_slice(&(code.len() as u32).to_le_bytes());
        let offset = d.len();
        let at = start + 16 * i;
        let rec = [
            offset as u32,
            (ctl.len() as u32) << 4,
            (offset + ctl.len()) as u32,
            (code.len() as u32) << 4,
        ];
        for (j, w) in rec.iter().enumerate() {
            d[at + 4 * j..at + 4 * j + 4].copy_from_slice(&w.to_le_bytes());
        }
        d.extend_from_slice(&ctl);
        d.extend_from_slice(&code);
        shaders.push((offset, ctl, code));
    }
    (d, shaders)
}

fn found(d: &[u8], label: &str) -> (Vec<Row>, BahsVar) {
    let mut var = BahsVar::new();
    let f = ndsr_shaders(d, label, &mut var).unwrap();
    (
        f.into_iter()
            .map(|f| (f.place, f.stage, f.ctl, f.code))
            .collect(),
        var,
    )
}

#[test]
fn a_short_empty_pack_does_not_stop_on_a_partial_record() {
    for size in 0x20..0x125 {
        let mut d = b"NDSR".to_vec();
        d.resize(size, 0);
        assert!(found(&d, "/empty").0.is_empty(), "size {size:#x}");
    }
}

#[test]
fn a_127_byte_pack_is_read() {
    let mut d = b"NDSR".to_vec();
    d.resize(127, 0);
    assert!(found(&d, "/short").0.is_empty());
}

#[test]
fn a_short_shader_is_kept_for_both_record_starts() {
    for start in [0x10, 0x14] {
        let (data, shaders) = pack(start, &[0x0461]);
        assert_eq!(data.len(), 0x108);
        let (offset, ctl, code) = shaders[0].clone();
        for tail in 0..16 {
            let mut d = data.clone();
            d.extend(std::iter::repeat_n(0u8, tail));
            let (f, _) = found(&d, "/short");
            let want = vec![(
                format!("/short#NDSR@{offset:#x}"),
                "vs",
                ctl.clone(),
                code.clone(),
            )];
            assert_eq!(f, want, "start {start:#x} tail {tail}");
        }
    }
}

#[test]
fn a_vertex_and_pixel_pair_is_kept_for_both_record_starts() {
    for start in [0x10, 0x14] {
        let (data, shaders) = pack(start, &[0x0461, 0x1462]);
        let (f, var) = found(&data, "/pair");
        let want: Vec<_> = shaders
            .iter()
            .zip(["vs", "ps"])
            .map(|((o, c, k), st)| (format!("/pair#NDSR@{o:#x}"), st, c.clone(), k.clone()))
            .collect();
        assert_eq!(f, want, "start {start:#x}");
        let mut pairs = BahsVar::new();
        pairs.insert("/pair#NDSR@0x40".into(), "/pair#NDSR#0x40".into());
        pairs.insert("/pair#NDSR@0x108".into(), "/pair#NDSR#0x40".into());
        assert_eq!(var, pairs, "start {start:#x}");
    }
}
