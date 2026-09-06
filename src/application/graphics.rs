use std::{cell::RefCell, collections::HashSet, mem, rc::Rc, sync::Arc, time::Duration};

use cgmath::{Quaternion, Rotation3, Vector3};
use hecs::{DynamicBundle, Entity, Query, QueryBorrow};
use wgpu::{Device, Queue};

use crate::{
    core::{
        binding::BindGroupBuilder,
        buffer::Buffer,
        engine::{
            Engine,
            EngineCommandQueue::{self, AddEntity},
            System,
        },
        entities::World,
        geometry::{ModelBuilder, Skybox, Vertex, VertexType},
        instance::{
            DefaultInstanceLayout, InstanceBuilder, InstanceControllerTrait, RawInstance, Transform,
        },
        physics::{collision::Collider, rigidbody::RigidBody},
        pipelines::{
            ComputeRenderingBuilder, MaterialBuilder, PipelineConfig, RenderPipelineBuilder,
        },
        render::{
            ComputeHandle, InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext,
            RenderInstanceRef, Renderable, RenderableHandle, SkyboxRenderable,
        },
        resource::BufferHandle,
        texture::{PbrTextureBuilder, Texture, TextureBuilder},
    },
    entities::meshes::Meshes,
    systems::{
        animation::AnimationHandler,
        compute::{ComputeBuilder, ReadbackState},
    },
};

pub struct Graphics {
    pub(crate) world: Rc<RefCell<World>>,
    pub engine: Engine,
}

pub struct PhysicsRenderBatch {
    pub batch: RenderableHandle,
    pub entities: Vec<Entity>,
}

//Main API access to all functions required for rendering objects
impl Graphics {
    pub fn add_renderable(
        &mut self,
        material_handle: MaterialHandle,
        mesh_handle: MeshHandle,
        instance_controller_handle: InstanceControllerHandle,
    ) -> RenderableHandle {
        let objects = &mut self.engine.render_context.gpu_objects;
        objects.add_renderable(Renderable {
            material_handle,
            mesh_handle,
            instance_controller_handle,
        })
    }

    pub fn get_renderable(&self, handle: RenderableHandle) -> &Renderable {
        self.engine
            .render_context
            .gpu_objects
            .renderable(handle)
            .expect("invalid RenderBatchHandle")
    }

    pub fn get_renderable_mut(&mut self, handle: RenderableHandle) -> &mut Renderable {
        self.engine
            .render_context
            .gpu_objects
            .renderable_mut(handle)
            .expect("invalid RenderBatchHandle")
    }

    /// Only render-only instances may have their transforms edited directly.
    /// Linked instance transforms must be changed through their ECS entity.
    pub fn get_instance_controller(
        &mut self,
        batch: RenderableHandle,
    ) -> &mut Box<dyn InstanceControllerTrait> {
        let handle = self.get_renderable(batch).instance_controller_handle;
        self.engine
            .render_context
            .gpu_objects
            .instance_controllers
            .get_mut(handle)
            .expect("invalid InstanceControllerHandle")
    }

    pub fn change_renderable_shader(&mut self, renderable: RenderableHandle, shader: &str) {
        let material = self.get_renderable(renderable).material_handle;
        self.change_shader(&material, shader);
    }

    /// Register one body. A controller slot may have only one ECS transform owner.
    pub fn spawn_physics_for_instance(
        &mut self,
        renderable: RenderableHandle,
        instance_index: usize,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> Entity {
        let render_ref = RenderInstanceRef {
            batch: renderable,
            instance_index,
        };
        let transform = self
            .engine
            .render_context
            .gpu_objects
            .render_instance(render_ref)
            .transform
            .clone();
        self.spawn_physics_transforms(
            renderable,
            vec![(instance_index, transform)],
            collider,
            rigid_body,
        )[0]
    }

    /// Register every existing instance as an independent body in the same batch.
    pub fn spawn_physics_for_renderable(
        &mut self,
        batch: RenderableHandle,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> Vec<Entity> {
        let transforms: Vec<Transform> = self
            .get_instance_controller(batch)
            .instances()
            .iter()
            .map(|instance| instance.transform.clone())
            .collect();
        self.spawn_physics_transforms(
            batch,
            transforms.into_iter().enumerate().collect(),
            collider,
            rigid_body,
        )
    }

    fn spawn_physics_transforms(
        &mut self,
        batch: RenderableHandle,
        transforms: Vec<(usize, Transform)>,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> Vec<Entity> {
        let bundles = transforms.into_iter().map(|(instance_index, transform)| {
            (
                transform,
                collider.clone(),
                rigid_body.clone(),
                RenderInstanceRef {
                    batch,
                    instance_index,
                },
            )
        });
        let world = Rc::clone(&self.world);
        if let Ok(mut world) = world.try_borrow_mut() {
            bundles.map(|bundle| world.add_entity(bundle)).collect()
        } else {
            // Game callbacks borrow the world. Queue the whole batch and revalidate
            // once when inserted, so pending registrations cannot claim a slot twice.
            let world = world.borrow();
            let pending: Vec<_> = bundles
                .map(|bundle| (world.entities.reserve_entity(), bundle))
                .collect();
            let entities = pending.iter().map(|(entity, _)| *entity).collect();
            self.engine
                .render_commands
                .push(AddEntity(Box::new(move |world| {
                    for (entity, bundle) in pending {
                        world
                            .insert(entity, bundle)
                            .expect("reserved physics entity must exist");
                    }
                })));
            entities
        }
    }
    ///Physics entities are added with defined hecs objects due to how we handle transform data
    ///We have to adjust the transform data
    pub fn add_physics_entity(
        &mut self,
        material: MaterialHandle,
        mesh: MeshHandle,
        instance_controller: InstanceControllerHandle,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> PhysicsRenderBatch {
        let batch = self.add_renderable(material, mesh, instance_controller);
        let entities = self.spawn_physics_for_renderable(batch, collider, rigid_body);
        PhysicsRenderBatch { batch, entities }
    }

    pub(crate) fn update_render_animations(&mut self, dt: Duration) {
        let objects = &mut self.engine.render_context.gpu_objects;
        self.world
            .borrow()
            .query::<(&RenderableHandle, &mut AnimationHandler)>(|mut query| {
                for (batch_ref, animation) in query.iter() {
                    let batch = objects
                        .renderable(*batch_ref)
                        .expect("invalid RenderBatchHandle");
                    let handle = batch.instance_controller_handle;
                    let controller = objects
                        .instance_controllers
                        .get_mut(handle)
                        .expect("invalid InstanceControllerHandle");
                    animation.update_instance(dt.as_secs_f32(), controller.instances_mut());
                }
            });
    }
    pub(crate) fn sync_render_instances(&mut self) {
        self.engine
            .render_context
            .gpu_objects
            .sync_render_instances(&self.world.borrow());
    }

    pub(crate) fn update_instance_controllers(&mut self) {
        let context = &mut self.engine.render_context;
        for (_, controller) in context.gpu_objects.instance_controllers.iter_mut() {
            controller.update(&context.queue);
        }
    }

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

    pub fn material(&mut self) -> MaterialBuilder<'_> {
        self.material_typed::<Vertex, DefaultInstanceLayout>()
    }

    pub fn material_vertex<V: VertexType>(&mut self) -> MaterialBuilder<'_> {
        self.material_typed::<V, DefaultInstanceLayout>()
    }

    pub fn material_instance<I: RawInstance>(&mut self) -> MaterialBuilder<'_> {
        self.material_typed::<Vertex, I>()
    }

    pub fn material_typed<V: VertexType, I: RawInstance>(&mut self) -> MaterialBuilder<'_> {
        MaterialBuilder {
            graphics: self,
            bindings: BindGroupBuilder::new(),
            shader: String::new(),
            vertex_layout: V::layout(),
            instance_layout: I::layout(),
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
            rotation: Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0)),
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

    pub fn add_system(&mut self, system: System) {
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
    pub(crate) fn get_buffers(&self) -> Vec<&Buffer> {
        self.engine.systems.get_buffers()
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
            .material_typed::<Skybox, DefaultInstanceLayout>()
            .shader("skybox")
            .config(PipelineConfig {
                culling: None,
                depth_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                target_format: None,
            })
            .texture(&skybox_texture, 1, 0)
            .build();

        let skybox_renderable = SkyboxRenderable {
            material_handle: skybox_pipeline,
            mesh_handle: skybox_mesh,
        };
        self.add_entity((skybox_renderable,));
    }

    pub(crate) fn material_with_texture(
        &mut self,
        base: MaterialHandle,
        texture: &Texture,
        group: u32,
        start_binding: u32,
    ) -> MaterialHandle {
        let material = self.engine.render_context.gpu_objects.materials[base].with_texture(
            self.get_device(),
            texture,
            group,
            start_binding,
        );
        let key = material.key.clone();
        if let Some(handle) = self
            .engine
            .render_context
            .gpu_objects
            .material_lookup
            .get(&key)
        {
            return *handle;
        }
        let handle = self
            .engine
            .render_context
            .gpu_objects
            .materials
            .insert(material);
        self.engine
            .render_context
            .gpu_objects
            .material_lookup
            .insert(key, handle);
        handle
    }
}

pub enum Markers {
    Skybox,
}
