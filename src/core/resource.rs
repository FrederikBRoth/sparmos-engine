use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};

use crate::core::buffer::Buffer;

new_key_type! { pub struct BufferHandle; }
pub struct Resources {
    pub buffers: SlotMap<BufferHandle, Buffer>,
    pub named_buffers: HashMap<String, BufferHandle>,
}

impl Resources {
    pub(crate) fn new() -> Self {
        Resources {
            buffers: SlotMap::with_key(),
            named_buffers: HashMap::new(),
        }
    }
}

pub trait Register {
    fn register(self, resources: &mut Resources);
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}
