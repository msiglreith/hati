use std::fs::File;
use std::path::Path;
use std::{mem, ptr, slice};

use winapi::shared::dxgi::*;
use winapi::shared::dxgi1_3::*;
use winapi::shared::dxgi1_4::*;
use winapi::shared::minwindef::UINT;
use winapi::shared::ntdef::HANDLE;
use winapi::shared::winerror;
use winapi::um::d3d12::*;
use winapi::um::d3d12sdklayers::*;
use winapi::um::d3dcommon::*;
use winapi::um::d3dcompiler::*;
use winapi::um::synchapi::*;
use winapi::Interface;

use wio::com::ComPtr;

const NUM_CBV_SRV_UAV_DESCRIPTORS: UINT = 2048;
const NUM_SAMPLER_DESCRIPTORS: UINT = 128;

pub struct Engine {
    pub factory: ComPtr<IDXGIFactory4>,
    pub device: ComPtr<ID3D12Device>,
    pub queue: ComPtr<ID3D12CommandQueue>,
    pub wait_event: HANDLE,

    frame_latency: u64,

    // global gpu descriptor heaps
    cbv_srv_uav_heap: ComPtr<ID3D12DescriptorHeap>,
    sampler_heap: ComPtr<ID3D12DescriptorHeap>,

    pub cbv_srv_uav_size: UINT,
    pub sampler_size: UINT,

    cbv_srv_uav_next: UINT,
    sampler_next: UINT,

    pub cbv_srv_uav_start: (D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_GPU_DESCRIPTOR_HANDLE),
    pub sampler_start: (D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_GPU_DESCRIPTOR_HANDLE),
}

impl Engine {
    fn init_debug() {
        let mut debug_controller: *mut ID3D12Debug = ptr::null_mut();
        let hr = unsafe {
            D3D12GetDebugInterface(
                &ID3D12Debug::uuidof(),
                &mut debug_controller as *mut *mut _ as *mut *mut _,
            )
        };

        if winerror::SUCCEEDED(hr) {
            unsafe {
                (*debug_controller).EnableDebugLayer();
                (*debug_controller).Release();
            }
        }
    }

    fn select_adapter(factory: &ComPtr<IDXGIFactory4>) -> ComPtr<IDXGIAdapter1> {
        let mut adapter_id = 0;
        loop {
            let adapter = {
                let mut adapter: *mut IDXGIAdapter1 = ptr::null_mut();
                let hr = unsafe { factory.EnumAdapters1(adapter_id, &mut adapter as *mut *mut _) };
                if hr == winerror::DXGI_ERROR_NOT_FOUND {
                    break;
                }
                unsafe { ComPtr::from_raw(adapter) }
            };

            adapter_id += 1;

            // Check for D3D12 support
            {
                let mut device: *mut ID3D12Device = ptr::null_mut();
                let hr = unsafe {
                    D3D12CreateDevice(
                        adapter.as_raw() as *mut _,
                        D3D_FEATURE_LEVEL_12_0,
                        &ID3D12Device::uuidof(),
                        &mut device as *mut *mut _ as *mut *mut _,
                    )
                };
                if !winerror::SUCCEEDED(hr) {
                    continue;
                }
                unsafe {
                    (*device).Release();
                }
            };

            return adapter;
        }

        panic!("Couldn't find suitable D3D12 adapter");
    }

    pub fn new(frame_latency: u64) -> Self {
        // Ceate DXGI factory.
        let factory = {
            let mut dxgi_factory: *mut IDXGIFactory4 = ptr::null_mut();

            let _ = if true {
                // cfg!(debug_assertions) {
                Self::init_debug();
                unsafe {
                    CreateDXGIFactory2(
                        DXGI_CREATE_FACTORY_DEBUG,
                        &IDXGIFactory4::uuidof(),
                        &mut dxgi_factory as *mut *mut _ as *mut *mut _,
                    )
                }
            } else {
                unsafe {
                    CreateDXGIFactory1(
                        &IDXGIFactory4::uuidof(),
                        &mut dxgi_factory as *mut *mut _ as *mut *mut _,
                    )
                }
            };

            unsafe { ComPtr::from_raw(dxgi_factory) }
        };

        // Find suitable adapter and open device.
        let adapter = Self::select_adapter(&factory);
        let device = {
            let mut device: *mut ID3D12Device = ptr::null_mut();
            let _ = unsafe {
                D3D12CreateDevice(
                    adapter.as_raw() as *mut _,
                    D3D_FEATURE_LEVEL_12_0,
                    &ID3D12Device::uuidof(),
                    &mut device as *mut *mut _ as *mut *mut _,
                )
            };
            unsafe { ComPtr::from_raw(device) }
        };

        // Create associated direct queue (also used for present).
        let queue = {
            let queue_desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                Priority: 0,
                Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };

            let mut queue: *mut ID3D12CommandQueue = ptr::null_mut();
            let _ = unsafe {
                device.CreateCommandQueue(
                    &queue_desc,
                    &ID3D12CommandQueue::uuidof(),
                    &mut queue as *mut *mut _ as *mut *mut _,
                )
            };
            unsafe { ComPtr::from_raw(queue) }
        };

        let wait_event = unsafe { CreateEventA(ptr::null_mut(), 0, 0, ptr::null_mut()) };

        let cbv_srv_uav_heap = create_descriptor_heap(
            &device,
            NUM_CBV_SRV_UAV_DESCRIPTORS as _,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        );
        let cbv_srv_uav_size = unsafe {
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
        };
        let cbv_srv_uav_start = unsafe {
            let cpu = cbv_srv_uav_heap.GetCPUDescriptorHandleForHeapStart();
            let gpu = cbv_srv_uav_heap.GetGPUDescriptorHandleForHeapStart();
            (cpu, gpu)
        };

        let sampler_heap = create_descriptor_heap(
            &device,
            NUM_SAMPLER_DESCRIPTORS as _,
            D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        );
        let sampler_size =
            unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER) };
        let sampler_start = unsafe {
            let cpu = sampler_heap.GetCPUDescriptorHandleForHeapStart();
            let gpu = sampler_heap.GetGPUDescriptorHandleForHeapStart();
            (cpu, gpu)
        };

        Engine {
            factory,
            device,
            queue,
            wait_event,
            frame_latency,
            cbv_srv_uav_heap,
            cbv_srv_uav_next: 0,
            cbv_srv_uav_size,
            cbv_srv_uav_start,
            sampler_heap,
            sampler_next: 0,
            sampler_size,
            sampler_start,
        }
    }

    pub fn create_command_allocator(&self) -> ComPtr<ID3D12CommandAllocator> {
        let mut command_allocator: *mut ID3D12CommandAllocator = ptr::null_mut();
        let _ = unsafe {
            self.device.CreateCommandAllocator(
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &ID3D12CommandAllocator::uuidof(),
                &mut command_allocator as *mut *mut _ as *mut *mut _,
            )
        };
        unsafe { ComPtr::from_raw(command_allocator) }
    }

    pub fn create_command_list(
        &self,
        allocator: &ComPtr<ID3D12CommandAllocator>,
    ) -> ComPtr<ID3D12GraphicsCommandList> {
        let mut command_list: *mut ID3D12GraphicsCommandList = ptr::null_mut();
        let _ = unsafe {
            self.device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                allocator.as_raw(),
                ptr::null_mut(),
                &ID3D12CommandList::uuidof(),
                &mut command_list as *mut *mut _ as *mut *mut _,
            )
        };

        unsafe {
            (*command_list).Close();
            ComPtr::from_raw(command_list)
        }
    }

    pub fn create_committed_resource(
        &self,
        ty: D3D12_HEAP_TYPE,
        desc: &D3D12_RESOURCE_DESC,
        initial: D3D12_RESOURCE_STATES,
        clear_value: Option<D3D12_CLEAR_VALUE>,
    ) -> ComPtr<ID3D12Resource> {
        let mut resource: *mut ID3D12Resource = ptr::null_mut();
        let heap_properties = unsafe { self.device.GetCustomHeapProperties(0, ty) };

        let _ = unsafe {
            self.device.CreateCommittedResource(
                &heap_properties,
                D3D12_HEAP_FLAG_NONE,
                desc,
                initial,
                match clear_value {
                    Some(ref cv) => cv,
                    None => ptr::null(),
                },
                &ID3D12Resource::uuidof(),
                &mut resource as *mut *mut _ as *mut *mut _,
            )
        };

        unsafe { ComPtr::from_raw(resource) }
    }

    pub fn create_descriptor_heap(
        &self,
        size: usize,
        ty: D3D12_DESCRIPTOR_HEAP_TYPE,
        flags: D3D12_DESCRIPTOR_HEAP_FLAGS,
    ) -> ComPtr<ID3D12DescriptorHeap> {
        create_descriptor_heap(&self.device, size, ty, flags)
    }

    pub fn create_fence(&self, initial: u64, flags: D3D12_FENCE_FLAGS) -> ComPtr<ID3D12Fence> {
        let mut fence: *mut ID3D12Fence = ptr::null_mut();
        let _ = unsafe {
            self.device.CreateFence(
                initial,
                flags,
                &ID3D12Fence::uuidof(),
                &mut fence as *mut *mut _ as *mut *mut _,
            )
        };
        unsafe { ComPtr::from_raw(fence) }
    }

    pub fn load_shader<P: AsRef<Path>>(
        &self,
        name: &str,
        path: P,
        entry_point: &str,
        target: &str,
    ) -> Result<ComPtr<ID3DBlob>, String> {
        use std::io::Read;

        let mut shader_file = File::open(path).unwrap();
        let mut shader_source = String::new();
        shader_file.read_to_string(&mut shader_source).unwrap();

        self.create_shader(name, &shader_source, entry_point, target)
    }

    pub fn create_shader(
        &self,
        _name: &str,
        source: &str,
        entry_point: &str,
        target: &str,
    ) -> Result<ComPtr<ID3DBlob>, String> {
        let mut shader = ptr::null_mut();
        let mut error = ptr::null_mut();

        let hr = unsafe {
            D3DCompile(
                source.as_ptr() as *const _,
                source.len(),
                ptr::null(),
                ptr::null(),
                D3D_COMPILE_STANDARD_FILE_INCLUDE,
                entry_point.as_ptr() as *const _,
                target.as_ptr() as *const i8,
                D3DCOMPILE_DEBUG | D3DCOMPILE_ENABLE_UNBOUNDED_DESCRIPTOR_TABLES, // TODO
                0,
                &mut shader as *mut *mut _,
                &mut error as *mut *mut _,
            )
        };
        if !winerror::SUCCEEDED(hr) {
            let error = unsafe { ComPtr::<ID3DBlob>::from_raw(error) };
            let message = unsafe {
                let pointer = error.GetBufferPointer();
                let size = error.GetBufferSize();
                let slice = slice::from_raw_parts(pointer as *const u8, size as usize);
                String::from_utf8_lossy(slice).into_owned()
            };

            Err(message)
        } else {
            Ok(unsafe { ComPtr::<ID3DBlob>::from_raw(shader) })
        }
    }

    pub fn create_graphics_pipeline(
        &self,
        desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC,
    ) -> ComPtr<ID3D12PipelineState> {
        let mut pipeline = ptr::null_mut();
        let _ = unsafe {
            self.device.CreateGraphicsPipelineState(
                desc as *const _,
                &ID3D12PipelineState::uuidof(),
                &mut pipeline as *mut *mut _ as *mut *mut _,
            )
        };
        unsafe { ComPtr::from_raw(pipeline) }
    }

    pub fn create_root_signature(
        &self,
        desc: &D3D12_ROOT_SIGNATURE_DESC,
    ) -> Result<ComPtr<ID3D12RootSignature>, String> {
        let mut signature = ptr::null_mut();
        let mut serialized = ptr::null_mut();
        let mut error = ptr::null_mut();

        unsafe {
            let _ = D3D12SerializeRootSignature(
                desc as *const _,
                D3D_ROOT_SIGNATURE_VERSION_1_0,
                &mut serialized,
                &mut error,
            );

            if !error.is_null() {
                let pointer = (*error).GetBufferPointer();
                let size = (*error).GetBufferSize();
                let slice = slice::from_raw_parts(pointer as *const u8, size as usize);
                let message = String::from_utf8_lossy(slice).into_owned();
                (*error).Release();

                return Err(message);
            }

            self.device.CreateRootSignature(
                0,
                (*serialized).GetBufferPointer(),
                (*serialized).GetBufferSize(),
                &ID3D12RootSignature::uuidof(),
                &mut signature as *mut *mut _ as *mut *mut _,
            );
            (*serialized).Release();

            Ok(ComPtr::from_raw(signature))
        }
    }

    pub fn create_compute_pipeline(
        &self,
        signature: &ComPtr<ID3D12RootSignature>,
        shader: &ComPtr<ID3DBlob>,
    ) -> ComPtr<ID3D12PipelineState> {
        let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: signature.as_raw(),
            CS: ::pass::unpack_shader_bc(shader),
            NodeMask: 0,
            CachedPSO: D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
        };

        let mut pipeline = ptr::null_mut();
        let _ = unsafe {
            self.device.CreateComputePipelineState(
                &desc as *const _,
                &ID3D12PipelineState::uuidof(),
                &mut pipeline as *mut *mut _ as *mut *mut _,
            )
        };
        unsafe { ComPtr::from_raw(pipeline) }
    }

    pub fn frame_latency(&self) -> u64 {
        self.frame_latency
    }

    pub fn reset_descriptors(&mut self, cbv_srv_uav: UINT, sampler: UINT) {
        self.cbv_srv_uav_next = cbv_srv_uav;
        self.sampler_next = sampler;
    }

    pub fn allocate_descriptors(&mut self, cbv_srv_uav: UINT, sampler: UINT) -> (UINT, UINT) {
        assert!(self.cbv_srv_uav_next + cbv_srv_uav < NUM_CBV_SRV_UAV_DESCRIPTORS);
        assert!(self.sampler_next + sampler < NUM_SAMPLER_DESCRIPTORS);

        let result = (self.cbv_srv_uav_next, self.sampler_next);
        self.cbv_srv_uav_next += cbv_srv_uav;
        self.sampler_next += sampler;
        result
    }

    pub fn bind_descriptor_heaps(&self, cmd_list: &ComPtr<ID3D12GraphicsCommandList>) {
        let mut descriptor_heaps = [self.cbv_srv_uav_heap.as_raw(), self.sampler_heap.as_raw()];
        unsafe {
            cmd_list.SetDescriptorHeaps(descriptor_heaps.len() as _, descriptor_heaps.as_mut_ptr());
        }
    }
}

pub fn gen_resource_transition(
    resource: &ComPtr<ID3D12Resource>,
    subresource: UINT,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
    flags: D3D12_RESOURCE_BARRIER_FLAGS,
) -> D3D12_RESOURCE_BARRIER {
    let mut barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: flags,
        u: unsafe { mem::zeroed() },
    };
    *unsafe { barrier.u.Transition_mut() } = D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: resource.as_raw(),
        Subresource: subresource,
        StateBefore: before,
        StateAfter: after,
    };
    barrier
}

pub fn gen_uav_barrier(
    resource: &ComPtr<ID3D12Resource>,
    flags: D3D12_RESOURCE_BARRIER_FLAGS,
) -> D3D12_RESOURCE_BARRIER {
    let mut barrier = D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_UAV,
        Flags: flags,
        u: unsafe { mem::zeroed() },
    };
    *unsafe { barrier.u.UAV_mut() } = D3D12_RESOURCE_UAV_BARRIER {
        pResource: resource.as_raw(),
    };
    barrier
}

fn create_descriptor_heap(
    device: &ComPtr<ID3D12Device>,
    size: usize,
    ty: D3D12_DESCRIPTOR_HEAP_TYPE,
    flags: D3D12_DESCRIPTOR_HEAP_FLAGS,
) -> ComPtr<ID3D12DescriptorHeap> {
    let mut heap: *mut ID3D12DescriptorHeap = ptr::null_mut();
    let desc = D3D12_DESCRIPTOR_HEAP_DESC {
        Type: ty,
        NumDescriptors: size as _,
        Flags: flags,
        NodeMask: 0,
    };
    let _ = unsafe {
        device.CreateDescriptorHeap(
            &desc,
            &ID3D12DescriptorHeap::uuidof(),
            &mut heap as *mut *mut _ as *mut *mut _,
        )
    };
    unsafe { ComPtr::from_raw(heap) }
}
