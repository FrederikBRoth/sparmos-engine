use std::sync::{Arc, atomic::AtomicUsize};

use crate::entity::core::geometry::VertexBufferLayoutOwned;

#[derive(Clone)]
pub struct InstanceController {
    pub instances: Vec<Instance>,
    pub offset: usize, // Where in the big instance buffer this mesh's data starts
    pub count: usize,
    pub atomic_usize: Arc<AtomicUsize>,
    pub buffer_layout: VertexBufferLayoutOwned,
    pub instance_buffer: wgpu::Buffer,
}
impl InstanceController {
    pub fn new<T: InstanceToRaw + Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        instances: Vec<Instance>,
        device: &wgpu::Device,
    ) -> InstanceController {
        let len = instances
            .clone()
            .iter()
            .filter(|instance| instance.should_render)
            .map(T::to_raw)
            .collect::<Vec<_>>()
            .len();
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Instance Buffer"),
            size: (len * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        InstanceController {
            instances: instances.clone(),
            offset: 0,
            atomic_usize: Arc::new(AtomicUsize::new(len)),
            count: len,
            instance_buffer,
            buffer_layout: T::desc(),
        }
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

pub trait InstanceToRaw {
    fn desc() -> VertexBufferLayoutOwned;
    fn to_raw(instance: &Instance) -> Self;
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
