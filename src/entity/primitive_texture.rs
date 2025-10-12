use winit::dpi::PhysicalSize;

#[derive(Clone)]
pub struct PrimitiveTexture {
    #[allow(unused)]
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl PrimitiveTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    #[allow(unused)]
    pub fn create_depth_texture(
        device: &wgpu::Device,
        size: &PhysicalSize<u32>,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float, // or Depth32Float if stencil not needed
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Depth32Float],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}
