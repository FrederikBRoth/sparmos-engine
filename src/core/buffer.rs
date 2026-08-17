use std::mem;

use wgpu::{BindGroupLayoutEntry, util::DeviceExt};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Color {
    pub color: [f32; 3],
    pub _pad: f32, // 4 bytes padding to align to 16 bytes total
}

pub enum BufferType {
    StorageBuffer(StorageParameters),
    UniformBuffer(UniformParameters),
}
impl BufferType {
    pub fn usage(&self) -> wgpu::BufferUsages {
        match self {
            BufferType::StorageBuffer(params) => params.usage,
            BufferType::UniformBuffer(params) => params.usage,
        }
    }
}

pub struct StorageParameters {
    pub read_only: bool,
    pub init: bool,
    pub shader_stages: wgpu::ShaderStages,
    pub usage: wgpu::BufferUsages,
}

impl Default for StorageParameters {
    fn default() -> Self {
        Self {
            read_only: true,
            init: true,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            shader_stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }
}

pub struct UniformParameters {
    pub init: bool,
    pub shader_stages: wgpu::ShaderStages,
    pub usage: wgpu::BufferUsages,
}

impl Default for UniformParameters {
    fn default() -> Self {
        Self {
            init: true,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            shader_stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct BufferKey {
    layout: Vec<BindGroupLayoutEntry>,
}

#[derive(Clone)]
pub struct Buffer {
    pub key: BufferKey,
    // Multiple Buffer objects can reference the same underlying GPU buffer
    // while having different layouts/bind groups for different pipelines.
    pub buffer: wgpu::Buffer,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl Buffer {
    fn create_layout(
        device: &wgpu::Device,
        buffer_type: &BufferType,
    ) -> (wgpu::BindGroupLayout, BufferKey) {
        let entry = match buffer_type {
            BufferType::StorageBuffer(params) => wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: params.shader_stages,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage {
                        read_only: params.read_only,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },

            BufferType::UniformBuffer(params) => wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: params.shader_stages,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        };

        let entries = vec![entry];

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &entries,
            label: Some("Buffer Bind Group Layout"),
        });

        let key = BufferKey { layout: entries };

        (bind_group_layout, key)
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Buffer Bind Group"),
        })
    }

    fn from_buffer(buffer: wgpu::Buffer, device: &wgpu::Device, buffer_type: BufferType) -> Self {
        let (bind_group_layout, key) = Self::create_layout(device, &buffer_type);

        let bind_group = Self::create_bind_group(device, &bind_group_layout, &buffer);

        Self {
            buffer,
            bind_group_layout,
            bind_group,
            key,
        }
    }

    pub fn new_init<T: bytemuck::Pod>(
        data: &[T],
        device: &wgpu::Device,
        buffer_type: BufferType,
    ) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Buffer"),
            contents: bytemuck::cast_slice(data),
            usage: buffer_type.usage(),
        });

        Self::from_buffer(buffer, device, buffer_type)
    }

    pub fn new(
        object_size: usize,
        size: usize,
        device: &wgpu::Device,
        buffer_type: BufferType,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buffer"),
            size: (object_size * size) as u64,
            usage: buffer_type.usage(),
            mapped_at_creation: false,
        });

        Self::from_buffer(buffer, device, buffer_type)
    }

    pub fn from_existing(source: &Buffer, device: &wgpu::Device, buffer_type: BufferType) -> Self {
        Self::from_buffer(source.buffer.clone(), device, buffer_type)
    }

    pub fn update<T: bytemuck::Pod>(&self, queue: &wgpu::Queue, data: &[T]) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }
}
