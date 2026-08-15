use std::{mem, sync::Arc};

use cgmath::Vector3;
use indexmap::IndexMap;
use wgpu::{Device, Queue};

use crate::{
    core::{
        engine::Engine,
        entities::World,
        geometry::Vertex,
        instance::{InstanceBuilder, RawInstance},
        material::MaterialBuilder,
        render::RenderContext,
    },
    systems::compute::ComputeBuilder,
};

pub struct Graphics {
    pub world: World,
    pub engine: Engine,
}

impl Graphics {
    pub fn shader(&mut self, label: &str, shader_path: &str) {
        self.engine.render_context.add_shader(label, shader_path);
    }

    pub(crate) fn get_device(&self) -> &Arc<Device> {
        &self.engine.render_context.device
    }
    pub(crate) fn get_queue(&self) -> &Arc<Queue> {
        &self.engine.render_context.queue
    }

    pub(crate) fn get_device_mut(&mut self) -> &mut Arc<Device> {
        &mut self.engine.render_context.device
    }
    pub(crate) fn get_queue_mut(&mut self) -> &mut Arc<Queue> {
        &mut self.engine.render_context.queue
    }

    pub(crate) fn get_render_context_mut(&mut self) -> &mut RenderContext {
        &mut self.engine.render_context
    }
    pub(crate) fn get_render_context(&self) -> &RenderContext {
        &self.engine.render_context
    }

    pub fn material<V: Vertex, I: RawInstance>(&mut self) -> MaterialBuilder<'_> {
        MaterialBuilder {
            graphics: self,
            buffers: IndexMap::new(),
            texture: None,
            shader: String::new(),
            vertex_layout: V::layout(),
            instance_layout: I::layout(),
        }
    }

    pub fn instances<I: RawInstance>(&mut self) -> InstanceBuilder<'_, I> {
        InstanceBuilder::<I> {
            gfx: self,
            origin: Vector3::new(0.0, 0.0, 0.0),
            template: None,
            phantom_data: Default::default(),
            instances: vec![],
        }
    }

    pub fn compute<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable>(
        &mut self,
    ) -> ComputeBuilder<'_> {
        let output_size = mem::size_of::<T>();
        ComputeBuilder {
            gfx: self,
            output_object_size: output_size,
            size: 0,
            input_buffers: vec![],
            shader: String::new(),
        }
    }
}
