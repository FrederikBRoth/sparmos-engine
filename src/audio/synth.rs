use std::fmt;

use cgmath::{Vector2, vec2};

use crate::systems::animation::Interpolation;

// synth.rs
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioState {
    Playing,
    Stopping,
    #[default]
    Stopped,
}

#[derive(Clone)]
pub struct EnvelopeSegment {
    pub interpolation: Interpolation,
    pub length: f32,
}
impl From<(Interpolation, f32, f32)> for EnvelopeSegment {
    fn from((interpolation, length, _): (Interpolation, f32, f32)) -> Self {
        Self {
            interpolation,
            length,
        }
    }
}

impl Default for EnvelopeSegment {
    fn default() -> Self {
        Self {
            interpolation: Interpolation::Linear,
            length: 1.0,
        }
    }
}

#[derive(Clone, Default)]
pub struct Envelope {
    pub gain: f32,
    pub prev_gain: f32,
    pub audio_state: AudioState,
    pub elapsed: f32,
    pub timeframe: f32,
    pub attack: EnvelopeSegment,
    pub decay: EnvelopeSegment,
    pub sustain: f32,
    pub refrain: EnvelopeSegment,
}

impl Envelope {
    pub fn new(
        attack: EnvelopeSegment,
        decay: EnvelopeSegment,
        refrain: EnvelopeSegment,
        sustain: f32,
        audio_state: AudioState,
    ) -> Self {
        let timeframe = attack.length + decay.length + refrain.length;
        Self {
            attack,
            decay,
            sustain,
            refrain,
            timeframe,
            elapsed: 0.0,
            audio_state,
            gain: 0.0,
            prev_gain: 0.0,
        }
    }

    pub fn advance(&mut self, dt: f32) {
        self.elapsed += dt;
    }
    pub fn update(&mut self) -> f32 {
        match self.audio_state {
            AudioState::Playing => {
                let t = self.elapsed as f32;

                let attack_end = self.attack.length;
                let decay_end = attack_end + self.decay.length;
                //Attack
                let value = if t < attack_end {
                    let x = t / self.attack.length;
                    let interp = self.attack.interpolation.lerp(x, false);
                    self.prev_gain + (1.0 - self.prev_gain) * interp.0
                //Decay
                } else if t < decay_end {
                    let x = (t - attack_end) / self.decay.length;
                    let interp = self.decay.interpolation.lerp(x, true);
                    self.sustain + (1.0 - self.sustain) * interp.0
                //Sustain
                } else {
                    self.sustain
                };

                value
            }
            AudioState::Stopping => {
                let t = self.elapsed as f32;

                //Refrain
                let value = if t < self.refrain.length {
                    let x = t / self.refrain.length;
                    let interp = self.refrain.interpolation.lerp(x, false);

                    self.prev_gain * (1.0 - interp.0)
                } else {
                    self.audio_state = AudioState::Stopped;
                    0.0
                };

                value
            }
            AudioState::Stopped => 0.0,
        }
    }

    // pub fn update_
}
#[derive(Clone, Default, Debug)]
pub enum Waveform {
    #[default]
    SineWave,
    SquareWave,
    TriangleWave,
    SawtoothWave,
}

impl Waveform {
    pub fn position(&self, phase: f32) -> f32 {
        match self {
            Waveform::SineWave => (phase * std::f32::consts::TAU).sin(),
            Waveform::SquareWave => Waveform::SineWave.position(phase).signum(),
            Waveform::TriangleWave => 2.0 * (2.0 * phase - 1.0).abs() - 1.0,
            Waveform::SawtoothWave => 2.0 * (phase - (phase + 0.5).floor()),
        }
    }
}
#[derive(Clone)]
pub struct Sound {
    pub freq: f32,
    pub phases: Vec<f32>,
    pub harmonics: Vec<f32>,
    pub waveform: Waveform,
    pub envelope: Envelope,
}

impl fmt::Display for Sound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Frequency: {:.2} Hz, Waveform: {:?}, Harmonics: {:?}, Envelope lengths (ADSR): {}, {}, {}, {}",
            self.freq,
            self.waveform,
            self.harmonics,
            self.envelope.attack.length,
            self.envelope.decay.length,
            self.envelope.sustain,
            self.envelope.refrain.length,
        )
    }
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            freq: 440.0,
            phases: [0.0].into(),
            harmonics: [1.0].into(),
            waveform: Default::default(),
            envelope: Default::default(),
        }
    }
}

impl Sound {
    pub fn new(
        harmonics: Vec<f32>,
        freq: f32,
        sustain: f32,
        waveform: Waveform,
        attack: EnvelopeSegment,
        decay: EnvelopeSegment,
        refrain: EnvelopeSegment,
    ) -> Self {
        let phases = harmonics.iter().map(|_| 0.0).collect();
        Self {
            phases,
            freq,
            harmonics,
            waveform,
            envelope: Envelope::new(attack, decay, refrain, sustain, AudioState::Stopped),
        }
    }

    pub fn update(&mut self, change: Sound) {
        self.harmonics = change.harmonics;
        if change.phases.len() > self.phases.len() {
            self.phases.resize(change.phases.len(), Default::default());
        }
        self.freq = change.freq;
        self.envelope.attack.length = change.envelope.attack.length;
        self.envelope.decay.length = change.envelope.decay.length;
        self.envelope.refrain.length = change.envelope.refrain.length;
        self.envelope.attack.interpolation = change.envelope.attack.interpolation;
        self.envelope.decay.interpolation = change.envelope.decay.interpolation;
        self.envelope.refrain.interpolation = change.envelope.refrain.interpolation;
        self.envelope.sustain = change.envelope.sustain;
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }

    pub fn next(&mut self, dt: f32) -> f32 {
        let mut sample = 0.0;
        let norm = 1.0 / self.harmonics.iter().sum::<f32>();
        for (index, harmonic) in self.harmonics.iter().enumerate() {
            let harmonic_freq = self.freq * (1.0 + index as f32);
            self.phases[index] += harmonic_freq * dt;
            if self.phases[index] >= 1.0 {
                self.phases[index] -= 1.0;
            }

            sample +=
                self.waveform.position(self.phases[index]) * norm * harmonic * self.envelope.gain
        }
        sample
    }

    pub fn start(&mut self) {
        if self.envelope.audio_state != AudioState::Playing {
            self.envelope.audio_state = AudioState::Playing;
            self.envelope.elapsed = 0.0;
            self.envelope.prev_gain = self.envelope.gain;
        }
    }
    pub fn force_start(&mut self) {
        self.envelope.audio_state = AudioState::Playing;
        self.envelope.elapsed = 0.0;
        self.envelope.prev_gain = self.envelope.gain;
    }

    pub fn release(&mut self) {
        if self.envelope.audio_state != AudioState::Stopping {
            self.envelope.audio_state = AudioState::Stopping;
            self.envelope.elapsed = 0.0;
            self.envelope.prev_gain = self.envelope.gain;
        }
    }

    pub fn update_envelope(&mut self, dt: f32) -> f32 {
        self.envelope.gain = self.envelope.update();
        self.envelope.advance(dt);
        self.next(dt)
        // println!("gain: {}", self.envelope.gain)
    }

    pub fn duration(&self) -> f32 {
        self.envelope.attack.length + self.envelope.refrain.length + self.envelope.decay.length
    }

    pub fn get_envelope_bounds(&self) -> (Vector2<f32>, Vector2<f32>) {
        let attack_bounds = vec2(0.0, self.envelope.attack.length);
        let decay_bounds = vec2(
            self.envelope.attack.length,
            self.envelope.attack.length + self.envelope.decay.length,
        );
        (attack_bounds, decay_bounds)
    }
}
