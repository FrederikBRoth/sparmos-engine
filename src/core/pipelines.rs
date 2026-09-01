use log::warn;
use std::collections::{BTreeMap, HashMap};
use wgpu::{
    BindGroup, BindGroupLayout, Device, PipelineLayout, RenderPipeline, ShaderModule, TextureFormat,
};

use crate::{
    application::graphics::Graphics,
    core::{
        binding::{BindGroupBuilder, MaterialBindingKey},
        buffer::{Buffer, BufferType, UniformParameters},
        geometry::{Vertex, VertexBufferLayoutOwned, VertexLayoutKey},
        render::{ComputeHandle, ComputeRenderingHandle, MaterialHandle, RenderContext},
        resource::BufferHandle,
        texture::Texture,
    },
};

#[derive(Clone)]
pub struct RenderPipelineState {
    pub pipeline: RenderPipeline,
    pub pipeline_layout: PipelineLayout,
}

pub struct RenderPipelineBuilder<'a> {
    device: &'a Device,
    shaders: Option<&'a HashMap<String, ShaderModule>>,
    shader: Option<&'a ShaderModule>,
    shader_name: Option<String>,
    label: String,
    bind_group_layouts: BTreeMap<u32, BindGroupLayout>,
    vertex_layouts: Vec<VertexBufferLayoutOwned>,
    config: PipelineConfig,
    primitive: wgpu::PrimitiveState,
    blend: Option<wgpu::BlendState>,
    pipeline_layout: Option<PipelineLayout>,
}

impl<'a> RenderPipelineBuilder<'a> {
    pub(crate) fn new(render_context: &'a RenderContext, label: &str) -> Self {
        Self::from_parts(
            &render_context.device,
            Some(&render_context.shaders),
            None,
            label,
        )
    }

    pub(crate) fn from_shader(device: &'a Device, label: &str, shader: &'a ShaderModule) -> Self {
        Self::from_parts(device, None, Some(shader), label)
    }

    fn from_parts(
        device: &'a Device,
        shaders: Option<&'a HashMap<String, ShaderModule>>,
        shader: Option<&'a ShaderModule>,
        label: &str,
    ) -> Self {
        Self {
            device,
            shaders,
            shader,
            shader_name: None,
            label: label.to_string(),
            bind_group_layouts: BTreeMap::new(),
            vertex_layouts: Vec::new(),
            config: PipelineConfig::default(),
            primitive: wgpu::PrimitiveState::default(),
            blend: Some(wgpu::BlendState::REPLACE),
            pipeline_layout: None,
        }
    }

    pub fn shader(mut self, shader: &str) -> Self {
        self.shader_name = Some(shader.to_string());
        self
    }

    pub fn target_format(mut self, format: TextureFormat) -> Self {
        self.config.target_format = Some(format);
        self
    }

    pub fn vertex_layout(mut self, layout: VertexBufferLayoutOwned) -> Self {
        self.vertex_layouts.push(layout);
        self
    }

    pub fn bind_group_layout(mut self, group: u32, layout: &BindGroupLayout) -> Self {
        assert!(
            self.bind_group_layouts
                .insert(group, layout.clone())
                .is_none(),
            "duplicate pipeline bind group layout at group {group}"
        );
        self
    }

    pub fn config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn primitive(mut self, primitive: wgpu::PrimitiveState) -> Self {
        self.config.culling = primitive.cull_mode;
        self.primitive = primitive;
        self
    }

    pub fn blend(mut self, blend: Option<wgpu::BlendState>) -> Self {
        self.blend = blend;
        self
    }

    pub fn depth(mut self, enabled: bool) -> Self {
        self.config.depth_enabled = enabled.then_some(true);
        self
    }

    pub(crate) fn existing_pipeline_layout(mut self, layout: &PipelineLayout) -> Self {
        self.pipeline_layout = Some(layout.clone());
        self
    }

    pub fn build(self) -> RenderPipelineState {
        let shader = self.shader.unwrap_or_else(|| {
            let shader_name = self
                .shader_name
                .as_deref()
                .expect("Render pipeline shader must be configured");
            self.shaders
                .expect("Named shaders require a render context")
                .get(shader_name)
                .unwrap_or_else(|| panic!("Shader '{shader_name}' is not registered"))
        });
        let target_format = self
            .config
            .target_format
            .expect("Render pipeline target format must be configured");
        let group_count = self
            .bind_group_layouts
            .last_key_value()
            .map(|(&group, _)| group as usize + 1)
            .unwrap_or(0);
        let pipeline_layouts = (0..group_count)
            .map(|group| self.bind_group_layouts.get(&(group as u32)))
            .collect::<Vec<_>>();
        let pipeline_layout = self.pipeline_layout.unwrap_or_else(|| {
            self.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(&self.label),
                    bind_group_layouts: &pipeline_layouts,
                    ..Default::default()
                })
        });
        let vertex_layouts = self
            .vertex_layouts
            .iter()
            .map(VertexBufferLayoutOwned::to_wgpu)
            .collect::<Vec<_>>();
        let mut primitive = self.primitive;
        primitive.cull_mode = self.config.culling;
        let depth_stencil =
            self.config
                .depth_enabled
                .map(|depth_write_enabled| wgpu::DepthStencilState {
                    format: Texture::DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write_enabled),
                    depth_compare: self.config.depth_compare,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&self.label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &vertex_layouts,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: self.blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive,
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        RenderPipelineState {
            pipeline,
            pipeline_layout,
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct MaterialKey {
    pub(crate) bindings: Vec<MaterialBindingKey>,
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
    pub bind_groups: Vec<Option<BindGroup>>,
    pub ic_buffer_layout: VertexBufferLayoutOwned,
    pub mesh_buffer_layout: VertexBufferLayoutOwned,
    bind_group_layouts: Vec<Option<BindGroupLayout>>,
    binding_layout_entries: Vec<Option<Vec<wgpu::BindGroupLayoutEntry>>>,
    bindings: BindGroupBuilder,
}

impl Material {
    pub fn change_shader(&mut self, device: &Device, format: TextureFormat, shader: &ShaderModule) {
        let target_format = self.key.target_format.unwrap_or(format);
        let state = RenderPipelineBuilder::from_shader(device, "Render Pipeline", shader)
            .target_format(target_format)
            .vertex_layout(self.mesh_buffer_layout.clone())
            .vertex_layout(self.ic_buffer_layout.clone())
            .existing_pipeline_layout(&self.pipeline_layout)
            .build();
        self.pipeline = state.pipeline;
    }

    pub(crate) fn with_texture(
        &self,
        device: &Device,
        texture: &Texture,
        group: u32,
        start_binding: u32,
    ) -> Self {
        let mut bindings = self.bindings.clone();
        register_texture_bundle(&mut bindings, texture, group, start_binding, true);
        let bind_groups = bindings.build_with_layouts(
            device,
            &self.bind_group_layouts,
            &self.binding_layout_entries,
            "derived material bind group",
        );
        let mut key = self.key.clone();
        key.bindings = bindings.keys();

        Self {
            key,
            pipeline_layout: self.pipeline_layout.clone(),
            pipeline: self.pipeline.clone(),
            bind_groups,
            ic_buffer_layout: self.ic_buffer_layout.clone(),
            mesh_buffer_layout: self.mesh_buffer_layout.clone(),
            bind_group_layouts: self.bind_group_layouts.clone(),
            binding_layout_entries: self.binding_layout_entries.clone(),
            bindings,
        }
    }
}

#[derive(Clone, Copy)]
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
    pub(crate) bindings: BindGroupBuilder,
    pub(crate) shader: String,
    pub(crate) vertex_layout: VertexBufferLayoutOwned,
    pub(crate) instance_layout: VertexBufferLayoutOwned,
    pub(crate) config: PipelineConfig,
}

impl<'a> MaterialBuilder<'a> {
    fn key(&self, bindings: &BindGroupBuilder) -> MaterialKey {
        MaterialKey {
            bindings: bindings.keys(),
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

    pub fn buffer(mut self, buffer: &Buffer, bind_group: u32, binding: u32) -> Self {
        self.bindings.buffer(buffer, bind_group, binding);
        self
    }

    pub fn buffer_handle(mut self, handle: BufferHandle, bind_group: u32, binding: u32) -> Self {
        let buffer = self.graphics.engine.resources.buffers.get(handle).unwrap();
        self.bindings.buffer(buffer, bind_group, binding);
        self
    }

    pub fn compute_buffer(mut self, handle: ComputeHandle, bind_group: u32, binding: u32) -> Self {
        let buffer =
            &self.graphics.engine.render_context.gpu_objects.computes[handle].render_buffer;
        self.bindings.buffer(buffer, bind_group, binding);
        self
    }

    pub fn config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn texture_from_color(
        mut self,
        color: [f32; 3],
        label: &str,
        bind_group: u32,
        start_binding: u32,
    ) -> Self {
        let texture = self.graphics.texture(label).color(color).build();
        register_texture_bundle(
            &mut self.bindings,
            &texture,
            bind_group,
            start_binding,
            false,
        );
        self
    }

    pub fn texture(mut self, texture: &Texture, bind_group: u32, start_binding: u32) -> Self {
        register_texture_bundle(
            &mut self.bindings,
            texture,
            bind_group,
            start_binding,
            false,
        );
        self
    }

    pub fn texture_view(
        mut self,
        texture: &Texture,
        texture_index: usize,
        bind_group: u32,
        binding: u32,
    ) -> Self {
        let definition = texture.texture.get(texture_index).unwrap_or_else(|| {
            panic!(
                "texture '{}' has no view at index {texture_index}",
                texture.label
            )
        });
        self.bindings.texture_view(
            &definition.view,
            texture.id,
            texture_index as u32,
            definition.view_dimension(),
            texture.sample_type,
            texture.visibility,
            bind_group,
            binding,
        );
        self
    }

    pub fn sampler(mut self, texture: &Texture, bind_group: u32, binding: u32) -> Self {
        self.bindings.sampler(
            &texture.sampler,
            texture.id,
            texture.sampler_binding_type,
            texture.visibility,
            bind_group,
            binding,
        );
        self
    }

    pub fn texture_parameters(mut self, texture: &Texture, bind_group: u32, binding: u32) -> Self {
        self.bindings
            .buffer(&texture.radiance_scale_buffer, bind_group, binding);
        self
    }

    pub fn build(mut self) -> MaterialHandle {
        for (group, binding, buffer) in self.graphics.engine.systems.get_bindings() {
            if !self.bindings.contains_buffer(buffer) && !self.bindings.contains(group, binding) {
                self.bindings.buffer(buffer, group, binding);
            }
        }
        let key = self.key(&self.bindings);

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
        let built_bindings = self
            .bindings
            .build(self.graphics.get_device(), "material bind group");

        let target_format = self
            .config
            .target_format
            .unwrap_or(self.graphics.engine.render_context.config.format);
        let mut pipeline_builder =
            RenderPipelineBuilder::new(&self.graphics.engine.render_context, "Render Pipeline")
                .shader(&self.shader)
                .config(self.config)
                .target_format(target_format)
                .vertex_layout(self.vertex_layout.clone())
                .vertex_layout(self.instance_layout.clone());
        for (group, layout) in built_bindings.layouts.iter().enumerate() {
            if let Some(layout) = layout {
                pipeline_builder = pipeline_builder.bind_group_layout(group as u32, layout);
            }
        }
        let state = pipeline_builder.build();
        let material = Material {
            key: key.clone(),
            pipeline: state.pipeline,
            pipeline_layout: state.pipeline_layout,
            bind_groups: built_bindings.bind_groups,
            ic_buffer_layout: self.instance_layout,
            mesh_buffer_layout: self.vertex_layout,
            bind_group_layouts: built_bindings.layouts,
            binding_layout_entries: built_bindings.layout_entries,
            bindings: self.bindings,
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

fn register_texture_bundle(
    bindings: &mut BindGroupBuilder,
    texture: &Texture,
    group: u32,
    start_binding: u32,
    replace: bool,
) {
    for (index, definition) in texture.texture.iter().enumerate() {
        let binding = start_binding + index as u32;
        if replace {
            bindings.replace_texture_view(
                &definition.view,
                texture.id,
                index as u32,
                definition.view_dimension(),
                texture.sample_type,
                texture.visibility,
                group,
                binding,
            );
        } else {
            bindings.texture_view(
                &definition.view,
                texture.id,
                index as u32,
                definition.view_dimension(),
                texture.sample_type,
                texture.visibility,
                group,
                binding,
            );
        }
    }

    let sampler_binding = start_binding + texture.texture.len() as u32;
    if replace {
        bindings.replace_sampler(
            &texture.sampler,
            texture.id,
            texture.sampler_binding_type,
            texture.visibility,
            group,
            sampler_binding,
        );
        bindings.replace_buffer(&texture.radiance_scale_buffer, group, sampler_binding + 1);
    } else {
        bindings.sampler(
            &texture.sampler,
            texture.id,
            texture.sampler_binding_type,
            texture.visibility,
            group,
            sampler_binding,
        );
        bindings.buffer(&texture.radiance_scale_buffer, group, sampler_binding + 1);
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct ComputeRenderingKey {
    pub compute: ComputeHandle,
    pub shader: String,
    bindings: Vec<MaterialBindingKey>,
}

#[derive(Clone)]
pub struct ComputeRendering {
    pub key: ComputeRenderingKey,

    pub pipeline_layout: PipelineLayout,
    pub pipeline: RenderPipeline,
    pub length: u32,
    pub bind_groups: Vec<Option<BindGroup>>,
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

    fn key(&self, bindings: &BindGroupBuilder) -> ComputeRenderingKey {
        ComputeRenderingKey {
            compute: self.compute,
            shader: self.shader.clone(),
            bindings: bindings.keys(),
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
        let length = compute.length;
        let mut bindings = BindGroupBuilder::new();
        let system_bindings = self.graphics.engine.systems.get_bindings();
        let mut next_group = system_bindings
            .iter()
            .map(|(group, _, _)| *group)
            .max()
            .map(|group| group + 1)
            .unwrap_or(0);
        for (group, binding, buffer) in system_bindings {
            bindings.buffer(buffer, group, binding);
        }
        for buffer in &self.input_buffers {
            bindings.buffer(buffer, next_group, 0);
            next_group += 1;
        }
        bindings.buffer(&render_buffer, next_group, 0);
        let key = self.key(&bindings);

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

        let built_bindings = bindings.build(device, "compute rendering bind group");
        let bind_group_layouts = built_bindings
            .layouts
            .iter()
            .map(Option::as_ref)
            .collect::<Vec<_>>();

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
            bind_groups: built_bindings.bind_groups,
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
