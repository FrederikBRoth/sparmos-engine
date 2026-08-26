use ahash::{AHashMap, HashMap, HashMapExt};
use std::{
    io::{BufReader, Cursor},
    mem,
};
use wgpu::util::DeviceExt;

use crate::{
    
    application::graphics::Graphics,
    core::{
        render::{
            InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext, TextureHandle,
        },
        texture::Texture,
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

    pub fn build(mut self) -> Model {
        let model = Model::load_obj(
            self.data,
            self.mtl_data,
            &mut self.gfx,
            self.texture_material,
            self.primitive_material,
            self.instance,
        );
        model.unwrap()
    }
}

pub struct Model {
    pub meshes: Vec<(MeshHandle, Option<TextureHandle>)>,
    pub instance: InstanceControllerHandle,
    pub materials: HashMap<MeshHandle, MaterialHandle>,
}

#[derive(Debug)]
pub enum VertexType {
    Textured(Textured),
    Primitive(Primitive),
}

impl Model {
    pub fn load_obj(
        obj_data: &[u8],
        mtl_data: Option<&[u8]>,
        gfx: &mut Graphics,
        textured_material_handle: Option<MaterialHandle>,
        primitive_material_handle: Option<MaterialHandle>,
        instance_handle: Option<InstanceControllerHandle>,
    ) -> Option<Self> {
        let obj_cursor = Cursor::new(obj_data);
        let mut obj_reader = BufReader::new(obj_cursor);

        let (models, materials) = tobj::load_obj_buf(
            &mut obj_reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |_| {
                mtl_data
                    .map(|mtl| tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mtl))))
                    .unwrap_or_else(|| Ok((Vec::new(), AHashMap::new())))
            },
        )
        .unwrap();

        let mut vertices = Vec::<(MeshHandle, Option<TextureHandle>)>::new();

        let mut texture_list = Vec::<TextureHandle>::new();
        if let Some(materials) = materials.ok() {
            for material in materials {
                let texture = gfx
                    .texture(&material.name)
                    .color(material.diffuse.unwrap())
                    .build();
                let texture_handle = gfx
                    .engine
                    .render_context
                    .gpu_objects
                    .textures
                    .insert(texture);

                texture_list.push(texture_handle);
            }
        };

        let mut materials = HashMap::new();
        for model in models {
            println!(
                "name: {:?}, material_id: {:?}, vertices: {}, indices: {}",
                model.name,
                model.mesh.material_id,
                model.mesh.positions.len() / 3,
                model.mesh.indices.len(),
            );

            let texture_handle = if let Some(id) = model.mesh.material_id
                && let Some(handle) = texture_list.get(id)
            {
                Some(handle.clone())
            } else {
                None
            };
            if model.mesh.texcoords.is_empty() {
                let mesh_handle = Primitive::try_from(model)
                    .unwrap()
                    .make_mb(gfx.get_render_context_mut());
                vertices.push((mesh_handle, texture_handle));
                let material = primitive_material_handle.unwrap();
                materials.insert(mesh_handle, material);
            } else {
                let mesh_handle = Textured::try_from(model)
                    .unwrap()
                    .make_mb(gfx.get_render_context_mut());
                vertices.push((mesh_handle, texture_handle));
                let material = textured_material_handle.unwrap();
                materials.insert(mesh_handle, material);
            }
        }

        //creates default handle for instances:
        let instance_handle = if let Some(handle) = instance_handle {
            handle
        } else {
            gfx.instances().build()
        };
        Some(Self {
            meshes: vertices,
            instance: instance_handle,
            materials: materials,
        })
    }
}

impl TryFrom<tobj::Model> for Textured {
    type Error = &'static str;

    fn try_from(model: tobj::Model) -> Result<Self, Self::Error> {
        let mesh = model.mesh;

        if mesh.positions.len() % 3 != 0 {
            return Err("OBJ positions are not a multiple of 3");
        }

        let vertex_count = mesh.positions.len() / 3;

        if !mesh.normals.is_empty() && mesh.normals.len() != vertex_count * 3 {
            return Err("OBJ normals don't match position count");
        }

        if !mesh.texcoords.is_empty() && mesh.texcoords.len() != vertex_count * 2 {
            return Err("OBJ texcoords don't match position count");
        }

        let vertices = (0..vertex_count)
            .map(|i| {
                let position = [
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                ];

                let tex_coords = if mesh.texcoords.is_empty() {
                    [0.0, 0.0]
                } else {
                    [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]]
                };

                let normal = if mesh.normals.is_empty() {
                    [0.0, 0.0, 0.0]
                } else {
                    [
                        mesh.normals[i * 3],
                        mesh.normals[i * 3 + 1],
                        mesh.normals[i * 3 + 2],
                    ]
                };

                TexturedVertex {
                    position,
                    tex_coords,
                    normal,
                }
            })
            .collect();

        Ok(Self {
            vertices,
            indices: mesh.indices,
        })
    }
}

impl TryFrom<tobj::Model> for Primitive {
    type Error = &'static str;

    fn try_from(model: tobj::Model) -> Result<Self, Self::Error> {
        let mesh = model.mesh;

        if mesh.positions.len() % 3 != 0 {
            return Err("OBJ positions are not a multiple of 3");
        }

        let vertex_count = mesh.positions.len() / 3;

        if !mesh.normals.is_empty() && mesh.normals.len() != vertex_count * 3 {
            return Err("OBJ normals don't match position count");
        }

        let vertices = (0..vertex_count)
            .map(|i| {
                let position = [
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                ];

                let normal = if mesh.normals.is_empty() {
                    [0.0, 0.0, 0.0]
                } else {
                    [
                        mesh.normals[i * 3],
                        mesh.normals[i * 3 + 1],
                        mesh.normals[i * 3 + 2],
                    ]
                };

                PrimitiveVertex {
                    position,
                    color: [1.0, 1.0, 1.0],
                    normal,
                    quad_id: 0,
                }
            })
            .collect();

        Ok(Self {
            vertices,
            indices: mesh.indices,
        })
    }
}
