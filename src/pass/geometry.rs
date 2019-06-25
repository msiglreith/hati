//! Geometry buffer creation pass

use engine::Engine;
use pass;

use std::ptr;
use winapi::shared::dxgiformat::*;
use winapi::shared::minwindef::TRUE;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

pub struct Geometry {
    pub signature: ComPtr<ID3D12RootSignature>,
    pub pipeline: ComPtr<ID3D12PipelineState>,
}

impl Geometry {
    pub fn new(engine: &Engine) -> Self {
        let vs_shader = engine
            .load_shader(
                "geometry_vs",
                "shaders/geometry.hlsl",
                "vs_main\0",
                "vs_5_1\0",
            )
            .unwrap();
        let ps_shader = engine
            .load_shader(
                "geometry_ps",
                "shaders/geometry.hlsl",
                "ps_main\0",
                "ps_5_1\0",
            )
            .unwrap();

        // Vertex and Index buffer SRVs
        let table_data = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 2,
                BaseShaderRegister: 0,
                RegisterSpace: 1,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            },
        ];

        let parameters = [
            // View data
            pass::gen_root_descriptor_param(
                D3D12_ROOT_PARAMETER_TYPE_CBV,
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_DESCRIPTOR {
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                },
            ),
            // Vertex and Index SRVs
            pass::gen_root_table_param(
                D3D12_SHADER_VISIBILITY_PIXEL,
                D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table_data.len() as _,
                    pDescriptorRanges: table_data.as_ptr(),
                },
            ),
            // Base instance root constants
            pass::gen_root_constants_param(
                D3D12_SHADER_VISIBILITY_PIXEL,
                D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 2,
                    Num32BitValues: 2,
                },
            ),
            // Draw ID
            pass::gen_root_constants_param(
                D3D12_SHADER_VISIBILITY_PIXEL,
                D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 1,
                    RegisterSpace: 2,
                    Num32BitValues: 1,
                },
            ),
        ];

        let signature = engine
            .create_root_signature(&D3D12_ROOT_SIGNATURE_DESC {
                NumParameters: parameters.len() as _,
                pParameters: parameters.as_ptr(),
                NumStaticSamplers: 0,
                pStaticSamplers: ptr::null(),
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            })
            .unwrap();

        let input_layout = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: b"Attr\0" as *const _ as *const _,
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: D3D12_APPEND_ALIGNED_ELEMENT,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];

        let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: signature.as_raw(),
            VS: pass::unpack_shader_bc(&vs_shader),
            PS: pass::unpack_shader_bc(&ps_shader),
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_layout.as_ptr(),
                NumElements: input_layout.len() as _,
            },
            DSVFormat: pass::DS_FORMAT,
            ..pass::DEFAULT_PIPELINE_STATE_DESC
        };
        pso_desc.RTVFormats[0] = DXGI_FORMAT_R16G16B16A16_UINT;
        pso_desc.DepthStencilState.DepthEnable = TRUE;
        pso_desc.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_LESS;

        let pipeline = engine.create_graphics_pipeline(&pso_desc);

        Geometry {
            signature,
            pipeline,
        }
    }
}
