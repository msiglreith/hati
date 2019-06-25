use specs::prelude::*;
use winapi::shared::minwindef::UINT;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

pub struct LightDataBuffer {
    pub point_buffer: ComPtr<ID3D12Resource>,
    pub start_srvs: UINT,
}
unsafe impl Send for LightDataBuffer {}
unsafe impl Sync for LightDataBuffer {}

pub struct PointLight {
    pub intensity: f32,
}

impl Component for PointLight {
    type Storage = HashMapStorage<Self>;
}
