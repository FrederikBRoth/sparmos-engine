use crate::{application::graphics::Graphics, core::resource::BufferHandle};

pub trait EguiInspectable {
    fn ui(&mut self, ui: &mut egui::Ui, gfx: &mut Graphics, handle: BufferHandle);
}

#[derive(Default)]
pub struct BufferController<T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable + EguiInspectable>
{
    buffer_state: T,
}

impl<T> BufferController<T>
where
    T: Copy + Clone + bytemuck::Pod + bytemuck::Zeroable + EguiInspectable,
{
    pub fn ui(&mut self, ui: &mut egui::Ui, gfx: &mut Graphics, name: &str) {
        let handle = gfx.get_buffer_by_register(name);
        self.buffer_state.ui(ui, gfx, handle);
    }
}
