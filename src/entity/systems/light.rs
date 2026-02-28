use cgmath::Vector3;
use wgpu::util::DeviceExt;

use crate::entity::core::{
    storage_buffer::StorageBuffer,
    system::{GpuBindable, System},
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}
#[derive(Clone)]
pub struct Light {
    pub position: Vector3<f32>,
    color: Vector3<f32>,
}

impl Light {
    pub fn to_raw(&self) -> LightUniform {
        LightUniform {
            position: cgmath::Vector3::from(self.position).into(),
            _padding: 0,
            color: cgmath::Vector3::from(self.color).into(),
            _padding2: 0,
        }
    }
}
pub struct LightSystem {
    pub lights: Vec<Light>,
    pub storage_buffer: StorageBuffer,
}

impl LightSystem {
    pub fn init(lights: Vec<Light>, device: &wgpu::Device) -> Self {
        let lights_uniform: Vec<LightUniform> = lights.clone().iter().map(Light::to_raw).collect();
        let storage_buffer = StorageBuffer::new_layout(&lights_uniform, device);

        Self {
            lights,
            storage_buffer,
        }
    }
}

impl GpuBindable for LightSystem {
    fn get_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.storage_buffer.storage_bind_group_layout
    }
}

impl System for LightSystem {
    fn make_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.storage_buffer.storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.storage_buffer.storage_buffer.as_entire_binding(),
            }],
            label: Some("Quad Color Bind Group"),
        })
    }

    fn get_system_name(&self) -> String {
        "Light System".to_string()
    }
}
