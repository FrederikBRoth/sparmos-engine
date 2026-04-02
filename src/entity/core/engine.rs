use std::{any::Any, collections::HashMap};

use hecs::{DynamicBundle, Entity, Query, World};

use crate::{
    entity::core::{
        render::{MaterialHandle, RenderContext, Renderable},
        resource::{Resources, System},
    },
    helpers::animation::AnimationHandler,
};

pub enum RenderCommands {
    ChangeShader(MaterialHandle, String),
}

pub struct Engine {
    pub frame_count: u32,
    pub time_acc: std::time::Duration,
    pub render_commands: Vec<RenderCommands>,
    pub render_context: RenderContext,
    pub args: HashMap<String, Box<dyn Any>>,
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

    // pub fn change_shader(&mut self, material: &MaterialHandle, shader: &str) {
    //     self.render_commands.push(RenderCommands::ChangeShader(
    //         material.clone(),
    //         shader.to_string(),
    //     ));
    // }
}
