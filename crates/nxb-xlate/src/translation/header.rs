//! `ShaderHeader` (the 0x50-byte SPH of a graphics shader).

use crate::attr::Stage;

/// `PixelImap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelImap {
    Unused = 0,
    Constant = 1,
    Perspective = 2,
    ScreenLinear = 3,
}

impl PixelImap {
    fn from(v: u8) -> PixelImap {
        match v & 3 {
            0 => PixelImap::Unused,
            1 => PixelImap::Constant,
            2 => PixelImap::Perspective,
            _ => PixelImap::ScreenLinear,
        }
    }
}

/// `ImapPixelType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImapPixelType {
    pub x: PixelImap,
    pub y: PixelImap,
    pub z: PixelImap,
    pub w: PixelImap,
}

impl ImapPixelType {
    /// `GetFirstUsedType`.
    pub fn first_used_type(&self) -> PixelImap {
        [self.x, self.y, self.z]
            .into_iter()
            .find(|v| *v != PixelImap::Unused)
            .unwrap_or(self.w)
    }
}

/// `ShaderHeader`: the fields the translator reads.
#[derive(Clone, Debug)]
pub struct ShaderHeader {
    pub stage: Stage,
    pub gp_passthrough: bool,
    pub local_memory_low_size: i32,
    pub local_memory_high_size: i32,
    pub local_memory_crs_size: i32,
    pub threads_per_input_primitive: i32,
    pub output_topology: i32,
    pub max_output_vertex_count: i32,
    pub imap_types: Vec<ImapPixelType>,
    pub omap_targets: i32,
    pub omap_sample_mask: bool,
    pub omap_depth: bool,
}

fn extract(v: i32, lsb: u32, bits: u32) -> i32 {
    (((v as u32) >> lsb) & ((1u32 << bits) - 1)) as i32
}

impl ShaderHeader {
    /// Parse the header at the start of `code`.
    pub fn parse(code: &[u8]) -> Result<ShaderHeader, String> {
        if code.len() < 0x50 {
            return Err("shader shorter than its header".into());
        }
        #[allow(clippy::chunks_exact_to_as_chunks)]
        let h: Vec<i32> = code[..0x50]
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let stage = match extract(h[0], 10, 4) {
            0 | 1 => Stage::Vertex, // 0 (compute) is invalid in a header
            2 => Stage::TessellationControl,
            3 => Stage::TessellationEvaluation,
            4 => Stage::Geometry,
            5 => Stage::Fragment,
            n => return Err(format!("unknown shader stage {n} in the header")),
        };
        let imap_types = (0..32)
            .map(|i| {
                let b = (h[6 + (i >> 2)] >> ((i & 3) * 8)) as u8;
                ImapPixelType {
                    x: PixelImap::from(b),
                    y: PixelImap::from(b >> 2),
                    z: PixelImap::from(b >> 4),
                    w: PixelImap::from(b >> 6),
                }
            })
            .collect();
        Ok(ShaderHeader {
            stage,
            gp_passthrough: extract(h[0], 24, 1) != 0,
            local_memory_low_size: extract(h[1], 0, 24),
            local_memory_high_size: extract(h[2], 0, 24),
            threads_per_input_primitive: extract(h[2], 24, 8),
            local_memory_crs_size: extract(h[3], 0, 24),
            output_topology: extract(h[3], 24, 4),
            max_output_vertex_count: extract(h[4], 0, 12),
            imap_types,
            omap_targets: h[18],
            omap_sample_mask: extract(h[19], 0, 1) != 0,
            omap_depth: extract(h[19], 1, 1) != 0,
        })
    }

    /// `GetLocalMemorySize`.
    pub fn local_memory_size(&self) -> i32 {
        self.local_memory_low_size + self.local_memory_high_size + self.local_memory_crs_size / 32
    }
}
