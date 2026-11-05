//! Produced by tools/gen_spirv_enums.py from Spv.Generator/spirv.cs: do not edit.
#![allow(non_upper_case_globals, dead_code)]

pub mod execution_model {
    pub const Vertex: u32 = 0;
    pub const TessellationControl: u32 = 1;
    pub const TessellationEvaluation: u32 = 2;
    pub const Geometry: u32 = 3;
    pub const Fragment: u32 = 4;
    pub const GLCompute: u32 = 5;
    pub const Kernel: u32 = 6;
    pub const TaskNV: u32 = 5267;
    pub const MeshNV: u32 = 5268;
    pub const RayGenerationKHR: u32 = 5313;
    pub const RayGenerationNV: u32 = 5313;
    pub const IntersectionKHR: u32 = 5314;
    pub const IntersectionNV: u32 = 5314;
    pub const AnyHitKHR: u32 = 5315;
    pub const AnyHitNV: u32 = 5315;
    pub const ClosestHitKHR: u32 = 5316;
    pub const ClosestHitNV: u32 = 5316;
    pub const MissKHR: u32 = 5317;
    pub const MissNV: u32 = 5317;
    pub const CallableKHR: u32 = 5318;
    pub const CallableNV: u32 = 5318;
}

pub mod addressing_model {
    pub const Logical: u32 = 0;
    pub const Physical32: u32 = 1;
    pub const Physical64: u32 = 2;
    pub const PhysicalStorageBuffer64: u32 = 5348;
    pub const PhysicalStorageBuffer64EXT: u32 = 5348;
}

pub mod memory_model {
    pub const Simple: u32 = 0;
    pub const GLSL450: u32 = 1;
    pub const OpenCL: u32 = 2;
    pub const Vulkan: u32 = 3;
    pub const VulkanKHR: u32 = 3;
}

pub mod execution_mode {
    pub const Invocations: u32 = 0;
    pub const SpacingEqual: u32 = 1;
    pub const SpacingFractionalEven: u32 = 2;
    pub const SpacingFractionalOdd: u32 = 3;
    pub const VertexOrderCw: u32 = 4;
    pub const VertexOrderCcw: u32 = 5;
    pub const PixelCenterInteger: u32 = 6;
    pub const OriginUpperLeft: u32 = 7;
    pub const OriginLowerLeft: u32 = 8;
    pub const EarlyFragmentTests: u32 = 9;
    pub const PointMode: u32 = 10;
    pub const Xfb: u32 = 11;
    pub const DepthReplacing: u32 = 12;
    pub const DepthGreater: u32 = 14;
    pub const DepthLess: u32 = 15;
    pub const DepthUnchanged: u32 = 16;
    pub const LocalSize: u32 = 17;
    pub const LocalSizeHint: u32 = 18;
    pub const InputPoints: u32 = 19;
    pub const InputLines: u32 = 20;
    pub const InputLinesAdjacency: u32 = 21;
    pub const Triangles: u32 = 22;
    pub const InputTrianglesAdjacency: u32 = 23;
    pub const Quads: u32 = 24;
    pub const Isolines: u32 = 25;
    pub const OutputVertices: u32 = 26;
    pub const OutputPoints: u32 = 27;
    pub const OutputLineStrip: u32 = 28;
    pub const OutputTriangleStrip: u32 = 29;
    pub const VecTypeHint: u32 = 30;
    pub const ContractionOff: u32 = 31;
    pub const Initializer: u32 = 33;
    pub const Finalizer: u32 = 34;
    pub const SubgroupSize: u32 = 35;
    pub const SubgroupsPerWorkgroup: u32 = 36;
    pub const SubgroupsPerWorkgroupId: u32 = 37;
    pub const LocalSizeId: u32 = 38;
    pub const LocalSizeHintId: u32 = 39;
    pub const PostDepthCoverage: u32 = 4446;
    pub const DenormPreserve: u32 = 4459;
    pub const DenormFlushToZero: u32 = 4460;
    pub const SignedZeroInfNanPreserve: u32 = 4461;
    pub const RoundingModeRTE: u32 = 4462;
    pub const RoundingModeRTZ: u32 = 4463;
    pub const StencilRefReplacingEXT: u32 = 5027;
    pub const OutputLinesNV: u32 = 5269;
    pub const OutputPrimitivesNV: u32 = 5270;
    pub const DerivativeGroupQuadsNV: u32 = 5289;
    pub const DerivativeGroupLinearNV: u32 = 5290;
    pub const OutputTrianglesNV: u32 = 5298;
    pub const PixelInterlockOrderedEXT: u32 = 5366;
    pub const PixelInterlockUnorderedEXT: u32 = 5367;
    pub const SampleInterlockOrderedEXT: u32 = 5368;
    pub const SampleInterlockUnorderedEXT: u32 = 5369;
    pub const ShadingRateInterlockOrderedEXT: u32 = 5370;
    pub const ShadingRateInterlockUnorderedEXT: u32 = 5371;
    pub const MaxWorkgroupSizeINTEL: u32 = 5893;
    pub const MaxWorkDimINTEL: u32 = 5894;
    pub const NoGlobalOffsetINTEL: u32 = 5895;
    pub const NumSIMDWorkitemsINTEL: u32 = 5896;
}

pub mod storage_class {
    pub const UniformConstant: u32 = 0;
    pub const Input: u32 = 1;
    pub const Uniform: u32 = 2;
    pub const Output: u32 = 3;
    pub const Workgroup: u32 = 4;
    pub const CrossWorkgroup: u32 = 5;
    pub const Private: u32 = 6;
    pub const Function: u32 = 7;
    pub const Generic: u32 = 8;
    pub const PushConstant: u32 = 9;
    pub const AtomicCounter: u32 = 10;
    pub const Image: u32 = 11;
    pub const StorageBuffer: u32 = 12;
    pub const CallableDataKHR: u32 = 5328;
    pub const CallableDataNV: u32 = 5328;
    pub const IncomingCallableDataKHR: u32 = 5329;
    pub const IncomingCallableDataNV: u32 = 5329;
    pub const RayPayloadKHR: u32 = 5338;
    pub const RayPayloadNV: u32 = 5338;
    pub const HitAttributeKHR: u32 = 5339;
    pub const HitAttributeNV: u32 = 5339;
    pub const IncomingRayPayloadKHR: u32 = 5342;
    pub const IncomingRayPayloadNV: u32 = 5342;
    pub const ShaderRecordBufferKHR: u32 = 5343;
    pub const ShaderRecordBufferNV: u32 = 5343;
    pub const PhysicalStorageBuffer: u32 = 5349;
    pub const PhysicalStorageBufferEXT: u32 = 5349;
    pub const CodeSectionINTEL: u32 = 5605;
}

pub mod dim {
    pub const Dim1D: u32 = 0;
    pub const Dim2D: u32 = 1;
    pub const Dim3D: u32 = 2;
    pub const Cube: u32 = 3;
    pub const Rect: u32 = 4;
    pub const Buffer: u32 = 5;
    pub const SubpassData: u32 = 6;
}

pub mod sampler_addressing_mode {
    pub const None: u32 = 0;
    pub const ClampToEdge: u32 = 1;
    pub const Clamp: u32 = 2;
    pub const Repeat: u32 = 3;
    pub const RepeatMirrored: u32 = 4;
}

pub mod sampler_filter_mode {
    pub const Nearest: u32 = 0;
    pub const Linear: u32 = 1;
}

pub mod image_format {
    pub const Unknown: u32 = 0;
    pub const Rgba32f: u32 = 1;
    pub const Rgba16f: u32 = 2;
    pub const R32f: u32 = 3;
    pub const Rgba8: u32 = 4;
    pub const Rgba8Snorm: u32 = 5;
    pub const Rg32f: u32 = 6;
    pub const Rg16f: u32 = 7;
    pub const R11fG11fB10f: u32 = 8;
    pub const R16f: u32 = 9;
    pub const Rgba16: u32 = 10;
    pub const Rgb10A2: u32 = 11;
    pub const Rg16: u32 = 12;
    pub const Rg8: u32 = 13;
    pub const R16: u32 = 14;
    pub const R8: u32 = 15;
    pub const Rgba16Snorm: u32 = 16;
    pub const Rg16Snorm: u32 = 17;
    pub const Rg8Snorm: u32 = 18;
    pub const R16Snorm: u32 = 19;
    pub const R8Snorm: u32 = 20;
    pub const Rgba32i: u32 = 21;
    pub const Rgba16i: u32 = 22;
    pub const Rgba8i: u32 = 23;
    pub const R32i: u32 = 24;
    pub const Rg32i: u32 = 25;
    pub const Rg16i: u32 = 26;
    pub const Rg8i: u32 = 27;
    pub const R16i: u32 = 28;
    pub const R8i: u32 = 29;
    pub const Rgba32ui: u32 = 30;
    pub const Rgba16ui: u32 = 31;
    pub const Rgba8ui: u32 = 32;
    pub const R32ui: u32 = 33;
    pub const Rgb10a2ui: u32 = 34;
    pub const Rg32ui: u32 = 35;
    pub const Rg16ui: u32 = 36;
    pub const Rg8ui: u32 = 37;
    pub const R16ui: u32 = 38;
    pub const R8ui: u32 = 39;
    pub const R64ui: u32 = 40;
    pub const R64i: u32 = 41;
}

pub mod image_operands_mask {
    pub const MaskNone: u32 = 0;
    pub const Bias: u32 = 1;
    pub const Lod: u32 = 2;
    pub const Grad: u32 = 4;
    pub const ConstOffset: u32 = 8;
    pub const Offset: u32 = 16;
    pub const ConstOffsets: u32 = 32;
    pub const Sample: u32 = 64;
    pub const MinLod: u32 = 128;
    pub const MakeTexelAvailable: u32 = 256;
    pub const MakeTexelAvailableKHR: u32 = 256;
    pub const MakeTexelVisible: u32 = 512;
    pub const MakeTexelVisibleKHR: u32 = 512;
    pub const NonPrivateTexel: u32 = 1024;
    pub const NonPrivateTexelKHR: u32 = 1024;
    pub const VolatileTexel: u32 = 2048;
    pub const VolatileTexelKHR: u32 = 2048;
    pub const SignExtend: u32 = 4096;
    pub const ZeroExtend: u32 = 8192;
    pub const Offsets: u32 = 65536;
}

pub mod fprounding_mode {
    pub const RTE: u32 = 0;
    pub const RTZ: u32 = 1;
    pub const RTP: u32 = 2;
    pub const RTN: u32 = 3;
}

pub mod access_qualifier {
    pub const ReadOnly: u32 = 0;
    pub const WriteOnly: u32 = 1;
    pub const ReadWrite: u32 = 2;
}

pub mod decoration {
    pub const RelaxedPrecision: u32 = 0;
    pub const SpecId: u32 = 1;
    pub const Block: u32 = 2;
    pub const BufferBlock: u32 = 3;
    pub const RowMajor: u32 = 4;
    pub const ColMajor: u32 = 5;
    pub const ArrayStride: u32 = 6;
    pub const MatrixStride: u32 = 7;
    pub const GLSLShared: u32 = 8;
    pub const GLSLPacked: u32 = 9;
    pub const CPacked: u32 = 10;
    pub const BuiltIn: u32 = 11;
    pub const NoPerspective: u32 = 13;
    pub const Flat: u32 = 14;
    pub const Patch: u32 = 15;
    pub const Centroid: u32 = 16;
    pub const Sample: u32 = 17;
    pub const Invariant: u32 = 18;
    pub const Restrict: u32 = 19;
    pub const Aliased: u32 = 20;
    pub const Volatile: u32 = 21;
    pub const Constant: u32 = 22;
    pub const Coherent: u32 = 23;
    pub const NonWritable: u32 = 24;
    pub const NonReadable: u32 = 25;
    pub const Uniform: u32 = 26;
    pub const UniformId: u32 = 27;
    pub const SaturatedConversion: u32 = 28;
    pub const Stream: u32 = 29;
    pub const Location: u32 = 30;
    pub const Component: u32 = 31;
    pub const Index: u32 = 32;
    pub const Binding: u32 = 33;
    pub const DescriptorSet: u32 = 34;
    pub const Offset: u32 = 35;
    pub const XfbBuffer: u32 = 36;
    pub const XfbStride: u32 = 37;
    pub const FuncParamAttr: u32 = 38;
    pub const FPRoundingMode: u32 = 39;
    pub const FPFastMathMode: u32 = 40;
    pub const LinkageAttributes: u32 = 41;
    pub const NoContraction: u32 = 42;
    pub const InputAttachmentIndex: u32 = 43;
    pub const Alignment: u32 = 44;
    pub const MaxByteOffset: u32 = 45;
    pub const AlignmentId: u32 = 46;
    pub const MaxByteOffsetId: u32 = 47;
    pub const NoSignedWrap: u32 = 4469;
    pub const NoUnsignedWrap: u32 = 4470;
    pub const ExplicitInterpAMD: u32 = 4999;
    pub const OverrideCoverageNV: u32 = 5248;
    pub const PassthroughNV: u32 = 5250;
    pub const ViewportRelativeNV: u32 = 5252;
    pub const SecondaryViewportRelativeNV: u32 = 5256;
    pub const PerPrimitiveNV: u32 = 5271;
    pub const PerViewNV: u32 = 5272;
    pub const PerTaskNV: u32 = 5273;
    pub const PerVertexNV: u32 = 5285;
    pub const NonUniform: u32 = 5300;
    pub const NonUniformEXT: u32 = 5300;
    pub const RestrictPointer: u32 = 5355;
    pub const RestrictPointerEXT: u32 = 5355;
    pub const AliasedPointer: u32 = 5356;
    pub const AliasedPointerEXT: u32 = 5356;
    pub const ReferencedIndirectlyINTEL: u32 = 5602;
    pub const CounterBuffer: u32 = 5634;
    pub const HlslCounterBufferGOOGLE: u32 = 5634;
    pub const HlslSemanticGOOGLE: u32 = 5635;
    pub const UserSemantic: u32 = 5635;
    pub const UserTypeGOOGLE: u32 = 5636;
    pub const RegisterINTEL: u32 = 5825;
    pub const MemoryINTEL: u32 = 5826;
    pub const NumbanksINTEL: u32 = 5827;
    pub const BankwidthINTEL: u32 = 5828;
    pub const MaxPrivateCopiesINTEL: u32 = 5829;
    pub const SinglepumpINTEL: u32 = 5830;
    pub const DoublepumpINTEL: u32 = 5831;
    pub const MaxReplicatesINTEL: u32 = 5832;
    pub const SimpleDualPortINTEL: u32 = 5833;
    pub const MergeINTEL: u32 = 5834;
    pub const BankBitsINTEL: u32 = 5835;
    pub const ForcePow2DepthINTEL: u32 = 5836;
}

pub mod built_in {
    pub const Position: u32 = 0;
    pub const PointSize: u32 = 1;
    pub const ClipDistance: u32 = 3;
    pub const CullDistance: u32 = 4;
    pub const VertexId: u32 = 5;
    pub const InstanceId: u32 = 6;
    pub const PrimitiveId: u32 = 7;
    pub const InvocationId: u32 = 8;
    pub const Layer: u32 = 9;
    pub const ViewportIndex: u32 = 10;
    pub const TessLevelOuter: u32 = 11;
    pub const TessLevelInner: u32 = 12;
    pub const TessCoord: u32 = 13;
    pub const PatchVertices: u32 = 14;
    pub const FragCoord: u32 = 15;
    pub const PointCoord: u32 = 16;
    pub const FrontFacing: u32 = 17;
    pub const SampleId: u32 = 18;
    pub const SamplePosition: u32 = 19;
    pub const SampleMask: u32 = 20;
    pub const FragDepth: u32 = 22;
    pub const HelperInvocation: u32 = 23;
    pub const NumWorkgroups: u32 = 24;
    pub const WorkgroupSize: u32 = 25;
    pub const WorkgroupId: u32 = 26;
    pub const LocalInvocationId: u32 = 27;
    pub const GlobalInvocationId: u32 = 28;
    pub const LocalInvocationIndex: u32 = 29;
    pub const WorkDim: u32 = 30;
    pub const GlobalSize: u32 = 31;
    pub const EnqueuedWorkgroupSize: u32 = 32;
    pub const GlobalOffset: u32 = 33;
    pub const GlobalLinearId: u32 = 34;
    pub const SubgroupSize: u32 = 36;
    pub const SubgroupMaxSize: u32 = 37;
    pub const NumSubgroups: u32 = 38;
    pub const NumEnqueuedSubgroups: u32 = 39;
    pub const SubgroupId: u32 = 40;
    pub const SubgroupLocalInvocationId: u32 = 41;
    pub const VertexIndex: u32 = 42;
    pub const InstanceIndex: u32 = 43;
    pub const SubgroupEqMask: u32 = 4416;
    pub const SubgroupEqMaskKHR: u32 = 4416;
    pub const SubgroupGeMask: u32 = 4417;
    pub const SubgroupGeMaskKHR: u32 = 4417;
    pub const SubgroupGtMask: u32 = 4418;
    pub const SubgroupGtMaskKHR: u32 = 4418;
    pub const SubgroupLeMask: u32 = 4419;
    pub const SubgroupLeMaskKHR: u32 = 4419;
    pub const SubgroupLtMask: u32 = 4420;
    pub const SubgroupLtMaskKHR: u32 = 4420;
    pub const BaseVertex: u32 = 4424;
    pub const BaseInstance: u32 = 4425;
    pub const DrawIndex: u32 = 4426;
    pub const PrimitiveShadingRateKHR: u32 = 4432;
    pub const DeviceIndex: u32 = 4438;
    pub const ViewIndex: u32 = 4440;
    pub const ShadingRateKHR: u32 = 4444;
    pub const BaryCoordNoPerspAMD: u32 = 4992;
    pub const BaryCoordNoPerspCentroidAMD: u32 = 4993;
    pub const BaryCoordNoPerspSampleAMD: u32 = 4994;
    pub const BaryCoordSmoothAMD: u32 = 4995;
    pub const BaryCoordSmoothCentroidAMD: u32 = 4996;
    pub const BaryCoordSmoothSampleAMD: u32 = 4997;
    pub const BaryCoordPullModelAMD: u32 = 4998;
    pub const FragStencilRefEXT: u32 = 5014;
    pub const ViewportMaskNV: u32 = 5253;
    pub const SecondaryPositionNV: u32 = 5257;
    pub const SecondaryViewportMaskNV: u32 = 5258;
    pub const PositionPerViewNV: u32 = 5261;
    pub const ViewportMaskPerViewNV: u32 = 5262;
    pub const FullyCoveredEXT: u32 = 5264;
    pub const TaskCountNV: u32 = 5274;
    pub const PrimitiveCountNV: u32 = 5275;
    pub const PrimitiveIndicesNV: u32 = 5276;
    pub const ClipDistancePerViewNV: u32 = 5277;
    pub const CullDistancePerViewNV: u32 = 5278;
    pub const LayerPerViewNV: u32 = 5279;
    pub const MeshViewCountNV: u32 = 5280;
    pub const MeshViewIndicesNV: u32 = 5281;
    pub const BaryCoordNV: u32 = 5286;
    pub const BaryCoordNoPerspNV: u32 = 5287;
    pub const FragSizeEXT: u32 = 5292;
    pub const FragmentSizeNV: u32 = 5292;
    pub const FragInvocationCountEXT: u32 = 5293;
    pub const InvocationsPerPixelNV: u32 = 5293;
    pub const LaunchIdKHR: u32 = 5319;
    pub const LaunchIdNV: u32 = 5319;
    pub const LaunchSizeKHR: u32 = 5320;
    pub const LaunchSizeNV: u32 = 5320;
    pub const WorldRayOriginKHR: u32 = 5321;
    pub const WorldRayOriginNV: u32 = 5321;
    pub const WorldRayDirectionKHR: u32 = 5322;
    pub const WorldRayDirectionNV: u32 = 5322;
    pub const ObjectRayOriginKHR: u32 = 5323;
    pub const ObjectRayOriginNV: u32 = 5323;
    pub const ObjectRayDirectionKHR: u32 = 5324;
    pub const ObjectRayDirectionNV: u32 = 5324;
    pub const RayTminKHR: u32 = 5325;
    pub const RayTminNV: u32 = 5325;
    pub const RayTmaxKHR: u32 = 5326;
    pub const RayTmaxNV: u32 = 5326;
    pub const InstanceCustomIndexKHR: u32 = 5327;
    pub const InstanceCustomIndexNV: u32 = 5327;
    pub const ObjectToWorldKHR: u32 = 5330;
    pub const ObjectToWorldNV: u32 = 5330;
    pub const WorldToObjectKHR: u32 = 5331;
    pub const WorldToObjectNV: u32 = 5331;
    pub const HitTNV: u32 = 5332;
    pub const HitKindKHR: u32 = 5333;
    pub const HitKindNV: u32 = 5333;
    pub const IncomingRayFlagsKHR: u32 = 5351;
    pub const IncomingRayFlagsNV: u32 = 5351;
    pub const RayGeometryIndexKHR: u32 = 5352;
    pub const WarpsPerSMNV: u32 = 5374;
    pub const SMCountNV: u32 = 5375;
    pub const WarpIDNV: u32 = 5376;
    pub const SMIDNV: u32 = 5377;
}

pub mod selection_control_mask {
    pub const MaskNone: u32 = 0;
    pub const Flatten: u32 = 1;
    pub const DontFlatten: u32 = 2;
}

pub mod loop_control_mask {
    pub const MaskNone: u32 = 0;
    pub const Unroll: u32 = 1;
    pub const DontUnroll: u32 = 2;
    pub const DependencyInfinite: u32 = 4;
    pub const DependencyLength: u32 = 8;
    pub const MinIterations: u32 = 16;
    pub const MaxIterations: u32 = 32;
    pub const IterationMultiple: u32 = 64;
    pub const PeelCount: u32 = 128;
    pub const PartialCount: u32 = 256;
    pub const InitiationIntervalINTEL: u32 = 65536;
    pub const MaxConcurrencyINTEL: u32 = 131072;
    pub const DependencyArrayINTEL: u32 = 262144;
    pub const PipelineEnableINTEL: u32 = 524288;
    pub const LoopCoalesceINTEL: u32 = 1048576;
    pub const MaxInterleavingINTEL: u32 = 2097152;
    pub const SpeculatedIterationsINTEL: u32 = 4194304;
}

pub mod function_control_mask {
    pub const MaskNone: u32 = 0;
    pub const Inline: u32 = 1;
    pub const DontInline: u32 = 2;
    pub const Pure: u32 = 4;
    pub const Const: u32 = 8;
}

pub mod memory_semantics_mask {
    pub const MaskNone: u32 = 0;
    pub const Acquire: u32 = 2;
    pub const Release: u32 = 4;
    pub const AcquireRelease: u32 = 8;
    pub const SequentiallyConsistent: u32 = 16;
    pub const UniformMemory: u32 = 64;
    pub const SubgroupMemory: u32 = 128;
    pub const WorkgroupMemory: u32 = 256;
    pub const CrossWorkgroupMemory: u32 = 512;
    pub const AtomicCounterMemory: u32 = 1024;
    pub const ImageMemory: u32 = 2048;
    pub const OutputMemory: u32 = 4096;
    pub const OutputMemoryKHR: u32 = 4096;
    pub const MakeAvailable: u32 = 8192;
    pub const MakeAvailableKHR: u32 = 8192;
    pub const MakeVisible: u32 = 16384;
    pub const MakeVisibleKHR: u32 = 16384;
    pub const Volatile: u32 = 32768;
}

pub mod memory_access_mask {
    pub const MaskNone: u32 = 0;
    pub const Volatile: u32 = 1;
    pub const Aligned: u32 = 2;
    pub const Nontemporal: u32 = 4;
    pub const MakePointerAvailable: u32 = 8;
    pub const MakePointerAvailableKHR: u32 = 8;
    pub const MakePointerVisible: u32 = 16;
    pub const MakePointerVisibleKHR: u32 = 16;
    pub const NonPrivatePointer: u32 = 32;
    pub const NonPrivatePointerKHR: u32 = 32;
}

pub mod scope {
    pub const CrossDevice: u32 = 0;
    pub const Device: u32 = 1;
    pub const Workgroup: u32 = 2;
    pub const Subgroup: u32 = 3;
    pub const Invocation: u32 = 4;
    pub const QueueFamily: u32 = 5;
    pub const QueueFamilyKHR: u32 = 5;
    pub const ShaderCallKHR: u32 = 6;
}

pub mod group_operation {
    pub const Reduce: u32 = 0;
    pub const InclusiveScan: u32 = 1;
    pub const ExclusiveScan: u32 = 2;
    pub const ClusteredReduce: u32 = 3;
    pub const PartitionedReduceNV: u32 = 6;
    pub const PartitionedInclusiveScanNV: u32 = 7;
    pub const PartitionedExclusiveScanNV: u32 = 8;
}

