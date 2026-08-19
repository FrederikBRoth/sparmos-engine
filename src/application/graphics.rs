use std::{any::TypeId, cell::RefCell, mem, rc::Rc, sync::Arc, time::Duration};

use cgmath::Vector3;
use hecs::{DynamicBundle, Entity, Query, QueryBorrow};
use indexmap::IndexMap;
use wgpu::{Device, Queue};

use crate::{
    core::{
        engine::{
            Engine,
            EngineCommandQueue::{self, AddEntity},
        },
        entities::World,
        geometry::Vertex,
        instance::{InstanceBuilder, RawInstance},
        pipelines::{ComputeRenderingBuilder, MaterialBuilder},
        render::{ComputeHandle, MaterialHandle, RenderContext},
        resource::System,
    },
    systems::compute::{ComputeBuilder, ReadbackState},
};

pub struct Graphics {
    pub(crate) world: Rc<RefCell<World>>,
    pub engine: Engine,
}

impl Graphics {
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
            texture: None,
            shader: String::new(),
            vertex_layout: V::layout(),
            instance_layout: I::layout(),
            compute_render_buffer: None,
        }
    }

    pub fn instances<I: RawInstance>(&mut self) -> InstanceBuilder<'_, I> {
        InstanceBuilder::<I> {
            gfx: self,
            origin: Vector3::new(0.0, 0.0, 0.0),
            template: None,
            phantom_data: Default::default(),
            instances: vec![],
        }
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
        system.register(&mut self.engine.resources);
    }

    pub fn add_entity<B: DynamicBundle + 'static>(&mut self, bundle: B) -> Entity {
        let world = Rc::clone(&self.world);
        if let Some(mut world) = world.try_borrow_mut().ok() {
            world.add_entity(bundle)
        } else {
            println!("queueing!");
            let entity = world.borrow().entities.reserve_entity();
            let entity_clone = entity.clone();
            let command = AddEntity(Box::new(move |world| {
                world.insert(entity_clone, bundle).unwrap();
            }));
            self.engine.render_commands.push(command);
            entity
        }
    }

    pub fn get_bindgroup<T: 'static>(&self) -> Option<&wgpu::BindGroup> {
        self.engine.resources.bind_groups.get(&TypeId::of::<T>())
    }

    pub fn get_system<T: 'static>(&self) -> &T {
        self.engine
            .resources
            .resource_map
            .get(&TypeId::of::<T>())
            .and_then(|system| system.downcast_ref::<T>())
            .unwrap()
    }

    pub fn get_system_mut<T: 'static>(&mut self) -> &mut T {
        self.engine
            .resources
            .resource_map
            .get_mut(&TypeId::of::<T>())
            .and_then(|system| system.downcast_mut::<T>())
            .unwrap()
    }

    pub(crate) fn get_bind_group_layouts(&self) -> Vec<Option<&wgpu::BindGroupLayout>> {
        self.engine
            .resources
            .bind_group_layouts
            .values()
            .map(|resource| Some(resource))
            .collect()
    }

    pub fn entity_query_first<B: Query>(&self, f: impl for<'a> FnOnce(<B as Query>::Item<'a>))
    where
        B: Query,
    {
        let world = &self.world.borrow_mut();

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
    pub fn entity_query<B: Query>(&self, f: impl for<'a> FnOnce(QueryBorrow<'a, B>))
    where
        B: Query,
    {
        let world = &self.world.borrow_mut();

        world.query(f);
    }

    pub fn dt(&self) -> Duration {
        self.engine.engine_time.dt()
    }
}
