use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

use wgpu::{ShaderModule, wgc::MAX_BIND_GROUPS};

use crate::{
    core::state::DeviceBackend,
    entity::{
        core::{geometry::Mesh, instance::InstanceController, material::Material, system::Systems},
        systems,
        texture::{Texture, TextureSampleView},
    },
};

pub struct GlobalRenderContext {
    pub depth_texture: TextureSampleView,
    pub shaders: HashMap<String, ShaderModule>,
    pub device: Arc<wgpu::Device>, // Logical GPU device
    pub queue: Arc<wgpu::Queue>,   // Command queue for GPU
    pub config: wgpu::SurfaceConfiguration,
}
impl GlobalRenderContext {
    pub fn add_shader(&mut self, device: &wgpu::Device, label: &str, shader_path: &str) {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(shader_path.into()),
        });
    }
}

impl<'a> DrawMesh for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, backend: &DeviceBackend, scene: &Scene, systems: &Systems) {
        let render_items = scene.to_render_items();
        let mut bind_group_id = 0;
        //binds all system bind groups
        for (name, bind_group) in systems.bind_groups.iter() {
            self.set_bind_group(bind_group_id, bind_group, &[]);
            bind_group_id += 1;
        }
        for render_item in render_items {
            self.set_pipeline(&render_item.material.pipeline);
            if let Some(texture) = &render_item.material.texture {
                self.set_bind_group(bind_group_id, &texture.bind_group, &[]);
            }

            self.set_vertex_buffer(0, render_item.mesh.vertex_buffer.slice(..));
            self.set_vertex_buffer(1, render_item.instance_controller.instance_buffer.slice(..));
            self.set_index_buffer(
                render_item.mesh.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );

            self.draw_indexed(
                0..render_item.mesh.index_count,
                0,
                0..render_item
                    .instance_controller
                    .atomic_usize
                    .load(Ordering::Relaxed) as u32,
            );
        }
    }
}

pub trait DrawMesh {
    #[allow(unused)]
    fn draw_scene(&mut self, backend: &DeviceBackend, scene: &Scene, systems: &Systems);
}

pub struct Scene {
    pub objects: Vec<RenderObject>,
}

impl Scene {
    pub fn new() -> Self {
        Scene { objects: vec![] }
    }
    pub fn to_render_items(&self) -> Vec<RenderItem> {
        let mut items = Vec::new();

        for object in &self.objects {
            items.push(RenderItem {
                mesh: &object.mesh,
                material: &object.material,
                instance_controller: &object.instance_controller,
            });
        }

        items
    }
}
//The two structs below might look identical, but the render item is useful for the render pipeline
//iterating through refences is faster if we need multi pass rendering for shados etc.
pub struct RenderObject {
    pub mesh: Mesh,
    pub instance_controller: InstanceController,
    pub material: Material,
}
//
pub struct RenderItem<'a> {
    mesh: &'a Mesh,
    instance_controller: &'a InstanceController,
    material: &'a Material,
}
