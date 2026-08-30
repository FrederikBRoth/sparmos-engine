use std::collections::HashMap;

use gltf::{Document, buffer::Data, image::Format};
use image::{DynamicImage, RgbaImage};

use crate::{
    application::graphics::Graphics,
    core::{
        geometry::{Mesh, TexturedVertex},
        object_loading::model::Model,
        render::{InstanceControllerHandle, MaterialHandle, TextureHandle},
        texture::Texture,
    },
};

pub fn load_gltf(
    gfx: &mut Graphics,
    data: &[u8],
    instance_handle: InstanceControllerHandle,
    material: MaterialHandle,
) -> Model {
    let (spec, buffer_data, image_data) =
        gltf::import_slice(data).expect("GLTF object not imported correctly");
    let meshes = load_meshes(gfx, &spec, &buffer_data, &image_data);

    let mut mesh_texture_pairs = vec![];
    for (mesh, texture_handle) in meshes {
        let mesh_handle = gfx.get_render_context_mut().gpu_objects.meshes.insert(mesh);

        mesh_texture_pairs.push((mesh_handle, Some(texture_handle)));
    }

    let mut materials = HashMap::new();
    for (mesh, _) in mesh_texture_pairs.iter() {
        materials.insert(*mesh, material);
    }
    Model {
        meshes: mesh_texture_pairs,
        instance: instance_handle,
        materials,
    }
}

fn load_meshes<'a>(
    gfx: &mut Graphics,
    document: &'a Document,
    buffer_data: &'a [Data],
    image_data: &'a [gltf::image::Data],
) -> Vec<(Mesh, TextureHandle)> {
    let mut meshes = Vec::new();
    let mut material_textures = HashMap::<Option<usize>, TextureHandle>::new();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));
            let positions = reader.read_positions().unwrap();

            let normals = reader.read_normals().unwrap();

            let tex_coords = reader.read_tex_coords(0).unwrap().into_f32();

            let indices = reader
                .read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<u32>>();
            let vertices: Vec<TexturedVertex> = positions
                .into_iter()
                .zip(normals)
                .zip(tex_coords)
                .map(|((position, normal), tex_coord)| TexturedVertex {
                    position,
                    tex_coords: tex_coord,
                    normal,
                })
                .collect();
            let mesh = Mesh::new(
                gfx.get_device(),
                &vertices,
                &indices,
                vertices.len() as u32,
                indices.len() as u32,
            );

            let material = primitive.material();
            let material_key = material.index();
            let texture_handle = if let Some(texture_handle) = material_textures.get(&material_key)
            {
                *texture_handle
            } else {
                let texture = load_material_texture(gfx, &material, image_data);
                let texture_handle = gfx
                    .get_render_context_mut()
                    .gpu_objects
                    .textures
                    .insert(texture);
                material_textures.insert(material_key, texture_handle);
                texture_handle
            };

            meshes.push((mesh, texture_handle));
        }
    }

    meshes
}

fn load_material_texture(
    gfx: &mut Graphics,
    material: &gltf::Material<'_>,
    image_data: &[gltf::image::Data],
) -> Texture {
    let pbr = material.pbr_metallic_roughness();
    let mut builder = gfx.pbr_texture(material.name().unwrap_or("gltf_material"));

    if let Some(info) = pbr.base_color_texture() {
        let image = &image_data[info.texture().source().index()];
        let image =
            DynamicImage::ImageRgba8(gltf_image_to_rgba8(image, pbr.base_color_factor(), true));

        builder = builder.diffuse_image(&image, wgpu::TextureFormat::Rgba8UnormSrgb);
    } else {
        let factor = pbr.base_color_factor();

        builder = builder.diffuse_color([factor[0], factor[1], factor[2]]);
    }

    if let Some(info) = material.normal_texture() {
        let image = &image_data[info.texture().source().index()];
        let image =
            DynamicImage::ImageRgba8(gltf_image_to_rgba8(image, [1.0, 1.0, 1.0, 1.0], false));

        builder = builder.normal_image(&image, wgpu::TextureFormat::Rgba8Unorm);
    }

    if let Some(info) = pbr.metallic_roughness_texture() {
        let image = &image_data[info.texture().source().index()];
        let (metallic, roughness) =
            gltf_metallic_roughness_images(image, pbr.metallic_factor(), pbr.roughness_factor());
        let metallic = DynamicImage::ImageRgba8(metallic);
        let roughness = DynamicImage::ImageRgba8(roughness);

        builder = builder
            .metallic_image(&metallic, wgpu::TextureFormat::Rgba8Unorm)
            .roughness_image(&roughness, wgpu::TextureFormat::Rgba8Unorm);
    } else {
        builder = builder
            .metallic(pbr.metallic_factor())
            .roughness(pbr.roughness_factor());
    }

    if let Some(info) = material.occlusion_texture() {
        let image = &image_data[info.texture().source().index()];
        let image = DynamicImage::ImageRgba8(gltf_channel_image(image, 0, 1.0));

        builder = builder.ao_image(&image, wgpu::TextureFormat::Rgba8Unorm);
    } else {
        builder = builder.ao(1.0);
    }

    builder.build()
}

fn gltf_image_to_rgba8(image: &gltf::image::Data, factor: [f32; 4], srgb: bool) -> RgbaImage {
    let mut rgba = Vec::with_capacity(image.width as usize * image.height as usize * 4);

    match image.format {
        Format::R8G8B8 => {
            for pixel in image.pixels.chunks_exact(3) {
                rgba.extend_from_slice(&[
                    apply_factor(pixel[0], factor[0], srgb),
                    apply_factor(pixel[1], factor[1], srgb),
                    apply_factor(pixel[2], factor[2], srgb),
                    apply_factor(255, factor[3], false),
                ]);
            }
        }
        Format::R8G8B8A8 => {
            for pixel in image.pixels.chunks_exact(4) {
                rgba.extend_from_slice(&[
                    apply_factor(pixel[0], factor[0], srgb),
                    apply_factor(pixel[1], factor[1], srgb),
                    apply_factor(pixel[2], factor[2], srgb),
                    apply_factor(pixel[3], factor[3], false),
                ]);
            }
        }
        format => panic!("Unsupported glTF image format: {format:?}"),
    }

    RgbaImage::from_raw(image.width, image.height, rgba).expect("Invalid glTF image dimensions")
}

fn gltf_metallic_roughness_images(
    image: &gltf::image::Data,
    metallic_factor: f32,
    roughness_factor: f32,
) -> (RgbaImage, RgbaImage) {
    let channel_count = match image.format {
        Format::R8G8B8 => 3,
        Format::R8G8B8A8 => 4,
        format => panic!("Unsupported glTF metallic-roughness image format: {format:?}"),
    };
    let capacity = image.width as usize * image.height as usize * 4;
    let mut metallic = Vec::with_capacity(capacity);
    let mut roughness = Vec::with_capacity(capacity);

    for pixel in image.pixels.chunks_exact(channel_count) {
        let metallic_value = apply_factor(pixel[2], metallic_factor, false);
        let roughness_value = apply_factor(pixel[1], roughness_factor, false);
        metallic.extend_from_slice(&[metallic_value, metallic_value, metallic_value, 255]);
        roughness.extend_from_slice(&[roughness_value, roughness_value, roughness_value, 255]);
    }

    (
        RgbaImage::from_raw(image.width, image.height, metallic)
            .expect("Invalid glTF metallic image dimensions"),
        RgbaImage::from_raw(image.width, image.height, roughness)
            .expect("Invalid glTF roughness image dimensions"),
    )
}

fn gltf_channel_image(image: &gltf::image::Data, channel: usize, factor: f32) -> RgbaImage {
    let channel_count = match image.format {
        Format::R8G8B8 => 3,
        Format::R8G8B8A8 => 4,
        format => panic!("Unsupported glTF channel image format: {format:?}"),
    };
    assert!(channel < channel_count, "glTF image channel is missing");
    let mut rgba = Vec::with_capacity(image.width as usize * image.height as usize * 4);

    for pixel in image.pixels.chunks_exact(channel_count) {
        let value = apply_factor(pixel[channel], factor, false);
        rgba.extend_from_slice(&[value, value, value, 255]);
    }

    RgbaImage::from_raw(image.width, image.height, rgba).expect("Invalid glTF image dimensions")
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn apply_factor(value: u8, factor: f32, srgb: bool) -> u8 {
    let value = value as f32 / 255.0;

    let result = if srgb {
        // Texture value is stored as sRGB.
        //
        // Decode to linear first, apply the glTF factor in
        // linear space, then encode it back to sRGB because
        // we're going to upload this as Rgba8UnormSrgb.
        let linear = srgb_to_linear(value);
        let factored = linear * factor;

        linear_to_srgb(factored)
    } else {
        // Metallic, roughness, AO, normals etc. are linear.
        value * factor
    };

    (result.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(format: Format, pixels: Vec<u8>, width: u32, height: u32) -> gltf::image::Data {
        gltf::image::Data {
            pixels,
            format,
            width,
            height,
        }
    }

    #[test]
    fn decoded_images_are_factored_without_an_encode_round_trip() {
        let source = image(Format::R8G8B8A8, vec![100, 150, 200, 255], 1, 1);
        let converted = gltf_image_to_rgba8(&source, [0.5, 1.0, 0.25, 0.5], false);

        assert_eq!(converted.as_raw(), &[50, 150, 50, 128]);
    }

    #[test]
    fn metallic_and_roughness_use_the_gltf_blue_and_green_channels() {
        let source = image(Format::R8G8B8A8, vec![7, 101, 203, 255], 1, 1);
        let (metallic, roughness) = gltf_metallic_roughness_images(&source, 0.5, 0.25);

        assert_eq!(metallic.as_raw(), &[102, 102, 102, 255]);
        assert_eq!(roughness.as_raw(), &[25, 25, 25, 255]);
    }

    #[test]
    fn occlusion_uses_the_gltf_red_channel() {
        let source = image(Format::R8G8B8, vec![73, 101, 203], 1, 1);
        let occlusion = gltf_channel_image(&source, 0, 1.0);

        assert_eq!(occlusion.as_raw(), &[73, 73, 73, 255]);
    }
}
