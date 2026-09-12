use std::{
    hash::{Hash, Hasher},
    mem,
};
use wgpu::util::DeviceExt;

use crate::{
    application::graphics::Graphics,
    core::{
        models::model::Model,
        render::render::{InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext},
    },
};

pub trait VertexType {
    fn layout() -> VertexBufferLayoutOwned;
}
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct VertexAttributeKey {
    pub format: wgpu::VertexFormat,
    pub offset: u64,
    pub shader_location: u32,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
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
pub struct SkyboxVertex {
    pub position: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DefaultVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
}

#[derive(Debug)]
pub struct Skybox {
    pub vertices: Vec<SkyboxVertex>,
    pub indices: Vec<u32>,
}
impl VertexType for Skybox {
    fn layout() -> VertexBufferLayoutOwned {
        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<SkyboxVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: vec![wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}
impl Skybox {
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

#[derive(Debug)]
pub struct Vertex {
    // pub num_indices: u32,
    pub vertices: Vec<DefaultVertex>,
    pub indices: Vec<u32>,
}

impl VertexType for Vertex {
    fn layout() -> VertexBufferLayoutOwned {
        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<DefaultVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}
impl Vertex {
    pub fn make_mb(&self, rc: &mut RenderContext) -> MeshHandle {
        let key = MeshKey::new(&self.vertices, &self.indices);
        if let Some(handle) = rc.gpu_objects.mesh_lookup.get(&key) {
            return *handle;
        }

        let mesh = Mesh::new(
            &rc.device,
            &self.vertices,
            &self.indices,
            self.vertices.len() as u32,
            self.indices.len() as u32,
        );
        let handle = rc.gpu_objects.meshes.insert(mesh);
        rc.gpu_objects.mesh_lookup.insert(key, handle);
        handle
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshKey {
    vertex_hash: u64,
    index_hash: u64,
    vertex_stride: usize,
    vertex_count: u32,
    index_count: u32,
}

impl MeshKey {
    pub fn new<T: bytemuck::Pod>(vertices: &[T], indices: &[u32]) -> Self {
        Self::from_bytes(
            bytemuck::cast_slice(vertices),
            bytemuck::cast_slice(indices),
            std::mem::size_of::<T>(),
            vertices.len() as u32,
            indices.len() as u32,
        )
    }

    pub fn from_bytes(
        vertices: &[u8],
        indices: &[u8],
        vertex_stride: usize,
        vertex_count: u32,
        index_count: u32,
    ) -> Self {
        Self {
            vertex_hash: hash_bytes(vertices),
            index_hash: hash_bytes(indices),
            vertex_stride,
            vertex_count,
            index_count,
        }
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
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
        Mesh::new_from_bytes(
            device,
            bytemuck::cast_slice(vertices),
            bytemuck::cast_slice(indices),
            vertex_count,
            index_count,
        )
    }

    pub fn new_from_bytes(
        device: &wgpu::Device,
        vertices: &[u8],
        indices: &[u8],
        vertex_count: u32,
        index_count: u32,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Vertex Buffer"),
            contents: vertices,
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Big Index Buffer"),
            contents: indices,
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_count,
            index_count,
            vertex_buffer,
            index_buffer,
        }
    }
}

//Models can contain multiple meshes each with (potentially)
pub struct ModelBuilder<'a> {
    pub(crate) gfx: &'a mut Graphics,
    pub(crate) data: &'a [u8],
    pub(crate) mtl_data: Option<&'a [u8]>,
    pub(crate) material: Option<MaterialHandle>,
    pub(crate) instance: Option<InstanceControllerHandle>,
}

impl<'a> ModelBuilder<'a> {
    pub(crate) fn new(gfx: &'a mut Graphics) -> Self {
        ModelBuilder {
            gfx,
            data: &[],
            mtl_data: None,
            material: None,
            instance: None,
        }
    }

    pub fn model(mut self, data: &'a [u8]) -> Self {
        self.data = data;
        self
    }

    pub fn material(mut self, data: &'a [u8]) -> Self {
        self.mtl_data = Some(data);
        self
    }

    pub fn pipeline(mut self, handle: MaterialHandle) -> Self {
        self.material = Some(handle);
        self
    }

    pub fn instances(mut self, instance: InstanceControllerHandle) -> Self {
        self.instance = Some(instance);
        self
    }

    pub fn build(self) -> Model {
        let model = Model::load_obj(
            self.data,
            self.mtl_data,
            self.gfx,
            self.material.unwrap(),
            self.instance,
        );
        model.unwrap()
    }
}
