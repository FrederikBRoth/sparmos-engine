use std::collections::HashMap;

use wgpu::Device;

use crate::{
    application::graphics::Graphics,
    core::{
        object_loading::{self, obj::load_obj},
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, TextureHandle},
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
        textured_material_handle: Option<MaterialHandle>,
        primitive_material_handle: Option<MaterialHandle>,
        instance_handle: Option<InstanceControllerHandle>,
    ) -> Option<Self> {
        object_loading::obj::load_obj(
            obj_data,
            mtl_data,
            gfx,
            textured_material_handle,
            primitive_material_handle,
            instance_handle,
        )
    }

    pub fn load_gltf(
        gfx: &mut Graphics,
        data: &[u8],
        instance_handle: InstanceControllerHandle,
        material: MaterialHandle,
    ) -> Self {
        object_loading::gltf::load_gltf(gfx, data, instance_handle, material)
    }

    pub fn materials(&self) -> &HashMap<MeshHandle, MaterialHandle> {
        &self.materials
    }
}
