use indexmap::IndexMap;
use slotmap::{SlotMap, new_key_type};
use wgpu::{BindGroupLayout, RenderPipeline};

use crate::entity::{
    core::{
        buffer::Buffer,
        geometry::{Mesh, VertexBufferLayoutOwned},
        instance::InstanceController,
        render::{InstanceControllerHandle, MaterialHandle, MeshHandle, RenderContext},
        resource::GpuBindable,
    },
    texture::Texture,
};

#[derive(Clone)]
pub struct Material {
    pub pipeline: RenderPipeline,
    pub texture: Option<Texture>,
    pub layouts: IndexMap<String, BindGroupLayout>,
    pub buffers: IndexMap<u32, Buffer>,
}

new_key_type! { pub struct BufferHandle; }
pub struct MaterialBuilder {
    layouts: IndexMap<String, BindGroupLayout>,
    buffers: IndexMap<u32, Buffer>,

    texture: Option<Texture>,
    shader: String,
}

impl MaterialBuilder {
    pub fn new() -> Self {
        MaterialBuilder {
            layouts: IndexMap::new(),
            buffers: IndexMap::new(),
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

    pub fn add_buffer(&mut self, handle: u32, buffer: Buffer) -> &mut Self {
        self.buffers.insert(handle, buffer);
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
        mesh: &MeshHandle,
        instance_controller: &InstanceControllerHandle,
        render_context: &mut RenderContext,
    ) -> MaterialHandle {
        let mesh = &render_context
            .gpu_objects
            .meshes
            .get(*mesh)
            .unwrap()
            .buffer_layout;
        let instance_controller = render_context
            .gpu_objects
            .instance_controllers
            .get(*instance_controller)
            .unwrap()
            .layout();

        let mut bind_group_layouts: Vec<Option<&BindGroupLayout>> =
            self.layouts.iter().map(|(_, v)| Some(v)).collect();
        for buffer in self.buffers.values() {
            bind_group_layouts.push(Some(&buffer.bind_group_layout));
        }
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
                        ..Default::default() // push_constant_ranges: &[],
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
            let render_pipeline_layout =
                render_context
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("Render Pipeline Layout"),
                        bind_group_layouts: &bind_group_layouts,
                        ..Default::default()
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
        };

        let material = Material {
            pipeline,
            layouts: self.layouts.clone(),
            texture: self.texture.clone(),
            //TODO FIX
            buffers: self.buffers.clone(),
        };
        render_context.gpu_objects.materials.insert(material)
    }
}

impl Default for MaterialBuilder {
    fn default() -> Self {
        Self::new()
    }
}
