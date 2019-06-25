use std::{mem, ptr};
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::um::d3d12::*;
use winapi::um::d3dcommon::ID3DBlob;
use wio::com::ComPtr;

pub mod geometry;
pub mod lighting;
pub mod pipeline;
pub mod postprocess;

pub const DS_FORMAT: DXGI_FORMAT = DXGI_FORMAT_D32_FLOAT;

const NULL_SHADER: D3D12_SHADER_BYTECODE = D3D12_SHADER_BYTECODE {
    pShaderBytecode: ptr::null_mut(),
    BytecodeLength: 0,
};

const DEFAULT_PIPELINE_STATE_DESC: D3D12_GRAPHICS_PIPELINE_STATE_DESC =
    D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: ptr::null_mut(),
        VS: NULL_SHADER,
        PS: NULL_SHADER,
        DS: NULL_SHADER,
        HS: NULL_SHADER,
        GS: NULL_SHADER,
        StreamOutput: D3D12_STREAM_OUTPUT_DESC {
            pSODeclaration: ptr::null(),
            NumEntries: 0,
            pBufferStrides: ptr::null(),
            NumStrides: 0,
            RasterizedStream: 0,
        },
        BlendState: D3D12_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: FALSE,
                LogicOpEnable: FALSE,
                SrcBlend: D3D12_BLEND_ONE,
                DestBlend: D3D12_BLEND_ZERO,
                BlendOp: D3D12_BLEND_OP_ADD,
                SrcBlendAlpha: D3D12_BLEND_ONE,
                DestBlendAlpha: D3D12_BLEND_ZERO,
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                LogicOp: D3D12_LOGIC_OP_NOOP,
                RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
            }; 8],
        },
        SampleMask: !0,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_NONE,
            FrontCounterClockwise: TRUE,
            DepthBias: 0,
            DepthBiasClamp: 0.0,
            SlopeScaledDepthBias: 0.0,
            DepthClipEnable: TRUE,
            MultisampleEnable: FALSE,
            AntialiasedLineEnable: FALSE,
            ForcedSampleCount: 0,
            ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
            DepthEnable: FALSE,
            DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL,
            DepthFunc: D3D12_COMPARISON_FUNC_LESS,
            StencilEnable: FALSE,
            StencilReadMask: D3D12_DEFAULT_STENCIL_READ_MASK as _,
            StencilWriteMask: D3D12_DEFAULT_STENCIL_WRITE_MASK as _,
            FrontFace: D3D12_DEPTH_STENCILOP_DESC {
                StencilFailOp: D3D12_STENCIL_OP_KEEP,
                StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
                StencilPassOp: D3D12_STENCIL_OP_KEEP,
                StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            },
            BackFace: D3D12_DEPTH_STENCILOP_DESC {
                StencilFailOp: D3D12_STENCIL_OP_KEEP,
                StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
                StencilPassOp: D3D12_STENCIL_OP_KEEP,
                StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            },
        },
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: ptr::null(),
            NumElements: 0,
        },
        IBStripCutValue: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_UNDEFINED,
        NumRenderTargets: 0,
        RTVFormats: [DXGI_FORMAT_UNKNOWN; 8],
        DSVFormat: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        NodeMask: 0,
        CachedPSO: D3D12_CACHED_PIPELINE_STATE {
            pCachedBlob: ptr::null(),
            CachedBlobSizeInBytes: 0,
        },
        Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
    };

pub fn unpack_shader_bc(shader: &ComPtr<ID3DBlob>) -> D3D12_SHADER_BYTECODE {
    unsafe {
        D3D12_SHADER_BYTECODE {
            pShaderBytecode: shader.GetBufferPointer() as *const _,
            BytecodeLength: shader.GetBufferSize(),
        }
    }
}

pub fn gen_root_descriptor_param(
    ty: D3D12_ROOT_PARAMETER_TYPE,
    visibility: D3D12_SHADER_VISIBILITY,
    descriptor: D3D12_ROOT_DESCRIPTOR,
) -> D3D12_ROOT_PARAMETER {
    let mut view_parameter = D3D12_ROOT_PARAMETER {
        ParameterType: ty,
        ShaderVisibility: visibility,
        ..unsafe { mem::zeroed() }
    };
    unsafe {
        *view_parameter.u.Descriptor_mut() = descriptor;
    }
    view_parameter
}

pub fn gen_root_table_param(
    visibility: D3D12_SHADER_VISIBILITY,
    table: D3D12_ROOT_DESCRIPTOR_TABLE,
) -> D3D12_ROOT_PARAMETER {
    let mut view_parameter = D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        ShaderVisibility: visibility,
        ..unsafe { mem::zeroed() }
    };
    unsafe {
        *view_parameter.u.DescriptorTable_mut() = table;
    }
    view_parameter
}

pub fn gen_root_constants_param(
    visibility: D3D12_SHADER_VISIBILITY,
    constants: D3D12_ROOT_CONSTANTS,
) -> D3D12_ROOT_PARAMETER {
    let mut view_parameter = D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        ShaderVisibility: visibility,
        ..unsafe { mem::zeroed() }
    };
    unsafe {
        *view_parameter.u.Constants_mut() = constants;
    }
    view_parameter
}
