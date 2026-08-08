use indexmap::IndexMap;
use wgpu::{BindGroupLayout, ComputePipeline, RenderPipeline};

use crate::core::{
    buffer::Buffer,
    render::{ComputeHandle, RenderContext},
};

#[derive(Clone)]
pub struct Compute {
    pub pipeline: ComputePipeline,
    pub input_buffer: Buffer,
    pub output_buffer: Buffer,
    pub temp_buffer: wgpu::Buffer,
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
    ) -> ComputeHandle {
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
        let material = Compute {
            pipeline,
            input_buffer: input_buffer,
            output_buffer: output_buffer,
            temp_buffer,
        };
        render_context.gpu_objects.computes.insert(material)
    }
}
