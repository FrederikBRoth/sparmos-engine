use cgmath::{EuclideanSpace, InnerSpace, Point3, SquareMatrix, Vector3, Vector4};
use wgpu::{BindGroupLayout, Device, util::DeviceExt};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{
    entity::core::{
        render::GlobalRenderContext,
        resource::{GpuBindable, System},
    },
    helpers::{animation::Linear, line_trace::OPENGL_TO_WGPU_MATRIX},
};

pub struct CameraAnimator {
    pub speed: f32,
    pub animating: bool,
    pub time: f32,
    pub start_eye: Point3<f32>,
    pub end_eye: Point3<f32>,
    pub start_target: Point3<f32>,
    pub end_target: Point3<f32>,
    pub aspect_ratio_limit: f32,
    pub height_modifier: f32,
}

impl CameraAnimator {
    pub fn lerp(&self) -> (Point3<f32>, Point3<f32>) {
        let lerp_value = Linear::ease_linear(self.time);
        let eye_anim = self.start_eye + (self.end_eye - self.start_eye) * lerp_value;
        let target_anim = self.start_target + (self.end_target - self.start_target) * lerp_value;
        (eye_anim, target_anim)
    }
}

pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    // pub camera_animator: CameraAnimator,
    pub yaw: f32,
    pub pitch: f32,
    pub forward: Vector3<f32>,
}

impl Camera {
    pub fn new(screen_size: PhysicalSize<f32>) -> Self {
        let eye = Point3::new(140.0, -17.0, -382.0);
        let target = Point3::new(20.0, 25.0, 20.0);

        let mut camera = Camera {
            eye,
            target,
            up: cgmath::Vector3::unit_y(),
            forward: Vector3::unit_z(),
            yaw: 111.0,
            pitch: 1.0,
            aspect: screen_size.width / screen_size.height,
            fovy: 25.0,
            znear: 0.1,
            zfar: 100.0,
            // camera_animator: CameraAnimator {
            //     animating: false,
            //     time: 0.0,
            //     start_eye: eye,
            //     end_eye: eye,
            //     start_target: target,
            //     end_target: target,
            //     aspect_ratio_limit: 0.8,
            //     height_modifier: 0.0,
            //     speed: 100.0,
            // },
        };
        camera.update_forward();
        camera
    }
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        // let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.eye + self.forward, self.up);

        // let ortho = cgmath::ortho(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        proj * view
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

        (Point3::from_vec(back), -(front - back).normalize())
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
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_position = camera.eye.to_homogeneous().into();
        self.view_proj = (OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix()).into();
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CameraSystem {
    // pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: BindGroupLayout,
    pub auto: bool,
    pub speed: f32,

    pub sensitivity: f32,

    pub is_up_pressed: bool,
    pub is_down_pressed: bool,
    pub is_forward_pressed: bool,
    pub is_backward_pressed: bool,
    pub is_left_pressed: bool,
    pub is_right_pressed: bool,
    pub is_tilt_up_pressed: bool,
    pub is_tilt_down_pressed: bool,
    pub is_turn_left_pressed: bool,
    pub is_turn_right_pressed: bool,
}

impl CameraSystem {
    pub fn new(speed: f32, sensitivity: f32, device: &Device, camera: &Camera) -> Self {
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(camera);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create layout and bind group for camera
        let camera_bind_group_layout: wgpu::BindGroupLayout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        log::warn!("Shader");
        Self {
            auto: false,
            speed,
            sensitivity,
            camera_uniform,
            camera_bind_group_layout,
            camera_buffer,
            is_up_pressed: false,
            is_down_pressed: false,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_tilt_up_pressed: false,
            is_tilt_down_pressed: false,
            is_turn_left_pressed: false,
            is_turn_right_pressed: false,
        }
    }

    pub fn process_events(&mut self, event: &WindowEvent, camera: &mut Camera) -> bool {
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
                        self.is_up_pressed = is_pressed;
                        true
                    }
                    KeyCode::ControlLeft => {
                        self.is_down_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyW => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyA => {
                        self.is_left_pressed = is_pressed;

                        true
                    }
                    KeyCode::KeyS => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyD => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    KeyCode::ArrowUp => {
                        self.is_tilt_up_pressed = is_pressed;
                        true
                    }
                    KeyCode::ArrowDown => {
                        self.is_tilt_down_pressed = is_pressed;
                        true
                    }
                    KeyCode::ArrowLeft => {
                        self.is_turn_left_pressed = is_pressed;
                        true
                    }
                    KeyCode::ArrowRight => {
                        self.is_turn_right_pressed = is_pressed;
                        true
                    }
                    KeyCode::Insert => {
                        println!(
                            "Position: {:?}, Yaw: {:?}, Pitch {:?}",
                            camera.eye, camera.yaw, camera.pitch
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

    pub fn update_camera(
        &mut self,
        dt: std::time::Duration,
        rc: &GlobalRenderContext,
        camera: &mut Camera,
    ) {
        let right: Vector3<f32> = camera.forward.cross(camera.up).normalize();

        if self.is_forward_pressed {
            camera.eye += camera.forward * self.speed * dt.as_secs_f32();
        }
        if self.is_backward_pressed {
            camera.eye -= camera.forward * self.speed * dt.as_secs_f32();
        }
        if self.is_right_pressed {
            camera.eye += right * self.speed * dt.as_secs_f32();
        }
        if self.is_left_pressed {
            camera.eye -= right * self.speed * dt.as_secs_f32();
        }
        if self.is_up_pressed {
            camera.eye += camera.up * self.speed * dt.as_secs_f32();
        }
        if self.is_down_pressed {
            camera.eye -= camera.up * self.speed * dt.as_secs_f32();
        }
        if self.is_tilt_up_pressed {
            camera.pitch -= self.sensitivity * dt.as_secs_f32();
            camera.update_forward();
        }
        if self.is_tilt_down_pressed {
            camera.pitch += self.sensitivity * dt.as_secs_f32();
            camera.update_forward();
        }
        if self.is_turn_left_pressed {
            camera.yaw -= self.sensitivity * dt.as_secs_f32();
            camera.update_forward();
        }
        if self.is_turn_right_pressed {
            camera.yaw += self.sensitivity * dt.as_secs_f32();
            camera.update_forward();
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

        self.camera_uniform.update_view_proj(camera);
        rc.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    // pub fn animate_camera(&mut self, dt: f32) {
    //     if !self.camera.camera_animator.animating
    //         || self.camera.camera_animator.aspect_ratio_limit > self.camera.aspect
    //     {
    //         return;
    //     }
    //     self.camera.camera_animator.time += dt * self.camera.camera_animator.speed;
    //     self.camera.camera_animator.time = self.camera.camera_animator.time.clamp(0.0, 1.0);
    //
    //     let lerped = self.camera.camera_animator.lerp();
    //     // self.camera.eye = Point3::new(lerped.0.x, self.camera.eye.y, lerped.0.z);
    //     // self.camera.target = Point3::new(lerped.1.x, self.camera.target.y, lerped.1.z);
    //     self.camera.eye = lerped.0;
    //     self.camera.target = lerped.1;
    //     if self.camera.camera_animator.time >= 1.0 {
    //         self.camera.camera_animator.animating = false;
    //     }
    // }
    //
    // pub fn animate(&mut self, animation_point: (Point3<i32>, Point3<i32>), speed: f32) {
    //     // let factor = (self.camera.aspect - 1.0).max(0.0);
    //     let factor = 1.0;
    //     let end_eye: Point3<f32> = animation_point.0.cast().unwrap();
    //     let end_target: Point3<f32> = animation_point.1.cast().unwrap();
    //     self.camera.camera_animator.end_eye = Point3::new(
    //         end_eye.x * factor,
    //         end_eye.y + self.camera.camera_animator.height_modifier,
    //         end_eye.z * factor,
    //     );
    //     self.camera.camera_animator.end_target = Point3::new(
    //         end_target.x * factor,
    //         end_target.y + self.camera.camera_animator.height_modifier,
    //         end_target.z * factor,
    //     );
    //     self.camera.camera_animator.start_eye = self.camera.eye;
    //     self.camera.camera_animator.start_target = self.camera.target;
    //     self.camera.camera_animator.animating = true;
    //     self.camera.camera_animator.time = 0.0;
    //     self.camera.camera_animator.speed = speed;
    // }
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

impl GpuBindable for CameraSystem {
    fn get_bind_group_layout(&self) -> &BindGroupLayout {
        &self.camera_bind_group_layout
    }
}

impl System for CameraSystem {
    fn make_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        })
    }

    fn get_system_name(&self) -> String {
        "Camera System".to_string()
    }
}
