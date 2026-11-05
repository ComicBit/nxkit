//! `TranslatorContext` and `Translator.CreateContext`/`DecodeShader`.

use super::definitions::Definitions;
use super::header::ShaderHeader;
use crate::accessor::Accessor;
use crate::attr::Stage;
use crate::maxwell::decoder::{DecodedProgram, decode};

/// `TranslationOptions` flags the job driver sets (target is always Vulkan SPIR-V, Metal image atomics on).
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub debug_mode: bool,
    pub compute: bool,
    pub ampere: bool,
}

const HEADER_SIZE: usize = 0x50;
const AMPERE_HEADER_SIZE: usize = 0x80;

/// The decoded shader with its definitions.
#[derive(Clone, Debug)]
pub struct TranslatorContext {
    pub definitions: Definitions,
    pub options: Options,
    pub program: DecodedProgram,
    pub local_memory_size: i32,
    pub size: i32,
}

impl TranslatorContext {
    pub fn stage(&self) -> Stage {
        self.definitions.stage
    }

    /// `Translator.CreateContext`: header, definitions, decode.
    pub fn create(
        code: &[u8],
        acc: &mut Accessor,
        options: Options,
    ) -> Result<TranslatorContext, String> {
        let gs = acc.query_graphics_state();
        let (mut definitions, local_memory_size, start) = if options.compute {
            let s = acc.query_compute_local_size();
            (
                Definitions::compute(s, gs),
                0x1000, /* IGpuAccessor default */
                0,
            )
        } else {
            let h = ShaderHeader::parse(code)?;
            let hs = if options.ampere {
                AMPERE_HEADER_SIZE
            } else {
                HEADER_SIZE
            };
            (Definitions::graphics(&h, gs), h.local_memory_size(), hs)
        };
        let (program, idx) = if options.ampere {
            crate::maxwell::ampere_decode::decode(definitions.stage, acc, code, start as u64)?
        } else {
            decode(definitions.stage, acc, code, start as u64)?
        };
        definitions.ia_indexing |= idx.input_indexing;
        definitions.oa_indexing |= idx.output_indexing;
        let max_end = program
            .functions
            .iter()
            .flat_map(|f| f.blocks.iter().map(|b| b.end_address))
            .max()
            .unwrap_or(0);
        let size = max_end as i32 + if options.compute { 0 } else { start as i32 };
        Ok(TranslatorContext {
            definitions,
            options,
            program,
            local_memory_size,
            size,
        })
    }
}

impl From<Stage> for super::props::ShaderStage {
    fn from(s: Stage) -> Self {
        use super::props::ShaderStage as P;
        match s {
            Stage::Compute => P::Compute,
            Stage::Vertex => P::Vertex,
            Stage::TessellationControl => P::TessellationControl,
            Stage::TessellationEvaluation => P::TessellationEvaluation,
            Stage::Geometry => P::Geometry,
            Stage::Fragment => P::Fragment,
        }
    }
}
