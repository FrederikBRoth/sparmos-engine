use wgpu::{BufferUsages, ComputePipeline, ShaderStages};

use crate::{
    application::graphics::Graphics,
    core::{
        binding::BindGroupBuilder,
        buffer::{Buffer, BufferType, StorageParameters, UniformParameters},
        render::{ComputeHandle, RenderContext},
    },
};

#[derive(Clone, PartialEq)]
pub enum ReadbackState {
    NoReadback,
    Pending,
    Available,
}
#[derive(Clone)]
pub struct Compute {
    pub readback_status: ReadbackState,
    pub pipeline: ComputePipeline,
    pub input_buffers: Vec<Buffer>,
    pub output_buffer: Buffer,
    pub render_buffer: Buffer,
    pub bind_groups: Vec<Option<wgpu::BindGroup>>,

    pub temp_buffer: Option<wgpu::Buffer>,
    pub length: u32,
    pub data: Vec<u8>,
}

pub struct ComputeBuilder<'a> {
    pub(crate) gfx: &'a mut Graphics,
    pub(crate) output_object_size: usize,
    pub(crate) size: usize,
    pub(crate) input_buffers: Vec<Buffer>,
    pub(crate) shader: String,
    pub(crate) readback: ReadbackState,
    pub(crate) initial_data: Option<Buffer>,
}

impl<'a> ComputeBuilder<'a> {
    pub fn size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }
    pub fn input_buffer<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        mut self,
        data: &[T],
    ) -> Self {
        let buffer = Buffer::new_init(
            data,
            self.gfx.get_device(),
            BufferType::UniformBuffer(UniformParameters {
                shader_stages: ShaderStages::COMPUTE,
                ..Default::default()
            }),
        );
        self.input_buffers.push(buffer);
        self
    }

    pub fn shader(mut self, shader: &str) -> Self {
        self.shader = shader.to_string();
        self
    }

    pub fn initial_data<T: bytemuck::Pod>(mut self, data: &[T]) -> Self {
        self.initial_data = Some(Buffer::new_init(
            data,
            self.gfx.get_device(),
            BufferType::StorageBuffer(StorageParameters {
                shader_stages: ShaderStages::COMPUTE,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                read_only: false,
                ..Default::default()
            }),
        ));

        self
    }
    pub fn readback(mut self) -> Self {
        self.readback = ReadbackState::Available;
        self
    }

    pub fn build(self) -> ComputeHandle {
        let output_buffer = if let Some(buffer) = self.initial_data {
            buffer
        } else {
            Buffer::new(
                self.output_object_size,
                self.size,
                self.gfx.get_device(),
                BufferType::StorageBuffer(StorageParameters {
                    shader_stages: ShaderStages::COMPUTE,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                    init: false,
                    read_only: false,
                }),
            )
        };

        //Preallocate output data for fast writing
        let output_size = self.output_object_size * self.size;
        let data = vec![0u8; output_size];
        let compute = Compute::new(
            self.gfx.get_render_context_mut(),
            self.input_buffers,
            output_buffer,
            &self.shader,
            self.size,
            data,
            self.readback,
        );
        self.gfx
            .engine
            .render_context
            .gpu_objects
            .computes
            .insert(compute)
    }
}

//marker struct for ECS
#[derive(Clone)]
pub struct ComputeObject;

impl Compute {
    pub fn new(
        render_context: &mut RenderContext,
        input_buffers: Vec<Buffer>,
        output_buffer: Buffer,
        shader: &str,
        length: usize,
        data: Vec<u8>,
        readback_state: ReadbackState,
    ) -> Compute {
        let mut bindings = BindGroupBuilder::new();
        for (group, buffer) in input_buffers.iter().enumerate() {
            bindings.buffer(buffer, group as u32, 0);
        }
        bindings.buffer(&output_buffer, input_buffers.len() as u32, 0);
        let built_bindings = bindings.build(&render_context.device, "compute bind group");
        let bind_group_layouts = built_bindings
            .layouts
            .iter()
            .map(Option::as_ref)
            .collect::<Vec<_>>();

        let render_buffer = Buffer::from_existing(
            &output_buffer,
            &render_context.device,
            BufferType::StorageBuffer(StorageParameters {
                read_only: true,
                shader_stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ..Default::default()
            }),
        );
        let temp_buffer = match readback_state {
            ReadbackState::NoReadback => None,
            ReadbackState::Available | ReadbackState::Pending => Some(
                render_context
                    .device
                    .create_buffer(&wgpu::BufferDescriptor {
                        label: Some("temp"),
                        size: output_buffer.buffer.size(),
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
            ),
        };
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
                    module: shader,
                    entry_point: None,
                    compilation_options: Default::default(),
                    cache: Default::default(),
                });

        Compute {
            readback_status: readback_state,
            pipeline,
            input_buffers,
            output_buffer,
            temp_buffer,
            length: length as u32,
            data,
            render_buffer,
            bind_groups: built_bindings.bind_groups,
        }
    }
    pub fn read_result(&mut self, data: Result<wgpu::BufferView, wgpu::MapRangeError>) {
        let mapped = data.unwrap();
        self.data.copy_from_slice(&mapped);
        println!("TEST DATA: {:?}", self.data);
    }

    pub fn result_as<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(&self) -> &[T] {
        bytemuck::cast_slice(&self.data)
    }
}
