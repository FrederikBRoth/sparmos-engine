use std::sync::Arc;

use hecs::{DynamicBundle, Entity, Query, QueryBorrow};
use wgpu::Device;

use crate::core::resource::{Resources, System};

pub struct World {
    pub device: Arc<Device>,
    pub entities: hecs::World,
    pub resources: Resources,
}

impl World {
    pub fn new(device: Arc<Device>, entities: hecs::World, resources: Resources) -> Self {
        Self {
            device,
            entities,
            resources,
        }
    }

    pub fn add_system<T: System + 'static>(&mut self, system: T) {
        system.register(&mut self.resources, &self.device);
    }
    #[inline]
    pub fn add_entity<B: DynamicBundle>(&mut self, bundle: B) -> Entity {
        self.entities.spawn(bundle)
    }
    pub fn query_first<B: Query>(&mut self, f: impl for<'a> FnOnce(<B as Query>::Item<'a>))
    where
        B: Query,
    {
        let world = &mut self.entities;

        let mut query = world.query::<B>();

        if let Some(item) = query.iter().next() {
            f(item);
        }
    }

    pub fn query_first_with_resources<B: Query>(
        &mut self,
        f: impl for<'a> FnOnce(&mut Resources, <B as Query>::Item<'a>),
    ) where
        B: Query,
    {
        let world = &mut self.entities;

        let mut query = world.query::<B>();

        if let Some(item) = query.iter().next() {
            f(&mut self.resources, item);
        }
    }
    pub fn query<B: Query>(&mut self, f: impl for<'a> FnOnce(QueryBorrow<'a, B>))
    where
        B: Query,
    {
        let world = &mut self.entities;

        let query = world.query::<B>();

        f(query);
    }
    pub fn query_with_resources<B: Query>(
        &mut self,
        f: impl for<'a> FnOnce(&mut Resources, QueryBorrow<'a, B>),
    ) where
        B: Query,
    {
        let world = &mut self.entities;

        let query = world.query::<B>();

        f(&mut self.resources, query);
    }
}
