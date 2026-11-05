//! The translator's queries (`IGpuAccessor`), answered from the job as `dmsh_xlate` does.

use crate::job::Job;
use crate::types::{AttributeType, HandleType, SamplerType, parse_format, unpack_offsets};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const TEX_BASE: i32 = 0x20;
const IMG_BASE: i32 = 0x120;
const IMG_SLOTS: i32 = 8;
const SEP_TEX_BASE: i32 = 0x168;
const SEP_TEX_SLOT0: i32 = 128;
const SAMP_BASE: i32 = 0x568;
/// Handle words in c2: one every 8 bytes up to the end of the separate sampler table.
pub const C2_HANDLE_WORDS: i32 = (SAMP_BASE + 8 * 128) / 8;

/// A set and binding pair (the set is always 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetBinding {
    pub set: i32,
    pub binding: i32,
}

/// What the translator reads as its graphics state.
#[derive(Clone, Debug)]
pub struct GraphicsState {
    pub early_z: bool,
    /// `InputTopology` as a number (Triangles when the job has no topology)
    pub topology: i32,
    pub attr: [AttributeType; 32],
    pub fout: [AttributeType; 8],
}

/// One accessor per translation attempt.
pub struct Accessor<'a> {
    job: &'a Job,
    binding: i32,
    pub notes: Vec<String>,
    image_types: Option<BTreeMap<i32, SamplerType>>,
    texture_types: Option<BTreeMap<i32, SamplerType>>,
    pub image_query_guessed: bool,
    guessed_handles: Vec<i32>,
    /// Texture slots read by a 1D integer fetch that became a wrapped 2D read.
    pub fetch1d: BTreeSet<i32>,
}

/// A texture or image as the translator reports it (the fields the types queries read).
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub ty: SamplerType,
    pub cbuf_slot: i32,
    pub handle: i32,
    pub array_length: i32,
}

/// `Slot(handle, cbufSlot, image)`: the answer table slot of a handle, or -1.
pub fn slot(handle: i32, cbuf_slot: i32, image: bool) -> i32 {
    if cbuf_slot >= 0 && (cbuf_slot & 0xffff) != 2 {
        return -1;
    }
    let (w0, w1, ty) = unpack_offsets(handle);
    let words: &[i32] = if matches!(
        ty,
        HandleType::CombinedSampler | HandleType::SeparateConstantSamplerHandle
    ) {
        &[w0]
    } else {
        &[w0, w1]
    };
    for &w in words {
        let b = w * 4;
        if b % 8 != 0 {
            continue;
        }
        if image && (IMG_BASE..IMG_BASE + 8 * IMG_SLOTS).contains(&b) {
            return (b - IMG_BASE) / 8;
        }
        if !image && (TEX_BASE..SEP_TEX_BASE).contains(&b) {
            return (b - TEX_BASE) / 8;
        }
        if !image && (SEP_TEX_BASE..SAMP_BASE).contains(&b) {
            return SEP_TEX_SLOT0 + (b - SEP_TEX_BASE) / 8;
        }
    }
    -1
}

/// `ImageTypesOf`: type by c2 handle of the images a program found.
pub fn image_types_of(images: &[Binding]) -> BTreeMap<i32, SamplerType> {
    let mut m = BTreeMap::new();
    for im in images {
        if im.cbuf_slot == 2 || im.cbuf_slot < 0 {
            m.insert(im.handle, im.ty.and(SamplerType::MASK | SamplerType::ARRAY));
        }
    }
    m
}

fn type_of(name: &str) -> SamplerType {
    match name {
        "1d" => SamplerType::TEXTURE_1D,
        "1darray" => SamplerType::TEXTURE_1D.with(SamplerType::ARRAY),
        "buffer" => SamplerType::TEXTURE_BUFFER,
        "2d" => SamplerType::TEXTURE_2D,
        "2darray" => SamplerType::TEXTURE_2D.with(SamplerType::ARRAY),
        "2dms" => SamplerType::TEXTURE_2D.with(SamplerType::MULTISAMPLE),
        "2dmsarray" => SamplerType::TEXTURE_2D.with(SamplerType::MULTISAMPLE | SamplerType::ARRAY),
        "3d" => SamplerType::TEXTURE_3D,
        "cube" => SamplerType::TEXTURE_CUBE,
        "cubearray" => SamplerType::TEXTURE_CUBE.with(SamplerType::ARRAY),
        _ => SamplerType::TEXTURE_2D,
    }
}

fn attr(s: &str) -> AttributeType {
    match s {
        "sint" => AttributeType::Sint,
        "uint" => AttributeType::Uint,
        _ => AttributeType::Float,
    }
}

impl<'a> Accessor<'a> {
    /// A first attempt.
    pub fn new(job: &'a Job) -> Self {
        Self::with_types(job, None, None)
    }

    /// A retry with the types a first run found.
    pub fn with_types(
        job: &'a Job,
        image_types: Option<BTreeMap<i32, SamplerType>>,
        texture_types: Option<BTreeMap<i32, SamplerType>>,
    ) -> Self {
        Accessor {
            job,
            binding: if job.geometry_topology.is_some() {
                40
            } else {
                1
            },
            notes: Vec::new(),
            image_types,
            texture_types,
            image_query_guessed: false,
            guessed_handles: Vec::new(),
            fetch1d: BTreeSet::new(),
        }
    }

    pub fn job(&self) -> &Job {
        self.job
    }

    /// `TextureTypesOf`: the retry's texture types by slot.
    pub fn texture_types_of(&self, textures: &[Binding]) -> BTreeMap<i32, SamplerType> {
        let mut types: BTreeMap<i32, SamplerType> = BTreeMap::new();
        let mut mixed = HashSet::new();
        let keep = SamplerType::MASK | SamplerType::ARRAY | SamplerType::MULTISAMPLE;
        for t in textures {
            if t.array_length != 1 {
                continue;
            }
            let ty = t.ty.and(keep);
            let s = slot(t.handle, t.cbuf_slot, false);
            if s < 0 || (ty == SamplerType::TEXTURE_2D && self.guessed_handles.contains(&t.handle))
            {
                continue;
            }
            match types.get(&s) {
                Some(seen) if *seen != ty => {
                    mixed.insert(s);
                }
                _ => {
                    types.insert(s, ty);
                }
            }
        }
        let guessed: HashSet<i32> = self
            .guessed_handles
            .iter()
            .map(|&h| slot(h, -1, false))
            .collect();
        types
            .into_iter()
            .filter(|(k, v)| {
                !mixed.contains(k) && guessed.contains(k) && *v != SamplerType::TEXTURE_2D
            })
            .collect()
    }

    pub fn query_constant_buffer1_size(&self) -> usize {
        self.job.c1.len()
    }

    pub fn constant_buffer1_read(&mut self, offset: i32) -> u32 {
        let c1 = &self.job.c1;
        if offset >= 0 && offset as usize + 4 <= c1.len() {
            let o = offset as usize;
            return u32::from_le_bytes([c1[o], c1[o + 1], c1[o + 2], c1[o + 3]]);
        }
        self.notes.push(format!(
            "c1 read out of bounds +{:X} (c1 length {})",
            offset,
            c1.len()
        ));
        0
    }

    pub fn query_ampere_compute_c0_layout(&self) -> Option<&[i32]> {
        self.job.nvn2_cs.as_deref()
    }

    pub fn query_ampere_stage_memory(&self) -> Option<&[i32]> {
        self.job.stage_memory.as_deref()
    }

    /// The translator's own messages go into the notes, marked.
    pub fn log(&mut self, message: &str) {
        self.notes.push(format!("translator: {message}"));
    }

    pub fn create_constant_buffer_binding(&mut self, _index: i32) -> SetBinding {
        self.next(1)
    }

    pub fn create_image_binding(&mut self, count: i32, _is_buffer: bool) -> SetBinding {
        self.next(count)
    }

    pub fn create_storage_buffer_binding(&mut self, _index: i32) -> SetBinding {
        self.next(1)
    }

    pub fn create_texture_binding(&mut self, count: i32, _is_buffer: bool) -> SetBinding {
        self.next(count)
    }

    fn next(&mut self, count: i32) -> SetBinding {
        let b = self.binding;
        self.binding += count;
        SetBinding { set: 0, binding: b }
    }

    pub fn query_sampler_array_length_from_pool(&self) -> i32 {
        1 << 12
    }

    pub fn query_texture_array_length_from_buffer(&self, slot: i32) -> i32 {
        if slot == 2 { C2_HANDLE_WORDS } else { 32 }
    }

    pub fn query_texture_array_length_from_pool(&self) -> i32 {
        1 << 20
    }

    fn get(&self, table: &str, slot: i32) -> Option<String> {
        match table {
            "tex" => self.job.tex.get(&slot).cloned(),
            _ => self.job.img.get(&slot).and_then(|e| e.0.clone()),
        }
    }

    /// `QuerySamplerType`.
    pub fn query_sampler_type(&mut self, handle: i32, cbuf_slot: i32) -> SamplerType {
        let s = slot(handle, cbuf_slot, false);
        let is = slot(handle, cbuf_slot, true);
        let mut t = if s >= 0 { self.get("tex", s) } else { None };
        if t.is_none() && is >= 0 {
            t = self.get("img", is);
        }
        if t.is_none() && is >= 0 && cbuf_slot < 0 {
            if let Some(it) = self
                .image_types
                .as_ref()
                .and_then(|m| m.get(&handle))
                .copied()
            {
                self.notes.push(format!(
                    "texture handle {handle:X} is image slot {is}: type {} from the shader's image instructions",
                    it.name()
                ));
                return it;
            }
            self.image_query_guessed = true;
        }
        if t.is_none() && s >= 0 && is < 0 {
            if let Some(tt) = self.texture_types.as_ref().and_then(|m| m.get(&s)).copied() {
                self.notes.push(format!(
                    "texture handle {handle:X} is texture slot {s}: type {} from the shader's other instructions",
                    tt.name()
                ));
                return tt;
            }
            if !self.guessed_handles.contains(&handle) {
                self.guessed_handles.push(handle);
            }
        }
        match t {
            None => {
                self.notes.push(format!(
                    "texture handle {handle:X} (constant buffer {cbuf_slot}, slot {s}) has no type in the answer table, using 2D"
                ));
                SamplerType::TEXTURE_2D
            }
            Some(t) => type_of(&t),
        }
    }

    pub fn record_texel_fetch_1d(&mut self, handle: i32) {
        let s = slot(handle, -1, false);
        if s >= 0 {
            self.fetch1d.insert(s);
        }
    }

    /// `QueryTextureFormat`: index into [`crate::types::TEXTURE_FORMATS`].
    pub fn query_texture_format(&mut self, handle: i32, cbuf_slot: i32) -> usize {
        let s = slot(handle, cbuf_slot, true);
        let f = if s >= 0 {
            self.job.img.get(&s).and_then(|e| e.1.clone())
        } else {
            None
        };
        match f {
            None => {
                self.notes.push(format!(
                    "image handle {handle:X} slot {s} has no format in the answer table, using R8G8B8A8Unorm"
                ));
                25
            }
            Some(f) => parse_format(&f).unwrap_or(25),
        }
    }

    fn local(&self, i: usize) -> i32 {
        match &self.job.local {
            Some(a) if a.len() > i => a[i] as i32,
            _ => 1,
        }
    }

    pub fn query_compute_local_size(&self) -> [i32; 3] {
        [self.local(0), self.local(1), self.local(2)]
    }

    pub fn query_compute_shared_memory_size(&self) -> i32 {
        self.job.shared as i32
    }

    pub fn query_geometry_input_fixed(&self) -> bool {
        self.job.geometry_topology.is_some()
    }

    pub fn query_host_supports_geometry_shader_passthrough(&self) -> bool {
        self.job.geometry_topology.is_none()
    }

    pub fn query_graphics_state(&self) -> GraphicsState {
        let mut a = [AttributeType::Float; 32];
        let mut f = [AttributeType::Float; 8];
        for (k, v) in &self.job.attr {
            if let Some(slot) = a.get_mut(*k as usize) {
                *slot = attr(v);
            }
        }
        for (k, v) in &self.job.fout {
            if let Some(slot) = f.get_mut(*k as usize) {
                *slot = attr(v);
            }
        }
        GraphicsState {
            early_z: self.job.early_fragment_tests,
            // InputTopology.Triangles is 3 in the translator
            topology: self.job.geometry_topology.unwrap_or(3),
            attr: a,
            fout: f,
        }
    }

    pub fn query_host_supports_texture_shadow_lod(&self) -> bool {
        true
    }

    pub fn query_host_supports_depth_clip_control(&self) -> bool {
        false
    }

    pub fn query_host_reduced_precision(&self) -> bool {
        self.job.reduced
    }

    pub fn query_host_supports_shader_float64(&self) -> bool {
        !self.job.softfloat64
    }
}

impl nxb_maxwell::decoder::Cb1Reader for Accessor<'_> {
    fn constant_buffer1_read(&mut self, offset: i32) -> u32 {
        Accessor::constant_buffer1_read(self, offset)
    }

    fn constant_buffer1_size(&self) -> i32 {
        self.query_constant_buffer1_size() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots() {
        // combined handle: word 8 is byte 0x20, texture slot 0
        assert_eq!(slot(8, -1, false), 0);
        assert_eq!(slot(8 + 2, -1, false), 1);
        assert_eq!(slot(0x48, -1, true), 0);
        assert_eq!(slot(8, 5, false), -1);
        assert_eq!(slot(0x5a, -1, false), SEP_TEX_SLOT0);
    }

    #[test]
    fn names() {
        assert_eq!(
            SamplerType::TEXTURE_2D.with(SamplerType::ARRAY).name(),
            "Texture2D, Array"
        );
        assert_eq!(SamplerType::TEXTURE_3D.name(), "Texture3D");
        assert_eq!(
            SamplerType(3 | 0x200 | 0x100).name(),
            "Texture2D, Array, Multisample"
        );
    }
}
