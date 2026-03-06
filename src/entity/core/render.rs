use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

use slotmap::{SlotMap, new_key_type};
use wgpu::ShaderModule;

use crate::{
    application::state::{Core, DeviceBackend},
    entity::{
        core::{
            engine::Engine,
            geometry::Mesh,
            instance::{InstanceController, InstanceToRaw},
            material::Material,
        },
        texture::TextureSampleView,
    },
};

pub struct RenderContext {
    pub depth_texture: TextureSampleView,
    pub shaders: HashMap<String, ShaderModule>,
    pub device: Arc<wgpu::Device>, // Logical GPU device
    pub queue: Arc<wgpu::Queue>,   // Command queue for GPU
    pub config: wgpu::SurfaceConfiguration,
    pub gpu_objects: GpuObjects,
}
impl RenderContext {
    pub fn add_shader(&mut self, device: &wgpu::Device, label: &str, shader_path: &str) {
        let _ = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(shader_path.into()),
        });
    }
}

pub struct Renderable {
    pub material_handle: MaterialHandle,
    pub mesh_handle: MeshHandle,
    pub instance_controller_handle: InstanceControllerHandle,
}

impl<'a> DrawMesh for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, _backend: &DeviceBackend, core: &Core) {
        let engine = &core.engine;
        let scene = &core.render_context.gpu_objects;
        let mut bind_group_id = 0;
        for (_name, bind_group) in engine.resources.bind_groups.iter() {
            self.set_bind_group(bind_group_id, bind_group, &[]);
            bind_group_id += 1;
        }

        for (renderable) in engine.world.query::<&Renderable>().iter() {
            let mesh = &scene.meshes[renderable.mesh_handle];
            let material = &scene.materials[renderable.material_handle];
            let instance_controller =
                &scene.instance_controllers[renderable.instance_controller_handle];
            //binds all system bind groups
            self.set_pipeline(&material.pipeline);
            if let Some(texture) = &material.texture {
                self.set_bind_group(bind_group_id, &texture.bind_group, &[]);
                bind_group_id += 1;
            }

            for buffers in &material.buffers {
                self.set_bind_group(bind_group_id, &buffers.1.bind_group, &[]);
            }

            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_vertex_buffer(1, instance_controller.instance_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            self.draw_indexed(
                0..mesh.index_count,
                0,
                0..instance_controller.atomic_usize.load(Ordering::Relaxed) as u32,
            );
        }
    }
}

pub trait DrawMesh {
    #[allow(unused)]
    fn draw_scene(&mut self, backend: &DeviceBackend, core: &Core);
}
new_key_type! { pub struct MeshHandle; }
new_key_type! { pub struct MaterialHandle; }
new_key_type! { pub struct InstanceControllerHandle; }

pub struct GpuObjects {
    pub instance_controllers: SlotMap<InstanceControllerHandle, InstanceController>,
    pub materials: SlotMap<MaterialHandle, Material>,
    pub meshes: SlotMap<MeshHandle, Mesh>,
}

impl GpuObjects {
    pub fn insert_ic(&mut self, ic: InstanceController) -> InstanceControllerHandle {
        self.instance_controllers.insert(ic)
    }
}

impl Default for GpuObjects {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuObjects {
    pub fn new() -> Self {
        GpuObjects {
            instance_controllers: SlotMap::with_key(),
            materials: SlotMap::with_key(),
            meshes: SlotMap::with_key(),
        }
    }
}
//The two structs below might look identical, but the render item is useful for the render pipeline
//iterating through refences is faster if we need multi pass rendering for shados etc.
//
pub struct RenderItem<'a> {
    mesh: &'a Mesh,
    instance_controller: &'a InstanceController,
    material: &'a Material,
}
