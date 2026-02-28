use std::collections::HashMap;

use wgpu::{BindGroup, BindGroupLayout};

use crate::entity::systems::{camera::CameraSystem, light::LightSystem};

pub trait GpuBindable {
    fn get_bind_group_layout(&self) -> &BindGroupLayout;
}

pub trait System {
    fn make_bind_group(&self, device: &wgpu::Device) -> BindGroup;
    fn get_system_name(&self) -> String;
}

pub struct Systems {
    pub bind_groups: HashMap<String, wgpu::BindGroup>,
}

impl Systems {
    fn insert(&mut self, key: &str, group: wgpu::BindGroup) {
        self.bind_groups.insert(key.to_string(), group);
    }

    pub fn get(&self, key: &str) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(&key.to_string())
    }

    pub fn new() -> Self {
        Systems {
            bind_groups: HashMap::new(),
        }
    }
    pub fn register<T: System>(&mut self, system: &T, device: &wgpu::Device) {
        self.insert(
            system.get_system_name().as_str(),
            system.make_bind_group(&device),
        );
    }
}
