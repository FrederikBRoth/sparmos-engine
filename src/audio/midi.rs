use std::collections::HashMap;

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MidiNote {
    pub name: String,
    pub key: usize,
    pub start: u32,
    pub length: u32,
}
#[derive(Debug)]
pub struct Midi {
    pub length: u32,
    pub tempos: Vec<u32>,
    pub ticks_per_quarter: u16,
    pub channels: HashMap<u8, Vec<MidiNote>>,
}

impl Midi {
    pub fn load_midi(bytes: &[u8]) -> Self {
        let smf = Smf::parse(bytes).unwrap();

        let mut current_time: u32 = 0;
        let mut tempos = vec![];

        let beat_length = match smf.header.timing {
            midly::Timing::Metrical(u15) => u15.as_int(),
            midly::Timing::Timecode(_, _) => 480,
        };
        let mut active_notes: HashMap<(u8, u8), u32> = HashMap::new();
        let mut channels: HashMap<u8, Vec<MidiNote>> = HashMap::new();

        for track in &smf.tracks {
            current_time = 0;

            for event in track {
                // accumulate delta time
                current_time += event.delta.as_int();

                match &event.kind {
                    TrackEventKind::Midi { channel, message } => {
                        let ch = channel.as_int();

                        match message {
                            MidiMessage::NoteOn { key, vel } => {
                                let key = key.as_int();

                                if vel.as_int() == 0 {
                                    // treat as NoteOff
                                    if let Some(start) = active_notes.remove(&(ch, key)) {
                                        let length = current_time - start;

                                        channels.entry(ch).or_default().push(MidiNote {
                                            name: format!("Note {key} in Channel {ch}"),
                                            key: key as usize,
                                            start,
                                            length,
                                        });
                                    }
                                } else {
                                    active_notes.insert((ch, key), current_time);
                                }
                            }

                            MidiMessage::NoteOff { key, .. } => {
                                let key = key.as_int();

                                if let Some(start) = active_notes.remove(&(ch, key)) {
                                    let length = current_time - start;

                                    channels.entry(ch).or_default().push(MidiNote {
                                        name: format!("Note {key} in Channel {ch}"),
                                        key: key as usize,
                                        start,
                                        length,
                                    });
                                }
                            }

                            _ => {}
                        }
                    }

                    TrackEventKind::Meta(meta) => {
                        if let MetaMessage::Tempo(t) = meta {
                            tempos.push(t.as_int());
                        }
                    }

                    _ => {}
                }
            }
        }

        Midi {
            length: current_time,
            tempos: tempos,
            ticks_per_quarter: beat_length,
            channels,
        }
    }
}
