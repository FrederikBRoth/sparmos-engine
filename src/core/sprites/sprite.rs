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

const PIXELS_PER_UNIT: f32 = 32.0;
impl Sprite {
    pub fn new(
        gfx: &mut Graphics,
        sprite_sheet: SpriteSheetHandle,
        sprite_name: &str,
        material: MaterialHandle,
        position: Vector3<f32>,
    ) -> RenderableHandle {
        //TODO: should not recreate meshes every time
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

        let instance_scale = Vector3::new(
            frame.w as f32 / PIXELS_PER_UNIT,
            frame.h as f32 / PIXELS_PER_UNIT,
            1.0,
        );

        let mut instance = Instance::new(position, instance_scale);
        instance.uv = uv_rect;
        let instance_controller = gfx
            .instances_typed::<SpriteInstanceLayout>()
            .from_instances([instance].into())
            .build();

        gfx.add_renderable(material, mesh, instance_controller)
    }
}
