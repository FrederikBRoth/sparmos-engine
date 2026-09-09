use std::any::Any;

use ahash::HashMap;
use winit::{dpi::PhysicalSize, event::WindowEvent};

use crate::{
    application::{graphics::Graphics, state::Game},
    core::entities::World,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    KeyboardInput,
    MouseInput,
    MouseWheel,
    CursorMoved,
    CursorEntered,
    CursorLeft,
    Focused,
    GeneralWindowEvent,
}

pub struct EventContext<'a> {
    pub event: &'a WindowEvent,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

//Function
type EventHandler = Box<dyn Fn(&mut dyn Game, &mut EventContext)>;

#[derive(Default)]
pub struct EventRegistry {
    handlers: HashMap<EventKind, Vec<EventHandler>>,
}

impl EventRegistry {
    pub fn on<G>(&mut self, kind: EventKind, handler: fn(&mut G, &mut EventContext))
    where
        G: Game + 'static,
    {
        self.handlers
            .entry(kind)
            .or_default()
            .push(Box::new(move |game, ec| {
                let game = (game as &mut dyn Any)
                    .downcast_mut::<G>()
                    .expect("event handler registered for wrong Game type");

                handler(game, ec);
            }));
    }
    pub fn process(
        &self,
        game: &mut dyn Game,
        event: &WindowEvent,
        screen: &PhysicalSize<f32>,
        gfx: &mut Graphics,
        world: &World,
    ) {
        let kind = EventKind::GeneralWindowEvent;

        if let Some(handlers) = self.handlers.get(&kind) {
            for handler in handlers {
                let mut ec = EventContext {
                    event,
                    size: screen,
                    gfx,
                    world,
                };

                handler(game, &mut ec);
            }
        }
    }
}
