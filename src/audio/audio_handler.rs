use heapless::spsc::{Consumer, Producer, Queue};

use std::collections::HashMap;

use cpal::{
    Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use winit::{
    event::{KeyEvent, WindowEvent},
    keyboard::KeyCode,
};

use crate::{
    application::state::State,
    audio::synth::{AudioState, EnvelopeSegment, Sound, Waveform},
    systems::animation::Interpolation,
};
#[derive(PartialEq, Eq, Hash, Clone)]
pub enum AudioTrigger {
    Keyboard(KeyCode),
    GameLogic(String),
}

impl AudioTrigger {
    pub fn gamelogic(name: &str) -> Self {
        AudioTrigger::GameLogic(name.to_string())
    }
    pub fn keyboard(keycode: KeyCode) -> Self {
        AudioTrigger::Keyboard(keycode)
    }
}
pub enum AudioCommand {
    ForcePlay(AudioTrigger),
    Play(AudioTrigger),
    Stop(AudioTrigger),
    Edit(AudioTrigger, Sound),
    Add(AudioTrigger, Sound),
}
const QUEUE_SIZE: usize = 128;

pub struct AudioHandler {
    _audio_stream: Stream,
    pub sample_rate: f32,
    producer: Producer<'static, AudioCommand>,
}

pub struct AudioEngine {
    audio_triggers: HashMap<AudioTrigger, Sound>,
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
    pub fn update_from_gamelogic(&mut self, command: AudioCommand) {
        let _ = self.producer.enqueue(command);
    }

    pub fn start_audio(
        audio_triggers: HashMap<AudioTrigger, Sound>,
        pre_gain: f32,
        saturation: f32,
    ) -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let config = device.default_output_config().unwrap();

        log::warn!("{:?}", config.buffer_size());
        let sample_rate = config.sample_rate() as f32;

        let channels = config.channels() as usize;

        #[cfg(target_arch = "wasm32")]
        let mut config: StreamConfig = config.into();
        #[cfg(not(target_arch = "wasm32"))]
        let config: StreamConfig = config.into();
        //in order to have as low latency as possible, we adjust the buffer for wasm targets
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
                config.clone(),
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
            _audio_stream: stream,
            sample_rate: sample_rate,
            producer,
        }
    }

    pub fn init_sounds(state: &mut State, sounds: HashMap<AudioTrigger, Sound>) {
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
    audio_triggers: &mut HashMap<AudioTrigger, Sound>,
) {
    while let Some(cmd) = consumer.dequeue() {
        match cmd {
            AudioCommand::Play(trigger) => {
                if let Some(sound) = audio_triggers.get_mut(&trigger) {
                    sound.start();
                }
            }
            AudioCommand::Stop(trigger) => {
                if let Some(sound) = audio_triggers.get_mut(&trigger) {
                    sound.release();
                }
            }
            AudioCommand::Edit(trigger, s) => {
                if let Some(sound) = audio_triggers.get_mut(&trigger) {
                    sound.update(s);
                }
            }
            AudioCommand::ForcePlay(trigger) => {
                if let Some(sound) = audio_triggers.get_mut(&trigger) {
                    sound.force_start();
                }
            }
            AudioCommand::Add(trigger, sound) => {
                if !audio_triggers.contains_key(&trigger) {
                    audio_triggers.insert(trigger, sound);
                }
            }
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

    let n = index;

    Some(440.0 * 2f32.powf((n as f32 - 57.0) / 12.0))
}

pub fn index_to_key(index: usize) -> String {
    let key = KEYS[index % 12];
    let octave = index / 12;
    format!("{}{}", key, octave)
}
pub fn hz_to_index(freq: f32) -> usize {
    (57.0 + 12.0 * (freq / 440.0).log2())
        .round()
        .clamp(0.0, 90.0) as usize
}
pub fn index_to_hz(index: usize) -> f32 {
    let n = index as f32;
    440.0 * 2f32.powf((n - 57.0) / 12.0)
}

pub fn get_full_piano() -> Vec<Sound> {
    let mut sounds = vec![];

    let harmonics = [1.00, 0.30, 0.10, 0.05, 0.10, 0.7, 0.02];
    for key in 0..88 {
        let freq = index_to_hz(key);
        sounds.push(Sound::new(
            harmonics.into(),
            freq,
            0.0,
            Waveform::SineWave,
            EnvelopeSegment {
                length: 0.01,
                interpolation: Interpolation::EaseInEaseOut,
            },
            EnvelopeSegment {
                interpolation: Interpolation::EaseInEaseOut,
                length: 1.98,
            },
            EnvelopeSegment {
                length: 0.01,
                ..Default::default()
            },
        ));
    }
    sounds
}
