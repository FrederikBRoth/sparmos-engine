use std::{cell::RefCell, collections::HashMap, rc::Rc};

use slotmap::{SlotMap, new_key_type};

use crate::core::{engine::System, entities::World};

pub struct Scene {
    pub world: Rc<RefCell<World>>,
    pub name: String,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            world: Rc::new(RefCell::new(World::new(hecs::World::new()))),
            name: Default::default(),
        }
    }
}

new_key_type! { pub struct SceneHandle; }
#[derive(Default)]
pub struct SceneHandler {
    pub scenes: SlotMap<SceneHandle, Scene>,
    pub scenes_lookup: HashMap<String, SceneHandle>,
    //TODO: TEMP CURRENT VIEWPORT
    pub current: SceneHandle,
}
