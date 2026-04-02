use std::{collections::binary_heap, str::RSplitTerminator, sync::Arc};

use indexmap::IndexMap;
use wgpu::{
    BindGroup, BindGroupLayout, Device, Queue, RenderPipeline, Texture, TextureFormat, TextureView,
};
use winit::dpi::PhysicalSize;

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub enum Effect {
    None,
    ChromaticAberration,
    ChromaticTwo,
}
pub struct PostProcess {
    pub view: TextureView,
    pub bind_group: BindGroup,
    pub pipeline: RenderPipeline,
    pub format: TextureFormat,
}

pub struct PostProcessHandler {
    device: Arc<Device>,
    queue: Arc<Queue>,
    pub post_processes: IndexMap<Effect, PostProcess>,
}

impl PostProcessHandler {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            device,
            queue,
            post_processes: IndexMap::new(),
        }
    }
    pub fn new_effect(
        &mut self,
        screen_size: PhysicalSize<u32>,
        format: TextureFormat,
        effect: Effect,
    ) {
        let render_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PostProcess Texture"),
            size: wgpu::Extent3d {
                width: (screen_size.width as f32 * 1.1) as u32,
                height: (screen_size.height as f32 * 1.1) as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = render_texture.create_view(&Default::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,

            ..Default::default()
        });
        let post_process_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("post_process_layout"),
                    entries: &[
                        // Texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &post_process_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("post_process_bind_group"),
        });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("post_process_pipeline_layout"),
                bind_group_layouts: &[Some(&post_process_layout)],
                ..Default::default()
            });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("post_process_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("post_processing_shaders/chromatic_aberration.wgsl").into(),
                ),
            });

        let post_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("post_process_pipeline"),
                layout: Some(&pipeline_layout),

                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },

                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: format, // MUST match swapchain
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),

                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None, // 👈 important for fullscreen
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },

                depth_stencil: None, // 👈 no depth for post process

                multisample: wgpu::MultisampleState {
                    count: 1,
                    ..Default::default()
                },

                multiview_mask: None,
                cache: None,
            });

        let pp = PostProcess {
            view,
            bind_group,
            pipeline: post_pipeline,
            format: format,
        };
        self.post_processes.insert(effect, pp);
    }

    pub fn resize(&mut self, screen_size: PhysicalSize<u32>) {
        for post_process in self.post_processes.values_mut() {
            let render_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("PostProcess Texture"),
                size: wgpu::Extent3d {
                    width: screen_size.width,
                    height: screen_size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: post_process.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = render_texture.create_view(&Default::default());
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: 8,

                ..Default::default()
            });
            let post_process_layout =
                self.device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("post_process_layout"),
                        entries: &[
                            // Texture
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                },
                                count: None,
                            },
                            // Sampler
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &post_process_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: Some("post_process_bind_group"),
            });
            post_process.view = view;
            post_process.bind_group = bind_group;
        }
    }
}
