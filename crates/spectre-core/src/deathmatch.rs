//! New two-player demo rules. Movement and scenery use the recovered game core;
//! scoring, respawn timing, and player hit boxes are native-port multiplayer rules.
use super::{Game, Input, Shot, Target};
use serde::{Deserialize, Serialize};

pub const WIN_SCORE: u32 = 5;
pub const HEALTH: u8 = 3;
#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub speed: f32,
    pub health: u8,
    pub kills: u32,
    pub deaths: u32,
    pub respawn: u32,
    pub shield: u32,
    pub fired: u64,
    pub motion: MotionState,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub owner: usize,
    pub id: i32,
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub height: f32,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub players: [Player; 2],
    pub shots: Vec<Projectile>,
    pub winner: Option<usize>,
    pub sounds: Vec<u16>,
    pub player_sounds: [Vec<u16>; 2],
}
pub struct Match {
    games: [Game; 2],
    players: [Player; 2],
    tick: u64,
    winner: Option<usize>,
    sounds: Vec<u16>,
    player_sounds: [Vec<u16>; 2],
}
impl Match {
    pub fn new(mut games: [Game; 2]) -> Self {
        for g in &mut games {
            g.targets.clear();
            g.initial_targets.clear();
            g.encounter.enabled = false;
            g.flags.enabled = false;
            g.optional_weapons = Default::default();
            g.shots.clear();
            g.enemy_shots.clear();
        }
        let players = std::array::from_fn(|_| Player {
            x: 0.,
            z: 0.,
            heading: 0.,
            speed: 0.,
            health: HEALTH,
            kills: 0,
            deaths: 0,
            respawn: 0,
            shield: 90,
            fired: 0,
            motion: MotionState::capture(&games[0]),
        });
        let mut result = Self {
            games,
            players,
            tick: 0,
            winner: None,
            sounds: vec![],
            player_sounds: Default::default(),
        };
        result.spawn(0);
        result.spawn(1);
        result
    }
    fn spawn(&mut self, slot: usize) {
        let other = &self.players[1 - slot];
        let preferred = if slot == 0 {
            [320., 512.]
        } else {
            [704., 512.]
        };
        let mut candidates: Vec<_> = (64..=960)
            .step_by(32)
            .flat_map(|z| (64..=960).step_by(32).map(move |x| [x as f32, z as f32]))
            .collect();
        candidates.sort_by(|a, b| {
            let score = |p: &[f32; 2]| (p[0] - preferred[0]).hypot(p[1] - preferred[1]);
            score(a).total_cmp(&score(b))
        });
        let g = &mut self.games[slot];
        let position = candidates
            .into_iter()
            .find(|p| g.free_at(p[0], p[1], 0.) && (p[0] - other.x).hypot(p[1] - other.z) > 160.)
            .unwrap_or(g.spawn);
        let heading = if slot == 0 {
            std::f32::consts::FRAC_PI_2
        } else {
            3. * std::f32::consts::FRAC_PI_2
        };
        set_pose(g, position[0], position[1], heading);
        g.speed_byte = 0;
        g.turn_ramp = 0;
        g.tank.speed = 0.;
        g.vertical = Default::default();
        g.shots.clear();
        let p = &mut self.players[slot];
        p.health = HEALTH;
        p.respawn = 0;
        p.shield = 90;
        self.sync_player(slot);
    }
    fn sync_player(&mut self, slot: usize) {
        let g = &self.games[slot];
        let p = &mut self.players[slot];
        p.x = g.tank.x;
        p.z = g.tank.z;
        p.heading = g.tank.heading;
        p.speed = g.tank.speed;
        p.fired = g.fired;
        p.motion = MotionState::capture(g);
    }
    pub fn step(&mut self, inputs: [Input; 2]) {
        if self.winner.is_some() {
            return;
        }
        self.tick += 1;
        let before: [Vec<_>; 2] = std::array::from_fn(|i| {
            self.games[i]
                .shots
                .iter()
                .map(|s| (s.deadline, [s.x, s.z]))
                .collect()
        });
        let old_positions = self.players.clone();
        for (i, input) in inputs.into_iter().enumerate() {
            if self.players[i].respawn > 0 {
                // Existing rounds keep flying while their owner waits to respawn.
                let g = &mut self.games[i];
                g.speed_byte = 0;
                g.turn_ramp = 0;
                g.audio_events.clear();
                g.step(Input::default());
                self.players[i].respawn -= 1;
                if self.players[i].respawn == 0 {
                    self.spawn(i);
                }
                continue;
            }
            self.players[i].shield = self.players[i].shield.saturating_sub(1);
            let g = &mut self.games[i];
            g.ammo = 999;
            // Cannon only. Firing ends spawn protection so it cannot be exploited.
            if input.fire {
                self.players[i].shield = 0;
            }
            g.audio_events.clear();
            g.step(Input {
                select_weapon: false,
                fire_optional: false,
                ..input
            });
            self.player_sounds[i].extend_from_slice(&g.audio_events);
            self.sync_player(i);
        }
        if self.players.iter().all(|p| p.health > 0)
            && crate::scene::footprints_overlap(
                &Game::tank_outline(
                    self.players[0].x,
                    self.players[0].z,
                    self.players[0].heading,
                ),
                &Game::tank_outline(
                    self.players[1].x,
                    self.players[1].z,
                    self.players[1].heading,
                ),
            )
        {
            for (i, old) in old_positions.iter().enumerate() {
                set_pose(&mut self.games[i], old.x, old.z, old.heading);
                self.games[i].speed_byte = 0;
                self.games[i].tank.speed = 0.;
                self.sync_player(i);
            }
        }
        let mut damage = [0u8; 2];
        for owner in 0..2 {
            let victim = 1 - owner;
            if self.players[victim].health == 0 {
                continue;
            }
            let p = &self.players[victim];
            let polygon = Game::tank_outline(p.x, p.z, p.heading);
            for shot in &mut self.games[owner].shots {
                if shot.retiring || shot.height > 12. {
                    continue;
                }
                let start = before[owner]
                    .iter()
                    .find(|s| s.0 == shot.deadline)
                    .map_or([old_positions[owner].x, old_positions[owner].z], |s| s.1);
                if segment_hits(start, [shot.x, shot.z], &polygon) {
                    shot.life = 0;
                    if p.shield == 0 {
                        damage[victim] = damage[victim].saturating_add(1);
                    }
                }
            }
            self.games[owner].shots.retain(|s| s.life > 0);
        }
        for (victim, hits) in damage.into_iter().enumerate() {
            if hits == 0 {
                continue;
            }
            self.sounds.push(10003);
            self.players[victim].health = self.players[victim].health.saturating_sub(hits);
            if self.players[victim].health == 0 {
                self.players[victim].deaths += 1;
                self.players[victim].respawn = 120;
                self.players[1 - victim].kills += 1;
            }
        }
        // A simultaneous fifth kill is a draw: play on until one score leads.
        if self.players[0].kills != self.players[1].kills {
            self.winner = (0..2).find(|i| self.players[*i].kills >= WIN_SCORE);
        }
        self.sounds.truncate(64);
        for sounds in &mut self.player_sounds {
            sounds.truncate(64);
        }
        // Contacts may change pose or remove rounds after each player's movement step.
        for i in 0..2 {
            self.sync_player(i);
        }
    }
    pub fn snapshot(&mut self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            players: self.players.clone(),
            winner: self.winner,
            shots: self
                .games
                .iter()
                .enumerate()
                .flat_map(|(owner, g)| {
                    g.shots
                        .iter()
                        .filter(|s| !s.retiring)
                        .map(move |s| Projectile {
                            owner,
                            id: s.deadline,
                            x: s.x,
                            z: s.z,
                            heading: s.heading,
                            height: s.height,
                        })
                })
                .collect(),
            sounds: std::mem::take(&mut self.sounds),
            player_sounds: std::mem::take(&mut self.player_sounds),
        }
    }
}
fn set_pose(g: &mut Game, x: f32, z: f32, heading: f32) {
    g.tank.x = x;
    g.tank.z = z;
    g.tank.heading = heading;
    g.position_fixed = [(x * 65536.) as i32, (z * 65536.) as i32];
    g.heading_fixed = ((90. - heading.to_degrees()).rem_euclid(360.) * 65536.).round() as i32;
}
// Clip a swept projectile against the actual eight-vertex player footprint.
fn segment_hits(a: [f32; 2], b: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let cross = |u: [f32; 2], v: [f32; 2]| u[0] * v[1] - u[1] * v[0];
    let sub = |u: [f32; 2], v: [f32; 2]| [u[0] - v[0], u[1] - v[1]];
    let mut positive = false;
    let mut negative = false;
    for i in 0..polygon.len() {
        let c = polygon[i];
        let d = polygon[(i + 1) % polygon.len()];
        let side = cross(sub(d, c), sub(a, c));
        positive |= side > 0.;
        negative |= side < 0.;
        let denom = cross(sub(b, a), sub(d, c));
        if denom.abs() > 0.00001 {
            let t = cross(sub(c, a), sub(d, c)) / denom;
            let u = cross(sub(c, a), sub(b, a)) / denom;
            if (0. ..=1.).contains(&t) && (0. ..=1.).contains(&u) {
                return true;
            }
        }
    }
    !(positive && negative)
}
impl Snapshot {
    pub fn apply(&self, g: &mut Game, slot: usize, template: crate::scene::Item) {
        let p = &self.players[slot];
        set_pose(g, p.x, p.z, p.heading);
        g.tank.speed = p.speed;
        g.tank.ticks = self.tick;
        g.targets.clear();
        g.enemy_shots.clear();
        g.shots.clear();
        g.impacts.clear();
        g.encounter.enabled = false;
        g.flags.enabled = false;
        let other = &self.players[1 - slot];
        if other.health > 0 {
            let mut item = template;
            item.x = other.x;
            item.z = other.z;
            item.heading = other.heading;
            g.targets.push(Target {
                item,
                health: i16::from(other.health),
                max_health: i16::from(HEALTH),
                fire: Default::default(),
                motion: Default::default(),
                navigation: Default::default(),
                lives: 1,
                deaths: other.deaths,
                retired: false,
                kill_score: 0,
                vulnerable: true,
                animation_frame: 0,
            });
        }
        for s in &self.shots {
            g.shots.push(Shot {
                lifecycle_record: None,
                retiring: false,
                missile: None,
                weapon: 0,
                enemy_owner: None,
                bouncing_record: None,
                height: s.height,
                x: s.x,
                z: s.z,
                heading: s.heading,
                life: 1,
                position_fixed: [0; 2],
                direction_fixed: [0; 2],
                heading_fixed: 0,
                deadline: 0,
                last_update: 0,
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn arena() -> Match {
        Match::new(std::array::from_fn(|_| {
            Game::new(crate::formats::World { obstacles: vec![] }).unwrap()
        }))
    }
    #[test]
    fn cannon_kills_respawns_and_wins() {
        let mut m = arena();
        for _ in 0..6000 {
            m.step([
                Input {
                    fire: true,
                    ..Default::default()
                },
                Input::default(),
            ]);
            if m.winner.is_some() {
                break;
            }
        }
        let s = m.snapshot();
        assert_eq!(s.winner, Some(0));
        assert_eq!(s.players[0].kills, WIN_SCORE);
        assert_eq!(s.players[1].deaths, WIN_SCORE);
        assert!(s.players[0].fired > 15);
        assert!(s.players[1].respawn > 0);
        let tick = s.tick;
        m.step([Input::default(); 2]);
        assert_eq!(m.snapshot().tick, tick);
    }
    #[test]
    fn wall_blocks_cannon_and_dead_owners_shots_keep_moving() {
        let wall = crate::scene::Item {
            kind: 13,
            shape_id: 0,
            x: 512.,
            z: 512.,
            heading: 0.,
            source: "Test cover".into(),
            min: [-20., 0., -80.],
            max: [20., 40., 80.],
            shape: None,
        };
        let mut m = Match::new(std::array::from_fn(|_| {
            Game::with_items(
                crate::formats::World { obstacles: vec![] },
                vec![wall.clone()],
            )
            .unwrap()
        }));
        for _ in 0..400 {
            m.step([
                Input {
                    fire: true,
                    ..Default::default()
                },
                Input::default(),
            ]);
        }
        assert_eq!(m.players[1].health, HEALTH, "cover blocks cannon fire");
        let mut m = arena();
        for _ in 0..12 {
            m.step([
                Input {
                    fire: true,
                    ..Default::default()
                },
                Input::default(),
            ]);
        }
        let before = m.games[0].shots[0].x;
        m.players[0].health = 0;
        m.players[0].respawn = 120;
        m.step([Input::default(); 2]);
        assert_ne!(
            m.games[0].shots[0].x, before,
            "dead owners do not freeze their rounds"
        );
    }
    #[test]
    fn footprint_has_no_large_invisible_padding() {
        let p = Game::tank_outline(100., 100., 0.);
        assert!(segment_hits([100., 0.], [100., 200.], &p));
        assert!(!segment_hits([111., 0.], [111., 200.], &p));
    }
    #[test]
    fn both_players_drive_and_spawn_protection_expires() {
        let mut m = arena();
        let start = m.snapshot();
        for _ in 0..100 {
            m.step(
                [Input {
                    throttle: 1.,
                    ..Default::default()
                }; 2],
            );
        }
        let end = m.snapshot();
        for i in 0..2 {
            assert_ne!(start.players[i].x, end.players[i].x);
            assert_eq!(end.players[i].shield, 0);
        }
    }
}

/// Authoritative movement checkpoint. Kept in fixed point so client replay uses
/// the same collision, acceleration, turning, fire clock, and surface state.
#[derive(Clone, Serialize, Deserialize)]
pub struct MotionState {
    pose: [i32; 3],
    speed: i8,
    ramp: i8,
    vertical: crate::recovered::VerticalState,
    ticks: u64,
    last_fire: i32,
    ice_deadline: i32,
    mud_deadline: i32,
    flow_contact: Option<i32>,
    flight_energy: i16,
    fired: u64,
    shots: Vec<CannonState>,
}
#[derive(Clone, Serialize, Deserialize)]
struct CannonState {
    pose: [i32; 3],
    direction: [i32; 2],
    deadline: i32,
    last_update: i32,
    height: f32,
}
impl MotionState {
    fn capture(g: &Game) -> Self {
        Self {
            pose: [g.position_fixed[0], g.position_fixed[1], g.heading_fixed],
            speed: g.speed_byte,
            ramp: g.turn_ramp,
            vertical: g.vertical.clone(),
            ticks: g.tank.ticks,
            last_fire: g.last_fire_attempt,
            ice_deadline: g.ice_sound_deadline,
            mud_deadline: g.mud_sound_deadline,
            flow_contact: g.flow_last_contact,
            flight_energy: g.flight_energy,
            fired: g.fired,
            shots: g
                .shots
                .iter()
                .filter(|s| !s.retiring && s.weapon == 0)
                .map(|s| CannonState {
                    pose: [s.position_fixed[0], s.position_fixed[1], s.heading_fixed],
                    direction: s.direction_fixed,
                    deadline: s.deadline,
                    last_update: s.last_update,
                    height: s.height,
                })
                .collect(),
        }
    }
    pub fn restore(&self, g: &mut Game) {
        g.position_fixed = [self.pose[0], self.pose[1]];
        g.heading_fixed = self.pose[2];
        g.tank.x = self.pose[0] as f32 / 65536.;
        g.tank.z = self.pose[1] as f32 / 65536.;
        g.tank.heading = (90. - self.pose[2] as f32 / 65536.)
            .to_radians()
            .rem_euclid(std::f32::consts::TAU);
        g.tank.speed = self.speed as f32 * 15.;
        g.tank.ticks = self.ticks;
        g.speed_byte = self.speed;
        g.turn_ramp = self.ramp;
        g.vertical = self.vertical.clone();
        g.last_fire_attempt = self.last_fire;
        g.ice_sound_deadline = self.ice_deadline;
        g.mud_sound_deadline = self.mud_deadline;
        g.flow_last_contact = self.flow_contact;
        g.flight_energy = self.flight_energy;
        g.fired = self.fired;
        g.targets.clear();
        g.enemy_shots.clear();
        g.impacts.clear();
        g.audio_events.clear();
        g.encounter.enabled = false;
        g.flags.enabled = false;
        g.optional_weapons = Default::default();
        g.death_transition = None;
        g.ammo = 999;
        g.shots = self
            .shots
            .iter()
            .map(|s| Shot {
                lifecycle_record: None,
                retiring: false,
                missile: None,
                weapon: 0,
                enemy_owner: None,
                bouncing_record: None,
                height: s.height,
                x: s.pose[0] as f32 / 65536.,
                z: s.pose[1] as f32 / 65536.,
                heading: (90. - s.pose[2] as f32 / 65536.)
                    .to_radians()
                    .rem_euclid(std::f32::consts::TAU),
                life: s.deadline.wrapping_sub(self.ticks as i32).max(0) as u32,
                position_fixed: [s.pose[0], s.pose[1]],
                direction_fixed: s.direction,
                heading_fixed: s.pose[2],
                deadline: s.deadline,
                last_update: s.last_update,
            })
            .collect();
    }
}
pub fn prediction_state(g: &Game) -> serde_json::Value {
    serde_json::json!({"x":g.tank.x,"z":g.tank.z,"heading":g.tank.heading,"speed":g.tank.speed,"fired":g.fired,
        "shots":g.shots.iter().filter(|s|!s.retiring).map(|s|Projectile {owner:0,id:s.deadline,x:s.x,z:s.z,heading:s.heading,height:s.height}).collect::<Vec<_>>(),
        "sounds":g.audio_events})
}
