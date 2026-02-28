use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{core::state::DeviceBackend, entity::{entities::cube::PrimitiveCube, texture::Texture}};
use cgmath::{Deg, Vector2, Vector3, prelude::*};
use wgpu::{BindGroup, BindGroupLayout, TextureFormat, util::DeviceExt};

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub struct MeshBufferManager {
    pub vertex_buffer: wgpu::Buffer,
    pub indices_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub capacity: usize,
}

impl MeshBufferManager {
    pub fn new(
        device: &wgpu::Device,
        capacity: usize,
        vertices: &Vec<PrimitiveVertex>,
        indices: &Vec<u32>,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Instance Buffer"),
            size: (capacity * std::mem::size_of::<InstanceRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            instance_buffer: buffer,
            capacity,
            indices_buffer,
            vertex_buffer,
        }
    }

    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        vertices: &Vec<PrimitiveVertex>,
        indices: &Vec<u32>,
    ) {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.vertex_buffer = vertex_buffer;
        self.indices_buffer = indices_buffer;
    }
}
#[derive(Clone)]
pub struct InstanceController {
    pub instances: Vec<Instance>,
    pub offset: usize, // Where in the big instance buffer this mesh's data starts
    pub count: usize,
    pub atomic_usize: Arc<AtomicUsize>,
}
impl InstanceController {
    pub fn new(instances: Vec<Instance>) -> InstanceController {
        let len = instances
            .clone()
            .iter()
            .filter(|instance| instance.should_render)
            .map(Instance::to_raw)
            .collect::<Vec<_>>()
            .len();
        InstanceController {
            instances: instances.clone(),
            offset: 0,
            atomic_usize: Arc::new(AtomicUsize::new(len)),
            count: len,
        }
    }
}
pub struct RenderableController {
    pub buffer_address: u64,
    pub buffer_manager: MeshBufferManager,
    pub render_information: RenderInformation,
    pub render_mesh_information: Vec<RenderMeshInformation>,
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u32>,
    pub storage_buffer: Option<InstanceStorage>,
}

impl RenderableController {
    pub fn new(
        buffer_manager: MeshBufferManager,
        vertices: Vec<PrimitiveVertex>,
        indices: Vec<u32>,
        buffer_address: u64,
        render_information: RenderInformation,
        render_mesh_information: Vec<RenderMeshInformation>,
        storage_buffer: Option<InstanceStorage>,
    ) -> RenderableController {
        RenderableController {
            buffer_address,
            buffer_manager,
            render_mesh_information,
            render_information,
            vertices,
            indices,
            storage_buffer,
        }
    }
    pub fn update_all(&mut self, queue: &wgpu::Queue) {
        // Flatten all instances from all meshes

        let mut offset = 0;
        for controller in self.render_mesh_information.iter_mut() {
            controller.instance_controller.offset = offset;
            controller.instance_controller.count = controller.instance_controller.instances.len();
            offset += controller.instance_controller.count;
        }

        let all_instances: Vec<&Instance> = self
            .render_mesh_information
            .iter()
            .flat_map(|rmi| rmi.instance_controller.instances.iter())
            .collect();

        let all_instances_raw: Vec<InstanceRaw> = all_instances
            .iter()
            .map(|inst| inst.to_raw_fast())
            .collect();
        queue.write_buffer(
            &self.buffer_manager.instance_buffer,
            0,
            bytemuck::cast_slice(&all_instances_raw),
        );
        if let Some(storage) = &self.storage_buffer {
            queue.write_buffer(
                &storage.storage_buffer,
                0,
                bytemuck::cast_slice(&storage.instances),
            );
        }
    }

    pub fn update_mesh_data(&mut self, meshes: Vec<PrimitiveMesh>, device: &wgpu::Device) {
        let vertices = meshes
            .iter()
            .flat_map(|mesh| mesh.vertices.iter().cloned())
            .collect();
        let indices = meshes
            .iter()
            .flat_map(|mesh| mesh.indices.iter().cloned())
            .collect();

        self.buffer_manager
            .update_buffers(device, &vertices, &indices);
        let mut vertex_offset = 0;
        let mut index_offset = 0;
        let instances = instance_cube(
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        );
        let render_meshes: Vec<RenderMeshInformation> = meshes
            .iter()
            .enumerate()
            .map(|(i, mesh)| {
                let ri = RenderMeshInformation {
                    index: i,
                    instance_controller: InstanceController::new(vec![instances.clone()]),
                    num_vertices: mesh.vertices.len() as u32,
                    num_indices: mesh.indices.len() as u32,
                    vertex_offset,
                    index_offset,
                };

                // increment offsets for next mesh
                vertex_offset += mesh.vertices.len() as u32;
                index_offset += mesh.indices.len() as u32;

                ri
            })
            .collect();
        self.render_mesh_information = render_meshes;
        self.vertices = vertices;
        self.indices = indices;
    }
}

#[derive(Clone)]
pub struct Instance {
    pub index: u32,
    pub position: cgmath::Vector3<f32>,
    pub rotation: cgmath::Quaternion<f32>,
    pub should_render: bool,
    pub scale: f32,
    pub color: cgmath::Vector3<f32>,
    pub size: cgmath::Vector3<f32>,
    pub bounding: cgmath::Vector3<f32>,
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: ((cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation))
                * cgmath::Matrix4::from_nonuniform_scale(self.scale, self.scale, self.scale))
            .into(),
            color: self.color.into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
        }
    }
    pub fn to_raw_fast(&self) -> InstanceRaw {
        let s = self.scale;
        let rotation: [[f32; 3]; 3] = cgmath::Matrix3::from(self.rotation).into();

        // Compute R * S (scale each column of rotation)
        let mut model = [[0.0; 4]; 4];
        for i in 0..3 {
            model[0][i] = rotation[0][i] * s;
            model[1][i] = rotation[1][i] * s;
            model[2][i] = rotation[2][i] * s;
        }

        // Now apply translation (T * R * S)
        model[3][0] = self.position.x;
        model[3][1] = self.position.y;
        model[3][2] = self.position.z;
        model[3][3] = 1.0;
        InstanceRaw {
            model,
            color: self.color.into(),
            normal: rotation,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    #[allow(dead_code)]
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub normal: [[f32; 3]; 3],
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We don't have to do this in code though.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 25]>() as wgpu::BufferAddress,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub trait DrawMesh {
    #[allow(unused)]
    fn draw_meshes(
        &mut self,
        instances: &RenderableController,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
        backend: &DeviceBackend,
    );
}

impl<'a> DrawMesh for wgpu::RenderPass<'a> {
    fn draw_meshes(
        &mut self,
        renderables: &RenderableController,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
        backend: &DeviceBackend,
    ) {
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        if let Some(storage) = &renderables.render_information.instance_storage_layout {
            self.set_bind_group(2, storage, &[]);
        }
        self.set_pipeline(&renderables.render_information.pipeline);
        if let Some(diffuse) = &renderables.render_information.diffuse {
            self.set_bind_group(3, diffuse, &[]);
        }
        self.set_vertex_buffer(1, renderables.buffer_manager.instance_buffer.slice(..));
        self.set_index_buffer(
            renderables.buffer_manager.indices_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        self.set_vertex_buffer(0, renderables.buffer_manager.vertex_buffer.slice(..));

        self.draw_indexed(0..renderables.indices.len() as u32, 0, 0..1);
        match backend {
            DeviceBackend::WebGL => {
                let vertex_size = std::mem::size_of::<PrimitiveVertex>() as wgpu::BufferAddress;
                for renderable in renderables.render_mesh_information.iter() {
                    self.set_vertex_buffer(
                        0,
                        renderables.buffer_manager.vertex_buffer.slice(
                            (renderable.vertex_offset as wgpu::BufferAddress * vertex_size)
                                ..((renderable.vertex_offset + renderable.num_vertices)
                                    as wgpu::BufferAddress
                                    * vertex_size),
                        ),
                    );
                    self.draw_indexed(
                        renderable.index_offset..renderable.index_offset + renderable.num_indices,
                        0,
                        renderable.instance_controller.offset as u32
                            ..renderable.instance_controller.offset as u32
                                + renderable
                                    .instance_controller
                                    .atomic_usize
                                    .load(Ordering::Relaxed)
                                    as u32,
                    );
                }
            }
            DeviceBackend::WebGPU => {
                self.set_vertex_buffer(0, renderables.buffer_manager.vertex_buffer.slice(..));
                for renderable in renderables.render_mesh_information.iter() {
                    self.draw_indexed(
                        renderable.index_offset..renderable.index_offset + renderable.num_indices,
                        renderable.vertex_offset as i32,
                        renderable.instance_controller.offset as u32
                            ..renderable.instance_controller.offset as u32
                                + renderable
                                    .instance_controller
                                    .atomic_usize
                                    .load(Ordering::Relaxed)
                                    as u32,
                    );
                }
            }
        }
    }
}

pub trait Rendering {
    fn get_render_definitions(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: TextureFormat,
        queue: &wgpu::Queue,
        camera_bind_group_layout: BindGroupLayout,
        light_bind_group_layout: BindGroupLayout,
        light_bind_group: BindGroup,
        texture_bytes: Option<Vec<u8>>,
        storage_buffer: &Option<InstanceStorage>,
    ) -> RenderInformation;
}

pub struct TexturedMesh {
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u16>,
    pub texture_bytes: Vec<u8>,
    pub vertex_offset: u32,
    pub index_offset: u32,
}

impl Rendering for TexturedMesh {
    fn get_render_definitions(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: TextureFormat,
        queue: &wgpu::Queue,
        camera_bind_group_layout: BindGroupLayout,
        light_bind_group_layout: BindGroupLayout,
        light_bind_group: BindGroup,
        texture_bytes: Option<Vec<u8>>,
    ) -> RenderInformation {
        // let diffuse_bytes = texture_bytes.unwrap();
        // let diffuse_texture =
        //     Texture::from_bytes(&device, &queue, &diffuse_bytes, "happy-tree.png").unwrap();
        // log::warn!("Texture");


        let ri: RenderInformation = RenderInformation {
            diffuse: Some(diffuse_bind_group),
            light_bind_group,
            pipeline: render_pipeline,
        };

        ri
    }
// }
pub struct PrimitiveMesh {
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u32>,
}

    fn get_render_definitions(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        format: TextureFormat,
        _queue: &wgpu::Queue,
        camera_bind_group_layout: BindGroupLayout,
        light_bind_group_layout: BindGroupLayout,
        light_bind_group: BindGroup,
        _texture_bytes: Option<Vec<u8>>,
        storage_buffer: &Option<InstanceStorage>,
    ) -> RenderInformation {
        let (render_pipeline_layout, storage_buffer) = if let Some(storage) = storage_buffer {
            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &camera_bind_group_layout,
                        &light_bind_group_layout,
                        &storage.storage_bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });
            (
                render_pipeline_layout,
                Some(storage.storage_bind_group.clone()),
            )
        } else {
            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout],
                    push_constant_ranges: &[],
                });
            (render_pipeline_layout, None)
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[PrimitiveVertex::desc(), InstanceRaw::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less, // standard depth test
                stencil: wgpu::StencilState::default(),     // no stencil operations
                bias: wgpu::DepthBiasState::default(),
            }),
            // depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview: None,
            // Useful for optimizing shader compilation on Android
            cache: None,
        });

    }
}
// pub struct RenderInformation {
//     pub pipeline: wgpu::RenderPipeline,
//     pub light_bind_group: wgpu::BindGroup,
//     pub instance_storage_layout: Option<wgpu::BindGroup>,
//     pub diffuse: Option<wgpu::BindGroup>,
// }
//
// #[derive(Clone)]
// pub struct RenderMeshInformation {
//     pub index: usize,
//     pub instance_controller: InstanceController,
//     pub vertex_offset: u32,
//     pub index_offset: u32,
//     pub num_indices: u32,
//     pub num_vertices: u32,
// }

// pub struct Quad {
//     pub p00: PrimitiveVertex,
//     pub p01: PrimitiveVertex,
//     pub p10: PrimitiveVertex,
//     pub p11: PrimitiveVertex,
// }
pub fn make_cube_primitive() -> PrimitiveMesh {
    let cube = PrimitiveCube::new();
    let polygon: PrimitiveMesh = PrimitiveMesh {
        vertices: cube.vertices,
        indices: cube.indices,
    };

    polygon
}

// pub fn make_face_primitive() -> PrimitiveMesh {
//     let cube = PrimitiveFace::new();
//     let polygon: PrimitiveMesh = PrimitiveMesh {
//         vertices: cube.vertices,
//         indices: cube.indices,
//     };

//     polygon
// }

// pub fn make_mesh_from_face(prim_face: &PrimitiveFace) -> PrimitiveMesh {
//     let polygon: PrimitiveMesh = PrimitiveMesh {
//         vertices: prim_face.vertices.clone(),
//         indices: prim_face.indices.clone(),
//     };

//     polygon
// }

pub fn instance_cube(position: Vector3<f32>, color: Vector3<f32>) -> Instance {
    let rotation = if position.is_zero() {
        // this is needed so an object at (0, 0, 0) won't get scaled to zero
        // as Quaternions can effect scale if they're not created correctly
        cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
    } else {
        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
    };
    let default_color = cgmath::Vector3::new(1.0, 0.0, 0.0);
    let default_size = cgmath::Vector3::new(1.0, 1.0, 1.0) * 50.0;
    let default_bounding = default_size + position;

    Instance {
        index: 0,
        position,
        rotation,
        scale: 25.0,
        should_render: true,
        color: color,
        size: default_size,
        bounding: default_bounding,
    }
}

pub fn instances_list_cylinder(chunk_size: Vector3<u32>) -> Vec<Instance> {
    let center = (chunk_size.x / 2, chunk_size.z / 2);
    let radius = center.0 as i32;
    (0..(chunk_size.x * chunk_size.y * chunk_size.z))
        .map(move |n| {
            let x = n % chunk_size.x;
            let z = (n / chunk_size.x) % chunk_size.z;
            let y = n / (chunk_size.x * chunk_size.z);

            let dx = x as i32 - center.0 as i32;
            let dy = z as i32 - center.1 as i32;

            let distance_squared = dx * dx + dy * dy;
            let position = cgmath::Vector3 {
                x: x as f32 + (chunk_size.x as i32) as f32,
                y: y as f32,
                z: z as f32 + (chunk_size.z as i32) as f32,
            };

            let mut rotation = if position.is_zero() {
                // this is needed so an object at (0, 0, 0) won't get scaled to zero
                // as Quaternions can effect scale if they're not created correctly
                cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
            } else {
                cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
            };
            // let rotate_45 = cgmath::Quaternion::from_axis_angle(Vector3::unit_z(), Deg(45.0));
            let rotate_y_45 = cgmath::Quaternion::from_axis_angle(Vector3::unit_x(), Deg(90.0));

            // 🔹 Apply the rotation
            rotation = rotate_y_45 * rotation; // order matters
            let default_color = cgmath::Vector3::new(1.0, 0.0, 0.0);
            let default_size = cgmath::Vector3::new(1.0, 1.0, 1.0);
            let default_bounding = default_size + position;

            if distance_squared > radius * radius
                || x == 0
                || x == radius as u32
                || z == 0
                || z == radius as u32
            {
                Instance {
                    index: n,
                    position,
                    rotation,
                    scale: 0.5,
                    should_render: false,
                    color: default_color,
                    size: default_size,
                    bounding: default_bounding,
                }
            } else {
                Instance {
                    index: n,
                    position,
                    rotation,
                    scale: 0.5,
                    should_render: true,
                    color: default_color,
                    size: default_size,
                    bounding: default_bounding,
                }
            }
        })
        .collect::<Vec<_>>()
}
// pub fn instances_list2() -> Vec<Instance> {
//     (0..NUM_INSTANCES)
//         .map(move |n| {
//             let x = n % NUM_INSTANCES_PER_ROW;
//             let z = n / NUM_INSTANCES_PER_ROW;
//             let position = cgmath::Vector3 {
//                 x: x as f32 + 10.0,
//                 y: 0.0,
//                 z: z as f32 + 10.0,
//             };

//             let rotation = if position.is_zero() {
//                 // this is needed so an object at (0, 0, 0) won't get scaled to zero
//                 // as Quaternions can effect scale if they're not created correctly
//                 cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
//             } else {
//                 cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
//             };

//             let default_color = cgmath::Vector3::new(1.0, 0.0, 0.0);
//             let default_size = cgmath::Vector3::new(1.0, 1.0, 1.0);
//             let default_bounding = default_size + position;

//             Instance {
//                 index: n,
//                 position,
//                 rotation,
//                 scale: 0.5,
//                 should_render: true,
//                 color: default_color,
//                 size: default_size,
//                 bounding: default_bounding,
//             }
//         })
//         .collect::<Vec<_>>()
//     // Vec::new()
// }
