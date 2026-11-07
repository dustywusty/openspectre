//! Encounter adapter: recovered type-7/type-8 and moving-surface AI paths.
//! Runtime records, Random generation, and OS angle conversion remain provisional.
use super::*;

#[derive(Clone, Debug, Serialize)]
pub struct EnemyNavigation {
    pub attack_range: i32,
    pub effective_difficulty: i16,
    pub intent: crate::recovered::AiIntentState,
    pub tracked_pose: [i32; 3],
    pub last_check: i32,
    pub phase_deadline: i32,
    pub mode: u8,
    pub movement: crate::recovered::AiMovementState,
    pub aim: crate::recovered::AiAimCache,
}
impl Default for EnemyNavigation {
    fn default() -> Self {
        Self {
            attack_range: 250 << 16,
            effective_difficulty: 1,
            intent: crate::recovered::AiIntentState {
                behavior: 3,
                target: 0,
                movement: 3,
                aim_target: 0,
                heading_deadline: -1,
                field_26: 0,
            },
            tracked_pose: [0; 3],
            last_check: -1,
            phase_deadline: -1,
            mode: 3,
            movement: crate::recovered::AiMovementState {
                heading: 0,
                angle_threshold: 40 << 16,
                reverse: 0,
                deadline: -1,
            },
            // The preview starts at tick zero instead of the original uptime.
            // Force an initial refresh instead of consuming an empty cache.
            aim: crate::recovered::AiAimCache {
                pose: [0; 3],
                angle: 0,
                timestamp: -5,
            },
        }
    }
}
impl EnemyNavigation {
    pub fn reset(&mut self) {
        *self = Self {
            attack_range: self.attack_range,
            effective_difficulty: self.effective_difficulty,
            ..Self::default()
        };
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EnemyMotionParameters {
    pub radial_velocity: i16,
    pub angular_rate: i32,
}
impl EnemyMotionParameters {
    pub fn from_parameters(parameters: &crate::formats::Beastie) -> Self {
        Self {
            radial_velocity: parameters.radial_velocity,
            angular_rate: (parameters.angular_velocity as i32) << 16,
        }
    }
}
impl Default for EnemyMotionParameters {
    fn default() -> Self {
        Self::from_parameters(&crate::recovered::default_enemy_parameters(7))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EnemyFireState {
    pub weapon: i16,
    pub interval: i16,
    pub short_range: i32,
    pub arc: i16,
    pub timer: i32,
}
impl EnemyFireState {
    pub fn from_parameters(parameters: &crate::formats::Beastie) -> Self {
        Self {
            weapon: parameters.weapon,
            interval: parameters.firing_interval,
            short_range: (parameters.short_firing_range as i32) << 16,
            arc: parameters.firing_arc,
            timer: -1,
        }
    }
}
impl Default for EnemyFireState {
    fn default() -> Self {
        Self::from_parameters(&crate::recovered::default_enemy_parameters(7))
    }
}

/// Candidate OS conversion, not verified System 7 FixRatio/AngleFromSlope.
/// Preserves the historical scan bias but estimates its table with atan.
pub(super) fn candidate_slope_angle(numerator: i16, denominator: i16) -> i16 {
    let slope = if denominator == 0 {
        if numerator < 0 { i32::MIN } else { i32::MAX }
    } else {
        (numerator as i64 * 65536 / denominator as i64).clamp(i32::MIN as i64, i32::MAX as i64)
            as i32
    };
    let biased = slope.wrapping_abs().wrapping_sub(500);
    let angle = (biased as f64 / 65536.)
        .atan()
        .to_degrees()
        .ceil()
        .clamp(0., 90.) as i16;
    if slope < 0 { angle } else { 180 - angle }
}

#[derive(Clone, Debug, Serialize)]
pub struct Encounter {
    pub enabled: bool,
    pub health: i16,
    pub status: &'static str,
    pub enemy_fired: u64,
    pub hit_flash: u8,
    pub(super) unlock_started: bool,
}
impl Default for Encounter {
    fn default() -> Self {
        Self {
            enabled: false,
            health: 6,
            status: "playing",
            enemy_fired: 0,
            hit_flash: 0,
            unlock_started: false,
        }
    }
}
impl Game {
    /// Changing modes starts a new round, so research replays retain their baseline.
    pub fn set_encounter(&mut self, enabled: bool) {
        self.encounter.enabled = enabled;
        self.reset();
    }
    pub(super) fn enemy_pose_free(&self, index: usize, x: f32, z: f32, heading: f32) -> bool {
        let mut item = self.targets[index].item.clone();
        item.x = x;
        item.z = z;
        item.heading = heading;
        let outline = item.footprint(true);
        outline
            .iter()
            .flatten()
            .all(|v| (0. ..=ARENA_SIZE).contains(v))
            && (x - self.tank.x).hypot(z - self.tank.z) > 40.
            && self
                .items
                .iter()
                .map(|item| (item, false))
                .chain(
                    self.targets
                        .iter()
                        .enumerate()
                        .filter(|(i, t)| *i != index && t.health > 0)
                        .map(|(_, t)| (&t.item, true)),
                )
                .filter(|(other, _)| other.kind != 12)
                .all(|(other, participant)| {
                    !crate::scene::footprints_overlap(&outline, &other.footprint(participant))
                })
    }
    pub(super) fn step_encounter(&mut self) {
        self.encounter.hit_flash = self.encounter.hit_flash.saturating_sub(1);
        if !self.flags.enabled && self.targets.is_empty() {
            self.encounter.status = "no_enemies";
            return;
        }
        // Temporary round objective: clear escorts, then expose type 31 using its
        // recovered animation. The original movie/event scheduler is not recovered.
        if !self.encounter.unlock_started
            && self
                .targets
                .iter()
                .all(|t| t.health <= 0 || t.item.kind == 31)
        {
            self.replay_type31_event();
            self.encounter.unlock_started = true;
        }
        if !self.flags.enabled
            && self.encounter.health > 0
            && self.targets.iter().all(|t| t.health <= 0)
        {
            self.encounter.status = "won";
            self.enemy_shots.clear();
            return;
        }
        let now = self.tank.ticks as i32;
        // Preview participant order: player first, then at most eleven enemies.
        // Refresh positions as each enemy moves, as a sequential dispatcher does.
        let mut flags = [[0_u8; 2]; 12];
        let mut positions = [[0_i32; 2]; 12];
        flags[0] = [1, 0];
        positions[0] = [(self.tank.x * 65536.) as i32, (self.tank.z * 65536.) as i32];
        for (slot, target) in self.targets.iter().take(11).enumerate() {
            flags[slot + 1] = [u8::from(target.health > 0), 1];
            positions[slot + 1] = [
                (target.item.x * 65536.) as i32,
                (target.item.z * 65536.) as i32,
            ];
        }
        for index in 0..self.targets.len() {
            let t = &self.targets[index];
            if t.health <= 0 || !matches!(t.item.kind, 7 | 8 | 23 | 24) {
                continue;
            }
            let kind = t.item.kind as u8;
            let surface = matches!(kind, 23 | 24);
            let (x, z, heading) = (t.item.x, t.item.z, t.item.heading);
            let pose = [
                (x * 65536.) as i32,
                (z * 65536.) as i32,
                crate::recovered::wrap_angle(
                    ((90. - heading.to_degrees()).rem_euclid(360.) * 65536.) as i32,
                ),
            ];
            let radial = t.motion.radial_velocity;
            let angular = t.motion.angular_rate;
            let yielding = index < 11
                && crate::recovered::ai_nearby_predecessor(index as i8 + 2, &flags, &positions)
                    .unwrap_or(false);
            let player = [(self.tank.x * 65536.) as i32, (self.tank.z * 65536.) as i32];
            let distance = crate::recovered::ai_planar_distance(player, [pose[0], pose[1]]);
            // CODE 25 initializes the command-distance limit to 500 units.
            // Original controls can change it; the preview has no such control yet.
            if crate::recovered::ai_commands_suppressed(
                t.item.kind as u8,
                0,
                self.targets.iter().filter(|target| !target.retired).count() as i16,
                distance,
                500 << 16,
            ) {
                continue;
            }
            let mut navigation = t.navigation.clone();
            // Single-player participant adapter: player is slot 1. Enemy counters
            // used by the local auto-control handler are not incremented here.
            crate::recovered::refresh_ai_aim_cache(
                now,
                1,
                kind,
                &mut navigation.aim,
                |mode| {
                    let target = [player[0], player[1], self.heading_fixed];
                    crate::recovered::ai_predict_pose(
                        pose,
                        target,
                        self.speed_byte,
                        mode,
                        &self.unit_table,
                    )
                    // Host guard for original division faults or invalid headings.
                    .unwrap_or(target)
                },
                |target| {
                    crate::recovered::ai_aim_angle(
                        [target[0], target[1]],
                        pose,
                        candidate_slope_angle,
                    )
                    .unwrap_or(0)
                },
            );
            let mut tracking = crate::recovered::AiTrackingParticipant {
                intent: navigation.intent,
                tracked_pose: navigation.tracked_pose,
                counter: 0,
                active: 1,
            };
            tracking.intent.heading_deadline = navigation.movement.deadline;
            if tracking.intent.behavior == 3 && surface {
                crate::recovered::ai_initialize_surface(
                    &mut tracking,
                    &mut navigation.last_check,
                    1,
                    |_| navigation.aim.pose,
                );
            } else if tracking.intent.behavior == 3 && kind == 8 {
                // CODE 20 +0b6c: kind 8 skips the world-object search.
                crate::recovered::ai_initialize_type7(
                    &mut tracking,
                    &mut navigation.last_check,
                    1,
                    0,
                    distance,
                    navigation.attack_range,
                    |_| navigation.aim.pose,
                );
            } else if tracking.intent.behavior == 3 {
                let assignments = std::array::from_fn(|slot| {
                    let target = slot.checked_sub(1).and_then(|i| self.targets.get(i));
                    crate::recovered::AiObjectAssignment {
                        active: u8::from(target.is_some_and(|t| !t.retired)),
                        owner: u8::from(target.is_some()),
                        behavior: target.map_or(0, |t| t.navigation.intent.behavior),
                        target: target.map_or(0, |t| t.navigation.intent.target),
                    }
                });
                // Stable placement slots also survive flag collection. These IDs
                // match the spawn adapter, not the compact collision-query list.
                let objects: Vec<_> = self
                    .placement_items
                    .iter()
                    .enumerate()
                    .map(|(slot, item)| crate::recovered::AiObjectCandidate {
                        kind: item.kind as u8,
                        position: [(item.x * 65536.) as i32, (item.z * 65536.) as i32],
                        assignments: crate::recovered::ai_object_assignment_count(
                            slot as i16 + 1,
                            &assignments,
                        ),
                    })
                    .collect();
                let remaining = self.targets.iter().filter(|t| !t.retired).count() as i16;
                let object = crate::recovered::ai_select_object(
                    [pose[0], pose[1]],
                    player,
                    navigation.effective_difficulty,
                    remaining,
                    &objects,
                    || {
                        // Reproducible adapter stream, not the original Random generator.
                        self.spawn_random ^= self.spawn_random << 13;
                        self.spawn_random ^= self.spawn_random >> 17;
                        self.spawn_random ^= self.spawn_random << 5;
                        (self.spawn_random >> 16) as i16
                    },
                )
                .unwrap_or(0);
                crate::recovered::ai_initialize_type7(
                    &mut tracking,
                    &mut navigation.last_check,
                    1,
                    object,
                    distance,
                    navigation.attack_range,
                    |_| navigation.aim.pose,
                );
            }
            let action = crate::recovered::ai_intent_action(crate::recovered::AiIntentCheck {
                now,
                last_check: navigation.last_check,
                distance,
                attack_range: navigation.attack_range,
                kind,
                behavior: tracking.intent.behavior as i8,
                movement: tracking.intent.movement,
                game_mode: 0,
                target: tracking.intent.target,
                participant: index as i8 + 2,
            })
            .0;
            match action {
                Some(0xe6c) => crate::recovered::ai_begin_attack(&mut tracking.intent, 1),
                Some(0x1240) if surface => {
                    // Enemy counters are zero in this single-player adapter:
                    // compose the verified shared tail and surface phase directly.
                    crate::recovered::ai_type7_tracking_tail(
                        &mut tracking.intent,
                        &mut tracking.tracked_pose,
                        0,
                        1,
                        || navigation.aim.pose,
                    );
                    (
                        tracking.intent.behavior,
                        tracking.intent.movement,
                        navigation.phase_deadline,
                    ) = crate::recovered::ai_surface_phase(
                        now,
                        tracking.intent.behavior,
                        tracking.intent.movement,
                        navigation.phase_deadline,
                    );
                }
                Some(0xfb8) if surface => {
                    let (behavior, movement, deadline, sound) =
                        crate::recovered::ai_cooldown(crate::recovered::AiCooldown {
                            kind: kind as i8,
                            behavior: tracking.intent.behavior,
                            movement: tracking.intent.movement,
                            deadline: navigation.phase_deadline,
                            now,
                            distance,
                            local_dying: u8::from(self.death_transition.is_some()),
                        });
                    tracking.intent.behavior = behavior;
                    tracking.intent.movement = movement;
                    navigation.phase_deadline = deadline;
                    if let Some(id) = sound.and_then(crate::recovered::sound_request_id) {
                        self.audio_events.push(id);
                    }
                }
                Some(0xef4) => {
                    crate::recovered::ai_attack_tracking(
                        &mut tracking,
                        0,
                        pose,
                        navigation.effective_difficulty,
                        || navigation.aim.pose,
                    );
                }
                Some(0x1240) => {
                    crate::recovered::ai_type7_tracking_tail(
                        &mut tracking.intent,
                        &mut tracking.tracked_pose,
                        0,
                        1,
                        || navigation.aim.pose,
                    );
                }
                Some(0x153a) => {
                    if let Some(item) = usize::try_from(tracking.intent.target - 1)
                        .ok()
                        .and_then(|slot| self.placement_items.get(slot))
                    {
                        crate::recovered::ai_track_object(
                            &mut tracking,
                            index as i8 + 2,
                            pose,
                            [
                                (item.x * 65536.) as i32,
                                (item.z * 65536.) as i32,
                                item.native_heading(false),
                            ],
                        );
                    }
                }
                _ => {}
            }
            navigation.intent = tracking.intent;
            navigation.tracked_pose = tracking.tracked_pose;
            navigation.mode = tracking.intent.movement;
            navigation.movement.deadline = tracking.intent.heading_deadline;
            let mode = navigation.mode;
            // Ordinary movement reads the stored target pose, including the
            // original five-tick cache lag for participant pursuit.
            let destination = [tracking.tracked_pose[0], tracking.tracked_pose[1]];
            let query = self
                .collision_models
                .as_ref()
                .and_then(|models| crate::collision_preview::Query::from_game(self, models));
            let mut clearance = |heading: i32, width: i32, distance: i32| {
                query
                    .as_ref()
                    .and_then(|q| q.enemy_clearance(index, heading, [width, distance]))
                    .unwrap_or_else(|| {
                        // Unsupported model slots retain an explicit preview footprint probe.
                        let direction = crate::recovered::candidate_collision_table()
                            [crate::recovered::direction_table_index(heading)]
                        .map(i32::from);
                        crate::recovered::ai_clearance_probe(
                            [pose[0], pose[1]],
                            heading,
                            distance,
                            1024,
                            direction,
                            |center| {
                                let h = (90. - center[2] as f32 / 65536.).to_radians();
                                (!self.enemy_pose_free(
                                    index,
                                    center[0] as f32 / 65536.,
                                    center[1] as f32 / 65536.,
                                    h,
                                ))
                                .then_some(0)
                            },
                        )
                    })
            };
            use crate::recovered::AiMovementPolicy;
            let (steering, mut throttle) =
                match crate::recovered::ai_movement_policy(mode, yielding, distance) {
                    AiMovementPolicy::Ordinary => crate::recovered::ai_standard_movement(
                        pose,
                        destination,
                        now,
                        &mut navigation.movement,
                        || {
                            crate::recovered::ai_aim_angle(destination, pose, candidate_slope_angle)
                                .unwrap_or(0)
                        },
                        &mut clearance,
                    ),
                    AiMovementPolicy::Combat => crate::recovered::ai_combat_movement(
                        pose,
                        [navigation.aim.pose[0], navigation.aim.pose[1]],
                        now,
                        &mut navigation.movement,
                        None,
                        || navigation.aim.angle,
                        &mut clearance,
                    )
                    .unwrap_or_else(|| {
                        // Preserve steering but hold position on the original unwritten-local path.
                        (crate::recovered::ai_turn_only(navigation.aim.angle), 0)
                    }),
                    AiMovementPolicy::TurnOnly => {
                        (crate::recovered::ai_turn_only(navigation.aim.angle), 0)
                    }
                    _ => (0, 0),
                };
            if yielding {
                throttle = 0;
            }
            navigation.intent.heading_deadline = navigation.movement.deadline;
            self.targets[index].navigation = navigation;
            let next = crate::recovered::move_type7(
                pose,
                [steering, throttle],
                radial,
                angular,
                1,
                &self.direction_table,
            )
            .unwrap();
            let nx = next[0] as f32 / 65536.;
            let nz = next[1] as f32 / 65536.;
            let next_heading = (90. - next[2] as f32 / 65536.)
                .to_radians()
                .rem_euclid(std::f32::consts::TAU);
            if self.enemy_pose_free(index, nx, nz, next_heading) {
                let item = &mut self.targets[index].item;
                item.x = nx;
                item.z = nz;
                item.heading = next_heading;
            } else if self.enemy_pose_free(index, x, z, next_heading) {
                self.targets[index].item.heading = next_heading;
            }
            let item = &self.targets[index].item;
            if index < 11 {
                positions[index + 1] = [(item.x * 65536.) as i32, (item.z * 65536.) as i32];
            }
            let heading_fixed = crate::recovered::wrap_angle(
                ((90. - item.heading.to_degrees()).rem_euclid(360.) * 65536.) as i32,
            );
            let source = [(item.x * 65536.) as i32, (item.z * 65536.) as i32];
            let player = [(self.tank.x * 65536.) as i32, (self.tank.z * 65536.) as i32];
            // The original caller clears the angle result slot. Coincident
            // coordinates leave that zero unchanged. Native guards do likewise.
            let aim = if crate::recovered::ai_fire_uses_cached_angle(
                1,
                index as i8 + 2,
                self.targets[index].navigation.intent.movement,
            ) {
                self.targets[index].navigation.aim.angle
            } else {
                crate::recovered::ai_aim_angle(
                    player,
                    [source[0], source[1], heading_fixed],
                    candidate_slope_angle,
                )
                .unwrap_or(0)
            };
            let fire = &self.targets[index].fire;
            let (eligible, timer, _, _, _) = crate::recovered::enemy_fire_check(
                crate::recovered::Type7FireCheck {
                    now,
                    timer: fire.timer,
                    distance: crate::recovered::ai_planar_distance(player, source),
                    interval: fire.interval,
                    short_range: fire.short_range,
                    firing_arc: fire.arc,
                    angle: aim,
                    // Original geometry +28 is signed fixed-point vertical position.
                    target_field_28: self.vertical.vertical,
                },
                kind,
                0,
                0,
                self.targets[index].navigation.intent.behavior,
                self.targets[index].navigation.intent.movement,
            );
            self.targets[index].fire.timer = timer;
            if eligible {
                // CODE 20 +0x26dc records the accepted firing command.
                self.targets[index].fire.timer = now;
                let requests = crate::recovered::enemy_weapon_requests(
                    1,
                    self.targets[index].fire.weapon,
                    0,
                    0,
                    0,
                );
                for (weapon, _, _) in requests {
                    // CODE 30 admission uses the firing owner's active-shot count.
                    // Preview target slots remain stable across death and respawn.
                    let active_shots = self
                        .enemy_shots
                        .iter()
                        .filter(|shot| shot.enemy_owner == Some(index) && !shot.retiring)
                        .count();
                    if active_shots >= crate::recovered::concurrent_shot_limit(1) as usize {
                        continue;
                    }
                    let (_, shot_heading) = crate::recovered::weapon_initial_heading(
                        weapon,
                        heading_fixed,
                        || {
                            self.spawn_random ^= self.spawn_random << 13;
                            self.spawn_random ^= self.spawn_random >> 17;
                            self.spawn_random ^= self.spawn_random << 5;
                            (self.spawn_random >> 16) as i16
                        },
                        // Targeted weapon construction still uses the preview path.
                        || None,
                    );
                    let item = &self.targets[index].item;
                    let unit = crate::recovered::movement_direction(shot_heading, &self.unit_table)
                        .unwrap();
                    let (mut position_fixed, mut deadline) = crate::recovered::launch_basic_shot(
                        [(item.x * 65536.) as i32, (item.z * 65536.) as i32],
                        unit,
                        now,
                        75,
                    );
                    let mut bouncing_record = None;
                    let mut height = 4.;
                    if weapon == 6 {
                        let mut record = [0; 128];
                        for (offset, value) in [
                            (0, (item.x * 65536.) as i32),
                            (4, (item.z * 65536.) as i32),
                            (8, shot_heading),
                        ] {
                            record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                        }
                        crate::recovered::initialize_weapon_record(
                            &mut record,
                            crate::recovered::WeaponRecordInput {
                                weapon,
                                now,
                                lifetime: 75,
                                owner: 1,
                                source_kind: item.kind as u8,
                                owner_field_5c: 0,
                                direction: unit,
                            },
                            || {
                                self.spawn_random ^= self.spawn_random << 13;
                                self.spawn_random ^= self.spawn_random >> 17;
                                self.spawn_random ^= self.spawn_random << 5;
                                (self.spawn_random >> 16) as i16
                            },
                        );
                        position_fixed = [0, 4]
                            .map(|i| i32::from_be_bytes(record[i..i + 4].try_into().unwrap()));
                        deadline = i32::from_be_bytes(record[0x16..0x1a].try_into().unwrap());
                        height = f32::from(crate::recovered::render_height_word(
                            i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()),
                        )) / 8.;
                        bouncing_record = Some(record);
                    }
                    self.enemy_shots.push(Shot {
                        lifecycle_record: None,
                        retiring: false,
                        missile: None,
                        weapon: weapon as u8,
                        enemy_owner: Some(index),
                        bouncing_record,
                        height,
                        x: position_fixed[0] as f32 / 65536.,
                        z: position_fixed[1] as f32 / 65536.,
                        heading: (90. - shot_heading as f32 / 65536.)
                            .to_radians()
                            .rem_euclid(std::f32::consts::TAU),
                        life: deadline.wrapping_sub(now).max(0) as u32,
                        position_fixed,
                        direction_fixed: unit.map(|v| v.wrapping_mul(6)),
                        heading_fixed: shot_heading,
                        deadline,
                        last_update: now,
                    });
                    self.encounter.enemy_fired += 1;
                    self.combat_counters.launch(1, (index + 2) as i8);
                    if let Some(request) = crate::recovered::weapon_fire_sound(weapon) {
                        self.queue_sound_request(request);
                    }
                }
            }
        }
        // Ordinary enemy rounds do not use guidance during contact retirement.
        let inert_targets = [crate::recovered::AiTargetCandidate {
            team: 0,
            owner: 0,
            active: 0,
            health: 0,
            kind: 0,
            pose: [0; 3],
        }; 12];
        for shot in &mut self.enemy_shots {
            if shot.retiring {
                if crate::recovered::projectile_lifecycle(
                    shot.lifecycle_record
                        .as_mut()
                        .expect("retired projectile record"),
                    now,
                    &mut [0; 12],
                ) == Some(crate::recovered::ProjectileLifecycleAction::Delete)
                {
                    shot.life = 0;
                }
                continue;
            }
            if crate::recovered::basic_shot_expired(now, shot.deadline) {
                shot.life = 0;
                continue;
            }
            let elapsed = crate::recovered::projectile_elapsed(now, shot.last_update);
            shot.last_update = now;
            let start = shot.position_fixed;
            let start_height = shot.height;
            let start_collision_height = shot.bouncing_record.map_or(0., |record| {
                i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()) as f32 / 65536.
            });
            let end = if let Some(record) = &mut shot.bouncing_record {
                crate::recovered::move_bouncing_projectile(record, shot.direction_fixed, elapsed);
                shot.height = f32::from(crate::recovered::render_height_word(i32::from_be_bytes(
                    record[0x28..0x2c].try_into().unwrap(),
                ))) / 8.;
                [0, 4].map(|i| i32::from_be_bytes(record[i..i + 4].try_into().unwrap()))
            } else {
                crate::recovered::move_basic_shot(start, shot.direction_fixed, elapsed)
            };
            let end_collision_height = shot.bouncing_record.map_or(0., |record| {
                i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()) as f32 / 65536.
            });
            for substep in 1..=6 * elapsed {
                let sample: [i32; 2] = std::array::from_fn(|i| {
                    start[i].wrapping_add(
                        (end[i].wrapping_sub(start[i]) as i64 * substep as i64
                            / (6 * elapsed) as i64) as i32,
                    )
                });
                let sample_height = start_height
                    + (shot.height - start_height) * substep as f32 / (6 * elapsed) as f32;
                shot.x = sample[0] as f32 / 65536.;
                shot.z = sample[1] as f32 / 65536.;
                let wall = !(0. ..=ARENA_SIZE).contains(&shot.x)
                    || !(0. ..=ARENA_SIZE).contains(&shot.z)
                    || self.items.iter().any(|o| {
                        crate::recovered::projectile_object_contact(
                            if shot.bouncing_record.is_some() {
                                29
                            } else {
                                9
                            },
                            1,
                            o.kind as u8,
                            1,
                        ) != crate::recovered::ProjectileObjectContact::Ignore
                            && o.contains(shot.x, shot.z, 1., false)
                            && o.min[1] <= sample_height
                            && o.max[1] >= sample_height
                    });
                let dx = shot.x - self.tank.x;
                let dz = shot.z - self.tank.z;
                let (s, c) = self.tank.heading.sin_cos();
                // Preview player hit box; original ownership/damage dispatch pending.
                let collision_height = start_collision_height
                    + (end_collision_height - start_collision_height) * substep as f32
                        / (6 * elapsed) as f32;
                let player = !wall
                    // Recovered projectile candidate height gate; planar hit box
                    // and interpolated collision scheduling remain preview behavior.
                    && (collision_height - self.vertical.vertical as f32 / 65536.).abs() <= 60.
                    && (dx * c - dz * s).abs() <= 9.
                    && (-17. ..=22.).contains(&(dx * s + dz * c));
                if wall
                    && matches!(shot.weapon, 0 | 3 | 4 | 5 | 6)
                    && shot.retire_basic_contact(
                        end,
                        elapsed,
                        now,
                        &inert_targets,
                        &self.unit_table,
                    )
                {
                    break;
                }
                if wall || player {
                    shot.life = 0;
                    self.impacts.push(Impact {
                        x: shot.x,
                        z: shot.z,
                        life: 18,
                    });
                    if player
                        && self.encounter.health > 0
                        && self.combat_counters.damage(
                            crate::recovered::DamageCounterEvent {
                                local: 1,
                                attacker: 2,
                                victim: 1,
                                geometry_id: 1,
                                mode: 0,
                                now: now.wrapping_add(75),
                            },
                            &mut self.damage_grace_start,
                        )
                    {
                        if !self.infinite_health {
                            self.encounter.health -= 1;
                        }
                        if let Some(event) = crate::radar::damage_disturbance(
                            1,
                            1,
                            self.encounter.health,
                            self.selected_difficulty,
                            now,
                        ) {
                            self.radar_display.get_mut().disturb(&event);
                            self.audio_events
                                .extend(event.sounds.into_iter().filter_map(|(_, request)| {
                                    crate::recovered::sound_request_id(request)
                                }));
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
                        self.audio_events
                            .extend(sounds.requests.into_iter().filter_map(|(_, request)| {
                                crate::recovered::sound_request_id(request)
                            }));
                    }
                    break;
                }
            }
            if !shot.retiring {
                shot.position_fixed = end;
            }
        }
        self.enemy_shots.retain(|s| s.life > 0);
        self.start_player_death_if_needed();
    }
    pub(super) fn start_player_death_if_needed(&mut self) {
        if self.encounter.health == 0 && self.death_transition.is_none() {
            // CODE 31 local-death notifications. Preview target slots map to
            // participants 2..12; unrepresented AI fields remain recovery work.
            let mut active = [0; 12];
            let mut enemy = [0; 12];
            for (index, target) in self.targets.iter().take(11).enumerate() {
                active[index + 1] = u8::from(!target.retired && target.health > 0);
                enemy[index + 1] = 1;
            }
            for participant in crate::recovered::death_ai_reset_targets(&active, &enemy, 0) {
                let target = &mut self.targets[participant as usize - 2];
                let mut record = [0; 256];
                crate::recovered::reset_ai_participant(&mut record, participant, &mut [0; 16]);
                // Copy only represented reset fields. Original reset preserves
                // tracked pose, targets, heading, reverse, and aim cache here.
                target.navigation.intent.behavior = record[9];
                target.navigation.intent.movement = record[0x10];
                target.navigation.mode = record[0x10];
                target.fire.timer = i32::from_be_bytes(record[0x44..0x48].try_into().unwrap());
                target.navigation.phase_deadline =
                    i32::from_be_bytes(record[0x4e..0x52].try_into().unwrap());
            }
            self.begin_player_death();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::Obstacle;
    fn duel(distance: i16) -> Game {
        let mut g = Game::new(World { obstacles: vec![] }).unwrap();
        let item = crate::scene::Catalog::default()
            .instantiate(&World {
                obstacles: vec![Obstacle {
                    h: g.tank.x as i16,
                    v: g.tank.z as i16 + distance,
                    kind: 7,
                    facing: 180,
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
            kill_score: 10,
            item,
            health: 1,
            max_health: 1,
            vulnerable: true,
            animation_frame: 0,
        });
        g.capture_targets();
        g.set_encounter(true);
        g
    }
    #[test]
    fn death_transition_blocks_controls_and_finishes_at_original_deadline() {
        for lives in [1, 2] {
            let mut g = duel(130);
            g.flags.enabled = true;
            g.mission.lives = lives;
            g.encounter.health = 0;
            g.begin_player_death();
            let start = g.tank.ticks;
            let position = [g.tank.x, g.tank.z, g.tank.heading];
            let ammo = g.ammo;
            let camera = g.death_camera().unwrap();
            let mut first = crate::render::Frame::default();
            first.first_person(&g);
            let input = Input {
                throttle: 1.,
                steer: 1.,
                fire: true,
                ..Input::default()
            };
            for _ in 0..259 {
                g.advance(STEP, input);
                assert_eq!([g.tank.x, g.tank.z, g.tank.heading], position);
                assert_eq!(g.ammo, ammo);
                assert_eq!(g.fired, 0);
                assert_eq!(g.encounter.health, 0);
                assert_eq!(g.mission.deaths, 0);
                assert_eq!(g.encounter.status, "playing");
            }
            assert_eq!(g.tank.ticks, start + 259);
            assert!(
                g.encounter.enemy_fired > 0,
                "world continues during death transition"
            );
            assert_ne!(g.death_camera().unwrap().pose, camera.pose);
            let mut later = crate::render::Frame::default();
            later.first_person(&g);
            assert_ne!(first.rgba, later.rgba);
            g.advance(STEP, input);
            assert!(g.death_transition.is_none());
            assert_eq!(g.tank.ticks, start + 260);
            assert_eq!(g.mission.deaths, 1);
            assert_eq!(g.mission.lives, lives - 1);
            assert_eq!(
                g.encounter.status,
                if lives == 1 { "lost" } else { "playing" }
            );
            if lives == 1 {
                g.advance(STEP, input);
                assert_eq!(g.tank.ticks, start + 260);
            } else {
                assert!(g.encounter.health > 0);
                g.advance(STEP, Input::default());
                assert_eq!(g.mission.deaths, 1);
            }
            g.reset();
            assert!(g.death_transition.is_none());
        }
    }

    #[test]
    fn scattershot_uses_basic_damage_and_preserves_vulnerability_checks() {
        for vulnerable in [true, false] {
            let mut game = duel(70);
            game.encounter.enabled = false;
            game.targets[0].vulnerable = vulnerable;
            game.optional_weapons.selected = 5;
            game.weapon_ammo_costs[3..=5].fill(1);
            game.primary_damage = 1;
            game.ammo = 3;
            for tick in 0..40 {
                game.step(Input {
                    fire_optional: tick < 16,
                    ..Default::default()
                });
            }
            assert_eq!(game.fired, 3);
            assert_eq!(game.ammo, 0);
            assert_eq!(game.targets[0].health, if vulnerable { 0 } else { 1 });
            assert_eq!(game.destroyed, u32::from(vulnerable));
        }
    }

    #[test]
    fn player_death_notifies_active_enemy_without_erasing_tracking() {
        let mut g = duel(130);
        g.encounter.health = 1;
        g.targets[0].fire.interval = 0;
        g.targets[0].fire.arc = 200;
        let mut died = false;
        for _ in 0..1000 {
            g.advance(STEP, Input::default());
            if g.encounter.health == 0 {
                died = true;
                let enemy = &g.targets[0];
                assert_eq!(enemy.navigation.intent.behavior, 3);
                assert_eq!(enemy.navigation.intent.movement, 3);
                assert_eq!(enemy.navigation.mode, 3);
                assert_eq!(enemy.fire.timer, -1);
                assert_ne!(enemy.navigation.tracked_pose, [0; 3]);
                assert_eq!(enemy.navigation.intent.target, 1);
                break;
            }
        }
        assert!(died);
    }

    #[test]
    fn surface_enemies_pause_pursue_and_restart_their_phase() {
        for kind in [23, 24] {
            let mut g = duel(200);
            g.targets[0].item.kind = kind;
            g.targets[0].fire.weapon = -1;
            g.targets[0].motion = EnemyMotionParameters::from_parameters(
                &crate::recovered::default_enemy_parameters(kind),
            );
            let start = [g.targets[0].item.x, g.targets[0].item.z];
            let mut stopped = false;
            let mut moved = false;
            let mut resumed_pause = false;
            let mut previous = start;
            for _ in 0..200 {
                g.step(Input::default());
                let target = &g.targets[0];
                let current = [target.item.x, target.item.z];
                if target.navigation.intent.behavior == 5 {
                    assert_eq!(current, previous, "phase 5 suppresses translation");
                    stopped = true;
                    resumed_pause |= moved;
                } else if current != previous {
                    moved = true;
                }
                previous = current;
            }
            assert!(
                stopped && moved && resumed_pause,
                "kind {kind} must pause, move, and pause again"
            );
            let end_distance = (previous[0] - g.tank.x).hypot(previous[1] - g.tank.z);
            assert!(end_distance < 200., "kind {kind} must pursue the player");
            if kind == 23 {
                assert!(g.audio_events.contains(&10020));
            }
            assert!(g.enemy_shots.is_empty(), "weapon -1 remains suppressed");
            g.targets[0].navigation.reset();
            assert_eq!(g.targets[0].navigation.phase_deadline, -1);
        }
    }

    #[test]
    fn player_damage_sound_distinguishes_hit_from_final_death() {
        for health in [1, 2] {
            let mut g = duel(130);
            g.encounter.health = health;
            g.selected_difficulty = 3;
            g.targets[0].fire.interval = 0;
            g.targets[0].fire.arc = 200;
            let mut hit = false;
            for _ in 0..1000 {
                g.audio_events.clear();
                g.advance(STEP, Input::default());
                if g.encounter.health < health {
                    hit = true;
                    assert!(g.audio_events.contains(&10009), "low-health warning");
                    assert_eq!(g.radar_display.borrow().stable, 0);
                    assert_eq!(g.radar_display.borrow().deadline, g.tank.ticks as i32 + 180);
                    assert!(
                        g.audio_events
                            .contains(&if health == 1 { 10013 } else { 10003 })
                    );
                    assert!(!g.audio_events.contains(&10005));
                    if health == 1 {
                        assert!(!g.audio_events.contains(&10003));
                    }
                    break;
                }
            }
            assert!(
                hit,
                "enemy must hit the player to exercise live sound routing"
            );
        }
    }

    #[test]
    fn later_enemy_yields_until_earlier_enemy_moves_away() {
        let mut g = duel(200);
        let mut later = g.targets[0].clone();
        later.item.z += 50.;
        g.targets.push(later);
        let first_start = g.targets[0].item.z;
        let later_start = g.targets[1].item.z;
        g.advance(STEP, Input::default());
        assert!(g.targets[0].item.z < first_start);
        assert_eq!(g.targets[1].item.z, later_start);
        // Move the leader out of both priority range and the clearance corridor.
        // This isolates yielding from the newly integrated obstacle search.
        g.targets[0].item.x += 100.;
        g.advance(STEP, Input::default());
        assert!(g.targets[1].item.z < later_start);

        let mut g = duel(200);
        let mut later = g.targets[0].clone();
        later.item.z += 50.;
        let start = later.item.z;
        g.targets.push(later);
        g.targets[0].health = 0;
        g.targets[0].retired = true;
        g.advance(STEP, Input::default());
        assert!(g.targets[1].item.z < start);
    }

    #[test]
    fn ordinary_pursuit_steers_toward_cached_pose_until_refresh() {
        let mut stationary = duel(350);
        let mut shifted = duel(350);
        stationary.advance(STEP, Input::default());
        shifted.advance(STEP, Input::default());
        shifted.position_fixed[0] += 100 << 16;
        shifted.tank.x += 100.;
        stationary.advance(STEP, Input::default());
        shifted.advance(STEP, Input::default());
        assert_eq!(shifted.targets[0].navigation.mode, 0);
        assert_eq!(shifted.targets[0].navigation.aim.timestamp, 1);
        assert_eq!(
            shifted.targets[0].navigation.tracked_pose,
            stationary.targets[0].navigation.tracked_pose
        );
        assert_eq!(
            shifted.targets[0].navigation.movement.heading,
            stationary.targets[0].navigation.movement.heading
        );
        for _ in 0..4 {
            stationary.advance(STEP, Input::default());
            shifted.advance(STEP, Input::default());
        }
        assert_eq!(shifted.targets[0].navigation.aim.timestamp, 6);
        assert_ne!(
            shifted.targets[0].navigation.tracked_pose[0],
            stationary.targets[0].navigation.tracked_pose[0]
        );
        assert_ne!(
            shifted.targets[0].navigation.movement.heading,
            stationary.targets[0].navigation.movement.heading
        );
    }

    #[test]
    fn distant_commands_wait_until_enemy_count_drops_to_five() {
        let mut g = duel(501);
        let template = g.targets[0].clone();
        for _ in 0..5 {
            g.targets.push(template.clone());
        }
        let before = serde_json::to_value(&g.targets[0].navigation).unwrap();
        g.advance(STEP, Input::default());
        assert_eq!(
            serde_json::to_value(&g.targets[0].navigation).unwrap(),
            before
        );
        assert_eq!(g.encounter.enemy_fired, 0);
        // Retire one slot: command generation must resume even at the same distance.
        g.targets[5].retired = true;
        g.targets[5].health = 0;
        g.advance(STEP, Input::default());
        assert_ne!(g.targets[0].navigation.intent.behavior, 3);
        assert!(g.targets[0].navigation.aim.timestamp >= 0);
    }

    #[test]
    fn original_intent_approaches_guards_then_keeps_attack_mode_after_range_exit() {
        let mut g = duel(350);
        g.targets[0].navigation.attack_range = 100 << 16;
        let mut flag = g.targets[0].item.clone();
        flag.kind = 12;
        flag.x += 70.;
        g.placement_items = vec![flag.clone()];
        g.advance(STEP, Input::default());
        let navigation = &g.targets[0].navigation;
        assert_eq!(navigation.intent.behavior, 1);
        assert_eq!(navigation.intent.target, 1);
        assert_eq!(navigation.mode, 0);
        assert_eq!(navigation.tracked_pose[0], (flag.x * 65536.) as i32);
        g.targets[0].item.x = flag.x - 20.;
        g.targets[0].item.z = flag.z;
        g.advance(STEP, Input::default());
        assert_eq!(g.targets[0].navigation.mode, 2);
        assert_eq!(g.targets[0].navigation.intent.aim_target, 1);
        // Move the enemy near the player to trigger the recovered attack transition.
        g.targets[0].item.x = g.tank.x;
        g.targets[0].item.z = g.tank.z + 80.;
        g.advance(STEP, Input::default());
        assert_eq!(g.targets[0].navigation.mode, 1);
        assert_eq!(g.targets[0].navigation.intent.behavior, 0);
        g.targets[0].item.z = g.tank.z + 350.;
        g.advance(STEP, Input::default());
        assert_eq!(g.targets[0].navigation.mode, 1);
        assert_eq!(g.targets[0].navigation.last_check, -1);
    }

    #[test]
    fn combat_policy_uses_original_standoff_and_restores_navigation_on_restart() {
        let mut g = duel(130);
        let initial = serde_json::to_value(&g.targets[0].navigation).unwrap();
        for _ in 0..100 {
            g.advance(STEP, Input::default());
        }
        let separation = g.targets[0].item.z - g.tank.z;
        assert!((118. ..120.).contains(&separation));
        assert_eq!(g.targets[0].navigation.mode, 1);
        assert!(g.targets[0].navigation.movement.deadline > 0);
        assert!(g.targets[0].navigation.aim.timestamp >= 95);
        g.reset();
        assert_eq!(
            serde_json::to_value(&g.targets[0].navigation).unwrap(),
            initial
        );
    }

    #[test]
    fn resource_motion_parameters_control_live_translation_and_turning() {
        let mut slow = duel(250);
        let mut fast = duel(250);
        slow.targets[0].motion.radial_velocity = 4;
        fast.targets[0].motion.radial_velocity = 8;
        let start = slow.targets[0].item.z;
        for _ in 0..10 {
            slow.advance(STEP, Input::default());
            fast.advance(STEP, Input::default());
        }
        assert!((slow.targets[0].item.z - (start - 10.)).abs() < 0.001);
        assert!((fast.targets[0].item.z - (start - 20.)).abs() < 0.001);

        let mut turn = duel(250);
        turn.targets[0].motion.radial_velocity = 0;
        turn.targets[0].motion.angular_rate = 2 << 16;
        turn.targets[0].item.heading = std::f32::consts::FRAC_PI_2;
        let origin = [turn.targets[0].item.x, turn.targets[0].item.z];
        for _ in 0..10 {
            turn.advance(STEP, Input::default());
        }
        assert_eq!([turn.targets[0].item.x, turn.targets[0].item.z], origin);
        assert!((turn.targets[0].item.heading.to_degrees() - 110.).abs() < 0.01);
    }

    #[test]
    fn recovered_firing_timer_starts_on_range_entry_and_reset_restores_it() {
        let mut g = duel(305);
        g.targets[0].fire.interval = 2;
        g.capture_targets();
        for _ in 0..3 {
            g.advance(STEP, Input::default());
        }
        assert_eq!(g.targets[0].fire.timer, -1);
        g.advance(STEP, Input::default());
        assert_eq!(g.targets[0].fire.timer, 4);
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 0);
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 1);
        assert_eq!(g.targets[0].fire.timer, 6);
        g.reset();
        assert_eq!(g.targets[0].fire.timer, -1);
        assert_eq!(g.targets[0].fire.interval, 2);
    }

    #[test]
    fn bouncing_render_scale_does_not_change_player_height_gate() {
        for raw_units in [60, 61] {
            let mut g = duel(130);
            g.targets[0].fire.weapon = 6;
            g.targets[0].fire.interval = 0;
            g.targets[0].fire.arc = 200;
            g.advance(STEP, Input::default());
            g.targets[0].fire.weapon = -1;
            let shot = &mut g.enemy_shots[0];
            shot.position_fixed = g.position_fixed;
            shot.direction_fixed = [0; 2];
            shot.height = raw_units as f32 / 8.;
            let record = shot.bouncing_record.as_mut().unwrap();
            for (offset, value) in [
                (0, g.position_fixed[0]),
                (4, g.position_fixed[1]),
                (0x24, 0),
                (0x28, raw_units << 16),
            ] {
                record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            }
            g.damage_grace_start = 0;
            let health = g.encounter.health;
            g.advance(STEP, Input::default());
            assert_eq!(g.encounter.health, health - i16::from(raw_units == 60));
            if raw_units == 61 {
                assert_eq!(g.enemy_shots.len(), 1);
                assert_eq!(g.enemy_shots[0].height, 61. / 8.);
            } else {
                assert!(g.enemy_shots.is_empty());
            }
        }
    }

    #[test]
    fn bouncing_enemy_shot_rises_lands_rebounds_and_expires() {
        let mut g = duel(130);
        g.targets[0].fire.weapon = 6;
        g.targets[0].fire.interval = 0;
        g.targets[0].fire.arc = 200;
        g.advance(STEP, Input::default());
        assert_eq!(g.enemy_shots.len(), 1);
        let shot = &g.enemy_shots[0];
        assert!((5. / 8. ..=9. / 8.).contains(&shot.height));
        assert_eq!(shot.life, 300);
        assert_eq!(shot.deadline, 301);
        assert_eq!(shot.bouncing_record.as_ref().unwrap()[0x14], 29);
        let initial_height = shot.height;
        let initial_position = shot.position_fixed;
        let direction = shot.direction_fixed;
        // Isolate one trajectory from subsequent launches and player impacts.
        g.targets[0].fire.weapon = -1;
        g.position_fixed[0] += 250 << 16;
        g.tank.x += 250.;
        g.advance(STEP, Input::default());
        assert_eq!(g.enemy_shots[0].height, initial_height + 15. / 8.);
        assert_eq!(
            g.enemy_shots[0].position_fixed,
            std::array::from_fn(|i| initial_position[i] + direction[i] / 8)
        );
        let mut peak = 0_f32;
        let mut landed = false;
        let mut rebounded = false;
        for _ in 0..100 {
            g.advance(STEP, Input::default());
            let height = g.enemy_shots[0].height;
            peak = peak.max(height);
            if landed && height > 0. {
                rebounded = true;
            }
            if height == 0. {
                landed = true;
            }
        }
        assert!((25. ..35.).contains(&peak));
        assert!(landed && rebounded);
        while g.tank.ticks < 301 {
            g.advance(STEP, Input::default());
        }
        assert!(g.enemy_shots.is_empty());
        assert_eq!(g.encounter.enemy_fired, 1);
    }

    #[test]
    fn resource_weapon_controls_no_weapon_and_scattershot_requests() {
        let mut g = duel(130);
        let mut parameters = crate::recovered::default_enemy_parameters(7);
        parameters.weapon = -1;
        parameters.firing_interval = 0;
        parameters.firing_arc = 200;
        g.targets[0].fire = EnemyFireState::from_parameters(&parameters);
        g.capture_targets();
        g.advance(STEP, Input::default());
        assert_eq!(g.targets[0].fire.timer, 1);
        assert_eq!(g.encounter.enemy_fired, 0);
        assert!(g.enemy_shots.is_empty());
        assert!(!g.audio_events.contains(&10001));
        g.targets[0].fire.weapon = 5;
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 3);
        assert_eq!(g.enemy_shots.len(), 3);
        assert_eq!(g.audio_events.iter().filter(|&&id| id == 10038).count(), 1);
        assert!(
            !g.audio_events.contains(&10001),
            "spread subshots have no sound request"
        );
        let angles: Vec<i32> = g.enemy_shots.iter().map(|s| s.heading_fixed).collect();
        assert!(
            (7 << 16..=14 << 16).contains(&crate::recovered::wrap_angle(angles[1] - angles[0]))
        );
        assert!(
            (7 << 16..=14 << 16).contains(&crate::recovered::wrap_angle(angles[0] - angles[2]))
        );
        assert!(g.enemy_shots.iter().all(|s| s.enemy_owner == Some(0)));
        // Only the center request fits. Rejected spread requests consume no Random sample.
        let mut parked = g.enemy_shots[0].clone();
        parked.position_fixed = [100 << 16; 2];
        parked.direction_fixed = [0; 2];
        parked.deadline = 1000;
        g.enemy_shots = vec![parked; 19];
        let seed = g.spawn_random;
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 4);
        assert_eq!(g.enemy_shots.len(), 20);
        assert_eq!(g.spawn_random, seed);
        g.reset();
        assert_eq!(g.targets[0].fire.weapon, -1);
        assert!(g.enemy_shots.is_empty());
    }

    #[test]
    fn enemy_wall_retirement_releases_capacity_before_deletion() {
        for weapon in [0, 3, 4, 5, 6] {
            let mut g = duel(130);
            g.targets[0].fire.interval = 0;
            g.targets[0].fire.arc = 200;
            g.advance(STEP, Input::default());
            let mut parked = g.enemy_shots[0].clone();
            parked.position_fixed = [300 << 16; 2];
            parked.direction_fixed = [0; 2];
            parked.deadline = 1000;
            g.enemy_shots = vec![parked.clone(); 19];
            parked.weapon = weapon;
            parked.position_fixed = [100 << 16; 2];
            parked.direction_fixed = [0, 6 << 16];
            if weapon == 6 {
                let mut record = [0; 128];
                record[0x14] = 29;
                record[..4].copy_from_slice(&(100_i32 << 16).to_be_bytes());
                record[4..8].copy_from_slice(&(100_i32 << 16).to_be_bytes());
                record[0x24..0x28].copy_from_slice(&(4_i32 << 16).to_be_bytes());
                record[0x28..0x2c].copy_from_slice(&(4_i32 << 16).to_be_bytes());
                parked.bouncing_record = Some(record);
            }
            g.enemy_shots.push(parked);
            let mut wall = crate::scene::Catalog::default()
                .instantiate(&World {
                    obstacles: vec![Obstacle {
                        kind: 0,
                        h: 100,
                        v: 100,
                        facing: 0,
                    }],
                })
                .remove(0);
            wall.shape = None;
            wall.min = [-20., 0., -20.];
            wall.max = [20., 20., 20.];
            g.items.push(wall);
            let health = g.encounter.health;
            g.items[0].kind = 39;
            g.advance(STEP, Input::default());
            assert!(g.enemy_shots.iter().all(|s| !s.retiring));
            assert_eq!(
                g.enemy_shots[19].position_fixed,
                [
                    100 << 16,
                    (100 << 16) + if weapon == 6 { 49152 } else { 6 << 16 }
                ]
            );
            assert!(
                g.impacts.is_empty(),
                "Flow Field contact preserves enemy rounds"
            );
            g.enemy_shots[19].position_fixed = [100 << 16; 2];
            if let Some(record) = &mut g.enemy_shots[19].bouncing_record {
                record[..4].copy_from_slice(&(100_i32 << 16).to_be_bytes());
                record[4..8].copy_from_slice(&(100_i32 << 16).to_be_bytes());
            }
            g.items[0].kind = 0;
            g.advance(STEP, Input::default());
            assert_eq!(
                g.encounter.enemy_fired, 1,
                "full owner's new shot is rejected"
            );
            assert!(g.enemy_shots[19].retiring);
            assert_eq!(g.enemy_shots[19].position_fixed, [100 << 16; 2]);
            assert_eq!(g.enemy_shots[19].lifecycle_record.unwrap()[0x1a], 0);
            if weapon == 6 {
                let record = g.enemy_shots[19].lifecycle_record.unwrap();
                assert_eq!(record[0x15], 29);
                assert_eq!(
                    i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()),
                    425984
                );
                assert_eq!(
                    i32::from_be_bytes(record[0x24..0x28].try_into().unwrap()),
                    3 << 16
                );
            }
            assert!(g.impacts.is_empty());
            g.advance(STEP, Input::default());
            assert_eq!(
                g.encounter.enemy_fired, 2,
                "retirement frees capacity before deletion"
            );
            assert_eq!(g.enemy_shots.len(), 21);
            g.targets[0].fire.interval = i16::MAX;
            for _ in 0..2 {
                g.advance(STEP, Input::default());
                assert_eq!(g.enemy_shots.iter().filter(|s| s.retiring).count(), 1);
            }
            g.advance(STEP, Input::default());
            assert!(g.enemy_shots.iter().all(|s| !s.retiring));
            assert_eq!(g.encounter.health, health);
            assert!(g.impacts.is_empty());
        }
    }

    #[test]
    fn enemy_shot_limits_are_per_owner_and_expiry_releases_only_its_owner() {
        let mut g = duel(130);
        g.targets[0].fire.interval = 0;
        g.targets[0].fire.arc = 200;
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 1);
        let mut parked = g.enemy_shots[0].clone();
        assert_eq!(parked.enemy_owner, Some(0));
        parked.position_fixed = [100 << 16; 2];
        parked.direction_fixed = [0; 2];
        parked.deadline = 1000;
        g.enemy_shots = vec![parked; 20];
        g.enemy_shots[0].deadline = 2;
        let mut other = g.targets[0].clone();
        other.item.x += 100.;
        g.targets.push(other);
        g.audio_events.clear();
        g.advance(STEP, Input::default());
        assert_eq!(
            g.targets[0].fire.timer, 2,
            "rejected construction still follows an accepted firing command"
        );
        assert_eq!(
            g.encounter.enemy_fired, 2,
            "only the second enemy can launch"
        );
        assert_eq!(
            g.enemy_shots
                .iter()
                .filter(|s| s.enemy_owner == Some(0))
                .count(),
            19
        );
        assert_eq!(
            g.enemy_shots
                .iter()
                .filter(|s| s.enemy_owner == Some(1))
                .count(),
            1
        );
        assert_eq!(g.audio_events.iter().filter(|&&id| id == 10001).count(), 1);
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 4);
        assert_eq!(
            g.enemy_shots
                .iter()
                .filter(|s| s.enemy_owner == Some(0))
                .count(),
            20
        );
        assert_eq!(
            g.enemy_shots
                .iter()
                .filter(|s| s.enemy_owner == Some(1))
                .count(),
            2
        );
        // An impact releases a slot just as expiry does, after admission.
        g.enemy_shots[0].position_fixed = g.position_fixed;
        g.damage_grace_start = 0;
        let hull = g.encounter.health;
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 5);
        assert_eq!(g.encounter.health, hull - 1);
        assert_eq!(
            g.enemy_shots
                .iter()
                .filter(|s| s.enemy_owner == Some(0))
                .count(),
            19
        );
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.enemy_fired, 7);
        assert_eq!(
            g.enemy_shots
                .iter()
                .filter(|s| s.enemy_owner == Some(0))
                .count(),
            20
        );
        g.reset();
        assert!(g.enemy_shots.is_empty());
    }

    #[test]
    fn firing_uses_cached_angle_only_in_original_combat_mode() {
        for kind in [7, 8] {
            for (mode, cached_angle, fired) in [(1, 90, 0), (1, 0, 1), (3, 90, 1)] {
                let mut g = duel(70);
                let target = &mut g.targets[0];
                target.item.kind = kind;
                target.motion.radial_velocity = 0;
                target.motion.angular_rate = 0;
                target.navigation.intent.behavior = 0;
                target.navigation.intent.movement = mode;
                target.navigation.last_check = 0;
                target.navigation.aim.timestamp = 0;
                target.navigation.aim.angle = cached_angle;
                target.fire.interval = 0;
                target.fire.arc = 0;
                g.step_encounter();
                assert_eq!(
                    g.encounter.enemy_fired, fired,
                    "kind {kind}, movement {mode}, cached angle {cached_angle}"
                );
                assert_eq!(g.targets[0].navigation.intent.movement, mode);
            }
        }
    }

    #[test]
    fn type8_leads_a_moving_player_and_pursues_without_object_selection() {
        let mut g = duel(200);
        g.targets[0].item.kind = 8;
        g.targets[0].fire.weapon = -1;
        g.targets[0].navigation.intent.behavior = 3;
        g.targets[0].navigation.attack_range = 0;
        g.targets[0].navigation.aim.timestamp = -100;
        g.speed_byte = 5;
        g.heading_fixed = 0;
        let player = [
            (g.tank.x * 65536.) as i32,
            (g.tank.z * 65536.) as i32,
            g.heading_fixed,
        ];
        let random = g.spawn_random;
        let start = [g.targets[0].item.x, g.targets[0].item.z];
        g.step_encounter();
        let enemy = &g.targets[0];
        assert_eq!(enemy.navigation.intent.target, 1);
        assert_eq!(enemy.navigation.intent.behavior, 0);
        assert_eq!(
            g.spawn_random, random,
            "initialization skips object-selection Random calls"
        );
        assert!(
            enemy.navigation.aim.pose[0] > player[0],
            "aim leads the eastbound player"
        );
        assert_eq!(enemy.navigation.aim.pose[2], player[2]);
        for _ in 0..120 {
            g.tank.ticks += 1;
            g.step_encounter();
        }
        assert_ne!(
            [g.targets[0].item.x, g.targets[0].item.z],
            start,
            "type 8 pursues after turning toward the target"
        );
        let mut ordinary = duel(200);
        ordinary.speed_byte = 5;
        ordinary.heading_fixed = 0;
        ordinary.targets[0].navigation.aim.timestamp = -100;
        ordinary.step_encounter();
        assert_eq!(
            ordinary.targets[0].navigation.aim.pose, player,
            "kind 7 retains current-pose aiming"
        );
    }

    #[test]
    fn attack_tracking_reinitializes_at_the_original_difficulty_range() {
        for behavior in [4, 9] {
            let mut near = duel(200);
            near.targets[0].navigation.intent.behavior = behavior;
            near.targets[0].navigation.attack_range = 0;
            near.targets[0].navigation.effective_difficulty = 0;
            near.targets[0].motion.radial_velocity = 0;
            let mut far = duel(200);
            far.targets[0].navigation = near.targets[0].navigation.clone();
            far.targets[0].motion.radial_velocity = 0;
            far.targets[0].navigation.effective_difficulty = 127;
            near.advance(STEP, Input::default());
            far.advance(STEP, Input::default());
            assert_eq!(near.targets[0].navigation.intent.behavior, behavior);
            assert_eq!(far.targets[0].navigation.intent.behavior, 3);
            assert_eq!(
                near.targets[0].navigation.tracked_pose,
                near.targets[0].navigation.aim.pose
            );
            assert_eq!(
                far.targets[0].navigation.tracked_pose,
                far.targets[0].navigation.aim.pose
            );
            far.advance(STEP, Input::default());
            assert_ne!(
                far.targets[0].navigation.intent.behavior, 3,
                "the next update runs normal initialization"
            );
        }
    }

    #[test]
    fn enemies_hold_fire_below_ground_then_resume_after_emergence() {
        let mut g = duel(70);
        g.targets[0].motion.radial_velocity = 0;
        g.targets[0].fire.interval = 0;
        g.vertical.renderer_height = -96;
        g.vertical.camera_scale = -12 * 65536;
        g.vertical.vertical = -128 * 65536;
        for _ in 0..30 {
            g.advance(STEP, Input::default());
        }
        assert!(g.vertical.vertical < 0);
        assert_eq!(g.encounter.enemy_fired, 0);
        assert!(g.enemy_shots.is_empty());
        assert_ne!(
            g.targets[0].fire.timer, -1,
            "submerged target still initializes the original firing clock"
        );
        g.vertical = Default::default();
        g.advance(STEP, Input::default());
        assert_eq!(
            g.encounter.enemy_fired, 1,
            "emergence restores eligibility without resetting the enemy"
        );

        let mut airborne = duel(70);
        airborne.targets[0].motion.radial_velocity = 0;
        airborne.targets[0].fire.interval = 0;
        airborne.flight_energy = 400;
        airborne.vertical.hover = 100;
        airborne.advance(STEP, Input::default());
        assert!(airborne.vertical.vertical > 0);
        assert_eq!(
            airborne.encounter.enemy_fired, 1,
            "positive height does not trigger the submergence exclusion"
        );
    }

    #[test]
    fn recovered_short_range_and_resource_arc_control_live_fire() {
        let mut close = duel(70);
        for _ in 0..20 {
            close.advance(STEP, Input::default());
        }
        assert_eq!(close.encounter.enemy_fired, 0);
        close.advance(STEP, Input::default());
        assert_eq!(close.encounter.enemy_fired, 1);

        let mut narrow = duel(130);
        narrow.targets[0].fire.interval = 0;
        narrow.targets[0].fire.arc = 0;
        narrow.targets[0].item.heading = std::f32::consts::FRAC_PI_2;
        let mut wide = duel(130);
        wide.targets[0].fire.interval = 0;
        wide.targets[0].fire.arc = 200;
        wide.targets[0].item.heading = std::f32::consts::FRAC_PI_2;
        narrow.advance(STEP, Input::default());
        wide.advance(STEP, Input::default());
        assert_eq!(narrow.encounter.enemy_fired, 0);
        assert_eq!(wide.encounter.enemy_fired, 1);
    }

    #[test]
    fn enemies_pursue_fire_damage_and_end_round_then_reset() {
        let mut g = duel(250);
        let start = g.targets[0].item.z;
        for _ in 0..180 {
            g.advance(STEP, Input::default());
        }
        assert!(g.targets[0].item.z < start - 50.);
        for _ in 0..1500 {
            g.advance(STEP, Input::default());
        }
        assert!(g.encounter.enemy_fired >= 6);
        assert_eq!(g.encounter.health, 0);
        assert_eq!(g.encounter.status, "lost");
        let tick = g.tank.ticks;
        g.advance(
            0.1,
            Input {
                fire: true,
                ..Input::default()
            },
        );
        assert_eq!(g.tank.ticks, tick);
        g.reset();
        assert_eq!(g.encounter.health, 6);
        assert_eq!(g.encounter.status, "playing");
        assert!(g.enemy_shots.is_empty());
        assert_eq!(g.targets[0].item.z, start);
    }
    #[test]
    fn normal_shot_wins_and_empty_ammo_does_not_end_round() {
        let mut g = duel(130);
        for _ in 0..60 {
            g.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            );
        }
        assert_eq!(g.encounter.status, "won");
        assert_eq!(g.destroyed, 1);
        g.reset();
        g.ammo = 0;
        g.primary_ammo_cost = 1;
        g.advance(STEP, Input::default());
        assert_eq!(g.encounter.status, "playing");
        let ticks = g.tank.ticks;
        g.advance(
            STEP,
            Input {
                throttle: -1.,
                fire: true,
                ..Input::default()
            },
        );
        assert_eq!(g.tank.ticks, ticks + 1);
        assert!(g.tank.speed < 0.);
        assert_eq!(g.fired, 0);
    }
    #[test]
    fn cover_blocks_enemy_projectiles_and_movement() {
        let mut g = duel(250);
        g.items = crate::scene::Catalog::default().instantiate(&World {
            obstacles: vec![Obstacle {
                h: g.tank.x as i16,
                v: g.tank.z as i16 + 70,
                kind: 0,
                facing: 0,
            }],
        });
        for _ in 0..900 {
            g.advance(STEP, Input::default());
        }
        assert_eq!(g.encounter.health, 6);
        assert!(g.encounter.enemy_fired > 0);
        assert!(!crate::scene::footprints_overlap(
            &g.targets[0].item.footprint(true),
            &g.items[0].footprint(false)
        ));
    }
    #[test]
    fn encounter_is_independent_of_host_frame_rate_and_research_stays_stationary() {
        let mut a = duel(250);
        let mut b = duel(250);
        for _ in 0..240 {
            a.advance(1. / 30., Input::default());
        }
        for _ in 0..1152 {
            b.advance(1. / 144., Input::default());
        }
        assert_eq!(
            serde_json::to_value(&a.targets).unwrap(),
            serde_json::to_value(&b.targets).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&a.encounter).unwrap(),
            serde_json::to_value(&b.encounter).unwrap()
        );
        a.set_encounter(false);
        let z = a.targets[0].item.z;
        for _ in 0..300 {
            a.advance(STEP, Input::default());
        }
        assert_eq!(a.targets[0].item.z, z);
        assert!(a.enemy_shots.is_empty());
    }
}
