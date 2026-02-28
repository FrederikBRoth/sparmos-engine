use wgpu::{BindGroupLayout, RenderPipeline, SurfaceConfiguration, TextureFormat};

use crate::{
    core::camera::{Camera, CameraController},
    entity::{
        self,
        core::{geometry::Mesh, instance::InstanceController, light::Light},
        texture::Texture,
    },
};

//
pub struct Material {
    pub pipeline: RenderPipeline,
}
pub struct RenderContext {
    pub light: BindGroupLayout,
    pub camera: BindGroupLayout,
    pub surface_config: SurfaceConfiguration,
}
pub fn create_material(
    mesh: &Mesh,
    instance_controller: &InstanceController,
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    texture: &Option<entity::texture::Texture>,
    global_layouts: &RenderContext,
    extra_bind_groups: &Vec<BindGroupLayout>,
) -> Material {
    //First check is if a texture was passed to the material. If it was, do a textured pipeline, if
    //not go primitive
    let layout = match texture {
        Some(texture) => {
            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[
                        &global_layouts.camera,
                        &global_layouts.light,
                        &texture.bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                });

            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        mesh.buffer_layout.to_wgpu(),
                        instance_controller.buffer_layout.to_wgpu(),
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: global_layouts.surface_config.format,
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
                    count: 2,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });
            render_pipeline
        }
        None => {
            let render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[&global_layouts.camera, &global_layouts.light],
                    push_constant_ranges: &[],
                });

            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        mesh.buffer_layout.to_wgpu(),
                        instance_controller.buffer_layout.to_wgpu(),
                    ],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: global_layouts.surface_config.format,
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
            });

            render_pipeline
        }
    };

    Material { pipeline: layout }
}
