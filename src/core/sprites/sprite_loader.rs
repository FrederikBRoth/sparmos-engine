use std::collections::HashMap;

use serde::Deserialize;

use crate::{application::graphics::Graphics, core::render::TextureHandle};

#[derive(Debug, Deserialize)]
pub struct SpriteSheetData {
    frames: HashMap<String, SpriteFrame>,
    meta: SpriteSheetMeta,
}
#[derive(Clone)]
pub struct SpriteSheet {
    pub(crate) frames: HashMap<String, SpriteFrame>,
    pub(crate) meta: SpriteSheetMeta,
    pub texture_handle: TextureHandle,
}

impl SpriteSheet {
    pub fn from_bytes(gfx: &mut Graphics, data: &[u8], image_data: &[u8]) -> SpriteSheet {
        let sheet: SpriteSheetData = serde_json::from_slice(data).unwrap();
        let texture = gfx
            .texture(&sheet.meta.image)
            .bytes(image_data, wgpu::TextureFormat::Rgba8UnormSrgb)
            .linear()
            .build();

        let handle = gfx
            .get_render_context_mut()
            .gpu_objects
            .textures
            .insert(texture);

        Self {
            frames: sheet.frames,
            meta: sheet.meta,
            texture_handle: handle,
        }
    }

    pub(crate) fn normalized_uv_rect(&self, frame: &Rect) -> cgmath::Vector4<f32> {
        normalize_uv_rect(frame, &self.meta.size)
    }
}

fn normalize_uv_rect(frame: &Rect, sheet: &SpriteSheetSize) -> cgmath::Vector4<f32> {
    cgmath::Vector4::new(
        frame.x as f32 / sheet.w as f32,
        frame.y as f32 / sheet.h as f32,
        frame.w as f32 / sheet.w as f32,
        frame.h as f32 / sheet.h as f32,
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpriteSheetSize {
    w: u32,
    h: u32,
}
#[derive(Debug, Clone, Deserialize)]
pub struct SpriteSheetMeta {
    size: SpriteSheetSize,
    image: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct SpriteFrame {
    pub frame: Rect,
    pub duration: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[cfg(test)]
mod tests {
    use super::{Rect, SpriteSheetSize, normalize_uv_rect};

    #[test]
    fn normalizes_aseprite_pixel_rect() {
        let uv = normalize_uv_rect(
            &Rect {
                x: 64,
                y: 55,
                w: 37,
                h: 42,
            },
            &SpriteSheetSize { w: 115, h: 181 },
        );

        assert_eq!(uv.x, 64.0 / 115.0);
        assert_eq!(uv.y, 55.0 / 181.0);
        assert_eq!(uv.z, 37.0 / 115.0);
        assert_eq!(uv.w, 42.0 / 181.0);
    }
}
