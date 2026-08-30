use std::{
    collections::HashMap,
    io::{BufReader, Cursor},
};

use ahash::AHashMap;

use crate::{
    application::graphics::Graphics,
    core::{
        geometry::{Primitive, PrimitiveVertex, Textured, TexturedVertex},
        object_loading::model::Model,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, TextureHandle},
    },
};

pub fn load_obj(
    obj_data: &[u8],
    mtl_data: Option<&[u8]>,
    gfx: &mut Graphics,
    textured_material_handle: Option<MaterialHandle>,
    primitive_material_handle: Option<MaterialHandle>,
    instance_handle: Option<InstanceControllerHandle>,
) -> Option<Model> {
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
    if let Ok(materials) = materials {
        for material in materials {
            let texture = gfx
                .texture(&material.name)
                .color(material.diffuse.unwrap_or_default())
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
            Some(*handle)
        } else {
            None
        };
        if let Some(primitive_material_handle) = primitive_material_handle {
            let mesh_handle = Primitive::try_from(model)
                .unwrap()
                .make_mb(gfx.get_render_context_mut());
            vertices.push((mesh_handle, texture_handle));
            let material = primitive_material_handle;
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
    Some(Model {
        meshes: vertices,
        instance: instance_handle,
        materials,
    })
}

impl TryFrom<tobj::Model> for Primitive {
    type Error = &'static str;

    fn try_from(model: tobj::Model) -> Result<Self, Self::Error> {
        let mesh = model.mesh;

        if !mesh.positions.len().is_multiple_of(3) {
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
impl TryFrom<tobj::Model> for Textured {
    type Error = &'static str;

    fn try_from(model: tobj::Model) -> Result<Self, Self::Error> {
        let mesh = model.mesh;

        if !mesh.positions.len().is_multiple_of(3) {
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
