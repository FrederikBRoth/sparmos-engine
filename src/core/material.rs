use indexmap::IndexMap;
use log::warn;
use slotmap::new_key_type;
use wgpu::{BindGroupLayout, Device, PipelineLayout, RenderPipeline, ShaderModule, TextureFormat};

use crate::core::{
    buffer::{Buffer, BufferKey},
    geometry::{Vertex, VertexBufferLayoutOwned, VertexLayoutKey},
    instance::InstanceToRaw,
    render::{MaterialHandle, RenderContext},
    resource::Resources,
    texture::Texture,
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
pub struct MaterialBuilder {
    buffers: IndexMap<u32, Buffer>,
    texture: Option<Texture>,
    shader: String,
}

impl MaterialBuilder {
    fn key(
        &self,
        vertex_layout: &VertexBufferLayoutOwned,
        instance_layout: &VertexBufferLayoutOwned,
    ) -> MaterialKey {
        let texture_key = self.texture.as_ref().map(|texture| texture.label.clone());

        let buffers = self
            .buffers
            .values()
            .map(|buffer| buffer.key.clone())
            .collect::<Vec<BufferKey>>();

        MaterialKey {
            buffers,
            texture: texture_key,
            vertex_layout: vertex_layout.key(),
            instance_layout: instance_layout.key(),
            shader: self.shader.clone(),
        }
    }

    pub fn new() -> Self {
        MaterialBuilder {
            buffers: IndexMap::new(),
            texture: None,
            shader: String::new(),
        }
    }
    //Will lookup shader in Global Context
    pub fn add_shader(mut self, shader: &str) -> Self {
        self.shader = shader.to_string();
        self
    }

    pub fn add_buffer(mut self, handle: u32, buffer: Buffer) -> Self {
        self.buffers.insert(handle, buffer);
        self
    }

    pub fn add_texture(mut self, texture: Texture) -> Self {
        self.texture = Some(texture);
        self
    }

    pub fn build<V: Vertex, I: InstanceToRaw>(
        self,
        resources: &Resources,
        render_context: &mut RenderContext,
    ) -> MaterialHandle {
        let shader = render_context.shaders.get(&self.shader).unwrap();
        let vertex_layout = V::layout();
        let instance_layout = I::layout();

        let key = self.key(&vertex_layout, &instance_layout);

        //if this material exists already, just return the existing handle
        if let Some(handle) = render_context.gpu_objects.get_material(&key) {
            warn!("{:?} clashes with another implemented material", key);
            return handle;
        }
        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> = Vec::new();

        for system in resources.get_bind_group_layouts() {
            bind_group_layouts.push(system);
        }
        if let Some(texture) = &self.texture {
            bind_group_layouts.push(Some(&texture.bind_group_layout));
        }
        for buffer in self.buffers.values() {
            bind_group_layouts.push(Some(&buffer.bind_group_layout));
        }

        //First check is if a texture was passed to the material. If it was, do a textured pipeline, if
        //not go primitive
        let render_pipeline_layout =
            render_context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &bind_group_layouts,
                    ..Default::default() // push_constant_ranges: &[],
                });

        let pipeline = create_pipeline(
            render_context.config.format.clone(),
            &render_context.device,
            &render_pipeline_layout,
            &self.texture,
            shader,
            &vertex_layout,
            &instance_layout,
        );
        let material = Material {
            key: key.clone(),
            pipeline,
            pipeline_layout: render_pipeline_layout,
            texture: self.texture.clone(),
            //TODO FIX
            buffers: self.buffers.clone(),
            ic_buffer_layout: instance_layout,
            mesh_buffer_layout: vertex_layout,
        };
        let handle = render_context.gpu_objects.materials.insert(material);
        render_context
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

impl Default for MaterialBuilder {
    fn default() -> Self {
        Self::new()
    }
}
