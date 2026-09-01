use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::util::DeviceExt;

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

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
    id: u64,
    layout: wgpu::BindGroupLayoutEntry,
}

#[derive(Clone)]
pub struct Buffer {
    pub key: BufferKey,
    pub buffer: wgpu::Buffer,
    binding_layout: wgpu::BindGroupLayoutEntry,
}

impl Buffer {
    fn create_layout(buffer_type: &BufferType) -> wgpu::BindGroupLayoutEntry {
        match buffer_type {
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
        }
    }

    pub(crate) fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            ..self.binding_layout
        }
    }

    pub(crate) fn bind_group_entry(&self, binding: u32) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: self.buffer.as_entire_binding(),
        }
    }

    fn from_buffer(buffer: wgpu::Buffer, buffer_type: BufferType) -> Self {
        let binding_layout = Self::create_layout(&buffer_type);
        let key = BufferKey {
            id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            layout: binding_layout,
        };

        Self {
            buffer,
            binding_layout,
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

        Self::from_buffer(buffer, buffer_type)
    }

    pub fn new_init_matching<T: bytemuck::Pod>(
        data: &[T],
        device: &wgpu::Device,
        buffer_type: BufferType,
        template: &Buffer,
    ) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Buffer"),
            contents: bytemuck::cast_slice(data),
            usage: buffer_type.usage(),
        });

        Self {
            buffer,
            binding_layout: template.binding_layout,
            key: BufferKey {
                id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
                layout: template.binding_layout,
            },
        }
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

        Self::from_buffer(buffer, buffer_type)
    }

    pub fn from_existing(source: &Buffer, device: &wgpu::Device, buffer_type: BufferType) -> Self {
        let _ = device;
        Self::from_buffer(source.buffer.clone(), buffer_type)
    }

    pub fn update<T: bytemuck::Pod>(&self, queue: &wgpu::Queue, data: &[T]) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }
}
