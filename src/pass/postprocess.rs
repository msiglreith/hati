use engine::Engine;
use pass;
use winapi::shared::dxgiformat::*;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

pub struct DisplayMap {
    pub signature: ComPtr<ID3D12RootSignature>,
    pub pipeline: ComPtr<ID3D12PipelineState>,
}

pub struct PostProcess {
    pub display_map: DisplayMap,
}

impl PostProcess {
    pub fn new(engine: &Engine) -> Self {
        // Display mapping
        let display_map = {
            let vs_shader = engine
                .load_shader(
                    "display_map_vs",
                    "shaders/displaymap.hlsl",
                    "vs_main\0",
                    "vs_5_1\0",
                )
                .unwrap();
            let ps_shader = engine
                .load_shader(
                    "display_map_ps",
                    "shaders/displaymap.hlsl",
                    "ps_displaymap\0",
                    "ps_5_1\0",
                )
                .unwrap();

            let table_input = [
                D3D12_DESCRIPTOR_RANGE {
                    RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                    NumDescriptors: 1,
                    BaseShaderRegister: 0,
                    RegisterSpace: 0,
                    OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
                },
            ];
            let parameters = [
                pass::gen_root_table_param(
                    D3D12_SHADER_VISIBILITY_PIXEL,
                    D3D12_ROOT_DESCRIPTOR_TABLE {
                        NumDescriptorRanges: table_input.len() as _,
                        pDescriptorRanges: table_input.as_ptr(),
                    },
                ),
            ];

            let static_samplers = [
                // Clamp to border sampler for basic material texture sampling (albedo, roughness, ..).
                D3D12_STATIC_SAMPLER_DESC {
                    Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
                    AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
                    AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
                    AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
                    MipLODBias: 0.0,
                    MaxAnisotropy: 0,
                    ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                    BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
                    MinLOD: 0.0,
                    MaxLOD: D3D12_FLOAT32_MAX,
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                    ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
                },
            ];

            let signature = engine
                .create_root_signature(&D3D12_ROOT_SIGNATURE_DESC {
                    NumParameters: parameters.len() as _,
                    pParameters: parameters.as_ptr(),
                    NumStaticSamplers: static_samplers.len() as _,
                    pStaticSamplers: static_samplers.as_ptr(),
                    Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE,
                })
                .unwrap();

            let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: signature.as_raw(),
                VS: pass::unpack_shader_bc(&vs_shader),
                PS: pass::unpack_shader_bc(&ps_shader),
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                ..pass::DEFAULT_PIPELINE_STATE_DESC
            };
            pso_desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM_SRGB;

            let pipeline = engine.create_graphics_pipeline(&pso_desc);

            DisplayMap {
                pipeline,
                signature,
            }
        };

        PostProcess { display_map }
    }
}
