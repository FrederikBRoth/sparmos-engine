use std::{
    collections::HashMap,
    io::{BufReader, Cursor},
};

use ahash::AHashMap;

use crate::{
    application::graphics::Graphics,
    core::{
        geometry::{DefaultVertex, Vertex},
        models::model::Model,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, TextureHandle},
    },
};

pub fn load_obj(
    obj_data: &[u8],
    mtl_data: Option<&[u8]>,
    gfx: &mut Graphics,
    material_handle: MaterialHandle,
    instance_handle: Option<InstanceControllerHandle>,
) -> Option<Model> {
    let obj_cursor = Cursor::new(obj_data);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, loaded_materials) = tobj::load_obj_buf(
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
    .ok()?;

    // Create textures for materials found in the MTL.
    let mut texture_list = Vec::<TextureHandle>::new();

    if let Ok(loaded_materials) = loaded_materials {
        for material in loaded_materials {
            let texture = gfx
                .texture(&material.name)
                .color(material.diffuse.unwrap_or([1.0, 1.0, 1.0]))
                .build();

            let texture_handle = gfx
                .engine
                .render_context
                .gpu_objects
                .textures
                .insert(texture);

            texture_list.push(texture_handle);
        }
    }

    // Shared fallback for meshes without an OBJ material.
    let white_texture = gfx
        .texture("obj_default_white")
        .color([1.0, 1.0, 1.0])
        .build();

    let white_texture_handle = gfx
        .engine
        .render_context
        .gpu_objects
        .textures
        .insert(white_texture);

    let mut meshes = Vec::<(MeshHandle, Option<TextureHandle>)>::new();
    let mut materials = HashMap::new();

    for model in models {
        println!(
            "name: {:?}, material_id: {:?}, vertices: {}, indices: {}",
            model.name,
            model.mesh.material_id,
            model.mesh.positions.len() / 3,
            model.mesh.indices.len(),
        );

        let texture_handle = model
            .mesh
            .material_id
            .and_then(|id| texture_list.get(id).copied())
            .unwrap_or(white_texture_handle);

        let mesh_handle = Vertex::try_from(model)
            .ok()?
            .make_mb(gfx.get_render_context_mut());

        let texture = gfx.engine.render_context.gpu_objects.textures[texture_handle].clone();

        let mesh_material = gfx.material_with_texture(material_handle, &texture, 1, 0);

        meshes.push((mesh_handle, Some(texture_handle)));
        materials.insert(mesh_handle, mesh_material);
    }

    let instance_handle = instance_handle.unwrap_or_else(|| gfx.instances().build());

    Some(Model {
        meshes,
        instance: instance_handle,
        materials,
    })
}

impl TryFrom<tobj::Model> for Vertex {
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

                DefaultVertex {
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
