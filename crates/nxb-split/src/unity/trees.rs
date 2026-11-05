//! Type tree layouts of Unity versions whose files carry none (`KNOWN_TREES` of `unityfs.py`, written out).
/// (class ID, type hash, root type, layout)
pub(super) const KNOWN: &[(i32, [u8; 16], &str, &str)] = &[
    (
        48,
        [
            0x44, 0x96, 0xe9, 0x3f, 0x21, 0x79, 0x25, 0x21, 0x04, 0x40, 0x1c, 0x8d, 0xda, 0x0a,
            0x17, 0x51,
        ],
        "Shader",
        r#"SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string>~ m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty>~ m_Props
FastPropertyName: string name
SerializedShaderFloatValue: float val; FastPropertyName name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; FastPropertyName name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>~! m_Channels; unsigned int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter>~ m_MatrixParams; vector<data:VectorParameter>~ m_VectorParams; int m_Size
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16>~ m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; vector<data:VectorParameter>~ m_VectorParams; vector<data:MatrixParameter>~ m_MatrixParams; vector<data:TextureParameter>~ m_TextureParams; vector<data:BufferBinding>~ m_BufferParams; vector<data:ConstantBuffer>~ m_ConstantBuffers; vector<data:BufferBinding>~ m_ConstantBufferBindings; vector<data:UAVParameter>~ m_UAVParams
SerializedProgram: vector<data:SerializedSubProgram>~ m_SubPrograms
SerializedPass: map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; bool! m_HasInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass>~ m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader>~ m_SubShaders; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency>~ m_Dependencies; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int>~ platforms; vector<data:unsigned int>~ offsets; vector<data:unsigned int>~ compressedLengths; vector<data:unsigned int>~ decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:PPtr<Shader>>~ m_Dependencies; bool! m_ShaderIsBaked"#,
    ),
    (
        48,
        [
            0xf0, 0xc1, 0x84, 0x27, 0x2a, 0x05, 0xe5, 0x44, 0x47, 0xe3, 0x63, 0x66, 0xec, 0x48,
            0x76, 0xa6,
        ],
        "Shader",
        r#"
SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
Hash128: UInt8 bytes[0]; UInt8 bytes[1]; UInt8 bytes[2]; UInt8 bytes[3]; UInt8 bytes[4]; UInt8 bytes[5]; UInt8 bytes[6]; UInt8 bytes[7]; UInt8 bytes[8]; UInt8 bytes[9]; UInt8 bytes[10]; UInt8 bytes[11]; UInt8 bytes[12]; UInt8 bytes[13]; UInt8 bytes[14]; UInt8 bytes[15]
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue conservative; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index; int m_ArraySize
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size; bool! m_IsPartialCB
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedProgramParameters: vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; SerializedProgramParameters m_Parameters; SInt64 m_ShaderRequirements
SerializedPlayerSubProgram: unsigned int m_BlobIndex; vector<data:UInt16> m_KeywordIndices; SInt64 m_ShaderRequirements; SInt8! m_GpuProgramType
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms; vector<data:vector<data:SerializedPlayerSubProgram>> m_PlayerSubPrograms; vector<data:vector<data:unsigned int>> m_ParameterBlobIndices; SerializedProgramParameters m_CommonParameters
SerializedPass: vector<data:Hash128> m_EditorDataHash; vector<data:UInt8> m_Platforms; map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags; vector<data:UInt16> m_SerializedKeywordStateMask
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedCustomEditorForRenderPipeline: string customEditorName; string renderPipelineType
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; vector<data:string> m_KeywordNames; vector<data:UInt8> m_KeywordFlags; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; vector<data:SerializedCustomEditorForRenderPipeline> m_CustomEditorForRenderPipelines; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:unsigned int> stageCounts; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked
"#,
    ),
    (
        48,
        [
            0xea, 0x42, 0x4a, 0x5f, 0xc1, 0xe4, 0x8d, 0xb6, 0x21, 0x44, 0xdf, 0x95, 0x6f, 0x9e,
            0x10, 0x05,
        ],
        "Shader",
        r#"
SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
Hash128: UInt8 bytes[0]; UInt8 bytes[1]; UInt8 bytes[2]; UInt8 bytes[3]; UInt8 bytes[4]; UInt8 bytes[5]; UInt8 bytes[6]; UInt8 bytes[7]; UInt8 bytes[8]; UInt8 bytes[9]; UInt8 bytes[10]; UInt8 bytes[11]; UInt8 bytes[12]; UInt8 bytes[13]; UInt8 bytes[14]; UInt8 bytes[15]
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue conservative; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index; int m_ArraySize
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size; bool! m_IsPartialCB
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedProgramParameters: vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; SerializedProgramParameters m_Parameters; SInt64 m_ShaderRequirements
SerializedPlayerSubProgram: unsigned int m_BlobIndex; vector<data:UInt16> m_KeywordIndices; SInt64 m_ShaderRequirements; SInt8! m_GpuProgramType
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms; vector<data:vector<data:SerializedPlayerSubProgram>> m_PlayerSubPrograms; vector<data:vector<data:unsigned int>> m_ParameterBlobIndices; SerializedProgramParameters m_CommonParameters; vector<data:UInt16> m_SerializedKeywordStateMask
SerializedPass: vector<data:Hash128> m_EditorDataHash; vector<data:UInt8> m_Platforms; map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedCustomEditorForRenderPipeline: string customEditorName; string renderPipelineType
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; vector<data:string> m_KeywordNames; vector<data:UInt8> m_KeywordFlags; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; vector<data:SerializedCustomEditorForRenderPipeline> m_CustomEditorForRenderPipelines; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:unsigned int> stageCounts; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked
"#,
    ),
    (
        48,
        [
            0xd8, 0x23, 0x6c, 0x9b, 0xd8, 0x7d, 0x85, 0xed, 0xb2, 0x03, 0xac, 0xf3, 0x52, 0x34,
            0x81, 0x24,
        ],
        "Shader",
        r#"
SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
Hash128: UInt8 bytes[0]; UInt8 bytes[1]; UInt8 bytes[2]; UInt8 bytes[3]; UInt8 bytes[4]; UInt8 bytes[5]; UInt8 bytes[6]; UInt8 bytes[7]; UInt8 bytes[8]; UInt8 bytes[9]; UInt8 bytes[10]; UInt8 bytes[11]; UInt8 bytes[12]; UInt8 bytes[13]; UInt8 bytes[14]; UInt8 bytes[15]
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue conservative; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index; int m_ArraySize
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size; bool! m_IsPartialCB
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedProgramParameters: vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; SerializedProgramParameters m_Parameters; SInt64 m_ShaderRequirements
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms; SerializedProgramParameters m_CommonParameters
SerializedPass: vector<data:Hash128> m_EditorDataHash; vector<data:UInt8> m_Platforms; map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags; vector<data:UInt16> m_SerializedKeywordStateMask
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedCustomEditorForRenderPipeline: string customEditorName; string renderPipelineType
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; vector<data:string> m_KeywordNames; vector<data:UInt8> m_KeywordFlags; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; vector<data:SerializedCustomEditorForRenderPipeline> m_CustomEditorForRenderPipelines; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked
"#,
    ),
    (
        48,
        [
            0x2e, 0xc7, 0x22, 0x3d, 0xce, 0x3a, 0xf4, 0x23, 0x2b, 0x14, 0xae, 0x14, 0x41, 0xbd,
            0x79, 0x99,
        ],
        "Shader",
        r#"
SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
Hash128: UInt8 bytes[0]; UInt8 bytes[1]; UInt8 bytes[2]; UInt8 bytes[3]; UInt8 bytes[4]; UInt8 bytes[5]; UInt8 bytes[6]; UInt8 bytes[7]; UInt8 bytes[8]; UInt8 bytes[9]; UInt8 bytes[10]; UInt8 bytes[11]; UInt8 bytes[12]; UInt8 bytes[13]; UInt8 bytes[14]; UInt8 bytes[15]
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue conservative; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index; int m_ArraySize
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size; bool! m_IsPartialCB
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedProgramParameters: vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; SerializedProgramParameters m_Parameters; SInt64 m_ShaderRequirements
SerializedPlayerSubProgram: unsigned int m_BlobIndex; vector<data:UInt16> m_KeywordIndices; SInt64 m_ShaderRequirements; SInt8! m_GpuProgramType
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms; vector<data:vector<data:SerializedPlayerSubProgram>> m_PlayerSubPrograms; vector<data:vector<data:unsigned int>> m_ParameterBlobIndices; SerializedProgramParameters m_CommonParameters; vector<data:UInt16> m_SerializedKeywordStateMask
SerializedPass: map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedCustomEditorForRenderPipeline: string customEditorName; string renderPipelineType
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; vector<data:string> m_KeywordNames; vector<data:UInt8> m_KeywordFlags; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; vector<data:SerializedCustomEditorForRenderPipeline> m_CustomEditorForRenderPipelines; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:unsigned int> stageCounts; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked; GUID m_AssetGUID
GUID: unsigned int data[0]; unsigned int data[1]; unsigned int data[2]; unsigned int data[3]
"#,
    ),
    (
        48,
        [
            0x57, 0xf0, 0xdd, 0x2d, 0x4e, 0xf2, 0x44, 0x37, 0xb4, 0x65, 0x55, 0x29, 0xa6, 0x73,
            0xa9, 0x36,
        ],
        "Shader",
        r#"
SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
Hash128: UInt8 bytes[0]; UInt8 bytes[1]; UInt8 bytes[2]; UInt8 bytes[3]; UInt8 bytes[4]; UInt8 bytes[5]; UInt8 bytes[6]; UInt8 bytes[7]; UInt8 bytes[8]; UInt8 bytes[9]; UInt8 bytes[10]; UInt8 bytes[11]; UInt8 bytes[12]; UInt8 bytes[13]; UInt8 bytes[14]; UInt8 bytes[15]
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue conservative; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index; int m_ArraySize
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size; bool! m_IsPartialCB
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedProgramParameters: vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; SerializedProgramParameters m_Parameters; SInt64 m_ShaderRequirements
SerializedPlayerSubProgram: unsigned int m_BlobIndex; vector<data:UInt16> m_KeywordIndices; SInt64 m_ShaderRequirements; SInt8! m_GpuProgramType
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms; vector<data:vector<data:SerializedPlayerSubProgram>> m_PlayerSubPrograms; vector<data:vector<data:unsigned int>> m_ParameterBlobIndices; SerializedProgramParameters m_CommonParameters; vector<data:UInt16> m_SerializedKeywordStateMask
SerializedPass: map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedCustomEditorForRenderPipeline: string customEditorName; string renderPipelineType
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; vector<data:string> m_KeywordNames; vector<data:UInt8> m_KeywordFlags; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; vector<data:SerializedCustomEditorForRenderPipeline> m_CustomEditorForRenderPipelines; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:unsigned int> stageCounts; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked; GUID m_AssetGUID; SInt64 m_AssetLocalIdentifierInFile
GUID: unsigned int data[0]; unsigned int data[1]; unsigned int data[2]; unsigned int data[3]
"#,
    ),
    (
        48,
        [
            0xa3, 0x0e, 0x3d, 0xcf, 0x6c, 0x6d, 0x25, 0x15, 0x3d, 0x6f, 0x64, 0xe5, 0xe4, 0x1c,
            0xaf, 0x4e,
        ],
        "Shader",
        r#"
SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
Hash128: UInt8 bytes[0]; UInt8 bytes[1]; UInt8 bytes[2]; UInt8 bytes[3]; UInt8 bytes[4]; UInt8 bytes[5]; UInt8 bytes[6]; UInt8 bytes[7]; UInt8 bytes[8]; UInt8 bytes[9]; UInt8 bytes[10]; UInt8 bytes[11]; UInt8 bytes[12]; UInt8 bytes[13]; UInt8 bytes[14]; UInt8 bytes[15]
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue conservative; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index; int m_ArraySize
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size; bool! m_IsPartialCB
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedProgramParameters: vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_GlobalKeywordIndices; vector<data:UInt16> m_LocalKeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; SerializedProgramParameters m_Parameters; int m_ShaderRequirements
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms; SerializedProgramParameters m_CommonParameters
SerializedPass: vector<data:Hash128> m_EditorDataHash; vector<data:UInt8> m_Platforms; vector<data:UInt16> m_LocalKeywordMask; vector<data:UInt16> m_GlobalKeywordMask; map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked
"#,
    ),
    (
        48,
        [
            0xed, 0x2a, 0x06, 0xce, 0x7f, 0xb8, 0x20, 0x3f, 0x6e, 0xfb, 0x5a, 0xb5, 0xe0, 0xce,
            0x20, 0xd5,
        ],
        "Shader",
        r#"SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_GlobalKeywordIndices; vector<data:UInt16> m_LocalKeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers; int m_ShaderRequirements
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms
SerializedPass: map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; SerializedProgram progRayTracing; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:vector<data:unsigned int>> offsets; vector<data:vector<data:unsigned int>> compressedLengths; vector<data:vector<data:unsigned int>> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked
"#,
    ),
    (
        48,
        [
            0x8d, 0xb7, 0x60, 0x07, 0x3e, 0x13, 0x92, 0x06, 0x1e, 0x4a, 0x36, 0x99, 0x5f, 0x07,
            0x35, 0xaf,
        ],
        "Shader",
        r#"SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers; int m_ShaderRequirements
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms
SerializedPass: map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; bool m_HasInstancingVariant; bool! m_HasProceduralInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:unsigned int> offsets; vector<data:unsigned int> compressedLengths; vector<data:unsigned int> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:PPtr<Shader>> m_Dependencies; map<data:pair(string,PPtr<Texture>)>~ m_NonModifiableTextures; bool! m_ShaderIsBaked
"#,
    ),
    (
        48,
        [
            0x26, 0x6d, 0x53, 0x11, 0x3f, 0xa3, 0x0d, 0x2b, 0x85, 0x8f, 0x27, 0x68, 0xf9, 0x2e,
            0xaa, 0x14,
        ],
        "Shader",
        r#"SerializedTextureProperty: string m_DefaultName; int m_TexDim
SerializedProperty: string m_Name; string m_Description; vector<data:string> m_Attributes; int m_Type; unsigned int m_Flags; float m_DefValue[0]; float m_DefValue[1]; float m_DefValue[2]; float m_DefValue[3]; SerializedTextureProperty m_DefTexture
SerializedProperties: vector<data:SerializedProperty> m_Props
SerializedShaderFloatValue: float val; string name
SerializedShaderRTBlendState: SerializedShaderFloatValue srcBlend; SerializedShaderFloatValue destBlend; SerializedShaderFloatValue srcBlendAlpha; SerializedShaderFloatValue destBlendAlpha; SerializedShaderFloatValue blendOp; SerializedShaderFloatValue blendOpAlpha; SerializedShaderFloatValue colMask
SerializedStencilOp: SerializedShaderFloatValue pass; SerializedShaderFloatValue fail; SerializedShaderFloatValue zFail; SerializedShaderFloatValue comp
SerializedShaderVectorValue: SerializedShaderFloatValue x; SerializedShaderFloatValue y; SerializedShaderFloatValue z; SerializedShaderFloatValue w; string name
SerializedTagMap: map<data:pair(string,string)>~ tags
SerializedShaderState: string m_Name; SerializedShaderRTBlendState rtBlend0; SerializedShaderRTBlendState rtBlend1; SerializedShaderRTBlendState rtBlend2; SerializedShaderRTBlendState rtBlend3; SerializedShaderRTBlendState rtBlend4; SerializedShaderRTBlendState rtBlend5; SerializedShaderRTBlendState rtBlend6; SerializedShaderRTBlendState rtBlend7; bool! rtSeparateBlend; SerializedShaderFloatValue zClip; SerializedShaderFloatValue zTest; SerializedShaderFloatValue zWrite; SerializedShaderFloatValue culling; SerializedShaderFloatValue offsetFactor; SerializedShaderFloatValue offsetUnits; SerializedShaderFloatValue alphaToMask; SerializedStencilOp stencilOp; SerializedStencilOp stencilOpFront; SerializedStencilOp stencilOpBack; SerializedShaderFloatValue stencilReadMask; SerializedShaderFloatValue stencilWriteMask; SerializedShaderFloatValue stencilRef; SerializedShaderFloatValue fogStart; SerializedShaderFloatValue fogEnd; SerializedShaderFloatValue fogDensity; SerializedShaderVectorValue fogColor; int fogMode; int gpuProgramID; SerializedTagMap m_Tags; int m_LOD; bool! lighting
ShaderBindChannel: SInt8 source; SInt8 target
ParserBindChannels: vector<data:ShaderBindChannel>! m_Channels; int m_SourceMap
VectorParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_Dim
MatrixParameter: int m_NameIndex; int m_Index; int m_ArraySize; SInt8 m_Type; SInt8! m_RowCount
TextureParameter: int m_NameIndex; int m_Index; int m_SamplerIndex; bool m_MultiSampled; SInt8! m_Dim
BufferBinding: int m_NameIndex; int m_Index
StructParameter: int m_NameIndex; int m_Index; int m_ArraySize; int m_StructSize; vector<data:VectorParameter> m_VectorMembers; vector<data:MatrixParameter> m_MatrixMembers
ConstantBuffer: int m_NameIndex; vector<data:MatrixParameter> m_MatrixParams; vector<data:VectorParameter> m_VectorParams; vector<data:StructParameter> m_StructParams; int m_Size
UAVParameter: int m_NameIndex; int m_Index; int m_OriginalIndex
SamplerParameter: unsigned int sampler; int bindPoint
SerializedSubProgram: unsigned int m_BlobIndex; ParserBindChannels m_Channels; vector<data:UInt16> m_KeywordIndices; SInt8 m_ShaderHardwareTier; SInt8! m_GpuProgramType; vector<data:VectorParameter> m_VectorParams; vector<data:MatrixParameter> m_MatrixParams; vector<data:TextureParameter> m_TextureParams; vector<data:BufferBinding> m_BufferParams; vector<data:ConstantBuffer> m_ConstantBuffers; vector<data:BufferBinding> m_ConstantBufferBindings; vector<data:UAVParameter> m_UAVParams; vector<data:SamplerParameter> m_Samplers; int m_ShaderRequirements
SerializedProgram: vector<data:SerializedSubProgram> m_SubPrograms
SerializedPass: map<data:pair(string,int)>~ m_NameIndices; int m_Type; SerializedShaderState m_State; unsigned int m_ProgramMask; SerializedProgram progVertex; SerializedProgram progFragment; SerializedProgram progGeometry; SerializedProgram progHull; SerializedProgram progDomain; bool! m_HasInstancingVariant; string m_UseName; string m_Name; string m_TextureName; SerializedTagMap m_Tags
SerializedSubShader: vector<data:SerializedPass> m_Passes; SerializedTagMap m_Tags; int m_LOD
SerializedShaderDependency: string from; string to
SerializedShader: SerializedProperties m_PropInfo; vector<data:SerializedSubShader> m_SubShaders; string m_Name; string m_CustomEditorName; string m_FallbackName; vector<data:SerializedShaderDependency> m_Dependencies; bool! m_DisableNoSubshadersMessage
PPtr<Shader>: int m_FileID; SInt64 m_PathID
PPtr<Texture>: int m_FileID; SInt64 m_PathID
Shader: string m_Name; SerializedShader m_ParsedForm; vector<data:unsigned int> platforms; vector<data:unsigned int> offsets; vector<data:unsigned int> compressedLengths; vector<data:unsigned int> decompressedLengths; vector<data:UInt8> compressedBlob; vector<data:PPtr<Shader>> m_Dependencies; bool! m_ShaderIsBaked
"#,
    ),
    (
        72,
        [
            0x43, 0xa0, 0x94, 0x8a, 0x3f, 0x40, 0x30, 0x68, 0xc0, 0xb3, 0x1b, 0xff, 0x90, 0xba,
            0x0c, 0x9a,
        ],
        "ComputeShader",
        r#"ComputeShaderResource: string name; string generatedName; int bindPoint; int samplerBindPoint; int texDimension; int resType
ComputeShaderBuiltinSampler: unsigned int sampler; int bindPoint
ComputeShaderKernel: vector<data:unsigned int> cbVariantIndices; vector<data:ComputeShaderResource> cbs; vector<data:ComputeShaderResource> textures; vector<data:ComputeShaderBuiltinSampler> builtinSamplers; vector<data:ComputeShaderResource> inBuffers; vector<data:ComputeShaderResource> outBuffers; vector<data:UInt8> code; staticvector<data:unsigned int>~ threadGroupSize; SInt64 requirements
ComputeShaderKernelParent: string name; vector<data:ComputeShaderKernel> uniqueVariants; vector<data:pair(string,unsigned int)> variantIndices; vector<data:string> globalKeywords; vector<data:string> localKeywords; vector<data:string> dynamicKeywords
ComputeShaderParam: string name; int type; unsigned int offset; unsigned int arraySize; unsigned int rowCount; unsigned int colCount
ComputeShaderCB: string name; int byteSize; vector<data:ComputeShaderParam> params
ComputeShaderPlatformVariant: int targetRenderer; int targetLevel; vector<data:ComputeShaderKernelParent> kernels; vector<data:ComputeShaderCB> constantBuffers; bool! resourcesResolved
ComputeShader: string m_Name; vector<data:ComputeShaderPlatformVariant> variants
"#,
    ),
    (
        72,
        [
            0xbf, 0xdf, 0x63, 0xab, 0x0c, 0xe8, 0x4d, 0x5a, 0x2c, 0x76, 0x5e, 0xe5, 0x60, 0xd9,
            0x0f, 0xe7,
        ],
        "ComputeShader",
        r#"ComputeShaderResource: string name; string generatedName; int bindPoint; int samplerBindPoint; int texDimension
ComputeShaderBuiltinSampler: unsigned int sampler; int bindPoint
ComputeShaderKernel: vector<data:unsigned int> cbVariantIndices; vector<data:ComputeShaderResource> cbs; vector<data:ComputeShaderResource> textures; vector<data:ComputeShaderBuiltinSampler> builtinSamplers; vector<data:ComputeShaderResource> inBuffers; vector<data:ComputeShaderResource> outBuffers; vector<data:UInt8> code; vector<data:unsigned int>~ threadGroupSize; SInt64 requirements
ComputeShaderKernelParent: string name; vector<data:ComputeShaderKernel> uniqueVariants; vector<data:pair(string,unsigned int)> variantIndices; vector<data:string> globalKeywords; vector<data:string> localKeywords; vector<data:string> dynamicKeywords
ComputeShaderParam: string name; int type; unsigned int offset; unsigned int arraySize; unsigned int rowCount; unsigned int colCount
ComputeShaderCB: string name; int byteSize; vector<data:ComputeShaderParam> params
ComputeShaderPlatformVariant: int targetRenderer; int targetLevel; vector<data:ComputeShaderKernelParent> kernels; vector<data:ComputeShaderCB> constantBuffers; bool! resourcesResolved
ComputeShader: string m_Name; vector<data:ComputeShaderPlatformVariant> variants
"#,
    ),
    (
        72,
        [
            0xab, 0xd9, 0x13, 0x5b, 0x8c, 0xe8, 0x3d, 0x04, 0x3f, 0xef, 0x4e, 0x9e, 0xc7, 0xf5,
            0x33, 0x66,
        ],
        "ComputeShader",
        r#"ComputeShaderResource: string name; string generatedName; int bindPoint; int samplerBindPoint; int texDimension
ComputeShaderBuiltinSampler: unsigned int sampler; int bindPoint
ComputeShaderKernel: vector<data:unsigned int> cbVariantIndices; vector<data:ComputeShaderResource> cbs; vector<data:ComputeShaderResource> textures; vector<data:ComputeShaderBuiltinSampler> builtinSamplers; vector<data:ComputeShaderResource> inBuffers; vector<data:ComputeShaderResource> outBuffers; vector<data:UInt8> code; staticvector<data:unsigned int>~ threadGroupSize; SInt64 requirements
ComputeShaderKernelParent: string name; map<data:pair(string,ComputeShaderKernel)>~ variantMap; vector<data:string> globalKeywords; vector<data:string> localKeywords
ComputeShaderParam: string name; int type; unsigned int offset; unsigned int arraySize; unsigned int rowCount; unsigned int colCount
ComputeShaderCB: string name; int byteSize; vector<data:ComputeShaderParam> params
ComputeShaderPlatformVariant: int targetRenderer; int targetLevel; vector<data:ComputeShaderKernelParent> kernels; vector<data:ComputeShaderCB> constantBuffers; bool! resourcesResolved
ComputeShader: string m_Name; vector<data:ComputeShaderPlatformVariant> variants
"#,
    ),
    (
        72,
        [
            0xd0, 0x17, 0x1b, 0x84, 0x9b, 0xb0, 0x64, 0xb2, 0x42, 0x48, 0x93, 0xe7, 0xd4, 0x80,
            0x9b, 0xe5,
        ],
        "ComputeShader",
        r#"ComputeShaderResource: string name; string generatedName; int bindPoint; int samplerBindPoint; int texDimension
ComputeShaderBuiltinSampler: unsigned int sampler; int bindPoint
ComputeShaderKernel: vector<data:unsigned int> cbVariantIndices; vector<data:ComputeShaderResource> cbs; vector<data:ComputeShaderResource> textures; vector<data:ComputeShaderBuiltinSampler> builtinSamplers; vector<data:ComputeShaderResource> inBuffers; vector<data:ComputeShaderResource> outBuffers; vector<data:UInt8> code; staticvector<data:unsigned int>~ threadGroupSize
ComputeShaderKernelParent: string name; map<data:pair(string,ComputeShaderKernel)>~ variantMap; vector<data:string> globalKeywords; vector<data:string> localKeywords
ComputeShaderParam: string name; int type; unsigned int offset; unsigned int arraySize; unsigned int rowCount; unsigned int colCount
ComputeShaderCB: string name; int byteSize; vector<data:ComputeShaderParam> params
ComputeShaderPlatformVariant: int targetRenderer; int targetLevel; vector<data:ComputeShaderKernelParent> kernels; vector<data:ComputeShaderCB> constantBuffers; bool! resourcesResolved
ComputeShader: string m_Name; vector<data:ComputeShaderPlatformVariant> variants
"#,
    ),
    (
        72,
        [
            0x1a, 0x95, 0x52, 0x9a, 0xc2, 0xeb, 0x1a, 0x97, 0xcd, 0x9d, 0x2a, 0x43, 0x56, 0xbd,
            0x72, 0x2f,
        ],
        "ComputeShader",
        r#"ComputeShaderResource: string name; string generatedName; int bindPoint; int samplerBindPoint; int texDimension
ComputeShaderBuiltinSampler: unsigned int sampler; int bindPoint
ComputeShaderKernel: string name; vector<data:ComputeShaderResource> cbs; vector<data:ComputeShaderResource> textures; vector<data:ComputeShaderBuiltinSampler> builtinSamplers; vector<data:ComputeShaderResource> inBuffers; vector<data:ComputeShaderResource> outBuffers; vector<data:UInt8> code; staticvector<data:unsigned int>~ threadGroupSize
ComputeShaderParam: string name; int type; int offset; int arraySize; int rowCount; int colCount
ComputeShaderCB: string name; int byteSize; vector<data:ComputeShaderParam> params
ComputeShaderVariant: int targetRenderer; int targetLevel; vector<data:ComputeShaderKernel> kernels; vector<data:ComputeShaderCB> constantBuffers; bool! resourcesResolved
ComputeShader: string m_Name; vector<data:ComputeShaderVariant> variants
"#,
    ),
    (
        72,
        [
            0x20, 0x29, 0x1e, 0x02, 0x0d, 0x10, 0x07, 0x4c, 0x35, 0xd8, 0x28, 0x6f, 0x51, 0x91,
            0x59, 0x3f,
        ],
        "ComputeShader",
        r#"ComputeShaderResource: string name; string generatedName; int bindPoint; ComputeBufferCounter counter
ComputeShaderBuiltinSampler: unsigned int sampler; int bindPoint
ComputeShaderKernel: string name; vector<data:ComputeShaderResource> cbs; vector<data:ComputeShaderResource> textures; vector<data:ComputeShaderBuiltinSampler> builtinSamplers; vector<data:ComputeShaderResource> inBuffers; vector<data:ComputeShaderResource> outBuffers; vector<data:UInt8> code; staticvector<data:unsigned int>~ threadGroupSize
ComputeShaderParam: string name; int type; int offset; int arraySize; int rowCount; int colCount
ComputeShaderCB: string name; int byteSize; vector<data:ComputeShaderParam> params
ComputeShaderVariant: int targetRenderer; int targetLevel; vector<data:ComputeShaderKernel> kernels; vector<data:ComputeShaderCB> constantBuffers; bool! resourcesResolved
ComputeShader: string m_Name; vector<data:ComputeShaderVariant> variants
ComputeBufferCounter: int bindpoint; int offset
"#,
    ),
];
