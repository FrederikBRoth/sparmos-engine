use std::f32::consts::PI;

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
    pub indices: Vec<u16>,
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

        let mut i = 0u16;
        let mut push_face = |positions: [[f32; 3]; 6], normal: [f32; 3]| {
            for pos in positions.iter() {
                vertices.push(PrimitiveVertex {
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
pub fn mobius_strip(
    R: f32,
    w: f32,
    num_u: usize,
    num_v: usize,
    reversed: bool,
) -> Vec<PrimitiveFace> {
    let mut faces = Vec::new();
    let mut colorb = true;

    // Position function for the Möbius strip
    let p = |u: f32, v: f32| -> [f32; 3] {
        let cu = u.cos();
        let su = u.sin();
        let cu2 = (u / 2.0).cos();
        let su2 = (u / 2.0).sin();
        [(R + v * cu2) * cu, (R + v * cu2) * su, v * su2]
    };

    // Loop over grid cells
    for i in 0..num_u {
        let i_next = (i + 1) % num_u;
        let u0 = i as f32 / num_u as f32 * 2.0 * std::f32::consts::PI;
        let u1 = i_next as f32 / num_u as f32 * 2.0 * std::f32::consts::PI;

        for j in 0..num_v - 1 {
            // Compute v coordinates
            let v0 = -w + j as f32 / (num_v as f32 - 1.0) * 2.0 * w;
            let v1 = -w + (j + 1) as f32 / (num_v as f32 - 1.0) * 2.0 * w;

            // Möbius flip at the seam
            let p00 = p(u0, v0);
            let p01 = p(u0, v1);
            let p10 = if i_next == 0 { p(u1, -v0) } else { p(u1, v0) };
            let p11 = if i_next == 0 { p(u1, -v1) } else { p(u1, v1) };

            let color = if colorb {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            colorb = !colorb;

            // --- Shared vertices for this quad (4 unique vertices) ---
            let mut vertices = vec![
                PrimitiveVertex {
                    position: p00,
                    color,
                    normal: [0.0, 0.0, 0.0],
                },
                PrimitiveVertex {
                    position: p10,
                    color,
                    normal: [0.0, 0.0, 0.0],
                },
                PrimitiveVertex {
                    position: p11,
                    color,
                    normal: [0.0, 0.0, 0.0],
                },
                PrimitiveVertex {
                    position: p01,
                    color,
                    normal: [0.0, 0.0, 0.0],
                },
            ];

            // --- Indexed triangles (two per quad) ---
            let mut indices: Vec<u16> = if !reversed {
                vec![0, 1, 2, 0, 2, 3]
            } else {
                vec![0, 2, 1, 0, 3, 2]
            };

            // --- Compute averaged normals ---
            for tri in indices.chunks(3) {
                let v0 = vertices[tri[0] as usize].position;
                let v1 = vertices[tri[1] as usize].position;
                let v2 = vertices[tri[2] as usize].position;
                let n = compute_normal(v0, v1, v2);
                for &idx in tri {
                    let vert = &mut vertices[idx as usize];
                    vert.normal[0] += n[0];
                    vert.normal[1] += n[1];
                    vert.normal[2] += n[2];
                }
            }

            // Normalize normals
            for v in &mut vertices {
                let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
                if len > 1e-6 {
                    v.normal[0] /= len;
                    v.normal[1] /= len;
                    v.normal[2] /= len;
                }
            }

            // Store one PrimitiveFace (shared vertices + indices)
            faces.push(PrimitiveFace { vertices, indices });
        }
    }

    faces
}

fn compute_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 {
        [n[0] / len, n[1] / len, n[2] / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}
