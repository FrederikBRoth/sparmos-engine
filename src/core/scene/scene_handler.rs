use std::{cell::RefCell, collections::HashMap, rc::Rc};

use slotmap::{SlotMap, new_key_type};

use crate::core::entities::World;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneState {
    Loaded,
    #[default]
    Simulating,
    Sleeping,
}

pub struct Scene {
    pub world: Rc<RefCell<World>>,
    pub name: String,
    pub state: SceneState,
}

impl Scene {
    pub fn should_simulate(&self) -> bool {
        self.state == SceneState::Simulating
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            world: Rc::new(RefCell::new(World::new(hecs::World::new()))),
            name: Default::default(),
            state: SceneState::default(),
        }
    }
}

new_key_type! { pub struct SceneHandle; }

#[derive(Default)]
pub struct SceneHandler {
    pub(crate) scenes: SlotMap<SceneHandle, Scene>,
    pub scenes_lookup: HashMap<String, SceneHandle>,
    active_gameplay_scene: Option<SceneHandle>,
}

impl SceneHandler {
    pub fn get(&self, handle: SceneHandle) -> Option<&Scene> {
        self.scenes.get(handle)
    }

    pub fn get_mut(&mut self, handle: SceneHandle) -> Option<&mut Scene> {
        self.scenes.get_mut(handle)
    }

    pub fn iter(&self) -> impl Iterator<Item = (SceneHandle, &Scene)> {
        self.scenes.iter()
    }

    pub fn active_gameplay_scene(&self) -> Option<SceneHandle> {
        self.active_gameplay_scene
    }

    pub fn set_active_gameplay_scene(&mut self, handle: SceneHandle) {
        assert!(self.scenes.contains_key(handle), "invalid SceneHandle");
        self.active_gameplay_scene = Some(handle);
    }

    pub fn active_world(&self) -> Rc<RefCell<World>> {
        let handle = self
            .active_gameplay_scene
            .expect("no active gameplay scene; create or select a scene first");
        Rc::clone(&self.scenes[handle].world)
    }

    pub fn simulating_handles(&self) -> impl Iterator<Item = SceneHandle> + '_ {
        self.scenes
            .iter()
            .filter_map(|(handle, scene)| scene.should_simulate().then_some(handle))
    }

    pub(crate) fn remove(&mut self, handle: SceneHandle) -> Option<Scene> {
        let scene = self.scenes.remove(handle)?;
        self.scenes_lookup.retain(|_, value| *value != handle);
        if self.active_gameplay_scene == Some(handle) {
            self.active_gameplay_scene = self.scenes.keys().next();
        }
        Some(scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_state_does_not_depend_on_render_views() {
        let mut scenes = SceneHandler::default();
        let simulating = scenes.scenes.insert(Scene::default());
        let sleeping = scenes.scenes.insert(Scene {
            state: SceneState::Sleeping,
            ..Scene::default()
        });

        assert_eq!(
            scenes.simulating_handles().collect::<Vec<_>>(),
            vec![simulating]
        );
        assert_ne!(simulating, sleeping);
    }
}
