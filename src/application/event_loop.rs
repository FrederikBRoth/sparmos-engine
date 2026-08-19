use std::sync::Arc;
use wgpu::{BufferView, MapRangeError};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

use crate::{
    application::state::{Game, State},
    core::render::ComputeHandle,
    systems::compute::{ComputeSystem, ReadbackState},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub struct ComputePackage {
    data: Result<BufferView, MapRangeError>,
    handle: ComputeHandle,
}
pub enum EngineEvent {
    EngineReady,
    ComputeResult(ComputePackage),
}

pub enum UserEvent<U> {
    EngineEvent(EngineEvent),
    Custom(U),
}

// #[derive(Default)]
pub struct App<U>
where
    U: 'static,
{
    pub is_focused: bool,
    pub hooks: Box<dyn AppLifecycle<U>>,
    pub game_loop: Option<Box<dyn Game>>,
    pub state: Option<State>,
    next_frame: web_time::Instant,
    proxy: Option<EventLoopProxy<UserEvent<U>>>,
    last_time: web_time::Instant,

    #[cfg(target_arch = "wasm32")]
    pending: std::rc::Rc<std::cell::RefCell<Option<(State, Box<dyn Game>)>>>,
}

impl<U> App<U>
where
    U: 'static,
{
    pub fn new<G>(
        event_loop: &winit::event_loop::EventLoop<UserEvent<U>>,
        hooks: G,
        game_loop: impl Game + 'static,
    ) -> Self
    where
        G: AppLifecycle<U> + 'static,
    {
        Self {
            is_focused: true,
            state: None,
            hooks: Box::new(hooks),
            game_loop: Some(Box::new(game_loop)),
            proxy: Some(event_loop.create_proxy()),
            last_time: web_time::Instant::now(),
            next_frame: web_time::Instant::now(),
            #[cfg(target_arch = "wasm32")]
            pending: std::rc::Rc::new(std::cell::RefCell::new(None)),
        }
    }
}

pub trait AppLifecycle<U>: 'static {
    fn on_resumed(&mut self, _event_loop: &winit::event_loop::EventLoopProxy<UserEvent<U>>) {}
    fn on_user_event(&mut self, proxy: &mut State, _event: U);
    fn on_device_event(&mut self, event: DeviceEvent, proxy: &mut State);
}

impl<U: Send + 'static> ApplicationHandler<UserEvent<U>> for App<U>
where
    U: Send + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas_element));
        }
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.clone().unwrap();
            let mut game = self.game_loop.take().unwrap();

            let pending = self.pending.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let mut state = State::new(window.clone()).await;

                game.setup(&mut state);

                let size = state.window().inner_size();

                if size.width > 0 && size.height > 0 {
                    state.resize(size);
                }

                *pending.borrow_mut() = Some((state, game));

                let _ = proxy.send_event(UserEvent::EngineEvent(EngineEvent::EngineReady));
            });

            self.hooks.on_resumed(&self.proxy.clone().unwrap());
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(mut game_loop) = self.game_loop.take() {
                let mut state = pollster::block_on(State::new(window.clone()));
                game_loop.setup(&mut state);
                //INFO: to initiate sound in WASM scenarios, you must call this function from a
                //user input in the browser. Otherwise it wont launch
                state.graphics.engine.init_sound(1.6, 1.2);

                self.state = Some(state);
                self.game_loop = Some(game_loop);
            }

            let proxy = self.proxy.clone().unwrap();
            self.hooks.on_resumed(&proxy);
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent<U>) {
        match event {
            UserEvent::EngineEvent(engine_event) => {
                match engine_event {
                    EngineEvent::EngineReady => {
                        #[cfg(target_arch = "wasm32")]
                        {
                            let (mut state, mut game) = self.pending.borrow_mut().take().expect(
                                "Received EngineReady \
                                         without pending engine",
                            );

                            self.last_time = web_time::Instant::now();

                            let size = state.window().inner_size();

                            state.resize(size);
                            game.resize(&mut state.graphics);

                            state.window().request_redraw();

                            self.state = Some(state);
                            self.game_loop = Some(game);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            // Nothing to do.
                        }
                    }

                    EngineEvent::ComputeResult(package) => {
                        let state = self.state.as_mut().unwrap();
                        if let Some(computes) = state
                            .graphics
                            .engine
                            .resources
                            .get_system_mut::<ComputeSystem>()
                        {
                            let compute = computes.get(package.handle).unwrap();
                            compute.read_result(package.data);
                            compute.temp_buffer.as_ref().unwrap().unmap();
                            compute.readback_status = ReadbackState::Available;
                            // println!("tawd");
                        }
                    }
                }
            }

            UserEvent::Custom(user_event) => {
                if let Some(state) = self.state.as_mut() {
                    self.hooks.on_user_event(state, user_event);
                }
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let (state, game) = match (&mut self.state, &mut self.game_loop) {
            (Some(state), Some(game)) => (state, game),
            _ => {
                return;
            }
        };

        if let WindowEvent::Focused(focused) = event {
            self.is_focused = focused;

            if focused {
                self.last_time = web_time::Instant::now();
                let dt = self.last_time.elapsed();

                state.render(dt, game)
            }
        }

        if !self.is_focused {
            return;
        }

        #[cfg(feature = "gui")]
        if !state
            .egui_renderer
            .handle_input(state.window.as_ref(), &event)
        {
            // game.process_event(&event, &state.size, &mut state.core);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                let dt = self.last_time.elapsed();
                self.last_time = web_time::Instant::now();

                state.render(dt, game);

                if let Some(computes) = state
                    .graphics
                    .engine
                    .resources
                    .get_system_mut::<ComputeSystem>()
                {
                    state
                        .graphics
                        .world
                        .borrow()
                        .query::<&ComputeHandle>(|mut handle| {
                            for compute_handle in handle.iter() {
                                let compute = computes.get(*compute_handle).unwrap();
                                match compute.readback_status {
                                    ReadbackState::NoReadback | ReadbackState::Pending => continue,
                                    ReadbackState::Available => {
                                        compute.readback_status = ReadbackState::Pending;
                                        readback(
                                            &compute.temp_buffer.as_ref().unwrap(),
                                            *compute_handle,
                                            &self.proxy.clone().unwrap(),
                                        );
                                    }
                                }
                            }
                        });
                }
                state.update(dt);
                // println!("test");

                game.update(dt, &mut state.graphics);
            }

            WindowEvent::Resized(size) => {
                state.resize(size);
                game.resize(&mut state.graphics);
            }

            _ => {
                state.input(&event);
                game.process_event(&event, &state.size, &mut state.graphics);
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(state) = self.state.as_mut() {
            self.hooks.on_device_event(event, state);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = web_time::Instant::now();

        // Poll wgpu independently of the render rate.
        if let Some(state) = self.state.as_ref() {
            state
                .graphics
                .engine
                .render_context
                .device
                .poll(wgpu::PollType::Poll)
                .ok();
        }

        // Render deadline
        if now >= self.next_frame {
            self.next_frame += std::time::Duration::from_secs_f64(1.0 / 144.0);

            if let Some(state) = self.state.as_ref() {
                state.window().request_redraw();
            }
        }

        // Wake up frequently enough to service GPU callbacks,
        // but don't busy-spin.
        let poll_deadline = now + std::time::Duration::from_millis(1);

        let next_wakeup = poll_deadline.min(self.next_frame);

        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(next_wakeup));
    }
}

pub fn readback<U: Send + 'static>(
    compute: &wgpu::Buffer,
    handle: ComputeHandle,
    proxy: &EventLoopProxy<UserEvent<U>>,
) {
    let slice = compute.slice(..);
    let buffer = compute.clone();
    let proxy = proxy.clone();
    slice.map_async(wgpu::MapMode::Read, move |result| match result {
        Ok(()) => {
            let slice = buffer.slice(..);
            let data = slice.get_mapped_range();

            let _ = proxy.send_event(UserEvent::EngineEvent(EngineEvent::ComputeResult(
                ComputePackage {
                    data,
                    handle: handle,
                },
            )));
        }

        Err(err) => {
            log::error!("GPU mapping failed: {err:?}");
        }
    });
}
