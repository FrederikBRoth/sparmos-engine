use anyhow::*;
use half::f16;
use image::GenericImageView;
use wgpu::{BindGroup, BindGroupLayout, Sampler, TextureFormat};
use winit::dpi::PhysicalSize;

use crate::application::graphics::Graphics;

pub const DEFAULT_IRRADIANCE_MAP_FACE_SIZE: u32 = 32;

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
pub enum TextureViewType {
    D2,
    Cube,
}

#[derive(Clone)]
pub struct TextureDefinition {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub view_type: TextureViewType,
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
        Ok(Self {
            texture,
            view,
            view_type: TextureViewType::D2,
        })
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
        Ok(Self {
            texture,
            view,
            view_type: TextureViewType::D2,
        })
    }

    pub fn from_cubemap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: [image::DynamicImage; 6],
        label: Option<&str>,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let rgba_images: Vec<_> = images.iter().map(|img| img.to_rgba8()).collect();

        let dimensions = images[0].dimensions();

        // All six faces must have the same dimensions.
        for img in &images {
            if img.dimensions() != dimensions {
                bail!("All cubemap faces must have the same dimensions");
            }
        }

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 6,
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

        // Upload each face into one array layer.
        for (face, rgba) in rgba_images.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(dimensions.0 * 4),
                    rows_per_image: Some(dimensions.1),
                },
                wgpu::Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            view_type: TextureViewType::Cube,
        })
    }

    fn from_float_cubemap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        face_size: u32,
        label: Option<&str>,
        mut create_face: impl FnMut(u32) -> Vec<[f32; 4]>,
    ) -> Result<Self> {
        if face_size == 0 {
            bail!("Cubemap face size must be non-zero");
        }

        let size = wgpu::Extent3d {
            width: face_size,
            height: face_size,
            depth_or_array_layers: 6,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for face_index in 0..6 {
            let face = create_face(face_index);
            if face.len() != (face_size as usize) * (face_size as usize) {
                bail!("Cubemap face has the wrong number of pixels");
            }
            let pixels = face
                .iter()
                .flat_map(|pixel| pixel.map(|channel| f16::from_f32(channel).to_bits()))
                .collect::<Vec<_>>();

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face_index,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&pixels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(face_size * 8),
                    rows_per_image: Some(face_size),
                },
                wgpu::Extent3d {
                    width: face_size,
                    height: face_size,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            view_type: TextureViewType::Cube,
        })
    }

    fn from_hdr_cubemap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::Rgb32FImage,
        label: Option<&str>,
    ) -> Result<Self> {
        let face_size = hdri_face_size(image)?;
        Self::from_float_cubemap(device, queue, face_size, label, |face| {
            equirectangular_face(image, face, face_size)
        })
    }

    fn from_hdri_irradiance_map(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::Rgb32FImage,
        face_size: u32,
        label: Option<&str>,
    ) -> Result<Self> {
        hdri_face_size(image)?;
        let coefficients = project_environment_sh(image);
        Self::from_float_cubemap(device, queue, face_size, label, |face| {
            irradiance_face(&coefficients, face, face_size)
        })
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
                view_dimension: match texture.view_type {
                    TextureViewType::D2 => wgpu::TextureViewDimension::D2,
                    TextureViewType::Cube => wgpu::TextureViewDimension::Cube,
                },
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
    pub(crate) label: &'a str,
}

impl<'a> TextureBuilder<'a> {
    pub(crate) fn new(gfx: &'a mut Graphics, label: &'a str) -> Self {
        Self {
            gfx,
            textures: vec![],
            label: label,
        }
    }

    pub fn image(mut self, img: &image::DynamicImage, format: wgpu::TextureFormat) -> Self {
        let texture = TextureDefinition::from_image(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            img,
            Some(self.label),
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
            Some(self.label),
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
            Some(self.label),
        )
        .unwrap();
        self.textures.push(texture);
        self
    }

    pub fn build(mut self) -> Texture {
        let textures = std::mem::take(&mut self.textures);
        self.finish(textures)
    }

    fn finish(self, textures: Vec<TextureDefinition>) -> Texture {
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
            create_bind_group_and_layout(self.gfx.get_device(), &textures, &sampler);
        Texture {
            label: self.label.to_string(),
            texture: textures,
            sampler,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn cubemap(self, image: &[u8]) -> Texture {
        let img = image::load_from_memory(image).unwrap();
        let face_size = img.width() / 4;

        let faces = [
            // +X
            img.crop_imm(face_size * 2, face_size, face_size, face_size),
            // -X
            img.crop_imm(0, face_size, face_size, face_size),
            // +Y
            img.crop_imm(face_size, 0, face_size, face_size),
            // -Y
            img.crop_imm(face_size, face_size * 2, face_size, face_size),
            // +Z
            img.crop_imm(face_size, face_size, face_size, face_size),
            // -Z
            img.crop_imm(face_size * 3, face_size, face_size, face_size),
        ];

        let cubemap = TextureDefinition::from_cubemap(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            faces,
            Some(self.label),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
        .unwrap();
        self.finish(vec![cubemap])
    }

    /// Decodes a 2:1 equirectangular Radiance HDR image and converts it to a
    /// floating-point cubemap. The face size is one quarter of the HDRI width.
    pub fn hdri_cubemap(self, image: &[u8]) -> Texture {
        let hdri = decode_hdri(image).expect("Failed to decode Radiance HDR image");

        let cubemap = TextureDefinition::from_hdr_cubemap(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            &hdri,
            Some(self.label),
        )
        .unwrap();
        self.finish(vec![cubemap])
    }

    /// Builds a low-resolution diffuse irradiance cubemap from a 2:1 Radiance
    /// HDR image. Values are divided by PI for direct multiplication by albedo.
    pub fn hdri_irradiance_map(self, image: &[u8]) -> Texture {
        let hdri = decode_hdri(image).expect("Failed to decode Radiance HDR image");
        let irradiance = TextureDefinition::from_hdri_irradiance_map(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            &hdri,
            DEFAULT_IRRADIANCE_MAP_FACE_SIZE,
            Some(self.label),
        )
        .unwrap();

        self.finish(vec![irradiance])
    }
}

fn decode_hdri(bytes: &[u8]) -> Result<image::Rgb32FImage> {
    Ok(
        image::load_from_memory_with_format(bytes, image::ImageFormat::Hdr)
            .context("Invalid Radiance HDR data")?
            .to_rgb32f(),
    )
}

fn hdri_face_size(image: &image::Rgb32FImage) -> Result<u32> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        bail!("HDRI dimensions must be non-zero");
    }
    if height.checked_mul(2) != Some(width) || width % 4 != 0 {
        bail!("HDRI must use a 2:1 equirectangular projection");
    }

    Ok(width / 4)
}

fn equirectangular_face(image: &image::Rgb32FImage, face: u32, face_size: u32) -> Vec<[f32; 4]> {
    cubemap_face(face, face_size, |direction| {
        sample_equirectangular(image, direction)
    })
}

fn irradiance_face(coefficients: &ShCoefficients, face: u32, face_size: u32) -> Vec<[f32; 4]> {
    cubemap_face(face, face_size, |direction| {
        evaluate_irradiance(coefficients, direction)
    })
}

fn cubemap_face(
    face: u32,
    face_size: u32,
    mut sample: impl FnMut([f32; 3]) -> [f32; 3],
) -> Vec<[f32; 4]> {
    let mut pixels = Vec::with_capacity((face_size * face_size) as usize);
    for y in 0..face_size {
        for x in 0..face_size {
            let s = 2.0 * (x as f32 + 0.5) / face_size as f32 - 1.0;
            let t = 2.0 * (y as f32 + 0.5) / face_size as f32 - 1.0;
            let direction = cubemap_direction(face, s, t);
            let rgb = sample(direction);
            pixels.push([rgb[0], rgb[1], rgb[2], 1.0]);
        }
    }
    pixels
}

type ShCoefficients = [[f64; 3]; 9];

fn project_environment_sh(image: &image::Rgb32FImage) -> ShCoefficients {
    let width = image.width();
    let height = image.height();
    let longitude_steps = (0..width)
        .map(|x| {
            let longitude = ((x as f64 + 0.5) / width as f64 - 0.5) * std::f64::consts::TAU;
            (longitude.cos(), longitude.sin())
        })
        .collect::<Vec<_>>();
    let texel_angle = std::f64::consts::TAU / width as f64 * std::f64::consts::PI / height as f64;
    let mut coefficients = [[0.0; 3]; 9];
    let mut total_weight = 0.0;

    for y in 0..height {
        let polar_angle = (y as f64 + 0.5) / height as f64 * std::f64::consts::PI;
        let sin_polar = polar_angle.sin();
        let cos_polar = polar_angle.cos();
        let weight = sin_polar * texel_angle;
        total_weight += weight * width as f64;

        for x in 0..width {
            let (cos_longitude, sin_longitude) = longitude_steps[x as usize];
            let direction = [
                sin_polar * cos_longitude,
                cos_polar,
                sin_polar * sin_longitude,
            ];
            let basis = sh_basis(direction);
            let radiance = image.get_pixel(x, y).0;

            for coefficient in 0..9 {
                for channel in 0..3 {
                    coefficients[coefficient][channel] +=
                        radiance[channel] as f64 * basis[coefficient] * weight;
                }
            }
        }
    }

    let normalization = 4.0 * std::f64::consts::PI / total_weight;
    for coefficient in &mut coefficients {
        for channel in coefficient {
            *channel *= normalization;
        }
    }
    coefficients
}

fn sh_basis(direction: [f64; 3]) -> [f64; 9] {
    let [x, y, z] = direction;
    [
        0.282_094_791_773_878_14,
        0.488_602_511_902_919_9 * y,
        0.488_602_511_902_919_9 * z,
        0.488_602_511_902_919_9 * x,
        1.092_548_430_592_079_2 * x * y,
        1.092_548_430_592_079_2 * y * z,
        0.315_391_565_252_520_05 * (3.0 * z * z - 1.0),
        1.092_548_430_592_079_2 * x * z,
        0.546_274_215_296_039_6 * (x * x - y * y),
    ]
}

fn evaluate_irradiance(coefficients: &ShCoefficients, direction: [f32; 3]) -> [f32; 3] {
    let basis = sh_basis(direction.map(f64::from));
    let band_factors = [
        1.0,
        2.0 / 3.0,
        2.0 / 3.0,
        2.0 / 3.0,
        0.25,
        0.25,
        0.25,
        0.25,
        0.25,
    ];
    std::array::from_fn(|channel| {
        let value = (0..9)
            .map(|coefficient| {
                coefficients[coefficient][channel] * basis[coefficient] * band_factors[coefficient]
            })
            .sum::<f64>();
        value.max(0.0) as f32
    })
}

fn cubemap_direction(face: u32, s: f32, t: f32) -> [f32; 3] {
    let direction = match face {
        0 => [1.0, -t, -s],
        1 => [-1.0, -t, s],
        2 => [s, 1.0, t],
        3 => [s, -1.0, -t],
        4 => [s, -t, 1.0],
        5 => [-s, -t, -1.0],
        _ => unreachable!("A cubemap has exactly six faces"),
    };
    let length =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    [
        direction[0] / length,
        direction[1] / length,
        direction[2] / length,
    ]
}

fn sample_equirectangular(image: &image::Rgb32FImage, direction: [f32; 3]) -> [f32; 3] {
    let width = image.width() as i32;
    let height = image.height() as i32;
    let longitude = direction[2].atan2(direction[0]);
    let latitude = direction[1].clamp(-1.0, 1.0).asin();
    let u = 0.5 + longitude / std::f32::consts::TAU;
    let v = 0.5 - latitude / std::f32::consts::PI;

    let source_x = u * width as f32 - 0.5;
    let source_y = v * height as f32 - 0.5;
    let x0 = source_x.floor() as i32;
    let x1 = x0 + 1;
    let source_y0 = source_y.floor() as i32;
    let y0 = source_y0.clamp(0, height - 1);
    let y1 = (source_y0 + 1).clamp(0, height - 1);
    let tx = source_x - source_x.floor();
    let ty = (source_y - source_y.floor()).clamp(0.0, 1.0);

    let top_left = image.get_pixel(x0.rem_euclid(width) as u32, y0 as u32).0;
    let top_right = image.get_pixel(x1.rem_euclid(width) as u32, y0 as u32).0;
    let bottom_left = image.get_pixel(x0.rem_euclid(width) as u32, y1 as u32).0;
    let bottom_right = image.get_pixel(x1.rem_euclid(width) as u32, y1 as u32).0;

    std::array::from_fn(|channel| {
        let top = top_left[channel] * (1.0 - tx) + top_right[channel] * tx;
        let bottom = bottom_left[channel] * (1.0 - tx) + bottom_right[channel] * tx;
        top * (1.0 - ty) + bottom * ty
    })
}
