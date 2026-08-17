use std::{collections::HashMap, sync::Arc};

use slotmap::{SlotMap, new_key_type};
use wgpu::ShaderModule;

use crate::{
    application::state::DeviceBackend,
    core::{
        engine::Engine,
        entities::World,
        geometry::{Mesh, Model},
        instance::InstanceControllerTrait,
        pipelines::{ComputeRendering, ComputeRenderingKey, Material, MaterialKey},
        post_processing::PostProcessHandler,
        texture::{self, Texture, TextureSampleView},
    },
};

pub struct RenderContext {
    pub depth_texture: TextureSampleView,
    pub overscan_depth_texture: TextureSampleView,
    pub shaders: HashMap<String, ShaderModule>,
    pub device: Arc<wgpu::Device>, // Logical GPU device
    pub queue: Arc<wgpu::Queue>,   // Command queue for GPU
    pub config: wgpu::SurfaceConfiguration,
    pub gpu_objects: GpuObjects,
    pub post_processing: PostProcessHandler,
}
impl RenderContext {
    pub fn add_shader(&mut self, label: &str, shader_path: &str) {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(shader_path.into()),
            });

        self.shaders.insert(label.to_string(), shader);
    }
}

pub struct ComputeRenderable {
    pub rendering_handle: ComputeRenderingHandle,
    pub mesh_handle: MeshHandle,
}

pub struct Renderable {
    pub material_handle: MaterialHandle,
    pub mesh_handle: MeshHandle,
    pub instance_controller_handle: InstanceControllerHandle,
}

impl<'a> DrawMesh for wgpu::RenderPass<'a> {
    fn draw_scene(&mut self, _backend: &DeviceBackend, engine: &Engine, world: &World) {
        let scene = &engine.render_context.gpu_objects;
        let mut bind_group_id = 0;
        for (_name, bind_group) in world.resources.bind_groups.iter() {
            self.set_bind_group(bind_group_id, bind_group, &[]);
            bind_group_id += 1;
        }

        let pre_id = bind_group_id;

        for model in world.entities.query::<&Model>().iter() {
            let material = &scene.materials[model.material];
            let instance_controller = &scene.instance_controllers[model.instance];

            self.set_pipeline(&material.pipeline);
            for (mesh, texture) in model.meshes.iter().cloned() {
                bind_group_id = pre_id;

                let mesh = &scene.meshes[mesh];
                if let Some(texture_handle) = texture {
                    let texture = &scene.textures[texture_handle.clone()];
                    self.set_bind_group(bind_group_id, &texture.bind_group, &[]);
                    bind_group_id += 1;
                }

                for buffers in &material.buffers {
                    self.set_bind_group(bind_group_id, &buffers.1.bind_group, &[]);
                    bind_group_id += 1;
                }

                if let Some(compute_layout) = &material.compute_bind_group {
                    self.set_bind_group(bind_group_id, compute_layout, &[]);
                }
                self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                self.set_vertex_buffer(1, instance_controller.buffer().slice(..));
                self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                self.draw_indexed(
                    0..mesh.index_count,
                    0,
                    0..instance_controller.count() as u32,
                );
            }
        }
        let mut bind_group_id = 0;
        for (_name, bind_group) in world.resources.bind_groups.iter() {
            self.set_bind_group(bind_group_id, bind_group, &[]);
            bind_group_id += 1;
        }
        for renderable in world.entities.query::<&Renderable>().iter() {
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
                bind_group_id += 1;
            }
            if let Some(compute_layout) = &material.compute_bind_group {
                self.set_bind_group(bind_group_id, compute_layout, &[]);
            }

            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_vertex_buffer(1, instance_controller.buffer().slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            self.draw_indexed(
                0..mesh.index_count,
                0,
                0..instance_controller.count() as u32,
            );
        }

        let mut bind_group_id = 0;
        for (_name, bind_group) in world.resources.bind_groups.iter() {
            self.set_bind_group(bind_group_id, bind_group, &[]);
            bind_group_id += 1;
        }
        for renderable in world.entities.query::<&ComputeRenderable>().iter() {
            let rendering = &scene.compute_renderings[renderable.rendering_handle];
            let mesh = &scene.meshes[renderable.mesh_handle];
            self.set_pipeline(&rendering.pipeline);

            // Bind engine/system bind groups
            for buffer in &rendering.input_buffers {
                self.set_bind_group(bind_group_id, &buffer.bind_group, &[]);
                bind_group_id += 1;
            }

            self.set_bind_group(bind_group_id, &rendering.compute_bind_group, &[]);
            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            self.draw_indexed(0..mesh.index_count, 0, 0..rendering.length);
            // Bind compute output buffer

            // self.draw(0..renderable.vertex_count, 0..renderable.instance_count);
        }
    }
}

pub trait DrawMesh {
    #[allow(unused)]
    fn draw_scene(&mut self, backend: &DeviceBackend, engine: &Engine, world: &World);
}
new_key_type! { pub struct MeshHandle; }
new_key_type! { pub struct MaterialHandle; }
new_key_type! { pub struct TextureHandle; }

new_key_type! { pub struct ComputeHandle; }
new_key_type! { pub struct InstanceControllerHandle; }
new_key_type! {
    pub struct ComputeRenderingHandle;
}

pub struct GpuObjects {
    pub instance_controllers: SlotMap<InstanceControllerHandle, Box<dyn InstanceControllerTrait>>,
    pub meshes: SlotMap<MeshHandle, Mesh>,
    pub textures: SlotMap<TextureHandle, Texture>,
    pub materials: SlotMap<MaterialHandle, Material>,
    pub material_lookup: HashMap<MaterialKey, MaterialHandle>,
    pub compute_renderings: SlotMap<ComputeRenderingHandle, ComputeRendering>,

    pub compute_rendering_lookup: HashMap<ComputeRenderingKey, ComputeRenderingHandle>,
}

impl GpuObjects {
    pub fn insert_ic(&mut self, ic: Box<dyn InstanceControllerTrait>) -> InstanceControllerHandle {
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
            textures: SlotMap::with_key(),
            material_lookup: HashMap::new(),

            compute_renderings: SlotMap::with_key(),
            compute_rendering_lookup: HashMap::new(),
        }
    }

    pub fn get_material(&mut self, key: &MaterialKey) -> Option<MaterialHandle> {
        self.material_lookup.get(key).copied()
    }
    pub fn get_compute_rendering(
        &mut self,
        key: &ComputeRenderingKey,
    ) -> Option<ComputeRenderingHandle> {
        self.compute_rendering_lookup.get(key).copied()
    }
}
