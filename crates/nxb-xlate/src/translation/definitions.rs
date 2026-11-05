//! `ShaderDefinitions` for the job's graphics state (transform feedback is never enabled by the job accessor).

use super::header::{ImapPixelType, ShaderHeader};
use crate::accessor::GraphicsState;
use crate::attr::Stage;
use crate::types::AttributeType;

/// `InputTopology` numbers (`Points`, `Lines`, `LinesAdjacency`, `Triangles`, `TrianglesAdjacency`).
pub mod topo {
    pub const POINTS: i32 = 0;
    pub const LINES: i32 = 1;
    pub const LINES_ADJACENCY: i32 = 2;
    pub const TRIANGLES: i32 = 3;
    pub const TRIANGLES_ADJACENCY: i32 = 4;
}

/// `ShaderDefinitions`.
#[derive(Clone, Debug)]
pub struct Definitions {
    pub stage: Stage,
    pub compute_local_size: [i32; 3],
    pub gs: GraphicsState,
    pub gp_passthrough: bool,
    pub last_in_vertex_pipeline: bool,
    pub threads_per_input_primitive: i32,
    pub output_topology: i32,
    pub max_output_vertices: i32,
    pub imap_types: Vec<ImapPixelType>,
    pub ia_indexing: bool,
    pub oa_indexing: bool,
    pub omap_targets: i32,
    pub omap_sample_mask: bool,
    pub omap_depth: bool,
    pub supports_scaled_vertex_formats: bool,
}

impl Definitions {
    /// `new ShaderDefinitions(ShaderStage.Compute, x, y, z)`.
    pub fn compute(size: [i32; 3], gs: GraphicsState) -> Definitions {
        Definitions {
            stage: Stage::Compute,
            compute_local_size: size,
            gs,
            gp_passthrough: false,
            last_in_vertex_pipeline: false,
            threads_per_input_primitive: 0,
            output_topology: 0,
            max_output_vertices: 0,
            imap_types: Vec::new(),
            ia_indexing: false,
            oa_indexing: false,
            omap_targets: 0,
            omap_sample_mask: false,
            omap_depth: false,
            supports_scaled_vertex_formats: false,
        }
    }

    /// `CreateGraphicsDefinitions` (no transform feedback).
    pub fn graphics(h: &ShaderHeader, gs: GraphicsState) -> Definitions {
        let gp = h.stage == Stage::Geometry && h.gp_passthrough;
        // OutputTopology: PointList 1, LineStrip 6, TriangleStrip 7 (numbers of the C# enum)
        let (out_topo, max_vertices) = if gp {
            let t = match gs.topology {
                topo::POINTS => 1,
                topo::LINES | topo::LINES_ADJACENCY => 6,
                _ => 7,
            };
            let no_adj = match gs.topology {
                topo::POINTS => 1,
                topo::LINES | topo::LINES_ADJACENCY => 2,
                _ => 3,
            };
            (t, no_adj)
        } else {
            (h.output_topology, h.max_output_vertex_count)
        };
        Definitions {
            stage: h.stage,
            compute_local_size: [0; 3],
            gs,
            gp_passthrough: gp,
            last_in_vertex_pipeline: matches!(
                h.stage,
                Stage::Compute
                    | Stage::Vertex
                    | Stage::TessellationControl
                    | Stage::TessellationEvaluation
                    | Stage::Geometry
            ),
            threads_per_input_primitive: h.threads_per_input_primitive,
            output_topology: out_topo,
            max_output_vertices: max_vertices,
            imap_types: h.imap_types.clone(),
            ia_indexing: false,
            oa_indexing: false,
            omap_targets: h.omap_targets,
            omap_sample_mask: h.omap_sample_mask,
            omap_depth: h.omap_depth,
            supports_scaled_vertex_formats: true,
        }
    }

    pub fn attribute_type(&self, location: usize) -> AttributeType {
        self.gs.attr[location]
    }

    /// `HasPerLocationInputOrOutput`.
    pub fn has_per_location_input_or_output(
        &self,
        io_user_defined: bool,
        frag_color: bool,
        is_output: bool,
    ) -> bool {
        if io_user_defined {
            return (!is_output && !self.ia_indexing) || (is_output && !self.oa_indexing);
        }
        frag_color
    }
}

impl Definitions {
    /// `ViewportTransformDisable` of the job's graphics state (always false).
    pub fn gs_viewport_transform_disable(&self) -> bool {
        false
    }

    /// `DepthMode` of the job's graphics state (always false).
    pub fn gs_depth_mode(&self) -> bool {
        false
    }
}
