use hecs::{DynamicBundle, Entity, Query, QueryBorrow};

pub struct World {
    pub entities: hecs::World,
}

impl World {
    pub fn new(entities: hecs::World) -> Self {
        Self { entities }
    }

    #[inline]
    pub fn add_entity<B: DynamicBundle>(&mut self, bundle: B) -> Entity {
        self.entities.spawn(bundle)
    }
    pub fn query_first<B: Query>(&self, f: impl for<'a> FnOnce(<B as Query>::Item<'a>)) {
        let entities = &self.entities;

        let mut query = entities.query::<B>();

        if let Some(item) = query.iter().next() {
            f(item);
        }
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
    pub fn query<B: Query>(&self, f: impl for<'a> FnOnce(QueryBorrow<'a, B>)) {
        let entities = &self.entities;

        let query = entities.query::<B>();

        f(query);
    }
    // pub fn query_with_resources<B: Query>(
    //     &mut self,
    //     f: impl for<'a> FnOnce(&mut Resources, QueryBorrow<'a, B>),
    // ) where
    //     B: Query,
    // {
    //     let world = &mut self.entities;
    //
    //     let query = world.query::<B>();
    //
    //     f(&mut self.resources, query);
    // }
}
