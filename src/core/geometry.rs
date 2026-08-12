use std::mem;
use wgpu::util::DeviceExt;

use crate::core::render::{MeshHandle, RenderContext};

pub trait Vertex {
    fn layout() -> VertexBufferLayoutOwned;
}
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct VertexAttributeKey {
    pub format: wgpu::VertexFormat,
    pub offset: u64,
    pub shader_location: u32,
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct VertexLayoutKey {
    pub array_stride: u64,
    pub step_mode: wgpu::VertexStepMode,
    pub attributes: Vec<VertexAttributeKey>,
}

#[derive(Clone)]
pub struct VertexBufferLayoutOwned {
    pub array_stride: u64,
    pub step_mode: wgpu::VertexStepMode,
    pub attributes: Vec<wgpu::VertexAttribute>,
}

impl VertexBufferLayoutOwned {
    pub fn to_wgpu<'a>(&'a self) -> Option<wgpu::VertexBufferLayout<'a>> {
        Some(wgpu::VertexBufferLayout {
            array_stride: self.array_stride,
            step_mode: self.step_mode,
            attributes: &self.attributes,
        })
    }

    pub fn key(&self) -> VertexLayoutKey {
        VertexLayoutKey {
            array_stride: self.array_stride,
            step_mode: self.step_mode,
            attributes: self
                .attributes
                .iter()
                .map(|a| VertexAttributeKey {
                    format: a.format,
                    offset: a.offset,
                    shader_location: a.shader_location,
                })
                .collect::<Vec<VertexAttributeKey>>(),
        }
    }
}

//Own vertex implementations. It is possible to create your own if you want
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PrimitiveVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
    pub quad_id: u32,
}
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TexturedVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
}

pub struct Primitive {
    // pub num_indices: u32,
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u32>,
}

impl Vertex for Primitive {
    fn layout() -> VertexBufferLayoutOwned {
        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<PrimitiveVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: vec![
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
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}
impl Primitive {
    pub fn make_mb(&self, rc: &mut RenderContext) -> MeshHandle {
        let mesh = Mesh::new(
            &rc.device,
            &self.vertices,
            &self.indices,
            self.vertices.len() as u32,
            self.indices.len() as u32,
        );

        rc.gpu_objects.meshes.insert(mesh)
    }
}

pub struct Textured {
    // pub num_indices: u32,
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u32>,
}

impl Vertex for Textured {
    fn layout() -> VertexBufferLayoutOwned {
        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<TexturedVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: vec![
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
impl Textured {
    pub fn make_mb(&self, device: &wgpu::Device) -> Mesh {
        let buffer_layout = Mesh::new(
            device,
            &self.vertices,
            &self.indices,
            self.vertices.len() as u32,
            self.indices.len() as u32,
        );
        buffer_layout
    }
}
pub struct Mesh {
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
}

impl Mesh {
    pub fn new<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        device: &wgpu::Device,
        vertices: &[T],
        indices: &[u32],
        vertex_count: u32,

        index_count: u32,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_count,
            index_count,
            vertex_buffer,
            index_buffer,
        }
    }

    // pub fn update_buffers(&mut self, device: &wgpu::Device, vertices: &[u8], indices: &Vec<u32>) {
    //     let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //         label: Some("Big Vertex Buffer"),
    //         contents: vertices,
    //         usage: wgpu::BufferUsages::VERTEX,
    //     });
    //
    //     let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //         label: Some("Big Index Buffer"),
    //         contents: bytemuck::cast_slice(indices),
    //         usage: wgpu::BufferUsages::INDEX,
    //     });
    //     self.vertex_buffer = vertex_buffer;
    //     self.index_buffer = index_buffer;
    // }
}
