use std::marker::PhantomData;

use cgmath::{InnerSpace, Quaternion, Rotation3, Vector2, Vector3, Zero};

use crate::{
    application::graphics::Graphics,
    core::{geometry::VertexBufferLayoutOwned, render::InstanceControllerHandle},
};

#[derive(Clone)]
pub struct Transform {
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    //simple scale at this point
    pub scale: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: cgmath::Vector3::new(0.0, 0.0, 0.0),
            rotation: cgmath::Quaternion::from_axis_angle(
                cgmath::Vector3::unit_z(),
                cgmath::Deg(0.0),
            ), // Identity rotation,
            scale: 20.0,
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
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            index: 0,
            transform: Default::default(),
            should_render: true,
            color: cgmath::Vector3::new(1.0, 1.0, 1.0), // white
            size: cgmath::Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Instance {
    pub fn new(position: cgmath::Vector3<f32>, scale: f32) -> Self {
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
pub trait RawInstance: bytemuck::Pod + bytemuck::Zeroable {
    fn layout() -> VertexBufferLayoutOwned;
    fn to_raw(instance: &Instance) -> Self;
}

//Default instance layout in Sparmos Engine
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DefaultInstanceLayout {
    pub position: [f32; 3],
    pub scale: f32,
    pub rotation: [f32; 4], // quaternion
    pub color: [f32; 3],
    _pad: f32, // alignment (important!)
}

impl RawInstance for DefaultInstanceLayout {
    fn layout() -> VertexBufferLayoutOwned {
        use std::mem;

        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<DefaultInstanceLayout>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: vec![
                // position + scale
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // rotation quaternion
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as _,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as _,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }

    fn to_raw(instance: &Instance) -> Self {
        DefaultInstanceLayout {
            position: instance.transform.position.into(),
            scale: instance.transform.scale,
            rotation: instance.transform.rotation.into(), // must be quaternion
            color: instance.color.into(),
            _pad: 0.0,
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
        let rotation: [[f32; 3]; 3] = cgmath::Matrix3::from(instance.transform.rotation).into();

        // Compute R * S (scale each column of rotation)
        let mut model = [[0.0; 4]; 4];
        for i in 0..3 {
            model[0][i] = rotation[0][i] * s;
            model[1][i] = rotation[1][i] * s;
            model[2][i] = rotation[2][i] * s;
        }

        // Now apply translation (T * R * S)
        model[3][0] = instance.transform.position.x;
        model[3][1] = instance.transform.position.y;
        model[3][2] = instance.transform.position.z;
        model[3][3] = 1.0;
        InstanceRaw {
            model,
            color: instance.color.into(),
            normal: cgmath::Matrix3::from(instance.transform.rotation).into(),
        }
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
    pub fn get_instances(&self, origin: Vector3<f32>, scale: f32) -> Vec<Instance> {
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
                    cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
                } else {
                    cgmath::Quaternion::from_axis_angle(
                        local_position.normalize(),
                        cgmath::Deg(0.0),
                    )
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
    pub(crate) global_size: f32,
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
    pub fn scale(mut self, scale: f32) -> Self {
        self.global_size = scale;
        self
    }

    pub fn build(self) -> InstanceControllerHandle {
        let instances = if let Some(template) = self.template {
            template.get_instances(self.origin, self.global_size)
        } else {
            if !self.instances.is_empty() {
                self.instances
            } else {
                InstanceTemplate::Single.get_instances(self.origin, self.global_size)
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
