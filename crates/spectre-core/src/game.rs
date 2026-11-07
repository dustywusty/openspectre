//! Playable recovery preview. Type-6 speed/turn/translation use recovered rules.
//! Direction values are a modeled-SANE candidate. Single-tick scheduling,
//! collision, spawn, and scheduling remain preview behavior.
use crate::{Result, formats::World};
use serde::Serialize;

#[path = "deathmatch.rs"]
pub mod deathmatch;
#[path = "encounter.rs"]
mod encounter;
pub use encounter::{Encounter, EnemyFireState, EnemyMotionParameters, EnemyNavigation};

pub const STEP: f64 = 1.0 / 60.0;
pub const RADIUS: f32 = 22.;
pub const PROXY_HALF: f32 = 12.;
pub const ARENA_SIZE: f32 = 1024.;
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub throttle: f32,
    pub steer: f32,
    pub brake: bool,
    pub fire: bool,
    pub select_weapon: bool,
    pub fire_optional: bool,
}
#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub struct OptionalWeapons {
    pub selected: u8,
    pub available: [u8; 18],
}

#[derive(Clone, Debug, Serialize)]
pub struct Tank {
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub speed: f32,
    pub ticks: u64,
    pub blocked: bool,
    pub surface: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct Shot {
    #[serde(skip)]
    lifecycle_record: Option<[u8; 128]>,
    pub retiring: bool,
    pub missile: Option<crate::recovered::MissileGuidance>,
    pub weapon: u8,
    /// Stable preview target slot; None denotes a player shot.
    #[serde(skip)]
    enemy_owner: Option<usize>,
    #[serde(skip)]
    bouncing_record: Option<[u8; 128]>,
    pub height: f32,
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub life: u32,
    #[serde(skip)]
    position_fixed: [i32; 2],
    #[serde(skip)]
    direction_fixed: [i32; 2],
    #[serde(skip)]
    heading_fixed: i32,
    #[serde(skip)]
    deadline: i32,
    #[serde(skip)]
    last_update: i32,
}
impl Shot {
    fn retire_basic_contact(
        &mut self,
        end: [i32; 2],
        elapsed: i32,
        now: i32,
        targets: &[crate::recovered::AiTargetCandidate; 12],
        unit_table: &[i32; 360],
    ) -> bool {
        use crate::recovered;
        if self.lifecycle_record.is_none()
            && (matches!(self.weapon, 0 | 3 | 4 | 5 | 9 | 10)
                || (self.weapon == 6 && self.bouncing_record.is_some()))
        {
            // This adapter retains only fields consumed by movement, retirement, and expiry.
            // The original geometry allocator and grid remain separate recovery work.
            let mut record = self.bouncing_record.unwrap_or([0; 128]);
            record[0x14] = if self.bouncing_record.is_some() {
                29
            } else {
                9
            };
            let owner = self.enemy_owner.map_or(Some(1), |slot| slot.checked_add(2));
            let Some(owner) = owner.filter(|owner| *owner <= 12) else {
                return false;
            };
            record[0x1a] = owner as u8;
            record[0x1c..0x1e].copy_from_slice(&i16::from(self.weapon).to_be_bytes());
            if self.bouncing_record.is_none() {
                record[0x28..0x2c]
                    .copy_from_slice(&(((self.height - 4.) * 524288.) as i32).to_be_bytes());
            }
            if let Some(missile) = self.missile {
                record[0x14] = 34;
                record[0x24..0x28].copy_from_slice(&missile.vertical_velocity.to_be_bytes());
                record[0x28..0x2c].copy_from_slice(&missile.height.to_be_bytes());
            }
            self.lifecycle_record = Some(record);
        }
        let Some(record) = &mut self.lifecycle_record else {
            return false;
        };
        record[..4].copy_from_slice(&end[0].to_be_bytes());
        record[4..8].copy_from_slice(&end[1].to_be_bytes());
        record[8..12].copy_from_slice(&self.heading_fixed.to_be_bytes());
        // Native policy uses the original enabled effect path, without old-Mac throttling.
        // Active shot limits derive from !retiring; there is no original participant pool yet.
        recovered::retire_projectile_contact(record, elapsed, now, 1, &mut [0; 12], |r, dt| {
            if dt == 0 {
                return;
            }
            if let Some(bouncing) = &mut self.bouncing_record {
                *bouncing = *r;
                recovered::move_bouncing_projectile(bouncing, self.direction_fixed, dt);
                *r = *bouncing;
                self.height = f32::from(recovered::render_height_word(i32::from_be_bytes(
                    r[0x28..0x2c].try_into().unwrap(),
                ))) / 8.;
                return;
            }
            if let Some(missile) = &mut self.missile {
                missile.pose = [end[0], end[1], self.heading_fixed];
                let adjusted = recovered::smart_missile_elapsed(dt, Some(6));
                recovered::smart_missile_guidance(
                    missile,
                    r[0x1a] as i8,
                    adjusted,
                    targets,
                    &[0; 12],
                    |target, source| {
                        recovered::ai_aim_angle(
                            [target[0], target[1]],
                            source,
                            encounter::candidate_slope_angle,
                        )
                        .unwrap_or(0)
                    },
                );
                recovered::move_smart_missile(
                    missile,
                    adjusted,
                    &unit_table.map(|v| v.wrapping_mul(6)),
                )
                .expect("wrapped missile heading");
                for (offset, value) in [
                    (0, missile.pose[0]),
                    (4, missile.pose[1]),
                    (8, missile.pose[2]),
                    (0x24, missile.vertical_velocity),
                    (0x28, missile.height),
                ] {
                    r[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                }
                self.heading_fixed = missile.pose[2];
                self.heading = (90. - self.heading_fixed as f32 / 65536.)
                    .to_radians()
                    .rem_euclid(std::f32::consts::TAU);
                self.height = 4. + missile.height as f32 / 524288.;
                return;
            }
            if self.weapon == 9 {
                let pose = std::array::from_fn(|i| {
                    i32::from_be_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
                });
                self.heading_fixed =
                    recovered::seeker_heading(r[0x1a] as i8, pose, targets, |target, source| {
                        recovered::ai_aim_angle(
                            [target[0], target[1]],
                            source,
                            encounter::candidate_slope_angle,
                        )
                        .unwrap_or(0)
                    });
                r[8..12].copy_from_slice(&self.heading_fixed.to_be_bytes());
                self.direction_fixed =
                    recovered::movement_direction(self.heading_fixed, unit_table)
                        .expect("wrapped Seeker heading")
                        .map(|v| v.wrapping_mul(6));
                self.heading = (90. - self.heading_fixed as f32 / 65536.)
                    .to_radians()
                    .rem_euclid(std::f32::consts::TAU);
            }
            let position = std::array::from_fn(|i| {
                i32::from_be_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
            });
            let moved = recovered::move_basic_shot(position, self.direction_fixed, dt);
            for (i, value) in moved.iter().enumerate() {
                r[i * 4..i * 4 + 4].copy_from_slice(&value.to_be_bytes());
            }
        })
        .expect("validated projectile owner");
        self.position_fixed = std::array::from_fn(|i| {
            i32::from_be_bytes(record[i * 4..i * 4 + 4].try_into().unwrap())
        });
        self.x = self.position_fixed[0] as f32 / 65536.;
        self.z = self.position_fixed[1] as f32 / 65536.;
        self.retiring = true;
        self.life = 60;
        true
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Impact {
    pub x: f32,
    pub z: f32,
    pub life: u32,
}
#[derive(Clone, Serialize)]
pub struct Target {
    pub fire: EnemyFireState,
    pub motion: EnemyMotionParameters,
    pub navigation: EnemyNavigation,
    pub lives: i16,
    pub deaths: u32,
    pub retired: bool,
    pub kill_score: i32,
    pub item: crate::scene::Item,
    pub health: i16,
    pub max_health: i16,
    pub vulnerable: bool,
    pub animation_frame: usize,
}
#[derive(Clone, Default, Serialize)]
pub struct FlagObjective {
    pub enabled: bool,
    pub total: usize,
    pub collected: usize,
    pub pickup_points: u32,
}
#[derive(Clone, Serialize)]
pub struct MissionProgress {
    pub score: i32,
    pub lives: i16,
    pub deaths: u32,
}
impl Default for MissionProgress {
    fn default() -> Self {
        Self {
            score: 0,
            lives: 3,
            deaths: 0,
        }
    }
}
#[derive(Clone, Serialize)]
pub struct DeathTransition {
    pub started: i32,
    pub deadline: i32,
    #[serde(skip)]
    record: [u8; 128],
    #[serde(skip)]
    state: [u8; 30],
}

pub struct Game {
    pub palette: Option<[[u16; 3]; 13]>,
    pub ground_color_index: Option<u8>,
    pub color_bindings: Option<crate::recovered::ColorBindings>,
    pub radar_glyphs: Option<Vec<Vec<[i8; 2]>>>,
    pub mission: MissionProgress,
    entry_mission: MissionProgress,
    pub vehicle_settings: [u8; 7],
    entry_vehicle_settings: [u8; 7],
    pub score_rules: Option<crate::formats::ScoreRules>,
    pub flags: FlagObjective,
    initial_items: Vec<crate::scene::Item>,
    placement_items: Vec<crate::scene::Item>,
    item_placement_ids: Vec<usize>,
    pub world: World,
    pub items: Vec<crate::scene::Item>,
    pub tank: Tank,
    pub shots: Vec<Shot>,
    pub impacts: Vec<Impact>,
    pub encounter: Encounter,
    pub death_transition: Option<DeathTransition>,
    pub infinite_health: bool,
    pub enemy_shots: Vec<Shot>,
    pub fired: u64,
    pub radar_flash: u8,
    pub selected_difficulty: i16,
    pub radar_display: std::cell::RefCell<crate::radar::DisplayState>,
    radar_flash_deadline: i32,
    pub combat_counters: crate::recovered::CombatCounters,
    pub par_seconds: Option<i16>,
    pub completion: Option<crate::recovered::CompletionRatings>,
    // Normalize the original uptime clock to start at 75. A zeroed grace
    // timestamp then means expired, while a fresh arena keeps its 75-tick grace.
    damage_grace_start: i32,
    pub targets: Vec<Target>,
    initial_targets: Vec<Target>,
    pub destroyed: u32,
    pub enemy_clear_bonus: Option<i16>,
    pub enemy_clear_award: i16,
    enemy_clear_awarded: bool,
    pub scenario: String,
    pub audio_events: Vec<u16>,
    pub collision_mode: &'static str,
    collision_models: Option<crate::model_slots::ModelSlots>,
    pub primary_damage: i16,
    pub acid_damage: i16,
    acid_deadline: i32,
    ice_sound_deadline: i32,
    mud_sound_deadline: i32,
    pub primary_ammo_cost: i16,
    pub weapon_ammo_costs: [i16; 18],
    pub missile_model: Option<crate::scene::Item>,
    pub weapon_damage: [i16; 18],
    pub optional_weapons: OptionalWeapons,
    initial_optional_weapons: OptionalWeapons,
    pub ammo: i16,
    pub dump_weights: [i8; 4],
    pub pickups: Vec<crate::pickups::Pickup>,
    initial_pickups: Vec<crate::pickups::Pickup>,
    pub pickup_random: u32,
    initial_pickup_random: u32,
    pub pickups_collected: i16,
    pub flight_energy: i16,
    pub vertical: crate::recovered::VerticalState,
    pub pending_shields: Vec<crate::pickups::ShieldRequest>,
    last_fire_attempt: i32,
    last_weapon_selection: i32,
    spawn: [f32; 2],
    spawn_random: u32,
    flag_spawn_cursor: i16,
    accumulator: f64,
    position_fixed: [i32; 2],
    heading_fixed: i32,
    flow_last_contact: Option<i32>,
    speed_byte: i8,
    turn_ramp: i8,
    direction_table: [i32; 360],
    unit_table: [i32; 360],
    type31_frames: Vec<crate::formats::Shape>,
    type31_frame: i8,
    type31_timer: i32,
}
impl Game {
    /// CODE 33 clears the initial set; CODE 25 applies all hints through this level.
    pub fn configure_optional_weapons(&mut self, waves: &[crate::formats::Wave], level: i16) {
        let mut state = OptionalWeapons::default();
        for wave in waves.iter().take(usize::try_from(level).unwrap_or(0)) {
            crate::recovered::unlock_optional_weapon(
                wave.hint,
                &mut state.selected,
                &mut state.available,
            );
        }
        self.optional_weapons = state;
        self.initial_optional_weapons = state;
    }

    pub(super) fn begin_player_death(&mut self) {
        let now = self.tank.ticks as i32;
        let mut record = [0; 128];
        for (offset, value) in [
            (0, self.position_fixed[0]),
            (4, self.position_fixed[1]),
            (8, self.heading_fixed),
        ] {
            record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        }
        record[0x28..0x2c].copy_from_slice(&self.vertical.vertical.to_be_bytes());
        record[0x14] = 6;
        let mut state = [0; 30];
        crate::recovered::local_death_state(&mut record, &mut state, now, true, 1);
        crate::recovered::damage_response_fields(
            &mut record,
            &mut [0; 256],
            crate::recovered::DamageResponseInput {
                now,
                local: 1,
                victim: 1,
                attacker: 0,
                killed: 1,
                game_mode: 0,
            },
        );
        self.death_transition = Some(DeathTransition {
            started: now,
            deadline: i32::from_be_bytes(record[0x16..0x1a].try_into().unwrap()),
            record,
            state,
        });
        self.speed_byte = 0;
        self.turn_ramp = 0;
        self.tank.speed = 0.;
    }
    pub fn death_camera(&self) -> Option<crate::recovered::DeathCamera> {
        let death = self.death_transition.as_ref()?;
        crate::recovered::death_camera(&death.state, self.tank.ticks as i32, 32, |angle| {
            crate::recovered::movement_direction(angle, &self.unit_table).unwrap()
        })
    }
    /// Fixed position, ground-renderer direction order, and renderer height.
    pub fn first_person_heading(&self) -> i32 {
        self.death_camera()
            .map_or(self.heading_fixed, |camera| camera.pose[2])
    }
    pub fn first_person_camera(&self) -> ([i32; 2], [i32; 2], i32) {
        let (position, heading, height) = self.death_camera().map_or(
            (
                self.position_fixed,
                self.heading_fixed,
                self.vertical.renderer_height as i32 + self.vertical.hover.max(0),
            ),
            |camera| {
                (
                    [camera.pose[0], camera.pose[1]],
                    camera.pose[2],
                    camera.height,
                )
            },
        );
        let [cosine, sine] =
            crate::recovered::movement_direction(heading, &self.unit_table).unwrap();
        (position, [sine, cosine], height)
    }
    fn finish_player_death_if_due(&mut self) {
        let Some(death) = &self.death_transition else {
            return;
        };
        if crate::recovered::participant_transition_ready(
            crate::recovered::ParticipantTransitionInput {
                now: self.tank.ticks as i32,
                deadline: death.deadline,
                kind: death.record[0x14],
                game_mode: 0,
                local: true,
                authority: 0,
                enemy: 0,
                active: 1,
                changed: 1,
            },
        ) == 0
        {
            return;
        }
        let mut death = self.death_transition.take().unwrap();
        crate::recovered::begin_participant_transition(&mut [0; 256], &mut death.record);
        if self.flags.enabled {
            self.respawn_player();
        } else {
            self.encounter.status = "lost";
        }
    }
    /// Exact internal movement state for conditional original-code replay checks.
    pub fn handling_snapshot(&self) -> ([i32; 2], i32, i8, i8) {
        (
            self.position_fixed,
            self.heading_fixed,
            self.speed_byte,
            self.turn_ramp,
        )
    }
    pub fn new(world: World) -> Result<Self> {
        let items = crate::scene::Catalog::default().instantiate(&world);
        Self::with_items(world, items)
    }
    pub fn with_items(world: World, items: Vec<crate::scene::Item>) -> Result<Self> {
        let mut game = Self {
            palette: None,
            ground_color_index: None,
            color_bindings: None,
            radar_glyphs: None,
            world,
            initial_items: items.clone(),
            placement_items: items.clone(),
            item_placement_ids: (0..items.len()).collect(),
            mission: MissionProgress::default(),
            entry_mission: MissionProgress::default(),
            vehicle_settings: crate::recovered::select_vehicle_settings(3, [0; 7], [0; 7]),
            entry_vehicle_settings: crate::recovered::select_vehicle_settings(3, [0; 7], [0; 7]),
            score_rules: None,
            flags: FlagObjective::default(),
            items,
            shots: Vec::new(),
            impacts: Vec::new(),
            encounter: Encounter::default(),
            death_transition: None,
            infinite_health: false,
            enemy_shots: Vec::new(),
            fired: 0,
            combat_counters: crate::recovered::CombatCounters::default(),
            par_seconds: None,
            completion: None,
            damage_grace_start: 75,
            targets: Vec::new(),
            initial_targets: Vec::new(),
            destroyed: 0,
            enemy_clear_bonus: None,
            enemy_clear_award: 0,
            enemy_clear_awarded: false,
            scenario: "No wave references this map".into(),
            audio_events: Vec::new(),
            collision_mode: "preview bounds",
            collision_models: None,
            primary_damage: 1,
            acid_damage: crate::recovered::default_enemy_parameters(23).collision_damage,
            acid_deadline: 0,
            ice_sound_deadline: 0,
            mud_sound_deadline: 0,
            primary_ammo_cost: 0,
            weapon_ammo_costs: [0; 18],
            weapon_damage: [1; 18],
            missile_model: None,
            optional_weapons: OptionalWeapons::default(),
            initial_optional_weapons: OptionalWeapons::default(),
            ammo: 40,
            dump_weights: [5, 2, 1, 2],
            pickups: Vec::new(),
            initial_pickups: Vec::new(),
            pickup_random: 0x44554d50,
            initial_pickup_random: 0x44554d50,
            pickups_collected: 0,
            flight_energy: 0,
            vertical: Default::default(),
            pending_shields: Vec::new(),
            last_fire_attempt: 0,
            last_weapon_selection: 0,
            radar_flash: 1,
            selected_difficulty: 0,
            radar_display: Default::default(),
            radar_flash_deadline: 25,
            tank: Tank {
                x: 512.,
                z: 512.,
                heading: 0.,
                speed: 0.,
                ticks: 0,
                blocked: false,
                surface: "unavailable",
            },
            spawn: [512., 512.],
            spawn_random: 0x53565231,
            flag_spawn_cursor: 1,
            accumulator: 0.,
            position_fixed: [0; 2],
            heading_fixed: 90 << 16,
            flow_last_contact: None,
            speed_byte: 0,
            turn_ramp: 0,
            direction_table: crate::recovered::candidate_movement_table(),
            unit_table: crate::recovered::candidate_unit_table(),
            type31_frames: Vec::new(),
            type31_frame: 0,
            type31_timer: -1,
        };
        // Search outward from the center. The original spawn convention is unknown.
        let mut candidates = Vec::new();
        for z in (48..=976).step_by(32) {
            for x in (48..=976).step_by(32) {
                candidates.push([x as f32, z as f32]);
            }
        }
        candidates.sort_by(|a, b| {
            let distance = |p: &[f32; 2]| (p[0] - 512.).powi(2) + (p[1] - 512.).powi(2);
            distance(a).total_cmp(&distance(b))
        });
        game.spawn = candidates
            .into_iter()
            .find(|p| game.free(p[0], p[1]))
            .ok_or("No safe prototype spawn in this world")?;
        game.reset();
        Ok(game)
    }
    pub fn add_target(&mut self, mut item: crate::scene::Item, health: i16) -> Result<()> {
        // Provisional deterministic placement; original spawn RNG/selection is pending.
        let mut candidates = Vec::new();
        for z in (64..=960).step_by(48) {
            for x in (64..=960).step_by(48) {
                let x = x as f32;
                let z = z as f32;
                if (x - self.tank.x).hypot(z - self.tank.z) < 100. {
                    continue;
                }
                candidates.push([x, z]);
            }
        }
        candidates.sort_by(|a, b| {
            let score =
                |p: &[f32; 2]| (p[0] - self.tank.x).powi(2) + (p[1] - self.tank.z - 150.).powi(2);
            score(a).total_cmp(&score(b))
        });
        let p = candidates
            .into_iter()
            .find(|p| {
                self.free(p[0], p[1])
                    && self
                        .targets
                        .iter()
                        .all(|t| (t.item.x - p[0]).hypot(t.item.z - p[1]) > 65.)
            })
            .ok_or("No safe enemy preview position")?;
        item.x = p[0];
        item.z = p[1];
        self.targets.push(Target {
            fire: Default::default(),
            motion: Default::default(),
            navigation: Default::default(),
            lives: 1,
            deaths: 0,
            retired: false,
            kill_score: 10,
            item,
            health,
            max_health: health,
            vulnerable: true,
            animation_frame: 0,
        });
        Ok(())
    }
    /// Original projectile models at runtime poses; raw height uses original eighth-units.
    pub fn projectile_models(&self, include_retired: bool) -> Vec<(crate::scene::Item, i32)> {
        let Some(model) = &self.missile_model else {
            return Vec::new();
        };
        self.shots
            .iter()
            .chain(&self.enemy_shots)
            .filter(|shot| include_retired || !shot.retiring)
            .filter_map(|shot| {
                let missile = shot.missile?;
                let mut item = model.clone();
                item.x = shot.x;
                item.z = shot.z;
                item.heading = shot.heading;
                Some((item, missile.height))
            })
            .collect()
    }

    pub fn configure_collision(&mut self, catalog: &crate::scene::Catalog) {
        self.missile_model = catalog
            .instantiate(&World {
                obstacles: vec![crate::formats::Obstacle {
                    kind: 34,
                    h: 0,
                    v: 0,
                    facing: 0,
                }],
            })
            .into_iter()
            .next()
            .filter(|item| item.shape.is_some());
        self.collision_models = None;
        let mut models = crate::model_slots::ModelSlots::default();
        // Explicit preview loading order, not recovered startup order.
        let kinds = std::iter::once(6)
            .chain(self.items.iter().map(|i| i.kind))
            .chain(self.targets.iter().map(|t| t.item.kind))
            .chain(self.pickups.iter().map(|p| p.item.kind));
        for kind in kinds {
            if kind == 12 {
                // Keep projectile geometry usable even if the flag dependency
                // cannot load. Flag bounds honor the current type-2 slot, if any.
                let _ = models.load_type(catalog, 12);
                continue;
            }
            // The original candidate filter excludes these before dimensions.
            if matches!(kind, 9 | 10 | 28 | 29 | 30 | 33 | 41) {
                continue;
            }
            if kind < 0 || models.load_type(catalog, kind as usize).is_none() {
                self.collision_mode = "preview bounds (builder pending)";
                return;
            }
            let slot = models.state.current[kind as usize] as i16;
            if !models.geometry.contains_key(&slot) {
                self.collision_mode = "preview bounds (slot unavailable)";
                return;
            }
        }
        self.collision_models = Some(models);
        self.collision_mode = "recovered query / preview records";
    }
    pub fn capture_targets(&mut self) {
        self.initial_targets = self.targets.clone();
    }
    pub fn configure_type31(&mut self, frames: Vec<crate::formats::Shape>) {
        if frames.len() < 2 || frames.len() > 127 {
            return;
        }
        self.type31_frames = frames;
        for target in &mut self.targets {
            if target.item.kind == 31 {
                target.vulnerable = false;
            }
        }
    }
    /// Explicit research control: replay CODE 16 action 24 after movie completion.
    /// Automatic movie/event selection is not implemented yet.
    pub fn replay_type31_event(&mut self) -> bool {
        if self.type31_frames.is_empty() || !self.targets.iter().any(|t| t.item.kind == 31) {
            return false;
        }
        self.type31_timer = 0;
        true
    }
    fn advance_type31(&mut self) {
        if self.type31_frames.is_empty() {
            return;
        }
        let count = self.type31_frames.len() as i8;
        let (frame, timer) = crate::recovered::advance_type31_model(
            self.type31_frame,
            0,
            count,
            self.type31_timer,
            self.tank.ticks as i32,
            6,
        );
        self.type31_timer = timer;
        if frame == self.type31_frame {
            return;
        }
        self.type31_frame = frame;
        for target in &mut self.targets {
            if target.item.kind != 31 {
                continue;
            }
            target.vulnerable = crate::recovered::damage_phase_ready([frame, 0, count]);
            target.animation_frame = frame as usize;
            if let Some(shape) = self.type31_frames.get(frame as usize) {
                target.item.shape = Some(shape.clone());
                target.item.source = format!("original type-31 animation, frame {}", frame + 1);
                // Collision remains the preview's model AABB, updated for this frame.
                target.item.min = [f32::INFINITY; 3];
                target.item.max = [f32::NEG_INFINITY; 3];
                for vertex in &shape.vertices {
                    for (i, value) in [vertex.x, vertex.y, vertex.z].iter().enumerate() {
                        target.item.min[i] = target.item.min[i].min(*value);
                        target.item.max[i] = target.item.max[i].max(*value);
                    }
                }
            }
        }
    }
    pub fn enable_flag_objective(&mut self) {
        self.initial_pickups = self.pickups.clone();
        self.initial_pickup_random = self.pickup_random;
        self.placement_items = self.items.clone();
        self.item_placement_ids = (0..self.items.len()).collect();
        self.flags.enabled = true;
        self.flags.total = self.initial_items.iter().filter(|i| i.kind == 12).count();
        self.set_encounter(true);
    }
    fn award_points(&mut self, points: i16) {
        if !self.flags.enabled {
            return;
        }
        if let Some((score, lives, awards)) = crate::recovered::award_score(
            self.mission.score,
            self.mission.lives,
            points,
            false,
            self.score_rules.as_ref(),
        ) {
            self.mission.score = score;
            self.mission.lives = lives;
            for _ in 0..awards {
                self.queue_sound_request(0x0001002a);
            }
        }
    }
    fn queue_sound_request(&mut self, request: u32) {
        if let Some(id) = crate::recovered::sound_request_id(request) {
            self.audio_events.push(id);
        }
    }
    fn collect_flag(&mut self, index: usize) {
        if !self.flags.enabled || self.items.get(index).is_none_or(|i| i.kind != 12) {
            return;
        }
        // CODE 28 leaves the collected flag's geometry slot as kind 14.
        // Rendering/query items can be compacted, but placement indices cannot.
        let slot = self.item_placement_ids.remove(index);
        self.placement_items[slot].kind = 14;
        self.items.remove(index);
        self.flags.collected += 1;
        // CODE 28 +059c calls CODE 31's score handler with 20 points.
        self.flags.pickup_points += 20;
        self.award_points(20);
        self.queue_sound_request(if self.flags.total - self.flags.collected == 1 {
            0x00010017
        } else {
            0x00010011
        });
        if self.flags.total > 0 && self.flags.collected == self.flags.total {
            self.encounter.status = "won";
            self.enemy_shots.clear();
            self.finish_mission();
        }
    }
    fn finish_mission(&mut self) {
        if self.completion.is_some() {
            return;
        }
        let Some(par) = self.par_seconds else {
            return;
        };
        let c = self.combat_counters;
        if let Some(result) = crate::recovered::completion_ratings(
            [0, self.tank.ticks as i32],
            par,
            [
                c.player_shots,
                c.player_hits,
                c.other_shots,
                c.player_damage_events,
            ],
            [1; 3],
        ) {
            // Original presentation awards the categories through separate score calls.
            for bonus in result.bonuses {
                self.award_points(bonus as i16);
            }
            self.completion = Some(result);
        }
    }
    /// Carry campaign progress into a fresh arena and retain its restart baseline.
    pub fn enter_with_progress(&mut self, progress: MissionProgress, settings: [u8; 7]) {
        self.vehicle_settings = settings;
        self.entry_vehicle_settings = settings;
        self.entry_mission = progress.clone();
        self.mission = progress;
        let settings = self.vehicle_settings;
        let (health, ammo) = crate::recovered::wave_refill(settings[1] as i8, settings[2] as i8);
        self.encounter.health = health;
        self.radar_display.get_mut().restore();
        self.ammo = ammo;
    }
    pub fn reset(&mut self) {
        self.pickups = self.initial_pickups.clone();
        self.pickup_random = self.initial_pickup_random;
        self.pickups_collected = 0;
        self.flight_energy = 0;
        self.vertical = Default::default();
        self.pending_shields.clear();
        self.death_transition = None;
        self.mission = self.entry_mission.clone();
        self.vehicle_settings = self.entry_vehicle_settings;
        self.spawn_random = 0x53565231;
        self.flag_spawn_cursor = 1;
        if self.flags.enabled {
            self.items = self.initial_items.clone();
            self.placement_items = self.initial_items.clone();
            self.item_placement_ids = (0..self.items.len()).collect();
        }
        self.flags.collected = 0;
        self.flags.pickup_points = 0;
        let settings = self.vehicle_settings;
        self.ammo = (settings[2] as i8 as i16) * 10;
        self.tank = Tank {
            x: self.spawn[0],
            z: self.spawn[1],
            heading: 0.,
            speed: 0.,
            ticks: 0,
            blocked: false,
            surface: "unavailable",
        };
        self.accumulator = 0.;
        self.radar_flash = 1;
        *self.radar_display.get_mut() = Default::default();
        self.radar_flash_deadline = 25;
        self.position_fixed = self.spawn.map(|n| (n * 65536.) as i32);
        self.heading_fixed = 90 << 16;
        self.flow_last_contact = None;
        self.speed_byte = 0;
        self.turn_ramp = 0;
        self.encounter = Encounter {
            enabled: self.encounter.enabled,
            ..Encounter::default()
        };
        self.encounter.health =
            crate::recovered::wave_refill(settings[1] as i8, settings[2] as i8).0;
        self.enemy_shots.clear();
        self.audio_events.clear();
        self.shots.clear();
        self.impacts.clear();
        self.fired = 0;
        self.combat_counters = crate::recovered::CombatCounters::default();
        self.completion = None;
        self.damage_grace_start = 75;
        self.acid_deadline = 0;
        self.ice_sound_deadline = 0;
        self.mud_sound_deadline = 0;
        self.last_fire_attempt = 0;
        self.last_weapon_selection = 0;
        self.optional_weapons = self.initial_optional_weapons;
        self.targets = self.initial_targets.clone();
        self.type31_frame = 0;
        self.type31_timer = -1;
        self.destroyed = 0;
        self.enemy_clear_award = 0;
        self.enemy_clear_awarded = false;
    }
    /// Original local life/refill and spawn-search rules, with a provisional RNG.
    /// Preserve the running arena, score, flags, enemies, shots, and tick clock.
    pub(super) fn respawn_player(&mut self) {
        let settings = self.vehicle_settings;
        let (health, lives, ammo, game_over, respawn) = crate::recovered::local_respawn(
            true,
            self.encounter.health,
            self.mission.lives,
            self.ammo,
            0,
            settings[1] as i8,
            settings[2] as i8,
        );
        self.mission.lives = lives;
        if game_over || respawn {
            self.mission.deaths += 1;
        }
        if game_over {
            self.encounter.status = "lost";
            return;
        }
        if !respawn {
            return;
        }
        // Recovered search ordering and clearance. Xorshift supplies reproducible
        // host Random words; it is NOT the original Mac OS Random generator.
        let mut seed = self.spawn_random;
        let mut random = std::iter::from_fn(|| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            Some((seed >> 16) as i16)
        });
        let mut query = self
            .collision_models
            .as_ref()
            .and_then(|models| crate::collision_preview::Query::from_game(self, models));
        let mut positions = [[0_i32; 2]; 12];
        let mut flags = [[0_u8; 2]; 12];
        let mut ids = [-1_i16; 12];
        ids[0] = 1;
        flags[0] = [1, 0];
        for (slot, target) in self.targets.iter().take(11).enumerate() {
            ids[slot + 1] = slot as i16 + 2;
            flags[slot + 1] = [u8::from(target.health > 0), 1];
            positions[slot + 1] = [
                (target.item.x * 65536.) as i32,
                (target.item.z * 65536.) as i32,
            ];
        }
        let result = crate::recovered::normal_spawn_search(0, 6, 1024, &mut random, |pose| {
            positions[0] = [pose[0], pose[1]];
            let participant =
                crate::recovered::spawn_participant_blocked(0, 0, 0, &flags, &ids, &positions)
                    .unwrap_or(true);
            let collision = query
                .as_mut()
                .and_then(|q| q.player_spawn_blocked(pose))
                .unwrap_or_else(|| {
                    !self.free_at(
                        pose[0] as f32 / 65536.,
                        pose[1] as f32 / 65536.,
                        (90. - pose[2] as f32 / 65536.).to_radians(),
                    )
                });
            [u8::from(collision), u8::from(participant)]
        });
        self.spawn_random = seed;
        let Some(result) = result else {
            self.encounter.status = "no_spawn";
            return;
        };
        // Original exhaustion retains the final candidate, even if blocked.
        let position = [
            result.pose[0] as f32 / 65536.,
            result.pose[1] as f32 / 65536.,
        ];
        self.tank.surface = "unavailable";
        self.encounter.health = health;
        self.radar_display.get_mut().restore();
        self.encounter.hit_flash = 0;
        self.ammo = ammo;
        self.tank.x = position[0];
        self.tank.z = position[1];
        self.tank.heading = (90. - result.pose[2] as f32 / 65536.)
            .to_radians()
            .rem_euclid(std::f32::consts::TAU);
        self.tank.speed = 0.;
        self.tank.blocked = !result.clear;
        self.position_fixed = [result.pose[0], result.pose[1]];
        self.heading_fixed = result.pose[2];
        self.flow_last_contact = None;
        self.speed_byte = 0;
        self.turn_ramp = 0;
        self.last_fire_attempt = self.tank.ticks as i32;
        self.vertical = Default::default();
    }
    fn respawn_enemies(&mut self) {
        use crate::recovered::{self, SpawnContext, SpawnGeometry};
        // Mission plans cap enemy slots at eleven; slot zero is the local player.
        if self.targets.len() > 11 {
            return;
        }
        for index in 0..self.targets.len() {
            if self.targets[index].health > 0 || self.targets[index].retired {
                continue;
            }
            let mut flags = [[0_u8; 3]; 12];
            let mut lives = [0_i16; 12];
            let mut ids = [-1_i16; 12];
            let mut positions = [[0_i32; 2]; 12];
            flags[0] = [1, 0, 6];
            ids[0] = 1;
            positions[0] = self.position_fixed;
            for (i, target) in self.targets.iter().enumerate() {
                flags[i + 1] = [u8::from(!target.retired), 1, target.item.kind as u8];
                lives[i + 1] = target.lives;
                ids[i + 1] = i as i16 + 2;
                positions[i + 1] = [
                    (target.item.x * 65536.) as i32,
                    (target.item.z * 65536.) as i32,
                ];
            }
            let contact_flags = flags.map(|f| [f[0], f[1]]);
            let convert = |item: &crate::scene::Item, participant| SpawnGeometry {
                kind: item.kind as i8,
                pose: [
                    (item.x * 65536.) as i32,
                    (item.z * 65536.) as i32,
                    item.native_heading(participant),
                ],
            };
            let mut geometry: Vec<_> = self
                .placement_items
                .iter()
                .map(|item| convert(item, false))
                .collect();
            geometry.push(SpawnGeometry {
                kind: 6,
                pose: [
                    self.position_fixed[0],
                    self.position_fixed[1],
                    self.heading_fixed,
                ],
            });
            geometry.extend(self.targets.iter().map(|t| convert(&t.item, true)));
            let context = SpawnContext {
                enemy: 1,
                kind: self.targets[index].item.kind as i8,
                extent: 1024,
                mode: 0,
                team: -1,
                object_id: (self.placement_items.len() + index + 2) as i16,
            };
            let mut query = self
                .collision_models
                .as_ref()
                .and_then(|models| crate::collision_preview::Query::from_game(self, models));
            let query_id = (self.items.len() + index + 2) as i16;
            let mut cursor = self.flag_spawn_cursor;
            let mut seed = self.spawn_random;
            let mut random = std::iter::from_fn(|| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                Some((seed >> 16) as i16)
            });
            // The original places the dead participant before consuming its life.
            // Preserve that Random/cursor ordering even when it will retire.
            let result = recovered::spawn_search(
                &context,
                &mut geometry,
                &mut cursor,
                &mut random,
                |heading| {
                    recovered::movement_direction(heading, &self.unit_table).unwrap_or([0, 0])
                },
                |pose| {
                    positions[index + 1] = [pose[0], pose[1]];
                    let participant = recovered::spawn_participant_blocked(
                        index + 1,
                        0,
                        0,
                        &contact_flags,
                        &ids,
                        &positions,
                    )
                    .unwrap_or(true);
                    let collision = query
                        .as_mut()
                        .and_then(|q| {
                            q.entity_spawn_blocked(
                                pose,
                                query_id,
                                context.kind as u8,
                                (index + 2) as i8,
                            )
                        })
                        .unwrap_or_else(|| {
                            !self.enemy_pose_free(
                                index,
                                pose[0] as f32 / 65536.,
                                pose[1] as f32 / 65536.,
                                (90. - pose[2] as f32 / 65536.).to_radians(),
                            )
                        });
                    [u8::from(collision), u8::from(participant)]
                },
            );
            self.spawn_random = seed;
            self.flag_spawn_cursor = cursor;
            let retire =
                recovered::consume_enemy_life(index + 1, &flags, &mut lives, 0).unwrap_or(true);
            for (i, target) in self.targets.iter_mut().enumerate() {
                target.lives = lives[i + 1];
            }
            let target = &mut self.targets[index];
            if retire {
                target.retired = true;
                continue;
            }
            let Some(result) = result else {
                // Invalid preview geometry is explicit rather than silently losing a life.
                self.encounter.status = "no_spawn";
                return;
            };
            target.item.x = result.pose[0] as f32 / 65536.;
            target.item.z = result.pose[1] as f32 / 65536.;
            target.item.heading = (90. - result.pose[2] as f32 / 65536.)
                .to_radians()
                .rem_euclid(std::f32::consts::TAU);
            // CODE 23 +0x0684 caps the refill at nine health points.
            target.health = target.max_health.min(9);
            target.fire.timer = -1;
            target.navigation.reset();
        }
        if !self.enemy_clear_awarded
            && !self.targets.is_empty()
            && self.targets.iter().all(|t| t.retired)
            && let Some(bonus) = self.enemy_clear_bonus
        {
            self.enemy_clear_awarded = true;
            self.enemy_clear_award = bonus;
            self.queue_sound_request(0x0001002c);
            self.award_points(bonus);
        }
    }
    pub fn free(&self, x: f32, z: f32) -> bool {
        self.free_at(x, z, self.tank.heading)
    }
    fn free_at(&self, x: f32, z: f32, heading: f32) -> bool {
        // Retain the preview arena-edge policy. Supported objects use original
        // collision geometry below; unsupported models retain the footprint fallback.
        let tank = Self::tank_outline(x, z, heading);
        if tank
            .iter()
            .flatten()
            .any(|v| !(0. ..=ARENA_SIZE).contains(v))
        {
            return false;
        }
        if let Some(blocked) = self
            .collision_models
            .as_ref()
            .and_then(|models| crate::collision_preview::Query::from_game(self, models))
            .and_then(|query| {
                query.player_movement_blocked(
                    [
                        (x * 65536.) as i32,
                        (z * 65536.) as i32,
                        ((90. - heading.to_degrees()).rem_euclid(360.) * 65536.).round() as i32,
                    ],
                    self.vertical.vertical,
                )
            })
        {
            return !blocked;
        }
        self.items
            .iter()
            .map(|item| (item, false))
            .chain(
                self.targets
                    .iter()
                    .filter(|t| t.health > 0)
                    .map(|t| (&t.item, true)),
            )
            .filter(|(item, _)| {
                item.kind != 12
                    && !(matches!(item.kind, 16 | 17 | 18 | 23 | 24 | 36 | 37 | 39)
                        && self.tank.surface != "unavailable")
            })
            .all(|(item, participant)| {
                self.vertical.vertical.wrapping_abs() > 60 * 65536
                    || !crate::scene::footprints_overlap(&tank, &item.footprint(participant))
            })
    }
    fn tank_outline(x: f32, z: f32, heading: f32) -> [[f32; 2]; 8] {
        let (s, c) = heading.sin_cos();
        [
            [-9., -13.],
            [0., -17.],
            [9., -13.],
            [9., -10.],
            [5., 11.],
            [0., 22.],
            [-5., 11.],
            [-9., -10.],
        ]
        .map(|p| [x + p[0] * c + p[1] * s, z - p[0] * s + p[1] * c])
    }
    /// The host supplies elapsed seconds, never a variable physics timestep.
    /// Discard excess wall time after stalls; a frame can run at most six ticks.
    pub fn advance(&mut self, seconds: f64, input: Input) {
        self.audio_events.clear();
        if !seconds.is_finite() || seconds <= 0. {
            return;
        }
        self.accumulator += seconds.min(0.1);
        while self.accumulator + 1e-10 >= STEP {
            self.step(input);
            self.accumulator = (self.accumulator - STEP).max(0.);
        }
    }
    fn apply_acid_contact(&mut self, attacker: i8) {
        let now = self.tank.ticks as i32;
        let Some(contact) = crate::recovered::acid_contact(
            now,
            self.acid_deadline,
            0,
            self.acid_damage,
            attacker,
            1,
        ) else {
            return;
        };
        self.acid_deadline = contact.deadline;
        self.queue_sound_request(contact.sound.1);
        if self.encounter.health <= 0
            || !self.combat_counters.damage(
                crate::recovered::DamageCounterEvent {
                    local: 1,
                    attacker,
                    victim: 1,
                    geometry_id: 1,
                    mode: 0,
                    now: now.wrapping_add(75),
                },
                &mut self.damage_grace_start,
            )
        {
            return;
        }
        if !self.infinite_health {
            self.encounter.health =
                crate::recovered::subtract_health(self.encounter.health, contact.damage);
        }
        if let Some(event) = crate::radar::damage_disturbance(
            1,
            1,
            self.encounter.health,
            self.selected_difficulty,
            now,
        ) {
            self.radar_display.get_mut().disturb(&event);
            for (_, request) in event.sounds {
                self.queue_sound_request(request);
            }
        }
        self.encounter.hit_flash = 18;
        let sounds = crate::recovered::damage_sounds(
            true,
            u8::from(self.encounter.health == 0),
            if self.flags.enabled {
                self.mission.lives
            } else {
                1
            },
        );
        for (_, request) in sounds.requests {
            self.queue_sound_request(request);
        }
        self.start_player_death_if_needed();
    }
    pub fn pickup_color(&self, item: &crate::scene::Item) -> Option<u8> {
        let pickup = self
            .pickups
            .iter()
            .find(|pickup| std::ptr::eq(&pickup.item, item))?;
        crate::radar::pickup_color(pickup.subtype, &self.color_bindings.as_ref()?.effects)
    }
    fn collect_dump(&mut self, index: usize) {
        let pickup = &self.pickups[index];
        let pose: [i32; 3] = std::array::from_fn(|axis| {
            i32::from_be_bytes(pickup.record[axis * 4..axis * 4 + 4].try_into().unwrap())
        });
        let effect = crate::recovered::dump_effect(&crate::recovered::DumpEffectInput {
            subtype: pickup.subtype,
            health: self.encounter.health,
            ammo: self.ammo,
            shields: self.vehicle_settings[1] as i8,
            ammo_setting: self.vehicle_settings[2] as i8,
            vertical: self.vertical.vertical,
            local_energy: self.flight_energy,
            mode: 0,
        });
        self.pickups_collected = self.pickups_collected.wrapping_add(1);
        self.encounter.health = effect.health;
        self.ammo = effect.ammo;
        self.flight_energy = effect.local_energy;
        self.queue_sound_request(0x1fc1c);
        if let Some((_, request)) = effect.sound {
            self.queue_sound_request(request);
        }
        if let Some((false, height)) = effect.effect {
            self.pending_shields.push(crate::pickups::ShieldRequest {
                position: pose,
                height,
                timestamp: self.tank.ticks as i32,
            });
        }
        // CODE 31 +10f6 uses the 108-unit Manhattan prefilter over live records.
        // Entity order, dead-record representation, and fixed-step scheduling remain adapters.
        let points: Vec<[i32; 2]> = std::iter::once(self.position_fixed)
            .chain(
                self.items
                    .iter()
                    .filter(|i| i.kind != 14)
                    .map(|i| [(i.x * 65536.) as i32, (i.z * 65536.) as i32]),
            )
            .chain(
                self.targets
                    .iter()
                    .filter(|t| !t.retired)
                    .map(|t| [(t.item.x * 65536.) as i32, (t.item.z * 65536.) as i32]),
            )
            .chain(
                self.pickups
                    .iter()
                    .enumerate()
                    .filter(|(n, _)| *n != index)
                    .map(|(_, p)| [(p.item.x * 65536.) as i32, (p.item.z * 65536.) as i32]),
            )
            .collect();
        let mut seed = self.pickup_random;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as i16
        };
        let subtype = crate::recovered::dump_subtype(
            random(),
            [
                self.dump_weights[0],
                self.dump_weights[1],
                self.dump_weights[2],
            ],
        );
        let (position, _) = crate::recovered::relocate_dump(pose, 1024, &mut random, |p| {
            !points
                .iter()
                .any(|other| crate::recovered::contact_near([p[0], p[1]], *other))
        })
        .expect("fixed nonzero arena extent");
        self.pickup_random = seed;
        let pickup = &mut self.pickups[index];
        pickup.subtype = subtype;
        pickup.record[0x1c..0x1e].copy_from_slice(&subtype.to_be_bytes());
        for (axis, value) in position.iter().enumerate() {
            pickup.record[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_be_bytes());
        }
        pickup.item.x = position[0] as f32 / 65536.;
        pickup.item.z = position[1] as f32 / 65536.;
    }
    /// Ordinary solid contacts only; special handlers and unsupported models use the existing adapter.
    fn try_recovered_motion(&mut self, turn: i16, throttle: i16, adjustment: i8) -> bool {
        use crate::recovered;
        if adjustment != 0 || self.vehicle_settings[3] != 0 {
            return false;
        }
        let Some(query) = self
            .collision_models
            .as_ref()
            .and_then(|models| crate::collision_preview::Query::from_game(self, models))
        else {
            return false;
        };
        let mut motion = recovered::Type6Motion {
            pose: [
                self.position_fixed[0],
                self.position_fixed[1],
                self.heading_fixed,
            ],
            speed: self.speed_byte,
            ramp: self.turn_ramp,
            vertical: self.vertical.clone(),
        };
        let mut input = recovered::Type6Step {
            commands: [turn, throttle],
            limit: self.vehicle_settings[0] as i8,
            energy: self.flight_energy,
            turn_rate: 65536,
            now: self.tank.ticks as i32,
            elapsed: 1,
        };
        if recovered::type6_step(&mut motion, &input, &self.direction_table).is_none() {
            return false;
        }
        let Some(contact) = query.player_movement_contact(motion.pose, motion.vertical.vertical)
        else {
            return false;
        };
        if let Some((_, kind, _)) = contact
            && matches!(
                kind,
                5 | 12 | 13 | 16 | 17 | 18 | 19 | 21 | 23 | 24 | 30 | 35 | 36 | 37 | 39 | 41 | 45
            )
        {
            return false;
        }
        if let Some((id, _, other_height)) = contact
            && let Some((elapsed, sound)) =
                recovered::solid_response_request(0, id, motion.vertical.hold as u8, true, 1)
        {
            input.elapsed = elapsed;
            if recovered::type6_step(&mut motion, &input, &self.direction_table).is_none() {
                return false;
            }
            let mut record = [0; 128];
            record[0x28..0x2c].copy_from_slice(&motion.vertical.vertical.to_be_bytes());
            record[0x5a] = motion.vertical.hold as u8;
            record[0x22] = motion.speed as u8;
            recovered::contact_rebound(&mut record, id == -999, other_height);
            motion.speed = record[0x22] as i8;
            motion.vertical.vertical = i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap());
            motion.vertical.hold = record[0x5a] as i8;
            if let Some((_, request)) = sound {
                self.queue_sound_request(request);
            }
        }
        self.position_fixed = [motion.pose[0], motion.pose[1]];
        self.heading_fixed = motion.pose[2];
        self.speed_byte = motion.speed;
        self.turn_ramp = motion.ramp;
        self.vertical = motion.vertical;
        self.tank.x = self.position_fixed[0] as f32 / 65536.;
        self.tank.z = self.position_fixed[1] as f32 / 65536.;
        self.tank.heading = (90. - self.heading_fixed as f32 / 65536.)
            .to_radians()
            .rem_euclid(std::f32::consts::TAU);
        self.tank.blocked = contact.is_some();
        self.ice_sound_deadline = self.tank.ticks as i32;
        self.mud_sound_deadline = self.tank.ticks as i32;
        true
    }

    /// Cannon, Scattershot, and Seekers share basic launch geometry.
    fn fire_player_basic_variant(&mut self, weapon: u8, now: i32) {
        use crate::recovered;
        let cost = if weapon == 0 {
            self.primary_ammo_cost
        } else {
            self.weapon_ammo_costs[weapon as usize]
        };
        let result = recovered::local_weapon_attempt(recovered::LocalWeaponAttempt {
            now,
            last_attempt: self.last_fire_attempt,
            disabled: 0,
            weapon,
            ammo: self.ammo,
            cost,
            health: if self.encounter.enabled {
                self.encounter.health
            } else {
                1
            },
            inhibit: 0,
            height: self.vertical.vertical,
        })
        .expect("basic variants use original weapon slots");
        self.last_fire_attempt = result.timestamp;
        let eligible = result.request.is_some();
        let empty_sound = result.empty_sound;
        let admitted = if eligible {
            // Fresh single-player settings select the twenty-shot limit.
            // The Vec-backed preview still has no original pool allocation.
            // The allocator result is an admission token, not a geometry index.
            let admission = recovered::admit_weapon(
                recovered::WeaponAdmission {
                    owner: 1,
                    local: 1,
                    team: 0,
                    enemy: 0,
                    active_shots: self
                        .shots
                        .iter()
                        .filter(|s| !s.retiring)
                        .count()
                        .min(i16::MAX as usize) as i16,
                    shot_limit: recovered::concurrent_shot_limit(1),
                    ammo: self.ammo,
                    enemy_cost: cost,
                    player_cost: cost,
                    free_ammo: 0,
                },
                || 1,
            );
            self.ammo = admission.ammo;
            for (_, request) in admission.sounds {
                self.queue_sound_request(request as u32);
            }
            admission.slot.is_some()
        } else {
            false
        };
        if admitted {
            let (_, shot_heading) = recovered::weapon_initial_heading(
                weapon as i8,
                self.heading_fixed,
                || {
                    self.spawn_random ^= self.spawn_random << 13;
                    self.spawn_random ^= self.spawn_random >> 17;
                    self.spawn_random ^= self.spawn_random << 5;
                    (self.spawn_random >> 16) as i16
                },
                || None,
            );
            let unit = recovered::movement_direction(shot_heading, &self.unit_table).unwrap();
            let (mut position_fixed, mut deadline) =
                recovered::launch_basic_shot(self.position_fixed, unit, self.tank.ticks as i32, 75);
            let mut lifecycle_record = None;
            let missile = if matches!(weapon, 1 | 10) {
                let mut record = [0; 128];
                let height = i32::from(result.request.unwrap().1) << 16;
                for (offset, value) in [
                    (0, self.position_fixed[0]),
                    (4, self.position_fixed[1]),
                    (8, shot_heading),
                    (0x28, height),
                ] {
                    record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                }
                recovered::initialize_weapon_record(
                    &mut record,
                    recovered::WeaponRecordInput {
                        weapon: weapon as i8,
                        now,
                        lifetime: 75,
                        owner: 1,
                        source_kind: 6,
                        owner_field_5c: 0,
                        direction: unit,
                    },
                    || unreachable!("missile launch has no Random call"),
                );
                let long =
                    |offset| i32::from_be_bytes(record[offset..offset + 4].try_into().unwrap());
                position_fixed = [long(0), long(4)];
                deadline = long(0x16);
                if weapon == 1 {
                    recovered::finalize_weapon_record(&mut record, 1, now, 1, &mut 0);
                    lifecycle_record = Some(record);
                    None
                } else {
                    Some(recovered::MissileGuidance {
                        pose: [long(0), long(4), long(8)],
                        height: long(0x28),
                        vertical_velocity: long(0x24),
                    })
                }
            } else {
                None
            };
            // Original launch offset, deadline, and initial update timestamp.
            self.shots.push(Shot {
                lifecycle_record,
                retiring: false,
                missile,
                weapon,
                enemy_owner: None,
                bouncing_record: None,
                height: 4.
                    + missile.map_or_else(
                        || {
                            lifecycle_record.map_or(self.vertical.vertical, |r| {
                                i32::from_be_bytes(r[0x28..0x2c].try_into().unwrap())
                            })
                        },
                        |m| m.height,
                    ) as f32
                        / 524288.,
                x: position_fixed[0] as f32 / 65536.,
                z: position_fixed[1] as f32 / 65536.,
                heading: if weapon == 0 {
                    self.tank.heading
                } else {
                    (90. - shot_heading as f32 / 65536.)
                        .to_radians()
                        .rem_euclid(std::f32::consts::TAU)
                },
                life: deadline.wrapping_sub(now) as u32,
                position_fixed,
                direction_fixed: unit.map(|v| v.wrapping_mul(6)),
                heading_fixed: shot_heading,
                deadline,
                last_update: now,
            });
            self.fired += 1;
            self.combat_counters.launch(1, 1);
            if let Some(request) = recovered::weapon_fire_sound(weapon as i8) {
                self.queue_sound_request(request);
            }
        }
        if empty_sound {
            self.queue_sound_request(0x30006);
        }
    }

    /// Participant adapter for the original area selector. Flying-enemy heights remain zero.
    fn apply_projectile_blast(&mut self, source: [u8; 128], now: i32) {
        use crate::recovered;
        let mut records = vec![source, [0; 128]];
        let mut participants = [[0; 256]; 12];
        for (i, target) in self.targets.iter().take(11).enumerate() {
            let mut record = [0; 128];
            record[..4].copy_from_slice(&((target.item.x * 65536.) as i32).to_be_bytes());
            record[4..8].copy_from_slice(&((target.item.z * 65536.) as i32).to_be_bytes());
            record[0x14] = if target.health <= 0 {
                10
            } else {
                target.item.kind as u8
            };
            record[0x1d] = target.fire.weapon as u8;
            records.push(record);
            participants[i + 1][5] = 1;
            participants[i + 1][0x88] = u8::from(!target.retired);
            participants[i + 1][0x56..0x58].copy_from_slice(&((i + 3) as i16).to_be_bytes());
        }
        let Some(requests) = recovered::projectile_area_requests(
            &records,
            &participants,
            1,
            now,
            &self.weapon_damage,
        ) else {
            return;
        };
        let mut awards = Vec::new();
        for request in requests {
            let Some(target) = usize::try_from(request.victim - 2)
                .ok()
                .and_then(|i| self.targets.get_mut(i))
            else {
                continue;
            };
            let was_alive = target.health > 0;
            target.health = if target.vulnerable {
                recovered::subtract_health(target.health, request.damage)
            } else {
                target.health.max(0)
            };
            self.damage_grace_start = 0;
            self.combat_counters.damage(
                recovered::DamageCounterEvent {
                    local: 1,
                    attacker: request.attacker,
                    victim: request.victim,
                    geometry_id: request.victim as i16 + 1,
                    mode: 0,
                    now,
                },
                &mut self.damage_grace_start,
            );
            let killed = target.health == 0;
            self.audio_events.extend(
                recovered::damage_sounds(false, u8::from(killed), target.lives)
                    .requests
                    .into_iter()
                    .filter_map(|(_, request)| recovered::sound_request_id(request)),
            );
            if was_alive && killed {
                target.deaths += 1;
                self.destroyed += 1;
                if let Some((points, _)) = recovered::combat_score_award(
                    target.item.kind as u8,
                    target.kill_score,
                    [request.attacker, request.victim],
                    [0, 1],
                    1,
                    0,
                ) {
                    awards.push(points);
                }
            }
        }
        for points in awards {
            self.award_points(points);
        }
        let coordinate = |offset| {
            i32::from_be_bytes(source[offset..offset + 4].try_into().unwrap()) as f32 / 65536.
        };
        self.impacts.push(Impact {
            x: coordinate(0),
            z: coordinate(4),
            life: 36,
        });
    }

    pub fn step(&mut self, input: Input) {
        if self.encounter.enabled && self.encounter.status != "playing" {
            return;
        }
        self.advance_type31();
        use crate::recovered;
        let dying = self.death_transition.is_some();
        // Fixed-step adapter scheduling: energy precedes contact effects and hover.
        self.flight_energy = recovered::energy_clock(
            self.flight_energy,
            2000,
            false,
            (self.tank.ticks as i32).wrapping_add(1),
            self.tank.ticks as i32,
        )
        .0;

        let mut acid_source = None;
        let input = if dying { Input::default() } else { input };
        if !dying {
            let command = |x: f32| {
                if !x.is_finite() || x == 0. {
                    0_i16
                } else if x > 0. {
                    1
                } else {
                    -1
                }
            };
            // Original query and speed adjustment; sampling at the start of each
            // preview tick is not yet the original caller's scheduling.
            let surface = self
                .collision_models
                .as_ref()
                .and_then(|m| crate::collision_preview::Query::from_game(self, m))
                .and_then(|q| q.surface_contact());
            if let Some(Some((kind @ (36 | 37), _, _))) = surface {
                let effect = recovered::transit_contact(
                    kind,
                    self.vertical.transition,
                    self.vertical.vertical,
                    self.flight_energy,
                )
                .unwrap();
                self.vertical.transition = effect.transition;
                self.flight_energy = effect.energy;
                if let Some((_, request)) = effect.sound {
                    self.queue_sound_request(request);
                }
            }
            if self.flags.enabled
                && let Some(Some((5, _, id))) = surface
            {
                let start = self.items.len() + self.targets.len() + 2;
                if let Some(index) = (id as usize)
                    .checked_sub(start)
                    .filter(|i| *i < self.pickups.len())
                {
                    self.collect_dump(index);
                }
            }
            if self.flags.enabled {
                let pickup = match surface {
                    Some(Some((12, _, id))) => usize::try_from(id - 2).ok(),
                    None => {
                        // Explicit preview fallback for unsupported model builders.
                        let outline =
                            Self::tank_outline(self.tank.x, self.tank.z, self.tank.heading);
                        self.items.iter().position(|i| {
                            i.kind == 12
                                && self.vertical.vertical < 140 * 65536
                                && crate::scene::footprints_overlap(&outline, &i.footprint(false))
                        })
                    }
                    _ => None,
                };
                if let Some(index) = pickup {
                    self.collect_flag(index);
                    if self.encounter.status == "won" {
                        return;
                    }
                }
            }
            if self.encounter.enabled
                && let Some(Some((16 | 23, _, id))) = surface
            {
                let target_start = self.items.len() + 2;
                acid_source = Some(if id as usize >= target_start {
                    (id as usize - target_start + 2) as i8
                } else {
                    0
                });
            }
            self.tank.surface = match surface {
                Some(Some((16 | 23, _, _))) => "acid",
                Some(Some((17, _, _))) => "ice",
                Some(Some((18 | 24, _, _))) => "mud",
                Some(Some((39, _, _))) => "flow",
                Some(Some((36, _, _))) => "up transit",
                Some(Some((37, _, _))) => "down transit",
                Some(_) => "none",
                None => "unavailable",
            };
            let mut adjustment = 0;
            if let Some(Some((kind, _, _))) = surface {
                let deadline = if kind == 17 {
                    self.ice_sound_deadline
                } else {
                    self.mud_sound_deadline
                };
                if let Some(contact) =
                    recovered::handling_contact(kind, self.tank.ticks as i32, deadline, 5)
                {
                    adjustment = contact.adjustment;
                    if kind == 17 {
                        self.ice_sound_deadline = contact.deadline;
                    } else {
                        self.mud_sound_deadline = contact.deadline;
                    }
                    if let Some((_, request)) = contact.sound {
                        self.queue_sound_request(request);
                    }
                }
            }
            let throttle = if input.brake {
                0
            } else {
                command(input.throttle)
            };
            // Render heading zero faces +Z. Original table heading 90 faces +Z.
            let turn = if adjustment < 0 && throttle != 0 {
                0
            } else {
                -command(input.steer)
            };
            if !self.try_recovered_motion(turn, throttle, adjustment) {
                recovered::update_vertical(
                    &mut self.vertical,
                    self.flight_energy,
                    self.speed_byte,
                    self.vehicle_settings[0] as i8,
                    [self.tank.ticks as i32, 1],
                )
                .expect("vehicle speed setting must be nonzero");
                let old_heading = self.heading_fixed;
                (self.heading_fixed, self.turn_ramp) = recovered::turn_object(
                    self.heading_fixed,
                    self.turn_ramp,
                    6,
                    turn,
                    1,
                    turn as i32 * 65536,
                );
                let render_heading = |h: i32| {
                    ((90. - h as f32 / 65536.).to_radians()).rem_euclid(std::f32::consts::TAU)
                };
                if !self.free_at(self.tank.x, self.tank.z, render_heading(self.heading_fixed)) {
                    self.heading_fixed = old_heading;
                }
                self.tank.heading = render_heading(self.heading_fixed);
                let direction =
                    recovered::movement_direction(self.heading_fixed, &self.direction_table)
                        .unwrap();
                // Current vehicle speed includes earned campaign upgrades.
                // Default Coast 0 does not request RNG; other presets are not exposed yet.
                let settings = self.vehicle_settings;
                let (next, result) = recovered::move_type6_planar(
                    self.position_fixed,
                    direction,
                    recovered::SpeedUpdate {
                        speed: self.speed_byte,
                        command: throttle,
                        elapsed: 1,
                        limit: settings[0] as i8,
                        coast: settings[3] as i8,
                        adjustment,
                        mode: 0,
                        random: 0,
                    },
                );
                self.speed_byte = result.speed;
                if result.reset_timestamps {
                    self.ice_sound_deadline = self.tank.ticks as i32;
                    self.mud_sound_deadline = self.tank.ticks as i32;
                }
                let position = next.map(|n| n as f32 / 65536.);
                let was_blocked = self.tank.blocked;
                self.tank.blocked = !self.free(position[0], position[1]);
                if self.tank.blocked && !was_blocked {
                    self.audio_events.push(10007);
                }
                if !self.tank.blocked {
                    self.position_fixed = next;
                    self.tank.x = position[0];
                    self.tank.z = position[1];
                } else {
                    // Provisional collision response; original response still pending.
                    self.speed_byte = 0;
                }
            }
            // Original flow displacement; the preview retains its solid-response guard
            // instead of claiming the original spatial-update/collision scheduling.
            if let Some(Some((39, field_heading, _))) = surface {
                let now = self.tank.ticks as i32;
                let previous = self
                    .flow_last_contact
                    .unwrap_or_else(|| now.wrapping_sub(1));
                let heading = recovered::wrap_angle(field_heading.wrapping_sub(90 << 16));
                let direction = recovered::movement_direction(heading, &self.unit_table)
                    .unwrap()
                    .map(|v| v.wrapping_mul(6));
                let pushed = recovered::flow_field_displacement(
                    self.position_fixed,
                    direction,
                    now,
                    previous,
                );
                let p = pushed.map(|v| v as f32 / 65536.);
                if self.free(p[0], p[1]) {
                    self.position_fixed = pushed;
                    self.tank.x = p[0];
                    self.tank.z = p[1];
                }
                self.flow_last_contact = Some(now);
            } else {
                self.flow_last_contact = None;
            }
            self.tank.speed = self.speed_byte as f32 * 15.;
            self.tank.heading = ((90. - self.heading_fixed as f32 / 65536.).to_radians())
                .rem_euclid(std::f32::consts::TAU);
        }
        self.tank.ticks += 1;
        let vertices = recovered::dump_animation(((self.tank.ticks as i32) % 60) as i16);
        for pickup in &mut self.pickups {
            if let Some(shape) = &mut pickup.item.shape {
                shape.packed_vertices = vertices.to_vec();
                shape.vertices = vertices
                    .iter()
                    .map(|v| crate::formats::Vertex {
                        x: v[0] as f32 / 8.,
                        y: v[1] as f32 / 8.,
                        z: v[2] as f32 / 8.,
                    })
                    .collect();
            }
        }

        (self.radar_flash_deadline, self.radar_flash) = crate::radar::advance_flash(
            self.tank.ticks as i32,
            self.radar_flash_deadline,
            self.radar_flash,
        );

        if let Some(attacker) = acid_source {
            self.apply_acid_contact(attacker);
            if self.death_transition.is_some() {
                return;
            }
        }
        self.finish_player_death_if_due();
        if self.encounter.enabled && self.encounter.status != "playing" {
            return;
        }
        for impact in &mut self.impacts {
            impact.life = impact.life.saturating_sub(1);
        }
        self.impacts.retain(|p| p.life > 0);
        // Preview player is alive, with the original firing-inhibit flag clear.
        let now = self.tank.ticks as i32;
        if recovered::primary_fire_due(now, self.last_fire_attempt, input.fire) {
            self.fire_player_basic_variant(0, now);
        }
        // The original tests optional fire once, then dispatches every Scattershot request.
        // Other selected weapons await their own projectile/effect integration.
        if matches!(self.optional_weapons.selected, 0 | 1 | 5 | 9 | 10) {
            for weapon in recovered::optional_weapon_fire(
                now,
                self.last_fire_attempt,
                i16::from(input.fire_optional),
                self.optional_weapons.selected,
            ) {
                self.fire_player_basic_variant(weapon as u8, now);
            }
        }
        if let Some((selected, timestamp, changed)) = recovered::optional_weapon_selection(
            now,
            self.last_weapon_selection,
            i16::from(input.select_weapon),
            self.optional_weapons.selected,
            &self.optional_weapons.available,
        ) {
            self.optional_weapons.selected = selected;
            self.last_weapon_selection = timestamp;
            if changed {
                self.queue_sound_request(0x10022);
            }
        }
        let mut query = self
            .collision_models
            .as_ref()
            .and_then(|models| crate::collision_preview::Query::from_game(self, models));
        if self.collision_models.is_some() {
            self.collision_mode = if query.is_some() {
                "recovered query / preview records"
            } else {
                "preview bounds (query capacity)"
            };
        }
        // Match the existing participant adapter: player 1, then eleven stable target slots.
        // Keep dead slots in place so later participants do not change identity.
        let mut seeker_targets = [recovered::AiTargetCandidate {
            team: 0,
            owner: 0,
            active: 0,
            health: 0,
            kind: 0,
            pose: [0; 3],
        }; 12];
        for (candidate, target) in seeker_targets[1..].iter_mut().zip(&self.targets) {
            *candidate = recovered::AiTargetCandidate {
                team: 1,
                owner: 1,
                active: u8::from(!target.retired),
                health: target.health,
                kind: target.item.kind as u8,
                pose: [
                    (target.item.x * 65536.) as i32,
                    (target.item.z * 65536.) as i32,
                    recovered::wrap_angle(
                        ((90. - target.item.heading.to_degrees()).rem_euclid(360.) * 65536.) as i32,
                    ),
                ],
            };
        }
        let mut kill_awards = Vec::new();
        let mut shots = std::mem::take(&mut self.shots);
        for shot in &mut shots {
            if let Some(record) = &mut shot.lifecycle_record {
                for (offset, value) in [
                    (0, shot.position_fixed[0]),
                    (4, shot.position_fixed[1]),
                    (8, shot.heading_fixed),
                ] {
                    record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                }
                let action = recovered::projectile_lifecycle(record, now, &mut [0; 12]).unwrap();
                match action {
                    recovered::ProjectileLifecycleAction::Retire => {
                        shot.retiring = true;
                        shot.life = 60;
                        continue;
                    }
                    recovered::ProjectileLifecycleAction::None => {
                        continue;
                    }
                    recovered::ProjectileLifecycleAction::AreaEffectThenDelete => {
                        self.apply_projectile_blast(*record, now);
                        if let Some(q) = &mut query {
                            for (i, t) in self.targets.iter().enumerate() {
                                if t.health <= 0 {
                                    q.remove((q.target_start + i) as i16);
                                }
                            }
                        }
                        shot.life = 0;
                        continue;
                    }
                    recovered::ProjectileLifecycleAction::Delete => {
                        shot.life = 0;
                        continue;
                    }
                    recovered::ProjectileLifecycleAction::Move => {}
                }
            }
            if recovered::basic_shot_expired(now, shot.deadline) {
                shot.life = 0;
                continue;
            }
            shot.life = shot.deadline.wrapping_sub(now) as u32;
            let elapsed = recovered::projectile_elapsed(now, shot.last_update);
            shot.last_update = now;
            if matches!(shot.weapon, 9 | 10) && elapsed != 0 {
                // Earlier projectiles may have killed a candidate during this tick.
                for (candidate, target) in seeker_targets[1..].iter_mut().zip(&self.targets) {
                    candidate.health = target.health;
                }
                if shot.weapon == 9 {
                    shot.heading_fixed = recovered::seeker_heading(
                        1,
                        [
                            shot.position_fixed[0],
                            shot.position_fixed[1],
                            shot.heading_fixed,
                        ],
                        &seeker_targets,
                        |target, source| {
                            recovered::ai_aim_angle(
                                [target[0], target[1]],
                                source,
                                encounter::candidate_slope_angle,
                            )
                            .unwrap_or(0)
                        },
                    );
                } else if let Some(missile) = &mut shot.missile {
                    let adjusted = recovered::smart_missile_elapsed(elapsed, Some(6));
                    recovered::smart_missile_guidance(
                        missile,
                        1,
                        adjusted,
                        &seeker_targets,
                        &[0; 12],
                        |target, source| {
                            recovered::ai_aim_angle(
                                [target[0], target[1]],
                                source,
                                encounter::candidate_slope_angle,
                            )
                            .unwrap_or(0)
                        },
                    );
                    recovered::move_smart_missile(
                        missile,
                        adjusted,
                        &self.unit_table.map(|v| v.wrapping_mul(6)),
                    )
                    .expect("wrapped missile heading");
                    shot.heading_fixed = missile.pose[2];
                    shot.height = 4. + missile.height as f32 / 524288.;
                }
                shot.direction_fixed =
                    recovered::movement_direction(shot.heading_fixed, &self.unit_table)
                        .expect("wrapped Seeker heading")
                        .map(|v| v.wrapping_mul(6));
                shot.heading = (90. - shot.heading_fixed as f32 / 65536.)
                    .to_radians()
                    .rem_euclid(std::f32::consts::TAU);
            }
            let start = shot.position_fixed;
            let end = shot.missile.map_or_else(
                || recovered::move_basic_shot(start, shot.direction_fixed, elapsed),
                |m| [m.pose[0], m.pose[1]],
            );
            let damage = if shot.weapon == 10 {
                self.weapon_damage[10]
            } else {
                self.primary_damage
            };
            let ground_hit = shot.missile.is_some_and(|m| m.height <= 0);
            let raw_height = shot
                .missile
                .map_or(((shot.height - 4.) * 524288.) as i32, |m| m.height);
            if let Some(query) = &mut query {
                if let Some(hit) = query.hit_projectile(
                    start,
                    end,
                    shot.heading_fixed,
                    elapsed,
                    raw_height,
                    if shot.weapon == 10 { 34 } else { 9 },
                ) {
                    shot.position_fixed = end;
                    shot.x = end[0] as f32 / 65536.;
                    shot.z = end[1] as f32 / 65536.;
                    // Original selected-object exclusions precede damage and retirement.
                    // Keep the completed movement even when this contact is ignored.
                    if hit > 0
                        && query.object_kind(hit).is_some_and(|kind| {
                            recovered::projectile_object_contact(
                                if shot.weapon == 10 { 34 } else { 9 },
                                hit,
                                kind,
                                1,
                            ) == recovered::ProjectileObjectContact::Ignore
                        })
                    {
                        continue;
                    }
                    let boundary =
                        shot.x < 0. || shot.x > ARENA_SIZE || shot.z < 0. || shot.z > ARENA_SIZE;
                    let scenery_contact = hit > 0
                        && query.object_kind(hit).is_some_and(|kind| {
                            recovered::projectile_object_contact(9, hit, kind, 1)
                                == recovered::ProjectileObjectContact::RespondToObject
                        });
                    if (scenery_contact || (hit <= 0 && (boundary || ground_hit)))
                        && shot.retire_basic_contact(
                            end,
                            elapsed,
                            now,
                            &seeker_targets,
                            &self.unit_table,
                        )
                    {
                        continue;
                    }
                    if hit > 0 || boundary || ground_hit {
                        let target_index = usize::try_from(hit)
                            .ok()
                            .and_then(|id| id.checked_sub(query.target_start));
                        if recovered::projectile_impact_uses_area(shot.weapon as i16, 1, 0, 0)
                            && target_index
                                .and_then(|i| self.targets.get(i))
                                .is_some_and(|t| t.health > 0)
                        {
                            let mut record = shot.lifecycle_record.unwrap();
                            record[0..4].copy_from_slice(&end[0].to_be_bytes());
                            record[4..8].copy_from_slice(&end[1].to_be_bytes());
                            self.apply_projectile_blast(record, now);
                            for (i, t) in self.targets.iter().enumerate() {
                                if t.health <= 0 {
                                    query.remove((query.target_start + i) as i16);
                                }
                            }
                            shot.life = 0;
                            continue;
                        }
                        if let Some(target) = target_index.and_then(|i| self.targets.get_mut(i)) {
                            self.combat_counters.damage(
                                recovered::DamageCounterEvent {
                                    local: 1,
                                    attacker: 1,
                                    victim: 2,
                                    geometry_id: hit,
                                    mode: 0,
                                    now: now.wrapping_add(75),
                                },
                                &mut self.damage_grace_start,
                            );
                            let was_alive = target.health > 0;
                            if target.vulnerable {
                                target.health = recovered::subtract_health(target.health, damage);
                            }
                            self.audio_events.extend(
                                recovered::damage_sounds(
                                    false,
                                    u8::from(target.health == 0),
                                    target.lives,
                                )
                                .requests
                                .into_iter()
                                .filter_map(|(_, request)| recovered::sound_request_id(request)),
                            );
                            if was_alive && target.health == 0 {
                                target.deaths += 1;
                                if let Some((points, _)) = recovered::combat_score_award(
                                    target.item.kind as u8,
                                    target.kill_score,
                                    [1, 2],
                                    [0, 1],
                                    1,
                                    0,
                                ) {
                                    kill_awards.push(points);
                                }
                                self.destroyed += 1;
                                query.remove(hit);
                            }
                        }
                        self.impacts.push(Impact {
                            x: shot.x,
                            z: shot.z,
                            life: if target_index.is_some() { 36 } else { 18 },
                        });
                        shot.life = 0;
                    }
                    continue;
                }
                self.collision_mode = "preview bounds (invalid query input)";
            }
            // Provisional swept collision checks; exact fixed-point endpoint is retained.
            for substep in 1..=6 * elapsed {
                let sample: [i32; 2] = std::array::from_fn(|i| {
                    if shot.weapon == 10 {
                        start[i].wrapping_add(
                            (i64::from(end[i].wrapping_sub(start[i])) * i64::from(substep)
                                / i64::from(6 * elapsed)) as i32,
                        )
                    } else {
                        start[i].wrapping_add(
                            (shot.direction_fixed[i] as i64 * substep as i64 / 6) as i32,
                        )
                    }
                });
                shot.x = sample[0] as f32 / 65536.;
                shot.z = sample[1] as f32 / 65536.;
                let hit = ground_hit
                    || shot.x < 0.
                    || shot.x > ARENA_SIZE
                    || shot.z < 0.
                    || shot.z > ARENA_SIZE
                    || self.items.iter().any(|o| {
                        recovered::projectile_object_contact(9, 1, o.kind as u8, 1)
                            != recovered::ProjectileObjectContact::Ignore
                            && o.contains(shot.x, shot.z, 1., false)
                            && shot.height >= o.min[1]
                            && shot.height <= o.max[1]
                    });
                let target_hit = if hit {
                    None
                } else {
                    self.targets.iter().position(|t| {
                        t.health > 0
                            && recovered::projectile_object_contact(
                                9,
                                1,
                                t.item.kind as u8,
                                t.health,
                            ) != recovered::ProjectileObjectContact::Ignore
                            && t.item.contains(shot.x, shot.z, 1., true)
                            && shot.height >= t.item.min[1]
                            && shot.height <= t.item.max[1]
                    })
                };
                if recovered::projectile_impact_uses_area(shot.weapon as i16, 1, 0, 0)
                    && target_hit.is_some()
                {
                    let mut record = shot.lifecycle_record.unwrap();
                    record[0..4].copy_from_slice(&end[0].to_be_bytes());
                    record[4..8].copy_from_slice(&end[1].to_be_bytes());
                    self.apply_projectile_blast(record, now);
                    shot.life = 0;
                    break;
                }
                if let Some(index) = target_hit {
                    let target = &mut self.targets[index];
                    self.combat_counters.damage(
                        recovered::DamageCounterEvent {
                            local: 1,
                            attacker: 1,
                            victim: 2,
                            geometry_id: 1,
                            mode: 0,
                            now: now.wrapping_add(75),
                        },
                        &mut self.damage_grace_start,
                    );
                    let was_alive = target.health > 0;
                    // Borrowed participant subtraction arithmetic. Enemy damage routing remains provisional.
                    if target.vulnerable {
                        target.health = crate::recovered::subtract_health(target.health, damage);
                    }
                    self.audio_events.extend(
                        recovered::damage_sounds(false, u8::from(target.health == 0), target.lives)
                            .requests
                            .into_iter()
                            .filter_map(|(_, request)| recovered::sound_request_id(request)),
                    );
                    if was_alive && target.health == 0 {
                        target.deaths += 1;
                        if let Some((points, _)) = recovered::combat_score_award(
                            target.item.kind as u8,
                            target.kill_score,
                            [1, 2],
                            [0, 1],
                            1,
                            0,
                        ) {
                            kill_awards.push(points);
                        }
                        self.destroyed += 1;
                    }
                }
                if hit
                    && shot.retire_basic_contact(
                        end,
                        elapsed,
                        now,
                        &seeker_targets,
                        &self.unit_table,
                    )
                {
                    break;
                }
                if hit || target_hit.is_some() {
                    self.impacts.push(Impact {
                        x: shot.x,
                        z: shot.z,
                        life: if target_hit.is_some() { 36 } else { 18 },
                    });
                    shot.life = 0;
                    break;
                }
            }
            if !shot.retiring {
                shot.position_fixed = end;
            }
        }
        self.shots = shots;
        self.shots.retain(|p| p.life > 0);
        for points in kill_awards {
            self.award_points(points);
        }
        if self.flags.enabled && self.encounter.enabled {
            self.respawn_enemies();
        }
        if self.encounter.enabled {
            self.step_encounter();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::Obstacle;
    fn empty() -> Game {
        Game::new(World { obstacles: vec![] }).unwrap()
    }
    #[test]
    fn projectile_contacts_preserve_ignored_surfaces() {
        for (original_query, weapon) in [false, true]
            .into_iter()
            .flat_map(|query| [0, 3, 4, 5, 9].map(|weapon| (query, weapon)))
        {
            for kind in [0, 7, 23, 24, 39] {
                let mut g = empty();
                let world = World {
                    obstacles: vec![Obstacle {
                        kind,
                        h: 500,
                        v: 500,
                        facing: 0,
                    }],
                };
                let mut item = crate::scene::Catalog::default()
                    .instantiate(&world)
                    .remove(0);
                item.shape = None;
                item.min = [-20., 0., -20.];
                item.max = [20., 20., 20.];
                if matches!(kind, 7 | 23 | 24) {
                    g.add_target(item, 5).unwrap();
                    g.targets[0].item.x = 500.;
                    g.targets[0].item.z = 500.;
                } else {
                    g.items.push(item);
                }
                if original_query {
                    let mut models = crate::model_slots::ModelSlots::default();
                    models.state.current[6] = 1;
                    models.state.current[kind as usize] = 1;
                    models.geometry.insert(
                        1,
                        crate::model::PackedShape {
                            vertices: vec![],
                            dimensions: [40 << 16; 2],
                            candidate_radius: 30,
                        },
                    );
                    g.collision_models = Some(models);
                }
                g.shots.push(Shot {
                    lifecycle_record: None,
                    retiring: false,
                    missile: None,
                    weapon,
                    enemy_owner: None,
                    bouncing_record: None,
                    height: 4.,
                    x: 500.,
                    z: 500.,
                    heading: 0.,
                    life: 30,
                    position_fixed: [500 << 16, 500 << 16],
                    direction_fixed: [0, 6 << 16],
                    heading_fixed: 90 << 16,
                    deadline: 30,
                    last_update: 0,
                });
                g.step(Input::default());
                let ignored = matches!(kind, 23 | 24 | 39);
                assert_eq!(
                    g.shots.len(),
                    usize::from(ignored || kind == 0),
                    "kind {kind}, query {original_query}, weapon {weapon}"
                );
                if ignored {
                    assert_eq!(g.shots[0].position_fixed, [500 << 16, 506 << 16]);
                    assert!(g.impacts.is_empty());
                    assert_eq!(g.destroyed, 0);
                    assert!(g.targets.iter().all(|target| target.health == 5));
                } else if kind == 0 {
                    assert!(g.shots[0].retiring);
                    assert_eq!(g.shots[0].position_fixed, [500 << 16, 500 << 16]);
                    assert_eq!(g.shots[0].lifecycle_record.unwrap()[0x1a], 0);
                    for _ in 0..3 {
                        g.step(Input::default());
                        assert_eq!(g.shots.len(), 1);
                    }
                    g.step(Input::default());
                    assert!(g.shots.is_empty());
                    assert!(g.impacts.is_empty(), "cannon retirement has no area blast");
                    assert_eq!(g.destroyed, 0);
                } else if kind == 7 {
                    assert!(g.targets[0].health < 5, "ordinary tanks still take damage");
                }
            }
        }
    }

    #[test]
    fn grenade_expiry_delays_blast_and_impact_preserves_credit() {
        fn launch() -> Game {
            let mut game = empty();
            game.optional_weapons.selected = 1;
            game.optional_weapons.available[1] = 1;
            game.weapon_ammo_costs[1] = 10;
            game.weapon_damage[10] = 3;
            game.ammo = 30;
            for _ in 0..16 {
                game.step(Input {
                    fire_optional: true,
                    ..Default::default()
                });
            }
            assert_eq!(game.ammo, 20);
            assert_eq!(game.shots[0].life, 54);
            assert!(game.audio_events.contains(&10024));
            game
        }
        fn target_at_shot(game: &mut Game) {
            game.flags.enabled = true;
            let world = World {
                obstacles: vec![Obstacle {
                    kind: 7,
                    h: 500,
                    v: 500,
                    facing: 0,
                }],
            };
            let item = crate::scene::Catalog::default()
                .instantiate(&world)
                .remove(0);
            game.add_target(item, 5).unwrap();
            game.targets[0].item.x = game.shots[0].x;
            game.targets[0].item.z = game.shots[0].z;
            game.targets[0].fire.weapon = 10;
        }
        for original_query in [false, true] {
            let mut wall_hit = launch();
            let before = wall_hit.shots[0].position_fixed;
            let mut wall = crate::scene::Catalog::default()
                .instantiate(&World {
                    obstacles: vec![Obstacle {
                        kind: 0,
                        h: 500,
                        v: 500,
                        facing: 0,
                    }],
                })
                .remove(0);
            wall.x = wall_hit.shots[0].x;
            wall.z = wall_hit.shots[0].z;
            wall.shape = None;
            wall.min = [-20., -20., -20.];
            wall.max = [20., 100., 20.];
            wall_hit.items.push(wall);
            if original_query {
                let mut models = crate::model_slots::ModelSlots::default();
                models.state.current[6] = 1;
                models.state.current[0] = 1;
                models.geometry.insert(
                    1,
                    crate::model::PackedShape {
                        vertices: vec![],
                        dimensions: [40 << 16; 2],
                        candidate_radius: 30,
                    },
                );
                wall_hit.collision_models = Some(models);
            }
            wall_hit.step(Input::default());
            assert!(wall_hit.shots[0].retiring, "query {original_query}");
            assert_eq!(wall_hit.shots[0].position_fixed, before, "one-tick rewind");
            assert_eq!(wall_hit.shots[0].lifecycle_record.unwrap()[0x1a], 0);
            assert!(wall_hit.impacts.is_empty());
            wall_hit.items.clear();
            let mut frame = crate::render::Frame::default();
            frame.first_person(&wall_hit);
            let retired_pixels = frame.rgba.clone();
            wall_hit.shots[0].retiring = false;
            frame.first_person(&wall_hit);
            assert_ne!(frame.rgba, retired_pixels, "active shot marker is visible");
            wall_hit.shots[0].retiring = true;
            let shot = wall_hit.shots.pop().unwrap();
            frame.first_person(&wall_hit);
            assert_eq!(
                frame.rgba, retired_pixels,
                "retired shot has no world marker"
            );
            wall_hit.shots.push(shot);
            let mut palette = [[0; 3]; 13];
            palette[6] = [65535, 0, 0];
            palette[5] = [0, 0, 65535];
            wall_hit.palette = Some(palette);
            let mut kinds = vec![[0; 2]; 47];
            kinds[10] = [6, 5];
            wall_hit.color_bindings = Some(crate::recovered::ColorBindings {
                vehicles: [[0; 2]; 4],
                kinds,
                effects: [0; 4],
                special: [0; 8],
            });
            let mut glyphs = vec![vec![]; 41];
            glyphs[38] = vec![[0, 0]];
            wall_hit.radar_glyphs = Some(glyphs);
            let red_points = |frame: &crate::render::Frame| {
                frame
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[..3] == [255, 0, 0])
                    .count()
            };
            frame.first_person(&wall_hit);
            assert_eq!(red_points(&frame), 1, "retired radar glyph uses body color");
            wall_hit.color_bindings.as_mut().unwrap().kinds[10][1] = 13;
            frame.first_person(&wall_hit);
            assert_eq!(red_points(&frame), 0, "radar sentinel suppresses marker");
            wall_hit.color_bindings.as_mut().unwrap().kinds[10][1] = 5;
            let shot = wall_hit.shots.pop().unwrap();
            frame.first_person(&wall_hit);
            assert_eq!(
                red_points(&frame),
                0,
                "removed projectile leaves no radar marker"
            );
            wall_hit.shots.push(shot);
            target_at_shot(&mut wall_hit);
            for _ in 0..3 {
                wall_hit.step(Input::default());
                assert_eq!(wall_hit.targets[0].health, 5);
            }
            wall_hit.step(Input::default());
            assert!(wall_hit.shots.is_empty());
            assert_eq!(wall_hit.targets[0].health, 0);
            assert_eq!(wall_hit.combat_counters.player_hits, 0);
            assert_eq!(wall_hit.mission.score, 0);
        }
        let mut timed = launch();
        for _ in 0..54 {
            timed.step(Input::default());
        }
        assert!(timed.shots[0].retiring);
        target_at_shot(&mut timed);
        for _ in 0..3 {
            timed.step(Input::default());
            assert_eq!(timed.targets[0].health, 5);
        }
        timed.step(Input::default());
        assert_eq!(timed.targets[0].health, 0);
        assert!(timed.shots.is_empty());
        assert_eq!(timed.combat_counters.player_hits, 0);
        assert_eq!(timed.mission.score, 0);
        let mut impact = launch();
        target_at_shot(&mut impact);
        impact.step(Input::default());
        assert_eq!(impact.targets[0].health, 0);
        assert!(impact.shots.is_empty());
        assert_eq!(impact.combat_counters.player_hits, 1);
        assert_eq!(impact.mission.score, 10);
        let mut protected = launch();
        target_at_shot(&mut protected);
        protected.targets[0].vulnerable = false;
        protected.step(Input::default());
        assert_eq!(protected.targets[0].health, 5);
        assert_eq!(protected.mission.score, 0);
    }

    #[test]
    fn missiles_launch_climb_descend_and_use_resource_damage() {
        let mut game = empty();
        game.optional_weapons.selected = 10;
        game.optional_weapons.available[10] = 1;
        game.weapon_ammo_costs[10] = 6;
        game.weapon_damage[10] = 3;
        game.ammo = 20;
        for _ in 0..16 {
            game.step(Input {
                fire_optional: true,
                ..Default::default()
            });
        }
        assert_eq!(game.ammo, 14);
        assert_eq!(game.fired, 1);
        assert!(game.audio_events.contains(&10027));
        assert_eq!(game.shots[0].life, 300);
        assert_eq!(game.shots[0].missile.unwrap().height, 20 << 16);
        for original_query in [false, true] {
            for ground in [false, true] {
                let mut contact = empty();
                contact.tank.ticks = game.tank.ticks;
                let mut shot = game.shots[0].clone();
                let original_position = shot.position_fixed;
                if ground {
                    let missile = shot.missile.as_mut().unwrap();
                    missile.height = 1;
                    missile.vertical_velocity = -2 << 16;
                }
                let mut wall = crate::scene::Catalog::default()
                    .instantiate(&World {
                        obstacles: vec![Obstacle {
                            kind: 0,
                            h: 500,
                            v: 500,
                            facing: 0,
                        }],
                    })
                    .remove(0);
                wall.x = shot.x;
                wall.z = shot.z;
                wall.shape = None;
                wall.min = [-20., -20., -20.];
                wall.max = [20., 100., 20.];
                contact.missile_model = Some(wall.clone());
                if !ground {
                    contact.items.push(wall);
                }
                contact.shots.push(shot);
                if original_query {
                    let mut models = crate::model_slots::ModelSlots::default();
                    models.state.current[6] = 1;
                    models.state.current[0] = 1;
                    models.geometry.insert(
                        1,
                        crate::model::PackedShape {
                            vertices: vec![],
                            dimensions: [40 << 16; 2],
                            candidate_radius: 30,
                        },
                    );
                    contact.collision_models = Some(models);
                }
                contact.step(Input::default());
                assert!(
                    contact.shots[0].retiring,
                    "query {original_query}, ground {ground}"
                );
                assert_eq!(contact.shots[0].position_fixed, original_position);
                assert_eq!(
                    contact.shots[0].missile.unwrap().height,
                    if ground { 52428 } else { 20 << 16 }
                );
                let record = contact.shots[0].lifecycle_record.unwrap();
                assert_eq!((record[0x14], record[0x15], record[0x1a]), (10, 34, 0));
                assert!(contact.projectile_models(false).is_empty());
                assert_eq!(contact.projectile_models(true).len(), 1);
                for _ in 0..3 {
                    contact.step(Input::default());
                    assert_eq!(contact.shots.len(), 1);
                }
                contact.step(Input::default());
                assert!(contact.shots.is_empty());
                assert!(contact.impacts.is_empty());
                assert_eq!(contact.destroyed, 0);
            }
        }
        let start = game.shots[0].position_fixed;
        for _ in 0..14 {
            game.step(Input::default());
        }
        assert_eq!(game.shots[0].missile.unwrap().height, 104 << 16);
        assert_ne!(game.shots[0].position_fixed, start);
        game.step(Input::default());
        assert_eq!(game.shots[0].missile.unwrap().vertical_velocity, -2 << 16);
        assert_eq!(game.shots[0].missile.unwrap().height, 104 << 16);
        game.step(Input::default());
        assert!(game.shots[0].missile.unwrap().height < 104 << 16);
        let world = World {
            obstacles: vec![Obstacle {
                kind: 7,
                h: 500,
                v: 500,
                facing: 0,
            }],
        };
        let item = crate::scene::Catalog::default()
            .instantiate(&world)
            .remove(0);
        game.add_target(item, 5).unwrap();
        game.targets[0].item.x = game.shots[0].x;
        game.targets[0].item.z = game.shots[0].z;
        let missile = game.shots[0].missile.as_mut().unwrap();
        missile.height = 1 << 16;
        missile.vertical_velocity = 0;
        game.step(Input::default());
        assert_eq!(
            game.targets[0].health, 2,
            "missile uses slot-10 damage, not cannon damage"
        );
        assert!(game.shots.is_empty());
    }

    #[test]
    fn seekers_charge_two_rounds_and_reacquire_during_flight() {
        let mut game = empty();
        game.optional_weapons.selected = 9;
        game.optional_weapons.available[9] = 1;
        game.weapon_ammo_costs[9] = 2;
        game.ammo = 10;
        let held = Input {
            fire_optional: true,
            ..Default::default()
        };
        for _ in 0..16 {
            game.step(held);
        }
        assert_eq!(game.fired, 1);
        assert_eq!(game.ammo, 8);
        assert_eq!(game.shots[0].weapon, 9);
        let initial = game.shots[0].heading_fixed;
        let world = World {
            obstacles: vec![Obstacle {
                kind: 7,
                h: (game.shots[0].x + 80.) as i16,
                v: (game.shots[0].z + 60.) as i16,
                facing: 0,
            }],
        };
        let item = crate::scene::Catalog::default()
            .instantiate(&world)
            .remove(0);
        game.add_target(item, 3).unwrap();
        game.targets[0].item.x = game.shots[0].x + 80.;
        game.targets[0].item.z = game.shots[0].z + 60.;
        game.step(Input::default());
        let turned = game.shots[0].heading_fixed;
        assert_ne!(
            turned, initial,
            "a target appearing after launch must steer the Seeker"
        );
        game.targets[0].health = 0;
        game.step(Input::default());
        assert_eq!(
            game.shots[0].heading_fixed, turned,
            "dead target is no longer tracked"
        );
        game.ammo = 1;
        for _ in 0..16 {
            game.step(held);
        }
        assert_eq!(game.fired, 1);
        assert_eq!(game.ammo, 1);
    }

    #[test]
    fn scattershot_spreads_charges_each_request_and_respects_midburst_limits() {
        let setup = |ammo| {
            let mut game = Game::new(World { obstacles: vec![] }).unwrap();
            game.optional_weapons.selected = 5;
            game.optional_weapons.available[5] = 1;
            game.weapon_ammo_costs[3..=5].fill(1);
            game.ammo = ammo;
            game
        };
        let held = Input {
            fire_optional: true,
            ..Default::default()
        };
        let mut game = setup(10);
        let heading = game.heading_fixed;
        for _ in 0..15 {
            game.step(held);
        }
        assert_eq!(game.fired, 0);
        game.step(held);
        assert_eq!(game.fired, 3);
        assert_eq!(game.ammo, 7);
        assert_eq!(game.shots[0].heading_fixed, heading);
        let delta = |h: i32| (h - heading + (180 << 16)).rem_euclid(360 << 16) - (180 << 16);
        assert!((7 << 16..=14 << 16).contains(&delta(game.shots[1].heading_fixed)));
        assert!((-14 << 16..=-7 << 16).contains(&delta(game.shots[2].heading_fixed)));
        assert!(game.audio_events.contains(&10038));
        assert_eq!(game.last_fire_attempt, 16);
        let seed = game.spawn_random;
        game.shots = vec![game.shots[0].clone(); 19];
        game.tank.ticks = 32;
        game.step(held);
        assert_eq!(game.shots.len(), 20);
        assert_eq!(game.fired, 4);
        assert_eq!(game.ammo, 6);
        assert_eq!(
            game.spawn_random, seed,
            "denied spread shots do not consume Random"
        );
        let mut low = setup(2);
        for _ in 0..16 {
            low.step(held);
        }
        assert_eq!(low.fired, 2);
        assert_eq!(low.ammo, 0);
        assert!(low.audio_events.contains(&10006));
        let mut both = setup(10);
        both.primary_ammo_cost = 1;
        for _ in 0..16 {
            both.step(Input {
                fire: true,
                fire_optional: true,
                ..Default::default()
            });
        }
        assert_eq!(
            both.fired, 1,
            "primary fire updates the shared timestamp before optional fire"
        );
    }

    #[test]
    fn optional_selection_runs_on_ticks_repeats_and_sounds_only_on_change() {
        let mut game = Game::new(World { obstacles: vec![] }).unwrap();
        game.optional_weapons.selected = 5;
        game.optional_weapons.available[5] = 1;
        game.optional_weapons.available[9] = 1;
        let held = Input {
            select_weapon: true,
            ..Default::default()
        };
        game.advance(0., held);
        assert_eq!(game.optional_weapons.selected, 5);
        for _ in 0..10 {
            game.step(held);
        }
        assert_eq!(game.optional_weapons.selected, 5);
        assert!(!game.audio_events.contains(&10034));
        game.step(held);
        assert_eq!(game.optional_weapons.selected, 9);
        assert_eq!(game.last_weapon_selection, 11);
        assert_eq!(
            game.audio_events.iter().filter(|&&id| id == 10034).count(),
            1
        );
        for _ in 0..11 {
            game.step(held);
        }
        assert_eq!(game.optional_weapons.selected, 5);
        assert_eq!(
            game.audio_events.iter().filter(|&&id| id == 10034).count(),
            2
        );
        game.optional_weapons.available[9] = 0;
        for _ in 0..11 {
            game.step(held);
        }
        assert_eq!(game.last_weapon_selection, 33);
        assert_eq!(game.optional_weapons.selected, 5);
        assert_eq!(
            game.audio_events.iter().filter(|&&id| id == 10034).count(),
            2
        );
        game.optional_weapons.available[9] = 1;
        game.begin_player_death();
        for _ in 0..22 {
            game.step(held);
        }
        assert_eq!(
            game.last_weapon_selection, 33,
            "death transition suppresses selection input"
        );
        assert_eq!(game.optional_weapons.selected, 5);
        game.reset();
        assert_eq!(game.last_weapon_selection, 0);
        assert_eq!(game.optional_weapons, OptionalWeapons::default());
    }

    #[test]
    fn campaign_optional_loadout_accumulates_hints_and_reset_restores_selection() {
        let waves: Vec<_> = [18, 17, 10, 19]
            .into_iter()
            .map(|hint| crate::formats::Wave {
                world: 1,
                enemies: 1,
                hint,
                beasties: 1,
                start_movie: 0,
                end_movie: 0,
                song: 0,
                colors: 0,
            })
            .collect();
        let mut game = Game::new(World { obstacles: vec![] }).unwrap();
        game.configure_optional_weapons(&waves, 3);
        let initial = game.optional_weapons;
        assert_eq!(initial.selected, 1);
        assert_eq!(
            initial
                .available
                .iter()
                .enumerate()
                .filter_map(|(slot, &enabled)| (enabled == 1).then_some(slot))
                .collect::<Vec<_>>(),
            vec![1, 13, 17]
        );
        game.optional_weapons.selected = 13;
        game.reset();
        assert_eq!(game.optional_weapons, initial);
        game.configure_optional_weapons(&waves, 5);
        assert_eq!(
            game.optional_weapons, initial,
            "repeated worlds retain the complete unlocked set"
        );
        game.configure_optional_weapons(&waves, 1);
        assert_eq!(game.optional_weapons.selected, 13);
        assert_eq!(
            game.optional_weapons.available[17], 0,
            "loading an earlier level starts from fresh availability"
        );
    }

    #[test]
    fn overhead_uses_resource_plane_sides_for_solid_top_view() {
        let mut g = empty();
        let mut shape = crate::formats::Shape {
            packed_vertices: vec![],
            vertices: vec![],
            faces: vec![],
            radar_vertices: [[-20., -20.], [20., -20.], [20., 20.], [-20., 20.]]
                .map(|[x, z]| crate::formats::Vertex { x, y: 0., z })
                .to_vec(),
            radar_lines: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            plane_sides: -1,
        };
        let mut frame = crate::render::Frame::default();
        frame.overhead(&g, &shape, 1.);
        let offset = (305 * crate::render::WIDTH + 405) * 4;
        assert_eq!(&frame.rgba[offset..offset + 3], &[145, 255, 191]);
        let mut kinds = vec![[0, 0]; 47];
        kinds[6] = [5, 6];
        g.color_bindings = Some(crate::recovered::ColorBindings {
            vehicles: [[0; 2]; 4],
            kinds,
            effects: [0; 4],
            special: [0; 8],
        });
        let mut palette = [[0; 3]; 13];
        palette[5] = [65535, 0, 0];
        palette[6] = [0, 0, 65535];
        g.palette = Some(palette);
        frame.overhead(&g, &shape, 1.);
        assert_eq!(
            &frame.rgba[offset..offset + 3],
            &[255, 0, 0],
            "top view uses the primary kind color, not the separate radar color"
        );
        g.palette.as_mut().unwrap()[5] = [0, 65535, 0];
        frame.overhead(&g, &shape, 1.);
        assert_eq!(&frame.rgba[offset..offset + 3], &[0, 255, 0]);
        g.palette = None;
        shape.plane_sides = 0;
        frame.overhead(&g, &shape, 1.);
        assert_ne!(&frame.rgba[offset..offset + 3], &[145, 255, 191]);
        shape.plane_sides = 8;
        assert_eq!(
            shape.top_view_fill_count(),
            0,
            "reject a polygon beyond available vertices"
        );
    }

    #[test]
    fn final_flag_awards_video_matched_ratings_once_and_reset_clears_results() {
        let world = World {
            obstacles: vec![Obstacle {
                kind: 12,
                h: 16,
                v: 16,
                facing: 0,
            }],
        };
        let items = crate::scene::Catalog::default().instantiate(&world);
        let mut g = Game::with_items(world, items).unwrap();
        g.enable_flag_objective();
        g.score_rules = Some(crate::formats::ScoreRules {
            thresholds: [i32::MAX; 10],
            repeat_increment: 10000,
        });
        g.par_seconds = Some(60);
        g.mission.score = 115;
        g.tank.ticks = 36 * 60;
        g.combat_counters = crate::recovered::CombatCounters {
            player_shots: 10,
            player_hits: 6,
            other_shots: 10,
            player_damage_events: 2,
        };
        g.collect_flag(0);
        let result = g.completion.as_ref().unwrap();
        assert_eq!(result.ratings, [3, 3, 4]);
        assert_eq!(result.bonuses, [200, 200, 400]);
        assert_eq!(g.mission.score, 935);
        assert_eq!(g.encounter.status, "won");
        g.finish_mission();
        assert_eq!(g.mission.score, 935);
        g.reset();
        assert!(g.completion.is_none());
        assert_eq!(g.mission.score, 0);
    }

    #[test]
    fn collected_flags_keep_placement_slots_and_do_not_redirect_type20() {
        let world = World {
            obstacles: vec![
                Obstacle {
                    kind: 12,
                    h: 1,
                    v: 1,
                    facing: 0
                };
                2
            ],
        };
        let items = crate::scene::Catalog::default().instantiate(&world);
        let mut g = Game::with_items(world, items).unwrap();
        let mut enemy = g.items[0].clone();
        enemy.kind = 20;
        g.add_target(enemy, 2).unwrap();
        g.targets[0].lives = 2;
        g.capture_targets();
        g.enable_flag_objective();
        g.collect_flag(0);
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.item_placement_ids, vec![1]);
        assert_eq!(
            g.placement_items.iter().map(|i| i.kind).collect::<Vec<_>>(),
            vec![14, 12]
        );
        // Both flags have the same coordinates: identity must not use position matching.
        g.targets[0].health = 0;
        g.respawn_enemies();
        assert_eq!(g.targets[0].health, 2);
        assert_eq!(g.flag_spawn_cursor, 1, "a nonflag slot does not scan ahead");
        g.collect_flag(0);
        assert!(g.items.is_empty());
        assert!(g.placement_items.iter().all(|i| i.kind == 14));
        g.reset();
        assert_eq!(g.item_placement_ids, vec![0, 1]);
        assert!(g.placement_items.iter().all(|i| i.kind == 12));
        assert_eq!(g.flag_spawn_cursor, 1);
    }

    #[test]
    fn mission_enemy_lives_transfer_respawn_retire_and_reset() {
        let mut g = empty();
        let world = World {
            obstacles: vec![Obstacle {
                kind: 7,
                h: 0,
                v: 0,
                facing: 0,
            }],
        };
        let item = crate::scene::Catalog::default()
            .instantiate(&world)
            .remove(0);
        g.add_target(item.clone(), 2).unwrap();
        g.add_target(item, 2).unwrap();
        for target in &mut g.targets {
            target.lives = 2;
        }
        g.capture_targets();
        g.flags.enabled = true;
        g.enemy_clear_bonus = Some(200);
        let original = [g.targets[0].item.x, g.targets[0].item.z];
        g.targets[0].health = 0;
        g.targets[0].deaths = 1;
        g.respawn_enemies();
        assert_eq!((g.targets[0].health, g.targets[0].lives), (2, 1));
        assert_ne!([g.targets[0].item.x, g.targets[0].item.z], original);
        g.targets[0].health = 0;
        g.targets[0].deaths += 1;
        g.respawn_enemies();
        assert_eq!(
            (g.targets[0].health, g.targets[0].lives, g.targets[1].lives),
            (2, 1, 1)
        );
        g.targets[0].health = 0;
        g.targets[0].deaths += 1;
        g.respawn_enemies();
        assert!(g.targets[0].retired);
        assert_eq!(g.targets[0].health, 0);
        let seed = g.spawn_random;
        g.respawn_enemies();
        assert_eq!(g.spawn_random, seed, "retirement must not repeat each tick");
        assert_eq!(g.mission.score, 0, "one remaining enemy prevents the bonus");
        g.targets[1].health = 0;
        g.respawn_enemies();
        assert_eq!((g.mission.score, g.enemy_clear_award), (200, 200));
        g.respawn_enemies();
        assert_eq!(g.mission.score, 200, "last-enemy bonus is awarded once");
        g.reset();
        assert!(
            g.targets
                .iter()
                .all(|t| t.health == 2 && t.lives == 2 && t.deaths == 0 && !t.retired)
        );
    }

    #[test]
    fn recovered_respawn_changes_pose_and_reset_replays_the_random_stream() {
        let mut g = empty();
        g.enable_flag_objective();
        let initial = g.handling_snapshot();
        g.radar_display
            .get_mut()
            .disturb(&crate::radar::damage_disturbance(1, 1, 0, 7, 0).unwrap());
        g.encounter.health = 0;
        g.respawn_player();
        assert_eq!(g.radar_display.borrow().stable, 1, "respawn restores radar");
        let first = g.handling_snapshot();
        assert_ne!(first.0, initial.0);
        assert_ne!(first.1, initial.1);
        assert!(first.0.iter().all(|v| (100 << 16..=924 << 16).contains(v)));
        assert!(first.0.iter().all(|v| (v >> 16) % 2 == 0));
        g.encounter.health = 0;
        g.respawn_player();
        assert_ne!(g.handling_snapshot().0, first.0);
        g.reset();
        g.encounter.health = 0;
        g.respawn_player();
        assert_eq!(g.handling_snapshot(), first);
    }

    #[test]
    fn mission_respawn_preserves_progress_and_consumes_earned_lives() {
        let mut g = empty();
        let flag = crate::scene::Item {
            kind: 12,
            shape_id: 12,
            x: 16.,
            z: 16.,
            heading: 0.,
            source: "test".into(),
            min: [0., 0., 0.],
            max: [7., 20., 0.],
            shape: None,
        };
        g.items = vec![flag.clone(), flag.clone()];
        g.initial_items = g.items.clone();
        let mut enemy = flag;
        enemy.kind = 7;
        enemy.x = 800.;
        enemy.z = 800.;
        g.targets.push(Target {
            fire: Default::default(),
            motion: Default::default(),
            navigation: Default::default(),
            lives: 1,
            deaths: 0,
            retired: false,
            kill_score: 10,
            item: enemy,
            health: 1,
            max_health: 1,
            vulnerable: true,
            animation_frame: 0,
        });
        g.capture_targets();
        g.score_rules = Some(crate::formats::ScoreRules {
            thresholds: [
                5000, 10000, 20000, 30000, 40000, 60000, 80000, 100000, 120000, 140000,
            ],
            repeat_increment: -25536,
        });
        g.enable_flag_objective();
        g.mission.score = 4990;
        g.collect_flag(0);
        assert_eq!((g.mission.score, g.mission.lives), (5010, 4));
        assert_eq!(
            g.audio_events,
            vec![10042, 10023],
            "extra life precedes one-more-flag cue"
        );
        g.targets[0].health = 0;
        g.destroyed = 1;
        g.tank.ticks = 321;
        g.fired = 5;
        g.ammo = 3;
        g.encounter.health = 0;
        g.respawn_player();
        assert_eq!((g.mission.lives, g.mission.deaths), (3, 1));
        assert_eq!((g.encounter.health, g.ammo), (6, 40));
        assert_eq!(g.encounter.status, "playing");
        assert_eq!(g.tank.ticks, 321);
        assert_eq!((g.fired, g.destroyed), (5, 1));
        assert_eq!(g.targets[0].health, 0, "dead enemies remain dead");
        assert_eq!(g.items.len(), 1);
        assert_eq!(g.flags.collected, 1);
        assert_eq!(g.mission.score, 5010);
        assert!(g.free(g.tank.x, g.tank.z));
        for _ in 0..3 {
            g.encounter.health = 0;
            g.respawn_player();
        }
        assert_eq!((g.mission.lives, g.mission.deaths), (0, 4));
        assert_eq!(g.encounter.status, "lost");
        g.reset();
        assert_eq!(
            (g.mission.score, g.mission.lives, g.mission.deaths),
            (0, 3, 0)
        );
        assert_eq!(g.flags.collected, 0);
        assert_eq!(g.items.len(), 2);
        assert_eq!(g.targets[0].health, 1);
    }
    #[test]
    fn upgraded_vehicle_survives_respawn_and_restart_and_changes_speed_limit() {
        let mut g = empty();
        g.enable_flag_objective();
        let settings = [8, 8, 5, 0, 0, 0, 0];
        g.enter_with_progress(
            MissionProgress {
                score: 500,
                lives: 2,
                deaths: 0,
            },
            settings,
        );
        let mut fastest = 0;
        for _ in 0..60 {
            g.step(Input {
                throttle: 1.,
                ..Input::default()
            });
            fastest = fastest.max(g.speed_byte);
        }
        assert_eq!(fastest, 8);
        g.encounter.health = 0;
        g.ammo = 1;
        g.respawn_player();
        assert_eq!((g.encounter.health, g.ammo, g.mission.lives), (8, 50, 1));
        assert_eq!(g.vehicle_settings, settings);
        g.reset();
        assert_eq!((g.encounter.health, g.ammo, g.mission.lives), (8, 50, 2));
        assert_eq!(g.vehicle_settings, settings);
    }

    #[test]
    fn campaign_restart_restores_entry_score_and_lives_after_new_awards() {
        let mut g = empty();
        g.enable_flag_objective();
        g.enter_with_progress(
            MissionProgress {
                score: 900,
                lives: 1,
                deaths: 2,
            },
            g.vehicle_settings,
        );
        g.award_points(200);
        g.mission.lives = 2;
        g.mission.deaths = 3;
        g.reset();
        assert_eq!(
            (g.mission.score, g.mission.lives, g.mission.deaths),
            (900, 1, 2)
        );
        g.award_points(200);
        assert_eq!(g.mission.score, 1100);
    }

    #[test]
    fn flag_mission_collects_once_finishes_and_resets_with_both_contact_paths() {
        for recovered_contact in [false, true] {
            let mut g = empty();
            let flag = crate::scene::Item {
                kind: 12,
                shape_id: 0,
                x: g.tank.x,
                z: g.tank.z,
                heading: 0.,
                source: "test".into(),
                min: [-5., 0., -5.],
                max: [5., 40., 5.],
                shape: None,
            };
            let mut second = flag.clone();
            second.z += 100.;
            g.items = vec![flag, second];
            g.initial_items = g.items.clone();
            if recovered_contact {
                let mut models = crate::model_slots::ModelSlots::default();
                models.state.current[6] = 1;
                models.state.current[12] = 2;
                for (slot, dimensions, candidate_radius) in
                    [(1, [18 << 16, 35 << 16], 20), (2, [10 << 16, 10 << 16], 8)]
                {
                    models.geometry.insert(
                        slot,
                        crate::model::PackedShape {
                            vertices: vec![],
                            dimensions,
                            candidate_radius,
                        },
                    );
                }
                g.collision_models = Some(models);
            }
            g.enable_flag_objective();
            assert!(g.free(g.tank.x, g.tank.z), "flags never block driving");
            g.step(Input::default());
            assert_eq!(g.flags.collected, 1);
            assert_eq!(g.flags.pickup_points, 20);
            assert_eq!(g.items.len(), 1);
            assert_eq!(
                g.encounter.status, "playing",
                "empty enemy list is not a win"
            );
            g.step(Input::default());
            assert_eq!(g.flags.collected, 1, "cannot collect twice");
            for _ in 0..180 {
                g.step(Input {
                    throttle: 1.,
                    ..Input::default()
                });
                if g.encounter.status == "won" {
                    break;
                }
            }
            assert_eq!(g.flags.collected, 2);
            assert_eq!(g.flags.pickup_points, 40);
            assert_eq!(g.encounter.status, "won");
            let ticks = g.tank.ticks;
            g.step(Input {
                throttle: 1.,
                ..Input::default()
            });
            assert_eq!(g.tank.ticks, ticks, "completed mission stops simulation");
            g.reset();
            assert_eq!(g.items.len(), 2);
            assert_eq!(g.flags.collected, 0);
            assert_eq!(g.flags.pickup_points, 0);
            assert_eq!(g.encounter.status, "playing");
        }
    }
    #[test]
    fn projectile_loop_keeps_fixed_point_endpoints_for_every_heading() {
        let table = crate::recovered::candidate_unit_table();
        for degree in 0..360 {
            let mut g = empty();
            g.heading_fixed = (degree << 16) + 32768;
            g.tank.ticks = 10;
            let origin = g.position_fixed;
            let unit = crate::recovered::movement_direction(g.heading_fixed, &table).unwrap();
            g.step(Input {
                fire: true,
                ..Input::default()
            });
            // The launch update has zero elapsed time; only the six-unit offset applies.
            for tick in 1..=10 {
                let expected: [i32; 2] = std::array::from_fn(|i| origin[i] + unit[i] * 6 * tick);
                let shot = &g.shots[0];
                assert_eq!(
                    shot.position_fixed, expected,
                    "heading {degree}, tick {tick}"
                );
                assert_eq!([shot.x, shot.z], expected.map(|n| n as f32 / 65536.));
                assert_eq!(shot.life, 76 - tick as u32);
                g.step(Input::default());
            }
        }
    }
    #[test]
    fn projectile_expires_at_deadline_without_a_final_move_or_impact() {
        let mut g = empty();
        for _ in 0..10 {
            g.step(Input::default());
        }
        let origin = g.position_fixed;
        g.step(Input {
            fire: true,
            ..Input::default()
        });
        assert_eq!(g.shots[0].deadline, 86);
        assert_eq!(g.shots[0].life, 75);
        assert_eq!(
            g.shots[0].position_fixed,
            [origin[0], origin[1] + (6 << 16)]
        );
        for _ in 0..74 {
            g.step(Input::default());
        }
        assert_eq!(g.tank.ticks, 85);
        assert_eq!(g.shots.len(), 1);
        assert_eq!(g.shots[0].life, 1);
        assert_eq!(
            g.shots[0].position_fixed,
            [origin[0], origin[1] + (450 << 16)]
        );
        // Would hit the arena edge if movement ran before the expiry check.
        g.shots[0].position_fixed[1] = 1023 << 16;
        g.step(Input::default());
        assert!(g.shots.is_empty());
        assert!(g.impacts.is_empty());
    }
    #[test]
    fn recovered_preset_acceleration_stop_reverse_and_turn_ramp() {
        let mut g = empty();
        let start = g.tank.z;
        for _ in 0..5 {
            g.step(Input {
                throttle: 1.,
                ..Input::default()
            });
        }
        assert_eq!(g.tank.speed, 75.);
        assert_eq!(g.tank.z, start + 3.75);
        g.step(Input::default());
        assert_eq!(g.tank.speed, 0.);
        assert_eq!(g.tank.z, start + 3.75);
        for _ in 0..5 {
            g.step(Input {
                throttle: -1.,
                ..Input::default()
            });
        }
        assert_eq!(g.tank.speed, -75.);
        assert_eq!(g.tank.z, start);
        g.reset();
        for _ in 0..8 {
            g.step(Input {
                steer: 1.,
                ..Input::default()
            });
        }
        assert_eq!(g.heading_fixed, (90 << 16) - (9 << 15));
        assert_eq!(g.turn_ramp, 8);
        g.step(Input::default());
        assert_eq!(g.turn_ramp, 0);
    }
    #[test]
    fn same_input_same_ticks_at_different_frame_rates() {
        let input = Input {
            throttle: 1.,
            steer: 0.4,
            brake: false,
            fire: false,
            select_weapon: false,
            fire_optional: false,
        };
        let mut a = empty();
        let mut b = empty();
        for _ in 0..120 {
            a.advance(1. / 30., input);
        }
        for _ in 0..576 {
            b.advance(1. / 144., input);
        }
        assert_eq!(a.tank.ticks, 240);
        assert_eq!(
            serde_json::to_string(&a.tank).unwrap(),
            serde_json::to_string(&b.tank).unwrap()
        );
    }
    #[test]
    fn obstacles_and_boundary_cannot_be_crossed() {
        let mut g = Game::new(World {
            obstacles: vec![Obstacle {
                h: 496,
                v: 620,
                kind: 0,
                facing: 0,
            }],
        })
        .unwrap();
        let forward = Input {
            throttle: 1.,
            ..Input::default()
        };
        for _ in 0..1200 {
            g.step(forward);
        }
        assert!(g.free(g.tank.x, g.tank.z));
        assert!(g.tank.z < 620.);
        let mut b = empty();
        for _ in 0..1200 {
            b.step(forward);
        }
        assert!(b.tank.z <= ARENA_SIZE - RADIUS);
    }
    #[test]
    fn tank_can_pass_beside_obstacle_without_old_circle_padding() {
        let g = Game::new(World {
            obstacles: vec![Obstacle {
                h: 496,
                v: 620,
                kind: 0,
                facing: 0,
            }],
        })
        .unwrap();
        // The fallback obstacle ends at x=508. The actual tank side at x=511
        // clears it; the former 22-unit circle incorrectly blocked this pose.
        assert!(g.free_at(520., 620., 0.));
        assert!(!g.free_at(516., 620., 0.));
        // Pointing the longer nose toward that obstacle must still collide.
        assert!(!g.free_at(520., 620., -std::f32::consts::FRAC_PI_2));
        assert!(g.free_at(10., 400., 0.));
        assert!(!g.free_at(8., 400., 0.));
    }
    #[test]
    fn brake_reverse_reset_and_stalls() {
        let mut g = empty();
        for _ in 0..60 {
            g.step(Input {
                throttle: 1.,
                ..Input::default()
            });
        }
        assert!(g.tank.speed > 0.);
        for _ in 0..60 {
            g.step(Input {
                brake: true,
                ..Input::default()
            });
        }
        assert_eq!(g.tank.speed, 0.);
        for _ in 0..60 {
            g.step(Input {
                throttle: -1.,
                ..Input::default()
            });
        }
        assert!(g.tank.speed < 0.);
        g.reset();
        assert_eq!(g.tank.ticks, 0);
        assert_eq!(g.tank.x, g.spawn[0]);
        g.advance(f64::NAN, Input::default());
        assert_eq!(g.tank.ticks, 0);
        g.advance(60., Input::default());
        assert_eq!(g.tank.ticks, 6);
    }
    #[test]
    fn shots_hit_boxes_and_fire_rate_is_bounded() {
        let mut g = Game::new(World {
            obstacles: vec![Obstacle {
                h: 496,
                v: 620,
                kind: 0,
                facing: 0,
            }],
        })
        .unwrap();
        let mut saw_retirement = false;
        for _ in 0..30 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
            saw_retirement |= g.shots.iter().any(|shot| shot.retiring);
        }
        assert_eq!(g.fired, 2);
        assert!(
            saw_retirement,
            "wall contact enters original retirement state"
        );
        assert!(
            g.impacts.is_empty(),
            "ordinary shots do not create area blasts"
        );
        assert!(g.shots.iter().all(|p| p.z < 620.));
        for _ in 0..240 {
            g.step(Input::default());
        }
        assert!(g.shots.is_empty());
        assert!(g.impacts.is_empty());
        g.reset();
        assert_eq!(g.fired, 0);
    }
    #[test]
    fn fire_timer_records_empty_ammo_attempts_and_uses_strict_boundary() {
        let mut g = empty();
        g.primary_ammo_cost = 1;
        g.ammo = 0;
        let fire = Input {
            fire: true,
            ..Input::default()
        };
        for _ in 0..10 {
            g.step(fire);
        }
        assert_eq!(g.last_fire_attempt, 0);
        g.step(fire);
        assert_eq!(g.last_fire_attempt, 11);
        assert_eq!(g.fired, 0);
        assert_eq!(
            g.combat_counters.player_shots, 0,
            "empty-ammo attempts are not launches"
        );
        g.ammo = 2;
        for _ in 0..10 {
            g.step(fire);
        }
        assert_eq!(g.fired, 0);
        g.step(fire);
        assert_eq!(g.fired, 1);
        assert_eq!(g.last_fire_attempt, 22);
        for _ in 0..10 {
            g.step(fire);
        }
        assert_eq!(g.fired, 1);
        g.step(fire);
        assert_eq!(g.fired, 2);
        g.reset();
        assert_eq!(g.last_fire_attempt, 0);
    }
    #[test]
    fn radar_tracks_enemy_lifecycle_and_type31_animation() {
        let mut g = empty();
        let mut palette = [[0; 3]; 13];
        palette[5] = [0, 65535, 0];
        palette[6] = [65535, 0, 0];
        g.palette = Some(palette);
        g.radar_glyphs = Some(vec![vec![[0, 0]]; 41]);
        let mut kinds = vec![[0; 2]; 47];
        kinds[6][0] = 5;
        kinds[7][1] = 6;
        kinds[31][1] = 6;
        g.color_bindings = Some(crate::recovered::ColorBindings {
            vehicles: [[0; 2]; 4],
            kinds,
            effects: [0; 4],
            special: [0; 8],
        });
        g.targets.push(Target {
            fire: Default::default(),
            motion: Default::default(),
            navigation: Default::default(),
            lives: 1,
            deaths: 0,
            retired: false,
            kill_score: 10,
            item: crate::scene::Item {
                kind: 7,
                shape_id: 0,
                x: g.tank.x,
                z: g.tank.z + 100.,
                heading: 0.,
                source: "radar fixture".into(),
                min: [0.; 3],
                max: [0.; 3],
                shape: None,
            },
            health: 1,
            max_health: 1,
            vulnerable: true,
            animation_frame: 0,
        });
        let mut frame = crate::render::Frame::default();
        let red_points = |frame: &crate::render::Frame| {
            frame
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .filter(|(_, p)| p[..3] == [255, 0, 0])
                .map(|(i, _)| (i % crate::render::WIDTH, i / crate::render::WIDTH))
                .collect::<Vec<_>>()
        };
        frame.first_person(&g);
        let first = red_points(&frame);
        assert_eq!(first.len(), 1);
        assert!(
            first[0].0 > 600 && first[0].1 < 300,
            "enemy marker is in the upper-right radar"
        );
        g.targets[0].item.x += 100.;
        frame.first_person(&g);
        let moved = red_points(&frame);
        assert_eq!(moved.len(), 1);
        assert!(moved[0].0 > first[0].0);
        g.targets[0].item.kind = 31;
        g.targets[0].vulnerable = false;
        frame.first_person(&g);
        assert!(red_points(&frame).is_empty());
        g.targets[0].vulnerable = true;
        frame.first_person(&g);
        assert_eq!(red_points(&frame).len(), 1);
        g.targets[0].health = 0;
        frame.first_person(&g);
        assert!(red_points(&frame).is_empty());
        let mut dump_item = g.targets[0].item.clone();
        dump_item.kind = 5;
        g.pickups.push(crate::pickups::Pickup {
            item: dump_item,
            subtype: 0,
            record: [0; 128],
        });
        g.color_bindings.as_mut().unwrap().effects = [6, 13, 5, 6];
        frame.first_person(&g);
        assert_eq!(
            red_points(&frame),
            moved,
            "dump uses subtype color and recovered radar transform"
        );
        g.pickups[0].subtype = 1;
        frame.first_person(&g);
        assert!(
            red_points(&frame).is_empty(),
            "dump color 13 hides its marker"
        );
        g.pickups[0].subtype = 3;
        g.pickups[0].item.x += 100.;
        frame.first_person(&g);
        assert_ne!(
            red_points(&frame),
            moved,
            "relocated dump marker follows its new position"
        );
        assert_eq!(red_points(&frame).len(), 1);
        g.pickups.clear();
        g.items.push(g.targets[0].item.clone());
        g.items[0].kind = 19;
        g.color_bindings.as_mut().unwrap().kinds[19][1] = 6;
        frame.first_person(&g);
        assert_eq!(
            red_points(&frame),
            moved,
            "teleporter uses the same radar transform"
        );
        g.color_bindings.as_mut().unwrap().kinds[19][1] = 13;
        frame.first_person(&g);
        assert!(red_points(&frame).is_empty(), "original hidden radar color");
        g.items[0].kind = 12;
        g.color_bindings.as_mut().unwrap().kinds[12][1] = 6;
        frame.first_person(&g);
        assert_eq!(
            red_points(&frame),
            moved,
            "single-player flag starts visible"
        );
        for _ in 0..24 {
            g.step(Input::default());
        }
        assert_eq!(g.radar_flash, 1);
        g.step(Input::default());
        assert_eq!(g.radar_flash, 0);
        frame.first_person(&g);
        assert!(
            red_points(&frame).is_empty(),
            "flag flash deadline hides marker"
        );
        for _ in 0..25 {
            g.step(Input::default());
        }
        frame.first_person(&g);
        assert_eq!(red_points(&frame), moved, "flag reappears after 25 ticks");
        g.items[0].kind = 14;
        frame.first_person(&g);
        assert!(
            red_points(&frame).is_empty(),
            "collected flag has no contact"
        );
        g.items[0].kind = 19;
        g.items.push(g.items[0].clone());
        g.items[1].kind = 36;
        g.color_bindings.as_mut().unwrap().kinds[19][1] = 6;
        g.color_bindings.as_mut().unwrap().kinds[36][1] = 5;
        frame.first_person(&g);
        assert_eq!(
            red_points(&frame),
            moved,
            "higher color queue wins overlaps"
        );
        g.items.swap(0, 1);
        frame.first_person(&g);
        assert_eq!(
            red_points(&frame),
            moved,
            "overlap is independent of object order"
        );
        let stable_image = frame.rgba.clone();
        let simulation = (g.handling_snapshot(), g.spawn_random, g.tank.ticks);
        let event = crate::radar::damage_disturbance(1, 1, 2, 3, g.tank.ticks as i32).unwrap();
        g.radar_display.get_mut().disturb(&event);
        frame.first_person(&g);
        assert_ne!(frame.rgba, stable_image, "live contacts are disturbed");
        for _ in 0..3 {
            frame.first_person(&g);
        }
        assert_eq!(
            (g.handling_snapshot(), g.spawn_random, g.tank.ticks),
            simulation,
            "display randomness cannot change movement or spawn randomness"
        );
        g.tank.ticks = event.deadline as u64;
        frame.first_person(&g);
        assert_eq!(g.radar_display.borrow().stable, 1);
        frame.first_person(&g);
        assert_eq!(frame.rgba, stable_image, "subsequent redraw is stable");
        g.radar_display.get_mut().disturb(&event);
        g.reset();
        assert_eq!((g.radar_flash, g.radar_flash_deadline), (1, 25));
        assert_eq!(g.radar_display.borrow().stable, 1);
    }

    #[test]
    fn compact_first_person_preserves_surface_margins_and_releases_clip() {
        let mut frame = crate::render::Frame::default();
        frame.first_person(&empty());
        for y in 0..crate::render::HEIGHT {
            for x in 0..crate::render::WIDTH {
                if !(80..720).contains(&x) || !(187..514).contains(&y) {
                    let offset = (y * crate::render::WIDTH + x) * 4;
                    assert_eq!(&frame.rgba[offset..offset + 3], &[0, 0, 0]);
                }
            }
        }
        frame.line([0., 0.], [10., 0.], [255, 0, 0]);
        assert_eq!(
            &frame.rgba[..3],
            &[255, 0, 0],
            "later overlays are not clipped to the world"
        );
    }

    #[test]
    fn crosshair_uses_original_marks_and_bound_color() {
        let mut g = empty();
        g.palette = Some([[0; 3]; 13]);
        g.palette.as_mut().unwrap()[5] = [65535, 0, 65535];
        let mut special = [0; 8];
        special[1] = 5;
        g.color_bindings = Some(crate::recovered::ColorBindings {
            vehicles: [[0; 2]; 4],
            kinds: vec![],
            effects: [0; 4],
            special,
        });
        let mut frame = crate::render::Frame::default();
        frame.first_person(&g);
        let rgb = |x: usize, y: usize| {
            &frame.rgba[(y * crate::render::WIDTH + x) * 4..(y * crate::render::WIDTH + x) * 4 + 3]
        };
        for (x, y) in [(340, 400), (460, 400), (400, 340)] {
            assert_eq!(rgb(x, y), &[255, 0, 255]);
        }
        assert_ne!(rgb(400, 400), &[255, 0, 255], "the center remains open");
        assert_ne!(
            rgb(400, 310),
            &[255, 0, 255],
            "the old hand-positioned mark is gone"
        );
    }
    #[test]
    fn horizon_uses_level_palette_color_instead_of_fixed_magenta() {
        let mut g = empty();
        g.palette = Some([[0; 3]; 13]);
        let mut special = [0; 8];
        special[6] = 3;
        g.color_bindings = Some(crate::recovered::ColorBindings {
            vehicles: [[0; 2]; 4],
            kinds: vec![],
            effects: [0; 4],
            special,
        });
        let mut frame = crate::render::Frame::default();
        let offset = (390 * crate::render::WIDTH + 100) * 4;
        g.palette.as_mut().unwrap()[3] = [65535, 0, 65535];
        frame.first_person(&g);
        assert!(frame.rgba[offset] > 0 && frame.rgba[offset + 2] > 0);
        assert_eq!(frame.rgba[offset + 1], 0);
        g.vertical.renderer_height = 0;
        frame.first_person(&g);
        assert_eq!(
            &frame.rgba[offset..offset + 3],
            &[0, 0, 0],
            "lower horizon bands disappear during descent"
        );
        g.vertical.renderer_height = -96;
        frame.first_person(&g);
        let upper = (320 * crate::render::WIDTH + 100) * 4;
        assert_eq!(
            &frame.rgba[upper..upper + 3],
            &[0, 0, 0],
            "submerged horizon has no color bands"
        );
        let palette = g.palette.take();
        frame.first_person(&g);
        assert_eq!(
            &frame.rgba[upper..upper + 3],
            &[0, 0, 0],
            "missing palette must not restore the submerged horizon"
        );
        g.vertical.renderer_height = 32;
        frame.first_person(&g);
        assert!(
            frame.rgba[offset] > 0,
            "diagnostic surface horizon remains visible"
        );
        g.palette = palette;
        g.vertical.hover = 200;
        frame.first_person(&g);
        assert!(
            frame.rgba[offset] > 0,
            "hover altitude does not suppress the horizon"
        );
        g.vertical.hover = 0;
        g.palette.as_mut().unwrap()[3] = [0, 65535, 0];
        frame.first_person(&g);
        assert!(frame.rgba[offset + 1] > 0);
        assert_eq!([frame.rgba[offset], frame.rgba[offset + 2]], [0, 0]);
        assert_eq!(
            &frame.rgba[..3],
            &[0, 0, 0],
            "sky above the bands stays black"
        );
    }
    #[test]
    fn first_person_culls_faces_using_original_planes() {
        let mut g = empty();
        let model = crate::model::PackedShape {
            vertices: vec![[-64, 0, 0], [64, 0, 0], [64, 128, 0], [-64, 128, 0]],
            dimensions: [16 << 16, 0],
            candidate_radius: 8,
        };
        g.items.push(crate::scene::Item {
            kind: 0,
            shape_id: 0,
            x: g.tank.x,
            z: g.tank.z + 100.,
            heading: std::f32::consts::FRAC_PI_2,
            source: "face visibility fixture".into(),
            min: [-8., 0., 0.],
            max: [8., 16., 0.],
            shape: Some(model.shaded_preview(vec![vec![0, 3, 2, 1]], 0)),
        });
        let mut frame = crate::render::Frame::default();
        frame.first_person(&g);
        let offset = (381 * crate::render::WIDTH + 400) * 4;
        assert_eq!(&frame.rgba[offset..offset + 3], &[20, 80, 160]);
        g.items[0].shape = Some(model.shaded_preview(vec![vec![0, 1, 2, 3]], 0));
        frame.first_person(&g);
        assert_ne!(
            &frame.rgba[offset..offset + 3],
            &[20, 80, 160],
            "reversing the plane hides the back face"
        );
    }
    #[test]
    fn all_playable_views_use_native_scenery_heading() {
        let mut g = empty();
        let shape = crate::model::PackedShape {
            vertices: vec![[0, 32, 0], [0, 32, 160]],
            dimensions: [0; 2],
            candidate_radius: 0,
        }
        .preview(vec![vec![0, 1]]);
        g.items.push(crate::scene::Item {
            kind: 0,
            shape_id: 0,
            x: g.position_fixed[0] as f32 / 65536.,
            z: g.position_fixed[1] as f32 / 65536. + 100.,
            heading: 0.,
            source: "orientation fixture".into(),
            min: [0.; 3],
            max: [20.; 3],
            shape: Some(shape),
        });
        let mut frame = crate::render::Frame::default();
        frame.first_person(&g);
        let offset = (400 * crate::render::WIDTH + 440) * 4;
        assert_eq!(
            &frame.rgba[offset..offset + 3],
            &[20, 80, 160],
            "native heading zero rotates the local Z segment to screen right at camera heading 90"
        );
        let tank_shape = crate::model::PackedShape {
            vertices: vec![],
            dimensions: [0; 2],
            candidate_radius: 0,
        }
        .preview(vec![]);
        frame.overhead(&g, &tank_shape, 1.);
        let map_offset = (220 * crate::render::WIDTH + 410) * 4;
        assert_eq!(
            &frame.rgba[map_offset..map_offset + 3],
            &[222, 177, 100],
            "map shows the same segment extending along world X"
        );
        frame.driving(&g, &tank_shape, 1.);
        let chase_offset = (259 * crate::render::WIDTH + 430) * 4;
        assert_eq!(
            &frame.rgba[chase_offset..chase_offset + 3],
            &[222, 177, 100],
            "exterior view shows the segment to the right"
        );
        g.items[0].heading = std::f32::consts::FRAC_PI_2;
        frame.overhead(&g, &tank_shape, 1.);
        assert_ne!(&frame.rgba[map_offset..map_offset + 3], &[222, 177, 100]);
        frame.driving(&g, &tank_shape, 1.);
        assert_ne!(
            &frame.rgba[chase_offset..chase_offset + 3],
            &[222, 177, 100]
        );
        frame.first_person(&g);
        assert_ne!(
            &frame.rgba[offset..offset + 3],
            &[20, 80, 160],
            "matching native headings point that segment into depth"
        );
    }
    #[test]
    fn first_person_projectile_marker_tracks_height() {
        let mut g = empty();
        for _ in 0..11 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        g.shots[0].x = g.tank.x;
        g.shots[0].z = g.tank.z + 200.;
        let marker_y = |game: &Game| {
            let mut frame = crate::render::Frame::default();
            frame.first_person(game);
            let pixels: Vec<usize> = frame
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .filter(|(_, color)| color[..3] == [255, 245, 155])
                .map(|(i, _)| i / crate::render::WIDTH)
                .collect();
            assert!(!pixels.is_empty());
            pixels.iter().sum::<usize>() as f32 / pixels.len() as f32
        };
        let low = marker_y(&g);
        g.shots[0].height = 80.;
        let high = marker_y(&g);
        assert!(
            high < low - 20.,
            "elevated projectiles must render above ground-level shots"
        );
    }

    #[test]
    fn active_shot_limit_blocks_charge_until_expired_shots_are_removed() {
        let mut g = empty();
        g.primary_ammo_cost = 1;
        let fire = Input {
            fire: true,
            ..Input::default()
        };
        for _ in 0..11 {
            g.step(fire);
        }
        assert_eq!(g.fired, 1);
        let mut shot = g.shots[0].clone();
        shot.position_fixed = [512 << 16; 2];
        shot.direction_fixed = [0; 2];
        shot.deadline = 22;
        g.shots = vec![shot; 20];
        g.ammo = 6;
        g.audio_events.clear();
        for _ in 0..11 {
            g.step(fire);
        }
        assert_eq!(g.last_fire_attempt, 22);
        assert_eq!(g.fired, 1);
        assert_eq!(g.ammo, 6);
        assert!(g.shots.is_empty());
        assert!(!g.audio_events.contains(&10010));
        for _ in 0..11 {
            g.step(fire);
        }
        assert_eq!(g.fired, 2);
        assert_eq!(g.ammo, 5);
        assert_eq!(g.shots.len(), 1);
        assert_eq!(g.audio_events.iter().filter(|&&id| id == 10010).count(), 1);
        g.reset();
        assert!(g.shots.is_empty());
    }
    #[test]
    fn low_ammo_warning_occurs_on_threshold_crossing_only() {
        for (ammo, cost, expected_ammo, warning) in [
            (6, 1, 5, true),
            (8, 3, 5, true),
            (7, 1, 6, false),
            (5, 1, 4, false),
            (6, 0, 6, false),
            (2, 3, 2, false),
        ] {
            let mut g = empty();
            g.ammo = ammo;
            g.primary_ammo_cost = cost;
            for _ in 0..11 {
                g.step(Input {
                    fire: true,
                    ..Input::default()
                });
            }
            assert_eq!(g.ammo, expected_ammo);
            assert_eq!(
                g.audio_events.iter().filter(|&&id| id == 10010).count(),
                usize::from(warning)
            );
            assert_eq!(g.fired, u64::from(ammo >= cost));
            let events = g.audio_events.len();
            g.step(Input::default());
            assert_eq!(g.audio_events.len(), events);
        }
    }
    #[test]
    fn finite_ammo_stops_fire_and_reset_refills() {
        let mut g = empty();
        g.primary_ammo_cost = 1;
        for _ in 0..1000 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        assert_eq!(g.fired, 40);
        assert_eq!(g.ammo, 0);
        g.reset();
        assert_eq!(g.ammo, 40);
        for _ in 0..10 {
            g.step(Input::default());
        }
        g.step(Input {
            fire: true,
            ..Input::default()
        });
        assert_eq!(g.fired, 1);
        assert_eq!(g.ammo, 39);
        g.ammo = 1;
        g.primary_ammo_cost = 2;
        for _ in 0..30 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        assert_eq!(g.fired, 1);
        assert_eq!(g.ammo, 1);
    }
    #[test]
    fn type31_resists_shots_until_event_and_reset_restores_immunity() {
        let mut g = empty();
        let shape = crate::formats::Shape {
            packed_vertices: vec![],
            vertices: vec![
                crate::formats::Vertex {
                    x: -12.,
                    y: 0.,
                    z: -12.,
                },
                crate::formats::Vertex {
                    x: 12.,
                    y: 24.,
                    z: 12.,
                },
            ],
            faces: vec![],
            radar_vertices: vec![],
            radar_lines: vec![],
            plane_sides: 0,
        };
        let item = crate::scene::Catalog::default()
            .instantiate(&World {
                obstacles: vec![Obstacle {
                    kind: 31,
                    h: 496,
                    v: 620,
                    facing: 0,
                }],
            })
            .remove(0);
        g.targets.push(Target {
            fire: Default::default(),
            motion: Default::default(),
            navigation: Default::default(),
            lives: 1,
            deaths: 0,
            retired: false,
            kill_score: 0x10014,
            item,
            health: 3,
            max_health: 3,
            vulnerable: true,
            animation_frame: 0,
        });
        g.configure_type31(vec![shape; 10]);
        g.capture_targets();
        g.flags.enabled = true;
        g.mission.score = 4990;
        g.score_rules = Some(crate::formats::ScoreRules {
            thresholds: [
                5000, 10000, 20000, 30000, 40000, 60000, 80000, 100000, 120000, 140000,
            ],
            repeat_increment: -25536,
        });
        for _ in 0..120 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        assert!(
            !g.impacts.is_empty(),
            "shots actually hit the protected target"
        );
        assert_eq!(g.targets[0].health, 3);
        assert_eq!(g.destroyed, 0);
        assert_eq!(g.mission.score, 4990, "protected hits award no points");
        assert!(
            g.audio_events.contains(&10004),
            "protected hits retain the original nonfatal response"
        );
        assert!(
            !g.audio_events.contains(&10005),
            "protected target must not emit a death response"
        );
        assert!(
            g.combat_counters.player_hits > 0,
            "protected hits still count toward accuracy"
        );
        assert_eq!(g.combat_counters.player_shots as u64, g.fired);
        assert!(g.replay_type31_event());
        for _ in 0..56 {
            g.step(Input::default());
        }
        assert!(!g.targets[0].vulnerable);
        assert_eq!(g.targets[0].animation_frame, 8);
        g.step(Input::default());
        assert!(g.targets[0].vulnerable);
        assert_eq!(g.targets[0].animation_frame, 9);
        for _ in 0..120 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        assert_eq!(g.destroyed, 1);
        assert_eq!(g.targets[0].health, 0);
        assert_eq!(
            g.mission.score, 5010,
            "kill awards the signed low word only once"
        );
        assert_eq!(
            g.mission.lives, 4,
            "kill crosses the original extra-life threshold"
        );
        g.reset();
        assert_eq!(g.mission.score, 0);
        assert_eq!(g.mission.lives, 3);
        assert_eq!(g.targets[0].health, 3);
        assert_eq!(g.targets[0].animation_frame, 0);
        assert!(!g.targets[0].vulnerable);
        for _ in 0..120 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        assert_eq!(g.targets[0].health, 3);
    }
    #[test]
    fn destruction_removes_collision_and_reset_restores_target() {
        let mut g = empty();
        let model_world = World {
            obstacles: vec![Obstacle {
                kind: 7,
                h: 496,
                v: 620,
                facing: 0,
            }],
        };
        let item = crate::scene::Catalog::default()
            .instantiate(&model_world)
            .remove(0);
        g.targets.push(Target {
            fire: Default::default(),
            motion: Default::default(),
            navigation: Default::default(),
            lives: 1,
            deaths: 0,
            retired: false,
            kill_score: 10,
            item,
            health: 3,
            max_health: 3,
            vulnerable: true,
            animation_frame: 0,
        });
        g.capture_targets();
        assert!(!g.free(496., 620.));
        let mut hit_sounds = 0;
        let mut death_sounds = 0;
        for _ in 0..120 {
            g.audio_events.clear();
            let health = g.targets[0].health;
            g.step(Input {
                fire: true,
                ..Input::default()
            });
            let hits = g.audio_events.iter().filter(|&&id| id == 10004).count();
            let deaths = g.audio_events.iter().filter(|&&id| id == 10005).count();
            hit_sounds += hits;
            death_sounds += deaths;
            if g.targets[0].health < health {
                assert_eq!(
                    (hits, deaths),
                    if g.targets[0].health == 0 {
                        (0, 1)
                    } else {
                        (1, 0)
                    }
                );
            }
        }
        assert_eq!((hit_sounds, death_sounds), (2, 1));
        assert_eq!(g.destroyed, 1);
        assert_eq!(g.targets[0].health, 0);
        assert!(g.free(496., 620.));
        for _ in 0..60 {
            g.step(Input {
                fire: true,
                ..Input::default()
            });
        }
        assert_eq!(g.destroyed, 1);
        g.reset();
        assert_eq!(g.targets[0].health, 3);
        assert_eq!(g.destroyed, 0);
        assert!(!g.free(496., 620.));
        g.primary_damage = 3;
        g.shots.push(Shot {
            lifecycle_record: None,
            retiring: false,
            missile: None,
            weapon: 0,
            enemy_owner: None,
            bouncing_record: None,
            height: 4.,
            x: 496.,
            z: 619.,
            heading: 0.,
            life: 3,
            position_fixed: [496 << 16, 619 << 16],
            direction_fixed: [0, 6 << 16],
            heading_fixed: 90 << 16,
            deadline: g.tank.ticks as i32 + 3,
            last_update: g.tank.ticks as i32,
        });
        g.step(Input::default());
        assert_eq!(
            g.targets[0].health, 0,
            "one hit uses configured resource damage"
        );
        assert_eq!(g.destroyed, 1);
    }
    #[test]
    fn transit_contacts_submerge_then_return_to_surface() {
        let mut g = Game::new(World {
            obstacles: vec![Obstacle {
                kind: 37,
                h: 500,
                v: 500,
                facing: 0,
            }],
        })
        .unwrap();
        g.position_fixed = [500 << 16, 500 << 16];
        g.tank.x = 500.;
        g.tank.z = 500.;
        g.flight_energy = 400;
        let mut models = crate::model_slots::ModelSlots::default();
        models.state.current[6] = 1;
        models.state.current[36] = 2;
        models.state.current[37] = 2;
        for slot in [1, 2] {
            models.geometry.insert(
                slot,
                crate::model::PackedShape {
                    vertices: vec![],
                    dimensions: [40 << 16; 2],
                    candidate_radius: 30,
                },
            );
        }
        g.collision_models = Some(models);
        g.step(Input::default());
        assert_eq!(g.flight_energy, 0);
        assert_eq!(g.vertical.transition, -65536);
        assert!(g.vertical.vertical < 0);
        assert!(!g.tank.blocked, "transit remains passable");
        for _ in 0..20 {
            g.step(Input::default());
        }
        assert_eq!(g.vertical.transition, 0);
        assert_eq!(g.vertical.renderer_height, -96);
        assert_eq!(g.vertical.vertical, -128 * 65536);
        assert_eq!(g.audio_events.iter().filter(|id| **id == 10041).count(), 1);
        // The up-transit kind retains the original height-independent overlap exception.
        g.items[0].kind = 36;
        g.step(Input::default());
        assert_eq!(g.vertical.transition, 65536);
        assert_eq!(g.tank.surface, "up transit");
        for _ in 0..20 {
            g.step(Input::default());
        }
        assert_eq!(g.vertical.transition, 0);
        assert_eq!(g.vertical.vertical, 0);
        assert_eq!(g.vertical.renderer_height, 32);
        assert_eq!(g.audio_events.iter().filter(|id| **id == 10012).count(), 1);
    }

    #[test]
    fn ordinary_solid_contact_rolls_back_reverses_and_holds_airborne_height() {
        for hover in [0, 40] {
            let mut g = Game::new(World {
                obstacles: vec![Obstacle {
                    kind: 0,
                    h: 500,
                    v: 500,
                    facing: 0,
                }],
            })
            .unwrap();
            g.position_fixed = [500 << 16, 500 << 16];
            g.tank.x = 500.;
            g.tank.z = 500.;
            g.heading_fixed = 90 << 16;
            g.speed_byte = 5;
            g.vertical.hover = hover;
            g.vertical.vertical = hover << 16;
            g.flight_energy = if hover > 0 { 400 } else { 0 };
            let mut models = crate::model_slots::ModelSlots::default();
            models.state.current[6] = 1;
            models.state.current[0] = 2;
            for slot in [1, 2] {
                models.geometry.insert(
                    slot,
                    crate::model::PackedShape {
                        vertices: vec![],
                        dimensions: [40 << 16; 2],
                        candidate_radius: 30,
                    },
                );
            }
            g.collision_models = Some(models);
            assert!(g.try_recovered_motion(0, 1, 0));
            assert!(g.tank.blocked);
            assert_eq!(g.position_fixed, [500 << 16, 500 << 16]);
            assert_eq!(
                g.speed_byte, -4,
                "rollback changes capped speed before rebound reverses it"
            );
            assert!(g.audio_events.contains(&10007));
            if hover > 0 {
                assert_eq!(g.vertical.hold, 30);
                assert_eq!(g.vertical.vertical, (hover << 16) + 3);
                let sounds = g.audio_events.len();
                assert!(g.try_recovered_motion(0, 1, 0));
                assert_eq!(g.vertical.hold, 29);
                assert_eq!(
                    g.audio_events.len(),
                    sounds,
                    "hold suppresses the response sound and rollback"
                );
                assert_ne!(g.position_fixed, [500 << 16, 500 << 16]);
            }
        }
    }

    #[test]
    fn live_flight_shares_camera_shot_and_respawn_altitude() {
        let mut g = Game::new(World { obstacles: vec![] }).unwrap();
        g.flight_energy = 400;
        for _ in 0..100 {
            g.step(Input::default());
        }
        assert!(g.vertical.vertical > 60 * 65536);
        assert_eq!(g.first_person_camera().2, 32 + g.vertical.hover);
        g.step(Input {
            fire: true,
            ..Default::default()
        });
        assert!(
            g.shots
                .iter()
                .any(|s| s.height == 4. + g.vertical.vertical as f32 / 524288.)
        );
        let energy = g.flight_energy;
        g.encounter.health = 0;
        g.respawn_player();
        assert_eq!(g.vertical.vertical, 0);
        assert_eq!(g.vertical.hover, 0);
        assert_eq!(
            g.flight_energy, energy,
            "original local spawn retains the energy word"
        );
        g.step(Input::default());
        assert!(
            g.vertical.vertical > 0,
            "remaining energy lifts the respawned craft again"
        );
        for _ in 0..800 {
            g.step(Input::default());
        }
        assert_eq!(g.flight_energy, 0);
        assert_eq!(g.vertical.vertical, 0);
        assert_eq!(g.first_person_camera().2, 32);
    }

    #[test]
    fn pickup_animation_repeats_without_changing_contact_record() {
        let mut g = Game::new(World { obstacles: vec![] }).unwrap();
        let mut catalog = crate::scene::Catalog {
            choices: vec![[0; 3]; 47],
            ..Default::default()
        };
        catalog.choices[5] = [6; 3];
        let item = catalog
            .instantiate(&World {
                obstacles: vec![Obstacle {
                    kind: 5,
                    h: 0,
                    v: 0,
                    facing: 0,
                }],
            })
            .pop()
            .unwrap();
        g.pickups =
            crate::pickups::create(&item, vec![], [5, 2, 1, 2], &mut g.pickup_random).unwrap();
        let record = g.pickups[0].record;
        let bounds = (g.pickups[0].item.min, g.pickups[0].item.max);
        g.step(Input::default());
        let first = g.pickups[0]
            .item
            .shape
            .as_ref()
            .unwrap()
            .packed_vertices
            .clone();
        for _ in 0..58 {
            g.step(Input::default());
        }
        assert_ne!(
            first,
            g.pickups[0].item.shape.as_ref().unwrap().packed_vertices
        );
        for _ in 0..2 {
            g.step(Input::default());
        }
        assert_eq!(
            first,
            g.pickups[0].item.shape.as_ref().unwrap().packed_vertices
        );
        assert_eq!(record, g.pickups[0].record);
        assert_eq!(bounds, (g.pickups[0].item.min, g.pickups[0].item.max));
    }

    #[test]
    fn pickup_contact_refills_relocates_and_resets_all_subtypes() {
        for subtype in 0..4 {
            let mut g = Game::new(World { obstacles: vec![] }).unwrap();
            let mut catalog = crate::scene::Catalog {
                choices: vec![[0; 3]; 47],
                ..Default::default()
            };
            catalog.choices[5] = [6; 3];
            let item = catalog
                .instantiate(&World {
                    obstacles: vec![Obstacle {
                        kind: 5,
                        h: 0,
                        v: 0,
                        facing: 0,
                    }],
                })
                .pop()
                .unwrap();
            g.pickups =
                crate::pickups::create(&item, vec![], [5, 2, 1, 2], &mut g.pickup_random).unwrap();
            g.pickups.truncate(1);
            let original = g.position_fixed;
            let pickup = &mut g.pickups[0];
            pickup.item.x = g.tank.x;
            pickup.item.z = g.tank.z;
            pickup.subtype = subtype;
            for (axis, value) in original.iter().enumerate() {
                pickup.record[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_be_bytes());
            }
            pickup.record[0x1c..0x1e].copy_from_slice(&subtype.to_be_bytes());
            let mut models = crate::model_slots::ModelSlots::default();
            models.state.current[6] = 1;
            models.state.current[5] = 2;
            models.geometry.insert(
                1,
                crate::model::PackedShape {
                    vertices: vec![],
                    dimensions: [18 << 16, 35 << 16],
                    candidate_radius: 20,
                },
            );
            models
                .geometry
                .insert(2, crate::model::PackedShape::ammo_dump());
            g.collision_models = Some(models);
            g.enable_flag_objective();
            g.encounter.health = 2;
            g.ammo = 3;
            g.step(Input::default());
            assert_eq!(
                g.pickups_collected, 1,
                "subtype {subtype} must use recovered contact query"
            );
            let expected = if matches!(subtype, 1 | 2) {
                (6, 40)
            } else {
                (3, 23)
            };
            assert_eq!((g.encounter.health, g.ammo), expected);
            assert_eq!(g.flight_energy, if subtype == 3 { 400 } else { 0 });
            assert_eq!(g.pending_shields.len(), usize::from(subtype == 2));
            assert!(
                g.audio_events
                    .contains(&[10011, 10030, 10026, 10016][subtype as usize])
            );
            assert_ne!(
                [g.pickups[0].item.x, g.pickups[0].item.z],
                [g.tank.x, g.tank.z]
            );
            for _ in 0..3 {
                g.step(Input::default());
            }
            assert_eq!(
                g.pickups_collected, 1,
                "relocated dump cannot refill again in place"
            );
            g.reset();
            assert_eq!(g.pickups_collected, 0);
            assert_eq!(g.flight_energy, 0);
            assert!(g.pending_shields.is_empty());
            assert_eq!(g.pickups[0].subtype, subtype);
            assert_eq!(
                [g.pickups[0].item.x, g.pickups[0].item.z],
                [g.tank.x, g.tank.z]
            );
        }
    }

    #[test]
    fn recovered_surface_contacts_change_handling_and_clear_on_exit() {
        fn on_surface(kind: i16) -> Game {
            let mut g = Game::new(World {
                obstacles: vec![Obstacle {
                    h: 500,
                    v: 600,
                    kind: 0,
                    facing: 0,
                }],
            })
            .unwrap();
            g.items[0].kind = kind;
            g.items[0].x = g.tank.x;
            g.items[0].z = g.tank.z;
            let mut models = crate::model_slots::ModelSlots::default();
            models.state.current[6] = 1;
            models.state.current[kind as usize] = 2;
            for (slot, size, radius) in
                [(1, [18 << 16, 35 << 16], 20), (2, [60 << 16, 60 << 16], 43)]
            {
                models.geometry.insert(
                    slot,
                    crate::model::PackedShape {
                        vertices: vec![],
                        dimensions: size,
                        candidate_radius: radius,
                    },
                );
            }
            g.collision_models = Some(models);
            g.speed_byte = 5;
            g
        }
        for kind in [16, 23] {
            let mut acid = on_surface(kind);
            acid.flags.enabled = true;
            acid.encounter.enabled = true;
            acid.encounter.health = 10;
            acid.acid_damage = 3;
            acid.damage_grace_start = 0;
            if kind == 23 {
                let item = acid.items.remove(0);
                acid.add_target(item, 10).unwrap();
                acid.targets[0].item.x = acid.tank.x;
                acid.targets[0].item.z = acid.tank.z;
                acid.targets[0].motion.radial_velocity = 0;
                acid.targets[0].fire.weapon = -1;
            }
            acid.step(Input::default());
            assert_eq!(acid.tank.surface, "acid");
            assert_eq!(acid.encounter.health, 7);
            assert_eq!(acid.acid_deadline, 151);
            assert!(acid.audio_events.contains(&10029));
            assert!(!acid.tank.blocked, "acid is passable");
            for _ in 0..150 {
                acid.step(Input::default());
            }
            assert_eq!(
                acid.encounter.health, 7,
                "deadline equality does not damage"
            );
            acid.step(Input::default());
            assert_eq!(acid.encounter.health, 4);
            assert_eq!(acid.combat_counters.player_damage_events, 2);
            for _ in 0..302 {
                acid.step(Input::default());
            }
            assert_eq!(acid.encounter.health, 0);
            assert!(
                acid.death_transition.is_some(),
                "acid uses the normal death transition"
            );
            assert_eq!(acid.combat_counters.player_damage_events, 4);
            acid.reset();
            assert_eq!(acid.acid_deadline, 0);
        }
        let mut ice = on_surface(17);
        let start = ice.tank.z;
        ice.step(Input::default());
        assert_eq!(ice.tank.surface, "ice");
        assert!(ice.audio_events.contains(&10036));
        assert_eq!(ice.ice_sound_deadline, 6000);
        assert_eq!(
            ice.speed_byte, 5,
            "ice retains speed with released throttle"
        );
        assert!(ice.tank.z > start);
        assert!(!ice.tank.blocked, "ground surface is passable");
        let heading = ice.heading_fixed;
        ice.step(Input {
            throttle: 1.,
            steer: 1.,
            ..Input::default()
        });
        assert_eq!(
            ice.heading_fixed, heading,
            "original negative-adjustment steering guard"
        );
        assert_eq!(
            ice.audio_events.iter().filter(|&&id| id == 10036).count(),
            1
        );
        ice.items.clear();
        ice.step(Input::default());
        assert_eq!(ice.tank.surface, "none");
        assert_eq!(ice.ice_sound_deadline, ice.tank.ticks as i32 - 1);
        assert_eq!(ice.mud_sound_deadline, ice.ice_sound_deadline);
        assert_eq!(
            ice.speed_byte, 0,
            "ordinary coast-zero handling returns after exit"
        );
        for (degrees, expected) in [
            (0., [0., -3.]),
            (90., [3., 0.]),
            (180., [0., 3.]),
            (270., [-3., 0.]),
        ] {
            let mut flow = on_surface(39);
            flow.items[0].heading = (degrees as f32).to_radians();
            let start = [flow.tank.x, flow.tank.z];
            flow.step(Input::default());
            assert_eq!(flow.tank.surface, "flow");
            assert_eq!(
                flow.speed_byte, 0,
                "flow pushes position, not propulsion speed"
            );
            assert!((flow.tank.x - start[0] - expected[0]).abs() < 0.001);
            assert!((flow.tank.z - start[1] - expected[1]).abs() < 0.001);
            flow.step(Input::default());
            assert!((flow.tank.x - start[0] - expected[0] * 2.).abs() < 0.002);
            assert!((flow.tank.z - start[1] - expected[1] * 2.).abs() < 0.002);
            flow.items.clear();
            flow.step(Input::default());
            assert_eq!(flow.flow_last_contact, None);
            flow.reset();
            assert_eq!(flow.flow_last_contact, None);
        }
        let mut mud = on_surface(18);
        mud.step(Input {
            throttle: 1.,
            ..Input::default()
        });
        assert_eq!(mud.tank.surface, "mud");
        assert!(mud.audio_events.contains(&10035));
        assert_eq!(mud.mud_sound_deadline, 60);
        assert_eq!(mud.speed_byte, 1);
        assert!(!mud.tank.blocked);
        let mut resting_mud = on_surface(18);
        for _ in 0..60 {
            resting_mud.step(Input::default());
        }
        assert_eq!(
            resting_mud
                .audio_events
                .iter()
                .filter(|&&id| id == 10035)
                .count(),
            1
        );
        resting_mud.step(Input::default());
        assert_eq!(
            resting_mud
                .audio_events
                .iter()
                .filter(|&&id| id == 10035)
                .count(),
            2
        );
        resting_mud.items[0].x += 300.;
        resting_mud.step(Input::default());
        resting_mud.items[0].x -= 300.;
        resting_mud.step(Input::default());
        assert_eq!(
            resting_mud
                .audio_events
                .iter()
                .filter(|&&id| id == 10035)
                .count(),
            3,
            "ordinary movement resets the shared clock before re-entry"
        );
        let mut moving_mud = on_surface(24);
        let item = moving_mud.items.remove(0);
        moving_mud.add_target(item, 2).unwrap();
        moving_mud.targets[0].item.x = moving_mud.tank.x;
        moving_mud.targets[0].item.z = moving_mud.tank.z;
        moving_mud.step(Input {
            throttle: 1.,
            ..Input::default()
        });
        assert_eq!(moving_mud.tank.surface, "mud");
        assert_eq!(moving_mud.speed_byte, 1);
        assert!(
            !moving_mud.tank.blocked,
            "moving mud is a passable slowing surface"
        );
        moving_mud.targets[0].item.x += 300.;
        moving_mud.step(Input {
            throttle: 1.,
            ..Input::default()
        });
        assert_eq!(
            moving_mud.tank.surface, "none",
            "contact follows the moving actor"
        );
        assert!(
            moving_mud.speed_byte > 1,
            "normal acceleration returns after contact ends"
        );
        mud.reset();
        assert_eq!(
            mud.tank.surface, "unavailable",
            "reset clears surface status before resampling"
        );
    }
}
