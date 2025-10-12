use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::entity::{
    entities::cube::{PrimitiveCube, PrimitiveFace, TexturedCube},
    primitive_texture::PrimitiveTexture,
    texture::Texture,
};
use cgmath::{Deg, Vector2, Vector3, prelude::*};
use wgpu::{BindGroup, BindGroupLayout, TextureFormat, util::DeviceExt};
use winit::dpi::PhysicalSize;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PrimitiveVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
}
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TexturedVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
}
impl TexturedVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TexturedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

impl PrimitiveVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<PrimitiveVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub const NUM_INSTANCES_PER_ROW: u32 = 10;
pub const NUM_INSTANCES: u32 = 100;
pub const INSTANCE_DISPLACEMENT: cgmath::Vector3<f32> = cgmath::Vector3::new(
    NUM_INSTANCES_PER_ROW as f32,
    0.0,
    NUM_INSTANCES_PER_ROW as f32,
);

pub struct BufferManager {
    pub vertex_buffer: Vec<wgpu::Buffer>,
    pub indices_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub capacity: usize,
}

impl BufferManager {
    pub fn new(
        device: &wgpu::Device,
        capacity: usize,
        vertices: &Vec<PrimitiveMesh>,
        indices: &Vec<u16>,
    ) -> Self {
        let vertice_buffers = vertices
            .iter()
            .map(|v| {
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Big Vertex Buffer"),
                    contents: bytemuck::cast_slice(&v.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                vertex_buffer
            })
            .collect();
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
            vertex_buffer: vertice_buffers,
        }
    }

    pub fn update_all(&self, queue: &wgpu::Queue, controllers: &mut [RenderMeshInformation]) {
        // Flatten all instances from all meshes
        let mut all_instances: Vec<InstanceRaw> = Vec::new();

        let mut offset = 0;
        for controller in controllers.iter_mut() {
            controller.instance_controller.offset = offset;
            controller.instance_controller.count = controller.instance_controller.instances.len();
            offset += controller.instance_controller.count;

            for inst in &controller.instance_controller.instances {
                all_instances.push(inst.to_raw());
            }
        }

        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&all_instances),
        );
    }
}

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

    fn to_raw(&mut self) -> Vec<InstanceRaw> {
        self.instances
            .clone()
            .iter()
            .filter(|instance| instance.should_render) // only include visible instances
            .map(Instance::to_raw)
            .collect()
    }
}
pub struct RenderableController {
    pub buffer_address: u64,
    pub instance_manager: BufferManager,
    pub render_information: RenderInformation,
    pub render_mesh_information: Vec<RenderMeshInformation>,
}

impl RenderableController {
    pub fn new(
        buffer_address: u64,
        instance_manager: BufferManager,
        render_information: RenderInformation,
        render_mesh_information: Vec<RenderMeshInformation>,
    ) -> RenderableController {
        RenderableController {
            buffer_address,
            instance_manager,
            render_mesh_information,
            render_information,
        }
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
            color: cgmath::Vector3::from(self.color).into(),
            normal: cgmath::Matrix3::from(self.rotation).into(),
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
    );
}

impl<'a> DrawMesh for wgpu::RenderPass<'a> {
    fn draw_meshes(
        &mut self,
        renderables: &RenderableController,
        camera_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
    ) {
        self.set_bind_group(0, camera_bind_group, &[]);
        self.set_bind_group(1, light_bind_group, &[]);
        self.set_pipeline(&renderables.render_information.pipeline);
        if let Some(diffuse) = &renderables.render_information.diffuse {
            self.set_bind_group(2, diffuse, &[]);
        }
        self.set_vertex_buffer(1, renderables.instance_manager.instance_buffer.slice(..));
        self.set_index_buffer(
            renderables.instance_manager.indices_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        for (i, renderable) in renderables.render_mesh_information.iter().enumerate() {
            self.set_vertex_buffer(0, renderables.instance_manager.vertex_buffer[i].slice(..));
            self.draw_indexed(
                renderable.index_offset..renderable.index_offset + renderable.num_indices,
                0,
                renderable.instance_controller.offset as u32
                    ..renderable.instance_controller.offset as u32
                        + (*(&renderable
                            .instance_controller
                            .atomic_usize
                            .load(Ordering::Relaxed)
                            .clone()) as usize) as u32,
            );
        }
    }
}

pub struct MeshBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
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
    ) -> RenderInformation;
    fn get_render_mesh_definitions(&self, instances: Vec<Instance>) -> RenderMeshInformation;
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
        let diffuse_bytes = texture_bytes.unwrap();
        let diffuse_texture =
            Texture::from_bytes(&device, &queue, &diffuse_bytes, "happy-tree.png").unwrap();
        log::warn!("Texture");

        // Create bind group layout for texture and sampler
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        // Create bind group for the texture
        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                    &texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TexturedVertex::desc(), InstanceRaw::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let ri: RenderInformation = RenderInformation {
            diffuse: Some(diffuse_bind_group),
            light_bind_group,
            pipeline: render_pipeline,
        };

        ri
    }
    fn get_render_mesh_definitions(&self, instances: Vec<Instance>) -> RenderMeshInformation {
        let ri = RenderMeshInformation {
            instance_controller: InstanceController::new(instances),
            vertices: vec![],
            indices: self.indices.clone(),
            vertex_offset: 0,
            index_offset: 0,
            num_indices: self.indices.len() as u32,
        };

        ri
    }
}
pub struct PrimitiveMesh {
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u16>,
}

impl Rendering for PrimitiveMesh {
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
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[PrimitiveVertex::desc(), InstanceRaw::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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
                alpha_to_coverage_enabled: true,
            },
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview: None,
            // Useful for optimizing shader compilation on Android
            cache: None,
        });

        let ri = RenderInformation {
            pipeline: render_pipeline,
            light_bind_group,
            diffuse: None,
        };

        ri
    }
    fn get_render_mesh_definitions(&self, instances: Vec<Instance>) -> RenderMeshInformation {
        let ri = RenderMeshInformation {
            instance_controller: InstanceController::new(instances),
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            vertex_offset: 0,
            index_offset: 0,
            num_indices: self.indices.len() as u32,
        };

        ri
    }
}

pub struct RenderInformation {
    pub pipeline: wgpu::RenderPipeline,
    pub light_bind_group: wgpu::BindGroup,
    pub diffuse: Option<wgpu::BindGroup>,
}
pub struct RenderMeshInformation {
    pub instance_controller: InstanceController,
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u16>,
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub num_indices: u32,
}
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

pub struct Light {
    pub position: Vector3<f32>,
    color: Vector3<f32>,
    pub instance_controller: Option<RenderableController>,
    pub light_buffer: wgpu::Buffer,
    pub light_bind_group_layout: wgpu::BindGroupLayout,
    pub light_bind_group: wgpu::BindGroup,
}

impl Light {
    pub fn new(position: Vector3<f32>, color: Vector3<f32>, device: &wgpu::Device) -> Self {
        let uniform = LightUniform {
            position: cgmath::Vector3::from(position.clone()).into(),
            _padding: 0,
            color: cgmath::Vector3::from(color.clone()).into(),
            _padding2: 0,
        };
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });

        Self {
            position,
            color,
            light_buffer,
            light_bind_group_layout,
            light_bind_group,
            instance_controller: None,
        }
    }

    pub fn get_instance(&self) -> Instance {
        let rotation = if self.position.is_zero() {
            // this is needed so an object at (0, 0, 0) won't get scaled to zero
            // as Quaternions can effect scale if they're not created correctly
            cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
        } else {
            cgmath::Quaternion::from_axis_angle(self.position.normalize(), cgmath::Deg(0.0))
        };
        let default_color = cgmath::Vector3::new(1.0, 0.0, 0.0);
        let default_size = cgmath::Vector3::new(1.0, 1.0, 1.0) * 25.0;
        let default_bounding = default_size + self.position;

        Instance {
            index: 0,
            position: self.position.clone(),
            rotation,
            scale: 25.0,
            should_render: true,
            color: default_color,
            size: default_size,
            bounding: default_bounding,
        }
    }

    pub fn to_raw(&self) -> LightUniform {
        LightUniform {
            position: cgmath::Vector3::from(self.position).into(),
            _padding: 0,
            color: cgmath::Vector3::from(self.color).into(),
            _padding2: 0,
        }
    }
}

// pub fn make_cube_textured() -> TexturedMesh {
//     let cube = TexturedCube::new();

//     let polygon: TexturedMesh = TexturedMesh {
//         vertices: cube.vertices,
//         indices: cube.indices,
//         texture_bytes: include_bytes!("../happy-tree.png").to_vec(),
//     };
//     polygon
// }

pub fn make_cube_primitive() -> PrimitiveMesh {
    let cube = PrimitiveCube::new();
    let polygon: PrimitiveMesh = PrimitiveMesh {
        vertices: cube.vertices,
        indices: cube.indices,
    };

    polygon
}

pub fn make_face_primitive() -> PrimitiveMesh {
    let cube = PrimitiveFace::new();
    let polygon: PrimitiveMesh = PrimitiveMesh {
        vertices: cube.vertices,
        indices: cube.indices,
    };

    polygon
}

pub fn make_mesh_from_face(prim_face: &PrimitiveFace) -> PrimitiveMesh {
    let polygon: PrimitiveMesh = PrimitiveMesh {
        vertices: prim_face.vertices.clone(),
        indices: prim_face.indices.clone(),
    };

    polygon
}
// pub fn instances_list(chunk: Chunk, chunk_size: Vector2<u32>) -> Vec<Instance> {
//     (0..(chunk_size.x * chunk_size.y))
//         .map(move |n| {
//             let x = n % chunk_size.x;
//             let z = n / chunk_size.y;
//             let position = cgmath::Vector3 {
//                 x: x as f32 + (chunk.x * chunk_size.x as i32) as f32,
//                 y: 0.0,
//                 z: z as f32 + (chunk.y * chunk_size.y as i32) as f32,
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
// }
// pub fn instances_list_cube(chunk: Chunk, chunk_size: Vector3<u32>) -> Vec<Instance> {
//     (0..(chunk_size.x * chunk_size.y * chunk_size.z))
//         .map(move |n| {
//             let x = n % chunk_size.x;
//             let z = (n / chunk_size.x) % chunk_size.z;
//             let y = n / (chunk_size.x * chunk_size.z);

//             let position = cgmath::Vector3 {
//                 x: x as f32 + (chunk.x * chunk_size.x as i32) as f32,
//                 y: y as f32,
//                 z: z as f32 + (chunk.y * chunk_size.z as i32) as f32,
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
    let default_size = cgmath::Vector3::new(1.0, 1.0, 1.0) * 25.0;
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
// pub fn instances_list_circle(chunk: Chunk, chunk_size: Vector2<u32>) -> Vec<Instance> {
//     let center = (chunk_size.x / 2, chunk_size.y / 2);
//     let radius = center.0 as i32;
//     (0..(chunk_size.x * chunk_size.y))
//         .map(move |n| {
//             let x = n % chunk_size.x;
//             let z = n / chunk_size.y;

//             let dx = x as i32 - center.0 as i32;
//             let dy = z as i32 - center.1 as i32;

//             let distance_squared = dx * dx + dy * dy;
//             let position = cgmath::Vector3 {
//                 x: x as f32 + (chunk.x * chunk_size.x as i32) as f32,
//                 y: 0.0,
//                 z: z as f32 + (chunk.y * chunk_size.y as i32) as f32,
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

//             if distance_squared > radius * radius
//                 || x == 0
//                 || x == radius as u32
//                 || z == 0
//                 || z == radius as u32
//             {
//                 Instance {
//                     index: n,
//                     position,
//                     rotation,
//                     scale: 0.5,
//                     should_render: false,
//                     color: default_color,
//                     size: default_size,
//                     bounding: default_bounding,
//                 }
//             } else {
//                 Instance {
//                     index: n,
//                     position,
//                     rotation,
//                     scale: 0.5,
//                     should_render: true,
//                     color: default_color,
//                     size: default_size,
//                     bounding: default_bounding,
//                 }
//             }
//         })
//         .collect::<Vec<_>>()
// }
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
