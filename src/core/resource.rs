use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use indexmap::IndexMap;
use wgpu::{BindGroup, BindGroupLayout};

pub trait GpuBindable {
    fn get_bind_group_layout(&self) -> &BindGroupLayout;
}

pub trait System {
    fn get_system_name(&self) -> String;
    fn register(self, resources: &mut Resources);
}

pub struct Resources {
    //For quick bind_group reading, that avoids vtable lookups
    pub resource_map: HashMap<TypeId, Box<dyn Any>>,
    pub bind_group_layouts: IndexMap<TypeId, BindGroupLayout>,
    pub bind_groups: IndexMap<TypeId, BindGroup>,
}

impl Resources {
    pub(crate) fn get_bindgroup<T: 'static>(&self) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(&TypeId::of::<T>())
    }

    pub(crate) fn get_system<T: 'static>(&self) -> Option<&T> {
        self.resource_map
            .get(&TypeId::of::<T>())
            .and_then(|system| system.downcast_ref::<T>())
    }

    pub(crate) fn get_system_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resource_map
            .get_mut(&TypeId::of::<T>())
            .and_then(|system| system.downcast_mut::<T>())
    }
    pub(crate) fn new() -> Self {
        Resources {
            resource_map: HashMap::new(),
            bind_group_layouts: IndexMap::new(),
            bind_groups: IndexMap::new(),
        }
    }

    pub(crate) fn get_bind_group_layouts(&self) -> Vec<Option<&wgpu::BindGroupLayout>> {
        self.bind_group_layouts
            .values()
            .map(|resource| Some(resource))
            .collect()
    }
}

pub trait Register {
    fn register(self, resources: &mut Resources);
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}
