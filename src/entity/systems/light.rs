use cgmath::Vector3;

use crate::entity::core::{
    buffer::{Buffer, BufferType},
    resource::{GpuBindable, System},
};

const MAX_LIGHTS: usize = 16;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct LightUniform {
    position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightBlock {
    pub lights: [LightUniform; MAX_LIGHTS],
    pub light_count: u32,
    pub _padding: [u32; 3], // 16-byte align
}
#[derive(Clone)]
pub struct Light {
    pub position: Vector3<f32>,
    pub color: Vector3<f32>,
}

impl Light {
    pub fn to_raw(&self) -> LightUniform {
        LightUniform {
            position: self.position.into(),
            _padding: 0,
            color: self.color.into(),
            _padding2: 0,
        }
    }

    pub fn to_raw_list(lights: &[Light]) -> LightBlock {
        let mut light_uniforms = [LightUniform::default(); MAX_LIGHTS];

        for (i, light) in lights.iter().take(MAX_LIGHTS).enumerate() {
            light_uniforms[i] = light.to_raw();
        }

        LightBlock {
            lights: light_uniforms,
            light_count: lights.len().min(MAX_LIGHTS) as u32,
            _padding: [0; 3], // if needed for alignment
        }
    }
}
pub struct LightSystem {
    pub storage_buffer: Buffer,
}

impl LightSystem {
    pub fn init(lights: &[Light], device: &wgpu::Device) -> Self {
        let light_block = Light::to_raw_list(lights);
        let storage_buffer = Buffer::new_layout(&[light_block], device, &BufferType::UniformBuffer);
        Self { storage_buffer }
    }
}

impl GpuBindable for LightSystem {
    fn get_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.storage_buffer.bind_group_layout
    }
}

impl System for LightSystem {
    fn make_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.storage_buffer.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.storage_buffer.buffer.as_entire_binding(),
            }],
            label: Some("Quad Color Bind Group"),
        })
    }

    fn get_system_name(&self) -> String {
        "Light System".to_string()
    }
}
