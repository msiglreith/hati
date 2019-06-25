extern crate assimp;
extern crate assimp_sys;
extern crate cgmath;
extern crate failure;
extern crate image;
extern crate serde_json;
extern crate specs;
extern crate time;
extern crate winapi;
extern crate winit;
extern crate wio;

mod engine;
mod pass;
mod scene;
mod swapchain;

use cgmath::*;
use engine::Engine;
use failure::Error;
use pass::lighting;
use scene::{Scene, SceneLoader};
use specs::Join;
use std::{mem, ptr, slice};
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::*;
use winapi::shared::minwindef::FALSE;
use winapi::um::d3d12::*;
use winapi::um::d3dcommon::*;
use winapi::um::synchapi::*;
use winit::WindowEvent;

const FRAME_LATENCY: u64 = 2;

#[repr(C)]
struct ViewData {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub position: [f32; 4],
    pub _alignment: [f32; 28],
}

fn main() -> Result<(), Error> {
    let mut events_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new()
        .with_dimensions(1440, 704)
        .with_title("Hati")
        .build(&events_loop)?;

    // Initialization order matters to allow more efficient resource resetting:
    //  * Engine
    //  * Pipeline
    //  * Scene
    let mut engine = Engine::new(FRAME_LATENCY);
    let swapchain = engine.create_swapchain(&window);

    let (window_width, window_height) = window.get_inner_size().unwrap();
    let pipeline_settings = pass::pipeline::PipelineSettings {
        width: window_width,
        height: window_height,
        samples: 1,
    };
    let pipeline = pass::pipeline::Pipeline::new(&mut engine, pipeline_settings);
    let mut scene = Scene::new();

    let upload_alloc = engine.create_command_allocator();

    // Load Scene
    let upload_list = engine.create_command_list(&upload_alloc);
    unsafe {
        upload_list.Reset(upload_alloc.as_raw(), ptr::null_mut());
    }

    let upload_resources = {
        let mut scene_loader = SceneLoader::new(&mut scene, &mut engine);
        scene_loader.set_upload_list(upload_list.clone());
        scene_loader.load_hati_scene("scene/Sponza", "sponza.obj")
    };

    unsafe {
        upload_list.Close();
        engine
            .queue
            .ExecuteCommandLists(1, &(upload_list.as_raw() as *mut _));
    }

    // View data (camera)
    let view_data_size = engine.frame_latency() * mem::size_of::<ViewData>() as u64;
    let view_data_desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: view_data_size,
        Height: 1,
        DepthOrArraySize: 1,
        Format: DXGI_FORMAT_UNKNOWN,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    };
    let view_heap = engine.create_descriptor_heap(
        engine.frame_latency() as _,
        D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
    );
    let view_data = engine.create_committed_resource(
        D3D12_HEAP_TYPE_UPLOAD,
        &view_data_desc,
        D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
        None,
    );
    let view_cbvs = unsafe {
        let view_buffer_start = view_data.GetGPUVirtualAddress();
        (0..engine.frame_latency())
            .map(|i| view_buffer_start + i * mem::size_of::<ViewData>() as u64)
            .collect::<Vec<_>>()
    };

    let mut camera = scene::Camera {
        position: Point3::new(0.0, 100.0, 0.0),
        rotation: [Rad(-1.2), Rad(0.0), Rad(0.0)],
        up: Vector3::new(0.0, 1.0, 0.0),

        view_move: (false, false),
        view_rotate: (false, false, false, false),

        depth_range: 0.0..1.0,
        focal_length: 1.0,
    };

    let present_fence = engine.create_fence(0, D3D12_FENCE_FLAG_NONE);

    let cmd_allocs: [_; FRAME_LATENCY as _] = [
        engine.create_command_allocator(),
        engine.create_command_allocator(),
    ];
    let cmd_lists: [_; FRAME_LATENCY as _] = [
        engine.create_command_list(&cmd_allocs[0]),
        engine.create_command_list(&cmd_allocs[1]),
    ];

    let mut tick: u64 = 0;
    let time_start = time::PreciseTime::now();
    let mut time_last = time_start;
    let mut quit = false;

    loop {
        // Event handling
        events_loop.poll_events(|event| match event {
            winit::Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => quit = true,
            winit::Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                camera.on_event(input);
            }
            _ => {}
        });

        // Synchronize GPU -> CPU to handle resource reuse
        if tick >= FRAME_LATENCY {
            unsafe {
                present_fence.SetEventOnCompletion(tick - FRAME_LATENCY, engine.wait_event);
                WaitForSingleObject(engine.wait_event, 5_0000);
            }
        }

        if quit {
            break;
        }

        // TODO: not fully accurate handling seconds
        let time_now = time::PreciseTime::now();
        let time_elapsed_s =
            time_last.to(time_now).num_microseconds().unwrap() as f32 / 1_000_000.0;
        time_last = time_now;

        window.set_title(&format!("Hati - frame: {:.2} ms", time_elapsed_s * 1000.0));

        // ! Frame Begin ----------------------------------------------------------------------------------
        let frame = swapchain.begin_frame();
        let (present_target, present_rtv) = swapchain.get_render_target(frame);
        let (window_width, window_height) = window.get_inner_size().unwrap();

        let cmd_list = &cmd_lists[frame];
        unsafe {
            cmd_allocs[frame].Reset();
            cmd_list.Reset(cmd_allocs[frame].as_raw(), ptr::null_mut());
        }
        engine.bind_descriptor_heaps(&cmd_list);

        // Backbuffer: Present -> RenderTarget
        let present_rt_transition = [
            engine::gen_resource_transition(
                &present_target,
                D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_BARRIER_FLAG_NONE,
            ),
            engine::gen_resource_transition(
                &pipeline.geometry_buffer,
                D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_BARRIER_FLAG_NONE,
            ),
            engine::gen_resource_transition(
                &pipeline.lighting_buffer,
                D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                D3D12_RESOURCE_BARRIER_FLAG_NONE,
            ),
        ];
        unsafe {
            cmd_list.ResourceBarrier(
                present_rt_transition.len() as _,
                present_rt_transition.as_ptr(),
            );
            cmd_list.ClearRenderTargetView(
                pipeline.geometry_rtv_uint,
                &[0.0, 0.0, 0.0, 0.0],
                0,
                ptr::null(),
            );
            cmd_list.ClearDepthStencilView(
                pipeline.dsv,
                D3D12_CLEAR_FLAG_DEPTH,
                1.0,
                0,
                0,
                ptr::null(),
            );
        }

        // Geometry pass
        unsafe {
            cmd_list.SetGraphicsRootSignature(pipeline.geometry.signature.as_raw());
            cmd_list.SetPipelineState(pipeline.geometry.pipeline.as_raw());
            cmd_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            cmd_list.RSSetViewports(
                1,
                &D3D12_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: window_width as _,
                    Height: window_height as _,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                },
            );
            cmd_list.RSSetScissorRects(
                1,
                &D3D12_RECT {
                    left: 0,
                    top: 0,
                    right: window_width as _,
                    bottom: window_height as _,
                },
            );
            cmd_list.OMSetRenderTargets(1, &pipeline.geometry_rtv_uint, FALSE, &pipeline.dsv);
            cmd_list.SetGraphicsRootConstantBufferView(0, view_cbvs[frame]);
        }

        // Update view data
        camera.update(time_elapsed_s);

        let mut view_raw_data = ptr::null_mut();
        let view_data_cpu = unsafe {
            view_data.Map(0, ptr::null(), &mut view_raw_data);
            slice::from_raw_parts_mut::<ViewData>(view_raw_data as _, engine.frame_latency() as _)
        };
        view_data_cpu[frame].view = camera.view().into();
        view_data_cpu[frame].proj = {
            let aspect_ratio = window_width as f32 / window_height as f32;
            let mut perspective = cgmath::perspective(cgmath::Deg(60.0), aspect_ratio, 1.0, 8192.0);
            perspective.w.z /= 2.0; // OpenGL NDC -> DX12 NDC
            perspective.into()
        };
        view_data_cpu[frame].position =
            [camera.position.x, camera.position.y, camera.position.z, 1.0];
        unsafe {
            view_data.Unmap(0, ptr::null());
        }

        let mesh = scene.assets.read_resource::<scene::Mesh>();

        // Draw scene geometry
        {
            unsafe {
                cmd_list.IASetIndexBuffer(&D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: mesh.index_buffer.GetGPUVirtualAddress(),
                    SizeInBytes: mesh.index_buffer_size,
                    Format: mesh.index_format,
                });
                cmd_list.IASetVertexBuffers(
                    0,
                    1,
                    &D3D12_VERTEX_BUFFER_VIEW {
                        BufferLocation: mesh.vertex_buffer.GetGPUVirtualAddress(),
                        SizeInBytes: mesh.vertex_buffer_size,
                        StrideInBytes: mesh.vertex_stride,
                    },
                );
                cmd_list.SetGraphicsRootDescriptorTable(
                    1,
                    D3D12_GPU_DESCRIPTOR_HANDLE {
                        ptr: engine.cbv_srv_uav_start.1.ptr
                            + (mesh.start_srvs * engine.cbv_srv_uav_size) as u64,
                    },
                );
            }

            let transforms = scene.world.read_storage::<scene::LocalTransform>();
            let instances = scene.world.read_storage::<scene::Instance>();
            let geometries = scene.assets.read_storage::<scene::Geometry>();

            for (_, instance) in (&transforms, &instances).join() {
                let geometry = geometries.get(instance.geometry).unwrap();
                unsafe {
                    let draw_constants = [geometry.base_index as u32, geometry.base_vertex as u32];
                    cmd_list.SetGraphicsRoot32BitConstants(
                        2,
                        draw_constants.len() as _,
                        draw_constants.as_ptr() as _,
                        0,
                    );
                    cmd_list.SetGraphicsRoot32BitConstant(3, geometry.id as _, 0);
                    cmd_list.DrawIndexedInstanced(
                        geometry.num_indices as _,
                        1,
                        geometry.base_index as _,
                        geometry.base_vertex as _,
                        0,
                    );
                }
            }
        }

        // Geometry/visibility buffer: Render Target -> SRV
        let geometry_srv_transition = engine::gen_resource_transition(
            &pipeline.geometry_buffer,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            D3D12_RESOURCE_BARRIER_FLAG_NONE,
        );
        unsafe {
            cmd_list.ResourceBarrier(1, &geometry_srv_transition);
        }

        // Lighting/shading pass
        assert_eq!(pipeline_settings.width % lighting::TILE_THREADS_X, 0);
        assert_eq!(pipeline_settings.height % lighting::TILE_THREADS_Y, 0);

        let light_data = pass::lighting::LightData {
            num_point_lights: scene.point_lights.len() as _,
        };
        let light_data_raw: [u32; 1] = unsafe { mem::transmute(light_data) };
        let lights = scene.world.read_resource::<scene::light::LightDataBuffer>();

        unsafe {
            cmd_list.SetComputeRootSignature(pipeline.lighting.signature.as_raw());
            cmd_list.SetPipelineState(pipeline.lighting.pipeline.as_raw());
            cmd_list.SetComputeRootDescriptorTable(0, pipeline.lighting_uav);
            cmd_list.SetComputeRootDescriptorTable(1, pipeline.geometry_srv_uint);
            cmd_list.SetComputeRootDescriptorTable(
                2,
                D3D12_GPU_DESCRIPTOR_HANDLE {
                    ptr: engine.cbv_srv_uav_start.1.ptr
                        + engine.cbv_srv_uav_size as u64 * scene.texture_srvs.start_id as u64,
                },
            );
            cmd_list.SetComputeRootDescriptorTable(
                3,
                D3D12_GPU_DESCRIPTOR_HANDLE {
                    ptr: engine.cbv_srv_uav_start.1.ptr
                        + (mesh.start_srvs * engine.cbv_srv_uav_size) as u64,
                },
            );
            cmd_list.SetComputeRoot32BitConstants(
                4,
                light_data_raw.len() as _,
                light_data_raw.as_ptr() as _,
                0,
            );
            cmd_list.SetComputeRootDescriptorTable(
                5,
                D3D12_GPU_DESCRIPTOR_HANDLE {
                    ptr: engine.cbv_srv_uav_start.1.ptr
                        + (lights.start_srvs * engine.cbv_srv_uav_size) as u64,
                },
            );
            cmd_list.Dispatch(
                pipeline_settings.width / lighting::TILE_THREADS_X,
                pipeline_settings.height / lighting::TILE_THREADS_Y,
                1,
            );

            let lighting_uav_barriers = [
                engine::gen_uav_barrier(
                    &pipeline.lighting_buffer,
                    D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ),
                engine::gen_resource_transition(
                    &pipeline.lighting_buffer,
                    D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                    D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ),
            ];
            cmd_list.ResourceBarrier(
                lighting_uav_barriers.len() as _,
                lighting_uav_barriers.as_ptr(),
            );
        }

        // Post Processing
        unsafe {
            cmd_list.SetGraphicsRootSignature(pipeline.post_process.display_map.signature.as_raw());
            cmd_list.SetPipelineState(pipeline.post_process.display_map.pipeline.as_raw());
            cmd_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            cmd_list.RSSetViewports(
                1,
                &D3D12_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: window_width as _,
                    Height: window_height as _,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                },
            );
            cmd_list.RSSetScissorRects(
                1,
                &D3D12_RECT {
                    left: 0,
                    top: 0,
                    right: window_width as _,
                    bottom: window_height as _,
                },
            );
            cmd_list.OMSetRenderTargets(1, &present_rtv, FALSE, ptr::null());
            cmd_list.SetGraphicsRootDescriptorTable(0, pipeline.lighting_srv);
            cmd_list.DrawInstanced(3, 1, 0, 0);
        }

        // Backbuffer: RenderTarget -> Present
        let rt_present_transition = engine::gen_resource_transition(
            &present_target,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_BARRIER_FLAG_NONE,
        );
        unsafe {
            cmd_list.ResourceBarrier(1, &rt_present_transition);
        }

        unsafe {
            cmd_list.Close();
        }

        unsafe {
            engine
                .queue
                .ExecuteCommandLists(1, &(cmd_list.as_raw() as *mut _));
            engine.queue.Signal(present_fence.as_raw(), tick);
        }
        swapchain.end_frame();

        // ! Frame End ----------------------------------------------------------------------------------

        tick += 1;
    }

    unsafe {
        present_fence.SetEventOnCompletion(tick - 1, engine.wait_event);
        WaitForSingleObject(engine.wait_event, 5_0000);
    }

    Ok(())
}
