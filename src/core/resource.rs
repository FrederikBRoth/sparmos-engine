use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use indexmap::IndexMap;
use slotmap::{SlotMap, new_key_type};
use wgpu::{BindGroup, BindGroupLayout};

use crate::core::buffer::Buffer;

new_key_type! { pub struct BufferHandle; }
pub struct Resources {
    pub buffers: SlotMap<BufferHandle, Buffer>,
}

impl Resources {
    pub(crate) fn get_bindgroup(&self, handle: BufferHandle) -> Option<&wgpu::BindGroup> {
        self.buffers.get(handle).map(|buffer| &buffer.bind_group)
    }

    pub(crate) fn new() -> Self {
        Resources {
            buffers: SlotMap::with_key(),
        }
    }

    pub(crate) fn get_bind_group_layouts(&self) -> Vec<Option<&wgpu::BindGroupLayout>> {
        self.buffers
            .values()
            .map(|resource| Some(&resource.bind_group_layout))
            .collect()
    }
    pub(crate) fn get_bind_groups(&self) -> Vec<Option<&wgpu::BindGroup>> {
        self.buffers
            .values()
            .map(|resource| Some(&resource.bind_group))
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
