use anyhow::*;
use cgmath::{Deg, Matrix4, Point3, Vector3};
use half::f16;
use image::GenericImageView;
use wgpu::util::DeviceExt;
use wgpu::{BindGroup, BindGroupLayout, Sampler, TextureFormat};
use winit::dpi::PhysicalSize;

use crate::{
    application::graphics::Graphics,
    core::{
        buffer::{Buffer, BufferType, UniformParameters},
        geometry::VertexBufferLayoutOwned,
        pipelines::{PipelineConfig, RenderPipelineBuilder},
        render::RenderContext,
    },
};

pub const DEFAULT_IRRADIANCE_MAP_FACE_SIZE: u32 = 32;
pub const DEFAULT_PREFILTERED_ENVIRONMENT_FACE_SIZE: u32 = 128;
pub const DEFAULT_PREFILTERED_ENVIRONMENT_MIP_LEVELS: u32 = 5;
pub const DEFAULT_BRDF_LUT_SIZE: u32 = 512;
const HDR_HALF_FLOAT_STORAGE_TARGET: f32 = 16_376.0;
const HALF_FLOAT_MAX: f32 = 65_504.0;

#[derive(Clone)]
pub struct Texture {
    #[allow(unused)]
    pub label: String,
    pub texture: Vec<TextureDefinition>,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
    _radiance_scale_buffer: wgpu::Buffer,
}

pub struct TextureBinding {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub sampler: wgpu::Sampler,
}

#[derive(Clone, Copy)]
pub struct TextureBindingConfig {
    pub view_dimension: wgpu::TextureViewDimension,
    pub sample_type: wgpu::TextureSampleType,
    pub sampler_binding_type: wgpu::SamplerBindingType,
    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub address_mode_w: wgpu::AddressMode,
    pub mag_filter: wgpu::FilterMode,
    pub min_filter: wgpu::FilterMode,
    pub mipmap_filter: wgpu::MipmapFilterMode,
}

impl Default for TextureBindingConfig {
    fn default() -> Self {
        Self {
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            sampler_binding_type: wgpu::SamplerBindingType::Filtering,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
        }
    }
}

impl TextureBinding {
    pub fn new(
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        config: TextureBindingConfig,
        label: Option<&str>,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: config.view_dimension,
                        sample_type: config.sample_type,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(config.sampler_binding_type),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u: config.address_mode_u,
            address_mode_v: config.address_mode_v,
            address_mode_w: config.address_mode_w,
            mag_filter: config.mag_filter,
            min_filter: config.min_filter,
            mipmap_filter: config.mipmap_filter,
            ..Default::default()
        });
        let bind_group = Self::create_bind_group(device, &bind_group_layout, view, &sampler, label);

        Self {
            bind_group_layout,
            bind_group,
            sampler,
        }
    }

    pub fn bind_group_for_view(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        label: Option<&str>,
    ) -> wgpu::BindGroup {
        Self::create_bind_group(device, &self.bind_group_layout, view, &self.sampler, label)
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        label: Option<&str>,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
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
    pub radiance_scale: f32,
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
            radiance_scale: 1.0,
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
            radiance_scale: 1.0,
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
            radiance_scale: 1.0,
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
    pub(crate) fn create_depth_texture(
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

pub struct RenderableCubemap {
    pub texture: wgpu::Texture,
    pub cube_view: wgpu::TextureView,
    pub face_views: [wgpu::TextureView; 6],
}

pub fn create_renderable_cubemap(
    device: &wgpu::Device,
    face_size: u32,
    label: Option<&str>,
) -> Result<RenderableCubemap> {
    create_renderable_cubemap_with_mips(device, face_size, 1, label)
}

fn create_renderable_cubemap_with_mips(
    device: &wgpu::Device,
    face_size: u32,
    mip_level_count: u32,
    label: Option<&str>,
) -> Result<RenderableCubemap> {
    if face_size == 0 {
        bail!("Cubemap face size must be non-zero");
    }
    let maximum_mip_level_count = face_size.ilog2() + 1;
    if mip_level_count == 0 || mip_level_count > maximum_mip_level_count {
        bail!(
            "Cubemap mip level count must be between 1 and {maximum_mip_level_count} for a {face_size}x{face_size} face"
        );
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size: wgpu::Extent3d {
            width: face_size,
            height: face_size,
            depth_or_array_layers: 6,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let cube_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label,
        dimension: Some(wgpu::TextureViewDimension::Cube),
        base_array_layer: 0,
        array_layer_count: Some(6),
        base_mip_level: 0,
        mip_level_count: Some(mip_level_count),
        ..Default::default()
    });

    let face_views = std::array::from_fn(|face| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: face as u32,
            array_layer_count: Some(1),
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        })
    });

    Ok(RenderableCubemap {
        texture,
        cube_view,
        face_views,
    })
}

fn cubemap_face_views(
    texture: &wgpu::Texture,
    mip_level: u32,
    label: Option<&str>,
) -> [wgpu::TextureView; 6] {
    std::array::from_fn(|face| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: face as u32,
            array_layer_count: Some(1),
            base_mip_level: mip_level,
            mip_level_count: Some(1),
            ..Default::default()
        })
    })
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CubemapCaptureUniform {
    view_proj: [[f32; 4]; 4],
    roughness: f32,
    source_resolution: f32,
    _padding: [f32; 2],
}

// Capture-only clip conversion. Keeping this beside the cubemap passes means
// camera projection and screen-to-world raycasting keep their existing matrix.
#[rustfmt::skip]
const CUBEMAP_OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

const CAPTURE_CUBE_VERTICES: [[f32; 3]; 36] = [
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
    [1.0, -1.0, -1.0],
    [-1.0, -1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [1.0, 1.0, -1.0],
    [1.0, -1.0, 1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, -1.0],
    [1.0, 1.0, 1.0],
    [-1.0, -1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [-1.0, 1.0, 1.0],
    [-1.0, -1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, -1.0, 1.0],
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, 1.0],
    [-1.0, -1.0, 1.0],
];

fn upload_hdri(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: &image::Rgb32FImage,
    label: Option<&str>,
) -> Result<TextureDefinition> {
    if image.width() == 0 || image.height() == 0 {
        bail!("HDRI dimensions must be non-zero");
    }

    let radiance_scale = hdri_radiance_scale(image);

    // LearnOpenGL flips HDR input vertically before using the standard spherical
    // mapping. Very bright HDR pixels are scaled into a finite half-float range;
    // the scale is restored in the skybox and PBR shaders.
    let pixels = (0..image.height())
        .rev()
        .flat_map(|y| (0..image.width()).map(move |x| image.get_pixel(x, y)))
        .flat_map(|pixel| {
            [
                hdr_channel_to_f16_bits(pixel[0], radiance_scale),
                hdr_channel_to_f16_bits(pixel[1], radiance_scale),
                hdr_channel_to_f16_bits(pixel[2], radiance_scale),
                f16::ONE.to_bits(),
            ]
        })
        .collect::<Vec<_>>();
    let size = wgpu::Extent3d {
        width: image.width(),
        height: image.height(),
        depth_or_array_layers: 1,
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

    queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(&pixels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(image.width() * 8),
            rows_per_image: Some(image.height()),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok(TextureDefinition {
        texture,
        view,
        view_type: TextureViewType::D2,
        radiance_scale,
    })
}

fn hdr_channel_to_f16_bits(channel: f32, radiance_scale: f32) -> u16 {
    let stored = if channel.is_finite() {
        (channel / radiance_scale).clamp(-HALF_FLOAT_MAX, HALF_FLOAT_MAX)
    } else {
        0.0
    };
    f16::from_f32(stored).to_bits()
}

fn hdri_radiance_scale(image: &image::Rgb32FImage) -> f32 {
    let peak_radiance = image
        .pixels()
        .flat_map(|pixel| pixel.0)
        .filter(|channel| channel.is_finite())
        .fold(0.0f32, f32::max);
    (peak_radiance / HDR_HALF_FLOAT_STORAGE_TARGET).max(1.0)
}

fn cubemap_capture_matrices_with_parameters(
    roughness: f32,
    source_resolution: f32,
) -> [CubemapCaptureUniform; 6] {
    let origin = Point3::new(0.0, 0.0, 0.0);
    // OpenGL framebuffer rows run bottom to top. WebGPU render attachments run
    // top to bottom, so flip clip-space Y while retaining the standard
    // LearnOpenGL cubemap directions and up vectors.
    let render_target_flip = Matrix4::from_nonuniform_scale(1.0, -1.0, 1.0);
    let projection = render_target_flip
        * CUBEMAP_OPENGL_TO_WGPU_MATRIX
        * cgmath::perspective(Deg(90.0), 1.0, 0.1, 10.0);
    let directions = [
        (Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, -1.0, 0.0)),
        (Vector3::new(-1.0, 0.0, 0.0), Vector3::new(0.0, -1.0, 0.0)),
        (Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 0.0, 1.0)),
        (Vector3::new(0.0, -1.0, 0.0), Vector3::new(0.0, 0.0, -1.0)),
        (Vector3::new(0.0, 0.0, 1.0), Vector3::new(0.0, -1.0, 0.0)),
        (Vector3::new(0.0, 0.0, -1.0), Vector3::new(0.0, -1.0, 0.0)),
    ];

    directions.map(|(direction, up)| CubemapCaptureUniform {
        view_proj: (projection * Matrix4::look_at_rh(origin, origin + direction, up)).into(),
        roughness,
        source_resolution,
        _padding: [0.0; 2],
    })
}

fn render_cubemap_faces(
    render_context: &RenderContext,
    input: &TextureDefinition,
    input_dimension: wgpu::TextureViewDimension,
    shader_name: &str,
    face_size: u32,
    mip_level_count: u32,
    rendered_mip_level_count: u32,
    roughness_per_mip: bool,
    source_resolution: f32,
    label: Option<&str>,
    address_mode_u: wgpu::AddressMode,
) -> Result<TextureDefinition> {
    let device = &render_context.device;
    let queue = &render_context.queue;
    let target = create_renderable_cubemap_with_mips(device, face_size, mip_level_count, label)?;
    if rendered_mip_level_count == 0 || rendered_mip_level_count > mip_level_count {
        bail!("Rendered cubemap mip count must fit inside the target mip chain");
    }
    let input_binding = TextureBinding::new(
        device,
        &input.view,
        TextureBindingConfig {
            view_dimension: input_dimension,
            address_mode_u,
            ..Default::default()
        },
        Some("cubemap capture input"),
    );

    let capture_uniforms = (0..rendered_mip_level_count)
        .map(|mip_level| {
            let roughness = if roughness_per_mip && mip_level_count > 1 {
                mip_level as f32 / (mip_level_count - 1) as f32
            } else {
                0.0
            };
            cubemap_capture_matrices_with_parameters(roughness, source_resolution)
        })
        .collect::<Vec<_>>();
    let uniform_buffer_type = || BufferType::UniformBuffer(UniformParameters::default());
    let camera_template =
        Buffer::new_init(&[capture_uniforms[0][0]], device, uniform_buffer_type());
    let camera_buffers = capture_uniforms
        .iter()
        .map(|uniforms| {
            std::array::from_fn(|face| {
                Buffer::new_init_matching(
                    &[uniforms[face]],
                    device,
                    uniform_buffer_type(),
                    &camera_template,
                )
            })
        })
        .collect::<Vec<[Buffer; 6]>>();
    let position_layout = VertexBufferLayoutOwned {
        array_stride: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: vec![wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }],
    };
    let pipeline =
        RenderPipelineBuilder::new(render_context, label.unwrap_or("cubemap capture pipeline"))
            .shader(shader_name)
            .target_format(wgpu::TextureFormat::Rgba16Float)
            .vertex_layout(position_layout)
            .bind_group_layout(&camera_buffers[0][0].bind_group_layout)
            .bind_group_layout(&input_binding.bind_group_layout)
            .config(PipelineConfig {
                culling: None,
                depth_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                target_format: Some(wgpu::TextureFormat::Rgba16Float),
            })
            .blend(None)
            .build();
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("cubemap capture cube"),
        contents: bytemuck::cast_slice(&CAPTURE_CUBE_VERTICES),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mip_face_views = (0..rendered_mip_level_count)
        .map(|mip_level| cubemap_face_views(&target.texture, mip_level, label))
        .collect::<Vec<_>>();
    let depth_targets = (0..rendered_mip_level_count)
        .map(|mip_level| {
            let mip_size = (face_size >> mip_level).max(1);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("cubemap capture depth"),
                size: wgpu::Extent3d {
                    width: mip_size,
                    height: mip_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Texture::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (texture, view)
        })
        .collect::<Vec<_>>();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
    for mip_level in 0..rendered_mip_level_count as usize {
        for (face, face_view) in mip_face_views[mip_level].iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: face_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_targets[mip_level].1,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                ..Default::default()
            });
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &camera_buffers[mip_level][face].bind_group, &[]);
            pass.set_bind_group(1, &input_binding.bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..CAPTURE_CUBE_VERTICES.len() as u32, 0..1);
        }
    }
    queue.submit(Some(encoder.finish()));

    let RenderableCubemap {
        texture,
        cube_view,
        face_views: _,
    } = target;
    Ok(TextureDefinition {
        texture,
        view: cube_view,
        view_type: TextureViewType::Cube,
        radiance_scale: input.radiance_scale,
    })
}

fn generate_cubemap_mips(
    render_context: &RenderContext,
    cubemap: &TextureDefinition,
    face_size: u32,
    mip_level_count: u32,
    label: Option<&str>,
) -> Result<()> {
    let device = &render_context.device;
    let queue = &render_context.queue;
    if !matches!(&cubemap.view_type, TextureViewType::Cube) {
        bail!("Cubemap mip generation input must be a cubemap texture");
    }
    if cubemap.texture.width() != face_size {
        bail!("Cubemap mip generation face size does not match the texture");
    }
    if mip_level_count <= 1 {
        return Ok(());
    }
    if mip_level_count > cubemap.texture.mip_level_count() {
        bail!("Requested mip chain exceeds the cubemap texture mip count");
    }

    let source_views = (1..mip_level_count)
        .map(|target_mip| {
            cubemap.texture.create_view(&wgpu::TextureViewDescriptor {
                label,
                dimension: Some(wgpu::TextureViewDimension::Cube),
                base_array_layer: 0,
                array_layer_count: Some(6),
                base_mip_level: target_mip - 1,
                mip_level_count: Some(1),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    let source_binding = TextureBinding::new(
        device,
        &source_views[0],
        TextureBindingConfig {
            view_dimension: wgpu::TextureViewDimension::Cube,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        },
        Some("cubemap mip source"),
    );
    let mut source_bind_groups = vec![source_binding.bind_group.clone()];
    source_bind_groups.extend(
        source_views.iter().skip(1).map(|view| {
            source_binding.bind_group_for_view(device, view, Some("cubemap mip source"))
        }),
    );

    let face_uniforms = (0u32..6).map(|face| [face, 0, 0, 0]).collect::<Vec<_>>();
    let face_buffer_type = || {
        BufferType::UniformBuffer(UniformParameters {
            shader_stages: wgpu::ShaderStages::FRAGMENT,
            ..Default::default()
        })
    };
    let face_template = Buffer::new_init(
        std::slice::from_ref(&face_uniforms[0]),
        device,
        face_buffer_type(),
    );
    let face_buffers = face_uniforms
        .iter()
        .map(|uniform| {
            Buffer::new_init_matching(
                std::slice::from_ref(uniform),
                device,
                face_buffer_type(),
                &face_template,
            )
        })
        .collect::<Vec<_>>();
    let pipeline =
        RenderPipelineBuilder::new(render_context, label.unwrap_or("cubemap mip pipeline"))
            .shader("cubemap_mipmap")
            .target_format(wgpu::TextureFormat::Rgba16Float)
            .bind_group_layout(&source_binding.bind_group_layout)
            .bind_group_layout(&face_buffers[0].bind_group_layout)
            .config(PipelineConfig {
                depth_enabled: None,
                target_format: Some(wgpu::TextureFormat::Rgba16Float),
                ..Default::default()
            })
            .blend(None)
            .build();
    let target_views = (1..mip_level_count)
        .map(|mip_level| cubemap_face_views(&cubemap.texture, mip_level, label))
        .collect::<Vec<_>>();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
    for target_mip in 1..mip_level_count as usize {
        for face in 0..6 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_views[target_mip - 1][face],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                ..Default::default()
            });
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &source_bind_groups[target_mip - 1], &[]);
            pass.set_bind_group(1, &face_buffers[face].bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
    queue.submit(Some(encoder.finish()));

    Ok(())
}

fn render_equirectangular_to_cubemap(
    render_context: &RenderContext,
    hdri: &TextureDefinition,
    face_size: u32,
    label: Option<&str>,
) -> Result<TextureDefinition> {
    if !matches!(&hdri.view_type, TextureViewType::D2) {
        bail!("Equirectangular capture input must be a 2D texture");
    }
    let mip_level_count = face_size.ilog2() + 1;
    let environment = render_cubemap_faces(
        render_context,
        hdri,
        wgpu::TextureViewDimension::D2,
        "equirectangular_to_cubemap",
        face_size,
        mip_level_count,
        1,
        false,
        1.0,
        label,
        wgpu::AddressMode::Repeat,
    )?;
    generate_cubemap_mips(
        render_context,
        &environment,
        face_size,
        mip_level_count,
        label,
    )?;
    Ok(environment)
}

fn render_irradiance_map(
    render_context: &RenderContext,
    environment: &TextureDefinition,
    face_size: u32,
    label: Option<&str>,
) -> Result<TextureDefinition> {
    if !matches!(&environment.view_type, TextureViewType::Cube) {
        bail!("Irradiance capture input must be a cubemap texture");
    }
    render_cubemap_faces(
        render_context,
        environment,
        wgpu::TextureViewDimension::Cube,
        "irradiance_convolution",
        face_size,
        1,
        1,
        false,
        environment.texture.width() as f32,
        label,
        wgpu::AddressMode::ClampToEdge,
    )
}

fn render_prefiltered_environment_map(
    render_context: &RenderContext,
    environment: &TextureDefinition,
    face_size: u32,
    mip_level_count: u32,
    label: Option<&str>,
) -> Result<TextureDefinition> {
    if !matches!(&environment.view_type, TextureViewType::Cube) {
        bail!("Specular prefilter input must be a cubemap texture");
    }
    render_cubemap_faces(
        render_context,
        environment,
        wgpu::TextureViewDimension::Cube,
        "prefilter_environment",
        face_size,
        mip_level_count,
        mip_level_count,
        true,
        environment.texture.width() as f32,
        label,
        wgpu::AddressMode::ClampToEdge,
    )
}

fn render_brdf_lut(
    render_context: &RenderContext,
    size: u32,
    label: Option<&str>,
) -> Result<TextureDefinition> {
    let device = &render_context.device;
    let queue = &render_context.queue;
    if size == 0 {
        bail!("BRDF LUT size must be non-zero");
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let pipeline =
        RenderPipelineBuilder::new(render_context, label.unwrap_or("BRDF integration pipeline"))
            .shader("brdf_integration")
            .target_format(wgpu::TextureFormat::Rg16Float)
            .depth(false)
            .blend(None)
            .build();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            ..Default::default()
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit(Some(encoder.finish()));

    Ok(TextureDefinition {
        texture,
        view,
        view_type: TextureViewType::D2,
        radiance_scale: 1.0,
    })
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

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TextureParameters {
    values: [f32; 4],
}

fn create_bind_group_and_layout(
    device: &wgpu::Device,
    textures: &[TextureDefinition],
    sampler: &Sampler,
    radiance_scale: f32,
) -> (BindGroupLayout, BindGroup, wgpu::Buffer) {
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
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: entries.len() as u32,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
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
    let radiance_scale_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("texture radiance scale"),
        contents: bytemuck::bytes_of(&TextureParameters {
            values: [radiance_scale, 0.0, 0.0, 0.0],
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    entries.push(wgpu::BindGroupEntry {
        binding: entries.len() as u32,
        resource: radiance_scale_buffer.as_entire_binding(),
    });

    // Create bind group for the texture
    let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &texture_bind_group_layout,
        entries: &entries,
        label: Some("diffuse_bind_group"),
    });

    (
        texture_bind_group_layout,
        diffuse_bind_group,
        radiance_scale_buffer,
    )
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
        let radiance_scale = textures
            .first()
            .map(|texture| texture.radiance_scale)
            .unwrap_or(1.0);
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
        let (bind_group_layout, bind_group, radiance_scale_buffer) = create_bind_group_and_layout(
            self.gfx.get_device(),
            &textures,
            &sampler,
            radiance_scale,
        );
        Texture {
            label: self.label.to_string(),
            texture: textures,
            sampler,
            bind_group_layout,
            bind_group,
            _radiance_scale_buffer: radiance_scale_buffer,
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

    /// Decodes a 2:1 Radiance HDR image, uploads it, and renders a floating-point
    /// environment cubemap on the GPU.
    pub fn hdri_cubemap(self, image: &[u8]) -> Texture {
        ensure_hdr_preprocessing_supported(self.gfx.get_render_context().rgba16float_renderable)
            .expect("GPU HDR cubemap preprocessing is unavailable");
        let hdri = decode_hdri(image).expect("Failed to decode Radiance HDR image");
        let face_size = hdri_face_size(&hdri).expect("Invalid equirectangular HDR image");
        let hdri_texture = upload_hdri(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            &hdri,
            Some(self.label),
        )
        .expect("Failed to upload Radiance HDR image");
        let cubemap = render_equirectangular_to_cubemap(
            self.gfx.get_render_context(),
            &hdri_texture,
            face_size,
            Some(self.label),
        )
        .expect("Failed to render HDR environment cubemap");
        self.finish(vec![cubemap])
    }

    /// Renders a diffuse irradiance cubemap from an existing environment cubemap.
    pub fn irradiance_map(self, environment: &Texture) -> Texture {
        self.irradiance_map_with_size(environment, DEFAULT_IRRADIANCE_MAP_FACE_SIZE)
    }

    pub fn irradiance_map_with_size(self, environment: &Texture, face_size: u32) -> Texture {
        ensure_hdr_preprocessing_supported(self.gfx.get_render_context().rgba16float_renderable)
            .expect("GPU irradiance preprocessing is unavailable");
        let environment = environment
            .texture
            .first()
            .expect("An environment Texture must contain a cubemap");
        let irradiance = render_irradiance_map(
            self.gfx.get_render_context(),
            environment,
            face_size,
            Some(self.label),
        )
        .expect("Failed to render irradiance cubemap");

        self.finish(vec![irradiance])
    }

    /// Builds the complete image-based lighting texture set used by the PBR
    /// shader: diffuse irradiance, roughness-prefiltered specular radiance, and
    /// the split-sum BRDF integration lookup table.
    pub fn ibl_maps(self, environment: &Texture) -> Texture {
        self.ibl_maps_with_sizes(
            environment,
            DEFAULT_IRRADIANCE_MAP_FACE_SIZE,
            DEFAULT_PREFILTERED_ENVIRONMENT_FACE_SIZE,
            DEFAULT_PREFILTERED_ENVIRONMENT_MIP_LEVELS,
            DEFAULT_BRDF_LUT_SIZE,
        )
    }

    pub fn ibl_maps_with_sizes(
        self,
        environment: &Texture,
        irradiance_face_size: u32,
        prefiltered_face_size: u32,
        prefiltered_mip_level_count: u32,
        brdf_lut_size: u32,
    ) -> Texture {
        let render_context = self.gfx.get_render_context();
        ensure_ibl_preprocessing_supported(
            render_context.rgba16float_renderable,
            render_context.rg16float_renderable,
        )
        .expect("GPU specular IBL preprocessing is unavailable");
        let environment = environment
            .texture
            .first()
            .expect("An environment Texture must contain a cubemap");
        let irradiance = render_irradiance_map(
            render_context,
            environment,
            irradiance_face_size,
            Some(self.label),
        )
        .expect("Failed to render irradiance cubemap");
        let prefiltered_environment = render_prefiltered_environment_map(
            render_context,
            environment,
            prefiltered_face_size,
            prefiltered_mip_level_count,
            Some(self.label),
        )
        .expect("Failed to render prefiltered environment cubemap");
        let brdf_lut = render_brdf_lut(render_context, brdf_lut_size, Some(self.label))
            .expect("Failed to render BRDF integration lookup table");

        self.finish(vec![irradiance, prefiltered_environment, brdf_lut])
    }

    /// Compatibility helper. New code can retain the environment cubemap and
    /// pass it to `irradiance_map` for later IBL preprocessing stages.
    pub fn hdri_irradiance_map(self, image: &[u8]) -> Texture {
        ensure_hdr_preprocessing_supported(self.gfx.get_render_context().rgba16float_renderable)
            .expect("GPU irradiance preprocessing is unavailable");
        let hdri = decode_hdri(image).expect("Failed to decode Radiance HDR image");
        let environment_face_size =
            hdri_face_size(&hdri).expect("Invalid equirectangular HDR image");
        let hdri_texture = upload_hdri(
            self.gfx.get_device(),
            self.gfx.get_queue(),
            &hdri,
            Some(self.label),
        )
        .expect("Failed to upload Radiance HDR image");
        let environment = render_equirectangular_to_cubemap(
            self.gfx.get_render_context(),
            &hdri_texture,
            environment_face_size,
            Some(self.label),
        )
        .expect("Failed to render HDR environment cubemap");
        let irradiance = render_irradiance_map(
            self.gfx.get_render_context(),
            &environment,
            DEFAULT_IRRADIANCE_MAP_FACE_SIZE,
            Some(self.label),
        )
        .expect("Failed to render irradiance cubemap");
        self.finish(vec![irradiance])
    }
}

fn ensure_hdr_preprocessing_supported(supported: bool) -> Result<()> {
    if !supported {
        bail!(
            "The selected GPU adapter cannot use filterable Rgba16Float textures as render attachments"
        );
    }
    Ok(())
}

fn ensure_ibl_preprocessing_supported(
    rgba16float_renderable: bool,
    rg16float_renderable: bool,
) -> Result<()> {
    ensure_hdr_preprocessing_supported(rgba16float_renderable)?;
    if !rg16float_renderable {
        bail!(
            "The selected GPU adapter cannot use filterable Rg16Float textures as render attachments"
        );
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bright_hdri_channels_stay_finite_in_half_float_storage() {
        let image =
            image::Rgb32FImage::from_pixel(4, 2, image::Rgb([262_144.0, 196_608.0, 71_680.0]));
        let radiance_scale = hdri_radiance_scale(&image);

        assert!(radiance_scale > 1.0);
        for channel in image.get_pixel(0, 0).0 {
            let stored = f16::from_bits(hdr_channel_to_f16_bits(channel, radiance_scale));
            assert!(stored.is_finite());
            assert!(stored.to_f32().abs() <= HDR_HALF_FLOAT_STORAGE_TARGET);
        }

        let restored = f16::from_bits(hdr_channel_to_f16_bits(
            image.get_pixel(0, 0)[0],
            radiance_scale,
        ))
        .to_f32()
            * radiance_scale;
        let relative_error = (restored - 262_144.0).abs() / 262_144.0;
        assert!(relative_error < 0.001);
        assert_eq!(
            f16::from_bits(hdr_channel_to_f16_bits(f32::INFINITY, radiance_scale)),
            f16::ZERO,
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn hdr_scale_shader_bindings_validate() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let std::result::Result::Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
                ..Default::default()
            }))
        else {
            return;
        };
        let (device, _) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("Failed to create shader validation device");
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);

        let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR scale skybox validation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/skybox.wgsl").into()),
        });
        let _skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR scale skybox validation"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &skybox_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &skybox_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let pbr_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HDR scale PBR validation"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/pbr_shader_textured.wgsl").into(),
            ),
        });
        let _pbr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR scale PBR validation"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &pbr_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 32,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 12,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 20,
                                shader_location: 2,
                            },
                        ],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 44,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 5,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 6,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 32,
                                shader_location: 7,
                            },
                        ],
                    }),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &pbr_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let validation_error = pollster::block_on(error_scope.pop());
        assert!(
            validation_error.is_none(),
            "HDR scale shader validation failed: {validation_error:?}"
        );
    }
}
