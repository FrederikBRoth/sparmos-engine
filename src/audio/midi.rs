use midly::Smf;

pub struct Midi {}

impl Midi {
    pub fn load_midi(bytes: &[u8]) {
        let smf = Smf::parse(bytes).unwrap();
    }
}
