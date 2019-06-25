//! Rendering pipeline

use engine::Engine;
use pass;
use pass::geometry::Geometry;
use pass::lighting::Lighting;
use pass::postprocess::PostProcess;
use std::{mem, ptr};
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

#[derive(Copy, Clone, Debug)]
pub struct PipelineSettings {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
}

pub struct Pipeline {
    pub geometry: Geometry,
    pub lighting: Lighting,
    pub post_process: PostProcess,

    ///
    pub geometry_buffer: ComPtr<ID3D12Resource>,
    pub geometry_rtv_uint: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub geometry_srv_uint: D3D12_GPU_DESCRIPTOR_HANDLE,
    ///
    pub lighting_buffer: ComPtr<ID3D12Resource>,
    pub lighting_srv: D3D12_GPU_DESCRIPTOR_HANDLE,
    pub lighting_uav: D3D12_GPU_DESCRIPTOR_HANDLE,

    pub depth_target: ComPtr<ID3D12Resource>,
    pub dsv: D3D12_CPU_DESCRIPTOR_HANDLE,

    depth_heap: ComPtr<ID3D12DescriptorHeap>,
    rtv_heap: ComPtr<ID3D12DescriptorHeap>,

    srv_uav_start_cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
    srv_uav_start_gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
    srv_uav_num: usize,
}

impl Pipeline {
    pub fn new(engine: &mut Engine, settings: PipelineSettings) -> Self {
        // TODO: support multisampling
        assert_eq!(settings.samples, 1);

        // Create depth target
        let depth_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as _,
            Width: settings.width as _,
            Height: settings.height as _,
            DepthOrArraySize: 1,
            Format: pass::DS_FORMAT,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: settings.samples,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
        };
        let mut depth_clear_value = D3D12_CLEAR_VALUE {
            Format: pass::DS_FORMAT,
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            *depth_clear_value.u.DepthStencil_mut() = D3D12_DEPTH_STENCIL_VALUE {
                Depth: 1.0,
                Stencil: 0,
            };
        }
        let depth_target = engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &depth_desc,
            D3D12_RESOURCE_STATE_DEPTH_WRITE,
            Some(depth_clear_value),
        );

        // Geometry buffer, RGBA16F
        //
        // Storing only triangle identification data along with barycentric coordinates.
        //  * R: U16 for draw call number
        //  * G: U16 for index (relative for the current draw)
        //  * B: F16 Barycentric U [0,1]
        //  * A: F16 Barycentric V [0,1]
        let gbuffer_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as _,
            Width: settings.width as _,
            Height: settings.height as _,
            DepthOrArraySize: 1,
            Format: DXGI_FORMAT_R16G16B16A16_TYPELESS,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: settings.samples,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        };
        let mut gbuffer_clear_value = D3D12_CLEAR_VALUE {
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            *gbuffer_clear_value.u.Color_mut() = [0.0, 0.0, 0.0, 0.0];
        }
        let geometry_buffer = engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &gbuffer_desc,
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            Some(gbuffer_clear_value),
        );

        // Lighting buffer, RGBA16F
        let lighting_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: D3D12_DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT as _,
            Width: settings.width as _,
            Height: settings.height as _,
            DepthOrArraySize: 1,
            Format: DXGI_FORMAT_R16G16B16A16_TYPELESS,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: settings.samples,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        };
        let mut lighting_clear_value = D3D12_CLEAR_VALUE {
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            *lighting_clear_value.u.Color_mut() = [0.0, 0.0, 0.0, 0.0];
        }
        let lighting_buffer = engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &lighting_desc,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            None,
        );

        // Resoure views -------------------------------------
        //  Allocate heaps
        let rtv_size = unsafe {
            engine
                .device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
        };
        let srv_uav_size = unsafe {
            engine
                .device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
        };

        let srv_uav_num = 3;
        let rtv_num = 1;
        let dsv_num = 1;

        let rtv_heap = engine.create_descriptor_heap(
            rtv_num,
            D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        );
        let depth_heap = engine.create_descriptor_heap(
            dsv_num,
            D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        );
        let (srv_uav_start_cpu, srv_uav_start_gpu) = {
            let (start_idx, _) = engine.allocate_descriptors(srv_uav_num, 0);
            let (srv_uav_begin_cpu, srv_uav_begin_gpu) = engine.cbv_srv_uav_start;
            let ptr_offset = start_idx as u32 * srv_uav_size;
            let srv_uav_start_cpu = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: srv_uav_begin_cpu.ptr + ptr_offset as usize,
            };
            let srv_uav_start_gpu = D3D12_GPU_DESCRIPTOR_HANDLE {
                ptr: srv_uav_begin_gpu.ptr + ptr_offset as u64,
            };

            (srv_uav_start_cpu, srv_uav_start_gpu)
        };
        let rtv_start = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

        //  Geometry buffer
        let geometry_rtv_uint = rtv_start;
        let geometry_rtv_uint_desc = D3D12_RENDER_TARGET_VIEW_DESC {
            Format: DXGI_FORMAT_R16G16B16A16_UINT,
            ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            engine.device.CreateRenderTargetView(
                geometry_buffer.as_raw(),
                &geometry_rtv_uint_desc,
                geometry_rtv_uint,
            );
        }

        let geometry_srv_uint_cpu = srv_uav_start_cpu;
        let geometry_srv_uint = srv_uav_start_gpu;
        let mut geometry_srv_uint_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R16G16B16A16_UINT,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            *geometry_srv_uint_desc.u.Texture2D_mut() = D3D12_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
                PlaneSlice: 0,
                ResourceMinLODClamp: 0.0,
            };
            engine.device.CreateShaderResourceView(
                geometry_buffer.as_raw(),
                &geometry_srv_uint_desc,
                geometry_srv_uint_cpu,
            );
        }

        // Lighting buffer
        let lighting_uav_cpu = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: srv_uav_start_cpu.ptr + srv_uav_size as usize,
        };
        let lighting_uav_gpu = D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: srv_uav_start_gpu.ptr + srv_uav_size as u64,
        };
        let mut lighting_uav_desc = D3D12_UNORDERED_ACCESS_VIEW_DESC {
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            ViewDimension: D3D12_UAV_DIMENSION_TEXTURE2D,
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            *lighting_uav_desc.u.Texture2D_mut() = D3D12_TEX2D_UAV {
                MipSlice: 0,
                PlaneSlice: 0,
            };
            engine.device.CreateUnorderedAccessView(
                lighting_buffer.as_raw(),
                ptr::null_mut(),
                &lighting_uav_desc,
                lighting_uav_cpu,
            );
        }

        let lighting_srv_cpu = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: srv_uav_start_cpu.ptr + 2 * srv_uav_size as usize,
        };
        let lighting_srv_gpu = D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: srv_uav_start_gpu.ptr + 2 * srv_uav_size as u64,
        };
        let mut lighting_srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            *lighting_srv_desc.u.Texture2D_mut() = D3D12_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
                PlaneSlice: 0,
                ResourceMinLODClamp: 0.0,
            };
            engine.device.CreateShaderResourceView(
                lighting_buffer.as_raw(),
                &lighting_srv_desc,
                lighting_srv_cpu,
            );
        }

        //  Depth target
        let dsv = unsafe { depth_heap.GetCPUDescriptorHandleForHeapStart() };
        let mut dsv_desc = D3D12_DEPTH_STENCIL_VIEW_DESC {
            Format: pass::DS_FORMAT,
            ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            engine
                .device
                .CreateDepthStencilView(depth_target.as_raw(), &dsv_desc, dsv);
        }

        Pipeline {
            geometry: Geometry::new(engine),
            geometry_buffer,
            geometry_rtv_uint,
            geometry_srv_uint,
            lighting: Lighting::new(engine),
            lighting_buffer,
            lighting_srv: lighting_srv_gpu,
            lighting_uav: lighting_uav_gpu,
            post_process: PostProcess::new(engine),
            depth_target,
            depth_heap,
            rtv_heap,
            dsv,
            srv_uav_start_cpu,
            srv_uav_start_gpu,
            srv_uav_num: srv_uav_num as _,
        }
    }
}
