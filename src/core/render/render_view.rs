use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};
use winit::dpi::PhysicalSize;

use crate::{
    core::{render::render::TextureHandle, scene::scene_handler::SceneHandle},
    systems::camera::{Camera, CameraAnimator},
};

#[derive(Clone)]
pub enum RenderTarget {
    Window {
        size: PhysicalSize<u32>,
    },
    Texture {
        texture: TextureHandle,
        view_index: usize,
        depth_texture: Option<(TextureHandle, usize)>,
        size: PhysicalSize<u32>,
        format: wgpu::TextureFormat,
    },
}

impl RenderTarget {
    pub fn window(size: PhysicalSize<u32>) -> Self {
        Self::Window { size }
    }

    pub fn texture(
        texture: TextureHandle,
        view_index: usize,
        depth_texture: Option<(TextureHandle, usize)>,
        size: PhysicalSize<u32>,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self::Texture {
            texture,
            view_index,
            depth_texture,
            size,
            format,
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        match self {
            Self::Window { size } | Self::Texture { size, .. } => *size,
        }
    }

    pub fn resize_window(&mut self, new_size: PhysicalSize<u32>) -> bool {
        match self {
            Self::Window { size } => {
                *size = new_size;
                true
            }
            Self::Texture { .. } => false,
        }
    }

    pub fn is_window(&self) -> bool {
        matches!(self, Self::Window { .. })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderViewRole {
    #[default]
    Main,
    Auxiliary,
}

pub struct RenderView {
    pub scene: SceneHandle,
    pub camera: Camera,
    pub camera_animator: Option<CameraAnimator>,
    pub render_target: RenderTarget,
    pub enabled: bool,
    pub role: RenderViewRole,
}

impl RenderView {
    pub fn sync_camera_to_target(&mut self) {
        let size = self.render_target.size();
        self.camera
            .resize(PhysicalSize::new(size.width as f32, size.height as f32));
    }
}

new_key_type! { pub struct RenderViewHandle; }

#[derive(Default)]
pub struct RenderViewHandler {
    pub(crate) views: SlotMap<RenderViewHandle, RenderView>,
    view_lookup: HashMap<String, RenderViewHandle>,
}

impl RenderViewHandler {
    pub fn get_render_view_from_name(&self, name: &str) -> &RenderView {
        let handle = self.view_lookup.get(name).expect("unknown RenderView name");
        self.get_render_view(*handle)
    }

    pub fn get_render_view_from_name_mut(&mut self, name: &str) -> &mut RenderView {
        let handle = self.view_lookup.get(name).expect("unknown RenderView name");
        self.get_render_view_mut(*handle)
    }

    pub fn get_render_view(&self, handle: RenderViewHandle) -> &RenderView {
        self.views.get(handle).expect("invalid RenderViewHandle")
    }

    pub fn get_render_view_mut(&mut self, handle: RenderViewHandle) -> &mut RenderView {
        self.views
            .get_mut(handle)
            .expect("invalid RenderViewHandle")
    }

    pub fn iter(&self) -> impl Iterator<Item = (RenderViewHandle, &RenderView)> {
        self.views.iter()
    }

    pub(crate) fn new_view(
        &mut self,
        render_target: RenderTarget,
        scene: SceneHandle,
        name: impl Into<String>,
        role: RenderViewRole,
    ) -> RenderViewHandle {
        let name = name.into();
        assert!(
            !self.view_lookup.contains_key(&name),
            "RenderView name '{name}' is already in use"
        );
        let camera = Camera::new(render_target.clone(), 75.0, 50.0);
        let handle = self.views.insert(RenderView {
            scene,
            camera,
            camera_animator: None,
            render_target,
            enabled: true,
            role,
        });
        self.view_lookup.insert(name, handle);
        handle
    }

    pub fn set_enabled(&mut self, handle: RenderViewHandle, enabled: bool) {
        self.get_render_view_mut(handle).enabled = enabled;
    }

    pub fn set_role(&mut self, handle: RenderViewHandle, role: RenderViewRole) {
        self.get_render_view_mut(handle).role = role;
    }
    pub fn set_role_from_name(&mut self, name: &str, role: RenderViewRole) {
        let view = self.get_render_view_from_name_mut(name);
        view.role = role;
    }

    pub fn enabled_handles(&self) -> impl Iterator<Item = RenderViewHandle> + '_ {
        self.views
            .iter()
            .filter_map(|(handle, view)| view.enabled.then_some(handle))
    }

    pub(crate) fn remove(&mut self, handle: RenderViewHandle) -> Option<RenderView> {
        let view = self.views.remove(handle)?;
        self.view_lookup.retain(|_, value| *value != handle);
        Some(view)
    }

    pub(crate) fn remove_for_scene(&mut self, scene: SceneHandle) -> Vec<RenderViewHandle> {
        let handles = self
            .views
            .iter()
            .filter_map(|(handle, view)| (view.scene == scene).then_some(handle))
            .collect::<Vec<_>>();
        for handle in &handles {
            self.remove(*handle);
        }
        handles
    }

    pub(crate) fn resize_window_targets(&mut self, size: PhysicalSize<u32>) {
        for (_, view) in self.views.iter_mut() {
            if view.render_target.resize_window(size) {
                view.sync_camera_to_target();
            }
        }
    }
}
