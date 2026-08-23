const _VERTICES: &[TexturedVertex] = &[
    TexturedVertex {
        position: [0.0, 0.0, 0.0],
        tex_coords: [1.0, 0.0],

        normal: [0.0, -1.0, 0.0],
    }, // A
    TexturedVertex {
        position: [0.0, 0.0, 1.0],
        tex_coords: [0.0, 0.0],

        normal: [0.0, 0.0, 1.0],
    }, // B
    TexturedVertex {
        position: [1.0, 0.0, 0.0],
        tex_coords: [1.0, 1.0],

        normal: [0.0, 0.0, 0.0],
    }, // C
    TexturedVertex {
        position: [1.0, 0.0, 1.0],
        tex_coords: [0.0, 1.0],

        normal: [1.0, 0.0, 0.0],
    }, // D
    TexturedVertex {
        position: [1.0, 1.0, 0.0],
        tex_coords: [1.0, 0.0],

        normal: [0.0, 1.0, 0.0],
    }, // A
    TexturedVertex {
        position: [1.0, 1.0, 1.0],
        tex_coords: [0.0, 0.0],

        normal: [0.0, 0.0, 0.0],
    }, // B
    TexturedVertex {
        position: [0.0, 1.0, 0.0],
        tex_coords: [1.0, 1.0],

        normal: [0.0, 0.0, -1.0],
    }, // C
    TexturedVertex {
        position: [0.0, 1.0, 1.0],
        tex_coords: [0.0, 1.0],

        normal: [-1.0, 0.0, 0.0],
    }, // D
];
// impl TexturedCube {
//     pub fn new() -> TexturedCube {
//         TexturedCube {
//             vertices: VERTICES.to_vec(),
//             indices: INDICES.to_vec(),
//         }
//     }
// }
#[rustfmt::skip]
const _INDICES: &[u16] = &[
    //
    0, 2, 3,   0, 3, 1, // front
    4, 6, 7,   4, 7, 5, // back
    3, 2, 4,   3, 4, 5, // right
    7, 6, 0,   7, 0, 1, // left
    6, 4, 2,   6, 2, 0, // bottom
    1, 3, 5,   1, 5, 7  // top
];

use crate::core::geometry::{Primitive, PrimitiveVertex, Textured, TexturedVertex};

pub enum Meshes {
    Cube,
    Sphere,
}

impl Meshes {
    pub fn create_primitive(&self) -> Primitive {
        match self {
            Meshes::Cube => new_cube(),
            Meshes::Sphere => todo!(),
        }
    }

    pub fn create_textured(&self) -> Textured {
        match self {
            Meshes::Cube => todo!(),
            Meshes::Sphere => create_sphere(10.0, 64, 32),
        }
    }
}
pub fn new_cube() -> Primitive {
    let face_color = [1.0, 0.0, 1.0];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let mut i = 0u32;
    let mut push_face = |positions: [[f32; 3]; 6], normal: [f32; 3]| {
        for pos in positions.iter() {
            vertices.push(PrimitiveVertex {
                quad_id: 0,
                position: *pos,
                color: face_color,
                normal,
            });
            indices.push(i);
            i += 1;
        }
    };

    // Face vertices (two triangles per face)
    push_face(
        [
            // Front (Z+)
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        [0.0, 0.0, 1.0],
    );

    push_face(
        [
            // Back (Z-)
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        [0.0, 0.0, -1.0],
    );

    push_face(
        [
            // Right (X+)
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
        [1.0, 0.0, 0.0],
    );

    push_face(
        [
            // Left (X-)
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
        [-1.0, 0.0, 0.0],
    );

    push_face(
        [
            // Top (Y+)
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        [0.0, 1.0, 0.0],
    );

    push_face(
        [
            // Bottom (Y-)
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        [0.0, -1.0, 0.0],
    );

    Primitive { vertices, indices }
}
use std::f32::consts::PI;

pub fn create_sphere(radius: f32, segments: u32, rings: u32) -> Textured {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for ring in 0..=rings {
        // 0 = north pole, 1 = south pole
        let v = ring as f32 / rings as f32;
        let theta = v * PI;

        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let phi = u * 2.0 * PI;

            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            // Unit sphere position
            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            let normal = [x, y, z];

            let position = [x * radius, y * radius, z * radius];

            let tex_coords = [u, v];

            vertices.push(TexturedVertex {
                position,
                normal,
                tex_coords,
            });
        }
    }

    let row_size = segments + 1;

    for ring in 0..rings {
        for segment in 0..segments {
            let current = ring * row_size + segment;
            let next = current + row_size;

            indices.push(current);
            indices.push(current + 1);
            indices.push(next);

            indices.push(current + 1);
            indices.push(next + 1);
            indices.push(next);
        }
    }

    Textured { vertices, indices }
}
