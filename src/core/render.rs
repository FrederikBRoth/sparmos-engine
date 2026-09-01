use std::{collections::HashMap, sync::Arc};

use slotmap::{SlotMap, new_key_type};
use wgpu::ShaderModule;

use crate::{
    application::state::DeviceBackend,
    core::{
        engine::Engine,
        entities::World,
        geometry::Mesh,
        instance::InstanceControllerTrait,
        object_loading::model::Model,
        pipelines::{ComputeRendering, ComputeRenderingKey, Material, MaterialKey},
        post_processing::PostProcessHandler,
        texture::{Texture, TextureDepth},
    },
    systems::compute::Compute,
};

pub struct RenderContext {
    pub(crate) depth_texture: TextureDepth,
    pub(crate) overscan_depth_texture: TextureDepth,
    pub shaders: HashMap<String, ShaderModule>,
    pub device: Arc<wgpu::Device>, // Logical GPU device
    pub queue: Arc<wgpu::Queue>,   // Command queue for GPU
    pub config: wgpu::SurfaceConfiguration,
    pub rgba16float_renderable: bool,
    pub rg16float_renderable: bool,
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

pub struct SkyboxRenderable {
    pub material_handle: MaterialHandle,
    pub mesh_handle: MeshHandle,
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
        world.query::<&Model>(|mut query| {
            for model in query.iter() {
                let instance_controller = &scene.instance_controllers[model.instance];

                for (mesh, _) in model.meshes.iter().cloned() {
                    let material = &scene.materials[model.materials[&mesh]];
                    self.set_pipeline(&material.pipeline);
                    let mesh = &scene.meshes[mesh];
                    for (group, bind_group) in material.bind_groups.iter().enumerate() {
                        if let Some(bind_group) = bind_group {
                            self.set_bind_group(group as u32, bind_group, &[]);
                        }
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
        });
        world.query::<&Renderable>(|mut query| {
            for renderable in query.iter() {
                let mesh = &scene.meshes[renderable.mesh_handle];
                let material = &scene.materials[renderable.material_handle];
                let instance_controller =
                    &scene.instance_controllers[renderable.instance_controller_handle];
                self.set_pipeline(&material.pipeline);
                for (group, bind_group) in material.bind_groups.iter().enumerate() {
                    if let Some(bind_group) = bind_group {
                        self.set_bind_group(group as u32, bind_group, &[]);
                    }
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
        });

        world.query::<&ComputeRenderable>(|mut query| {
            for renderable in query.iter() {
                let rendering = &scene.compute_renderings[renderable.rendering_handle];
                let mesh = &scene.meshes[renderable.mesh_handle];
                self.set_pipeline(&rendering.pipeline);
                for (group, bind_group) in rendering.bind_groups.iter().enumerate() {
                    if let Some(bind_group) = bind_group {
                        self.set_bind_group(group as u32, bind_group, &[]);
                    }
                }
                self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                self.draw_indexed(0..mesh.index_count, 0, 0..rendering.length);
            }
        });

        world.query_first::<&mut SkyboxRenderable>(|skybox| {
            let material = &scene.materials[skybox.material_handle];
            let mesh = &scene.meshes[skybox.mesh_handle];
            self.set_pipeline(&material.pipeline);
            for (group, bind_group) in material.bind_groups.iter().enumerate() {
                if let Some(bind_group) = bind_group {
                    self.set_bind_group(group as u32, bind_group, &[]);
                }
            }

            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            self.draw_indexed(0..mesh.index_count, 0, 0..1);
        });
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
    pub computes: SlotMap<ComputeHandle, Compute>,
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
            computes: SlotMap::with_key(),
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

    pub fn get_compute_mut(&mut self, handle: ComputeHandle) -> Option<&mut Compute> {
        self.computes.get_mut(handle)
    }

    pub fn add_compute(&mut self, compute: Compute) -> ComputeHandle {
        self.computes.insert(compute)
    }
}
