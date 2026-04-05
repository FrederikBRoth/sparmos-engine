use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::vec;

use cpal::traits::{DeviceTrait, HostTrait};
use wgpu::{CurrentSurfaceTexture, InstanceDescriptor};
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::keyboard::KeyCode;
use winit::window::Window;

use crate::application::gui::EguiRenderer;
use crate::entity::audio::audio_handler::{self, AudioHandler, pianokey_to_hz};
use crate::entity::audio::synth::{EnvelopeSegment, Sound};
use crate::entity::core::engine::{self, Arguments, Engine, RenderCommands};
use crate::entity::core::entities::World;
use crate::entity::core::post_processing::{self, Effect, PostProcessHandler};
use crate::entity::core::render::{self, DrawMesh, GpuObjects, RenderContext, Renderable};
use crate::entity::core::resource::Resources;
use crate::entity::systems::camera::{Camera, CameraAnimator, CameraSystem};
use crate::entity::texture::Texture;
use crate::helpers::animation::{AnimationHandler, Interpolation};

pub enum DeviceBackend {
    WebGL,
    WebGPU,
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
    pub engine: Engine,
    pub world: World,
}
pub trait Game {
    fn update(&mut self, dt: std::time::Duration, engine: &mut Engine, world: &mut World);

    fn process_event(
        &mut self,
        event: &WindowEvent,
        screen: &PhysicalSize<u32>,
        engine: &mut Engine,
        world: &mut World,
    );

    fn resize(&mut self, engine: &mut Engine, world: &mut World);

    fn setup(&mut self, state: &mut State);

    #[cfg(feature = "gui")]
    fn gui_setup(&mut self, egui_renderer: &EguiRenderer);
}

impl State {
    // Creates a new State object, initializing all required resources
    pub async fn new(window: Arc<Window>) -> State {
        let size = window.inner_size();

        // Create a new GPU instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..InstanceDescriptor::new_without_display_handle()
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
                let gl_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::GL,
                    ..InstanceDescriptor::new_without_display_handle()
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
        let mut post_processing = PostProcessHandler::new(Arc::clone(&device), Arc::clone(&queue));

        let overscan_size = PhysicalSize::new(
            (size.width as f32 * 1.1) as u32,
            (size.height as f32 * 1.1) as u32,
        );
        let render_context = RenderContext {
            depth_texture: Texture::create_depth_texture(&device, &size, "depth_texture_primitive"),
            overscan_depth_texture: Texture::create_depth_texture(
                &device,
                &overscan_size,
                "overscan_depth_texture",
            ),
            shaders: HashMap::new(),
            device: Arc::clone(&device),
            queue: Arc::clone(&queue),
            config,
            gpu_objects: GpuObjects::new(),
            post_processing,
        };
        let egui_renderer = EguiRenderer::new(&device, surface_format, None, 1, &window);
        let arguments = Arguments {
            args: HashMap::new(),
        };
        let mut engine = Engine {
            render_context,
            arguments,
            frame_count: 0,
            time_acc: Duration::ZERO,
            render_commands: Vec::new(),
            audio_handler: None,
            audio_triggers: None,
        };

        // post_processing.new_effect(size, surface_format, Effect::ChromaticTwo);

        //We cant initialize audio in the browser before a user has interacted with the website.
        //Therefor we have to only instantiate the audio handler when in native
        Self {
            surface,
            surface_configured: false,
            size,
            window,
            engine,
            scroll_y: 0,
            egui_renderer,
            backend,
            world: World::new(Arc::clone(&device), hecs::World::new(), Resources::new()),
        }
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            println!("{:?}", new_size);
            self.engine.render_context.config.width = new_size.width;
            self.engine.render_context.config.height = new_size.height;
            self.surface.configure(
                &self.engine.render_context.device,
                &self.engine.render_context.config,
            );
            self.surface_configured = true;

            // if let Some(game_loop) = self.game_loop.as_mut() {
            //     game_loop.resize(&self.render_context.config);
            // }
            let overscan_size = PhysicalSize::new(
                (new_size.width as f32 * 1.1) as u32,
                (new_size.height as f32 * 1.1) as u32,
            );

            self.engine.render_context.depth_texture = Texture::create_depth_texture(
                &self.engine.render_context.device,
                &new_size,
                "depth_texture_primitive",
            );
            self.engine.render_context.overscan_depth_texture = Texture::create_depth_texture(
                &self.engine.render_context.device,
                &overscan_size,
                "overscan_depth_texture",
            );

            self.engine
                .render_context
                .post_processing
                .resize(overscan_size);
        } else {
            println!("Not configured");
            log::warn!("Not Configured");
            self.surface_configured = false;
        }
    }
    pub fn input(&mut self, event: &WindowEvent) {
        if let Some(audio_handler) = self.engine.audio_handler.as_mut() {
            audio_handler.update_from_keypress(event);
        }
    }
    //
    pub fn update(&mut self, dt: std::time::Duration) {
        self.engine.frame_count += 1;
        self.engine.time_acc += dt;

        if self.engine.time_acc >= std::time::Duration::from_secs(1) {
            let fps = self.engine.frame_count as f64 / self.engine.time_acc.as_secs_f64();
            println!("FPS: {:.2}", fps);

            // reset
            self.engine.frame_count = 0;
            self.engine.time_acc = std::time::Duration::ZERO;
        }
        {
            let mut query = self
                .world
                .entities
                .query::<(&Renderable, &mut AnimationHandler)>();
            for (renderable, ah) in query.iter() {
                // ah.animate(dt.as_secs_f32());
                let ic = self
                    .engine
                    .render_context
                    .gpu_objects
                    .instance_controllers
                    .get_mut(renderable.instance_controller_handle)
                    .unwrap();

                ah.update_instance(dt.as_secs_f32(), ic.instances_mut().as_mut());
            }
        }
        {
            let mut query = self.world.entities.query::<&Renderable>();
            for renderable in query.iter() {
                self.engine
                    .render_context
                    .gpu_objects
                    .instance_controllers
                    .get_mut(renderable.instance_controller_handle)
                    .unwrap()
                    .update_single(&self.engine.render_context.queue);
            }
        }
        self.world
            .query_first_with_resources::<(&mut Camera, &mut CameraAnimator)>(
                |resources, (camera, camera_animator)| {
                    let camera_system = resources.get_system_mut::<CameraSystem>();
                    camera_system.update_camera(dt, &self.engine.render_context, camera);
                    camera_animator.update(dt.as_secs_f32(), camera);
                },
            );
    }

    pub fn render(&mut self, game: &mut Box<dyn Game>) {
        // println!("FRAME");
        if !self.surface_configured {
            return;
        }

        self.window.request_redraw();

        match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(surface_texture) => {
                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = self.engine.render_context.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    },
                );
                if self
                    .engine
                    .render_context
                    .post_processing
                    .post_processes
                    .len()
                    > 0
                {
                    let mut post_processes = self
                        .engine
                        .render_context
                        .post_processing
                        .post_processes
                        .iter()
                        .peekable();

                    let mut current_post_process = post_processes.peek().unwrap().1;
                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Main Render Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &current_post_process.view,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: &self
                                            .engine
                                            .render_context
                                            .overscan_depth_texture
                                            .view,
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                ..Default::default()
                            });
                        render_pass.draw_scene(&self.backend, &self.engine, &self.world);
                    }

                    while let Some((key, post_process)) = post_processes.next() {
                        //There are no "extra post processes". Go to screen render
                        let next_pp = post_processes.peek();
                        match next_pp {
                            Some(pp) => {
                                let next = pp.1;
                                let mut post_pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("Post Process Pass"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: &next.view, // 👈 NOW we render to screen
                                                depth_slice: None,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        ..Default::default()
                                    });

                                post_pass.set_pipeline(&post_process.pipeline);
                                post_pass.set_bind_group(0, &post_process.bind_group, &[]);
                                post_pass.set_viewport(
                                    0.0,
                                    0.0,
                                    self.size.width as f32,
                                    self.size.height as f32,
                                    0.0,
                                    1.0,
                                );
                                post_pass.set_scissor_rect(0, 0, self.size.width, self.size.height);
                                post_pass.draw(0..3, 0..1); // fullscreen triangle

                                current_post_process = next;
                            }
                            None => break,
                        }
                    }
                    let mut post_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Post Process Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view, // 👈 NOW we render to screen
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        ..Default::default()
                    });

                    post_pass.set_pipeline(&current_post_process.pipeline);
                    post_pass.set_bind_group(0, &current_post_process.bind_group, &[]);
                    post_pass.set_viewport(
                        0.0,
                        0.0,
                        self.size.width as f32,
                        self.size.height as f32,
                        0.0,
                        1.0,
                    );
                    post_pass.set_scissor_rect(0, 0, self.size.width, self.size.height);
                    post_pass.draw(0..3, 0..1); // fullscreen triangle
                } else {
                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: &self.engine.render_context.depth_texture.view,
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                ..Default::default()
                            });

                        render_pass.draw_scene(&self.backend, &self.engine, &self.world);
                    }
                }

                #[cfg(feature = "gui")]
                {
                    use egui_wgpu::ScreenDescriptor;

                    let screen_descriptor = ScreenDescriptor {
                        size_in_pixels: [
                            self.engine.render_context.config.width,
                            self.engine.render_context.config.height,
                        ],
                        pixels_per_point: self.window.scale_factor() as f32,
                    };

                    self.egui_renderer.begin_frame(&self.window);
                    game.gui_setup(&self.egui_renderer);
                    self.egui_renderer.end_frame_and_draw(
                        &self.engine.render_context.device,
                        &self.engine.render_context.queue,
                        &mut encoder,
                        &self.window,
                        &view,
                        screen_descriptor,
                    );
                }

                self.engine
                    .render_context
                    .queue
                    .submit(std::iter::once(encoder.finish()));

                let mut commands = std::mem::take(&mut self.engine.render_commands);
                for command in commands.drain(..) {
                    match command {
                        RenderCommands::ChangeShader(material_handle, shader) => {
                            self.engine.change_shader_inner(&material_handle, &shader);
                        }
                    }
                }
                surface_texture.present();
            }
            CurrentSurfaceTexture::Suboptimal(surface_texture) => (),
            CurrentSurfaceTexture::Timeout => (),
            CurrentSurfaceTexture::Occluded => (),
            CurrentSurfaceTexture::Outdated => (),
            CurrentSurfaceTexture::Lost => (),
            CurrentSurfaceTexture::Validation => (),
        }
    }
}

pub fn map_value(value: f32, old_min: f32, old_max: f32, new_max: f32, new_min: f32) -> f32 {
    let value = value.clamp(old_min, old_max);
    new_min + ((value - old_min) / (old_max - old_min)) * (new_max - new_min)
}
