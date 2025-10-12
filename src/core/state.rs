use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::{iter, vec};

use cgmath::{Vector3, prelude::*};
use wgpu::{ShaderModel, ShaderModule, SurfaceConfiguration};
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

use crate::core::camera::CameraController;
use crate::entity::entity::{
    BufferManager, Instance, InstanceController, Light, PrimitiveMesh, RenderMeshInformation,
    RenderableController, Rendering, instance_cube,
};
use crate::entity::texture::Texture;

// The main application state holding all GPU resources and game logic
pub struct State<L>
where
    L: GameLoop,
{
    pub surface: wgpu::Surface<'static>,     // GPU rendering surface
    pub surface_configured: bool,            // Tracks if surface is configured
    pub render_context: RenderContext,       // Surface configuration settings
    pub size: winit::dpi::PhysicalSize<u32>, // Window size
    #[allow(dead_code)]
    // Handles input-based camera movement
    // Bind group for camera
    #[allow(dead_code)]
    pub window: Arc<Window>, // Application window
    pub game_loop: Option<L>,
    pub scroll_y: i64,
    pub depth_texture: Texture,
}
pub trait GameLoop {
    fn render(
        &mut self,
        render: &mut wgpu::RenderPass,
        texture_view: &wgpu::TextureView,
        depth_texture: Texture,
    );

    fn update(&mut self, dt: std::time::Duration, rc: &RenderContext);

    fn process_event(&mut self, event: &WindowEvent, screen: &PhysicalSize<u32>);

    fn resize(&mut self, config: &SurfaceConfiguration);

    fn setup<S: GameLoop>(&mut self, state: &mut State<S>);
}

impl<L> State<L>
where
    L: GameLoop,
{
    // Creates a new State object, initializing all required resources
    pub async fn new(window: Arc<Window>) -> State<L> {
        let size = window.inner_size();

        // Create a new GPU instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        // Create surface linked to window
        let surface = instance.create_surface(window.clone()).unwrap();

        // Select appropriate GPU adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        log::warn!("{:?}", adapter.get_info());

        // Request device and queue from adapter
        let (tdevice, tqueue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits {
                        max_texture_dimension_1d: 4096,
                        max_texture_dimension_2d: 4096,
                        ..wgpu::Limits::downlevel_webgl2_defaults()
                    }
                } else {
                    wgpu::Limits::default()
                },
                ..Default::default()
            })
            .await
            .unwrap();

        let device = Arc::new(tdevice);
        let queue = Arc::new(tqueue);

        log::warn!("Surface");

        // Get surface capabilities and select preferred format
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Configure surface
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,

            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        let depth_texture =
            Texture::create_depth_texture(&device, &size, "depth_texture_primitive");
        let render_context = RenderContext {
            device,
            queue,
            config,
        };
        // game_loop.setup(
        //     Arc::clone(&device),
        //     Arc::clone(&queue),
        //     size,
        //     surface_format,
        // );
        // Return initialized State
        Self {
            surface,
            surface_configured: false,
            render_context,
            size,
            window,
            game_loop: None::<L>,
            depth_texture,
            scroll_y: 0,
        }
    }

    pub fn set_loop(&mut self, gameloop: L) {
        self.game_loop = Some(gameloop);
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.render_context.config.width = new_size.width;
            self.render_context.config.height = new_size.height;
            self.surface
                .configure(&self.render_context.device, &self.render_context.config);
            self.surface_configured = true;

            if let Some(game_loop) = self.game_loop.as_mut() {
                game_loop.resize(&self.render_context.config);
            }
            self.depth_texture = Texture::create_depth_texture(
                &self.render_context.device,
                &new_size,
                "depth_texture_primitive",
            );
            // NEW!
        } else {
            println!("Not configured");
            self.surface_configured = false;
        }
    }
    pub fn input(&mut self, event: &WindowEvent) {
        if let Some(game_loop) = self.game_loop.as_mut() {
            game_loop.process_event(event, &self.size);
        }
    }

    pub fn update(&mut self, dt: std::time::Duration) {
        if let Some(game_loop) = self.game_loop.as_mut() {
            game_loop.update(dt, &self.render_context);
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // We can't render unless the surface is configured
        if !self.surface_configured {
            return Ok(());
        }
        self.window.request_redraw();
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.render_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: {
                    Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_texture.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    })
                },
                // depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if let Some(game_loop) = self.game_loop.as_mut() {
                game_loop.render(&mut render_pass, &view, self.depth_texture.clone());
            }
        }

        self.render_context
            .queue
            .submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub fn map_value(value: f32, old_min: f32, old_max: f32, new_max: f32, new_min: f32) -> f32 {
    let value = value.clamp(old_min, old_max);
    new_min + ((value - old_min) / (old_max - old_min)) * (new_max - new_min)
}

pub struct RenderContext {
    pub device: Arc<wgpu::Device>, // Logical GPU device
    pub queue: Arc<wgpu::Queue>,   // Command queue for GPU
    pub config: wgpu::SurfaceConfiguration,
}

impl RenderContext {
    pub fn create_renderable_controller(
        &mut self,
        meshes: Vec<PrimitiveMesh>,
        light: &Light,
        camera: &CameraController,
        shader: &ShaderModule,
        default_instances: Option<Vec<Instance>>,
    ) -> RenderableController {
        let ri = PrimitiveMesh::get_render_definitions(
            &self.device,
            shader,
            self.config.format,
            &self.queue,
            camera.camera_bind_group_layout.clone(),
            light.light_bind_group_layout.clone(),
            light.light_bind_group.clone(),
            None,
        );

        if let Some(default_instances) = default_instances {
            // let vertices = meshes
            //     .iter()
            //     .flat_map(|mesh| mesh.vertices.iter().cloned())
            //     .collect();
            let indices = meshes
                .iter()
                .flat_map(|mesh| mesh.indices.iter().cloned())
                .collect();
            let mut vertex_offset = 0;
            let mut index_offset = 0;

            let render_meshes: Vec<RenderMeshInformation> = meshes
                .iter()
                .map(|mesh| {
                    let ri = RenderMeshInformation {
                        instance_controller: InstanceController::new(default_instances.clone()),
                        vertices: mesh.vertices.clone(),
                        num_indices: mesh.indices.len() as u32,
                        indices: mesh.indices.clone(),
                        vertex_offset,
                        index_offset,
                    };

                    // increment offsets for next mesh
                    vertex_offset += mesh.vertices.len() as u32;
                    index_offset += mesh.indices.len() as u32;

                    ri
                })
                .collect();
            RenderableController::new(
                0,
                BufferManager::new(&self.device, 500000, &meshes, &indices),
                ri,
                render_meshes,
            )
        } else {
            let instances = instance_cube(
                Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            );
            // let vertices = meshes
            //     .iter()
            //     .flat_map(|mesh| mesh.vertices.iter().cloned())
            //     .collect();
            let indices = meshes
                .iter()
                .flat_map(|mesh| mesh.indices.iter().cloned())
                .collect();
            let mut vertex_offset = 0;
            let mut index_offset = 0;
            let render_meshes: Vec<RenderMeshInformation> = meshes
                .iter()
                .map(|mesh| {
                    let ri = RenderMeshInformation {
                        instance_controller: InstanceController::new(vec![instances.clone()]),
                        vertex_offset,
                        index_offset,
                        vertices: mesh.vertices.clone(),
                        num_indices: mesh.indices.len() as u32,
                        indices: mesh.indices.clone(),
                    };

                    // increment offsets for next mesh
                    vertex_offset += mesh.vertices.len() as u32;
                    index_offset += mesh.indices.len() as u32;

                    ri
                })
                .collect();
            RenderableController::new(
                0,
                BufferManager::new(&self.device, 500000, &meshes, &indices),
                ri,
                render_meshes,
            )
        }
    }
}

fn instances_list_cube(chunk_size: Vector3<u32>) -> Vec<Instance> {
    (0..(chunk_size.x * chunk_size.y * chunk_size.z))
        .map(move |n| {
            let x = n % chunk_size.x;
            let z = (n / chunk_size.x) % chunk_size.z;
            let y = n / (chunk_size.x * chunk_size.z);

            let position = cgmath::Vector3 {
                x: x as f32 + (chunk_size.x as i32) as f32,
                y: y as f32,
                z: z as f32 + (chunk_size.z as i32) as f32,
            };

            let rotation = if position.is_zero() {
                // this is needed so an object at (0, 0, 0) won't get scaled to zero
                // as Quaternions can effect scale if they're not created correctly
                cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
            } else {
                cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(0.0))
            };
            let default_color = cgmath::Vector3::new(1.0, 0.0, 0.0);
            let default_size = cgmath::Vector3::new(1.0, 1.0, 1.0);
            let default_bounding = default_size + position;

            Instance {
                index: n,
                position,
                rotation,
                scale: 0.5,
                should_render: true,
                color: default_color,
                size: default_size,
                bounding: default_bounding,
            }
        })
        .collect::<Vec<_>>()
}
