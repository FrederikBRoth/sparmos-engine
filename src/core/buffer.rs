use wgpu::{BufferUsages, util::DeviceExt};

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

#[derive(Clone)]
pub struct Buffer {
    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: Option<wgpu::BindGroup>,
}

impl Buffer {
    pub fn new<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        instances: &[T],
        device: &wgpu::Device,
        buffer_type: BufferType,
    ) -> Self {
        let mut buffer = Buffer::new_layout(instances, device, &buffer_type);

        let bind_group = match buffer_type {
            BufferType::StorageBuffer(params) => {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &buffer.bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.buffer.as_entire_binding(),
                    }],
                    label: Some("Quad Color Bind Group"),
                })
            }
            BufferType::UniformBuffer(params) => {
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

    pub fn new_layout<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        instances: &[T],
        device: &wgpu::Device,
        buffer_type: &BufferType,
    ) -> Self {
        let (bind_group_layout, buffer) = match buffer_type {
            BufferType::StorageBuffer(params) => {
                let storage_buffer = if params.init {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Storage"),
                        contents: bytemuck::cast_slice(instances),
                        // usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        usage: params.usage,
                    })
                } else {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("output"),
                        size: std::mem::size_of_val(instances) as u64,
                        usage: params.usage,
                        mapped_at_creation: false,
                    })
                };
                let storage_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        entries: &[wgpu::BindGroupLayoutEntry {
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
                        }],
                        label: None,
                    });

                (storage_bind_group_layout, storage_buffer)
            }
            BufferType::UniformBuffer(params) => {
                let uniform_buffer = if params.init {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Storage"),
                        contents: bytemuck::cast_slice(instances),
                        // usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        usage: params.usage,
                    })
                } else {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("output"),
                        size: std::mem::size_of_val(instances) as u64,
                        usage: params.usage,
                        mapped_at_creation: false,
                    })
                };
                let uniform_bind_group_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: params.shader_stages,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                        label: Some("light_bind_group_layout"),
                    });
                (uniform_bind_group_layout, uniform_buffer)
            }
        };

        Self {
            buffer,
            bind_group_layout,
            bind_group: None,
        }
    }
}
