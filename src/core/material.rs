use indexmap::IndexMap;
use log::warn;
use slotmap::new_key_type;
use wgpu::{
    BindGroup, BindGroupLayout, Device, PipelineLayout, RenderPipeline, ShaderModule, TextureFormat,
};

use crate::{
    application::graphics::Graphics,
    core::{
        buffer::{Buffer, BufferKey},
        geometry::{VertexBufferLayoutOwned, VertexLayoutKey},
        render::{ComputeHandle, MaterialHandle},
        texture::Texture,
    },
    systems::compute::ComputeSystem,
};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct MaterialKey {
    pub buffers: Vec<BufferKey>,
    pub texture: Option<String>,
    pub vertex_layout: VertexLayoutKey,
    pub instance_layout: VertexLayoutKey,
    pub shader: String,
}

#[derive(Clone)]
pub struct Material {
    pub key: MaterialKey,
    pub pipeline_layout: PipelineLayout,
    pub pipeline: RenderPipeline,
    pub texture: Option<Texture>,
    pub buffers: IndexMap<u32, Buffer>,
    pub ic_buffer_layout: VertexBufferLayoutOwned,
    pub mesh_buffer_layout: VertexBufferLayoutOwned,
    pub compute_bind_group: Option<BindGroup>,
}

impl Material {
    pub fn change_shader(&mut self, device: &Device, format: TextureFormat, shader: &ShaderModule) {
        let new_pipeline = create_pipeline(
            format,
            device,
            &self.pipeline_layout,
            &self.texture,
            shader,
            &self.mesh_buffer_layout,
            &self.ic_buffer_layout,
        );
        self.pipeline = new_pipeline;
    }
}

new_key_type! { pub struct BufferHandle; }
pub struct MaterialBuilder<'a> {
    pub(crate) graphics: &'a mut Graphics,
    pub(crate) buffers: IndexMap<u32, Buffer>,
    pub(crate) texture: Option<Texture>,
    pub(crate) shader: String,
    pub(crate) vertex_layout: VertexBufferLayoutOwned,
    pub(crate) instance_layout: VertexBufferLayoutOwned,
    pub(crate) compute_layout: Option<(BindGroupLayout, BindGroup)>,
}

impl<'a> MaterialBuilder<'a> {
    fn key(&self) -> MaterialKey {
        let texture_key = self.texture.as_ref().map(|texture| texture.label.clone());

        let buffers = self
            .buffers
            .values()
            .map(|buffer| buffer.key.clone())
            .collect::<Vec<BufferKey>>();

        MaterialKey {
            buffers,
            texture: texture_key,
            vertex_layout: self.vertex_layout.key(),
            instance_layout: self.instance_layout.key(),
            shader: self.shader.clone(),
        }
    }

    //Will lookup shader in Global Context
    pub fn shader(mut self, shader: &str) -> Self {
        self.shader = shader.to_string();
        self
    }

    pub fn buffer(mut self, handle: u32, buffer: Buffer) -> Self {
        self.buffers.insert(handle, buffer);
        self
    }

    pub fn compute_buffer(mut self, handle: ComputeHandle) -> Self {
        self.compute_layout = Some(
            self.graphics
                .world
                .resources
                .get_system_mut::<ComputeSystem>()
                .unwrap()
                .get(handle)
                .unwrap()
                .render_bind_groups
                .clone(),
        );
        self
    }

    pub fn texture_from_color(mut self, color: [f32; 3], label: Option<&str>) -> Self {
        let texture = Texture::from_color(
            &self.graphics.engine.render_context.device,
            &self.graphics.engine.render_context.queue,
            color,
            label,
        )
        .unwrap();

        self.texture = Some(texture);
        self
    }

    pub fn build(self) -> MaterialHandle {
        let shader = self
            .graphics
            .engine
            .render_context
            .shaders
            .get(&self.shader)
            .unwrap();

        let key = self.key();

        //if this material exists already, just return the existing handle
        if let Some(handle) = self
            .graphics
            .engine
            .render_context
            .gpu_objects
            .get_material(&key)
        {
            warn!("{:?} clashes with another implemented material", key);
            return handle;
        }
        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> = Vec::new();

        for system in self.graphics.world.resources.get_bind_group_layouts() {
            bind_group_layouts.push(system);
        }
        if let Some(texture) = &self.texture {
            bind_group_layouts.push(Some(&texture.bind_group_layout));
        }
        for buffer in self.buffers.values() {
            bind_group_layouts.push(Some(&buffer.bind_group_layout));
        }
        if let Some(compute_layout) = &self.compute_layout {
            bind_group_layouts.push(Some(&compute_layout.0));
        }

        //First check is if a texture was passed to the material. If it was, do a textured pipeline, if
        //not go primitive
        let render_pipeline_layout = self
            .graphics
            .engine
            .render_context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &bind_group_layouts,
                ..Default::default() // push_constant_ranges: &[],
            });

        let pipeline = create_pipeline(
            self.graphics.engine.render_context.config.format.clone(),
            &self.graphics.engine.render_context.device,
            &render_pipeline_layout,
            &self.texture,
            shader,
            &self.vertex_layout,
            &self.instance_layout,
        );
        let compute_bind = self.compute_layout.map(|c| c.1);
        let material = Material {
            key: key.clone(),
            pipeline,
            pipeline_layout: render_pipeline_layout,
            texture: self.texture.clone(),
            //TODO FIX
            buffers: self.buffers.clone(),
            ic_buffer_layout: self.instance_layout,
            mesh_buffer_layout: self.vertex_layout,
            compute_bind_group: compute_bind,
        };
        let handle = self
            .graphics
            .engine
            .render_context
            .gpu_objects
            .materials
            .insert(material);
        self.graphics
            .engine
            .render_context
            .gpu_objects
            .material_lookup
            .insert(key, handle);
        handle
    }
}

fn create_pipeline(
    format: TextureFormat,
    device: &Device,
    render_pipeline_layout: &PipelineLayout,
    texture: &Option<Texture>,
    shader: &ShaderModule,
    mesh: &VertexBufferLayoutOwned,
    instance_controller: &VertexBufferLayoutOwned,
) -> RenderPipeline {
    if texture.is_some() {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[mesh.to_wgpu(), instance_controller.to_wgpu()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    } else {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[mesh.to_wgpu(), instance_controller.to_wgpu()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less), // standard depth test
                stencil: wgpu::StencilState::default(),           // no stencil operations
                bias: wgpu::DepthBiasState::default(),
            }),
            // depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview_mask: None,
            // Useful for optimizing shader compilation on Android
            cache: None,
        })
    }
}
