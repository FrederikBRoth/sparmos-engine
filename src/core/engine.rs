use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use crate::{
    audio::{
        audio_handler::{AudioHandler, AudioTrigger},
        synth::Sound,
    },
    core::{
        binding::BindGroupBuilder,
        buffer::Buffer,
        entities::World,
        geometry::Mesh,
        instance::InstanceControllerTrait,
        pipelines::Material,
        render::{
            render::{InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext},
            render_view::{RenderView, RenderViewHandle, RenderViewHandler},
        },
        resource::Resources,
        scene::scene_handler::{SceneHandle, SceneHandler},
    },
};

pub enum System {
    ViewBindable(Box<dyn ViewSystem>),
    Scene(Box<dyn SceneSystem>),
}

impl System {
    pub fn gpu_bindable<T>(system: T) -> Self
    where
        T: ViewSystem + 'static,
    {
        Self::ViewBindable(Box::new(system))
    }

    pub fn default<T>(system: T) -> Self
    where
        T: SceneSystem + 'static,
    {
        Self::Scene(Box::new(system))
    }

    pub fn view<T: ViewSystem + 'static>(system: T) -> Self {
        Self::ViewBindable(Box::new(system))
    }

    pub fn scene<T: SceneSystem + 'static>(system: T) -> Self {
        Self::Scene(Box::new(system))
    }
}

/// A scene system owns simulation work. It runs once per simulating scene per frame.
pub trait SceneSystem {
    fn run(
        &mut self,
        scene: SceneHandle,
        world: &mut World,
        resources: &mut RenderContext,
        dt: Duration,
    );

    fn remove_scene(&mut self, _scene: SceneHandle) {}
}

pub use SceneSystem as DefaultSystem;

/// A view system owns per-view GPU state. It never selects or simulates a scene.
pub trait ViewSystem {
    fn prepare_view(
        &mut self,
        _view: &RenderView,
        _resources: &mut RenderContext,
        _render_view: RenderViewHandle,
    ) {
    }

    fn run(
        &mut self,
        view: &mut RenderView,
        resources: &mut RenderContext,
        render_view: RenderViewHandle,
        dt: Duration,
    );

    fn get_buffer(&self, render_view: RenderViewHandle) -> &Buffer;
    fn binding_location(&self) -> (u32, u32);
    fn binding_layout_entry(&self) -> wgpu::BindGroupLayoutEntry;

    fn remove_view(&mut self, _render_view: RenderViewHandle) {}
}

pub use ViewSystem as GpuBindableSystem;

#[derive(Default)]
pub struct Systems {
    pub(crate) systems: Vec<System>,
    view_bind_group_layouts: Vec<Option<wgpu::BindGroupLayout>>,
    view_binding_layout_entries: Vec<Option<Vec<wgpu::BindGroupLayoutEntry>>>,
    view_bind_groups: HashMap<RenderViewHandle, Vec<Option<wgpu::BindGroup>>>,
}

impl Systems {
    pub fn add(&mut self, system: System) {
        self.systems.push(system);
        self.view_bind_group_layouts.clear();
        self.view_binding_layout_entries.clear();
        self.view_bind_groups.clear();
    }

    pub fn run_scene_systems(
        &mut self,
        scenes: &mut SceneHandler,
        resources: &mut RenderContext,
        dt: Duration,
    ) {
        for (handle, scene) in scenes.scenes.iter_mut() {
            if !scene.should_simulate() {
                continue;
            }
            let mut world = scene.world.borrow_mut();
            for system in &mut self.systems {
                if let System::Scene(system) = system {
                    system.run(handle, &mut world, resources, dt);
                }
            }
        }
    }

    pub fn run_view_systems(
        &mut self,
        views: &mut RenderViewHandler,
        resources: &mut RenderContext,
        dt: Duration,
    ) {
        self.ensure_view_layouts(&resources.device);
        let live_handles = views.views.keys().collect::<Vec<_>>();
        let stale_handles = self
            .view_bind_groups
            .keys()
            .filter(|handle| !live_handles.contains(handle))
            .copied()
            .collect::<Vec<_>>();
        for handle in stale_handles {
            self.remove_view(handle);
        }

        for (handle, view) in views.views.iter_mut() {
            if !view.enabled {
                continue;
            }
            view.sync_camera_to_target();
            for system in &mut self.systems {
                if let System::ViewBindable(system) = system {
                    system.prepare_view(view, resources, handle);
                    system.run(view, resources, handle, dt);
                }
            }
            if !self.view_bind_groups.contains_key(&handle) {
                self.rebuild_view_bind_group(handle, &resources.device);
            }
        }
    }

    fn ensure_view_layouts(&mut self, device: &wgpu::Device) {
        if !self.view_bind_group_layouts.is_empty() {
            return;
        }
        let mut entries = BTreeMap::<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>>::new();
        for system in &self.systems {
            let System::ViewBindable(system) = system else {
                continue;
            };
            let (group, binding) = system.binding_location();
            let mut entry = system.binding_layout_entry();
            entry.binding = binding;
            assert!(
                entries
                    .entry(group)
                    .or_default()
                    .insert(binding, entry)
                    .is_none(),
                "duplicate view binding at group {group}, binding {binding}"
            );
        }
        let count = entries
            .last_key_value()
            .map(|(&group, _)| group as usize + 1)
            .unwrap_or(0);
        self.view_bind_group_layouts = vec![None; count];
        self.view_binding_layout_entries = vec![None; count];
        for (group, bindings) in entries {
            let layout_entries = bindings.into_values().collect::<Vec<_>>();
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render view bind group layout"),
                entries: &layout_entries,
            });
            self.view_bind_group_layouts[group as usize] = Some(layout);
            self.view_binding_layout_entries[group as usize] = Some(layout_entries);
        }
    }

    fn rebuild_view_bind_group(&mut self, handle: RenderViewHandle, device: &wgpu::Device) {
        let mut bindings = BindGroupBuilder::new();
        for system in &self.systems {
            if let System::ViewBindable(system) = system {
                let (group, binding) = system.binding_location();
                bindings.buffer(system.get_buffer(handle), group, binding);
            }
        }
        let groups = bindings.build_with_layouts(
            device,
            &self.view_bind_group_layouts,
            &self.view_binding_layout_entries,
            "render view bind group",
        );
        self.view_bind_groups.insert(handle, groups);
    }

    pub(crate) fn view_bind_group_layouts(
        &mut self,
        device: &wgpu::Device,
    ) -> Vec<Option<wgpu::BindGroupLayout>> {
        self.ensure_view_layouts(device);
        self.view_bind_group_layouts.clone()
    }

    pub(crate) fn view_bind_groups(&self, handle: RenderViewHandle) -> &[Option<wgpu::BindGroup>] {
        self.view_bind_groups
            .get(&handle)
            .map(Vec::as_slice)
            .expect("view GPU state has not been prepared")
    }

    pub(crate) fn remove_view(&mut self, handle: RenderViewHandle) {
        self.view_bind_groups.remove(&handle);
        for system in &mut self.systems {
            if let System::ViewBindable(system) = system {
                system.remove_view(handle);
            }
        }
    }

    pub(crate) fn remove_scene(&mut self, handle: SceneHandle) {
        for system in &mut self.systems {
            if let System::Scene(system) = system {
                system.remove_scene(handle);
            }
        }
    }
}

pub struct EngineTime {
    pub(crate) frame_count: u32,
    pub(crate) time_acc: Duration,
    pub(crate) dt: Duration,
}

impl EngineTime {
    pub(crate) fn update_time(&mut self, delta_time: Duration, print_fps: bool) {
        self.frame_count += 1;
        self.time_acc += delta_time;
        if self.time_acc >= Duration::from_secs(1) {
            let fps = self.frame_count as f64 / self.time_acc.as_secs_f64();
            if print_fps {
                println!("FPS: {:.2}", fps);
            }
            self.frame_count = 0;
            self.time_acc = Duration::ZERO;
        }
        self.dt = delta_time;
    }

    pub(crate) fn dt(&self) -> Duration {
        self.dt
    }
}

pub enum EngineCommandQueue {
    ChangeShader(MaterialHandle, String),
    AddEntity(
        crate::core::scene::scene_handler::SceneHandle,
        Box<dyn FnOnce(&mut hecs::World) + 'static>,
    ),
}

pub struct Arguments {
    pub args: HashMap<String, Box<dyn Any>>,
}

impl Arguments {
    pub fn with_arg<T: 'static, R>(&mut self, key: &str, f: impl FnOnce(Option<&T>) -> R) -> R {
        f(self
            .args
            .get(key)
            .and_then(|boxed| boxed.downcast_ref::<T>()))
    }
}

pub struct Engine {
    pub engine_time: EngineTime,
    pub render_commands: Vec<EngineCommandQueue>,
    pub resources: Resources,
    pub render_context: RenderContext,
    pub arguments: Arguments,
    pub systems: Systems,
    pub audio_handler: Option<AudioHandler>,
    pub audio_triggers: Option<HashMap<AudioTrigger, Sound>>,
}

impl Engine {
    pub(crate) fn change_shader_inner(&mut self, material: &MaterialHandle, shader: &str) {
        if let Some(material) = self.render_context.gpu_objects.materials.get_mut(*material)
            && let Some(shader) = self.render_context.shaders.get(shader)
        {
            material.change_shader(
                &self.render_context.device,
                self.render_context.config.format,
                shader,
            );
        }
    }

    pub fn get_instance_controller(
        &mut self,
        handle: &InstanceControllerHandle,
    ) -> &mut Box<dyn InstanceControllerTrait> {
        self.render_context
            .gpu_objects
            .instance_controllers
            .get_mut(*handle)
            .unwrap()
    }

    pub fn get_mesh(&mut self, handle: &MeshHandle) -> &Mesh {
        self.render_context
            .gpu_objects
            .meshes
            .get_mut(*handle)
            .unwrap()
    }

    pub fn get_material(&mut self, handle: &MaterialHandle) -> &mut Material {
        self.render_context
            .gpu_objects
            .materials
            .get_mut(*handle)
            .unwrap()
    }

    pub fn init_sound(&mut self, pre_gain: f32, post_gain: f32) {
        if self.audio_triggers.is_none() {
            self.audio_triggers = Some(HashMap::new());
        }
        self.audio_handler = Some(AudioHandler::start_audio(
            self.audio_triggers.take().unwrap(),
            pre_gain,
            post_gain,
        ));
    }

    pub fn get_audio_handler(&mut self) -> &mut AudioHandler {
        self.audio_handler.as_mut().unwrap()
    }
}
