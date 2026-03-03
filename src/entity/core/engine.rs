use hecs::{DynamicBundle, Entity, World};

use crate::entity::core::resource::{Resources, System};

pub struct Engine {
    pub world: World,
    pub resources: Resources,
}

impl Engine {
    pub fn add_system<T: System + 'static>(&mut self, system: T, device: &wgpu::Device) {
        self.resources.register(system, device);
    }
    #[inline]
    pub fn add_entity<B: DynamicBundle>(&mut self, bundle: B) -> Entity {
        self.world.spawn(bundle)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            world: World::new(),
            resources: Resources::new(),
        }
    }
}
