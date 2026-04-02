use std::{
    any::Any,
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use hecs::{DynamicBundle, Entity, Query, World};

use crate::{
    entity::core::{
        geometry::Mesh,
        instance::InstanceControllerTrait,
        material::Material,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext, Renderable},
        resource::{Resources, System},
    },
    helpers::animation::AnimationHandler,
};

pub enum RenderCommands {
    ChangeShader(MaterialHandle, String),
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
    pub frame_count: u32,
    pub time_acc: std::time::Duration,
    pub render_commands: Vec<RenderCommands>,
    pub render_context: RenderContext,
    pub arguments: Arguments,
}

impl Engine {
    pub fn change_shader(&mut self, material: &MaterialHandle, shader: &str) {
        if let Some(material) = self
            .render_context
            .gpu_objects
            .materials
            .get_mut(material.clone())
            && let Some(shader) = self.render_context.shaders.get(shader)
        {
            material.change_shader(
                &self.render_context.device,
                self.render_context.config.format.clone(),
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

    // pub fn change_shader(&mut self, material: &MaterialHandle, shader: &str) {
    //     self.render_commands.push(RenderCommands::ChangeShader(
    //         material.clone(),
    //         shader.to_string(),
    //     ));
    // }
}
