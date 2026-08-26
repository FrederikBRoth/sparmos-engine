use std::{any::TypeId, cell::Ref};

use cgmath::{
    EuclideanSpace, InnerSpace, Point3, Quaternion, Rad, Rotation, Rotation3, SquareMatrix,
    Vector3, Vector4,
};
use wgpu::{BindGroupLayout, Device};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    application::graphics::Graphics,
    core::{
        buffer::{Buffer, BufferType, UniformParameters},
        engine::System,
        entities::World,
        render::RenderContext,
        resource::BufferHandle,
    },
    helpers::line_trace::OPENGL_TO_WGPU_MATRIX,
    systems::animation::{AnimationHandler, AnimationType},
};

pub struct CameraAnimator {
    pub disabled: bool,
    pub speed: f32,
    pub eye_animator: AnimationHandler,
    pub target_animator: AnimationHandler,
}

#[derive(PartialEq, Eq)]
pub enum MovementPress {
    Pressed,
    NotPressed,
    Override,
}

pub enum MovementKey {
    Up,
    Down,
    Forward,
    Backward,
    Left,
    Right,
    TiltUp,
    TiltDown,
    TurnLeft,
    TurnRight,
    RotateLeft,
    RotateRight,
}

impl MovementPress {
    fn is_pressed(&self) -> bool {
        match self {
            MovementPress::Pressed | MovementPress::Override => true,

            MovementPress::NotPressed => false,
        }
    }
}
impl CameraAnimator {
    pub fn new(speed: f32, eye_start: Point3<f32>, target_start: Point3<f32>) -> CameraAnimator {
        let eye_ah = AnimationHandler::new_from_point(eye_start, vec![]);
        let target_ah = AnimationHandler::new_from_point(target_start, vec![]);

        CameraAnimator {
            disabled: true,
            speed,
            eye_animator: eye_ah,
            target_animator: target_ah,
        }
    }

    pub fn update(&mut self, dt: f32, camera: &mut Camera) {
        if self.disabled {
            self.eye_animator
                .movement_list
                .get_mut(0)
                .unwrap()
                .base_position = camera.eye.to_vec();
            self.target_animator
                .movement_list
                .get_mut(0)
                .unwrap()
                .base_position = camera.target.to_vec();
            return;
        }
        self.eye_animator.update_point(dt, &mut camera.eye);
        self.target_animator.update_point(dt, &mut camera.target);
    }

    pub fn add_animation(
        &mut self,
        eye_anim: Option<AnimationType>,
        target_anim: Option<AnimationType>,
    ) {
        if let Some(anim) = eye_anim {
            self.eye_animator.add_animation(anim, 0);
        }
        if let Some(anim) = target_anim {
            self.target_animator.add_animation(anim, 0);
        }
    }

    pub fn reset_animation(&mut self, camera: &mut Camera) {
        self.eye_animator
            .reset_point_position_to_current_position(&mut camera.eye);
        self.target_animator
            .reset_point_position_to_current_position(&mut camera.target);
    }
}
pub enum CameraMode {
    FreeMode,
    AnimatedMode,
}

pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub forward: Vector3<f32>,
    pub camera_mode: CameraMode,

    pub auto: bool,
    pub speed: f32,

    pub sensitivity: f32,

    pub is_up_pressed: MovementPress,
    pub is_down_pressed: MovementPress,
    pub is_forward_pressed: MovementPress,
    pub is_backward_pressed: MovementPress,
    pub is_left_pressed: MovementPress,
    pub is_right_pressed: MovementPress,
    pub is_tilt_up_pressed: MovementPress,
    pub is_tilt_down_pressed: MovementPress,
    pub is_turn_left_pressed: MovementPress,
    pub is_turn_right_pressed: MovementPress,
    pub rotate_left: MovementPress,
    pub rotate_right: MovementPress,
}

impl Camera {
    pub fn new(screen_size: PhysicalSize<f32>, speed: f32, sensitivity: f32) -> Self {
        let eye = Point3::new(0.0, 0.0, -400.0);
        let target = Point3::new(0.0, 0.0, 0.0);

        let mut camera = Camera {
            eye,
            target,
            up: cgmath::Vector3::unit_y(),
            forward: Vector3::unit_z(),
            yaw: 90.0,
            pitch: 0.0,
            aspect: screen_size.width / screen_size.height,
            fovy: 90.0,
            znear: 0.1,
            zfar: 5000.0,
            camera_mode: CameraMode::FreeMode,
            is_up_pressed: MovementPress::NotPressed,
            is_down_pressed: MovementPress::NotPressed,
            is_forward_pressed: MovementPress::NotPressed,
            is_backward_pressed: MovementPress::NotPressed,
            is_left_pressed: MovementPress::NotPressed,
            is_right_pressed: MovementPress::NotPressed,
            is_tilt_up_pressed: MovementPress::NotPressed,
            is_tilt_down_pressed: MovementPress::NotPressed,
            is_turn_left_pressed: MovementPress::NotPressed,
            is_turn_right_pressed: MovementPress::NotPressed,
            rotate_left: MovementPress::NotPressed,
            rotate_right: MovementPress::NotPressed,
            auto: false,
            speed,
            sensitivity,
        };
        camera.update_forward();
        camera
    }
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        self.build_projection_matrix() * self.build_view_matrix()
    }

    fn build_view_matrix(&self) -> cgmath::Matrix4<f32> {
        match self.camera_mode {
            CameraMode::FreeMode => {
                cgmath::Matrix4::look_at_rh(self.eye, self.eye + self.forward, self.up)
            }

            CameraMode::AnimatedMode => cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up),
        }
    }

    fn build_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar)
    }
    pub fn screen_to_world_ray(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        screen_width: f32,
        screen_height: f32,
    ) -> (Point3<f32>, Vector3<f32>) {
        // Convert screen coords to normalized device coordinates (NDC)
        let front = self
            .project_screen_to_world(mouse_x, mouse_y, 1.0, screen_width, screen_height)
            .unwrap();
        let back = self
            .project_screen_to_world(mouse_x, mouse_y, 0.0, screen_width, screen_height)
            .unwrap();

        let test = (Point3::from_vec(back), -(front - back).normalize());
        // println!("{:?}", test);
        test
    }

    pub fn project_screen_to_world(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        mouse_z: f32,
        screen_width: f32,
        screen_height: f32,
    ) -> Option<Vector3<f32>> {
        let view_projection = OPENGL_TO_WGPU_MATRIX * self.build_view_projection_matrix();
        if let Some(inv_view_projection) = view_projection.invert() {
            let world = Vector4::new(
                (mouse_x) / (screen_width) * 2.0 - 1.0,
                // Screen Origin is Top Left    (Mouse Origin is Top Left)
                //          (screen.y - (viewport.y as f32)) / (viewport.w as f32) * 2.0 - 1.0,
                // Screen Origin is Bottom Left (Mouse Origin is Top Left)
                (1.0 - (mouse_y) / screen_height) * 2.0 - 1.0,
                mouse_z * 2.0 - 1.0,
                1.0,
            );
            let world = inv_view_projection * world;

            if world.w != 0.0 {
                Some(world.truncate() * (1.0 / world.w))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn update_forward(&mut self) {
        let yaw_rad: f32 = self.yaw.to_radians();
        let pitch_rad: f32 = self.pitch.to_radians();

        let forward: Vector3<f32> = Vector3 {
            x: yaw_rad.cos() * pitch_rad.cos(),
            y: pitch_rad.sin(),
            z: yaw_rad.sin() * pitch_rad.cos(),
        };

        self.forward = forward.normalize();
    }

    pub fn set_camera_mode(&mut self, mode: CameraMode) {
        self.camera_mode = mode;
    }

    pub fn process_events(&mut self, event: &WindowEvent) -> bool {
        if let CameraMode::AnimatedMode = self.camera_mode {
            self.reset_input();
            return false;
        }
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => {
                let var_name = *state == ElementState::Pressed;
                let is_pressed = var_name;
                match keycode {
                    KeyCode::ShiftLeft => {
                        self.is_up_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::ControlLeft => {
                        self.is_down_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::KeyW => {
                        self.is_forward_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::KeyA => {
                        self.is_left_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::KeyS => {
                        self.is_backward_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::KeyD => {
                        self.is_right_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::ArrowUp => {
                        self.is_tilt_up_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::ArrowDown => {
                        self.is_tilt_down_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::ArrowLeft => {
                        self.is_turn_left_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::ArrowRight => {
                        self.is_turn_right_pressed = get_press_from_bool(is_pressed);
                        true
                    }
                    KeyCode::KeyU => {
                        println!(
                            "Position: {:?}, Yaw: {:?}, Pitch {:?}",
                            self.eye, self.yaw, self.pitch
                        );
                        true
                    }
                    _ => false,
                }
            }

            _ => false,
        }
    }

    pub fn process_mouse(&mut self, delta_x: f32, delta_y: f32, camera: &mut Camera) {
        camera.yaw += delta_x * self.sensitivity;
        camera.pitch = (camera.pitch + delta_y * self.sensitivity).clamp(-89.0, 89.0);
        camera.update_forward();
    }

    pub fn update_camera(&mut self, dt: std::time::Duration) {
        let right: Vector3<f32> = self.forward.cross(self.up).normalize();

        if self.is_forward_pressed.is_pressed() {
            self.eye += self.forward * self.speed * dt.as_secs_f32();
        }
        if self.is_backward_pressed.is_pressed() {
            self.eye -= self.forward * self.speed * dt.as_secs_f32();
        }
        if self.is_right_pressed.is_pressed() {
            self.eye += right * self.speed * dt.as_secs_f32();
        }
        if self.is_left_pressed.is_pressed() {
            self.eye -= right * self.speed * dt.as_secs_f32();
        }
        if self.is_up_pressed.is_pressed() {
            self.eye += self.up * self.speed * dt.as_secs_f32();
        }
        if self.is_down_pressed.is_pressed() {
            self.eye -= self.up * self.speed * dt.as_secs_f32();
        }
        if self.is_tilt_up_pressed.is_pressed() {
            self.pitch -= self.sensitivity * dt.as_secs_f32();
            self.update_forward();
        }
        if self.is_tilt_down_pressed.is_pressed() {
            self.pitch += self.sensitivity * dt.as_secs_f32();
            self.update_forward();
        }
        if self.is_turn_left_pressed.is_pressed() {
            self.yaw -= self.sensitivity * dt.as_secs_f32();
            self.update_forward();
        }
        if self.is_turn_right_pressed.is_pressed() {
            self.yaw += self.sensitivity * dt.as_secs_f32();
            self.update_forward();
        }

        if self.rotate_left.is_pressed() {
            let dt = dt.as_secs_f32();

            // vector from target → eye
            let offset = self.eye - self.target;

            // preserve radius
            let radius = offset.magnitude();

            // create rotation around the up axis
            let rotation = Quaternion::from_axis_angle(self.up.normalize(), Rad(self.speed * dt));

            // rotate the offset
            let new_offset = rotation.rotate_vector(offset);

            // reapply radius (avoids drift)
            self.eye = self.target + new_offset.normalize() * radius;
        }
        if self.rotate_right.is_pressed() {
            // self.camera.eye =
            //     self.camera.target - (forward - right * self.speed).normalize() * forward_mag;
        }
        // if self.is_right_pressed {
        //     // Rescale the distance between the target and eye so
        //     // that it doesn't change. The eye therefore still
        //     // lies on the circle made by the target and eye.
        //     self.camera.eye -= right * self.speed;
        //     self.camera.target -= right * self.speed;
        // }
        // if self.is_left_pressed {
        //     self.camera.eye += right * self.speed;
        //     self.camera.target += right * self.speed;
        // }
    }

    fn reset_input(&mut self) {
        reset_if_not_override(&mut self.is_up_pressed);
        reset_if_not_override(&mut self.is_down_pressed);
        reset_if_not_override(&mut self.is_forward_pressed);
        reset_if_not_override(&mut self.is_backward_pressed);
        reset_if_not_override(&mut self.is_left_pressed);
        reset_if_not_override(&mut self.is_right_pressed);
        reset_if_not_override(&mut self.is_tilt_up_pressed);
        reset_if_not_override(&mut self.is_tilt_down_pressed);
        reset_if_not_override(&mut self.is_turn_left_pressed);
        reset_if_not_override(&mut self.is_turn_right_pressed);
    }
    pub fn set(&mut self, key: MovementKey, state: MovementPress) {
        match key {
            MovementKey::Up => self.is_up_pressed = state,
            MovementKey::Down => self.is_down_pressed = state,
            MovementKey::Forward => self.is_forward_pressed = state,
            MovementKey::Backward => self.is_backward_pressed = state,
            MovementKey::Left => self.is_left_pressed = state,
            MovementKey::Right => self.is_right_pressed = state,
            MovementKey::TiltUp => self.is_tilt_up_pressed = state,
            MovementKey::TiltDown => self.is_tilt_down_pressed = state,
            MovementKey::TurnLeft => self.is_turn_left_pressed = state,
            MovementKey::TurnRight => self.is_turn_right_pressed = state,
            MovementKey::RotateLeft => self.rotate_left = state,
            MovementKey::RotateRight => self.rotate_right = state,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            proj: cgmath::Matrix4::identity().into(),

            view: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_position = camera.eye.to_homogeneous().into();
        self.view = camera.build_view_matrix().into();
        self.proj = (OPENGL_TO_WGPU_MATRIX * camera.build_projection_matrix()).into();
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CameraSystem {
    pub camera_uniform: CameraUniform,
    pub camera_buffer: Buffer,
}

impl CameraSystem {
    pub fn new(gfx: &mut Graphics, camera: &Camera) -> Self {
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(camera);
        let camera_buffer = Buffer::new_init(
            &[camera_uniform],
            gfx.get_device_mut(),
            BufferType::UniformBuffer(UniformParameters::default()),
        );

        log::warn!("Shader");
        Self {
            camera_buffer,
            camera_uniform,
        }
    }

    pub fn update_camera(&mut self, camera: &Camera, rc: &mut RenderContext) {
        self.camera_uniform.update_view_proj(camera);
        rc.queue.write_buffer(
            &self.camera_buffer.buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }
}
fn reset_if_not_override(v: &mut MovementPress) {
    if *v != MovementPress::Override {
        *v = MovementPress::NotPressed;
    }
}

fn get_press_from_bool(state: bool) -> MovementPress {
    if state {
        MovementPress::Pressed
    } else {
        MovementPress::NotPressed
    }
}
pub fn normalize_and_map_camera_height(x: i64, a: i64, b: i64, start: f32, end: f32) -> f32 {
    if a == b {
        return end;
    }

    let x = x as f32;
    let a = a as f32;
    let b = b as f32;

    let normalized = (x - a) / (b - a);

    // Map from 0.0–1.0 to -25.0–25.0
    start + (end * 2.0) * normalized
}

impl System for CameraSystem {
    fn run(&mut self, world: Ref<'_, World>, rc: &mut RenderContext, dt: std::time::Duration) {
        world.query_first::<(&mut Camera, &mut CameraAnimator)>(|(camera, camera_animator)| {
            camera.update_camera(dt);
            self.update_camera(camera, rc);
            camera_animator.update(dt.as_secs_f32(), camera);
        });
    }

    fn get_buffer(&self) -> &Buffer {
        &self.camera_buffer
    }
    // fn register(self, resources: &mut crate::core::resource::Resources) {
    //     let type_id = TypeId::of::<Self>();
    //
    //     resources.buffers.insert(self.camera_buffer.clone());
    //     resources.resource_map.insert(type_id, Box::new(self));
    // }
}
