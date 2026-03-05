use indexmap::IndexMap;
use wgpu::{BindGroupLayout, RenderPipeline};

use crate::entity::{
    core::{
        geometry::{Mesh, VertexBufferLayoutOwned},
        instance::InstanceController,
        render::RenderContext,
        resource::GpuBindable,
    },
    texture::Texture,
};

#[derive(Clone)]
pub struct Material {
    pub pipeline: RenderPipeline,
    pub texture: Option<Texture>,
    pub layouts: IndexMap<String, BindGroupLayout>,
}

pub struct MaterialBuilder {
    layouts: IndexMap<String, BindGroupLayout>,
    texture: Option<Texture>,
    shader: String,
}

impl MaterialBuilder {
    pub fn new() -> Self {
        MaterialBuilder {
            layouts: IndexMap::new(),
            texture: None,
            shader: String::new(),
        }
    }

    pub fn add_layout<T: GpuBindable>(&mut self, name: &str, bindable: &T) -> &mut Self {
        self.layouts
            .insert(name.to_string(), bindable.get_bind_group_layout().clone());
        self
    }

    pub fn add_layout_raw(&mut self, name: &str, layout: &BindGroupLayout) -> &mut Self {
        self.layouts.insert(name.to_string(), layout.clone());
        self
    }

    //Will lookup shader in Global Context
    pub fn add_shader(&mut self, shader: &str) -> &mut Self {
        self.shader = shader.to_string();
        self
    }

    pub fn add_texture(&mut self, texture: Texture) -> &mut Self {
        self.layouts.insert(
            texture.label.clone(),
            texture.get_bind_group_layout().clone(),
        );
        self.texture = Some(texture);
        self
    }

    pub fn build(
        &self,
        mesh: &VertexBufferLayoutOwned,
        render_context: &RenderContext,
        instance_controller: &VertexBufferLayoutOwned,
    ) -> Material {
        let bind_group_layouts: Vec<&BindGroupLayout> =
            self.layouts.iter().map(|(_, v)| v).collect();
        let shader = render_context.shaders.get(&self.shader).unwrap();
        //First check is if a texture was passed to the material. If it was, do a textured pipeline, if
        //not go primitive
        let pipeline = if self.texture.is_some() {
            let render_pipeline_layout =
                render_context
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Render Pipeline Layout"),
                        bind_group_layouts: &bind_group_layouts,
                        push_constant_ranges: &[],
                    });

            render_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                            format: render_context.config.format,
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
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                    cache: None,
                })
        } else {
            let render_pipeline_layout =
                render_context
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Render Pipeline Layout"),
                        bind_group_layouts: &bind_group_layouts,
                        push_constant_ranges: &[],
                    });

            render_context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                            format: render_context.config.format,
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
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less, // standard depth test
                        stencil: wgpu::StencilState::default(),     // no stencil operations
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
                    multiview: None,
                    // Useful for optimizing shader compilation on Android
                    cache: None,
                })
        };

        Material {
            pipeline,
            layouts: self.layouts.clone(),
            texture: self.texture.clone(),
        }
    }
}

impl Default for MaterialBuilder {
    fn default() -> Self {
        Self::new()
    }
}
