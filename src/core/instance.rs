use std::sync::{Arc, Mutex, RwLock, atomic::AtomicUsize};

use cgmath::Rotation3;

use crate::core::{
    geometry::VertexBufferLayoutOwned,
    render::{InstanceControllerHandle, RenderContext},
};

#[derive(Clone)]
pub struct InstanceController<T>
where
    T: InstanceToRaw + bytemuck::Pod + Send + Sync + 'static,
{
    pub instances: Arc<RwLock<Vec<Instance>>>,
    pub pending: Arc<Mutex<Vec<T>>>,
    pub offset: usize,
    pub atomic_usize: Arc<AtomicUsize>,
    pub buffer_layout: VertexBufferLayoutOwned,
    pub instance_buffer: wgpu::Buffer,
}
impl<T> InstanceController<T>
where
    T: InstanceToRaw + bytemuck::Pod + Send + Sync + 'static,
{
    pub fn new(instances: Vec<Instance>, rc: &mut RenderContext) -> InstanceControllerHandle {
        let mut raw = Vec::with_capacity(instances.len());

        raw.extend(instances.iter().filter(|i| i.should_render).map(T::to_raw));

        let len = raw.len();

        let instance_buffer = rc.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (instances.len() * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let ic = InstanceController {
            instances: Arc::new(RwLock::new(instances)),
            pending: Arc::new(Mutex::new(raw)),
            offset: 0,
            atomic_usize: Arc::new(AtomicUsize::new(len)),
            buffer_layout: T::desc(),
            instance_buffer,
        };
        rc.gpu_objects.instance_controllers.insert(Box::new(ic))
    }
}
pub trait InstanceControllerTrait {
    fn update(&self, queue: &wgpu::Queue);
    fn update_single(&self, queue: &wgpu::Queue);

    fn buffer(&self) -> &wgpu::Buffer;
    fn layout(&self) -> &VertexBufferLayoutOwned;

    fn count(&self) -> usize;
    fn instances(&self) -> std::sync::RwLockReadGuard<'_, Vec<Instance>>;
    fn instances_mut(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Instance>>;
}
impl<T> InstanceControllerTrait for InstanceController<T>
where
    T: InstanceToRaw + bytemuck::Pod + Send + Sync,
{
    fn update_single(&self, queue: &wgpu::Queue) {
        let pending = Arc::clone(&self.pending);
        let instances = Arc::clone(&self.instances);
        let count_clone = Arc::clone(&self.atomic_usize);

        let mut pending = pending.lock().unwrap();

        pending.clear();
        pending.extend(
            instances
                .read()
                .unwrap()
                .iter()
                .filter(|i| i.should_render)
                .map(T::to_raw),
        );

        count_clone.store(pending.len(), std::sync::atomic::Ordering::Relaxed);

        let chunk_size = 10_000;
        let stride = std::mem::size_of::<T>();

        for (i, chunk) in pending.chunks(chunk_size).enumerate() {
            queue.write_buffer(
                &self.instance_buffer,
                (i * chunk_size * stride) as u64,
                bytemuck::cast_slice(chunk),
            );
        }
    }

    fn update(&self, queue: &wgpu::Queue) {
        let pending = Arc::clone(&self.pending);
        let instances = Arc::clone(&self.instances);
        let count_clone = Arc::clone(&self.atomic_usize);

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let mut pending = pending.lock().unwrap();

            pending.clear();
            pending.extend(
                instances
                    .read()
                    .unwrap()
                    .iter()
                    .filter(|i| i.should_render)
                    .map(T::to_raw),
            );

            count_clone.store(pending.len(), std::sync::atomic::Ordering::Relaxed);
        });

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen_futures::spawn_local;

            spawn_local(async move {
                let mut pending = pending.lock().unwrap();

                pending.clear();
                pending.extend(
                    instances
                        .read()
                        .unwrap()
                        .iter()
                        .filter(|i| i.should_render)
                        .map(T::to_raw),
                );

                count_clone.store(pending.len(), std::sync::atomic::Ordering::Relaxed);
            });
        }

        let pending = self.pending.lock().unwrap();

        let chunk_size = 10_000;
        let stride = std::mem::size_of::<T>();

        for (i, chunk) in pending.chunks(chunk_size).enumerate() {
            queue.write_buffer(
                &self.instance_buffer,
                (i * chunk_size * stride) as u64,
                bytemuck::cast_slice(chunk),
            );
        }
    }
    fn buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }

    fn layout(&self) -> &VertexBufferLayoutOwned {
        &self.buffer_layout
    }

    fn count(&self) -> usize {
        self.atomic_usize.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn instances(&self) -> std::sync::RwLockReadGuard<'_, Vec<Instance>> {
        self.instances.read().unwrap()
    }

    fn instances_mut(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Instance>> {
        self.instances.write().unwrap()
    }
}

#[derive(Clone)]
pub struct Instance {
    pub index: u32,
    pub position: cgmath::Vector3<f32>,
    pub rotation: cgmath::Quaternion<f32>,
    pub should_render: bool,
    pub scale: f32,
    pub color: cgmath::Vector3<f32>,
    pub size: cgmath::Vector3<f32>,
    pub bounding: cgmath::Vector3<f32>,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            index: 0,
            position: cgmath::Vector3::new(0.0, 0.0, 0.0),
            rotation: cgmath::Quaternion::from_axis_angle(
                cgmath::Vector3::unit_z(),
                cgmath::Deg(0.0),
            ), // Identity rotation
            should_render: true,
            scale: 20.0,
            color: cgmath::Vector3::new(1.0, 1.0, 1.0), // white
            size: cgmath::Vector3::new(1.0, 1.0, 1.0),
            bounding: cgmath::Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Instance {
    pub fn new(position: cgmath::Vector3<f32>, scale: f32) -> Self {
        Self {
            position,
            scale,
            ..Default::default()
        }
    }
}
pub trait InstanceToRaw {
    fn desc() -> VertexBufferLayoutOwned;
    fn to_raw(instance: &Instance) -> Self;
}
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuInstance {
    pub position: [f32; 3],
    pub scale: f32,
    pub rotation: [f32; 4], // quaternion
    pub color: [f32; 3],
    _pad: f32, // alignment (important!)
}

impl InstanceToRaw for GpuInstance {
    fn desc() -> VertexBufferLayoutOwned {
        use std::mem;

        VertexBufferLayoutOwned {
            array_stride: mem::size_of::<GpuInstance>() as wgpu::BufferAddress,
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
        GpuInstance {
            position: instance.position.into(),
            scale: instance.scale,
            rotation: instance.rotation.into(), // must be quaternion
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

impl InstanceToRaw for InstanceRaw {
    fn desc() -> VertexBufferLayoutOwned {
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
        let s = instance.scale;
        let rotation: [[f32; 3]; 3] = cgmath::Matrix3::from(instance.rotation).into();

        // Compute R * S (scale each column of rotation)
        let mut model = [[0.0; 4]; 4];
        for i in 0..3 {
            model[0][i] = rotation[0][i] * s;
            model[1][i] = rotation[1][i] * s;
            model[2][i] = rotation[2][i] * s;
        }

        // Now apply translation (T * R * S)
        model[3][0] = instance.position.x;
        model[3][1] = instance.position.y;
        model[3][2] = instance.position.z;
        model[3][3] = 1.0;
        InstanceRaw {
            model,
            color: instance.color.into(),
            normal: cgmath::Matrix3::from(instance.rotation).into(),
        }
    }
}
