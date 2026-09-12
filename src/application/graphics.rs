use std::{cell::RefCell, mem, rc::Rc, sync::Arc, time::Duration};

use cgmath::{Quaternion, Rotation3, Vector3};
use hecs::{DynamicBundle, Entity, Query, QueryBorrow};
use wgpu::{Device, Queue};
use winit::dpi::PhysicalSize;

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
        post_processing::Effect,
        render::{
            render::{
                ComputeHandle, InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext,
                RenderInstanceRef, Renderable, RenderableHandle, SkyboxRenderable, TextureHandle,
            },
            render_view::{self, RenderViewHandle, RenderViewHandler},
        },
        resource::BufferHandle,
        scene::scene_handler::{Scene, SceneHandle, SceneHandler},
        texture::{PbrTextureBuilder, Texture, TextureBuilder},
    },
    entities::meshes::Meshes,
    systems::{
        animation::AnimationHandler,
        compute::{ComputeBuilder, ReadbackState},
    },
};

pub struct Graphics {
    pub scenes: SceneHandler,
    pub render_views: RenderViewHandler,
    pub engine: Engine,
}

pub struct PhysicsRenderBatch {
    pub batch: RenderableHandle,
    pub entities: Vec<Entity>,
}

//Main API access to all functions required for rendering objects
impl Graphics {
    pub fn asset(&self, path: &str) -> Arc<[u8]> {
        self.engine.resources.assets.require(path)
    }

    pub fn shader_asset(&mut self, label: &str, path: &str) -> anyhow::Result<()> {
        let bytes = self.asset(path);
        let source = std::str::from_utf8(&bytes)?;
        self.shader(label, source);
        Ok(())
    }

    pub fn post_process_effect(&mut self, effect: Effect) -> anyhow::Result<()> {
        let bytes = self.asset("engine/shaders/chromatic_aberration.wgsl");
        let source = std::str::from_utf8(&bytes)?;
        let context = &mut self.engine.render_context;
        let size = (context.config.width, context.config.height).into();
        let format = context.config.format;
        context
            .post_processing
            .new_effect(size, format, effect, source);
        Ok(())
    }

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

    /// Register one body in the active gameplay scene.
    /// A controller slot may have only one ECS transform owner.
    pub fn spawn_physics_for_instance(
        &mut self,
        renderable: RenderableHandle,
        instance_index: usize,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> Entity {
        self.spawn_physics_for_instance_in_scene(
            self.active_gameplay_scene(),
            renderable,
            instance_index,
            collider,
            rigid_body,
        )
    }

    pub fn spawn_physics_for_instance_in_scene(
        &mut self,
        scene: SceneHandle,
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
        self.spawn_physics_transforms_in_scene(
            scene,
            renderable,
            vec![(instance_index, transform)],
            collider,
            rigid_body,
        )[0]
    }

    /// Register every existing instance in the active gameplay scene.
    pub fn spawn_physics_for_renderable(
        &mut self,
        batch: RenderableHandle,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> Vec<Entity> {
        self.spawn_physics_for_renderable_in_scene(
            self.active_gameplay_scene(),
            batch,
            collider,
            rigid_body,
        )
    }

    pub fn spawn_physics_for_renderable_in_scene(
        &mut self,
        scene: SceneHandle,
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
        self.spawn_physics_transforms_in_scene(
            scene,
            batch,
            transforms.into_iter().enumerate().collect(),
            collider,
            rigid_body,
        )
    }

    fn spawn_physics_transforms_in_scene(
        &mut self,
        scene: SceneHandle,
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
        let world = self.world(scene);
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
            self.engine.render_commands.push(AddEntity(
                scene,
                Box::new(move |world| {
                    for (entity, bundle) in pending {
                        world
                            .insert(entity, bundle)
                            .expect("reserved physics entity must exist");
                    }
                }),
            ));
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
        self.add_physics_entity_to_scene(
            self.active_gameplay_scene(),
            material,
            mesh,
            instance_controller,
            collider,
            rigid_body,
        )
    }

    pub fn add_physics_entity_to_scene(
        &mut self,
        scene: SceneHandle,
        material: MaterialHandle,
        mesh: MeshHandle,
        instance_controller: InstanceControllerHandle,
        collider: Collider,
        rigid_body: RigidBody,
    ) -> PhysicsRenderBatch {
        let batch = self.add_renderable(material, mesh, instance_controller);
        // The GPU batch is global, while this component declares that the batch
        // belongs to this scene and must be drawn by its RenderViews.
        self.add_entity_to_scene(scene, (batch,));
        let entities =
            self.spawn_physics_for_renderable_in_scene(scene, batch, collider, rigid_body);
        PhysicsRenderBatch { batch, entities }
    }

    pub(crate) fn update_render_animations(&mut self, dt: Duration) {
        let objects = &mut self.engine.render_context.gpu_objects;
        for (_, scene) in self.scenes.scenes.iter() {
            if !scene.should_simulate() {
                continue;
            }
            scene
                .world
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
    }
    pub(crate) fn sync_render_instances(&mut self) {
        let objects = &mut self.engine.render_context.gpu_objects;
        for (_, scene) in self.scenes.scenes.iter() {
            objects.sync_render_instances(&scene.world.borrow());
        }
    }

    pub(crate) fn update_instance_controllers(&mut self) {
        let context = &mut self.engine.render_context;
        for (_, controller) in context.gpu_objects.instance_controllers.iter_mut() {
            controller.update(&context.queue);
        }
    }

    pub(crate) fn run_scene_systems(&mut self) {
        self.engine.systems.run_scene_systems(
            &mut self.scenes,
            &mut self.engine.render_context,
            self.engine.engine_time.dt(),
        );
    }

    pub(crate) fn run_view_systems(&mut self) {
        self.engine.systems.run_view_systems(
            &mut self.render_views,
            &mut self.engine.render_context,
            self.engine.engine_time.dt(),
        );
    }

    pub(crate) fn resize_window_views(&mut self, size: PhysicalSize<u32>) {
        self.render_views.resize_window_targets(size);
    }

    pub fn get_world(&self) -> Rc<RefCell<World>> {
        self.scenes.active_world()
    }

    pub fn world(&self, scene: SceneHandle) -> Rc<RefCell<World>> {
        Rc::clone(&self.scenes.get(scene).expect("invalid SceneHandle").world)
    }

    pub fn active_gameplay_scene(&self) -> SceneHandle {
        self.scenes
            .active_gameplay_scene()
            .expect("no active gameplay scene; create or select a scene first")
    }

    pub fn set_active_gameplay_scene(&mut self, scene: SceneHandle) {
        self.scenes.set_active_gameplay_scene(scene);
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
            instance_layout: Some(I::layout()),
            config: PipelineConfig::default(),
        }
    }

    pub fn material_typed_no_ic<V: VertexType>(&mut self) -> MaterialBuilder<'_> {
        MaterialBuilder {
            graphics: self,
            bindings: BindGroupBuilder::new(),
            shader: String::new(),
            vertex_layout: V::layout(),
            instance_layout: None,
            config: PipelineConfig::default(),
        }
    }

    pub fn material_no_ic(&mut self) -> MaterialBuilder<'_> {
        self.material_typed_no_ic::<Vertex>()
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
            global_scale: Vector3::new(1.0, 1.0, 1.0),
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
        assert!(
            !matches!(&system, System::ViewBindable(_))
                || (self.engine.render_context.gpu_objects.materials.is_empty()
                    && self
                        .engine
                        .render_context
                        .gpu_objects
                        .compute_renderings
                        .is_empty()),
            "view systems must be registered before render materials are built"
        );
        self.engine.systems.add(system);
    }

    pub fn add_entity<B: DynamicBundle + 'static>(&mut self, bundle: B) -> Entity {
        self.add_entity_to_scene(self.active_gameplay_scene(), bundle)
    }

    pub fn add_entity_to_scene<B: DynamicBundle + 'static>(
        &mut self,
        scene: SceneHandle,
        bundle: B,
    ) -> Entity {
        let world = self.world(scene);
        if let Ok(mut world) = world.try_borrow_mut() {
            world.add_entity(bundle)
        } else {
            let entity = world.borrow().entities.reserve_entity();
            let entity_clone = entity;
            let command = AddEntity(
                scene,
                Box::new(move |world| {
                    world.insert(entity_clone, bundle).unwrap();
                }),
            );
            self.engine.render_commands.push(command);
            entity
        }
    }

    pub fn entity_query_first<B: Query>(&self, f: impl for<'a> FnOnce(<B as Query>::Item<'a>)) {
        self.entity_query_first_in_scene(self.active_gameplay_scene(), f);
    }

    pub fn entity_query_first_in_scene<B: Query>(
        &self,
        scene: SceneHandle,
        f: impl for<'a> FnOnce(<B as Query>::Item<'a>),
    ) {
        let world = self.world(scene);
        let world = world.borrow();

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
        self.entity_query_in_scene(self.active_gameplay_scene(), f);
    }

    pub fn entity_query_in_scene<B: Query>(
        &self,
        scene: SceneHandle,
        f: impl for<'a> FnOnce(QueryBorrow<'a, B>),
    ) {
        let world = self.world(scene);
        let world = world.borrow();

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

    pub fn add_texture(&mut self, texture: Texture) -> TextureHandle {
        self.engine
            .render_context
            .gpu_objects
            .textures
            .insert(texture)
    }

    pub fn pbr_texture<'a>(&'a mut self, label: &'a str) -> PbrTextureBuilder<'a> {
        PbrTextureBuilder::new(self, label)
    }

    pub fn add_skybox(&mut self, skybox_texture: &Texture, world: &mut World) {
        let skybox_mesh = Meshes::create_skybox().make_mb(self.get_render_context_mut());
        let skybox_pipeline = self
            .material_typed_no_ic::<Skybox>()
            .shader("skybox")
            .config(PipelineConfig {
                culling: None,
                depth_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                target_format: None,
            })
            .texture(skybox_texture, 1, 0)
            .build();

        let skybox_renderable = SkyboxRenderable {
            material_handle: skybox_pipeline,
            mesh_handle: skybox_mesh,
        };
        world.add_entity((skybox_renderable,));
        // self.add_entity((skybox_renderable,));
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

    pub fn new_scene(
        &mut self,
        name: &str,
        callback: impl Fn(&mut Graphics, &mut World),
    ) -> SceneHandle {
        assert!(
            !self.scenes.scenes_lookup.contains_key(name),
            "scene name '{name}' is already in use"
        );
        let scene = Scene {
            name: name.to_string(),
            ..Default::default()
        };

        let first = self.scenes.scenes.is_empty();

        let handle = self.scenes.scenes.insert(scene);
        if first {
            self.scenes.set_active_gameplay_scene(handle);
        }
        let scene = self.scenes.scenes.get(handle).unwrap();
        let world = Rc::clone(&scene.world);
        let mut world = world.borrow_mut();
        callback(self, &mut world);
        self.scenes.scenes_lookup.insert(name.to_string(), handle);
        handle
    }

    pub fn new_render_view(
        &mut self,
        name: impl Into<String>,
        scene: SceneHandle,
        target: render_view::RenderTarget,
        role: render_view::RenderViewRole,
    ) -> RenderViewHandle {
        assert!(
            self.scenes.scenes.contains_key(scene),
            "invalid SceneHandle"
        );
        self.render_views.new_view(target, scene, name, role)
    }

    pub fn set_render_view_scene(&mut self, view: RenderViewHandle, scene: SceneHandle) {
        assert!(
            self.scenes.scenes.contains_key(scene),
            "invalid SceneHandle"
        );
        self.render_views.get_render_view_mut(view).scene = scene;
    }

    pub fn remove_render_view(&mut self, handle: RenderViewHandle) -> bool {
        let removed = self.render_views.remove(handle).is_some();
        if removed {
            self.engine.systems.remove_view(handle);
        }
        removed
    }

    pub fn remove_scene(&mut self, handle: SceneHandle) -> bool {
        let dependent_views = self.render_views.remove_for_scene(handle);
        for view in dependent_views {
            self.engine.systems.remove_view(view);
        }
        self.engine.systems.remove_scene(handle);
        self.scenes.remove(handle).is_some()
    }
}

pub enum Markers {
    Skybox,
}
