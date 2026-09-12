use cgmath::Vector3;

use crate::core::{
    buffer::{Buffer, BufferType, UniformParameters},
    engine::ViewSystem,
    render::{
        render::RenderContext,
        render_view::{RenderView, RenderViewHandle},
    },
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
    pub intensity: f32,
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
    pub intensity: f32,
}

impl Light {
    pub fn to_raw(&self) -> LightUniform {
        LightUniform {
            position: self.position.into(),
            _padding: 0,
            color: self.color.into(),
            intensity: self.intensity,
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
        let storage_buffer = Buffer::new_init(
            &[light_block],
            device,
            BufferType::UniformBuffer(UniformParameters::default()),
        );
        Self { storage_buffer }
    }
}

impl ViewSystem for LightSystem {
    fn get_buffer(&self, _render_view: RenderViewHandle) -> &Buffer {
        &self.storage_buffer
    }

    fn binding_location(&self) -> (u32, u32) {
        (0, 1)
    }

    fn run(
        &mut self,
        _view: &mut RenderView,
        _resources: &mut RenderContext,
        _render_view: RenderViewHandle,
        _dt: std::time::Duration,
    ) {
    }

    fn binding_layout_entry(&self) -> wgpu::BindGroupLayoutEntry {
        self.storage_buffer.layout_entry(1)
    }

    // fn register(self, resources: &mut Resources) {
    //     let type_id = TypeId::of::<Self>();
    //
    //     resources.buffers.insert(self.storage_buffer.clone());
    //     resources.resource_map.insert(type_id, Box::new(self));
    // }
}
