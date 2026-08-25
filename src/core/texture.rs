use anyhow::*;
use image::GenericImageView;
use wgpu::{BindGroup, BindGroupLayout, Sampler, TextureFormat, TextureView};
use winit::dpi::PhysicalSize;

use crate::application::graphics::Graphics;

#[derive(Clone)]
pub struct Texture {
    #[allow(unused)]
    pub label: String,
    pub texture: Vec<TextureDefinition>,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
}

#[derive(Clone)]
pub struct TextureDefinition {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl TextureDefinition {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: Option<&str>,
        format: TextureFormat,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, label, format)
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
        format: TextureFormat,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        println!("{:?}", dimensions);
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            texture.as_image_copy(),
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(dimensions.0 * 4),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );
        // let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        //     address_mode_u: wgpu::AddressMode::ClampToEdge,
        //     address_mode_v: wgpu::AddressMode::ClampToEdge,
        //     address_mode_w: wgpu::AddressMode::ClampToEdge,
        //     mag_filter: wgpu::FilterMode::Linear,
        //     min_filter: wgpu::FilterMode::Linear,
        //     mipmap_filter: wgpu::MipmapFilterMode::Linear,
        //     anisotropy_clamp: 8,
        //
        //     ..Default::default()
        // });
        // let (bind_group_layout, bind_group) = create_bind_group_and_layout(device, &view, &sampler);
        Ok(Self { texture, view })
    }

    pub fn from_color(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: &[f32; 3],
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = [
            (color[0].clamp(0.0, 1.0) * 255.0) as u8,
            (color[1].clamp(0.0, 1.0) * 255.0) as u8,
            (color[2].clamp(0.0, 1.0) * 255.0) as u8,
            255,
        ];

        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            texture.as_image_copy(),
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );
        // let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        //     address_mode_u: wgpu::AddressMode::ClampToEdge,
        //     address_mode_v: wgpu::AddressMode::ClampToEdge,
        //     address_mode_w: wgpu::AddressMode::ClampToEdge,
        //     mag_filter: wgpu::FilterMode::Linear,
        //     min_filter: wgpu::FilterMode::Linear,
        //     mipmap_filter: wgpu::MipmapFilterMode::Linear,
        //     anisotropy_clamp: 8,
        //
        //     ..Default::default()
        // });
        // let (bind_group_layout, bind_group) = create_bind_group_and_layout(device, &view, &sampler);
        Ok(Self { texture, view })
    }

    // pub fn create_life_texture(
    //     device: &wgpu::Device,
    //     queue: &wgpu::Queue,
    //     width: u32,
    //     height: u32,
    //     label: &str,
    // ) -> Result<Self> {
    //     let size = wgpu::Extent3d {
    //         width,
    //         height,
    //         depth_or_array_layers: 1,
    //     };
    //
    //     let texture = device.create_texture(&wgpu::TextureDescriptor {
    //         label: Some(label),
    //         size,
    //         mip_level_count: 1,
    //         sample_count: 1,
    //         dimension: wgpu::TextureDimension::D2,
    //         format: wgpu::TextureFormat::R8Unorm,
    //         usage: wgpu::TextureUsages::TEXTURE_BINDING
    //             | wgpu::TextureUsages::COPY_DST
    //             | wgpu::TextureUsages::RENDER_ATTACHMENT,
    //         view_formats: &[],
    //     });
    //
    //     // Initialize with dead cells
    //     let cells = vec![0u8; (width * height) as usize];
    //
    //     queue.write_texture(
    //         texture.as_image_copy(),
    //         &cells,
    //         wgpu::TexelCopyBufferLayout {
    //             offset: 0,
    //             bytes_per_row: Some(width),
    //             rows_per_image: Some(height),
    //         },
    //         size,
    //     );
    //
    //     let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    //     // let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    //     //     address_mode_u: wgpu::AddressMode::ClampToEdge,
    //     //     address_mode_v: wgpu::AddressMode::ClampToEdge,
    //     //     address_mode_w: wgpu::AddressMode::ClampToEdge,
    //     //     mag_filter: wgpu::FilterMode::Linear,
    //     //     min_filter: wgpu::FilterMode::Linear,
    //     //     mipmap_filter: wgpu::MipmapFilterMode::Linear,
    //     //     anisotropy_clamp: 8,
    //     //
    //     //     ..Default::default()
    //     // });
    //     // let (bind_group_layout, bind_group) = create_bind_group_and_layout(device, &view, &sampler);
    //     Ok(Self { texture, view })
    // }
}

pub enum TextureType {
    Image,
    SolidColor,
}

pub(crate) struct TextureDepth {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    #[allow(unused)]
    pub fn create_depth_texture(
        device: &wgpu::Device,
        size: &PhysicalSize<u32>,
        label: &str,
    ) -> TextureDepth {
        let size = wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::DEPTH_FORMAT],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            anisotropy_clamp: 8,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        TextureDepth {
            texture,
            view,
            sampler,
        }
    }
}

// pub fn create_life_bind_group_and_layout(
//     device: &wgpu::Device,
//     view: &TextureView,
//     sampler: &Sampler,
// ) -> (BindGroupLayout, BindGroup) {
//     let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//         entries: &[
//             wgpu::BindGroupLayoutEntry {
//                 binding: 0,
//                 visibility: wgpu::ShaderStages::FRAGMENT,
//                 ty: wgpu::BindingType::Texture {
//                     multisampled: false,
//                     view_dimension: wgpu::TextureViewDimension::D2,
//                     sample_type: wgpu::TextureSampleType::Float {
//                         filterable: false, // IMPORTANT
//                     },
//                 },
//                 count: None,
//             },
//             wgpu::BindGroupLayoutEntry {
//                 binding: 1,
//                 visibility: wgpu::ShaderStages::FRAGMENT,
//                 ty: wgpu::BindingType::Sampler(
//                     wgpu::SamplerBindingType::NonFiltering, // IMPORTANT
//                 ),
//                 count: None,
//             },
//         ],
//         label: Some("life_texture_bind_group_layout"),
//     });
//
//     let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
//         layout: &layout,
//         entries: &[
//             wgpu::BindGroupEntry {
//                 binding: 0,
//                 resource: wgpu::BindingResource::TextureView(view),
//             },
//             wgpu::BindGroupEntry {
//                 binding: 1,
//                 resource: wgpu::BindingResource::Sampler(sampler),
//             },
//         ],
//         label: Some("life_texture_bind_group"),
//     });
//
//     (layout, bind_group)
// }

fn create_bind_group_and_layout(
    device: &wgpu::Device,
    textures: &[TextureDefinition],
    sampler: &Sampler,
) -> (BindGroupLayout, BindGroup) {
    let mut entries = textures
        .iter()
        .enumerate()
        .map(|(i, texture)| wgpu::BindGroupLayoutEntry {
            binding: i as u32,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: wgpu::TextureViewDimension::D2,
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        })
        .collect::<Vec<wgpu::BindGroupLayoutEntry>>();

    entries.push(wgpu::BindGroupLayoutEntry {
        binding: entries.len() as u32,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    });
    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &entries,
            label: Some("texture_bind_group_layout"),
        });

    let mut entries = textures
        .iter()
        .enumerate()
        .map(|(i, texture)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: wgpu::BindingResource::TextureView(&texture.view),
        })
        .collect::<Vec<wgpu::BindGroupEntry>>();

    entries.push(wgpu::BindGroupEntry {
        binding: entries.len() as u32,
        resource: wgpu::BindingResource::Sampler(sampler),
    });

    // Create bind group for the texture
    let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &texture_bind_group_layout,
        entries: &entries,
        label: Some("diffuse_bind_group"),
    });

    (texture_bind_group_layout, diffuse_bind_group)
}

pub struct TextureBuilder<'a> {
    pub(crate) gfx: &'a mut Graphics,
    pub(crate) textures: Vec<TextureDefinition>,
}

impl<'a> TextureBuilder<'a> {
    pub(crate) fn new(gfx: &'a mut Graphics) -> Self {
        Self {
            gfx,
            textures: vec![],
        }
    }

    pub fn image(mut self, img: &image::DynamicImage, format: wgpu::TextureFormat) -> Self {
        let texture = TextureDefinition::from_image(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            img,
            None,
            format,
        )
        .unwrap();
        self.textures.push(texture);
        self
    }
    pub fn bytes(mut self, data: &[u8], format: wgpu::TextureFormat) -> Self {
        let texture = TextureDefinition::from_bytes(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            data,
            None,
            format,
        )
        .unwrap();
        self.textures.push(texture);
        self
    }

    pub fn color(mut self, color: [f32; 3]) -> Self {
        let texture = TextureDefinition::from_color(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            &color,
            None,
        )
        .unwrap();
        self.textures.push(texture);
        self
    }

    pub fn build(self) -> Texture {
        let sampler = self
            .gfx
            .get_device()
            .create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: 8,

                ..Default::default()
            });
        let (bind_group_layout, bind_group) =
            create_bind_group_and_layout(self.gfx.get_device(), &self.textures, &sampler);
        Texture {
            label: "sut".to_string(),
            texture: self.textures,
            sampler,
            bind_group_layout,
            bind_group,
        }
    }
}
