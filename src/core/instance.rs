use std::marker::PhantomData;

use cgmath::{InnerSpace, Matrix3, Matrix4, Quaternion, Rotation3, Vector2, Vector3, Zero};

use crate::{
    application::graphics::Graphics,
    core::{geometry::VertexBufferLayoutOwned, render::InstanceControllerHandle},
};

#[derive(Clone)]
pub struct Transform {
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: cgmath::Vector3::new(0.0, 0.0, 0.0),
            rotation: cgmath::Quaternion::from_axis_angle(
                cgmath::Vector3::unit_z(),
                cgmath::Deg(0.0),
            ), // Identity rotation,
            scale: [1.0, 1.0, 1.0].into(),
        }
    }
}

#[derive(Clone)]
pub struct InstanceController<T>
where
    T: RawInstance,
{
    pub pending: Vec<T>,
    pub instances: Vec<Instance>,
    pub offset: usize,
    pub size: usize,
    pub buffer_layout: VertexBufferLayoutOwned,
    pub instance_buffer: wgpu::Buffer,
    phantom: PhantomData<T>,
}
pub trait InstanceControllerTrait {
    fn update(&mut self, queue: &wgpu::Queue);

    fn buffer(&self) -> &wgpu::Buffer;
    fn layout(&self) -> &VertexBufferLayoutOwned;

    fn count(&self) -> usize;
    fn instances(&self) -> &Vec<Instance>;
    fn instances_mut(&mut self) -> &mut Vec<Instance>;
}
impl<T> InstanceControllerTrait for InstanceController<T>
where
    T: RawInstance,
{
    fn update(&mut self, queue: &wgpu::Queue) {
        self.pending.clear();

        self.pending.extend(
            self.instances
                .iter()
                .filter(|i| i.should_render)
                .map(T::to_raw),
        );

        let chunk_size = 10_000;
        let stride = std::mem::size_of::<T>();

        self.size = self.pending.len();
        for (i, chunk) in self.pending.chunks(chunk_size).enumerate() {
            queue.write_buffer(
                &self.instance_buffer,
                (i * chunk_size * stride) as u64,
                bytemuck::cast_slice(chunk),
            );
        }
    }

    // fn update(&self, queue: &wgpu::Queue) {
    //     let pending = Arc::clone(&self.pending);
    //     let instances = Arc::clone(&self.instances);
    //     let count_clone = Arc::clone(&self.size);
    //
    //     #[cfg(not(target_arch = "wasm32"))]
    //     std::thread::spawn(move || {
    //         let mut pending = pending.lock().unwrap();
    //
    //         pending.clear();
    //         pending.extend(
    //             instances
    //                 .read()
    //                 .unwrap()
    //                 .iter()
    //                 .filter(|i| i.should_render)
    //                 .map(T::to_raw),
    //         );
    //
    //         count_clone.store(pending.len(), std::sync::atomic::Ordering::Relaxed);
    //     });
    //
    //     #[cfg(target_arch = "wasm32")]
    //     {
    //         use wasm_bindgen_futures::spawn_local;
    //
    //         spawn_local(async move {
    //             let mut pending = pending.lock().unwrap();
    //
    //             pending.clear();
    //             pending.extend(
    //                 instances
    //                     .read()
    //                     .unwrap()
    //                     .iter()
    //                     .filter(|i| i.should_render)
    //                     .map(T::to_raw),
    //             );
    //
    //             count_clone.store(pending.len(), std::sync::atomic::Ordering::Relaxed);
    //         });
    //     }
    //
    //     let pending = self.pending.lock().unwrap();
    //
    //     let chunk_size = 10_000;
    //     let stride = std::mem::size_of::<T>();
    //
    //     for (i, chunk) in pending.chunks(chunk_size).enumerate() {
    //         queue.write_buffer(
    //             &self.instance_buffer,
    //             (i * chunk_size * stride) as u64,
    //             bytemuck::cast_slice(chunk),
    //         );
    //     }
    // }
    fn buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }

    fn layout(&self) -> &VertexBufferLayoutOwned {
        &self.buffer_layout
    }

    fn count(&self) -> usize {
        self.size
    }

    fn instances(&self) -> &Vec<Instance> {
        &self.instances
    }

    fn instances_mut(&mut self) -> &mut Vec<Instance> {
        &mut self.instances
    }
}

#[derive(Clone)]
pub struct Instance {
    pub index: u32,
    pub transform: Transform,
    pub should_render: bool,
    pub color: cgmath::Vector3<f32>,
    pub size: cgmath::Vector3<f32>,
    pub uv: cgmath::Vector4<f32>,
}
impl Default for Instance {
    fn default() -> Self {
        Self {
            index: 0,
            transform: Default::default(),
            should_render: true,
            color: cgmath::Vector3::new(1.0, 1.0, 1.0), // white
            size: cgmath::Vector3::new(1.0, 1.0, 1.0),
            uv: cgmath::Vector4::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}

impl Instance {
    pub fn new(position: cgmath::Vector3<f32>, scale: cgmath::Vector3<f32>) -> Self {
        Self {
            transform: Transform {
                position,
                scale,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstanceLayout {
    pub position: [f32; 3],
    pub scale: [f32; 3],
    pub rotation: [f32; 4],
    pub color: [f32; 3],
    pub uv_rect: [f32; 4],
}

impl RawInstance for SpriteInstanceLayout {
    fn to_raw(instance: &Instance) -> Self {
        Self {
            position: instance.transform.position.into(),
            scale: instance.transform.scale.into(),
            rotation: instance.transform.rotation.into(),
            color: instance.color.into(),
            uv_rect: instance.uv.into(),
        }
    }

    fn layout() -> VertexBufferLayoutOwned {
        use std::mem;

        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<SpriteInstanceLayout>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vec![
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as _,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // rotation quaternion
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 6]>() as _,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 10]>() as _,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 13]>() as _,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}
pub trait RawInstance: bytemuck::Pod + bytemuck::Zeroable {
    fn layout() -> VertexBufferLayoutOwned;
    fn to_raw(instance: &Instance) -> Self;
}

//Default instance layout in Sparmos Engine
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DefaultInstanceLayout {
    pub position: [f32; 3],
    pub scale: [f32; 3],
    pub rotation: [f32; 4], // quaternion
    pub color: [f32; 3],
}

impl RawInstance for DefaultInstanceLayout {
    fn layout() -> VertexBufferLayoutOwned {
        use std::mem;

        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<DefaultInstanceLayout>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vec![
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as _,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // rotation quaternion
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 6]>() as _,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 10]>() as _,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }

    fn to_raw(instance: &Instance) -> Self {
        DefaultInstanceLayout {
            position: instance.transform.position.into(),
            scale: instance.transform.scale.into(),
            rotation: instance.transform.rotation.into(), // must be quaternion
            color: instance.color.into(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    #[allow(dead_code)]
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub normal: [[f32; 3]; 3],
}

impl RawInstance for InstanceRaw {
    fn layout() -> VertexBufferLayoutOwned {
        use std::mem;
        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vec![
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We don't have to do this in code though.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 25]>() as wgpu::BufferAddress,
                    shader_location: 12,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }

    fn to_raw(instance: &Instance) -> Self {
        let s = instance.transform.scale;
        let rotation = Matrix3::from(instance.transform.rotation);
        let model = Matrix4::from_translation(instance.transform.position)
            * Matrix4::from(instance.transform.rotation)
            * Matrix4::from_nonuniform_scale(s.x, s.y, s.z);
        let safe_scale = Vector3::new(nonzero_scale(s.x), nonzero_scale(s.y), nonzero_scale(s.z));
        let normal = Matrix3::from_cols(
            rotation.x / safe_scale.x,
            rotation.y / safe_scale.y,
            rotation.z / safe_scale.z,
        );

        InstanceRaw {
            model: model.into(),
            color: instance.color.into(),
            normal: normal.into(),
        }
    }
}

fn nonzero_scale(scale: f32) -> f32 {
    const EPSILON: f32 = 0.000001;

    if scale.abs() >= EPSILON {
        scale
    } else if scale.is_sign_negative() {
        -EPSILON
    } else {
        EPSILON
    }
}

pub enum InstanceTemplate {
    GridX(Vector2<f32>),
    GridY(Vector2<f32>),
    GridZ(Vector2<f32>),
    Cube(Vector3<f32>),
    LineX(u32),
    LineY(u32),
    LineZ(u32),
    Circle(f32),
    Single,
}

impl InstanceTemplate {
    pub fn get_instances(
        &self,
        origin: Vector3<f32>,
        scale: Vector3<f32>,
        rotation: Quaternion<f32>,
    ) -> Vec<Instance> {
        let positions: Vec<Vector3<f32>> = match self {
            InstanceTemplate::GridX(size) => {
                let y = size.x as u32;
                let z = size.y as u32;

                (0..y * z)
                    .map(|n| {
                        let y_pos = n % y;
                        let z_pos = n / y;

                        origin + Vector3::new(0.0, y_pos as f32, z_pos as f32)
                    })
                    .collect()
            }

            InstanceTemplate::GridY(size) => {
                let x = size.x as u32;
                let z = size.y as u32;

                (0..x * z)
                    .map(|n| {
                        let x_pos = n % x;
                        let z_pos = n / x;

                        origin + Vector3::new(x_pos as f32, 0.0, z_pos as f32)
                    })
                    .collect()
            }

            InstanceTemplate::GridZ(size) => {
                let x = size.x as u32;
                let y = size.y as u32;

                (0..x * y)
                    .map(|n| {
                        let x_pos = n % x;
                        let y_pos = n / x;

                        origin + Vector3::new(x_pos as f32, y_pos as f32, 0.0)
                    })
                    .collect()
            }

            InstanceTemplate::Cube(size) => {
                let x = size.x as u32;
                let y = size.y as u32;
                let z = size.z as u32;

                (0..x * y * z)
                    .map(|n| {
                        let x_pos = n % x;
                        let z_pos = (n / x) % z;
                        let y_pos = n / (x * z);

                        origin + Vector3::new(x_pos as f32, y_pos as f32, z_pos as f32)
                    })
                    .collect()
            }

            InstanceTemplate::LineX(size) => (0..*size)
                .map(|x| origin + Vector3::new(x as f32, 0.0, 0.0))
                .collect(),

            InstanceTemplate::LineY(size) => (0..*size)
                .map(|y| origin + Vector3::new(0.0, y as f32, 0.0))
                .collect(),

            InstanceTemplate::LineZ(size) => (0..*size)
                .map(|z| origin + Vector3::new(0.0, 0.0, z as f32))
                .collect(),

            InstanceTemplate::Circle(radius) => {
                let r = radius.ceil() as i32;

                (-r..=r)
                    .flat_map(|x| {
                        (-r..=r).filter_map(move |z| {
                            let distance_squared = (x * x + z * z) as f32;

                            if distance_squared <= radius * radius {
                                Some(origin + Vector3::new(x as f32, 0.0, z as f32))
                            } else {
                                None
                            }
                        })
                    })
                    .collect()
            }
            InstanceTemplate::Single => [origin].to_vec(),
        };

        positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| {
                let local_position = position - origin;

                let rotation = if local_position.is_zero() {
                    rotation
                } else {
                    let initial = cgmath::Quaternion::from_axis_angle(
                        local_position.normalize(),
                        cgmath::Deg(0.0),
                    );
                    initial * rotation
                };

                let color = Vector3::new(1.0, 1.0, 1.0);
                let size = Vector3::new(1.0, 1.0, 1.0);

                Instance {
                    index: index as u32,
                    transform: Transform {
                        position,
                        rotation,
                        scale,
                    },
                    should_render: true,
                    color,
                    size,
                    uv: [0.0, 0.0, 0.0, 0.0].into(),
                }
            })
            .collect()
    }
}

pub struct InstanceBuilder<'a, T: RawInstance> {
    pub(crate) gfx: &'a mut Graphics,
    pub(crate) origin: Vector3<f32>,
    pub(crate) template: Option<InstanceTemplate>,
    pub(crate) phantom_data: PhantomData<T>,
    pub(crate) instances: Vec<Instance>,
    pub(crate) global_scale: Vector3<f32>,
    pub(crate) rotation: Quaternion<f32>,
}

impl<'a, T: RawInstance> InstanceBuilder<'a, T> {
    pub fn from_instances(mut self, instances: Vec<Instance>) -> Self {
        self.instances = instances.to_vec();
        self
    }
    pub fn template(mut self, template: InstanceTemplate) -> Self {
        self.template = Some(template);
        self
    }

    pub fn origin(mut self, origin: Vector3<f32>) -> Self {
        self.origin = origin;
        self
    }
    pub fn scale(mut self, scale: Vector3<f32>) -> Self {
        self.global_scale = scale;
        self
    }

    pub fn uniform_scale(mut self, scale: f32) -> Self {
        self.global_scale = Vector3::new(scale, scale, scale);
        self
    }

    pub fn rotation(mut self, rotation: Quaternion<f32>) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn build(self) -> InstanceControllerHandle {
        let instances = if let Some(template) = self.template {
            template.get_instances(self.origin, self.global_scale, self.rotation)
        } else {
            if !self.instances.is_empty() {
                self.instances
            } else {
                InstanceTemplate::Single.get_instances(
                    self.origin,
                    self.global_scale,
                    self.rotation,
                )
            }
        };
        let mut raw = Vec::with_capacity(instances.len());

        raw.extend(instances.iter().filter(|i| i.should_render).map(T::to_raw));

        let len = raw.len();

        let instance_buffer =
            self.gfx
                .engine
                .render_context
                .device
                .create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Instance Buffer"),
                    size: (instances.len() * std::mem::size_of::<T>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
        let ic = InstanceController::<T> {
            pending: Vec::with_capacity(instances.len()),
            instances,
            offset: 0,
            size: len,
            buffer_layout: T::layout(),
            instance_buffer,
            phantom: Default::default(),
        };
        self.gfx
            .engine
            .render_context
            .gpu_objects
            .instance_controllers
            .insert(Box::new(ic))
    }
}

#[cfg(test)]
mod tests {
    use cgmath::Vector3;

    use super::{DefaultInstanceLayout, Instance, InstanceRaw, RawInstance, SpriteInstanceLayout};

    #[test]
    fn default_instance_layout_matches_shader_contract() {
        let layout = DefaultInstanceLayout::layout();

        assert_eq!(std::mem::size_of::<DefaultInstanceLayout>(), 52);
        assert_eq!(layout.array_stride, 52);
        assert_eq!(
            layout
                .attributes
                .iter()
                .map(|attribute| (attribute.shader_location, attribute.offset))
                .collect::<Vec<_>>(),
            vec![(5, 0), (6, 12), (7, 24), (8, 40)]
        );
    }

    #[test]
    fn sprite_instance_layout_includes_uv_rect() {
        let layout = SpriteInstanceLayout::layout();

        assert_eq!(std::mem::size_of::<SpriteInstanceLayout>(), 68);
        assert_eq!(layout.array_stride, 68);
        assert_eq!(
            layout
                .attributes
                .iter()
                .map(|attribute| {
                    (
                        attribute.shader_location,
                        attribute.offset,
                        attribute.format,
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (5, 0, wgpu::VertexFormat::Float32x3),
                (6, 12, wgpu::VertexFormat::Float32x3),
                (7, 24, wgpu::VertexFormat::Float32x4),
                (8, 40, wgpu::VertexFormat::Float32x3),
                (9, 52, wgpu::VertexFormat::Float32x4),
            ]
        );
    }

    #[test]
    fn raw_instance_uses_nonuniform_scale_for_model_and_normals() {
        let instance = Instance::new(Vector3::new(3.0, 4.0, 5.0), Vector3::new(2.0, 4.0, 0.5));
        let raw = InstanceRaw::to_raw(&instance);

        assert_eq!(raw.model[0], [2.0, 0.0, 0.0, 0.0]);
        assert_eq!(raw.model[1], [0.0, 4.0, 0.0, 0.0]);
        assert_eq!(raw.model[2], [0.0, 0.0, 0.5, 0.0]);
        assert_eq!(raw.model[3], [3.0, 4.0, 5.0, 1.0]);
        assert_eq!(raw.normal[0], [0.5, 0.0, 0.0]);
        assert_eq!(raw.normal[1], [0.0, 0.25, 0.0]);
        assert_eq!(raw.normal[2], [0.0, 0.0, 2.0]);
    }
}
