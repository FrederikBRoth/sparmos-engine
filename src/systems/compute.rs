use std::{any::TypeId, mem};

use slotmap::SlotMap;
use wgpu::{BindGroupLayout, BufferUsages, ComputePipeline, ShaderStages};

use crate::{
    application::graphics::Graphics,
    core::{
        buffer::{Buffer, BufferType, StorageParameters},
        render::{ComputeHandle, RenderContext},
        resource::System,
    },
};

#[derive(Clone)]
pub struct Compute {
    pub pending: bool,
    pub pipeline: ComputePipeline,
    pub input_buffers: Vec<Buffer>,
    pub output_buffer: Buffer,
    pub temp_buffer: wgpu::Buffer,
    pub length: u32,
    pub data: Vec<u8>,
}

pub struct ComputeBuilder<'a> {
    pub(crate) gfx: &'a mut Graphics,
    pub(crate) output_object_size: usize,
    pub(crate) size: usize,
    pub(crate) input_buffers: Vec<Buffer>,
    pub(crate) shader: String,
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
        assert_eq!(data.len(), self.size);
        let buffer = Buffer::new_init(
            data,
            &self.gfx.get_device(),
            BufferType::StorageBuffer(StorageParameters {
                shader_stages: ShaderStages::COMPUTE,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
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

    pub fn build(self) -> Compute {
        let output_buffer = Buffer::new(
            self.output_object_size,
            self.size,
            self.gfx.get_device(),
            BufferType::StorageBuffer(StorageParameters {
                shader_stages: ShaderStages::COMPUTE,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                init: false,
                read_only: false,
            }),
        );

        //Preallocate output data for fast writing
        let output_size = self.output_object_size * self.size;
        let data = vec![0u8; output_size];
        Compute::new(
            self.gfx.get_render_context_mut(),
            self.input_buffers,
            output_buffer,
            &self.shader,
            self.size,
            data,
        )
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
    ) -> Compute {
        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> = Vec::new();
        for buffer in input_buffers.iter() {
            bind_group_layouts.push(Some(&buffer.bind_group_layout));
        }
        bind_group_layouts.push(Some(&output_buffer.bind_group_layout));

        let temp_buffer = render_context
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("temp"),
                size: output_buffer.buffer.size(),
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
            input_buffers: input_buffers,
            output_buffer: output_buffer,
            temp_buffer,
            length: length as u32,
            data,
        };
        compute
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

pub struct ComputeSystem {
    computes: SlotMap<ComputeHandle, Compute>,
}

impl ComputeSystem {
    pub fn get(&mut self, handle: ComputeHandle) -> Option<&mut Compute> {
        self.computes.get_mut(handle)
    }

    pub fn add(&mut self, compute: Compute) -> ComputeHandle {
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

    fn register(self, resources: &mut crate::core::resource::Resources) {
        let type_id = TypeId::of::<Self>();
        resources.resource_map.insert(type_id, Box::new(self));
    }
}
