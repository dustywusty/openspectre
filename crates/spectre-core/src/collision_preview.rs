//! Adapter from preview entities to the recovered projectile query.
//! Entity order, ownership, spawn, and record lifecycle remain preview choices.
use crate::{
    game::Game,
    recovered::{self, CollisionQueryWorld},
};
pub struct Query {
    records: Vec<[u8; 128]>,
    grid: [Vec<i16>; 256],
    slots: [i8; 48],
    dimensions: Vec<[i32; 2]>,
    radii: Vec<i16>,
    directions: [[i16; 2]; 360],
    shot_id: i16,
    pub target_start: usize,
}
fn record(position: [i32; 2], heading: i32, kind: u8, owner: u8, height: i32) -> [u8; 128] {
    let mut out = [0; 128];
    for (i, v) in [position[0], position[1], heading].iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out[0x14] = kind;
    out[0x1a] = owner;
    out[0x28..0x2c].copy_from_slice(&height.to_be_bytes());
    out
}
impl Query {
    pub fn from_game(game: &Game, library: &crate::model_slots::ModelSlots) -> Option<Self> {
        let mut q = Self {
            records: vec![],
            grid: std::array::from_fn(|_| vec![]),
            slots: [-1; 48],
            dimensions: vec![[0; 2]; 100],
            radii: vec![0; 100],
            directions: recovered::candidate_collision_table(),
            shot_id: 0,
            target_start: game.items.len() + 2,
        };
        q.slots[..47].copy_from_slice(&library.state.current);
        for (&slot, model) in &library.geometry {
            *q.dimensions.get_mut(usize::try_from(slot - 1).ok()?)? = model.dimensions;
            *q.radii.get_mut(usize::try_from(slot - 1).ok()?)? = model.candidate_radius;
        }
        let heading = |r: f32| ((90. - r.to_degrees()).rem_euclid(360.) * 65536.).round() as i32;
        q.push(
            record(
                [(game.tank.x * 65536.) as i32, (game.tank.z * 65536.) as i32],
                heading(game.tank.heading),
                6,
                1,
                game.vertical.vertical,
            ),
            true,
        )?;
        for item in &game.items {
            q.push(
                record(
                    [(item.x * 65536.) as i32, (item.z * 65536.) as i32],
                    item.native_heading(false),
                    item.kind as u8,
                    0,
                    0,
                ),
                true,
            )?;
        }
        for target in &game.targets {
            let item = &target.item;
            if item.kind == 31 {
                let slot = library.state.first[31] as i16 + target.animation_frame as i16;
                if !library.geometry.contains_key(&slot) {
                    return None;
                }
                q.slots[31] = slot as i8;
            }
            q.push(
                record(
                    [(item.x * 65536.) as i32, (item.z * 65536.) as i32],
                    item.native_heading(true),
                    item.kind as u8,
                    0,
                    0,
                ),
                target.health > 0,
            )?;
        }
        for pickup in &game.pickups {
            q.push(pickup.record, true)?;
        }
        q.shot_id = i16::try_from(q.records.len() + 1).ok()?;
        q.records.push([0; 128]);
        Some(q)
    }
    fn push(&mut self, r: [u8; 128], active: bool) -> Option<()> {
        let id = i16::try_from(self.records.len() + 1).ok()?;
        if active {
            let position = std::array::from_fn(|i| {
                i32::from_be_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
            });
            let [x, z] = recovered::object_grid_cell(position);
            if self.grid[x * 16 + z].len() >= 20 {
                return None;
            }
            self.grid[x * 16 + z].push(id);
        }
        self.records.push(r);
        Some(())
    }
    /// The adapter updates cells from current poses. Original cached-cell update
    /// scheduling and record order remain preview choices.
    pub fn surface_contact(&self) -> Option<Option<(u8, i32, i16)>> {
        let r = &self.records[0];
        let position =
            std::array::from_fn(|i| i32::from_be_bytes(r[i * 4..i * 4 + 4].try_into().unwrap()));
        let cell = recovered::object_grid_cell(position).map(|v| v as i16);
        let world = CollisionQueryWorld {
            records: &self.records,
            participant_states: &[0; 12],
            grid: &self.grid,
            model_slots: &self.slots,
            dimensions: &self.dimensions,
            directions: &self.directions,
        };
        let selected = world.participant_contacts(1, 1, cell, &self.radii, 1024)?[0];
        Some(selected.filter(|id| *id > 0).map(|id| {
            let r = &self.records[id as usize - 1];
            (
                r[0x14],
                i32::from_be_bytes(r[8..12].try_into().unwrap()),
                id,
            )
        }))
    }
    /// Proposed movement pose using recovered overlap and vertical gates.
    /// Passable-kind filtering and rejection response remain adapter policy.
    pub fn player_movement_blocked(&self, pose: [i32; 3], height: i32) -> Option<bool> {
        Some(self.player_movement_contact(pose, height)?.is_some())
    }

    /// Selected movement contact: original identity, kind, and raw height.
    pub fn player_movement_contact(
        &self,
        pose: [i32; 3],
        height: i32,
    ) -> Option<Option<(i16, u8, i32)>> {
        let mut records = self.records.clone();
        records[0] = record([pose[0], pose[1]], pose[2], 6, 1, height);
        let grid = self.grid.clone().map(|cell| {
            cell.into_iter()
                .filter(|id| {
                    !matches!(
                        records[*id as usize - 1][0x14],
                        5 | 12 | 16 | 17 | 18 | 23 | 24 | 36 | 37 | 39
                    )
                })
                .collect()
        });
        let world = CollisionQueryWorld {
            records: &records,
            participant_states: &[0; 12],
            grid: &grid,
            model_slots: &self.slots,
            dimensions: &self.dimensions,
            directions: &self.directions,
        };
        let cell = recovered::object_grid_cell([pose[0], pose[1]]).map(|v| v as i16);
        Some(
            world.participant_contacts(1, 1, cell, &self.radii, 1024)?[0].map(|id| {
                if id == -999 {
                    return (id, 0, 0);
                }
                let record = &records[id as usize - 1];
                (
                    id,
                    record[0x14],
                    i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()),
                )
            }),
        )
    }

    /// Placement tests use the proposed player pose and a recomputed grid cell.
    /// The player's old grid entry is harmless: the query excludes its owner.
    pub fn player_spawn_blocked(&mut self, pose: [i32; 3]) -> Option<bool> {
        self.entity_spawn_blocked(pose, 1, 6, 1)
    }
    pub fn entity_spawn_blocked(
        &mut self,
        pose: [i32; 3],
        id: i16,
        kind: u8,
        owner: i8,
    ) -> Option<bool> {
        let index = usize::try_from(id.checked_sub(1)?).ok()?;
        *self.records.get_mut(index)? = record([pose[0], pose[1]], pose[2], kind, owner as u8, 0);
        let cell = recovered::object_grid_cell([pose[0], pose[1]]).map(|v| v as i16);
        let world = CollisionQueryWorld {
            records: &self.records,
            participant_states: &[0; 12],
            grid: &self.grid,
            model_slots: &self.slots,
            dimensions: &self.dimensions,
            directions: &self.directions,
        };
        Some(world.participant_contacts(owner, id, cell, &self.radii, 1024)?[0].is_some())
    }
    pub fn enemy_clearance(
        &self,
        target: usize,
        heading: i32,
        dimensions: [i32; 2],
    ) -> Option<bool> {
        let id = i16::try_from(self.target_start.checked_add(target)?).ok()?;
        let world = CollisionQueryWorld {
            records: &self.records,
            participant_states: &[0; 12],
            grid: &self.grid,
            model_slots: &self.slots,
            dimensions: &self.dimensions,
            directions: &self.directions,
        };
        world.ai_clearance(id, heading, dimensions, 1024)
    }

    /// Geometry kind is 9 for cannon/Seekers and 34 for Smart Missiles.
    /// Height is the raw original fixed-point value, not rendered scene height.
    pub fn hit_projectile(
        &mut self,
        start: [i32; 2],
        end: [i32; 2],
        heading: i32,
        elapsed: i32,
        height: i32,
        kind: u8,
    ) -> Option<i16> {
        if !matches!(kind, 9 | 34) {
            return None;
        }
        self.records[self.shot_id as usize - 1] = record(start, heading, kind, 1, height);
        let world = CollisionQueryWorld {
            records: &self.records,
            participant_states: &[0; 12],
            grid: &self.grid,
            model_slots: &self.slots,
            dimensions: &self.dimensions,
            directions: &self.directions,
        };
        world.basic_projectile_hit(self.shot_id, [end[0], end[1], heading], elapsed, &[1; 12])
    }
    pub fn object_kind(&self, id: i16) -> Option<u8> {
        let index = usize::try_from(id).ok()?.checked_sub(1)?;
        self.records.get(index).map(|record| record[0x14])
    }
    pub fn remove(&mut self, id: i16) {
        for cell in &mut self.grid {
            cell.retain(|&v| v != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_obstacle_collision_follows_native_heading() {
        let mut game = Game::new(crate::formats::World {
            obstacles: vec![crate::formats::Obstacle {
                kind: 0,
                h: 500,
                v: 500,
                facing: 0,
            }],
        })
        .unwrap();
        game.items[0].shape = None;
        game.items[0].min = [-40., 0., -4.];
        game.items[0].max = [40., 20., 4.];
        let mut models = crate::model_slots::ModelSlots::default();
        models.state.current[6] = 1;
        models.state.current[0] = 2;
        for (slot, dimensions, radius) in [(1, [4 << 16, 4 << 16], 3), (2, [80 << 16, 8 << 16], 41)]
        {
            models.geometry.insert(
                slot,
                crate::model::PackedShape {
                    vertices: vec![],
                    dimensions,
                    candidate_radius: radius,
                },
            );
        }
        game.vertical.vertical = 61 << 16;
        let flight_query = Query::from_game(&game, &models).unwrap();
        assert_eq!(
            &flight_query.records[0][0x28..0x2c],
            &(61_i32 << 16).to_be_bytes()
        );
        game.vertical = Default::default();
        let query = Query::from_game(&game, &models).unwrap();
        let pose = [500 << 16, 525 << 16, 90 << 16];
        assert_eq!(query.player_movement_blocked(pose, 0), Some(true));
        assert_eq!(query.player_movement_blocked(pose, 60 << 16), Some(true));
        assert_eq!(query.player_movement_blocked(pose, 61 << 16), Some(false));
        assert_eq!(
            query.player_movement_blocked([525 << 16, 500 << 16, 90 << 16], 0),
            Some(false)
        );
        let mut missile_query = Query::from_game(&game, &models).unwrap();
        for kind in [9, 34] {
            assert_eq!(
                missile_query.hit_projectile(
                    [500 << 16, 500 << 16],
                    [500 << 16, 500 << 16],
                    90 << 16,
                    1,
                    60 << 16,
                    kind
                ),
                Some(2)
            );
            assert_eq!(
                missile_query.hit_projectile(
                    [500 << 16, 500 << 16],
                    [500 << 16, 500 << 16],
                    90 << 16,
                    1,
                    (60 << 16) + 1,
                    kind
                ),
                Some(-1)
            );
        }
        assert_eq!(
            missile_query.hit_projectile([0; 2], [0; 2], 0, 1, 0, 33),
            None
        );
        let mut surface_query = Query::from_game(&game, &models).unwrap();
        surface_query.records[1][0x14] = 18;
        assert_eq!(
            surface_query.player_movement_blocked(pose, 0),
            Some(false),
            "mud remains passable even without a loaded mud slot"
        );
        assert_eq!(
            query.records[0][0x28..0x2c],
            [0; 4],
            "proposed altitude does not mutate source records"
        );
        let blocked = |game: &Game, x, z| {
            Query::from_game(game, &models)
                .unwrap()
                .player_spawn_blocked([x << 16, z << 16, 90 << 16])
                .unwrap()
        };
        assert!(
            blocked(&game, 500, 525),
            "native zero has its long local-X extent along world Z"
        );
        assert!(
            !blocked(&game, 525, 500),
            "space beside the narrow edge remains clear"
        );
        assert!(
            !game.free(500., 525.),
            "driving footprint uses the same native orientation"
        );
        assert!(
            game.free(525., 500.),
            "driving does not hit the old rotated footprint"
        );
        assert!(game.items[0].contains(500., 525., 1., false));
        assert!(!game.items[0].contains(525., 500., 1., false));
        game.items[0].heading = std::f32::consts::FRAC_PI_2;
        assert!(!blocked(&game, 500, 525));
        assert!(
            blocked(&game, 525, 500),
            "native ninety rotates the long extent to world X"
        );
    }
}
