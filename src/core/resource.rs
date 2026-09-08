use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};

use crate::core::{buffer::Buffer, sprites::sprite_loader::SpriteSheet};

new_key_type! { pub struct BufferHandle; }
new_key_type! {pub struct SpriteSheetHandle; }
pub struct Resources {
    pub buffers: SlotMap<BufferHandle, Buffer>,
    pub named_buffers: HashMap<String, BufferHandle>,
    pub sprite_sheets: SlotMap<SpriteSheetHandle, SpriteSheet>,
}

impl Resources {
    pub(crate) fn new() -> Self {
        Resources {
            buffers: SlotMap::with_key(),
            named_buffers: HashMap::new(),
            sprite_sheets: SlotMap::with_key(),
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
