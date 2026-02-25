use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

use crate::core::state::{GameLoop, State};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, closure::Closure};
#[cfg(target_arch = "wasm32")]
use web_sys::{Event, KeyboardEvent};

pub enum EngineEvent<L>
where
    L: GameLoop,
{
    StateReady(State<L>),
}

pub enum UserEvent<U, L>
where
    L: GameLoop,
{
    EngineEvent(EngineEvent<L>),
    Custom(U),
}

// #[derive(Default)]
pub struct App<U, G, L>
where
    U: 'static,
    G: AppLifecycle<U, L>,
    L: 'static + GameLoop,
{
    pub is_focused: bool,
    pub hooks: G,
    pub game_loop: Option<L>,
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<UserEvent<U, L>>>,
    state: Option<State<L>>,
    last_time: web_time::Instant,
    _marker: std::marker::PhantomData<U>,
}

impl<U, G, L> App<U, G, L>
where
    U: 'static,
    G: AppLifecycle<U, L>,
    L: 'static + GameLoop,
{
    pub fn new(
        #[cfg(target_arch = "wasm32")] event_loop: &EventLoop<UserEvent<U, L>>,
        game: G,
        game_loop: Option<L>,
    ) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            is_focused: true,
            state: None,
            hooks: game,
            game_loop,
            #[cfg(target_arch = "wasm32")]
            proxy,
            last_time: web_time::Instant::now(),
            _marker: std::marker::PhantomData,
        }
    }
}

pub trait AppLifecycle<U, L>: 'static
where
    L: 'static + GameLoop,
{
    #[cfg(target_arch = "wasm32")]
    fn on_resumed(&mut self, _event_loop: &winit::event_loop::EventLoopProxy<UserEvent<U, L>>) {}
    #[cfg(not(target_arch = "wasm32"))]
    fn on_resumed(&mut self) {}
    fn on_user_event(&mut self, proxy: &mut State<L>, _event: U) {}
    fn on_device_event(&mut self, event: DeviceEvent, proxy: &mut State<L>) {}
}

impl<U: 'static, G: 'static, L: 'static> ApplicationHandler<UserEvent<U, L>> for App<U, G, L>
where
    U: 'static,
    G: AppLifecycle<U, L>,
    L: GameLoop,
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

        // Create window object
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                let proxy_clone = proxy.clone();
                if let Some(mut game_loop) = self.game_loop.take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        let mut state = State::<L>::new(window.clone()).await;
                        game_loop.setup(&mut state);
                        state.set_loop(game_loop);

                        let size = state.window().inner_size();
                        if size.width > 0 && size.height > 0 {
                            state.resize(size);
                        }

                        state.window().request_redraw();

                        assert!(
                            proxy_clone
                                .send_event(UserEvent::EngineEvent(EngineEvent::StateReady(state)))
                                .is_ok()
                        )
                    });
                }

                self.hooks.on_resumed(&proxy.clone());
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(mut game_loop) = self.game_loop.take() {
            let mut state = pollster::block_on(State::<L>::new(window.clone()));
            game_loop.setup(&mut state);
            state.set_loop(game_loop);
            self.state = Some(state);
            self.hooks.on_resumed();
        } else {
            eprintln!("Warning: game loop already taken or not initialized.");
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent<U, L>) {
        match event {
            UserEvent::EngineEvent(engine_event) => match engine_event {
                EngineEvent::StateReady(state) => {
                    self.state = Some(state);
                }
            },

            UserEvent::Custom(user_event) => {
                self.hooks
                    .on_user_event(self.state.as_mut().unwrap(), user_event);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };
        if let WindowEvent::Focused(focused) = event {
            println!("Focused: {}", focused);
            self.is_focused = focused;
            if focused {
                state.render().unwrap();
                self.last_time = web_time::Instant::now();
            }
            // Do something when focused
        }
        if self.is_focused {
            #[cfg(feature = "gui")]
            if !state
                .egui_renderer
                .handle_input(state.window.as_ref(), &event)
            {
                state.input(&event);
            }
            match event {
                WindowEvent::CloseRequested => {
                    println!("The close button was pressed; stopping");
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => {
                    let dt = self.last_time.elapsed();
                    self.last_time = web_time::Instant::now();
                    state.update(dt);
                    state.render().unwrap();
                }
                WindowEvent::Resized(size) => {
                    // Reconfigures the size of the surface. We do not re-render
                    // here as this event is always followed up by redraw request.
                    state.resize(size);
                }
                _ => (),
            }
        }
    }

    // fn device_event(
    //     &mut self,
    //     event_loop: &ActiveEventLoop,
    //     device_id: DeviceId,
    //     event: DeviceEvent,
    // ) {
    //     self.hooks
    //         .on_device_event(event, self.state.as_mut().unwrap());
    // }
}
