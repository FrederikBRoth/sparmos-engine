use std::collections::HashMap;

use egui::TextureHandle;
use slotmap::{SlotMap, new_key_type};
use winit::dpi::PhysicalSize;

use crate::{core::scene::scene_handler::SceneHandle, systems::camera::Camera};

///A Camera can render to multiple targets. TextureBuffer for other shaders to use (portals,
///picture in picture) etc
//TODO: Add more target variants. FixedSize etc
#[derive(Clone)]
pub enum RenderTarget {
    Fullscreen(PhysicalSize<f32>),
    TextureBuffer(TextureHandle),
}
pub enum RenderViewMode {
    Disabled,
    Main,
    Auxiliary,
}
pub struct RenderView {
    pub scene: SceneHandle,
    pub camera: Camera,
    pub render_target: RenderTarget,
    pub render_view_mode: RenderViewMode,
}

new_key_type! { pub struct RenderViewHandle; }

#[derive(Default)]
pub struct RenderViewHandler {
    pub views: SlotMap<RenderViewHandle, RenderView>,
    view_lookup: HashMap<String, RenderViewHandle>,
}

impl RenderViewHandler {
    pub fn get_render_view_from_name(&self, name: &str) -> &RenderView {
        let handle = self.view_lookup.get(name).unwrap();
        self.get_render_view(*handle)
    }
    pub fn get_render_view_from_name_mut(&mut self, name: &str) -> &mut RenderView {
        let handle = self.view_lookup.get(name).unwrap();
        self.get_render_view_mut(*handle)
    }

    pub fn get_render_view(&self, handle: RenderViewHandle) -> &RenderView {
        self.views.get(handle).unwrap()
    }
    pub fn get_render_view_mut(&mut self, handle: RenderViewHandle) -> &mut RenderView {
        self.views.get_mut(handle).unwrap()
    }

    pub fn new_view(
        &mut self,
        render_target: RenderTarget,
        scene_handle: SceneHandle,
        view_name: String,
    ) -> RenderViewHandle {
        let camera = Camera::new(render_target.clone(), 75.0, 50.0);

        let handle = &self.views.insert(RenderView {
            scene: scene_handle,
            camera,
            render_target,
            render_view_mode: RenderViewMode::Disabled,
        });
        self.view_lookup.insert(view_name, *handle);
        *handle
    }

    pub fn set_viewmode(&mut self, view_handle: RenderViewHandle, view_mode: RenderViewMode) {
        self.get_render_view_mut(view_handle).render_view_mode = view_mode;
    }
}
