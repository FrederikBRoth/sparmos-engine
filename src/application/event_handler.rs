use std::any::Any;

use ahash::HashMap;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceId, ElementState, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, TouchPhase,
        WindowEvent,
    },
    keyboard::KeyCode,
};

use crate::{
    application::{graphics::Graphics, state::Game},
    core::entities::World,
};

#[derive(PartialEq, Eq, Hash)]
pub enum CursorState {
    Entered,
    Exit,
    Moved,
}

///Generic window event context
pub struct GenericEventContext<'a> {
    pub event: &'a WindowEvent,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

pub struct ModifierEventContext<'a> {
    pub modifiers: &'a Modifiers,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

pub struct KeyboardEventContext<'a> {
    pub device_id: &'a DeviceId,
    pub state: &'a ElementState,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

pub struct CursorStateEventContext<'a> {
    pub device_id: &'a DeviceId,
    pub position: Option<&'a PhysicalPosition<f64>>,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

pub struct MouseInputEventContext<'a> {
    pub device_id: &'a DeviceId,
    pub state: &'a ElementState,
    pub button: &'a MouseButton,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

pub struct MouseWheelEventContext<'a> {
    pub device_id: &'a DeviceId,
    pub delta: &'a MouseScrollDelta,
    pub phase: &'a TouchPhase,
    pub size: &'a PhysicalSize<f32>,
    pub gfx: &'a mut Graphics,
    pub world: &'a World,
}

//Function
type GenericEventHandler = Box<dyn Fn(&mut dyn Game, &mut GenericEventContext)>;
type ModifierEventHandler = Box<dyn Fn(&mut dyn Game, &mut ModifierEventContext)>;
type KeyboardEventHandler = Box<dyn Fn(&mut dyn Game, &mut KeyboardEventContext)>;
type CursorStateEventHandler = Box<dyn Fn(&mut dyn Game, &mut CursorStateEventContext)>;
type MouseInputEventHandler = Box<dyn Fn(&mut dyn Game, &mut MouseInputEventContext)>;
type MouseWheelEventHandler = Box<dyn Fn(&mut dyn Game, &mut MouseWheelEventContext)>;

#[derive(Default)]
pub struct EventRegistry {
    generic_events: Vec<GenericEventHandler>,
    modifier_events: Vec<ModifierEventHandler>,
    keyboard_events: HashMap<KeyCode, Vec<KeyboardEventHandler>>,
    cursor_state_events: HashMap<CursorState, Vec<CursorStateEventHandler>>,
    mouse_input_events: HashMap<MouseButton, Vec<MouseInputEventHandler>>,
    mouse_wheel_events: Vec<MouseWheelEventHandler>,
}

impl EventRegistry {
    ///Generic window event context. Can facilitate full event handling for whatever window event
    ///Winit outputs
    pub fn event<G>(&mut self, handler: fn(&mut G, &mut GenericEventContext))
    where
        G: Game + 'static,
    {
        self.generic_events.push(Box::new(move |game, ec| {
            let game = (game as &mut dyn Any)
                .downcast_mut::<G>()
                .expect("event handler registered for wrong Game type");

            handler(game, ec);
        }));
    }

    pub fn key<G>(&mut self, keycode: KeyCode, handler: fn(&mut G, &mut KeyboardEventContext))
    where
        G: Game + 'static,
    {
        self.keyboard_events
            .entry(keycode)
            .or_default()
            .push(Box::new(move |game, keyboard_context| {
                let game = (game as &mut dyn Any)
                    .downcast_mut::<G>()
                    .expect("event handler registered for wrong Game type");

                handler(game, keyboard_context);
            }));
    }
    pub fn modifier<G>(&mut self, handler: fn(&mut G, &mut ModifierEventContext))
    where
        G: Game + 'static,
    {
        self.modifier_events
            .push(Box::new(move |game, modifier_context| {
                let game = (game as &mut dyn Any)
                    .downcast_mut::<G>()
                    .expect("event handler registered for wrong Game type");

                handler(game, modifier_context);
            }));
    }
    pub fn cursor<G>(
        &mut self,
        cursor_state: CursorState,
        handler: fn(&mut G, &mut CursorStateEventContext),
    ) where
        G: Game + 'static,
    {
        self.cursor_state_events
            .entry(cursor_state)
            .or_default()
            .push(Box::new(move |game, cursor_state_event_context| {
                let game = (game as &mut dyn Any)
                    .downcast_mut::<G>()
                    .expect("event handler registered for wrong Game type");

                handler(game, cursor_state_event_context);
            }));
    }
    pub fn mouse<G>(
        &mut self,
        mouse_button: MouseButton,
        handler: fn(&mut G, &mut MouseInputEventContext),
    ) where
        G: Game + 'static,
    {
        self.mouse_input_events
            .entry(mouse_button)
            .or_default()
            .push(Box::new(move |game, mouse_input_context| {
                let game = (game as &mut dyn Any)
                    .downcast_mut::<G>()
                    .expect("event handler registered for wrong Game type");

                handler(game, mouse_input_context);
            }));
    }

    pub fn scrollwheel<G>(&mut self, handler: fn(&mut G, &mut MouseWheelEventContext))
    where
        G: Game + 'static,
    {
        self.mouse_wheel_events
            .push(Box::new(move |game, mouse_wheel_context| {
                let game = (game as &mut dyn Any)
                    .downcast_mut::<G>()
                    .expect("event handler registered for wrong Game type");

                handler(game, mouse_wheel_context);
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
        match event {
            WindowEvent::KeyboardInput {
                device_id,
                event:
                    KeyEvent {
                        state,
                        physical_key: winit::keyboard::PhysicalKey::Code(keycode),
                        ..
                    },
                is_synthetic,
            } => {
                let mut keyboard_event_context = KeyboardEventContext {
                    device_id,
                    state,
                    size: screen,
                    gfx,
                    world,
                };

                if let Some(handlers) = self.keyboard_events.get(keycode) {
                    for handler in handlers {
                        handler(game, &mut keyboard_event_context);
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let mut modifier_event_context = ModifierEventContext {
                    modifiers,
                    size: screen,
                    gfx,
                    world,
                };

                for handler in &self.modifier_events {
                    handler(game, &mut modifier_event_context);
                }
            }
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => {
                let mut cursor_state_event_context = CursorStateEventContext {
                    device_id,
                    position: Some(position),
                    size: screen,
                    gfx,
                    world,
                };

                if let Some(handlers) = self.cursor_state_events.get(&CursorState::Moved) {
                    for handler in handlers {
                        handler(game, &mut cursor_state_event_context);
                    }
                }
            }
            WindowEvent::CursorEntered { device_id } => {
                let mut cursor_state_event_context = CursorStateEventContext {
                    device_id,
                    position: None,
                    size: screen,
                    gfx,
                    world,
                };

                if let Some(handlers) = self.cursor_state_events.get(&CursorState::Entered) {
                    for handler in handlers {
                        handler(game, &mut cursor_state_event_context);
                    }
                }
            }
            WindowEvent::CursorLeft { device_id } => {
                let mut cursor_state_event_context = CursorStateEventContext {
                    device_id,
                    position: None,
                    size: screen,
                    gfx,
                    world,
                };

                if let Some(handlers) = self.cursor_state_events.get(&CursorState::Exit) {
                    for handler in handlers {
                        handler(game, &mut cursor_state_event_context);
                    }
                }
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                let mut mouse_wheel_event_context = MouseWheelEventContext {
                    device_id,
                    delta,
                    phase,
                    size: screen,
                    gfx,
                    world,
                };

                for handler in &self.mouse_wheel_events {
                    handler(game, &mut mouse_wheel_event_context);
                }
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                let mut mouse_input_event_context = MouseInputEventContext {
                    size: screen,
                    gfx,
                    world,
                    device_id,
                    state,
                    button,
                };

                if let Some(handlers) = self.mouse_input_events.get(button) {
                    for handler in handlers {
                        handler(game, &mut mouse_input_event_context);
                    }
                }
            }
            _ => {
                let mut ec = GenericEventContext {
                    event,
                    size: screen,
                    gfx,
                    world,
                };

                for handler in &self.generic_events {
                    handler(game, &mut ec);
                }
            }
        };
    }
}
