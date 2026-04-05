use heapless::spsc::{Consumer, Producer, Queue};

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::Instant,
};

use cgmath::num_traits::Float;
use cpal::{
    Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use egui::emath::normalized_angle;
use egui_winit::process_viewport_commands;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::KeyCode,
};

use crate::{
    application::state::State,
    entity::audio::synth::{AudioState, Sound},
};
#[derive(PartialEq, Eq)]
pub enum AudioTrigger {
    Keyboard(KeyCode),
    GameLogic(String),
}
enum AudioCommand {
    Play(AudioTrigger),
    Stop(AudioTrigger),
    Edit(AudioTrigger, Sound),
}
const QUEUE_SIZE: usize = 128;

pub struct AudioHandler {
    audio_stream: Stream,
    producer: Producer<'static, AudioCommand>,
}

pub struct AudioEngine {
    audio_triggers: HashMap<KeyCode, Sound>,
    limiter: Limiter,
    consumer: Consumer<'static, AudioCommand>,
}
impl AudioHandler {
    pub fn update_from_keypress(&mut self, keypress: &WindowEvent) {
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    state,
                    physical_key: winit::keyboard::PhysicalKey::Code(keycode),
                    ..
                },
            ..
        } = keypress
        {
            let cmd = if state.is_pressed() {
                AudioCommand::Play(AudioTrigger::Keyboard(*keycode))
            } else {
                AudioCommand::Stop(AudioTrigger::Keyboard(*keycode))
            };

            let _ = self.producer.enqueue(cmd);
        }
    }
    pub fn start_audio(
        audio_triggers: HashMap<KeyCode, Sound>,
        pre_gain: f32,
        saturation: f32,
    ) -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let config = device.default_output_config().unwrap();

        log::warn!("{:?}", config.buffer_size());
        let sample_rate = config.sample_rate() as f32;

        let channels = config.channels() as usize;

        let mut config: StreamConfig = config.into();
        //in order to have great as low latency as possible, we adjust the buffer for wasm targets
        #[cfg(target_arch = "wasm32")]
        {
            config.buffer_size = cpal::BufferSize::Fixed(1024);
        }
        let queue = Box::new(Queue::<AudioCommand, QUEUE_SIZE>::new());

        let queue: &'static mut _ = Box::leak(queue);

        let (producer, consumer) = queue.split();

        let mut audio_engine = AudioEngine {
            audio_triggers,
            limiter: Limiter::new(),
            consumer,
        };

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    process_audio_commands(
                        &mut audio_engine.consumer,
                        &mut audio_engine.audio_triggers,
                    );

                    let dt = 1.0 / sample_rate;

                    for frame in data.chunks_mut(channels) {
                        let mut mix = 0.0;

                        for synth in audio_engine
                            .audio_triggers
                            .values_mut()
                            .filter(|e| e.envelope.audio_state != AudioState::Stopped)
                        {
                            mix += synth.update_envelope(dt) * 0.2;
                        }

                        mix *= pre_gain;
                        mix = audio_engine.limiter.process(mix);
                        let value = mix * saturation;

                        for sample in frame.iter_mut() {
                            *sample = value;
                        }
                    }
                },
                |err| eprintln!("audio error: {err}"),
                None,
            )
            .unwrap();

        stream.play().unwrap();

        AudioHandler {
            audio_stream: stream,
            producer,
        }
    }

    pub fn init_sounds(state: &mut State, sounds: HashMap<KeyCode, Sound>) {
        state.engine.audio_triggers = Some(sounds);
    }
}
pub struct Limiter {
    env: f32,
    gain: f32,
    attack: f32,
    release: f32,
    threshold: f32,
}
impl Limiter {
    pub fn new() -> Self {
        Self {
            env: 0.0,
            gain: 1.0,
            attack: 0.01,
            release: 0.001,
            threshold: 0.8,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let x = input.abs();

        // envelope follower
        if x > self.env {
            self.env += (x - self.env) * self.attack;
        } else {
            self.env += (x - self.env) * self.release;
        }

        // compute gain reduction
        let target_gain = if self.env > self.threshold {
            self.threshold / self.env
        } else {
            1.0
        };

        // smooth gain
        self.gain += (target_gain - self.gain) * 0.001;

        input * self.gain
    }
}

fn process_audio_commands(
    consumer: &mut Consumer<AudioCommand>,
    audio_triggers: &mut HashMap<KeyCode, Sound>,
) {
    while let Some(cmd) = consumer.dequeue() {
        match cmd {
            AudioCommand::Play(AudioTrigger::Keyboard(key_code)) => {
                if let Some(sound) = audio_triggers.get_mut(&key_code) {
                    sound.start();
                }
            }
            AudioCommand::Stop(AudioTrigger::Keyboard(key_code)) => {
                if let Some(sound) = audio_triggers.get_mut(&key_code) {
                    sound.release();
                }
            }
            AudioCommand::Edit(_, _) => {}
            _ => {}
        }
    }
}

const KEYS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub fn pianokey_to_hz(key: &str) -> Option<f32> {
    if key.len() > 3 {
        return None;
    }

    let (key, octave) = key.split_at(key.len() - 1);
    let octave = octave.parse::<i32>().ok()?;

    let key_index = KEYS.iter().position(|&k| k == key)? as i32;

    let index = key_index + octave * 12;

    let n = index - 8;

    Some(440.0 * 2f32.powf((n as f32 - 49.0) / 12.0))
}
