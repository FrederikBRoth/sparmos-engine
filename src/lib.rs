pub mod application;
pub mod audio;
pub mod core;
pub mod entities;
pub mod helpers;
pub mod systems;
pub use cgmath;
pub use egui;
pub use hecs;
pub use log;
pub use web_time;
pub use wgpu;
pub use winit;

pub mod prelude {
    use er::{Er, ErResult};
    use winit::event_loop::EventLoop;

    use crate::application::{
        event_loop::{App, AppLifecycle, UserEvent},
        state::Game,
    };
    #[derive(Er)]
    pub struct EventLoopEr;
    // pub struct Event
    pub fn run_game<U, G, L>(hooks: G, gameloop: L) -> Er<(), EventLoopEr>
    where
        U: 'static + Send,
        G: AppLifecycle<U> + 'static,
        L: Game + 'static,
    {
        #[cfg(not(target_arch = "wasm32"))]
        {
            env_logger::init();
        }

        #[cfg(target_arch = "wasm32")]
        {
            console_log::init_with_level(log::Level::Info).unwrap();
        }
        // let _dummy = EventLoop::<UserEvent<U>>::with_user_event()
        //     .build()
        //     .unwrap();
        let event_loop = EventLoop::<UserEvent<U>>::with_user_event()
            .build()
            .er(EventLoopEr::new)?;

        let mut app = App::new(&event_loop, hooks, gameloop);
        event_loop.run_app(&mut app).er(EventLoopEr::new)?;

        if let Some(error) = app.error {
            return Err(error);
        }
        Ok(())
    }
}
