use crate::{
    application::{graphics::Graphics, gui_elements::interactive::EguiInspectable},
    core::resource::BufferHandle,
};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PhysicsBasedRenderingConstants {
    pub metallic: f32,
    pub roughness: f32,
    pub ao: f32,
}

impl EguiInspectable for PhysicsBasedRenderingConstants {
    fn ui(&mut self, ui: &mut egui::Ui, gfx: &mut Graphics, handle: BufferHandle) {
        if ui
            .add(egui::Slider::new(&mut self.metallic, 0.0..=1.0).text("Metallic"))
            .changed()
        {
            gfx.update_buffer(handle, &[*self]);
        };
        if ui
            .add(egui::Slider::new(&mut self.roughness, 0.0..=1.0).text("Roughness"))
            .changed()
        {
            gfx.update_buffer(handle, &[*self]);
        };
        if ui
            .add(egui::Slider::new(&mut self.ao, 0.0..=1.0).text("AO"))
            .changed()
        {
            gfx.update_buffer(handle, &[*self]);
        };
    }
}
