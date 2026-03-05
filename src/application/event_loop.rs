use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

use crate::application::state::{Game, State};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use web_sys::{Event, KeyboardEvent};

pub enum EngineEvent {
    StateReady { state: State, game: Box<dyn Game> },
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
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<UserEvent<U>>>,
    state: Option<State>,
    last_time: web_time::Instant,
}

impl<U> App<U>
where
    U: 'static,
{
    pub fn new<G>(
        #[cfg(target_arch = "wasm32")] event_loop: &winit::event_loop::EventLoop<UserEvent<U>>,
        hooks: G,
        game_loop: impl Game + 'static,
    ) -> Self
    where
        G: AppLifecycle<U> + 'static,
    {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());

        Self {
            is_focused: true,
            state: None,
            hooks: Box::new(hooks), // ← boxed lifecycle
            game_loop: Some(Box::new(game_loop)),
            #[cfg(target_arch = "wasm32")]
            proxy,
            last_time: web_time::Instant::now(),
        }
    }
}

pub trait AppLifecycle<U>: 'static {
    #[cfg(target_arch = "wasm32")]
    fn on_resumed(&mut self, _event_loop: &winit::event_loop::EventLoopProxy<UserEvent<U>>) {}
    #[cfg(not(target_arch = "wasm32"))]
    fn on_resumed(&mut self) {}
    fn on_user_event(&mut self, proxy: &mut State, _event: U) {}
    fn on_device_event(&mut self, event: DeviceEvent, proxy: &mut State) {}
}

impl<U: 'static> ApplicationHandler<UserEvent<U>> for App<U>
where
    U: 'static,
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
            let proxy = self.proxy.take().unwrap();
            let mut game = self.game_loop.take().unwrap();

            let value = proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut state = State::new(window.clone()).await;

                game.setup(&mut state);

                let size = state.window().inner_size();
                if size.width > 0 && size.height > 0 {
                    state.resize(size);
                }

                state.window().request_redraw();

                value
                    .send_event(UserEvent::EngineEvent(EngineEvent::StateReady {
                        state,
                        game,
                    }))
                    .is_ok();
            });

            self.hooks.on_resumed(&proxy);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(mut game_loop) = self.game_loop.take() {
                let mut state = pollster::block_on(State::new(window.clone()));
                game_loop.setup(&mut state);

                self.state = Some(state);
                self.game_loop = Some(game_loop);
            }

            self.hooks.on_resumed();
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent<U>) {
        match event {
            UserEvent::EngineEvent(engine_event) => match engine_event {
                EngineEvent::StateReady {
                    mut state,
                    mut game,
                } => {
                    self.last_time = web_time::Instant::now();

                    let size = state.window().inner_size();

                    log::warn!("{:?}", size);
                    state.resize(size);
                    game.resize(&mut state.core);

                    state.window().request_redraw();

                    self.state = Some(state);
                    self.game_loop = Some(game);
                }
            },

            UserEvent::Custom(user_event) => {
                self.hooks
                    .on_user_event(self.state.as_mut().unwrap(), user_event);
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
                state.render(game).unwrap();

                self.last_time = web_time::Instant::now();
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
            game.process_event(&event, &state.size, &mut state.core);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::RedrawRequested => {
                let dt = self.last_time.elapsed();
                self.last_time = web_time::Instant::now();
                state.update(dt);
                game.update(dt, &mut state.core);

                state.render(game).unwrap();
            }

            WindowEvent::Resized(size) => {
                state.resize(size);
                game.resize(&mut state.core);
            }

            _ => {
                game.process_event(&event, &state.size, &mut state.core);
            }
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.hooks
            .on_device_event(event, self.state.as_mut().unwrap());
    }
}
