use std::collections::HashMap;

use gltf::{Document, Material, buffer::Data};
use wgpu::Device;

use crate::{
    application::graphics::Graphics,
    core::{
        geometry::{Mesh, TexturedVertex},
        object_loading::model::Model,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, TextureHandle},
    },
};

struct MeshSpec<'a> {
    mesh: Mesh,
    material: gltf::Material<'a>,
}

pub fn load_gltf(
    gfx: &mut Graphics,
    data: &[u8],
    instance_handle: InstanceControllerHandle,
    material: MaterialHandle,
) -> Model {
    let (spec, buffer_data, image_data) =
        gltf::import_slice(data).expect("GLTF object not imported correctly");
    println!("{:?}", spec.buffers().len());

    for image_data in image_data {
        println!(
            "Width: {:?}, height: {:?}",
            image_data.width, image_data.height
        );
    }

    let meshes = load_meshes(gfx.get_device(), &spec, &buffer_data);
    let mut mesh_handles = vec![];
    for mesh in meshes {
        mesh_handles.push(gfx.get_render_context_mut().gpu_objects.meshes.insert(mesh));
    }

    let meshes = mesh_handles
        .iter()
        .map(|mesh| (mesh.clone(), None))
        .collect::<Vec<(MeshHandle, Option<TextureHandle>)>>();

    let mut materials = HashMap::new();

    for mesh in mesh_handles {
        materials.insert(mesh, material);
    }
    Model {
        meshes,
        instance: instance_handle,
        materials,
    }

    // println!("{:?}", import);
}

fn load_meshes<'a>(device: &Device, document: &'a Document, buffer_data: &'a [Data]) -> Vec<Mesh> {
    let mut meshes: Vec<Mesh> = vec![];
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));
            let positions = reader.read_positions().unwrap();

            let normals = reader.read_normals().unwrap();

            let tex_coords = reader.read_tex_coords(0).unwrap().into_f32();

            let indices = reader
                .read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<u32>>();
            let vertices: Vec<TexturedVertex> = positions
                .into_iter()
                .zip(normals)
                .zip(tex_coords)
                .map(|((position, normal), tex_coord)| TexturedVertex {
                    position,
                    tex_coords: tex_coord,
                    normal,
                })
                .collect();
            let mesh = Mesh::new(
                device,
                &vertices,
                &indices,
                vertices.len() as u32,
                indices.len() as u32,
            );

            meshes.push(mesh);
        }
    }

    meshes
}

// fn load_material()
//
// fn buffer_data(accessor: )
