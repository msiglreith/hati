use std::{mem, ptr};
use winapi::Interface;
use winapi::shared::dxgi::*;
use winapi::shared::dxgi1_2::*;
use winapi::shared::dxgi1_4::*;
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::*;
use winapi::shared::minwindef::*;
use winapi::um::d3d12::*;
use winit;
use winit::os::windows::WindowExt;
use wio::com::ComPtr;

use engine::Engine;

pub struct Swapchain {
    swapchain: ComPtr<IDXGISwapChain3>,
    render_targets: Vec<ComPtr<ID3D12Resource>>,
    rtv_pool: ComPtr<ID3D12DescriptorHeap>,
    rtv_size: usize,
}

pub type Frame = usize;

impl Engine {
    pub fn create_swapchain(&self, window: &winit::Window) -> Swapchain {
        let buffer_count = self.frame_latency() as UINT;
        let swapchain = {
            let mut swapchain: *mut IDXGISwapChain3 = ptr::null_mut();
            let format = DXGI_FORMAT_R8G8B8A8_UNORM;
            let (width, height) = window.get_inner_size().unwrap();
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                BufferCount: buffer_count,
                Width: width,
                Height: height,
                Format: format,
                Flags: 0,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Scaling: DXGI_SCALING_NONE,
                Stereo: FALSE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            };
            let _ = unsafe {
                self.factory.CreateSwapChainForHwnd(
                    self.queue.as_raw() as *mut _,
                    window.get_hwnd() as *mut _,
                    &desc,
                    ptr::null(),
                    ptr::null_mut(),
                    &mut swapchain as *mut *mut _ as *mut *mut _,
                )
            };
            unsafe { ComPtr::from_raw(swapchain) }
        };

        let rtv_pool = self.create_descriptor_heap(
            buffer_count as _,
            D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        );
        let rtv_start = unsafe { rtv_pool.GetCPUDescriptorHandleForHeapStart() };
        let rtv_size = unsafe {
            self.device
                .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
        };

        let render_targets = (0..buffer_count)
            .map(|i| {
                let mut resource: *mut ID3D12Resource = ptr::null_mut();
                unsafe {
                    swapchain.GetBuffer(
                        i as _,
                        &ID3D12Resource::uuidof(),
                        &mut resource as *mut *mut _ as *mut *mut _,
                    );
                }

                let rtv = D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: rtv_start.ptr + (i * rtv_size) as usize,
                };
                let rtv_desc = D3D12_RENDER_TARGET_VIEW_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
                    ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D,
                    ..unsafe { mem::zeroed() }
                };
                unsafe {
                    self.device.CreateRenderTargetView(resource, &rtv_desc, rtv);
                }

                unsafe { ComPtr::from_raw(resource) }
            })
            .collect::<Vec<_>>();

        Swapchain {
            swapchain,
            render_targets,
            rtv_pool,
            rtv_size: rtv_size as usize,
        }
    }
}

impl Swapchain {
    pub fn get_render_target(
        &self,
        idx: usize,
    ) -> (ComPtr<ID3D12Resource>, D3D12_CPU_DESCRIPTOR_HANDLE) {
        let rtv_start = unsafe { self.rtv_pool.GetCPUDescriptorHandleForHeapStart() };
        let rtv = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: rtv_start.ptr + (idx * self.rtv_size) as usize,
        };

        (self.render_targets[idx].clone(), rtv)
    }

    pub fn begin_frame(&self) -> Frame {
        unsafe { self.swapchain.GetCurrentBackBufferIndex() as _ }
    }

    pub fn end_frame(&self) {
        unsafe {
            self.swapchain.Present(0, 0);
        }
    }
}
