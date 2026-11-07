//! Resource vertices in the original runtime representation.
//! FixRound and FixDiv are modeled; faces and slot overrides are separate stages.
use crate::{Result, binary::Reader, recovered};

#[derive(Clone, Debug)]
pub struct PackedShape {
    /// Signed coordinates in eighth-unit increments, in original vertex order.
    pub vertices: Vec<[i16; 3]>,
    /// Base planar dimensions in 16.16 units, before loader overrides.
    pub dimensions: [i32; 2],
    /// Radius after slot finalization, conditional on modeled Macintosh FixDiv.
    pub candidate_radius: i16,
}

impl PackedShape {
    /// Selector 6: CODE 19 +37e8..382c vertices and +3984..39bc faces.
    /// Static base mesh; original live deformation and subtype colors are separate.
    pub fn ammo_dump() -> Self {
        let vertices = [[-9, 0, -9], [9, 0, -9], [9, 0, 9], [-9, 0, 9]]
            .map(|v| recovered::candidate_runtime_vertex(v.map(|n| n << 16), 65536))
            .to_vec();
        Self {
            dimensions: recovered::runtime_collision_dimensions(&vertices),
            candidate_radius: recovered::runtime_collision_radius(
                &vertices,
                &mut recovered::candidate_fix_div,
            ),
            vertices,
        }
    }

    /// CODE 19 +0x1014..+0x10f0. Base flag, before optional type-2 bounds copy.
    pub fn flag(filled: bool) -> (Self, Vec<Vec<usize>>) {
        let vertices = [[0, 0, 0], [0, 14, 0], [7, 14, 0], [7, 20, 0], [0, 20, 0]]
            .map(|v| recovered::candidate_runtime_vertex(v.map(|n| n << 16), 65536))
            .to_vec();
        let shape = Self {
            dimensions: recovered::runtime_collision_dimensions(&vertices),
            candidate_radius: recovered::runtime_collision_radius(
                &vertices,
                &mut recovered::candidate_fix_div,
            ),
            vertices,
        };
        let mut faces = vec![vec![0, 1]];
        if filled {
            faces.extend([vec![1, 2, 3, 4], vec![4, 3, 2, 1]]);
        } else {
            faces.extend([vec![1, 2], vec![2, 3], vec![3, 4], vec![4, 1]]);
        }
        (shape, faces)
    }

    /// Selector 2, original vertex arguments at CODE 19 +0x3144..+0x3180.
    pub fn tetrahedron() -> Self {
        let vertices = [
            [-9 << 16, 0, -342097],
            [9 << 16, 0, -342097],
            [0, 0, 684195],
            [0, 1132462, 0],
        ]
        .map(|v| recovered::candidate_runtime_vertex(v, 65536))
        .to_vec();
        Self {
            dimensions: recovered::runtime_collision_dimensions(&vertices),
            candidate_radius: recovered::runtime_collision_radius(
                &vertices,
                &mut recovered::candidate_fix_div,
            ),
            vertices,
        }
    }

    /// CODE 18 +0x10ac, arguments in stack order +8 through +0x24.
    /// Generates the eight corners shared by several procedural builders.
    pub fn tapered_box(arguments: [i32; 8]) -> Self {
        let [extra_z, extra_x, width_z, height, width_x, z, y, x] = arguments;
        let top_x = width_x / 2;
        let top_z = width_z / 2;
        let bottom_x = top_x.wrapping_add(extra_x / 2);
        let bottom_z = top_z.wrapping_add(extra_z / 2);
        let half_y = height / 2;
        let vertices = [
            [
                x.wrapping_sub(bottom_x),
                y.wrapping_sub(half_y),
                z.wrapping_sub(bottom_z),
            ],
            [
                x.wrapping_sub(bottom_x),
                y.wrapping_sub(half_y),
                z.wrapping_add(bottom_z),
            ],
            [
                x.wrapping_sub(top_x),
                y.wrapping_add(half_y),
                z.wrapping_sub(top_z),
            ],
            [
                x.wrapping_sub(top_x),
                y.wrapping_add(half_y),
                z.wrapping_add(top_z),
            ],
            [
                x.wrapping_add(bottom_x),
                y.wrapping_sub(half_y),
                z.wrapping_sub(bottom_z),
            ],
            [
                x.wrapping_add(bottom_x),
                y.wrapping_sub(half_y),
                z.wrapping_add(bottom_z),
            ],
            [
                x.wrapping_add(top_x),
                y.wrapping_add(half_y),
                z.wrapping_sub(top_z),
            ],
            [
                x.wrapping_add(top_x),
                y.wrapping_add(half_y),
                z.wrapping_add(top_z),
            ],
        ]
        .map(|v| recovered::candidate_runtime_vertex(v, 65536))
        .to_vec();
        let dimensions = recovered::runtime_collision_dimensions(&vertices);
        Self {
            candidate_radius: recovered::runtime_collision_radius(
                &vertices,
                &mut recovered::candidate_fix_div,
            ),
            vertices,
            dimensions,
        }
    }

    /// CODE 19 +3212..+328a. All four faces submit kind-one's primary color.
    pub fn tetrahedron_faces() -> Vec<Vec<usize>> {
        [[0, 3, 1], [1, 3, 2], [2, 3, 0], [0, 1, 2]]
            .map(Vec::from)
            .to_vec()
    }

    /// Use only when original face winding and kind-color selector are known.
    pub fn shaded_preview(&self, faces: Vec<Vec<usize>>, color: u16) -> crate::formats::Shape {
        let mut shape = self.preview(faces);
        for face in &mut shape.faces {
            face.color = color;
            if face.vertices.len() >= 3 {
                let triangle = [2, 1, 0].map(|i| self.vertices[face.vertices[i]]);
                let plane = recovered::face_plane(triangle, color as i16);
                face.plane = Some(plane);
                face.normal = Some([0, 8, 2].map(|o| i16::from_be_bytes([plane[o], plane[o + 1]])));
            }
        }
        shape
    }

    /// CODE 18 +0x11fa with starting vertex offset zero.
    pub fn box_faces() -> Vec<Vec<usize>> {
        [
            [0, 4, 5, 1],
            [6, 7, 5, 4],
            [7, 3, 1, 5],
            [2, 0, 1, 3],
            [2, 6, 4, 0],
            [2, 3, 7, 6],
        ]
        .map(Vec::from)
        .to_vec()
    }

    /// CODE 19 +257e and +2024 select kind-primary colors for these builders.
    pub fn fixed_obstacle_color(selector: i16) -> Option<u16> {
        match selector {
            1 => Some(0),
            5 => Some(4),
            _ => None,
        }
    }

    /// Selectors 1 and 5. Returns original topology for the supplied display flag.
    /// Selector 5's ten-unit collision override belongs to the slot loader.
    pub fn fixed_obstacle(selector: i16, filled: bool) -> Option<(Self, Vec<Vec<usize>>)> {
        let arguments = match selector {
            1 => [0, 0, 18, 18, 18, 0, 9, 0],
            5 => [3, 3, 4, 24, 4, 0, 12, 0],
            _ => return None,
        }
        .map(|v| v << 16);
        let mut shape = Self::tapered_box(arguments);
        let mut faces = Self::box_faces();
        if selector == 5 {
            shape.vertices.extend(
                [
                    [15, 15, -5],
                    [15, 21, -5],
                    [-3, 33, -5],
                    [3, 33, -5],
                    [-15, 15, -5],
                    [-15, 21, -5],
                    [-3, 3, -5],
                    [3, 3, -5],
                    [0, 18, -5],
                ]
                .map(|v| v.map(|n| n * 8)),
            );
            if filled {
                for i in 0..4 {
                    faces.push(vec![8 + 2 * i, 9 + 2 * i, 16]);
                    faces.push(vec![9 + 2 * i, 8 + 2 * i, 16]);
                }
            } else {
                for i in 0..2 {
                    let other = (i + 2) % 4;
                    faces.extend([
                        vec![8 + 2 * i, 8 + 2 * other],
                        vec![9 + 2 * i, 9 + 2 * other],
                        vec![8 + 2 * i, 9 + 2 * i],
                        vec![8 + 2 * other, 9 + 2 * other],
                    ]);
                }
            }
            shape.dimensions = recovered::runtime_collision_dimensions(&shape.vertices);
            shape.candidate_radius = recovered::runtime_collision_radius(
                &shape.vertices,
                &mut recovered::candidate_fix_div,
            );
        }
        Some((shape, faces))
    }

    /// Selector 3, CODE 19 +0x3342..+0x33be: two octagonal rings.
    /// The supplied unit sine table retains the modeled-SANE limitation.
    pub fn octagonal_column(table: &[i32; 360]) -> Self {
        let mut vertices = Vec::with_capacity(16);
        for ring in 0..2 {
            for side in 0..8 {
                let degree = side * 45;
                let cosine = if degree <= 90 {
                    90 - degree
                } else {
                    450 - degree
                };
                vertices.push(recovered::candidate_runtime_vertex(
                    [
                        table[cosine].wrapping_mul(6),
                        ring * (27 << 16),
                        table[degree].wrapping_mul(6),
                    ],
                    65536,
                ));
            }
        }
        let dimensions = recovered::runtime_collision_dimensions(&vertices);
        Self {
            candidate_radius: recovered::runtime_collision_radius(
                &vertices,
                &mut recovered::candidate_fix_div,
            ),
            vertices,
            dimensions,
        }
    }

    /// Original polygon index lists at CODE 19 +0x3472..+0x35d2.
    pub fn column_faces() -> Vec<Vec<usize>> {
        let mut faces = vec![(0..8).collect(), (8..16).rev().collect()];
        faces.extend((0..8).map(|i| vec![(i + 1) % 8, i, i + 8, (i + 1) % 8 + 8]));
        faces
    }

    /// Selector 3 submits kind-two primary color for every face.
    pub fn column_preview(&self) -> crate::formats::Shape {
        self.shaded_preview(Self::column_faces(), 2)
    }

    pub fn preview(&self, faces: Vec<Vec<usize>>) -> crate::formats::Shape {
        crate::formats::Shape {
            packed_vertices: self.vertices.clone(),
            vertices: self
                .vertices
                .iter()
                .map(|v| crate::formats::Vertex {
                    x: v[0] as f32 / 8.,
                    y: v[1] as f32 / 8.,
                    z: v[2] as f32 / 8.,
                })
                .collect(),
            faces: faces
                .into_iter()
                .map(|vertices| crate::formats::Face {
                    vertices,
                    color: 0,
                    normal: None,
                    plane: None,
                })
                .collect(),
            radar_vertices: vec![],
            radar_lines: vec![],
            plane_sides: 0,
        }
    }

    /// CODE 1 +0x16c0..+0x1724 and CODE 12 vertex insertion/dimension helpers.
    /// Reads only the vertex prefix. The caller separately validates the full shape.
    /// Resource loading sets scale to one; insertion stops at 60 runtime vertices.
    pub fn from_resource_vertices(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data);
        let count = reader.zcount()?;
        let mut vertices = Vec::with_capacity(count.min(60));
        for index in 0..count {
            let vertex = [reader.i32()?, reader.i32()?, reader.i32()?];
            if index < 60 {
                vertices.push(recovered::candidate_runtime_vertex(vertex, 65536));
            }
        }
        let dimensions = recovered::runtime_collision_dimensions(&vertices);
        Ok(Self {
            candidate_radius: recovered::runtime_collision_radius(
                &vertices,
                &mut recovered::candidate_fix_div,
            ),
            vertices,
            dimensions,
        })
    }
}
