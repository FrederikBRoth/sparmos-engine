use indexmap::IndexMap;

use crate::core::{
    engine::Engine, entities::World, geometry::Vertex, instance::RawInstance,
    material::MaterialBuilder,
};

pub struct Graphics {
    pub world: World,
    pub engine: Engine,
}

impl Graphics {
    pub fn shader(&mut self, label: &str, shader_path: &str) {
        self.engine.render_context.add_shader(label, shader_path);
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
}
