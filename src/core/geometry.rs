use std::mem;
use wgpu::util::DeviceExt;

use crate::{
    application::graphics::Graphics,
    core::{
        object_loading::model::Model,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext},
    },
};

pub trait Vertex {
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PbrVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
}

#[derive(Debug)]
pub struct Skybox {
    pub vertices: Vec<SkyboxVertex>,
    pub indices: Vec<u32>,
}
impl Vertex for Skybox {
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
pub struct Primitive {
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

#[derive(Debug)]
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
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}
impl Textured {
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
    pub(crate) texture_material: Option<MaterialHandle>,
    pub(crate) primitive_material: Option<MaterialHandle>,
    pub(crate) instance: Option<InstanceControllerHandle>,
}

impl<'a> ModelBuilder<'a> {
    pub(crate) fn new(gfx: &'a mut Graphics) -> Self {
        ModelBuilder {
            gfx,
            data: &[],
            mtl_data: None,
            texture_material: None,
            primitive_material: None,
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

    pub fn texture_pipeline(mut self, handle: MaterialHandle) -> Self {
        self.texture_material = Some(handle);
        self
    }
    pub fn primitive_pipeline(mut self, handle: MaterialHandle) -> Self {
        self.primitive_material = Some(handle);
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
            self.texture_material,
            self.primitive_material,
            self.instance,
        );
        model.unwrap()
    }
}

#[derive(Debug)]
pub enum VertexType {
    Textured(Textured),
    Primitive(Primitive),
}
