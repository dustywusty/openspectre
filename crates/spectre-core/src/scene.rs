//! Original shape-selection table, CODE 1 +0x1206.
//! Column 2 is the branch used when A5-0x7be2 is zero.
//! Display-flag semantics are not fully recovered; unsupported generated shapes
//! remain explicit diagnostic markers rather than guessed original models.
use crate::{
    Result,
    binary::Reader,
    formats::{Shape, World},
    resource::Archive,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Catalog {
    pub palettes: BTreeMap<i16, Vec<u8>>,
    pub color_bindings: Option<crate::recovered::ColorBindings>,
    pub radar_glyphs: Option<Vec<Vec<[i8; 2]>>>,
    pub choices: Vec<[i16; 3]>,
    pub shapes: BTreeMap<i16, Shape>,
    pub packed_shapes: BTreeMap<i16, crate::model::PackedShape>,
    pub animations: BTreeMap<i16, Vec<i16>>,
    pub waves: BTreeMap<i16, Vec<crate::formats::Wave>>,
    pub enemies: BTreeMap<i16, Vec<crate::formats::Spawn>>,
    pub values: BTreeMap<i16, Vec<crate::formats::Beastie>>,
    pub weapons: Vec<crate::formats::Weapon>,
    pub score_rules: BTreeMap<i16, crate::formats::ScoreRules>,
}
pub(crate) struct TargetScenario {
    pub resource: i16,
    pub index: usize,
    pub level: i16,
    pub difficulty: i16,
    pub health_modifier: i16,
    pub mission: bool,
}
#[derive(Clone, Serialize)]
pub struct Item {
    pub kind: i16,
    pub shape_id: i16,
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub source: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
    #[serde(skip)]
    pub shape: Option<Shape>,
}
impl Catalog {
    pub fn load(archive: &Archive) -> Result<Self> {
        let mut catalog = Self::default();
        for resource in &archive.resources {
            if resource.kind == "clut" {
                catalog.palettes.insert(resource.id, resource.data.clone());
            } else if resource.kind == "cob8" && resource.id == 999 {
                catalog.color_bindings = crate::recovered::color_bindings(&resource.data);
            } else if resource.kind == "PICT" && resource.id == 1000 {
                catalog.radar_glyphs = Some(
                    crate::radar::glyphs_from_pict(&resource.data)
                        .ok_or("Unsupported or malformed radar PICT 1000")?,
                );
            } else if resource.kind == "sstf" {
                catalog.score_rules.insert(
                    resource.id,
                    crate::formats::ScoreRules::parse(&resource.data)?,
                );
            } else if resource.kind == "wval" && resource.id == 10000 {
                catalog.weapons = crate::formats::weapons(&resource.data)?;
            } else if resource.kind == "wave" {
                catalog
                    .waves
                    .insert(resource.id, crate::formats::waves(&resource.data)?);
            } else if resource.kind == "enem" {
                catalog
                    .enemies
                    .insert(resource.id, crate::formats::enemies(&resource.data)?);
            } else if resource.kind == "bval" {
                catalog
                    .values
                    .insert(resource.id, crate::formats::beasties(&resource.data)?);
            } else if resource.kind == "shap" {
                catalog
                    .shapes
                    .insert(resource.id, Shape::parse(&resource.data)?);
                catalog.packed_shapes.insert(
                    resource.id,
                    crate::model::PackedShape::from_resource_vertices(&resource.data)?,
                );
            } else if resource.kind == "ashp" {
                // CODE 1 +0x15de..0x15f0: count word, then ordered frame IDs.
                let mut r = Reader::new(&resource.data);
                let count = r.u16()? as usize;
                let frames = (0..count).map(|_| r.i16()).collect::<Result<Vec<_>>>()?;
                r.finish()?;
                catalog.animations.insert(resource.id, frames);
            } else if resource.kind == "shps" && resource.id == 10000 {
                let mut r = Reader::new(&resource.data);
                let count = r.u16()? as usize;
                catalog.choices = (0..count)
                    .map(|_| Ok([r.i16()?, r.i16()?, r.i16()?]))
                    .collect::<Result<_>>()?;
                r.finish()?;
            }
        }
        Ok(catalog)
    }
    /// Earliest resource/sequence entry referencing this map. This is an explicit
    /// preview scenario, not a reconstruction of campaign progression.
    pub fn populate_targets(
        &self,
        world_id: i16,
        game: &mut crate::game::Game,
        mission: bool,
    ) -> Result<()> {
        let (damage, ammo) = crate::recovered::weapon_tables(&self.weapons);
        game.primary_damage = damage[0];
        game.primary_ammo_cost = ammo[0];
        game.weapon_ammo_costs = ammo;
        game.weapon_damage = damage;
        let Some((sequence, wave_index, wave)) = self.waves.iter().find_map(|(id, waves)| {
            waves
                .iter()
                .enumerate()
                .find(|(_, w)| w.world == world_id)
                .map(|(i, w)| (*id, i, w))
        }) else {
            return Ok(());
        };
        self.populate_wave_targets(
            wave,
            game,
            &TargetScenario {
                resource: sequence,
                index: wave_index,
                level: 1,
                difficulty: 0,
                health_modifier: 0,
                mission,
            },
        )
    }
    pub(crate) fn populate_wave_targets(
        &self,
        wave: &crate::formats::Wave,
        game: &mut crate::game::Game,
        scenario: &TargetScenario,
    ) -> Result<()> {
        let TargetScenario {
            resource: sequence,
            index: wave_index,
            level,
            difficulty,
            health_modifier,
            mission,
        } = *scenario;
        game.palette = self.palettes.get(&wave.colors).and_then(|base| {
            crate::recovered::compose_resource_palette(
                base,
                level,
                8,
                13,
                self.palettes.get(&2000).map(Vec::as_slice),
                self.palettes.get(&2001).map(Vec::as_slice),
            )
        });
        game.color_bindings = self.color_bindings.clone();
        game.radar_glyphs = self.radar_glyphs.clone();
        game.ground_color_index = self
            .color_bindings
            .as_ref()
            .map(|bindings| bindings.special[0]);
        let (damage, ammo) = crate::recovered::weapon_tables(&self.weapons);
        game.primary_damage = damage[0];
        game.primary_ammo_cost = ammo[0];
        game.weapon_ammo_costs = ammo;
        game.weapon_damage = damage;
        let spawns = self
            .enemies
            .get(&wave.enemies)
            .ok_or("Referenced enemy table missing")?;
        let values = self
            .values
            .get(&wave.beasties)
            .ok_or("Referenced enemy parameters missing")?;
        game.scenario = format!(
            "wave {sequence} / entry {} · enem {} · bval {} · minimum counts",
            wave_index + 1,
            wave.enemies,
            wave.beasties
        );
        // Until original Random generation is recovered, select the minimum
        // branch explicitly. Level and difficulty supply the initial effective value.
        let plan = if mission {
            Some(
                crate::recovered::enemy_plan(
                    spawns,
                    crate::recovered::initial_effective_difficulty(difficulty, level),
                    false,
                    &vec![0; spawns.len()],
                )
                .ok_or("Invalid mission enemy plan")?,
            )
        } else {
            None
        };
        // The special +50 override is disabled in normal campaign previews.
        let parameter_slots = crate::recovered::enemy_parameters(values, health_modifier, false);
        game.dump_weights = crate::recovered::dump_weights(values);
        // CODE 7 +10e4 reads +14a = collision-damage table +11c + 23*2.
        game.acid_damage = parameter_slots[23].collision_damage;
        let mut populated = [false; 47];
        if mission {
            game.scenario += " · recovered caps/lives, Random=0";
        }
        let mut spawn_rows: Vec<_> = spawns.iter().collect();
        if mission {
            spawn_rows.sort_by_key(|row| row.kind as i8);
        }
        for spawn in spawn_rows {
            let kind = spawn.kind as i8;
            if !(0..47).contains(&kind) {
                continue;
            }
            if mission && populated[kind as usize] {
                continue;
            }
            populated[kind as usize] = true;
            let count = plan
                .as_ref()
                .map_or(spawn.minimum, |p| p.counts[kind as usize]);
            if spawn.minimum < 0 || spawn.minimum > 128 {
                return Err("Unsupported enemy count".into());
            }
            let parameters = &parameter_slots[kind as usize];
            if !values.iter().any(|v| v.kind as i8 == kind && kind != 5) {
                game.scenario += &format!(" · type {kind} original default parameters");
            }
            for _ in 0..count {
                let world = World {
                    obstacles: vec![crate::formats::Obstacle {
                        kind: spawn.kind,
                        h: 0,
                        v: 0,
                        facing: 180,
                    }],
                };
                let Some(item) = self.instantiate(&world).pop() else {
                    continue;
                };
                game.add_target(item, parameters.hits_to_kill.max(1))?;
                let target = game.targets.last_mut().unwrap();
                target.kill_score = parameters.score;
                target.fire = crate::game::EnemyFireState::from_parameters(parameters);
                target.motion = crate::game::EnemyMotionParameters::from_parameters(parameters);
                target.navigation.attack_range = (parameters.attack_range as i32) << 16;
                target.navigation.effective_difficulty =
                    crate::recovered::initial_effective_difficulty(difficulty, level);
                target.lives = plan.as_ref().map_or(1, |p| p.lives[kind as usize]);
            }
        }
        if let Some(plan) = &plan {
            // Keep the original immutable tables: depleted participant lives are
            // not the input to the last-enemy award.
            if game.targets.len() == plan.total as usize {
                let bonuses = std::array::from_fn(|kind| parameter_slots[kind].bonus);
                game.par_seconds = Some(crate::recovered::arena_par_seconds(
                    &plan.counts,
                    &plan.lives,
                    &bonuses,
                    game.items.iter().filter(|i| i.kind == 12).count() as i8,
                ));
                game.enemy_clear_bonus = Some(crate::recovered::last_enemy_bonus(
                    level,
                    &plan.counts,
                    &plan.lives,
                ));
            } else {
                game.scenario += " · completion ratings unavailable: incomplete enemy plan";
            }
        }
        if let Some(ids) = self
            .choices
            .get(31)
            .and_then(|choice| self.animations.get(&choice[2]))
        {
            let frames: Option<Vec<Shape>> =
                ids.iter().map(|id| self.shapes.get(id).cloned()).collect();
            if let Some(frames) = frames {
                game.configure_type31(frames);
            }
        }
        game.capture_targets();
        Ok(())
    }
    pub fn instantiate(&self, world: &World) -> Vec<Item> {
        world
            .obstacles
            .iter()
            .filter_map(|o| {
                let id = self
                    .choices
                    .get(o.kind as usize)
                    .map(|c| c[2])
                    .unwrap_or(-1);
                if id == 0 {
                    return None;
                }
                // Preserve the selector ID. Only the first original animation
                // frame is shown until runtime frame selection is recovered.
                let frame = self.animations.get(&id).and_then(|frames| frames.first());
                let shape = self
                    .shapes
                    .get(&id)
                    .cloned()
                    .or_else(|| procedural(id))
                    .or_else(|| {
                        frame.and_then(|f| self.shapes.get(f).cloned().or_else(|| procedural(*f)))
                    });
                let source = if self.shapes.contains_key(&id) {
                    "original resource"
                } else if frame.is_some() && shape.is_some() {
                    "original animation, frozen first frame"
                } else if shape.is_some() {
                    "recovered procedural vertices"
                } else {
                    "unresolved shape"
                };
                let mut min = [-12., 0., -12.];
                let mut max = [12., 24., 12.];
                if let Some(s) = &shape {
                    min = [f32::INFINITY; 3];
                    max = [f32::NEG_INFINITY; 3];
                    for v in &s.vertices {
                        for (i, n) in [v.x, v.y, v.z].iter().enumerate() {
                            min[i] = min[i].min(*n);
                            max[i] = max[i].max(*n);
                        }
                    }
                    if s.vertices.is_empty() {
                        min = [-12., 0., -12.];
                        max = [12., 24., 12.];
                    }
                }
                let pose = crate::recovered::world_placement(o.h, o.v, o.facing, 1024).unwrap();
                Some(Item {
                    kind: o.kind,
                    shape_id: id,
                    x: pose[0] as f32 / 65536.,
                    z: pose[1] as f32 / 65536.,
                    heading: (pose[2] as f32 / 65536.).to_radians(),
                    source: source.into(),
                    min,
                    max,
                    shape,
                })
            })
            .collect()
    }
}
fn procedural(id: i16) -> Option<Shape> {
    if id == 6 {
        return Some(
            crate::model::PackedShape::ammo_dump()
                .shaded_preview(vec![vec![1, 2, 3, 0], vec![0, 3, 2, 1]], 13),
        );
    }
    if id == 12 {
        let (model, faces) = crate::model::PackedShape::flag(true);
        return Some(model.shaded_preview(faces, 12));
    }
    if let Some((model, faces)) = crate::model::PackedShape::fixed_obstacle(id, false) {
        return Some(
            model.shaded_preview(faces, crate::model::PackedShape::fixed_obstacle_color(id)?),
        );
    }
    if id == 3 {
        return Some(
            crate::model::PackedShape::octagonal_column(&crate::recovered::candidate_unit_table())
                .column_preview(),
        );
    }
    if id == 2 {
        return Some(
            crate::model::PackedShape::tetrahedron()
                .shaded_preview(crate::model::PackedShape::tetrahedron_faces(), 1),
        );
    }
    None
}

impl Item {
    /// Scenery stores authored degrees; participants store 90-minus-native degrees.
    pub fn native_heading(&self, participant: bool) -> i32 {
        let degrees = if participant {
            90. - self.heading.to_degrees()
        } else {
            self.heading.to_degrees().round()
        };
        crate::recovered::wrap_angle((degrees * 65536.).round() as i32)
    }
    /// Floating scene rotation corresponding to the native object heading.
    pub fn scene_heading(&self, participant: bool) -> f32 {
        (90. - self.native_heading(participant) as f32 / 65536.).to_radians()
    }
    /// Provisional collision using oriented bounds of the rendered model.
    pub fn contains(&self, x: f32, z: f32, radius: f32, participant: bool) -> bool {
        let (s, c) = self.scene_heading(participant).sin_cos();
        let dx = x - self.x;
        let dz = z - self.z;
        let local_x = dx * c - dz * s;
        let local_z = dx * s + dz * c;
        let closest_x = local_x.clamp(self.min[0], self.max[0]);
        let closest_z = local_z.clamp(self.min[2], self.max[2]);
        (local_x - closest_x).powi(2) + (local_z - closest_z).powi(2) <= radius * radius
    }
}

/// Convex ground footprint of visible vertices. This is preview collision,
/// not the original fixed-point collision query.
pub fn convex_footprint(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let mut hull = Vec::new();
    for &p in &points {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0. {
            hull.pop();
        }
        hull.push(p);
    }
    let lower = hull.len();
    for &p in points.iter().rev().skip(1) {
        while hull.len() > lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0. {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}

pub fn footprints_overlap(a: &[[f32; 2]], b: &[[f32; 2]]) -> bool {
    if a.is_empty() || b.is_empty() || (a.len() < 3 && b.len() < 3) {
        return false;
    }
    for polygon in [a, b] {
        for i in 0..polygon.len() {
            let p = polygon[i];
            let q = polygon[(i + 1) % polygon.len()];
            let axis = [p[1] - q[1], q[0] - p[0]];
            let interval = |vertices: &[[f32; 2]]| {
                vertices
                    .iter()
                    .fold([f32::INFINITY, f32::NEG_INFINITY], |r, v| {
                        let d = v[0] * axis[0] + v[1] * axis[1];
                        [r[0].min(d), r[1].max(d)]
                    })
            };
            let x = interval(a);
            let y = interval(b);
            if x[1] < y[0] || y[1] < x[0] {
                return false;
            }
        }
    }
    true
}

impl Item {
    pub fn footprint(&self, participant: bool) -> Vec<[f32; 2]> {
        let points = self
            .shape
            .as_ref()
            .filter(|s| s.vertices.len() >= 3)
            .map(|s| s.vertices.iter().map(|v| [v.x, v.z]).collect())
            .unwrap_or_else(|| {
                vec![
                    [self.min[0], self.min[2]],
                    [self.max[0], self.min[2]],
                    [self.max[0], self.max[2]],
                    [self.min[0], self.max[2]],
                ]
            });
        let (s, c) = self.scene_heading(participant).sin_cos();
        convex_footprint(points)
            .into_iter()
            .map(|p| [self.x + p[0] * c + p[1] * s, self.z - p[0] * s + p[1] * c])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_flag_footprint_intersects_tank_polygon() {
        let tank = [[-9., -17.], [9., -17.], [9., 22.], [-9., 22.]];
        assert!(footprints_overlap(&tank, &[[0., 0.], [7., 0.]]));
        assert!(footprints_overlap(&[[0., 0.], [7., 0.]], &tank));
        assert!(!footprints_overlap(&tank, &[[10., 0.], [17., 0.]]));
        assert!(!footprints_overlap(&tank, &[[0., 30.], [7., 30.]]));
    }
    #[test]
    fn original_table_selects_shapes_and_omits_zero() {
        use crate::formats::Obstacle;
        let c = Catalog {
            choices: vec![[1, 1, 2], [0, 0, 0]],
            ..Catalog::default()
        };
        let w = World {
            obstacles: vec![
                Obstacle {
                    kind: 0,
                    h: 100,
                    v: 200,
                    facing: 90,
                },
                Obstacle {
                    kind: 1,
                    h: 0,
                    v: 0,
                    facing: 0,
                },
            ],
        };
        let items = c.instantiate(&w);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].shape_id, 2);
        assert_eq!(items[0].shape.as_ref().unwrap().vertices.len(), 4);
        assert!(items[0].contains(100., 200., 1., false));
        assert!(!items[0].contains(150., 200., 1., false));
    }
}
