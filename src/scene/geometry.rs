//! Scene geometry.
//!
//! The scene geometry is split into multiple hierarchies:
//!
//!  * Mesh: Single GPU **resource** storing all vertex and index information.
//!          Fully contains all geometry information in the world.
//!          Vertex information split into multiple deinterleaved attributes:
//!             - Position: float3
//!
//!  * Geometry: Submesh **asset** defining a subslice of the index and vertex data
//!              from the `Mesh` resource for CPU command submission.
//!
//!  * DrawData: GPU representation of `Geometry` data. Unique **resource** allowing
//!              to rebuild submeshes on the GPU.
//!
//!  * Instance: Instantiations of a `Geometry` associated with an entity.
//!              Instance components are usually coupled with a `LocalTransform` for
//!              positioning and orientation in the world.

use specs::prelude::*;
use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::shared::minwindef::UINT;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

/// Vertex position attribute.
// #[repr(hlsl)]
#[repr(C)]
pub struct VertexPos(pub [f32; 3]);

/// Mesh resource.
///
/// Defining the whole scene geometry.
pub struct Mesh {
    pub vertex_buffer: ComPtr<ID3D12Resource>,
    pub vertex_buffer_size: UINT,
    pub vertex_stride: UINT,
    pub index_buffer: ComPtr<ID3D12Resource>,
    pub index_format: DXGI_FORMAT,
    pub index_buffer_size: UINT,
    // Index
    // Vertex position
    pub start_srvs: UINT,
}
unsafe impl Send for Mesh {}
unsafe impl Sync for Mesh {}

/// Submesh geometry asset.
///
/// Geometry of usually one independent object.
pub struct Geometry {
    pub id: usize,
    pub base_index: usize,
    pub num_indices: usize,
    pub base_vertex: usize,
}
impl Component for Geometry {
    type Storage = HashMapStorage<Self>;
}

#[repr(C)]
pub struct DrawData {
    pub base_index: u32,
    pub base_vertex: u32,
}

/// Draw data resource.
///
/// Connection between the geometry and mesh for the GPU.
/// GPU representation of the gemeotry assets.
pub struct DrawDataBuffer(pub ComPtr<ID3D12Resource>);
unsafe impl Send for DrawDataBuffer {}
unsafe impl Sync for DrawDataBuffer {}

/// Geometry instance.
///
/// Instance of the one submesh in the world.
pub struct Instance {
    pub geometry: Entity,
}
impl Component for Instance {
    type Storage = VecStorage<Self>;
}
