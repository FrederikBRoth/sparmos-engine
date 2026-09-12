use std::collections::HashMap;

use crate::{
    application::graphics::Graphics,
    core::{
        models::{self},
        render::render::{InstanceControllerHandle, MaterialHandle, MeshHandle, TextureHandle},
    },
};

pub struct Model {
    pub meshes: Vec<(MeshHandle, Option<TextureHandle>)>,
    pub instance: InstanceControllerHandle,
    pub materials: HashMap<MeshHandle, MaterialHandle>,
}

impl Model {
    pub fn load_obj(
        obj_data: &[u8],
        mtl_data: Option<&[u8]>,
        gfx: &mut Graphics,
        material_handle: MaterialHandle,
        instance_handle: Option<InstanceControllerHandle>,
    ) -> Option<Self> {
        models::obj::load_obj(obj_data, mtl_data, gfx, material_handle, instance_handle)
    }

    pub fn load_glb(
        gfx: &mut Graphics,
        data: &[u8],
        instance_handle: InstanceControllerHandle,
        material: MaterialHandle,
    ) -> Self {
        models::gltf::load_gltf(gfx, data, instance_handle, material)
    }

    pub fn materials(&self) -> &HashMap<MeshHandle, MaterialHandle> {
        &self.materials
    }
}
