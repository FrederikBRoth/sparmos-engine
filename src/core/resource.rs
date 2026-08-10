use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use indexmap::IndexMap;
use wgpu::{BindGroup, BindGroupLayout};

pub trait GpuBindable {
    fn get_bind_group_layout(&self) -> &BindGroupLayout;
    fn make_bind_group(&self, device: &wgpu::Device) -> BindGroup;
}

pub trait System {
    fn get_system_name(&self) -> String;
    fn register(self, resources: &mut Resources, device: &wgpu::Device);
}

pub struct Resources {
    //For quick bind_group reading, that avoids vtable lookups
    pub bind_groups: IndexMap<TypeId, wgpu::BindGroup>,
    pub resource_map: HashMap<TypeId, Box<dyn Any>>,
}

impl Resources {
    pub fn get_bindgroup<T: 'static>(&self) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(&TypeId::of::<T>())
    }

    pub fn get_system<T: 'static>(&self) -> &T {
        self.resource_map
            .get(&TypeId::of::<T>())
            .unwrap()
            .downcast_ref::<T>()
            .unwrap()
    }

    pub fn get_system_mut<T: 'static>(&mut self) -> &mut T {
        self.resource_map
            .get_mut(&TypeId::of::<T>())
            .unwrap()
            .downcast_mut::<T>()
            .unwrap()
    }
    pub fn new() -> Self {
        Resources {
            bind_groups: IndexMap::new(),
            resource_map: HashMap::new(),
        }
    }
}

pub trait Register {
    fn register(self, resources: &mut Resources, device: &wgpu::Device);
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}
