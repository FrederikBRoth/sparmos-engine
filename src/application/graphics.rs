use std::{cell::RefCell, mem, rc::Rc, sync::Arc, time::Duration};

use cgmath::Vector3;
use hecs::{DynamicBundle, Entity, Query, QueryBorrow};
use indexmap::IndexMap;
use wgpu::{Device, Queue};

use crate::{
    core::{
        buffer::Buffer,
        engine::{
            Engine,
            EngineCommandQueue::{self, AddEntity},
            System,
        },
        entities::World,
        geometry::{ModelBuilder, Skybox, Vertex},
        instance::{DefaultInstanceLayout, InstanceBuilder, RawInstance},
        pipelines::{
            ComputeRenderingBuilder, MaterialBuilder, PipelineConfig, RenderPipelineBuilder,
        },
        render::{ComputeHandle, MaterialHandle, RenderContext, SkyboxRenderable},
        resource::BufferHandle,
        texture::{PbrTextureBuilder, Texture, TextureBuilder},
    },
    entities::meshes::Meshes,
    systems::compute::{ComputeBuilder, ReadbackState},
};

pub struct Graphics {
    pub(crate) world: Rc<RefCell<World>>,
    pub engine: Engine,
}

//Main API access to all functions required for rendering objects
impl Graphics {
    pub(crate) fn run_all_systems(&mut self) {
        self.engine.systems.run_all(
            &mut self.world,
            &mut self.engine.render_context,
            self.engine.engine_time.dt(),
        );
    }
    pub fn get_world(&self) -> Rc<RefCell<World>> {
        Rc::clone(&self.world)
    }
    pub fn shader(&mut self, label: &str, shader_path: &str) {
        self.engine.render_context.add_shader(label, shader_path);
    }
    pub fn change_shader(&mut self, material: &MaterialHandle, shader: &str) {
        self.engine
            .render_commands
            .push(EngineCommandQueue::ChangeShader(
                *material,
                shader.to_string(),
            ));
    }

    /// Returns a reference to the get device of this [`Graphics`].
    pub(crate) fn get_device(&self) -> &Arc<Device> {
        &self.engine.render_context.device
    }
    /// Returns a reference to the get queue of this [`Graphics`].
    pub(crate) fn get_queue(&self) -> &Arc<Queue> {
        &self.engine.render_context.queue
    }

    /// Returns a mutable reference to the get device of this [`Graphics`].
    pub(crate) fn get_device_mut(&mut self) -> &mut Arc<Device> {
        &mut self.engine.render_context.device
    }
    /// Returns a mutable reference to the get queue of this [`Graphics`].
    #[allow(unused)]
    pub(crate) fn get_queue_mut(&mut self) -> &mut Arc<Queue> {
        &mut self.engine.render_context.queue
    }

    pub(crate) fn get_render_context_mut(&mut self) -> &mut RenderContext {
        &mut self.engine.render_context
    }
    pub(crate) fn get_render_context(&self) -> &RenderContext {
        &self.engine.render_context
    }

    /// Returns the material of this [`Graphics`].
    pub fn material<V: Vertex, I: RawInstance>(&mut self) -> MaterialBuilder<'_> {
        MaterialBuilder {
            graphics: self,
            buffers: IndexMap::new(),
            textures: vec![],
            shader: String::new(),
            vertex_layout: V::layout(),
            instance_layout: I::layout(),
            compute_render_buffer: None,
            config: PipelineConfig::default(),
        }
    }

    pub fn pipeline<'a>(&'a self, label: &str) -> RenderPipelineBuilder<'a> {
        RenderPipelineBuilder::new(&self.engine.render_context, label)
    }
    pub fn instances(&mut self) -> InstanceBuilder<'_, DefaultInstanceLayout> {
        self.instances_typed::<DefaultInstanceLayout>()
    }

    pub fn instances_typed<I: RawInstance>(&mut self) -> InstanceBuilder<'_, I> {
        InstanceBuilder::<I> {
            gfx: self,
            origin: Vector3::new(0.0, 0.0, 0.0),
            global_size: 1.0,
            template: None,
            phantom_data: Default::default(),
            instances: vec![],
        }
    }

    pub fn model(&mut self) -> ModelBuilder<'_> {
        ModelBuilder::new(self)
    }

    pub fn compute<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        &mut self,
    ) -> ComputeBuilder<'_> {
        let output_size = mem::size_of::<T>();
        ComputeBuilder {
            gfx: self,
            output_object_size: output_size,
            size: 0,
            input_buffers: vec![],
            shader: String::new(),
            readback: ReadbackState::NoReadback,
            initial_data: None,
        }
    }

    pub fn compute_rendering(&mut self, compute: ComputeHandle) -> ComputeRenderingBuilder<'_> {
        ComputeRenderingBuilder::new(self, compute)
    }

    pub fn add_system<T: System + 'static>(&mut self, system: T) {
        self.engine.systems.add(system);
    }

    pub fn add_entity<B: DynamicBundle + 'static>(&mut self, bundle: B) -> Entity {
        let world = Rc::clone(&self.world);
        if let Ok(mut world) = world.try_borrow_mut() {
            world.add_entity(bundle)
        } else {
            let entity = world.borrow().entities.reserve_entity();
            let entity_clone = entity;
            let command = AddEntity(Box::new(move |world| {
                world.insert(entity_clone, bundle).unwrap();
            }));
            self.engine.render_commands.push(command);
            entity
        }
    }

    #[allow(unused)]
    pub(crate) fn get_bind_group_layouts(&self) -> Vec<Option<&wgpu::BindGroupLayout>> {
        self.engine.systems.get_bind_group_layouts()
    }

    pub fn entity_query_first<B: Query>(&self, f: impl for<'a> FnOnce(<B as Query>::Item<'a>)) {
        let world = &self.world.borrow();

        world.query_first(f);
    }

    // pub fn query_first_with_resources<B: Query>(
    //     &mut self,
    //     f: impl for<'a> FnOnce(&mut Resources, <B as Query>::Item<'a>),
    // ) where
    //     B: Query,
    // {
    //     let world = &mut self.entities;
    //
    //     let mut query = world.query::<B>();
    //
    //     if let Some(item) = query.iter().next() {
    //         f(&mut self.resources, item);
    //     }
    // }
    pub fn entity_query<B: Query>(&self, f: impl for<'a> FnOnce(QueryBorrow<'a, B>)) {
        let world = &self.world.borrow();

        world.query(f);
    }

    pub fn dt(&self) -> Duration {
        self.engine.engine_time.dt()
    }

    pub fn get_buffer(&self, handle: BufferHandle) -> &Buffer {
        self.engine.resources.buffers.get(handle).unwrap()
    }

    pub fn get_buffer_by_register(&self, name: &str) -> BufferHandle {
        self.engine.resources.named_buffers[name]
    }

    pub fn update_buffer<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        &mut self,
        handle: BufferHandle,
        data: &[T],
    ) {
        let buffer = self.get_buffer(handle);
        buffer.update(self.get_queue(), data);
    }

    pub fn register_buffer(&mut self, buffer: Buffer, name: &str) -> BufferHandle {
        let map = &mut self.engine.resources.named_buffers;
        if !map.contains_key(name) {
            let handle = self.engine.resources.buffers.insert(buffer);
            map.insert(name.to_string(), handle);
            handle
        } else {
            map[name]
        }
    }

    pub fn texture<'a>(&'a mut self, label: &'a str) -> TextureBuilder<'a> {
        TextureBuilder::new(self, label)
    }

    pub fn pbr_texture<'a>(&'a mut self, label: &'a str) -> PbrTextureBuilder<'a> {
        PbrTextureBuilder::new(self, label)
    }

    pub fn add_skybox(&mut self, skybox_texture: Texture) {
        let skybox_mesh = Meshes::create_skybox().make_mb(self.get_render_context_mut());
        let skybox_pipeline = self
            .material::<Skybox, DefaultInstanceLayout>()
            .shader("skybox")
            .config(PipelineConfig {
                culling: None,
                depth_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                target_format: None,
            })
            .texture(skybox_texture)
            .build();

        let skybox_renderable = SkyboxRenderable {
            material_handle: skybox_pipeline,
            mesh_handle: skybox_mesh,
        };
        self.add_entity((skybox_renderable,));
    }
}

pub enum Markers {
    Skybox,
}
