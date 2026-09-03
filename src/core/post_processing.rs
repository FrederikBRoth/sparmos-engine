use std::sync::Arc;

use indexmap::IndexMap;
use wgpu::{BindGroup, Device, Queue, RenderPipeline, TextureFormat, TextureView};
use winit::dpi::PhysicalSize;

/// Render target enlargement used by the centered post-process crop.
pub const POST_PROCESS_OVERSCAN: f32 = 1.1;

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
    _queue: Arc<Queue>,
    pub post_processes: IndexMap<Effect, PostProcess>,
}

impl PostProcessHandler {
    pub fn overscan_size(screen_size: PhysicalSize<u32>) -> PhysicalSize<u32> {
        PhysicalSize::new(
            (screen_size.width as f32 * POST_PROCESS_OVERSCAN) as u32,
            (screen_size.height as f32 * POST_PROCESS_OVERSCAN) as u32,
        )
    }

    /// Map final display NDC back through the active centered crops.
    pub fn display_to_render_ndc_scale(&self) -> f32 {
        // Every currently implemented pass uses the same crop shader. An empty
        // chain renders directly to the surface and leaves NDC unchanged.
        self.post_processes
            .values()
            .fold(1.0, |scale, _| scale / POST_PROCESS_OVERSCAN)
    }

    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            device,
            _queue: queue,
            post_processes: IndexMap::new(),
        }
    }
    /// Create an effect from the normal window size; overscan is applied internally.
    pub fn new_effect(
        &mut self,
        screen_size: PhysicalSize<u32>,
        format: TextureFormat,
        effect: Effect,
    ) {
        let render_size = Self::overscan_size(screen_size);
        let render_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PostProcess Texture"),
            size: wgpu::Extent3d {
                width: render_size.width,
                height: render_size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
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
                        format, // MUST match swapchain
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[("post_process_overscan", POST_PROCESS_OVERSCAN as f64)],
                        ..Default::default()
                    },
                }),

                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },

                depth_stencil: None,

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
            format,
        };
        self.post_processes.insert(effect, pp);
    }

    /// Resize from the normal window size, using the same overscan as creation.
    pub fn resize(&mut self, screen_size: PhysicalSize<u32>) {
        let render_size = Self::overscan_size(screen_size);
        for post_process in self.post_processes.values_mut() {
            let render_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("PostProcess Texture"),
                size: wgpu::Extent3d {
                    width: render_size.width,
                    height: render_size.height,
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
