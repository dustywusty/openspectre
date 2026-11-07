//! Layouts verified against TMPL resources on the supplied CD.
use crate::{
    Result,
    binary::{Reader, text},
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
#[derive(Clone, Debug, Serialize)]
pub struct Face {
    pub vertices: Vec<usize>,
    pub color: u16,
    #[serde(skip)]
    pub normal: Option<[i16; 3]>,
    #[serde(skip)]
    pub plane: Option<[u8; 12]>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Shape {
    #[serde(skip)]
    pub packed_vertices: Vec<[i16; 3]>,
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Face>,
    pub radar_vertices: Vec<Vertex>,
    pub radar_lines: Vec<[usize; 2]>,
    /// First top-view vertices used for polygon fill; -1 selects line count, capped at eight.
    pub plane_sides: i16,
}
fn vertices(r: &mut Reader) -> Result<Vec<Vertex>> {
    let count = r.zcount()?;
    if count > 8192 {
        return Err("Too many vertices".into());
    }
    (0..count)
        .map(|_| {
            Ok(Vertex {
                x: r.i32()? as f32 / 65536.,
                y: r.i32()? as f32 / 65536.,
                z: r.i32()? as f32 / 65536.,
            })
        })
        .collect()
}
fn vertex_index(r: &mut Reader, count: usize) -> Result<usize> {
    let index = r.u16()? as usize;
    if index == 0 || index > count {
        return Err(format!("Vertex {index} outside 1..={count}"));
    }
    Ok(index - 1)
}
impl Shape {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let vertices = vertices(&mut r)?;
        let packed = crate::model::PackedShape::from_resource_vertices(data)?;
        let count = r.zcount()?;
        if count > 8192 {
            return Err("Too many faces".into());
        }
        let mut faces = Vec::new();
        for _ in 0..count {
            let count = r.zcount()?;
            if count > 8192 {
                return Err("Too many face vertices".into());
            }
            let indices = (0..count)
                .map(|_| vertex_index(&mut r, vertices.len()))
                .collect::<Result<Vec<_>>>()?;
            let color = r.u16()?;
            let plane = indices.get(..3).and_then(|indices| {
                let triangle = [
                    *packed.vertices.get(indices[2])?,
                    *packed.vertices.get(indices[1])?,
                    *packed.vertices.get(indices[0])?,
                ];
                Some(crate::recovered::face_plane(triangle, color as i16))
            });
            faces.push(Face {
                vertices: indices,
                color,
                normal: plane.map(|p| [0, 8, 2].map(|o| i16::from_be_bytes([p[o], p[o + 1]]))),
                plane,
            });
        }
        let radar_vertices = self::vertices(&mut r)?;
        let count = r.zcount()?;
        let radar_lines = (0..count)
            .map(|_| {
                Ok([
                    vertex_index(&mut r, radar_vertices.len())?,
                    vertex_index(&mut r, radar_vertices.len())?,
                ])
            })
            .collect::<Result<Vec<_>>>()?;
        let plane_sides = r.i16()?;
        r.finish()?;
        Ok(Self {
            packed_vertices: packed.vertices,
            vertices,
            faces,
            radar_vertices,
            radar_lines,
            plane_sides,
        })
    }
    /// Ordinary resource-tail path; compatibility overrides belong to the loader.
    pub fn top_view_fill_count(&self) -> usize {
        let Ok(lines) = i16::try_from(self.radar_lines.len()) else {
            return 0;
        };
        let count =
            crate::recovered::resource_plane_sides(self.plane_sides, lines.wrapping_sub(1), 0, 0).0;
        usize::try_from(count)
            .ok()
            .filter(|&n| n <= self.radar_vertices.len())
            .unwrap_or(0)
    }

    pub fn obj(&self) -> String {
        let mut out = String::from("# Spectre VR model extracted from a shap resource\n");
        for v in &self.vertices {
            out += &format!("v {} {} {}\n", v.x, v.y, v.z);
        }
        for f in &self.faces {
            if f.vertices.len() >= 3 {
                out += "f";
                for i in &f.vertices {
                    out += &format!(" {}", i + 1);
                }
                out += "\n";
            }
        }
        out
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Obstacle {
    pub kind: i16,
    pub v: i16,
    pub h: i16,
    pub facing: i16,
}
#[derive(Clone, Debug, Serialize)]
pub struct World {
    pub obstacles: Vec<Obstacle>,
}
impl World {
    /// Original leading-slot flag refresh for a supplied level. This preview
    /// excludes procedural worlds, optional extra objects, and campaign selection.
    pub fn with_level_flags(&self, level: i16) -> Result<Self> {
        if level < 1 || !(6..=50).contains(&self.obstacles.len()) {
            return Err("Flag layout preview needs level >= 1 and 6..50 authored obstacles".into());
        }
        let mut world = self.clone();
        let mut kinds: Vec<u8> = world
            .obstacles
            .iter()
            .map(|o| {
                let kind = o.kind as i8;
                if (0..=46).contains(&kind) {
                    kind as u8
                } else {
                    14
                }
            })
            .collect();
        crate::recovered::refresh_flag_slots(level, kinds.len() as i16, &mut kinds)
            .ok_or("Flag slots exceed authored layout")?;
        for (obstacle, kind) in world.obstacles.iter_mut().zip(kinds) {
            obstacle.kind = kind as i16;
        }
        Ok(world)
    }
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let count = r.u16()? as usize;
        let obstacles = (0..count)
            .map(|_| {
                Ok(Obstacle {
                    kind: r.i16()?,
                    v: r.i16()?,
                    h: r.i16()?,
                    facing: r.i16()?,
                })
            })
            .collect::<Result<_>>()?;
        r.finish()?;
        Ok(Self { obstacles })
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Wave {
    pub world: i16,
    pub enemies: i16,
    pub hint: i16,
    pub beasties: i16,
    pub start_movie: i16,
    pub end_movie: i16,
    pub song: i16,
    pub colors: i16,
}
pub fn waves(data: &[u8]) -> Result<Vec<Wave>> {
    let mut r = Reader::new(data);
    let n = r.u16()?;
    let result = (0..n)
        .map(|_| {
            Ok(Wave {
                world: r.i16()?,
                enemies: r.i16()?,
                hint: r.i16()?,
                beasties: r.i16()?,
                start_movie: r.i16()?,
                end_movie: r.i16()?,
                song: r.i16()?,
                colors: r.i16()?,
            })
        })
        .collect::<Result<_>>()?;
    r.finish()?;
    Ok(result)
}
#[derive(Clone, Debug, Serialize)]
pub struct Spawn {
    pub kind: i16,
    pub minimum: i16,
    pub maximum: i16,
    pub rebirths: i16,
}
pub fn enemies(data: &[u8]) -> Result<Vec<Spawn>> {
    let mut r = Reader::new(data);
    let n = r.u16()?;
    let result = (0..n)
        .map(|_| {
            Ok(Spawn {
                kind: r.i16()?,
                minimum: r.i16()?,
                maximum: r.i16()?,
                rebirths: r.i16()?,
            })
        })
        .collect::<Result<_>>()?;
    r.finish()?;
    Ok(result)
}
#[derive(Clone, Debug, Serialize)]
pub struct Beastie {
    pub kind: i16,
    pub hits_to_kill: i16,
    pub collision_damage: i16,
    pub attack_range: i16,
    pub short_firing_range: i16,
    pub firing_arc: i16,
    pub firing_interval: i16,
    pub radial_velocity: i16,
    pub angular_velocity: i16,
    pub score: i32,
    pub bonus: i32,
    pub weapon: i16,
}
pub fn beasties(data: &[u8]) -> Result<Vec<Beastie>> {
    let mut r = Reader::new(data);
    let n = r.u16()?;
    let result = (0..n)
        .map(|_| {
            Ok(Beastie {
                kind: r.i16()?,
                hits_to_kill: r.i16()?,
                collision_damage: r.i16()?,
                attack_range: r.i16()?,
                short_firing_range: r.i16()?,
                firing_arc: r.i16()?,
                firing_interval: r.i16()?,
                radial_velocity: r.i16()?,
                angular_velocity: r.i16()?,
                score: r.i32()?,
                bonus: r.i32()?,
                weapon: r.i16()?,
            })
        })
        .collect::<Result<_>>()?;
    r.finish()?;
    Ok(result)
}
#[derive(Clone, Debug, Serialize)]
pub struct Weapon {
    pub kind: i16,
    pub damage: i16,
    pub ammo_cost: i16,
}
pub fn weapons(data: &[u8]) -> Result<Vec<Weapon>> {
    let mut r = Reader::new(data);
    let n = r.u16()?;
    let result = (0..n)
        .map(|_| {
            Ok(Weapon {
                kind: r.i16()?,
                damage: r.i16()?,
                ammo_cost: r.i16()?,
            })
        })
        .collect::<Result<_>>()?;
    r.finish()?;
    Ok(result)
}
#[derive(Clone, Debug, Serialize)]
pub struct TemplateField {
    pub label: String,
    pub kind: String,
}
pub fn template(data: &[u8]) -> Result<Vec<TemplateField>> {
    let mut r = Reader::new(data);
    let mut fields = Vec::new();
    while r.pos < data.len() {
        let n = r.take(1)?[0] as usize;
        let label = text(r.take(n)?);
        let kind = text(r.take(4)?);
        fields.push(TemplateField { label, kind });
    }
    Ok(fields)
}

/// The sstf resource's ten extra-life thresholds and signed executable increment.
#[derive(Clone, Debug, Serialize)]
pub struct ScoreRules {
    pub thresholds: [i32; 10],
    pub repeat_increment: i16,
}
impl ScoreRules {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        r.take(14)?;
        let mut thresholds = [0; 10];
        for value in &mut thresholds {
            *value = r.i32()?;
        }
        Ok(Self {
            thresholds,
            repeat_increment: r.i16()?,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn score_rules_reject_truncation_and_preserve_signed_increment() {
        let mut bytes = vec![0; 56];
        bytes[14..18].copy_from_slice(&5000_i32.to_be_bytes());
        bytes[54..56].copy_from_slice(&40000_u16.to_be_bytes());
        for end in 0..56 {
            assert!(ScoreRules::parse(&bytes[..end]).is_err());
        }
        let rules = ScoreRules::parse(&bytes).unwrap();
        assert_eq!(rules.thresholds[0], 5000);
        assert_eq!(rules.repeat_increment, -25536);
    }
    #[test]
    fn flag_layout_preserves_authored_positions_and_only_converts_eligible_slots() {
        let world = World {
            obstacles: (0..12)
                .map(|i| Obstacle {
                    kind: if i == 2 { 15 } else { 14 },
                    h: i * 31,
                    v: i * 17,
                    facing: i * 9,
                })
                .collect(),
        };
        let level1 = world.with_level_flags(1).unwrap();
        assert_eq!(level1.obstacles.iter().filter(|o| o.kind == 12).count(), 2);
        assert_eq!(world.obstacles[0].kind, 14, "source layout is unchanged");
        for (original, prepared) in world.obstacles.iter().zip(&level1.obstacles) {
            assert_eq!(
                (original.h, original.v, original.facing),
                (prepared.h, prepared.v, prepared.facing)
            );
        }
        let later = world.with_level_flags(40).unwrap();
        assert_eq!(
            later.obstacles.iter().filter(|o| o.kind == 12).count(),
            5,
            "object count minus six limits eligible slots"
        );
        assert!(world.with_level_flags(0).is_err());
    }

    use super::*;
    fn triangle() -> Vec<u8> {
        let mut out = vec![0, 2];
        for n in [-65536i32, 0, 0, 65536, 0, 0, 0, 65536, 0] {
            out.extend(n.to_be_bytes());
        }
        for n in [0u16, 2, 1, 2, 3, 255, 0xffff, 0xffff, 0xffff] {
            out.extend(n.to_be_bytes());
        }
        out
    }
    #[test]
    fn signed_fixed_and_one_based_indices() {
        let s = Shape::parse(&triangle()).unwrap();
        assert_eq!(s.vertices[0].x, -1.);
        assert_eq!(s.faces[0].vertices, [0, 1, 2]);
        assert_eq!(s.plane_sides, -1);
        assert!(s.radar_vertices.is_empty());
        assert!(s.obj().contains("f 1 2 3\n"));
    }
    #[test]
    fn every_truncated_shape_is_rejected() {
        let data = triangle();
        for n in 0..data.len() {
            assert!(Shape::parse(&data[..n]).is_err(), "length {n}");
        }
        let mut bad = data.clone();
        bad[43] = 0;
        assert!(Shape::parse(&bad).is_err());
        let mut extra = data;
        extra.push(0);
        assert!(Shape::parse(&extra).is_err());
    }
    #[test]
    fn world_count_is_not_minus_one() {
        let w = World::parse(&[0, 1, 0, 14, 0, 16, 3, 240, 0, 90]).unwrap();
        assert_eq!(w.obstacles.len(), 1);
        assert_eq!(w.obstacles[0].h, 1008);
        assert!(World::parse(&[0, 1]).is_err());
    }
}
