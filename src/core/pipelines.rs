use indexmap::IndexMap;
use log::warn;
use wgpu::{
    BindGroup, BindGroupLayout, Device, PipelineLayout, RenderPipeline, ShaderModule, TextureFormat,
};

use crate::{
    application::graphics::Graphics,
    core::{
        buffer::{Buffer, BufferKey, BufferType, UniformParameters},
        geometry::{Vertex, VertexBufferLayoutOwned, VertexLayoutKey},
        render::{ComputeHandle, ComputeRenderingHandle, MaterialHandle},
        resource::BufferHandle,
        texture::Texture,
    },
};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct MaterialKey {
    pub buffers: Vec<BufferKey>,
    pub textures: Vec<String>,
    pub vertex_layout: VertexLayoutKey,
    pub instance_layout: VertexLayoutKey,
    pub shader: String,
    pub target_format: Option<wgpu::TextureFormat>,
}

#[derive(Clone)]
pub struct Material {
    pub key: MaterialKey,
    pub pipeline_layout: PipelineLayout,
    pub pipeline: RenderPipeline,
    pub texture: Vec<Texture>,
    pub buffers: IndexMap<u32, Buffer>,
    pub ic_buffer_layout: VertexBufferLayoutOwned,
    pub mesh_buffer_layout: VertexBufferLayoutOwned,
    pub compute_bind_group: Option<BindGroup>,
}

impl Material {
    pub fn change_shader(&mut self, device: &Device, format: TextureFormat, shader: &ShaderModule) {
        let target_format = self.key.target_format.unwrap_or(format);
        let new_pipeline = create_pipeline(
            target_format,
            device,
            &self.pipeline_layout,
            &self.texture,
            shader,
            &self.mesh_buffer_layout,
            &self.ic_buffer_layout,
            &PipelineConfig::default(),
        );
        self.pipeline = new_pipeline;
    }
}

pub struct PipelineConfig {
    pub culling: Option<wgpu::Face>,
    pub depth_enabled: Option<bool>,
    pub depth_compare: Option<wgpu::CompareFunction>,
    pub target_format: Option<wgpu::TextureFormat>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            culling: Some(wgpu::Face::Back),
            depth_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            target_format: None,
        }
    }
}
pub struct MaterialBuilder<'a> {
    pub(crate) graphics: &'a mut Graphics,
    pub(crate) buffers: IndexMap<u32, Buffer>,
    pub(crate) textures: Vec<Texture>,
    pub(crate) shader: String,
    pub(crate) vertex_layout: VertexBufferLayoutOwned,
    pub(crate) instance_layout: VertexBufferLayoutOwned,
    pub(crate) compute_render_buffer: Option<Buffer>,
    pub(crate) config: PipelineConfig,
}

impl<'a> MaterialBuilder<'a> {
    fn key(&self) -> MaterialKey {
        let texture_key = self
            .textures
            .iter()
            .map(|texture| texture.label.clone())
            .collect();

        let buffers = self
            .buffers
            .values()
            .map(|buffer| buffer.key.clone())
            .collect::<Vec<BufferKey>>();

        MaterialKey {
            buffers,
            textures: texture_key,
            vertex_layout: self.vertex_layout.key(),
            instance_layout: self.instance_layout.key(),
            shader: self.shader.clone(),
            target_format: self.config.target_format,
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
        self.compute_render_buffer = Some(
            self.graphics
                .engine
                .render_context
                .gpu_objects
                .get_compute_mut(handle)
                .unwrap()
                .render_buffer
                .clone(),
        );
        self
    }

    pub fn config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn texture_from_color(mut self, color: [f32; 3], label: &str) -> Self {
        let texture = self.graphics.texture(label).color(color).build();
        self.textures.push(texture);
        self
    }

    pub fn texture(mut self, texture: Texture) -> Self {
        self.textures.push(texture);
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
            println!("{:?} clashes with another implemented material", key);
            return handle;
        }
        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> = Vec::new();

        for system in self.graphics.engine.systems.get_bind_group_layouts() {
            bind_group_layouts.push(system);
        }
        for texture in self.textures.iter() {
            bind_group_layouts.push(Some(&texture.bind_group_layout));
        }
        for buffer in self.buffers.values() {
            bind_group_layouts.push(Some(&buffer.bind_group_layout));
        }
        if let Some(compute_layout) = &self.compute_render_buffer {
            bind_group_layouts.push(Some(&compute_layout.bind_group_layout));
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

        let target_format = self
            .config
            .target_format
            .unwrap_or(self.graphics.engine.render_context.config.format);
        let pipeline = create_pipeline(
            target_format,
            &self.graphics.engine.render_context.device,
            &render_pipeline_layout,
            &self.textures,
            shader,
            &self.vertex_layout,
            &self.instance_layout,
            &self.config,
        );
        let compute_bind = self.compute_render_buffer.map(|c| c.bind_group);
        let material = Material {
            key: key.clone(),
            pipeline,
            pipeline_layout: render_pipeline_layout,
            texture: self.textures.clone(),
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
    texture: &[Texture],
    shader: &ShaderModule,
    mesh: &VertexBufferLayoutOwned,
    instance_controller: &VertexBufferLayoutOwned,
    config: &PipelineConfig,
) -> RenderPipeline {
    if !texture.is_empty() {
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
                cull_mode: config.culling,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: config.depth_enabled,
                depth_compare: config.depth_compare,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            // depth_stencil: None,
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
                cull_mode: config.culling,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: config.depth_enabled,
                depth_compare: config.depth_compare,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            // depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct ComputeRenderingKey {
    pub compute: ComputeHandle,
    pub shader: String,
}

#[derive(Clone)]
pub struct ComputeRendering {
    pub key: ComputeRenderingKey,

    pub pipeline_layout: PipelineLayout,
    pub pipeline: RenderPipeline,
    pub length: u32,
    pub compute_bind_group: BindGroup,
    pub input_buffers: Vec<Buffer>,
}

pub struct ComputeRenderingBuilder<'a> {
    pub(crate) graphics: &'a mut Graphics,
    pub(crate) compute: ComputeHandle,
    pub(crate) input_buffers: Vec<Buffer>,
    pub(crate) shader: String,
    pub(crate) mesh_layout: Option<VertexBufferLayoutOwned>,
}

impl<'a> ComputeRenderingBuilder<'a> {
    pub fn new(graphics: &'a mut Graphics, compute: ComputeHandle) -> Self {
        Self {
            graphics,
            compute,
            shader: String::new(),
            mesh_layout: None,
            input_buffers: vec![],
        }
    }

    pub fn shader(mut self, shader: &str) -> Self {
        self.shader = shader.to_string();
        self
    }

    pub fn input_data<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        mut self,
        data: &[T],
    ) -> Self {
        let buffer = Buffer::new_init(
            data,
            self.graphics.get_device(),
            BufferType::UniformBuffer(UniformParameters::default()),
        );
        self.input_buffers.push(buffer);
        self
    }

    pub fn input_buffer(mut self, buffer: BufferHandle) -> Self {
        let buffer = self.graphics.engine.resources.buffers.get(buffer).unwrap();
        self.input_buffers.push(buffer.clone());
        self
    }

    fn key(&self) -> ComputeRenderingKey {
        ComputeRenderingKey {
            compute: self.compute,
            shader: self.shader.clone(),
        }
    }

    pub fn mesh<T: Vertex>(mut self) -> Self {
        self.mesh_layout = Some(T::layout());
        self
    }

    pub fn build(self) -> ComputeRenderingHandle {
        let device = &self.graphics.engine.render_context.device;

        let shader = self
            .graphics
            .engine
            .render_context
            .shaders
            .get(&self.shader)
            .unwrap();

        let compute = self
            .graphics
            .engine
            .render_context
            .gpu_objects
            .get_compute_mut(self.compute)
            .unwrap();

        let render_buffer = compute.render_buffer.clone();
        let length = compute.length.clone();
        let key = self.key();

        // Reuse existing ComputeRendering
        if let Some(handle) = self
            .graphics
            .engine
            .render_context
            .gpu_objects
            .get_compute_rendering(&key)
        {
            warn!("{:?} clashes with another compute rendering", key);

            return handle;
        }

        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> = Vec::new();

        for system in self.graphics.engine.systems.get_bind_group_layouts() {
            bind_group_layouts.push(system);
        }

        for buffer in &self.input_buffers {
            bind_group_layouts.push(Some(&buffer.bind_group_layout));
        }

        bind_group_layouts.push(Some(&render_buffer.bind_group_layout));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Rendering Pipeline Layout"),
            bind_group_layouts: &bind_group_layouts,
            ..Default::default()
        });

        let pipeline = self.create_compute_rendering_pipeline(device, &pipeline_layout, shader);

        let compute_rendering = ComputeRendering {
            key: key.clone(),
            length,
            pipeline_layout,
            pipeline,
            compute_bind_group: render_buffer.bind_group,
            input_buffers: self.input_buffers,
        };

        let handle = self
            .graphics
            .engine
            .render_context
            .gpu_objects
            .compute_renderings
            .insert(compute_rendering);

        self.graphics
            .engine
            .render_context
            .gpu_objects
            .compute_rendering_lookup
            .insert(key, handle);

        handle
    }

    fn create_compute_rendering_pipeline(
        &self,
        device: &Device,
        pipeline_layout: &PipelineLayout,
        shader: &ShaderModule,
    ) -> RenderPipeline {
        let mut buffers: Vec<Option<wgpu::VertexBufferLayout>> = vec![];

        if let Some(mesh) = &self.mesh_layout {
            buffers.push(mesh.to_wgpu());
        }
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compute Rendering Pipeline"),

            layout: Some(pipeline_layout),

            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),

                buffers: &buffers,

                compilation_options: Default::default(),
            },

            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),

                targets: &[Some(wgpu::ColorTargetState {
                    format: self.graphics.engine.render_context.config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],

                compilation_options: Default::default(),
            }),

            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,

                // Particles don't need back-face culling.
                cull_mode: None,

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
    }
}
