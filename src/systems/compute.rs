use std::{any::TypeId, sync::Arc};

use indexmap::IndexMap;
use slotmap::SlotMap;
use tokio::sync::mpsc::channel;
use wgpu::{BindGroupLayout, ComputePipeline, RenderPipeline};

use crate::core::{
    buffer::Buffer,
    render::{ComputeHandle, RenderContext},
    resource::System,
};

#[derive(Clone)]
pub struct Compute {
    pub pending: bool,
    pub pipeline: ComputePipeline,
    pub input_buffer: Buffer,
    pub output_buffer: Buffer,
    pub temp_buffer: wgpu::Buffer,
    pub length: u32,
}

//marker struct for ECS
#[derive(Clone)]
pub struct ComputeObject;

impl Compute {
    pub fn new(
        render_context: &mut RenderContext,
        input_buffer: Buffer,
        output_buffer: Buffer,
        shader: &str,
        length: usize,
    ) -> Compute {
        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> = Vec::new();
        bind_group_layouts.push(Some(&input_buffer.bind_group_layout));
        bind_group_layouts.push(Some(&output_buffer.bind_group_layout));

        let temp_buffer = render_context
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("temp"),
                size: input_buffer.buffer.size(),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

        let shader = render_context.shaders.get(shader).unwrap();
        let render_pipeline_layout =
            render_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &bind_group_layouts,
                    ..Default::default() // push_constant_ranges: &[],
                });

        let pipeline =
            render_context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Introduction Compute Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    module: &shader,
                    entry_point: None,
                    compilation_options: Default::default(),
                    cache: Default::default(),
                });
        let compute = Compute {
            pending: false,
            pipeline,
            input_buffer: input_buffer,
            output_buffer: output_buffer,
            temp_buffer,
            length: length as u32,
        };
        compute
    }
}

pub struct ComputeSystem {
    computes: SlotMap<ComputeHandle, Compute>,
}

impl ComputeSystem {
    pub fn get(&mut self, handle: ComputeHandle) -> Option<&mut Compute> {
        self.computes.get_mut(handle)
    }

    pub fn add(
        &mut self,
        render_context: &mut RenderContext,
        input_buffer: Buffer,
        output_buffer: Buffer,
        shader: &str,
        length: usize,
    ) -> ComputeHandle {
        let compute = Compute::new(render_context, input_buffer, output_buffer, shader, length);
        self.computes.insert(compute)
    }

    pub fn new() -> Self {
        Self {
            computes: SlotMap::with_key(),
        }
    }
}

impl System for ComputeSystem {
    fn get_system_name(&self) -> String {
        todo!()
    }

    fn register(self, resources: &mut crate::core::resource::Resources, _device: &wgpu::Device) {
        let type_id = TypeId::of::<Self>();
        resources.resource_map.insert(type_id, Box::new(self));
    }
}
