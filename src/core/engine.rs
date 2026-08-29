use std::{
    any::Any,
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use crate::{
    audio::{
        audio_handler::{AudioHandler, AudioTrigger},
        synth::Sound,
    },
    core::{
        buffer::Buffer,
        entities::World,
        geometry::Mesh,
        instance::InstanceControllerTrait,
        pipelines::Material,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext},
        resource::Resources,
    },
};
pub trait System {
    fn run(&mut self, world: Ref<'_, World>, resources: &mut RenderContext, dt: Duration);
    fn get_buffer(&self) -> &Buffer;
}

impl Systems {
    pub fn add<T: System + 'static>(&mut self, system: T) {
        self.systems.push(Box::new(system));
    }

    pub fn run_all(
        &mut self,
        world: &mut Rc<RefCell<World>>,
        resources: &mut RenderContext,
        dt: Duration,
    ) {
        for system in &mut self.systems {
            system.run(world.borrow(), resources, dt);
        }
    }

    pub(crate) fn get_bind_group_layouts(&self) -> Vec<Option<&wgpu::BindGroupLayout>> {
        self.systems
            .iter()
            .map(|resource| Some(&resource.as_ref().get_buffer().bind_group_layout))
            .collect()
    }
    pub(crate) fn get_bind_groups(&self) -> Vec<Option<&wgpu::BindGroup>> {
        self.systems
            .iter()
            .map(|resource| Some(&resource.as_ref().get_buffer().bind_group))
            .collect()
    }
}

pub struct Systems {
    pub(crate) systems: Vec<Box<dyn System>>,
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

        if self.time_acc >= std::time::Duration::from_secs(1) {
            let fps = self.frame_count as f64 / self.time_acc.as_secs_f64();
            if print_fps {
                println!("FPS: {:.2}", fps);
            }

            // reset
            self.frame_count = 0;
            self.time_acc = std::time::Duration::ZERO;
        }
        self.dt = delta_time;
    }

    pub(crate) fn dt(&self) -> Duration {
        self.dt
    }
}

pub enum EngineCommandQueue {
    ChangeShader(MaterialHandle, String),
    AddEntity(Box<dyn FnOnce(&mut hecs::World) + 'static>),
}

pub struct Arguments {
    pub args: HashMap<String, Box<dyn Any>>,
}

impl Arguments {
    pub fn with_arg<T: 'static, R>(&mut self, key: &str, f: impl FnOnce(Option<&T>) -> R) -> R {
        let value = self
            .args
            .get(key)
            .and_then(|boxed| boxed.downcast_ref::<T>());

        f(value)
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
        ic_handle: &InstanceControllerHandle,
    ) -> &mut Box<dyn InstanceControllerTrait> {
        self.render_context
            .gpu_objects
            .instance_controllers
            .get_mut(*ic_handle)
            .unwrap()
    }

    pub fn get_mesh(&mut self, mesh_handle: &MeshHandle) -> &Mesh {
        self.render_context
            .gpu_objects
            .meshes
            .get_mut(*mesh_handle)
            .unwrap()
    }
    pub fn get_material(&mut self, material_handle: &MaterialHandle) -> &mut Material {
        self.render_context
            .gpu_objects
            .materials
            .get_mut(*material_handle)
            .unwrap()
    }

    pub fn init_sound(&mut self, pre_gain: f32, post_gain: f32) {
        if self.audio_triggers.is_none() {
            self.audio_triggers = Some(HashMap::new());
        }
        let audio_handler =
            AudioHandler::start_audio(self.audio_triggers.take().unwrap(), pre_gain, post_gain);
        self.audio_handler = Some(audio_handler);
    }

    pub fn get_audio_handler(&mut self) -> &mut AudioHandler {
        self.audio_handler.as_mut().unwrap()
    }
}
