use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};
use winit::dpi::PhysicalSize;

use crate::{
    core::{
        render::render::{RenderContext, TextureHandle},
        scene::scene_handler::SceneHandle,
    },
    systems::camera::{Camera, CameraAnimator, ClipPlane},
};

pub(crate) struct ResolvedRenderTarget<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: Option<&'a wgpu::TextureView>,
    pub size: PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
}

impl<'a> ResolvedRenderTarget<'a> {
    pub(crate) fn post_process_scene(
        render_context: &'a RenderContext,
        color: &'a wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            color,
            depth: Some(&render_context.window_targets.overscan_depth.view),
            size: crate::core::post_processing::PostProcessHandler::overscan_size(
                PhysicalSize::new(render_context.config.width, render_context.config.height),
            ),
            format,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextureRenderTargetConfig {
    pub size: PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
    pub depth_enabled: bool,
}

impl TextureRenderTargetConfig {
    pub fn new(size: PhysicalSize<u32>, format: wgpu::TextureFormat) -> Self {
        Self {
            size,
            format,
            depth_enabled: true,
        }
    }

    pub fn without_depth(mut self) -> Self {
        self.depth_enabled = false;
        self
    }

    pub fn depth_enabled(mut self, enabled: bool) -> Self {
        self.depth_enabled = enabled;
        self
    }
}

#[derive(Clone)]
pub struct RenderTarget {
    kind: RenderTargetKind,
}

#[derive(Clone)]
enum RenderTargetKind {
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
        Self {
            kind: RenderTargetKind::Window { size },
        }
    }

    pub(crate) fn texture(
        texture: TextureHandle,
        view_index: usize,
        depth_texture: Option<(TextureHandle, usize)>,
        size: PhysicalSize<u32>,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            kind: RenderTargetKind::Texture {
                texture,
                view_index,
                depth_texture,
                size,
                format,
            },
        }
    }

    pub fn color_texture(&self) -> Option<TextureHandle> {
        match &self.kind {
            RenderTargetKind::Texture { texture, .. } => Some(*texture),
            RenderTargetKind::Window { .. } => None,
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        match &self.kind {
            RenderTargetKind::Window { size } | RenderTargetKind::Texture { size, .. } => *size,
        }
    }

    pub fn resize_window(&mut self, new_size: PhysicalSize<u32>) -> bool {
        match &mut self.kind {
            RenderTargetKind::Window { size } => {
                *size = new_size;
                true
            }
            RenderTargetKind::Texture { .. } => false,
        }
    }

    pub fn is_window(&self) -> bool {
        matches!(&self.kind, RenderTargetKind::Window { .. })
    }

    /// Reallocate a texture target, retaining its color/depth handles.
    /// Materials sampling it must be rebound with Graphics::set_material_texture.
    /// Cloned targets and externally retained texture views are not resized.
    pub fn resize_texture(
        &mut self,
        gfx: &mut crate::application::graphics::Graphics,
        new_size: PhysicalSize<u32>,
    ) -> bool {
        assert!(new_size.width > 0 && new_size.height > 0);
        let RenderTargetKind::Texture {
            texture,
            depth_texture,
            size,
            format,
            ..
        } = &mut self.kind
        else {
            return false;
        };
        if *size == new_size {
            return false;
        }
        let label = gfx.get_texture(*texture).label.clone();
        let color = gfx.texture(&label).render_target(new_size, *format).build();
        gfx.engine.render_context.gpu_objects.textures[*texture] = color;
        if let Some((handle, _)) = depth_texture {
            let label = gfx.get_texture(*handle).label.clone();
            let depth = gfx.texture(&label).depth_target(new_size).build();
            gfx.engine.render_context.gpu_objects.textures[*handle] = depth;
        }
        *size = new_size;
        true
    }

    pub(crate) fn resolve<'a>(
        &'a self,
        render_context: &'a RenderContext,
        surface_view: Option<&'a wgpu::TextureView>,
    ) -> ResolvedRenderTarget<'a> {
        match &self.kind {
            RenderTargetKind::Window { size } => ResolvedRenderTarget {
                color: surface_view.expect("a window RenderTarget requires a surface view"),
                depth: Some(&render_context.window_targets.depth.view),
                size: *size,
                format: render_context.config.format,
            },
            RenderTargetKind::Texture {
                texture,
                view_index,
                depth_texture,
                size,
                format,
            } => {
                let textures = &render_context.gpu_objects.textures;
                let color_texture = textures
                    .get(*texture)
                    .expect("RenderTarget references a deleted color texture");
                let color = &color_texture
                    .texture
                    .get(*view_index)
                    .expect("RenderTarget color view index is out of bounds")
                    .view;
                let depth = depth_texture.map(|(handle, index)| {
                    &textures
                        .get(handle)
                        .expect("RenderTarget references a deleted depth texture")
                        .texture
                        .get(index)
                        .expect("RenderTarget depth view index is out of bounds")
                        .view
                });
                ResolvedRenderTarget {
                    color,
                    depth,
                    size: *size,
                    format: *format,
                }
            }
        }
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
    pub render_mask: RenderMask,
    pub clip_plane: Option<ClipPlane>,
    /// False for cameras whose complete pose is supplied by game logic.
    pub simulate_camera: bool,
}

/// Optional ECS draw layer. Untagged registrations use the ordinary world layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderLayer(pub u32);

impl Default for RenderLayer {
    fn default() -> Self {
        Self(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderMask(pub u32);

impl Default for RenderMask {
    fn default() -> Self {
        Self(u32::MAX)
    }
}

impl RenderMask {
    pub fn includes(self, layer: Option<&RenderLayer>) -> bool {
        self.0 & layer.copied().unwrap_or_default().0 != 0
    }
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
        let camera = Camera::new(render_target.size(), 75.0, 50.0);
        self.new_view_with_camera(render_target, scene, name, role, camera)
    }
    pub(crate) fn new_view_with_camera(
        &mut self,
        render_target: RenderTarget,
        scene: SceneHandle,
        name: impl Into<String>,
        role: RenderViewRole,
        mut camera: Camera,
    ) -> RenderViewHandle {
        let name = name.into();
        assert!(
            !self.view_lookup.contains_key(&name),
            "RenderView name '{name}' is already in use"
        );
        let size = render_target.size();
        camera.resize(PhysicalSize::new(size.width as f32, size.height as f32));
        let handle = self.views.insert(RenderView {
            scene,
            camera,
            camera_animator: None,
            render_target,
            enabled: true,
            role,
            render_mask: RenderMask::default(),
            clip_plane: None,
            simulate_camera: true,
        });
        self.view_lookup.insert(name, handle);
        handle
    }

    pub fn set_enabled(&mut self, handle: RenderViewHandle, enabled: bool) {
        self.get_render_view_mut(handle).enabled = enabled;
    }

    pub(crate) fn update_cameras(&mut self, dt: std::time::Duration) {
        for (_, view) in self.views.iter_mut() {
            if !view.enabled || !view.simulate_camera {
                continue;
            }
            view.camera.update_camera(dt);
            if let Some(animator) = &mut view.camera_animator {
                animator.update(dt.as_secs_f32(), &mut view.camera);
            }
        }
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

    pub(crate) fn enabled_offscreen_handles(&self) -> impl Iterator<Item = RenderViewHandle> + '_ {
        self.views.iter().filter_map(|(handle, view)| {
            (view.enabled && !view.render_target.is_window()).then_some(handle)
        })
    }

    pub(crate) fn presenting_handle(&self) -> Option<RenderViewHandle> {
        self.views.iter().find_map(|(handle, view)| {
            (view.enabled && view.role == RenderViewRole::Main && view.render_target.is_window())
                .then_some(handle)
        })
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
