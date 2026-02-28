use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Color {
    pub color: [f32; 3],
    pub _pad: f32, // 4 bytes padding to align to 16 bytes total
}
pub struct StorageBuffer {
    pub instances: Vec<Color>,
    pub storage_buffer: wgpu::Buffer,
    pub storage_bind_group_layout: wgpu::BindGroupLayout,
    pub storage_bind_group: wgpu::BindGroup,
}

impl StorageBuffer {
    pub fn new(instances: Vec<Color>, device: &wgpu::Device) -> Self {
        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Storage"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let storage_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

        let storage_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }],
            label: Some("Quad Color Bind Group"),
        });

        Self {
            instances,
            storage_buffer,
            storage_bind_group_layout,
            storage_bind_group,
        }
    }
}
