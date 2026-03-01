pub mod core;
pub mod entity;
pub mod helpers;

pub use cgmath;
pub use egui;
pub use hecs;
pub use log;
pub use web_time;
pub use wgpu;
pub use winit;

pub mod prelude {
    use winit::event_loop::EventLoop;

    use crate::core::{
        event_loop::{App, AppLifecycle, UserEvent},
        state::GameLoop,
    };

    pub fn run_game<U, G, L>(hooks: G, gameloop: L) -> anyhow::Result<()>
    where
        U: 'static + Send,
        G: AppLifecycle<U> + 'static,
        L: GameLoop + 'static,
    {
        #[cfg(not(target_arch = "wasm32"))]
        {
            env_logger::init();
        }

        #[cfg(target_arch = "wasm32")]
        {
            console_log::init_with_level(log::Level::Info).unwrap();
        }

        let event_loop = EventLoop::<UserEvent<U>>::with_user_event()
            .build()
            .unwrap();

        #[cfg(target_arch = "wasm32")]
        let mut app = App::new(&event_loop, hooks, gameloop);

        #[cfg(not(target_arch = "wasm32"))]
        let mut app = App::new(hooks, gameloop);

        event_loop.run_app(&mut app).unwrap();
        Ok(())
    }
}
