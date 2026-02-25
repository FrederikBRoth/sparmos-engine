use std::f32::consts::PI;

use crate::entity::entity::PrimitiveMesh;
use crate::entity::entity::TexturedVertex;

pub struct TexturedCube {
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u16>,
}
const VERTICES: &[TexturedVertex] = &[
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
impl TexturedCube {
    pub fn new() -> TexturedCube {
        TexturedCube {
            vertices: VERTICES.to_vec(),
            indices: INDICES.to_vec(),
        }
    }
}
#[rustfmt::skip]
const INDICES: &[u16] = &[
    //
    0, 2, 3,   0, 3, 1, // front
    4, 6, 7,   4, 7, 5, // back
    3, 2, 4,   3, 4, 5, // right
    7, 6, 0,   7, 0, 1, // left
    6, 4, 2,   6, 2, 0, // bottom 
    1, 3, 5,   1, 5, 7  // top
];

use crate::entity::entity::PrimitiveVertex;

pub struct PrimitiveCube {
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u32>,
}
// const PRIMITIVE_VERTICES: &[PrimitiveVertex] = &[
//     PrimitiveVertex {
//         position: [0.0, 0.0, 0.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [0.0, -1.0, 0.0],
//     }, // A
//     PrimitiveVertex {
//         position: [0.0, 0.0, 1.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [0.0, 0.0, 1.0],
//     }, // B
//     PrimitiveVertex {
//         position: [1.0, 0.0, 0.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [0.0, 0.0, 0.0],
//     }, // C
//     PrimitiveVertex {
//         position: [1.0, 0.0, 1.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [1.0, 0.0, 0.0],
//     }, // D
//     PrimitiveVertex {
//         position: [1.0, 1.0, 0.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [0.0, 1.0, 0.0],
//     }, // A
//     PrimitiveVertex {
//         position: [1.0, 1.0, 1.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [0.0, 0.0, 0.0],
//     }, // B
//     PrimitiveVertex {
//         position: [0.0, 1.0, 0.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [0.0, 0.0, -1.0],
//     }, // C
//     PrimitiveVertex {
//         position: [0.0, 1.0, 1.0],
//         color: [1.0, 0.0, 1.0],
//         normal: [-1.0, 0.0, 0.0],
//     }, // D
// ];
// impl PrimitiveCube {
//     pub fn new() -> PrimitiveCube {
//         PrimitiveCube {
//             vertices: PRIMITIVE_VERTICES.to_vec(),
//             indices: INDICES.to_vec(),
//         }
//     }
// }

impl PrimitiveCube {
    pub fn new() -> Self {
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

        Self { vertices, indices }
    }
}

pub struct PrimitiveFace {
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u16>,
}
impl PrimitiveFace {
    pub fn new() -> Self {
        let face_color = [1.0, 0.0, 1.0];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let mut i = 0u16;
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

        Self { vertices, indices }
    }
}
