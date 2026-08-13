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
    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: Option<wgpu::BindGroup>,
}

impl Buffer {
    fn create_bind_group(&mut self, buffer_type: &BufferType, device: &wgpu::Device) {
        let bind_group = match buffer_type {
            BufferType::StorageBuffer(_params) => {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.buffer.as_entire_binding(),
                    }],
                    label: Some("Quad Color Bind Group"),
                })
            }
            BufferType::UniformBuffer(_params) => {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.buffer.as_entire_binding(),
                    }],
                    label: Some("Uniform Buffer"),
                })
            }
        };

        self.bind_group = Some(bind_group)
    }
    pub fn new_init<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        instances: &[T],
        device: &wgpu::Device,
        buffer_type: BufferType,
    ) -> Self {
        let mut buffer = Buffer::new_layout_init(instances, device, &buffer_type);

        buffer.create_bind_group(&buffer_type, device);
        buffer
    }
    pub fn new_layout_init<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        instances: &[T],
        device: &wgpu::Device,
        buffer_type: &BufferType,
    ) -> Self {
        let (bind_group_layout, buffer, key) = match buffer_type {
            BufferType::StorageBuffer(params) => {
                let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("input"),
                    contents: bytemuck::cast_slice(instances),
                    // usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    usage: params.usage,
                });
                let entries = [wgpu::BindGroupLayoutEntry {
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
                }];
                let storage_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        entries: &entries,
                        label: None,
                    });

                (
                    storage_bind_group_layout,
                    storage_buffer,
                    BufferKey {
                        layout: entries.to_vec(),
                    },
                )
            }
            BufferType::UniformBuffer(params) => {
                let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Storage"),
                    contents: bytemuck::cast_slice(instances),
                    // usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    usage: params.usage,
                });
                let entries = [wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: params.shader_stages,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }];
                let uniform_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        entries: &entries,
                        label: Some("light_bind_group_layout"),
                    });
                (
                    uniform_bind_group_layout,
                    uniform_buffer,
                    BufferKey {
                        layout: entries.to_vec(),
                    },
                )
            }
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Bind Group"),
        });

        Self {
            buffer,
            bind_group_layout,
            bind_group: Some(bind_group),
            key,
        }
    }

    pub fn new<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        size: usize,
        device: &wgpu::Device,
        buffer_type: BufferType,
    ) -> Self {
        let output_size = mem::size_of::<T>() * size;

        let mut buffer = Buffer::new_layout(output_size, device, &buffer_type);

        let bind_group = match buffer_type {
            BufferType::StorageBuffer(_params) => {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &buffer.bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.buffer.as_entire_binding(),
                    }],
                    label: Some("Quad Color Bind Group"),
                })
            }
            BufferType::UniformBuffer(_params) => {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &buffer.bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.buffer.as_entire_binding(),
                    }],
                    label: Some("Uniform Buffer"),
                })
            }
        };
        buffer.bind_group = Some(bind_group);
        buffer
    }

    pub fn update<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        &self,
        queue: &wgpu::Queue,
        instance: &[T],
    ) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(instance));
    }

    pub fn new_layout(size: usize, device: &wgpu::Device, buffer_type: &BufferType) -> Self {
        let (bind_group_layout, buffer, key) = match buffer_type {
            BufferType::StorageBuffer(params) => {
                let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("output"),
                    size: size as u64,
                    usage: params.usage,
                    mapped_at_creation: false,
                });
                let entries = [wgpu::BindGroupLayoutEntry {
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
                }];
                let storage_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        entries: &entries,
                        label: None,
                    });

                (
                    storage_bind_group_layout,
                    storage_buffer,
                    BufferKey {
                        layout: entries.to_vec(),
                    },
                )
            }
            BufferType::UniformBuffer(params) => {
                let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("output"),
                    size: size as u64,
                    usage: params.usage,
                    mapped_at_creation: false,
                });
                let entries = [wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: params.shader_stages,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }];
                let uniform_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        entries: &entries,
                        label: Some("light_bind_group_layout"),
                    });
                (
                    uniform_bind_group_layout,
                    uniform_buffer,
                    BufferKey {
                        layout: entries.to_vec(),
                    },
                )
            }
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Bind Group"),
        });

        Self {
            buffer,
            bind_group_layout,
            bind_group: Some(bind_group),
            key,
        }
    }
}
