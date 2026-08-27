use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::vec;

#[cfg(feature = "gui")]
use egui::Ui;
use wgpu::{CurrentSurfaceTexture, InstanceDescriptor};
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

use crate::application::graphics::Graphics;
use crate::application::gui::EguiRenderer;
use crate::core::engine::{Arguments, Engine, EngineCommandQueue, EngineTime, Systems};
use crate::core::entities::World;
use crate::core::geometry::Model;
use crate::core::post_processing::PostProcessHandler;
use crate::core::render::{ComputeHandle, DrawMesh, GpuObjects, RenderContext, Renderable};
use crate::core::resource::Resources;
use crate::core::texture::Texture;
use crate::systems::animation::AnimationHandler;
use crate::systems::compute::ReadbackState;

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
    pub graphics: Graphics,
}
pub trait Game {
    fn update(&mut self, gfx: &mut Graphics, world: Ref<'_, World>);

    fn process_event(
        &mut self,
        event: &WindowEvent,
        screen: &PhysicalSize<u32>,
        gfx: &mut Graphics,
        world: Ref<'_, World>,
    );

    fn resize(&mut self, gfx: &mut Graphics, world: Ref<'_, World>);

    fn setup(&mut self, state: &mut State);

    #[cfg(feature = "gui")]
    fn gui_setup(&mut self, dt: std::time::Duration, gfx: &mut Graphics, ui: &mut Ui);
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
                ..Default::default()
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
                        ..Default::default()
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
                        max_bind_groups: 4,
                        ..wgpu::Limits::default()
                    }
                } else {
                    wgpu::Limits {
                        ..Default::default()
                    }
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
        let rgba16float_features =
            adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
        let rgba16float_renderable = rgba16float_features.allowed_usages.contains(
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
        ) && rgba16float_features
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
        let rg16float_features =
            adapter.get_texture_format_features(wgpu::TextureFormat::Rg16Float);
        let rg16float_renderable = rg16float_features.allowed_usages.contains(
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        ) && rg16float_features
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE);
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

            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let post_processing = PostProcessHandler::new(Arc::clone(&device), Arc::clone(&queue));

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
            rgba16float_renderable,
            rg16float_renderable,
            gpu_objects: GpuObjects::new(),
            post_processing,
        };
        let egui_renderer = EguiRenderer::new(&device, surface_format, 1, &window);
        let arguments = Arguments {
            args: HashMap::new(),
        };
        let engine = Engine {
            render_context,
            arguments,
            engine_time: EngineTime {
                frame_count: 0,
                time_acc: Duration::ZERO,
                dt: Duration::ZERO,
            },
            render_commands: Vec::new(),
            audio_handler: None,
            systems: Systems { systems: vec![] },
            resources: Resources::new(),
            audio_triggers: None,
        };
        let mut gfx = Graphics {
            world: Rc::new(RefCell::new(World::new(hecs::World::new()))),
            engine,
        };

        //Setup basic systems
        //Compute

        gfx.shader("pbr", include_str!("../core/shaders/pbr_shader.wgsl"));
        gfx.shader("pbr2", include_str!("../core/shaders/pbr_shader2.wgsl"));

        gfx.shader(
            "pbr_textured",
            include_str!("../core/shaders/pbr_shader_textured.wgsl"),
        );
        gfx.shader("skybox", include_str!("../core/shaders/skybox.wgsl"));
        // post_processing.new_effect(size, surface_format, Effect::ChromaticTwo);

        //We cant initialize audio in the browser before a user has interacted with the website.
        //Therefor we have to only instantiate the audio handler when in native
        Self {
            surface,
            surface_configured: false,
            size,
            window,
            graphics: gfx,
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
            println!("{:?}", new_size);
            self.graphics.get_render_context_mut().config.width = new_size.width;
            self.graphics.get_render_context_mut().config.height = new_size.height;
            self.surface.configure(
                self.graphics.get_device(),
                &self.graphics.get_render_context().config,
            );
            self.surface_configured = true;

            // if let Some(game_loop) = self.game_loop.as_mut() {
            //     game_loop.resize(&self.render_context.config);
            // }
            let overscan_size = PhysicalSize::new(
                (new_size.width as f32 * 1.1) as u32,
                (new_size.height as f32 * 1.1) as u32,
            );

            self.graphics.get_render_context_mut().depth_texture = Texture::create_depth_texture(
                self.graphics.get_device(),
                &new_size,
                "depth_texture_primitive",
            );
            self.graphics
                .get_render_context_mut()
                .overscan_depth_texture = Texture::create_depth_texture(
                self.graphics.get_device(),
                &overscan_size,
                "overscan_depth_texture",
            );

            self.graphics
                .engine
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
        if let Some(audio_handler) = self.graphics.engine.audio_handler.as_mut() {
            audio_handler.update_from_keypress(event);
        }
    }
    //
    pub fn update(&mut self, dt: std::time::Duration) {
        self.graphics.engine.engine_time.update_time(dt, true);
        {
            self.graphics
                .world
                .borrow()
                .query::<(&Renderable, &mut AnimationHandler)>(|mut query| {
                    for (renderable, ah) in query.iter() {
                        // ah.animate(dt.as_secs_f32());
                        let ic = self
                            .graphics
                            .engine
                            .render_context
                            .gpu_objects
                            .instance_controllers
                            .get_mut(renderable.instance_controller_handle)
                            .unwrap();

                        ah.update_instance(dt.as_secs_f32(), ic.instances_mut().as_mut());
                    }
                });
        }
        {
            self.graphics
                .world
                .borrow()
                .query::<&Renderable>(|mut query| {
                    for renderable in query.iter() {
                        self.graphics
                            .engine
                            .render_context
                            .gpu_objects
                            .instance_controllers
                            .get_mut(renderable.instance_controller_handle)
                            .unwrap()
                            .update(&self.graphics.engine.render_context.queue);
                    }
                });

            self.graphics.world.borrow().query::<&Model>(|mut query| {
                for renderable in query.iter() {
                    self.graphics
                        .engine
                        .render_context
                        .gpu_objects
                        .instance_controllers
                        .get_mut(renderable.instance)
                        .unwrap()
                        .update(&self.graphics.engine.render_context.queue);
                }
            });
        }
        self.graphics.run_all_systems();
    }

    pub fn render(&mut self, dt: std::time::Duration, game: &mut Box<dyn Game>) {
        if !self.surface_configured {
            return;
        }

        match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(surface_texture) => {
                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = self
                    .graphics
                    .engine
                    .render_context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });
                {
                    self.graphics
                        .world
                        .borrow()
                        .query::<&ComputeHandle>(|mut query| {
                            for compute in query.iter() {
                                let compute_pipeline = self
                                    .graphics
                                    .engine
                                    .render_context
                                    .gpu_objects
                                    .computes
                                    .get(*compute)
                                    .unwrap();
                                if compute_pipeline.readback_status == ReadbackState::Pending {
                                    continue;
                                };
                                let num_dispatches = compute_pipeline.length.div_ceil(64) as u32;

                                {
                                    let mut pass = encoder.begin_compute_pass(&Default::default());

                                    pass.set_pipeline(&compute_pipeline.pipeline);
                                    let mut bind_group_index = 0;
                                    for input_buffer in compute_pipeline.input_buffers.iter() {
                                        pass.set_bind_group(
                                            bind_group_index,
                                            Some(&input_buffer.bind_group),
                                            &[],
                                        );
                                        bind_group_index += 1;
                                    }
                                    pass.set_bind_group(
                                        bind_group_index,
                                        Some(&compute_pipeline.output_buffer.bind_group),
                                        &[],
                                    );
                                    pass.dispatch_workgroups(num_dispatches, 1, 1);
                                }
                                if let Some(temp_buffer) = compute_pipeline.temp_buffer.as_ref() {
                                    encoder.copy_buffer_to_buffer(
                                        &compute_pipeline.output_buffer.buffer,
                                        0,
                                        &temp_buffer,
                                        0,
                                        compute_pipeline.output_buffer.buffer.size(),
                                    );
                                }
                            }
                        });
                }

                if self
                    .graphics
                    .engine
                    .render_context
                    .post_processing
                    .post_processes
                    .len()
                    > 0
                {
                    let mut post_processes = self
                        .graphics
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
                                            .graphics
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
                        render_pass.draw_scene(
                            &self.backend,
                            &self.graphics.engine,
                            &self.graphics.world.borrow(),
                        );
                    }

                    while let Some((_, post_process)) = post_processes.next() {
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
                                        view: &self
                                            .graphics
                                            .engine
                                            .render_context
                                            .depth_texture
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

                        render_pass.draw_scene(
                            &self.backend,
                            &self.graphics.engine,
                            &self.graphics.world.borrow(),
                        );
                    }
                }

                #[cfg(feature = "gui")]
                {
                    use egui_wgpu::ScreenDescriptor;

                    let screen_descriptor = ScreenDescriptor {
                        size_in_pixels: [
                            self.graphics.engine.render_context.config.width,
                            self.graphics.engine.render_context.config.height,
                        ],
                        pixels_per_point: self.window.scale_factor() as f32,
                    };

                    let full_output = self.egui_renderer.start_gui(&self.window, |ui| {
                        game.gui_setup(dt, &mut self.graphics, ui);
                    });
                    self.egui_renderer.end_frame_and_draw(
                        &self.graphics.engine.render_context.device,
                        &self.graphics.engine.render_context.queue,
                        &mut encoder,
                        &self.window,
                        &view,
                        screen_descriptor,
                        full_output,
                    );
                }

                self.graphics
                    .engine
                    .render_context
                    .queue
                    .submit(std::iter::once(encoder.finish()));

                let mut commands = std::mem::take(&mut self.graphics.engine.render_commands);
                for command in commands.drain(..) {
                    match command {
                        EngineCommandQueue::ChangeShader(material_handle, shader) => {
                            self.graphics
                                .engine
                                .change_shader_inner(&material_handle, &shader);
                        }
                        EngineCommandQueue::AddEntity(fn_once) => {
                            let mut world = self.graphics.world.borrow_mut();
                            fn_once(&mut world.entities);
                        }
                    }
                }
                self.graphics
                    .engine
                    .render_context
                    .queue
                    .present(surface_texture);
            }
            CurrentSurfaceTexture::Suboptimal(_) => (),
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
