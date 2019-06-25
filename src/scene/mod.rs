use assimp;
use assimp::import::Importer;
use assimp_sys;
use cgmath::*;
use engine::{self, Engine};
use image;
use pass;
use specs::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::{mem, ptr, slice};
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::shared::minwindef::UINT;
use winapi::um::d3d12::*;
use wio::com::ComPtr;

pub mod camera;
pub mod geometry;
pub mod light;
pub mod transform;

pub use self::camera::Camera;
pub use self::geometry::{Geometry, Instance, Mesh};
pub use self::transform::LocalTransform;

pub struct Scene {
    pub world: World,
    pub assets: World,

    pub texture_srvs: TextureViewGroup,
    pub point_lights: HashMap<Entity, usize>,
}

impl Scene {
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<camera::Camera>();
        world.register::<transform::LocalTransform>();
        world.register::<geometry::Instance>();
        world.register::<light::PointLight>();

        let mut assets = World::new();
        assets.register::<geometry::Geometry>();
        assets.register::<Texture>();
        assets.register::<TextureView>();

        Scene {
            world,
            assets,
            texture_srvs: TextureViewGroup { start_id: 0 },
            point_lights: HashMap::new(),
        }
    }

    pub fn unload(&mut self) {
        self.world.delete_all();
        self.assets.delete_all();

        // TODO: free all descriptors
    }
}

/// Temporary resources created during resource upload.
/// Can be destroyed once the upload is complete.
pub struct UploadResources {
    resources: Vec<ComPtr<ID3D12Resource>>,
}

pub struct SceneLoader<'a> {
    upload_cmd_list: Option<ComPtr<ID3D12GraphicsCommandList>>,
    engine: &'a mut Engine,
    scene: &'a mut Scene,
}

impl<'a> SceneLoader<'a> {
    pub fn new(scene: &'a mut Scene, engine: &'a mut Engine) -> Self {
        // Currently only support 1 index/vertex buffer.
        scene.unload();

        SceneLoader {
            upload_cmd_list: None,
            scene,
            engine,
        }
    }

    pub fn set_upload_list(&mut self, list: ComPtr<ID3D12GraphicsCommandList>) {
        self.upload_cmd_list = Some(list);
    }

    /*
    pub fn load_fscene<P0, P1>(&mut self, scene_dir: P0, scene_name: P1) -> Result<(), Error>
    where
        P0: AsRef<Path>,
        P1: AsRef<Path>,
    {
        let scene_file = File::open(scene_dir.as_ref().join(scene_name.as_ref()))?;
        let scene_desc: Value = serde_json::from_reader(scene_file)?;
        assert!(scene_desc.is_object());

        if let Some(cameras) = scene_desc.get("cameras") {
            self.load_fscene_cameras(&cameras)?;
        }
        if let Some(models) = scene_desc.get("models") {
            self.load_fscene_models(&models, scene_dir)?;
        }

        Ok(())
    }

    fn load_fscene_cameras(&mut self, cameras: &Value) -> Result<(), Error> {
        let cameras = cameras.as_array().unwrap();

        for camera in cameras {
            assert!(camera.is_object());

            let focal_length = match camera.get("focal_length") {
                Some(length) => length.as_f64().unwrap() as _,
                None => 1.0f32,
            };

            let depth_range = match camera.get("depth_range") {
                Some(range) => {
                    let range = range.as_array().expect("expected array for depth range");
                    let near = range[0].as_f64().unwrap() as f32;
                    let far = range[1].as_f64().unwrap() as f32;

                    near..far
                }
                None => 0.0..1.0,
            };

            self.scene.world
                .create_entity()
                // TODO
                /*
                .with(
                    camera::Camera {
                        depth_range,
                        focal_length,
                    }
                )
                */
                .build();
        }

        Ok(())
    }

    fn load_fscene_models<P: AsRef<Path>>(
        &mut self,
        models: &Value,
        scene_dir: P,
    ) -> Result<UploadResources, Error> {
        let mut upload_resources = UploadResources {
            resources: Vec::new(),
        };

        let models = models.as_array().unwrap();
        for model in models {
            let file_name = model
                .get("file")
                .and_then(Value::as_str)
                .expect("Missing model path");
            let upload = self.load_assimp(&scene_dir, file_name);
            upload_resources.resources.extend(upload.resources);
        }

        Ok(upload_resources)
    }
    */

    pub fn load_hati_scene<P0, P1>(&mut self, scene_dir: P0, path: P1) -> UploadResources
    where
        P0: AsRef<Path>,
        P1: AsRef<Path>,
    {
        // Generate point lights.
        for i in 0..10 {
            let e = self
                .scene
                .world
                .create_entity()
                .with(light::PointLight {
                    intensity: 1000.0 * i as f32,
                })
                .with(transform::LocalTransform::new(
                    Vector3::new(-1100.0 + i as f32 * 250.0, 80.0, 0.0),
                    1.0,
                    Euler {
                        x: Rad(0.0),
                        y: Rad(0.0),
                        z: Rad(0.0),
                    },
                    None,
                ))
                .build();
            self.scene.point_lights.insert(e, i);
        }

        let num_point_lights = self.scene.point_lights.len();
        let point_light_data_size = num_point_lights * mem::size_of::<pass::lighting::PointLight>();

        let light_data_point_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: point_light_data_size as _,
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
        let light_data_point = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_UPLOAD,
            &light_data_point_desc,
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
            None,
        );

        let mut light_data_point_raw = ptr::null_mut();
        let light_data_point_cpu = unsafe {
            light_data_point.Map(0, ptr::null(), &mut light_data_point_raw);
            slice::from_raw_parts_mut::<pass::lighting::PointLight>(
                light_data_point_raw as _,
                num_point_lights as _,
            )
        };

        {
            let transforms = self.scene.world.read_storage::<transform::LocalTransform>();
            let point_lights = self.scene.world.read_storage::<light::PointLight>();
            let entities = self.scene.world.entities();

            for (e, transform, light) in (&*entities, &transforms, &point_lights).join() {
                let idx = *self.scene.point_lights.get(&e).unwrap();
                let transform = transform.world_transform(&transforms);
                light_data_point_cpu[idx] = pass::lighting::PointLight {
                    position: [transform.w.x, transform.w.y, transform.w.z],
                    intensity: light.intensity,
                };
            }
        }

        unsafe {
            light_data_point.Unmap(0, ptr::null());
        }

        let light_srvs = self.engine.allocate_descriptors(1, 0).0;
        let light_point_srv = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.engine.cbv_srv_uav_start.0.ptr
                + (light_srvs * self.engine.cbv_srv_uav_size) as usize,
        };

        unsafe {
            let mut light_point_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
                ..mem::zeroed()
            };
            *light_point_desc.u.Buffer_mut() = D3D12_BUFFER_SRV {
                FirstElement: 0,
                NumElements: num_point_lights as _,
                StructureByteStride: mem::size_of::<pass::lighting::PointLight>() as _,
                Flags: D3D12_BUFFER_SRV_FLAG_NONE,
            };
            self.engine.device.CreateShaderResourceView(
                light_data_point.as_raw(),
                &light_point_desc,
                light_point_srv,
            );
        }

        self.scene.world.add_resource(light::LightDataBuffer {
            point_buffer: light_data_point,
            start_srvs: light_srvs,
        });

        self.load_obj(scene_dir, path)
    }

    pub fn load_obj<P0, P1>(&mut self, scene_dir: P0, path: P1) -> UploadResources
    where
        P0: AsRef<Path>,
        P1: AsRef<Path>,
    {
        self.load_assimp(scene_dir, path.as_ref().to_str().unwrap())
    }

    fn load_assimp<P: AsRef<Path>>(&mut self, scene_dir: P, scene: &str) -> UploadResources {
        let mut importer = Importer::new();
        importer.triangulate(true);

        let model_scene = importer
            .read_file(scene_dir.as_ref().join(Path::new(scene)).to_str().unwrap())
            .unwrap();

        let mut upload_resources = Vec::new();

        // let mut textures = Vec::new();
        // for material in model_scene.material_iter() {
        //     enum MaterialKey {
        //         Name,
        //         Texture(assimp_sys::AiTextureType),
        //     }
        //     fn get_material_string(
        //         material: &assimp::Material,
        //         key: MaterialKey,
        //     ) -> Option<String> {
        //         let (key, ty) = match key {
        //             MaterialKey::Name => (b"?mat.name\0", 0),
        //             MaterialKey::Texture(ty) => (b"$tex.file\0", ty as u32),
        //         };
        //         unsafe {
        //             let mut string_val: assimp_sys::AiString = mem::zeroed();
        //             let result = assimp_sys::aiGetMaterialString(
        //                 &**material,
        //                 key.as_ptr() as *const _,
        //                 ty,
        //                 0,
        //                 &mut string_val,
        //             ) as usize;
        //             // FFI result enum values are wrong
        //             match result {
        //                 0 => {
        //                     let string = ::std::ffi::CStr::from_bytes_with_nul_unchecked(
        //                         &string_val.data[..string_val.length + 1],
        //                     );
        //                     Some(string.to_str().unwrap().into())
        //                 }
        //                 _ => None,
        //             }
        //         }
        //     }
        //     println!("{:?}", get_material_string(&material, MaterialKey::Name));

        //     if let Some(albedo_name) = get_material_string(
        //         &material,
        //         MaterialKey::Texture(assimp_sys::AiTextureType::Diffuse),
        //     ) {
        //         let (albedo_img, albedo_upload) =
        //             self.load_image_rgba8(scene_dir.as_ref().join(Path::new(&albedo_name)));
        //         upload_resources.extend(albedo_upload.resources);
        //         textures.push(
        //             self.scene
        //                 .assets
        //                 .create_entity()
        //                 .with(Texture {
        //                     resource: albedo_img,
        //                 }).build(),
        //         );
        //     }
        // }

        // let (texture_srvs, _) = self.engine.allocate_descriptors(textures.len() as _, 0);
        // self.scene.texture_srvs.start_id = texture_srvs as _;
        // {
        //     let texture_strg = self.scene.assets.read_storage::<Texture>();
        //     for (i, tid) in textures.iter().enumerate() {
        //         let texture = texture_strg.get(*tid).unwrap();
        //         let srv = D3D12_CPU_DESCRIPTOR_HANDLE {
        //             ptr: self.engine.cbv_srv_uav_start.0.ptr
        //                 + ((texture_srvs + i as u32) * self.engine.cbv_srv_uav_size) as usize,
        //         };

        //         unsafe {
        //             let mut srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
        //                 Format: DXGI_FORMAT_R8G8B8A8_UNORM, // TODO
        //                 ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
        //                 Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
        //                 ..mem::zeroed()
        //             };
        //             *srv_desc.u.Texture2D_mut() = D3D12_TEX2D_SRV {
        //                 MostDetailedMip: 0,
        //                 MipLevels: 1,
        //                 PlaneSlice: 0,
        //                 ResourceMinLODClamp: 0.0,
        //             };
        //             self.engine.device.CreateShaderResourceView(
        //                 texture.resource.as_raw(),
        //                 &srv_desc,
        //                 srv,
        //             );
        //         }
        //     }
        // }

        let mut num_vertices = 0;
        let mut num_indices = 0;
        for mesh in model_scene.mesh_iter() {
            num_vertices += mesh.num_vertices();
            num_indices += mesh.num_faces() * 3;
        }

        let default_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: 0,
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

        let vertex_buffer_size = num_vertices as u64 * mem::size_of::<geometry::VertexPos>() as u64;
        let vertex_buffer = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Width: vertex_buffer_size,
                ..default_desc
            },
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );

        let vertex_buffer_upload = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_UPLOAD,
            &D3D12_RESOURCE_DESC {
                Width: vertex_buffer_size,
                ..default_desc
            },
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            None,
        );
        let mut vertex_data = ptr::null_mut();
        let vertices_pos_cpu = unsafe {
            vertex_buffer_upload.Map(0, ptr::null(), &mut vertex_data);
            slice::from_raw_parts_mut::<geometry::VertexPos>(vertex_data as _, num_vertices as _)
        };

        let index_buffer_size = num_indices as u64 * mem::size_of::<u32>() as u64;
        let index_buffer = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Width: index_buffer_size,
                ..default_desc
            },
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );

        let index_buffer_upload = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_UPLOAD,
            &D3D12_RESOURCE_DESC {
                Width: index_buffer_size,
                ..default_desc
            },
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            None,
        );
        let mut index_data = ptr::null_mut();
        let indices_cpu = unsafe {
            index_buffer_upload.Map(0, ptr::null(), &mut index_data);
            slice::from_raw_parts_mut::<u32>(index_data as _, num_indices as _)
        };

        // SRVs for index & vertex buffer and draw data.
        // Required for shading and barycentric coord calculation.
        let (buffer_srvs, _) = self.engine.allocate_descriptors(3, 0);
        let index_srv = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.engine.cbv_srv_uav_start.0.ptr
                + (buffer_srvs * self.engine.cbv_srv_uav_size) as usize,
        };
        let vertex_srv = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.engine.cbv_srv_uav_start.0.ptr
                + ((buffer_srvs + 1) * self.engine.cbv_srv_uav_size) as usize,
        };
        let draw_data_srv = D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.engine.cbv_srv_uav_start.0.ptr
                + ((buffer_srvs + 2) * self.engine.cbv_srv_uav_size) as usize,
        };
        unsafe {
            let mut vertex_srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
                ..mem::zeroed()
            };
            *vertex_srv_desc.u.Buffer_mut() = D3D12_BUFFER_SRV {
                FirstElement: 0,
                NumElements: num_vertices,
                StructureByteStride: mem::size_of::<geometry::VertexPos>() as _,
                Flags: D3D12_BUFFER_SRV_FLAG_NONE,
            };

            let mut index_srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
                ..mem::zeroed()
            };
            *index_srv_desc.u.Buffer_mut() = D3D12_BUFFER_SRV {
                FirstElement: 0,
                NumElements: num_indices,
                StructureByteStride: mem::size_of::<u32>() as _,
                Flags: D3D12_BUFFER_SRV_FLAG_NONE,
            };

            self.engine.device.CreateShaderResourceView(
                vertex_buffer.as_raw(),
                &vertex_srv_desc,
                vertex_srv,
            );
            self.engine.device.CreateShaderResourceView(
                index_buffer.as_raw(),
                &index_srv_desc,
                index_srv,
            );
        }

        self.scene.assets.add_resource(geometry::Mesh {
            vertex_buffer: vertex_buffer.clone(),
            vertex_buffer_size: vertex_buffer_size as _,
            vertex_stride: mem::size_of::<geometry::VertexPos>() as _,
            index_buffer: index_buffer.clone(),
            index_buffer_size: index_buffer_size as _,
            index_format: DXGI_FORMAT_R32_UINT,
            start_srvs: buffer_srvs,
        });

        let mut base_index = 0;
        let mut base_vertex = 0;

        let geometries = model_scene
            .mesh_iter()
            .enumerate()
            .map(|(id, mesh)| {
                let num_local_indices = mesh.num_faces() as usize * 3;
                let num_local_vertices = mesh.num_vertices() as usize;

                for (i, vertex) in mesh.vertex_iter().enumerate() {
                    let v = base_vertex + i as usize;
                    vertices_pos_cpu[v] = geometry::VertexPos([vertex.x, vertex.y, vertex.z]);
                }

                for (i, face) in mesh.face_iter().enumerate() {
                    let e = base_index + 3 * i;
                    let raw_indices = unsafe { slice::from_raw_parts(face.indices, 3) };
                    indices_cpu[e] = raw_indices[0];
                    indices_cpu[e + 1] = raw_indices[1];
                    indices_cpu[e + 2] = raw_indices[2];
                }

                let geometry = self
                    .scene
                    .assets
                    .create_entity()
                    .with(Geometry {
                        id,
                        base_index,
                        num_indices: num_local_indices,
                        base_vertex,
                    })
                    .build();

                base_index += num_local_indices;
                base_vertex += num_local_vertices;

                geometry
            })
            .collect::<Vec<_>>();

        unsafe {
            vertex_buffer_upload.Unmap(0, ptr::null());
            index_buffer_upload.Unmap(0, ptr::null());
        }

        let draw_data_buffer_size = geometries.len() * mem::size_of::<geometry::DrawData>();
        let draw_data = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Width: draw_data_buffer_size as _,
                ..default_desc
            },
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );

        let draw_data_upload = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_UPLOAD,
            &D3D12_RESOURCE_DESC {
                Width: draw_data_buffer_size as _,
                ..default_desc
            },
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            None,
        );

        let mut draw_data_raw = ptr::null_mut();
        let draw_data_cpu = unsafe {
            draw_data_upload.Map(0, ptr::null(), &mut draw_data_raw);
            slice::from_raw_parts_mut::<geometry::DrawData>(draw_data_raw as _, geometries.len())
        };

        {
            let geometry_data = self.scene.assets.read_storage::<Geometry>();
            for (i, geometry) in geometries.iter().enumerate() {
                let g = geometry_data.get(*geometry).unwrap();
                draw_data_cpu[i] = geometry::DrawData {
                    base_vertex: g.base_vertex as _,
                    base_index: g.base_index as _,
                };
            }
        }

        unsafe {
            draw_data_upload.Unmap(0, ptr::null());
        }

        unsafe {
            let mut draw_data_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_UNKNOWN,
                ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                Shader4ComponentMapping: 0x1688, // D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING
                ..mem::zeroed()
            };
            *draw_data_desc.u.Buffer_mut() = D3D12_BUFFER_SRV {
                FirstElement: 0,
                NumElements: geometries.len() as _,
                StructureByteStride: mem::size_of::<geometry::DrawData>() as _,
                Flags: D3D12_BUFFER_SRV_FLAG_NONE,
            };
            self.engine.device.CreateShaderResourceView(
                draw_data.as_raw(),
                &draw_data_desc,
                draw_data_srv,
            );
        }

        {
            // Staging vertex & index buffer and draw data
            let upload_list = self
                .upload_cmd_list
                .as_ref()
                .expect("upload command list not set");

            unsafe {
                upload_list.CopyResource(vertex_buffer.as_raw(), vertex_buffer_upload.as_raw());
                upload_list.CopyResource(index_buffer.as_raw(), index_buffer_upload.as_raw());
                upload_list.CopyResource(draw_data.as_raw(), draw_data_upload.as_raw());
            }

            // Use resources as index and vertex buffers.
            // Additionally used as buffer SRVs for barycentric coords calculation
            // in the geometry pixel shader.
            let mesh_data_transitions = [
                engine::gen_resource_transition(
                    &vertex_buffer,
                    D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER
                        | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                    D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ),
                engine::gen_resource_transition(
                    &index_buffer,
                    D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_INDEX_BUFFER | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                    D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ),
                engine::gen_resource_transition(
                    &index_buffer,
                    D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE,
                    D3D12_RESOURCE_BARRIER_FLAG_NONE,
                ),
            ];
            unsafe {
                upload_list.ResourceBarrier(
                    mesh_data_transitions.len() as _,
                    mesh_data_transitions.as_ptr(),
                );
            }
        }

        self.scene
            .assets
            .add_resource(geometry::DrawDataBuffer(draw_data));

        upload_resources.extend(vec![
            vertex_buffer_upload,
            index_buffer_upload,
            draw_data_upload,
        ]);

        self.load_node(&geometries, &model_scene.root_node(), None);

        UploadResources {
            resources: upload_resources,
        }
    }

    fn load_image_rgba8<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> (ComPtr<ID3D12Resource>, UploadResources) {
        let img = image::open(path).unwrap().to_rgba();
        let (width, height) = img.dimensions();

        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: width as _,
            Height: height as _,
            DepthOrArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            MipLevels: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        };
        let image = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_DEFAULT,
            &desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        );

        let num_texels = width * height;
        let upload_buffer_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: 4 * num_texels as u64,
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
        let upload_buffer = self.engine.create_committed_resource(
            D3D12_HEAP_TYPE_UPLOAD,
            &upload_buffer_desc,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            None,
        );

        let mut image_data = ptr::null_mut();
        let image_data_cpu = unsafe {
            upload_buffer.Map(0, ptr::null(), &mut image_data);
            slice::from_raw_parts_mut::<u8>(image_data as _, 4 * num_texels as usize)
        };

        let row_pitch = 4 * width as usize; // TODO: alignment
        for y in 0..height as usize {
            let row = &(*img)[y * row_pitch..(y + 1) * row_pitch];
            let dst = y * row_pitch as usize;
            image_data_cpu[dst..dst + row.len()].copy_from_slice(row);
        }

        unsafe {
            upload_buffer.Unmap(0, ptr::null());
        }

        {
            // Staging vertex and index buffer data
            let upload_list = self
                .upload_cmd_list
                .as_ref()
                .expect("upload command list not set");

            // Image mipmap
            let mut dst_location = D3D12_TEXTURE_COPY_LOCATION {
                pResource: image.as_raw(),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                ..unsafe { mem::zeroed() }
            };
            unsafe {
                *dst_location.u.SubresourceIndex_mut() = 0;
            }

            // Upload buffer
            let mut src_location = D3D12_TEXTURE_COPY_LOCATION {
                pResource: upload_buffer.as_raw(),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                ..unsafe { mem::zeroed() }
            };
            unsafe {
                *src_location.u.PlacedFootprint_mut() = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                    Offset: 0,
                    Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        Width: width as _,
                        Height: height as _,
                        Depth: 1,
                        RowPitch: 4 * width as UINT,
                    },
                };
            }

            unsafe {
                upload_list.CopyTextureRegion(&dst_location, 0, 0, 0, &src_location, ptr::null());
            }

            // Use image as shader resource view only
            let transitions = [engine::gen_resource_transition(
                &image,
                D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE
                    | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                D3D12_RESOURCE_BARRIER_FLAG_NONE,
            )];
            unsafe {
                upload_list.ResourceBarrier(transitions.len() as _, transitions.as_ptr());
            }
        }

        let upload_resources = UploadResources {
            resources: vec![upload_buffer],
        };

        (image, upload_resources)
    }

    fn load_node(&mut self, geometries: &[Entity], node: &assimp::Node, parent: Option<Entity>) {
        let tfm = node.transformation();
        let entity = self
            .scene
            .world
            .create_entity()
            .with(transform::LocalTransform {
                transform: [
                    [tfm.a1, tfm.a2, tfm.a3, tfm.a4],
                    [tfm.b1, tfm.b2, tfm.b3, tfm.b4],
                    [tfm.c1, tfm.c2, tfm.c3, tfm.c4],
                    [tfm.d1, tfm.d2, tfm.d3, tfm.d4],
                ],
                parent,
            })
            .build();

        for mesh in node.meshes() {
            self.scene
                .world
                .create_entity()
                .with(Instance {
                    geometry: geometries[*mesh as usize],
                })
                .with(transform::LocalTransform {
                    transform: [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                    parent: Some(entity),
                })
                .build();
        }

        for child in node.child_iter() {
            self.load_node(geometries, &child, Some(entity));
        }
    }
}

pub struct Texture {
    pub resource: ComPtr<ID3D12Resource>,
}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}
impl Component for Texture {
    type Storage = HashMapStorage<Self>;
}

pub struct TextureViewGroup {
    pub start_id: usize,
}

pub struct TextureView {
    // ID Offset within the allocated texture view group.
    pub id: usize,
}
impl Component for TextureView {
    type Storage = HashMapStorage<Self>;
}
