use engine::Engine;
use pass;
use std::mem;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

// Size of a compute tile.
//
// Must match with the number of threads specified in the shader.
pub const TILE_THREADS_X: u32 = 16;
pub const TILE_THREADS_Y: u32 = 16;

// #[repr(hlsl)]
#[repr(C)]
pub struct LightData {
    pub num_point_lights: u32,
}

// #[repr(hlsl)]
#[repr(C)]
pub struct PointLight {
    pub position: [f32; 3],
    pub intensity: f32,
}

pub struct Lighting {
    pub signature: ComPtr<ID3D12RootSignature>,
    pub pipeline: ComPtr<ID3D12PipelineState>,
}

impl Lighting {
    pub fn new(engine: &mut Engine) -> Self {
        let cs_shader = engine
            .load_shader(
                "lighting_cs",
                "shaders/lighting.hlsl",
                "cs_lighting\0",
                "cs_5_1\0",
            )
            .unwrap();

        // Lighting buffer UAV
        let table_data_uav = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
                NumDescriptors: 1,
                BaseShaderRegister: 0,
                RegisterSpace: 3,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            },
        ];

        let table_data_geometry = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 1,
                BaseShaderRegister: 1,
                RegisterSpace: 3,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            },
        ];

        let table_data_textures = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: !0,
                BaseShaderRegister: 0,
                RegisterSpace: 2,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            },
        ];

        let table_data_light = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: 1,
                BaseShaderRegister: 0,
                RegisterSpace: 4,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            },
        ];

        // * Index buffer
        // * Vertex position
        // * Base Index and Vertex
        let num_draw_buffers = 3;
        let table_data_draw = [
            D3D12_DESCRIPTOR_RANGE {
                RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                NumDescriptors: num_draw_buffers,
                BaseShaderRegister: 0,
                RegisterSpace: 1,
                OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
            },
        ];

        let parameters = [
            // Lighting buffer UAV
            pass::gen_root_table_param(
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table_data_uav.len() as _,
                    pDescriptorRanges: table_data_uav.as_ptr(),
                },
            ),
            pass::gen_root_table_param(
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table_data_geometry.len() as _,
                    pDescriptorRanges: table_data_geometry.as_ptr(),
                },
            ),
            pass::gen_root_table_param(
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table_data_textures.len() as _,
                    pDescriptorRanges: table_data_textures.as_ptr(),
                },
            ),
            pass::gen_root_table_param(
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table_data_draw.len() as _,
                    pDescriptorRanges: table_data_draw.as_ptr(),
                },
            ),
            // Light data
            pass::gen_root_constants_param(
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 4,
                    Num32BitValues: mem::size_of::<LightData>() as u32 / 4,
                },
            ),
            pass::gen_root_table_param(
                D3D12_SHADER_VISIBILITY_ALL,
                D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: table_data_light.len() as _,
                    pDescriptorRanges: table_data_light.as_ptr(),
                },
            ),
        ];

        let static_samplers = [
            // Clamp to border sampler for basic material texture sampling (albedo, roughness, ..).
            D3D12_STATIC_SAMPLER_DESC {
                Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
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
                RegisterSpace: 2,
                ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
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

        let pipeline = engine.create_compute_pipeline(&signature, &cs_shader);

        Lighting {
            signature,
            pipeline,
        }
    }
}
