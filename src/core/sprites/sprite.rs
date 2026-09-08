use cgmath::Vector3;

use crate::{
    application::graphics::Graphics,
    core::{
        instance::{Instance, SpriteInstanceLayout},
        render::{MaterialHandle, RenderableHandle},
        resource::SpriteSheetHandle,
    },
    entities::meshes::Meshes,
};

pub struct Sprite;

const PIXELS_PER_UNIT: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteSpace {
    World,
    Screen,
}

fn sprite_scale(width: f32, height: f32, space: SpriteSpace) -> Vector3<f32> {
    let units_per_pixel = match space {
        SpriteSpace::World => 1.0 / PIXELS_PER_UNIT,
        SpriteSpace::Screen => 1.0,
    };
    Vector3::new(width * units_per_pixel, height * units_per_pixel, 1.0)
}

impl Sprite {
    pub fn new(
        gfx: &mut Graphics,
        sprite_sheet: SpriteSheetHandle,
        sprite_name: &str,
        material: MaterialHandle,
        position: Vector3<f32>,
    ) -> RenderableHandle {
        Self::new_in_space(
            gfx,
            sprite_sheet,
            sprite_name,
            material,
            position,
            SpriteSpace::World,
        )
    }

    /// Creates a pixel-sized sprite for a material whose shader consumes screen coordinates.
    pub fn new_screen_space(
        gfx: &mut Graphics,
        sprite_sheet: SpriteSheetHandle,
        sprite_name: &str,
        material: MaterialHandle,
        position: Vector3<f32>,
    ) -> RenderableHandle {
        Self::new_in_space(
            gfx,
            sprite_sheet,
            sprite_name,
            material,
            position,
            SpriteSpace::Screen,
        )
    }

    pub fn new_in_space(
        gfx: &mut Graphics,
        sprite_sheet: SpriteSheetHandle,
        sprite_name: &str,
        material: MaterialHandle,
        position: Vector3<f32>,
        space: SpriteSpace,
    ) -> RenderableHandle {
        let sprite_sheet = &gfx.engine.resources.sprite_sheets[sprite_sheet];
        let sprite_frame = sprite_sheet
            .frames
            .get(sprite_name)
            .unwrap_or_else(|| panic!("sprite frame '{sprite_name}' does not exist"));
        let frame = sprite_frame.frame.clone();
        let uv_rect = sprite_sheet.normalized_uv_rect(&frame);
        let mesh = Meshes::Sprite
            .create()
            .make_mb(gfx.get_render_context_mut());

        let instance_scale = sprite_scale(frame.w as f32, frame.h as f32, space);

        let mut instance = Instance::new(position, instance_scale);
        instance.uv = uv_rect;
        let instance_controller = gfx
            .instances_typed::<SpriteInstanceLayout>()
            .from_instances([instance].into())
            .build();

        gfx.add_renderable(material, mesh, instance_controller)
    }
}
