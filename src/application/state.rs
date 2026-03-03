use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

use crate::application::gui::EguiRenderer;
use crate::entity::core::engine::Engine;
use crate::entity::core::render::GlobalRenderContext;
use crate::entity::texture::Texture;

pub enum DeviceBackend {
    WebGL,
    WebGPU,
}
pub struct Core {
    pub engine: Engine,
    pub render_context: GlobalRenderContext,
}

pub struct State {
    pub surface: wgpu::Surface<'static>,
    pub surface_configured: bool,
    pub size: winit::dpi::PhysicalSize<u32>,

    #[allow(dead_code)]
    pub window: Arc<Window>, // Application window
    pub scroll_y: i64,
    pub egui_renderer: EguiRenderer,
    pub backend: DeviceBackend,
    pub core: Core,
}
pub trait Game {
    fn render(
        &mut self,
        render: &mut wgpu::RenderPass,
        texture_view: &wgpu::TextureView,
        backend: &DeviceBackend,
        core: &mut Core,
    );

    fn update(&mut self, dt: std::time::Duration, core: &mut Core);

    fn process_event(&mut self, event: &WindowEvent, screen: &PhysicalSize<u32>, core: &mut Core);

    fn resize(&mut self, core: &mut Core);

    fn setup(&mut self, state: &mut State);

    #[cfg(feature = "gui")]
    fn gui_setup(&mut self, egui_renderer: &EguiRenderer);
}

impl State {
    // Creates a new State object, initializing all required resources
    pub async fn new(window: Arc<Window>) -> State {
        let size = window.inner_size();

        // Create a new GPU instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        // Create surface linked to window

        // Select appropriate GPU adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await;

        log::warn!("{:?}", adapter.clone().unwrap().get_info());

        // let adapter = Err("Something");
        let (surface, adapter, backend) = match adapter {
            Ok(a) => {
                let surface = instance.create_surface(window.clone()).unwrap();

                (surface, a, DeviceBackend::WebGPU)
            }
            Err(_) => {
                log::warn!("WebGPU unavailable, falling back to WebGL");

                // Recreate instance forcing GL backend
                let gl_instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::GL,
                    ..Default::default()
                });

                let gl_surface = gl_instance.create_surface(window.clone()).unwrap();

                let adapter = gl_instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::default(),
                        compatible_surface: Some(&gl_surface),
                        force_fallback_adapter: false,
                    })
                    .await
                    .expect("WebGL also unavailable!");

                (gl_surface, adapter, DeviceBackend::WebGL)
            }
        };

        let info = adapter.get_info();
        println!("test {:?}", info);
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
        let render_context = GlobalRenderContext {
            depth_texture: Texture::create_depth_texture(&device, &size, "depth_texture_primitive"),
            shaders: HashMap::new(),
            device: Arc::clone(&device),
            queue,
            config,
        };
        let egui_renderer = EguiRenderer::new(&device, surface_format, None, 1, &window);
        let core = Core {
            engine: Engine::default(),
            render_context,
        };
        Self {
            surface,
            surface_configured: false,
            size,
            window,
            core,
            scroll_y: 0,
            egui_renderer,
            backend,
        }
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.core.render_context.config.width = new_size.width;
            self.core.render_context.config.height = new_size.height;
            self.surface.configure(
                &self.core.render_context.device,
                &self.core.render_context.config,
            );
            self.surface_configured = true;

            // if let Some(game_loop) = self.game_loop.as_mut() {
            //     game_loop.resize(&self.render_context.config);
            // }
            self.core.render_context.depth_texture = Texture::create_depth_texture(
                &self.core.render_context.device,
                &new_size,
                "depth_texture_primitive",
            );
            // NEW!
        } else {
            println!("Not configured");
            log::warn!("Not Configured");
            self.surface_configured = false;
        }
    }
    // pub fn input(&mut self, event: &WindowEvent) {
    //     // if let Some(game_loop) = self.game_loop.as_mut() {
    //     //     game_loop.process_event(event, &self.size);
    //     // }
    // }
    //
    // pub fn update(&mut self, dt: std::time::Duration) {
    //     // if let Some(game_loop) = self.game_loop.as_mut() {
    //     //     game_loop.update(dt, &self.render_context);
    //     // }
    // }

    pub fn render(&mut self, game: &mut Box<dyn Game>) -> Result<(), wgpu::SurfaceError> {
        if !self.surface_configured {
            return Ok(());
        }

        self.window.request_redraw();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.core.render_context.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            },
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.core.render_context.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            game.render(&mut render_pass, &view, &self.backend, &mut self.core);
        }

        #[cfg(feature = "gui")]
        {
            use egui_wgpu::ScreenDescriptor;

            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [
                    self.core.render_context.config.width,
                    self.core.render_context.config.height,
                ],
                pixels_per_point: self.window.scale_factor() as f32,
            };

            self.egui_renderer.begin_frame(&self.window);
            game.gui_setup(&self.egui_renderer);
            self.egui_renderer.end_frame_and_draw(
                &self.core.render_context.device,
                &self.core.render_context.queue,
                &mut encoder,
                &self.window,
                &view,
                screen_descriptor,
            );
        }

        self.core
            .render_context
            .queue
            .submit(std::iter::once(encoder.finish()));

        output.present();

        Ok(())
    }
}

pub fn map_value(value: f32, old_min: f32, old_max: f32, new_max: f32, new_min: f32) -> f32 {
    let value = value.clamp(old_min, old_max);
    new_min + ((value - old_min) / (old_max - old_min)) * (new_max - new_min)
}
