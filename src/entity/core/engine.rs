use std::{any::Any, collections::HashMap};

use hecs::{DynamicBundle, Entity, World};

use crate::entity::core::{
    render::RenderContext,
    resource::{Resources, System},
};

pub struct Engine {
    pub world: World,
    pub resources: Resources,
    pub render_context: RenderContext,
    pub args: HashMap<String, Box<dyn Any>>,
}

impl Engine {
    pub fn add_system<T: System + 'static>(&mut self, system: T) {
        self.resources.register(system, &self.render_context.device);
    }
    #[inline]
    pub fn add_entity<B: DynamicBundle>(&mut self, bundle: B) -> Entity {
        self.world.spawn(bundle)
    }
}
