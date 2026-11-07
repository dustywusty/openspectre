//! Behavior transcribed from the supplied Spectre VR/CD 1.1 executable.
//! See docs/disassembly.md for offsets, evidence, and input contracts.
//! The preview uses recovered movement and selected weapon rules.

pub const FULL_TURN: i32 = 360 << 16;
pub const HALF_TURN: i32 = 180 << 16;

/// CODE 31 +0x1862..+0x1874: signed elapsed time must exceed ten ticks.
pub fn primary_fire_due(now: i32, last_attempt: i32, pressed: bool) -> bool {
    pressed && now.wrapping_sub(last_attempt) > 10
}

/// CODE 30 +0x08be..+0x08ce: record the attempt before testing ammunition.
pub fn primary_attempt_timestamp(now: i32, last_attempt: i32, disabled: u8) -> i32 {
    if disabled == 0 { now } else { last_attempt }
}

/// Candidate transcription of CODE 12's table generator, then CODE 11 scaling.
/// Uses the stored extended step rounded to f64, single-precision accumulation,
/// single-precision sine results, truncating integer conversion, and scale 1/4.
/// Matches the modeled-SANE research harness, NOT an original-SANE oracle.
/// Do not describe this table as bit-exact original runtime data yet.
pub fn candidate_movement_table() -> [i32; 360] {
    candidate_unit_table().map(|value| value / 4)
}

/// Unscaled candidate base table, shared by original unit and projectile tables.
/// Same modeled-SANE limitation as `candidate_movement_table`.
pub fn candidate_unit_table() -> [i32; 360] {
    let mut angle = 0_f32;
    std::array::from_fn(|_| {
        let sine = (angle as f64).sin() as f32;
        let value = (sine * 65536.) as i32;
        angle = (angle as f64 + 0.017_453_277_777_777_78) as f32;
        value
    })
}

/// CODE 6 +0x0550..+0x061a: independently generated collision cosine/sine.
/// The stored extended step rounds to 0.017453 in f64. Unlike movement,
/// this loop multiplies each degree directly and has no f32 intermediates.
/// Matches modeled SANE, not the original OS transcendental implementation.
pub fn candidate_collision_table() -> [[i16; 2]; 360] {
    std::array::from_fn(|degree| {
        let angle = degree as f64 * 0.017_453;
        [angle.cos(), angle.sin()].map(|v| ((v * 65536.) as i32 / 4) as i16)
    })
}

/// CODE 30 +0x0794..+0x07e8, ordinary player-owned weapon-0 launch block.
/// Direction is supplied from the unit table. Collision and allocation are separate.
pub fn launch_basic_shot(
    position: [i32; 2],
    direction: [i32; 2],
    now: i32,
    lifetime: i16,
) -> ([i32; 2], i32) {
    (
        std::array::from_fn(|i| position[i].wrapping_add(direction[i].wrapping_mul(6))),
        now.wrapping_add(lifetime as i32),
    )
}

/// CODE 30 +0x2c10..+0x2c18: expiry precedes projectile movement.
/// This compares signed timestamps directly, not their wrapping difference.
pub fn basic_shot_expired(now: i32, deadline: i32) -> bool {
    deadline <= now
}

/// CODE 30 +0x271a..+0x273a: low signed word of elapsed time, clamped to 0..63.
pub fn projectile_elapsed(now: i32, last_update: i32) -> i32 {
    (now.wrapping_sub(last_update) as i16).clamp(0, 63) as i32
}

/// CODE 30 +0x24fc..+0x252a after direction lookup in the six-unit table.
pub fn move_basic_shot(position: [i32; 2], direction: [i32; 2], elapsed: i32) -> [i32; 2] {
    std::array::from_fn(|i| position[i].wrapping_add(direction[i].wrapping_mul(elapsed)))
}

/// CODE 25 +0x2668..+0x269a. SysEnvirons machine type controls a flag
/// consumed by CODE 23 +0x05b8. Failed environment queries also halve rate.
pub fn half_turn_rate_for_machine(machine_type: i16, environment_error: i16) -> bool {
    environment_error != 0 || matches!(machine_type, 1 | 2 | 3 | 8 | 15)
}

/// CODE 31 +0x1764..+0x181c, after mouse input has updated the 13-key table.
/// Only words equal to 1 count as pressed. Conflicting commands have priority:
/// positive throttle wins, and negative turn wins. Other control branches are
/// outside this block (stop, disabled objects, mouse sampling, firing).
pub fn keyboard_commands(keys: &[i16; 13]) -> (i16, i16) {
    let pressed = |indices: [usize; 3]| indices.into_iter().any(|i| keys[i] == 1);
    let throttle = if pressed([11, 12, 10]) {
        1
    } else if pressed([6, 7, 5]) {
        -1
    } else {
        0
    };
    let turn = if pressed([9, 12, 7]) {
        -1
    } else if pressed([8, 5, 10]) {
        1
    } else {
        0
    };
    (throttle, turn)
}

/// CODE 22 +0x0132: selected vehicle settings. Bytes 0..3 are speed,
/// shields, ammo, and Coast, corroborated by the original settings dialogs.
/// Bytes 5 and 6 are retained from saved settings; their meaning is pending.
pub fn select_vehicle_settings(selector: i16, mut current: [u8; 7], saved: [u8; 7]) -> [u8; 7] {
    current[3] = 0;
    current[4] = 0;
    current[5] = saved[5];
    current[6] = saved[6];
    match selector {
        3 => current[..4].copy_from_slice(&[5, 6, 4, 0]),
        4 => current[..4].copy_from_slice(&[8, 4, 3, 0]),
        5 => current[..4].copy_from_slice(&[3, 8, 4, 0]),
        6 => current[..4].copy_from_slice(&saved[..4]),
        _ => {}
    }
    current
}

/// CODE 7 +0x0004, A5 jump entry +0x0522.
/// Signed 16.16 degrees wrapped into [0, 360).
pub fn wrap_angle(angle: i32) -> i32 {
    angle.rem_euclid(FULL_TURN)
}

/// CODE 7 +0x0036, A5 jump entry +0x052a.
/// The meaningful angular-distance domain is two already-normalized angles.
/// Wrapping operations preserve the original instructions even outside that domain.
pub fn angular_distance(a: i32, b: i32) -> i32 {
    let difference = a.wrapping_sub(b);
    let magnitude = if difference < 0 {
        difference.wrapping_neg()
    } else {
        difference
    };
    if magnitude > HALF_TURN {
        FULL_TURN.wrapping_sub(magnitude)
    } else {
        magnitude
    }
}

/// CODE 18 +0x0dcc, A5 jump entry +0x0bca.
/// Uses the signed high word; fractional degrees do not affect table selection.
/// This recovers the index, not the contents or interpretation of the two tables.
pub fn direction_table_index(angle: i32) -> usize {
    (angle >> 16).rem_euclid(360) as usize
}

/// CODE 30 +0x10b8. Updates heading and the signed byte at object offset 0x23.
/// Object type 6 has a turn ramp capped at 8; other types add the full delta.
/// Type 6's identity and the time/command units remain unconfirmed.
pub fn turn_object(
    heading: i32,
    ramp: i8,
    kind: i8,
    command: i16,
    elapsed: i32,
    delta: i32,
) -> (i32, i8) {
    let mut ramp = ramp;
    let change = if kind == 6 {
        if elapsed > 0 {
            ramp = if command == 0 {
                0
            } else {
                ramp.wrapping_add((elapsed as i16).min(4) as i8)
            };
        }
        ramp = ramp.min(8);
        let change = delta.wrapping_mul(ramp as i32) / 8;
        if elapsed < 0 {
            ramp = if command == 0 {
                0
            } else {
                ramp.wrapping_add((elapsed as i16).max(-4) as i8)
            };
        }
        change
    } else {
        delta
    };
    (wrap_angle(heading.wrapping_add(change)), ramp)
}

/// CODE 31 +0x0b2e..+0x0b40: normal damage path after special-case guards.
/// Subtract a signed word, then clamp negative results to zero.
pub fn subtract_health(health: i16, damage: i16) -> i16 {
    health.wrapping_sub(damage).max(0)
}

/// CODE 25 +0x1902: both damage banks start at one, both ammo banks at zero.
/// Resource entries overwrite both banks. Original indexing uses the signed low byte.
pub fn weapon_tables(entries: &[crate::formats::Weapon]) -> ([i16; 18], [i16; 18]) {
    let mut damage = [1; 18];
    let mut ammo = [0; 18];
    for entry in entries {
        let index = entry.kind as i8;
        if (0..18).contains(&index) {
            damage[index as usize] = entry.damage;
            ammo[index as usize] = entry.ammo_cost;
        }
    }
    (damage, ammo)
}

/// Complete predicate in CODE 28 +0x0004. The subtraction's SGE condition
/// compares signed operands; it is not a test of the wrapped result's sign.
/// Preserve the original byte result for noncanonical inhibit flags.
pub fn primary_fire_eligible(ammo: i16, cost: i16, health: i16, inhibit: u8) -> u8 {
    if ammo >= cost && health > 0 {
        inhibit ^ 1
    } else {
        0
    }
}

/// CODE 6 +0x197e: signed-byte globals at A5-25cf, -25ff, -262f.
/// Type-31 current model slot, first slot, and loaded slot count.
pub fn damage_phase_ready(state: [i8; 3]) -> bool {
    state[0] as i16 == state[1] as i16 + state[2] as i16 - 1
}

/// CODE 19 +0x4b6c: shared type-31 model transition. CODE 19 +0x602a
/// passes interval 6. A timer of -1 is idle; 0 starts the transition.
/// The caller that triggers the transition remains outside this routine.
pub fn advance_type31_model(
    current: i8,
    first: i8,
    count: i8,
    timer: i32,
    now: i32,
    interval: i16,
) -> (i8, i32) {
    if timer == -1 {
        (current, timer)
    } else if timer == 0 {
        (first.wrapping_add(1), now)
    } else if timer.wrapping_add(interval as i32) < now {
        let next = current.wrapping_add(1);
        let timer = if damage_phase_ready([next, first, count]) {
            -1
        } else {
            now
        };
        (next, timer)
    } else {
        (current, timer)
    }
}

/// CODE 31 +0x0b0c..+0x0b40, including the original CODE 6 predicate.
/// `kind` is participant record +8, not arena object record +0x14.
/// Earlier eligibility checks and later death effects are outside this block.
pub fn participant_health_after_guard(health: i16, damage: i16, kind: i8, phase: [i8; 3]) -> i16 {
    if kind == 31 && !damage_phase_ready(phase) {
        health.max(0)
    } else {
        subtract_health(health, damage)
    }
}

/// Inputs to CODE 30 +0x11d6. Field names describe observed effects;
/// their connection to player controls and elapsed-time units remains pending.
#[derive(Clone, Copy)]
pub struct SpeedUpdate {
    pub speed: i8,
    pub command: i16,
    pub elapsed: i32,
    pub limit: i8,
    pub coast: i8,
    pub adjustment: i8,
    pub mode: i8,
    /// Supplied result of the original Random trap, if the branch requests one.
    pub random: i16,
}

pub struct SpeedResult {
    pub speed: i8,
    pub reset_timestamps: bool,
    pub used_random: bool,
}

/// CODE 18 +0x0d24. Unlike direction_table_index, this requires a normalized
/// heading. The caller supplies the original initialized table values.
pub fn movement_direction(heading: i32, table: &[i32; 360]) -> Option<[i32; 2]> {
    if !(0..FULL_TURN).contains(&heading) {
        return None;
    }
    let index = (heading >> 16) as usize;
    let complement = if index <= 90 { 90 - index } else { 450 - index };
    // First coordinate receives the second output of the direction helper.
    Some([table[complement], table[index]])
}

/// CODE 23 +0x05b8: angular rate from enemy parameters or participant settings.
pub fn participant_angular_rate(
    enemy: u8,
    half_rate: u8,
    enemy_rate: i32,
    player_rate: i32,
) -> i32 {
    let rate = if enemy != 0 { enemy_rate } else { player_rate };
    if half_rate != 0 { rate / 2 } else { rate }
}

/// CODE 30 +0x1370, type-7 translation path, before collision handling.
pub fn move_type7_planar(
    position: [i32; 2],
    heading: i32,
    throttle: i16,
    radial_velocity: i16,
    elapsed: i32,
    table: &[i32; 360],
) -> Option<[i32; 2]> {
    let direction = movement_direction(heading, table)?;
    Some(std::array::from_fn(|i| {
        position[i].wrapping_add(
            direction[i]
                .wrapping_mul(throttle as i32)
                .wrapping_mul(radial_velocity as i32)
                .wrapping_mul(elapsed),
        )
    }))
}

/// CODE 30 +0x2578..+0x25e8, type 7: positive elapsed turns before moving;
/// nonpositive elapsed moves before turning. Collision response is separate.
pub fn move_type7(
    pose: [i32; 3],
    commands: [i16; 2],
    radial_velocity: i16,
    angular_rate: i32,
    elapsed: i32,
    table: &[i32; 360],
) -> Option<[i32; 3]> {
    let delta = angular_rate
        .wrapping_mul(commands[0] as i32)
        .wrapping_mul(elapsed);
    let heading = wrap_angle(pose[2].wrapping_add(delta));
    let moved = move_type7_planar(
        [pose[0], pose[1]],
        if elapsed > 0 { heading } else { pose[2] },
        commands[1],
        radial_velocity,
        elapsed,
        table,
    )?;
    Some([moved[0], moved[1], heading])
}

/// Planar type-6 path through CODE 30 +0x1370, excluding local-view and
/// mode-specific extra motion. No collision response occurs in this path.
/// Positive elapsed updates speed before translation; negative elapsed after.
pub fn move_type6_planar(
    position: [i32; 2],
    direction: [i32; 2],
    input: SpeedUpdate,
) -> ([i32; 2], SpeedResult) {
    let elapsed = input.elapsed;
    let result = if elapsed == 0 {
        SpeedResult {
            speed: input.speed,
            reset_timestamps: false,
            used_random: false,
        }
    } else {
        update_speed(input)
    };
    let speed = if elapsed > 0 {
        result.speed
    } else {
        input.speed
    } as i32;
    let position = std::array::from_fn(|i| {
        position[i].wrapping_add(direction[i].wrapping_mul(speed).wrapping_mul(elapsed))
    });
    (position, result)
}

/// Recover the speed byte, timestamp-write condition, and RNG-call condition.
/// The original also clears the adjustment global on every return path.
pub fn update_speed(input: SpeedUpdate) -> SpeedResult {
    let SpeedUpdate {
        speed,
        command,
        elapsed,
        limit,
        coast,
        adjustment,
        mode,
        random,
    } = input;
    let mut speed = (speed as i32).wrapping_add((command as i32).wrapping_mul(elapsed)) as i8;
    let reset_timestamps = adjustment == 0 && mode == 0;
    if adjustment > 0 {
        let amount = (adjustment as i32).wrapping_mul(elapsed);
        speed = if speed > 0 {
            ((speed as i32).wrapping_sub(amount) as i8).max(1)
        } else {
            ((speed as i32).wrapping_add(amount) as i8).min(-1)
        };
    }
    if speed > limit {
        speed = limit;
    } else if (speed as i16) < -(limit as i16) {
        speed = (-(limit as i16)) as i8;
    }
    let coast = if adjustment < 0 {
        adjustment.wrapping_neg()
    } else {
        coast
    };
    let used_random = (1..6).contains(&coast) && command == 0;
    if coast == 0 && command == 0 {
        speed = 0;
    } else if used_random && random % (2 * coast as i16) == 0 {
        if speed > 0 {
            speed = (speed as i32).wrapping_sub(elapsed) as i8;
        } else if speed < 0 {
            speed = (speed as i32).wrapping_add(elapsed) as i8;
        }
    }
    SpeedResult {
        speed,
        reset_timestamps,
        used_random,
    }
}

/// CODE 7 +0x00a0: separating-axis test with original integer truncation.
/// Poses are x/z/heading; dimensions preserve stack order +8,+c,+10,+14.
/// Table pairs are A5-0x38b2 and A5-0x3312, truncated to signed words.
pub fn collision_axes_separate(
    first: [i32; 3],
    second: [i32; 3],
    dimensions: [i32; 4],
    table: &[[i16; 2]; 360],
) -> bool {
    let delta: [i32; 2] = std::array::from_fn(|i| {
        ((second[i].wrapping_sub(first[i]).wrapping_shl(2) >> 16) as i16) as i32
    });
    let [c, s] = table[direction_table_index(first[2])].map(i32::from);
    let local = [
        delta[0]
            .wrapping_mul(c)
            .wrapping_add(delta[1].wrapping_mul(s)),
        delta[0]
            .wrapping_mul(s)
            .wrapping_neg()
            .wrapping_add(delta[1].wrapping_mul(c)),
    ];
    let mut relative = second[2].wrapping_sub(first[2]);
    if relative < 0 {
        relative = relative.wrapping_add(FULL_TURN);
    }
    let [c, s] = table[direction_table_index(relative)].map(i32::from);
    let rounded =
        |v: i32| (((v.wrapping_mul(2).wrapping_sub(1) >> 16) as i16).wrapping_add(1)) as i32;
    let d4 = rounded(dimensions[0]);
    let d5 = rounded(dimensions[1]);
    let projected = [
        c.wrapping_mul(d5)
            .wrapping_abs()
            .wrapping_add(s.wrapping_mul(d4).wrapping_abs()),
        s.wrapping_mul(d5)
            .wrapping_abs()
            .wrapping_add(c.wrapping_mul(d4).wrapping_abs()),
    ];
    local[0].wrapping_abs() > ((dimensions[3] as u32 >> 1) as i32).wrapping_add(projected[0])
        || local[1].wrapping_abs() > ((dimensions[2] as u32 >> 1) as i32).wrapping_add(projected[1])
}

/// CODE 7 +0x0250: overlap requires no separating axis from either rectangle.
pub fn collision_rectangles_overlap(
    first: [i32; 3],
    second: [i32; 3],
    dimensions: [i32; 4],
    table: &[[i16; 2]; 360],
) -> bool {
    !collision_axes_separate(first, second, dimensions, table)
        && !collision_axes_separate(
            second,
            first,
            [dimensions[2], dimensions[3], dimensions[0], dimensions[1]],
            table,
        )
}

/// CODE 11 +0x3cba: signed, wrapping fixed-point Manhattan prefilter.
pub fn contact_near(first: [i32; 2], second: [i32; 2]) -> bool {
    first[0]
        .wrapping_sub(second[0])
        .wrapping_abs()
        .wrapping_add(first[1].wrapping_sub(second[1]).wrapping_abs())
        <= 0x6c0000
}

/// CODE 7 +0x0f3a..+0x104e, after ownership/type exclusions and overlap.
/// Slots are ordinary contact (first retained) and projectile contact (last retained).
pub fn select_contact(
    contacts: &mut [Option<i16>; 2],
    moving_kind: u8,
    candidate_kind: u8,
    candidate_id: i16,
    same_owner: bool,
) {
    if matches!(candidate_kind, 9 | 28 | 29 | 32 | 33 | 34 | 43 | 44 | 46) {
        if !matches!(candidate_kind, 29 | 33)
            && (candidate_kind == 28 || !matches!(moving_kind, 23..=25))
            && !same_owner
        {
            contacts[1] = Some(candidate_id);
        }
    } else if !matches!(candidate_kind, 10 | 14)
        && !(candidate_kind == 6 && matches!(moving_kind, 13 | 21 | 23 | 24 | 35 | 45))
        && contacts[0].is_none()
    {
        contacts[0] = Some(candidate_id);
    }
}

/// CODE 7 +0x02ce: object-pair overlap with already resolved model sizes/radii.
/// Does not include participant exclusions or spatial-grid traversal.
pub fn object_pair_overlap(
    first: &[u8; 128],
    second: &[u8; 128],
    dimensions: [i32; 4],
    radii: [i16; 2],
    table: &[[i16; 2]; 360],
) -> bool {
    let long = |r: &[u8; 128], o| i32::from_be_bytes(r[o..o + 4].try_into().unwrap());
    let dx = (long(first, 0).wrapping_sub(long(second, 0)) >> 16) as i16;
    let dz = (long(first, 4).wrapping_sub(long(second, 4)) >> 16) as i16;
    if dx.wrapping_abs().wrapping_add(dz.wrapping_abs()) >= 108 {
        return false;
    }
    let [a, b] = [first[0x14], second[0x14]];
    let height = long(first, 0x28).wrapping_sub(long(second, 0x28));
    let mut allowed =
        matches!(a, 5 | 36) || matches!(b, 5 | 36) || height.wrapping_abs() <= 0x3c0000;
    if (a == 12 && height.wrapping_neg() < 0x8c0000) || (a != 12 && b == 12 && height < 0x8c0000) {
        allowed = true;
    }
    if !allowed {
        return false;
    }
    let radius = radii[0].wrapping_add(radii[1]);
    let distance_squared = dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz));
    if radius.wrapping_mul(radius) < distance_squared {
        return false;
    }
    collision_rectangles_overlap(
        [long(first, 0), long(first, 4), long(first, 8)],
        [long(second, 0), long(second, 4), long(second, 8)],
        [dimensions[2], dimensions[3], dimensions[0], dimensions[1]],
        table,
    )
}

/// CODE 12 +0x0944: planar dimensions from the packed runtime vertex list.
/// Runtime coordinates are signed words in eighth-units. This does not compute radius.
pub fn runtime_collision_dimensions(vertices: &[[i16; 3]]) -> [i32; 2] {
    let mut dimensions = [0; 2];
    for vertex in vertices {
        dimensions[0] = dimensions[0].max((vertex[0] as i32).abs() * 16384);
        dimensions[1] = dimensions[1].max((vertex[2] as i32).abs() * 16384);
    }
    dimensions
}

/// CODE 12 +0x00c4. The caller supplies Macintosh FixDiv semantics.
pub fn runtime_sqrt(value: i32, divide: &mut impl FnMut(i32, i32) -> i32) -> i32 {
    if value <= 0 {
        return 0;
    }
    let mut reduced = value as u32;
    let mut estimate = 65536_u32;
    while reduced > 0x20000 {
        reduced >>= 2;
        estimate = estimate.wrapping_mul(2);
    }
    while reduced < 0x8000 {
        reduced <<= 2;
        estimate >>= 1;
    }
    for _ in 0..3 {
        estimate = (divide(value, estimate as i32) as u32).wrapping_add(estimate) >> 1;
    }
    estimate as i32
}

/// CODE 12 +0x0944 radius accumulation and CODE 19 +0x3f64 word conversion.
/// FixDiv is supplied so the original OS operation can replace the model.
pub fn runtime_collision_radius(
    vertices: &[[i16; 3]],
    divide: &mut impl FnMut(i32, i32) -> i32,
) -> i16 {
    let mut radius = 0_i32;
    for v in vertices {
        let x = v[0] as i32 * 8192;
        let z = v[2] as i32 * 8192;
        let square = |n: i32| ((n as i64 * n as i64) / 65536) as i32;
        radius = radius.max(runtime_sqrt(square(x).wrapping_add(square(z)), divide));
    }
    ((radius.wrapping_sub(1) >> 16) as i16).wrapping_add(1)
}

/// Candidate FixDiv model: signed truncation with saturation. Not an executed
/// original Macintosh trap; radius results remain conditional on this model.
pub fn candidate_fix_div(numerator: i32, denominator: i32) -> i32 {
    if denominator == 0 {
        return if numerator < 0 { i32::MIN } else { i32::MAX };
    }
    ((numerator as i64 * 65536) / denominator as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// CODE 12 +0x0532 with the software fixed multiply and modeled FixRound.
/// Trap rounding follows Apple documentation; original OS trap is not executed.
pub fn candidate_runtime_vertex(vertex: [i32; 3], scale: i32) -> [i16; 3] {
    vertex.map(|v| {
        let scaled = ((v as i64 * scale as i64) / 65536) as i32;
        let shifted = scaled.wrapping_shl(3);
        (shifted.wrapping_add(32768) >> 16) as i16
    })
}

/// CODE 7 +0x08b4..+0x0914: broad-phase filter before rectangle overlap.
/// IDs are candidate/excluded; centers and extents are signed integer words.
pub fn projectile_candidate(
    ids: [i16; 2],
    position: [i32; 2],
    center: [i16; 2],
    extents: [i16; 2],
    kind: i8,
) -> bool {
    ids[0] != ids[1]
        && (0..2).all(|i| {
            let delta = ((position[i] >> 16) as i16)
                .wrapping_sub(center[i])
                .wrapping_abs();
            extents[i] >= delta
        })
        && !matches!(kind, 5 | 9 | 10 | 12 | 28 | 29 | 30 | 33 | 41)
}

/// CODE 7 +0x09ae..+0x0ab2: final rejection after selecting a collision.
/// IDs and kinds are selected/excluded. Owner states are participant byte +4,
/// present only when the object's signed owner byte is positive.
/// The meaning of state 3 remains untraced; this preserves its numeric test.
pub fn finalize_projectile_hit(
    ids: [i16; 2],
    kinds: [i8; 2],
    owner_states: [Option<u8>; 2],
) -> i16 {
    let [mut hit, excluded] = ids;
    if hit > 0
        && excluded > 0
        && ((kinds[0] == 6 && matches!(kinds[1], 21 | 23 | 24 | 35 | 45))
            || matches!(kinds[1], 35 | 45))
    {
        hit = 0;
    }
    if (hit > 0 && owner_states[0] == Some(3)) || (excluded > 0 && owner_states[1] == Some(3)) {
        hit = 0;
    }
    hit
}

/// CODE 7 +0x0618: replace the current hit only for a strictly smaller projection.
pub fn prefer_projectile_hit(current: [i32; 2], candidate: [i32; 2], direction: [i16; 2]) -> bool {
    let project = |p: [i32; 2]| {
        ((p[0] >> 16) as i16 as i32 * direction[0] as i32)
            .wrapping_add((p[1] >> 16) as i16 as i32 * direction[1] as i32)
    };
    project(current) > project(candidate)
}

/// CODE 7 +0x06ba..+0x0828: query extents and inclusive grid bounds.
/// Dimensions preserve stack order +0xa (width), +0xe (length).
/// Bounds are min-x/min-z/max-x/max-z. Empty ranges must remain empty.
pub fn projectile_grid_bounds(
    pose: [i32; 3],
    dimensions: [i32; 2],
    cells: i16,
    table: &[[i16; 2]; 360],
) -> ([i16; 2], [i16; 4]) {
    let [width, length] = dimensions.map(|v| ((v.wrapping_sub(1) >> 16) as i16).wrapping_add(1));
    let sum = width.wrapping_add(length);
    let extents = if sum <= 35 {
        [sum.wrapping_add(36); 2]
    } else {
        let [c, s] = table[direction_table_index(pose[2])].map(i32::from);
        let projection = |a: i32, b: i32| {
            let n = (length as i32 * a)
                .wrapping_abs()
                .wrapping_add((width as i32 * b).wrapping_abs());
            ((n.wrapping_shl(2) >> 16) as i16).wrapping_add(36)
        };
        [projection(c, s), projection(s, c)]
    };
    let center = [(pose[0] >> 16) as i16, (pose[1] >> 16) as i16];
    let lower: [i16; 2] =
        std::array::from_fn(|i| (center[i].wrapping_sub(extents[i]) as u16 >> 6) as i16);
    let upper: [i16; 2] = std::array::from_fn(|i| {
        ((center[i].wrapping_add(extents[i]) as u16 >> 6) as i16).min(cells.wrapping_sub(1))
    });
    (extents, [lower[0], lower[1], upper[0], upper[1]])
}

/// CODE 7 +0x0828..+0x09ae traversal, for a valid 16-by-16 grid.
/// Preserve stored order and duplicate IDs. Malformed nonempty ranges are rejected.
pub fn projectile_grid_candidates(bounds: [i16; 4], grid: &[Vec<i16>; 256]) -> Option<Vec<i16>> {
    let [min_x, min_z, max_x, max_z] = bounds;
    if min_x > max_x || min_z > max_z {
        return Some(Vec::new());
    }
    if min_x < 0 || min_z < 0 || max_x >= 16 || max_z >= 16 {
        return None;
    }
    let mut candidates = Vec::new();
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            let cell = &grid[x as usize * 16 + z as usize];
            if cell.len() > 20 {
                return None;
            }
            candidates.extend_from_slice(cell);
        }
    }
    Some(candidates)
}

/// Model loader bookkeeping at A5-0x25be, -0x25ee, -0x261e, and -0x264e.
/// Builders supply their actual loaded-slot counts; geometry loading is separate.
#[derive(Clone)]
pub struct ModelSlotState {
    pub next: i16,
    pub current: [i8; 47],
    pub first: [i8; 47],
    pub counts: [i8; 47],
    pub type19_requests: i16,
    pub builder_flag: i16,
}

/// Services requested by the original outer model loader.
pub enum ModelLoadRequest {
    Selected(usize),
    Resource { kind: usize, id: i16, slot: i16 },
    Halve { slot: i8 },
}

impl Default for ModelSlotState {
    fn default() -> Self {
        Self {
            next: 1,
            current: [-1; 47],
            first: [-1; 47],
            counts: [0; 47],
            type19_requests: 0,
            builder_flag: 0,
        }
    }
}

impl ModelSlotState {
    /// CODE 1 +0x1206..+0x1338. Resource Manager data and the display predicate
    /// (A5-0x7be5) are inputs; the callback executes the selected inner loader and returns
    /// its slot count. Entry offsets are relative to CODE 1.
    pub fn load_selected(
        &mut self,
        kind: usize,
        choices: Option<&[[i16; 3]]>,
        display_flag: bool,
        alternate: bool,
        loader: &mut impl FnMut(u16, i16, i16, usize) -> Option<i16>,
    ) -> Option<()> {
        let mut count = 0;
        if self.begin(kind)? {
            let column = if !display_flag {
                2
            } else if alternate {
                1
            } else {
                0
            };
            let selector = match choices {
                Some(rows) => rows.get(kind)?[column],
                None => 0,
            };
            let entry = match selector {
                10000..=10999 => Some(0x165e),
                1..=999 => Some(0x133a),
                1000..=1999 => Some(0x1434),
                2000..=2999 => Some(0x150c),
                3000..=3999 => Some(0x15ba),
                _ => None,
            };
            if let Some(entry) = entry {
                count = loader(entry, selector, self.next, kind)?;
            }
        }
        self.finish(kind, count)
    }

    /// Complete CODE 19 +0x5a80 dispatch with explicit geometry-loader services.
    /// Selected loads include reservation/completion. Raw resource loads return
    /// their slot count; Halve updates both dimensions and the signed-word radius.
    pub fn load_type(
        &mut self,
        kind: usize,
        service: &mut impl FnMut(&mut Self, ModelLoadRequest) -> Option<i16>,
    ) -> Option<()> {
        if kind >= 47 {
            return None;
        }
        if kind == 19 {
            self.type19_requests = self.type19_requests.wrapping_add(1);
        }
        if self.current[kind] != -1 {
            return Some(());
        }
        self.builder_flag = 1;
        match kind {
            14 | 23..=26 => self.load_alias_type(kind, &mut |state, kind| {
                service(state, ModelLoadRequest::Selected(kind)).map(|_| ())
            })?,
            16 => {
                service(self, ModelLoadRequest::Selected(16))?;
                service(
                    self,
                    ModelLoadRequest::Halve {
                        slot: self.current[16],
                    },
                )?;
            }
            20 => {
                if self.current[34] == -1 {
                    service(self, ModelLoadRequest::Selected(34))?;
                }
                service(self, ModelLoadRequest::Selected(20))?;
            }
            21 => {
                service(self, ModelLoadRequest::Selected(21))?;
                service(self, ModelLoadRequest::Selected(38))?;
                for id in [10099, 10100] {
                    let count = service(
                        self,
                        ModelLoadRequest::Resource {
                            kind: 14,
                            id,
                            slot: self.next,
                        },
                    )?;
                    self.next = self.next.wrapping_add(count);
                }
            }
            _ => {
                service(self, ModelLoadRequest::Selected(kind))?;
            }
        }
        Some(())
    }

    /// CODE 19 +0x5a80 alias branches. The callback is CODE 1's selected-model
    /// loader, including reservation and completion, not just a geometry builder.
    /// Other type branches return None without modifying the state.
    pub fn load_alias_type(
        &mut self,
        kind: usize,
        load_selected: &mut impl FnMut(&mut Self, usize) -> Option<()>,
    ) -> Option<()> {
        if !matches!(kind, 14 | 23 | 24 | 25 | 26) {
            return None;
        }
        if self.current[kind] != -1 {
            return Some(());
        }
        if kind == 14 {
            self.current[14] = 1;
            self.first[14] = 1;
            return Some(());
        }
        let (dependency, source) = match kind {
            23 => (Some(43), 16),
            24 => (Some(44), 18),
            25 => (None, 3),
            26 => (None, 7),
            _ => unreachable!(),
        };
        if let Some(dependency) = dependency
            && self.current[dependency] == -1
        {
            load_selected(self, dependency)?;
        }
        if self.current[source] == -1 {
            if kind == 26 {
                if self.begin(7)? {
                    self.next = self.next.wrapping_add(1);
                    // CODE 19 +0x1ce4 receives zero, then calls the selected loader.
                    load_selected(self, 7)?;
                }
            } else {
                load_selected(self, source)?;
            }
        }
        if self.current[source] != -1 {
            self.current[kind] = self.current[source];
            self.first[kind] = self.first[source];
        }
        Some(())
    }

    /// CODE 19 +0x2358. Reservation precedes resource lookup and builder success.
    /// At capacity, the original assigns slot 1 and leaves the first slot unchanged.
    pub fn begin(&mut self, kind: usize) -> Option<bool> {
        let current = self.current.get_mut(kind)?;
        if *current != -1 {
            return Some(false);
        }
        if self.next > 100 {
            *current = 1;
            return Some(false);
        }
        *current = self.next as i8;
        self.first[kind] = self.next as i8;
        Some(true)
    }

    /// CODE 1 +0x1322 and CODE 19 +0x23c6, including zero-count attempts.
    pub fn finish(&mut self, kind: usize, loaded: i16) -> Option<()> {
        *self.counts.get_mut(kind)? = loaded as i8;
        self.next = self.next.wrapping_add(loaded);
        Some(())
    }
}

/// CODE 1 +0x15ba..+0x165c. Animation frames use base slot plus frame index,
/// regardless of how many slots an earlier inner builder reports.
/// `previous_count` is incoming D5, preserved for out-of-range frame IDs.
/// The selected-model caller supplies zero. Missing resources return zero.
pub fn load_animation_models(
    frames: Option<&[i16]>,
    base_slot: i16,
    kind: u8,
    mut previous_count: i16,
    loader: &mut impl FnMut(u16, i16, i16, u8) -> Option<i16>,
) -> Option<i16> {
    let Some(frames) = frames else {
        return Some(0);
    };
    if frames.len() > i16::MAX as usize {
        return None;
    }
    let mut total = 0_i16;
    for (index, &id) in frames.iter().enumerate() {
        let entry = match id {
            10000..=10999 => Some(0x165e),
            1..=999 => Some(0x133a),
            _ => None,
        };
        if let Some(entry) = entry {
            previous_count = loader(entry, id, base_slot.wrapping_add(index as i16), kind)?;
        }
        total = total.wrapping_add(previous_count);
    }
    Some(total)
}

/// CODE 7 +0b76..+0d72 for cannon/Seeker kind 9 and Smart Missile kind 34.
/// Other projectile types have additional effects and require their own paths.
pub fn direct_projectile_contact(shot: &[u8; 128], target: &[u8; 128]) -> Option<bool> {
    if !matches!(shot[0x14], 9 | 34) {
        return None;
    }
    let height = |record: &[u8; 128]| i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap());
    Some(
        height(shot).wrapping_sub(height(target)).wrapping_abs() <= 60 << 16
            && !matches!(target[0x14], 29 | 42)
            && !(target[0x14] == 32 && target[0x1a] == shot[0x1a]),
    )
}

/// Original collision-query inputs. Model dimensions retain table order
/// A5-0x2572, A5-0x23e2; slots and object IDs are one-based.
pub struct CollisionQueryWorld<'a> {
    pub records: &'a [[u8; 128]],
    pub participant_states: &'a [u8; 12],
    pub grid: &'a [Vec<i16>; 256],
    pub model_slots: &'a [i8; 48],
    pub dimensions: &'a [[i32; 2]],
    pub directions: &'a [[i16; 2]; 360],
}

impl CollisionQueryWorld<'_> {
    /// CODE 20 +0x05b0 composed with the original CODE 7 collision selector.
    /// Width/distance are in collision-query table order. Invalid data returns None.
    pub fn ai_clearance(
        &self,
        moving_id: i16,
        heading: i32,
        dimensions: [i32; 2],
        boundary: i16,
    ) -> Option<bool> {
        let moving = self.record(moving_id)?;
        let position = std::array::from_fn(|i| {
            i32::from_be_bytes(moving[i * 4..i * 4 + 4].try_into().unwrap())
        });
        let direction = self.directions[direction_table_index(heading)].map(i32::from);
        let mut valid = true;
        let clear = ai_clearance_probe(
            position,
            heading,
            dimensions[1],
            boundary,
            direction,
            |pose| match self.projectile_hit(pose, dimensions, moving_id) {
                Some(0) => None,
                Some(id) => match self.record(id) {
                    Some(record) => Some(record[0x14]),
                    None => {
                        valid = false;
                        None
                    }
                },
                None => {
                    valid = false;
                    None
                }
            },
        );
        valid.then_some(clear)
    }

    /// CODE 7 +0x0d90 for participant-backed moving objects on a 16x16 grid.
    /// The participant's cached cell (+0x60/+0x62) must be supplied, not recomputed.
    pub fn participant_contacts(
        &self,
        participant: i8,
        moving_id: i16,
        cached_cell: [i16; 2],
        radii: &[i16],
        boundary: i16,
    ) -> Option<[Option<i16>; 2]> {
        let owner = usize::try_from(participant.checked_sub(1)?).ok()?;
        if *self.participant_states.get(owner)? == 3 {
            return Some([None, None]);
        }
        let moving = self.record(moving_id)?;
        if !matches!(moving[0x14], 6 | 7 | 8 | 13 | 20..=27 | 31 | 35 | 45) {
            return None;
        }
        let long = |r: &[u8; 128], o| i32::from_be_bytes(r[o..o + 4].try_into().unwrap());
        let pose = [long(moving, 0), long(moving, 4)];
        let mut contacts = [None, None];
        if pose.iter().any(|p| {
            let v = (p >> 16) as i16;
            v <= 0 || v >= boundary
        }) {
            contacts[0] = Some(-999);
        }
        let mut bounds = [0; 4];
        for axis in 0..2 {
            let cell = cached_cell[axis];
            let start = (cell as i32).wrapping_mul(0x400000);
            bounds[axis] = if pose[axis].wrapping_sub(start) < 0x480000 {
                cell.wrapping_sub(1)
            } else {
                cell
            }
            .max(0);
            bounds[axis + 2] =
                if start.wrapping_add(0x400000).wrapping_sub(pose[axis]) < 0x480000 {
                    cell.wrapping_add(1)
                } else {
                    cell
                }
                .min(15);
        }
        for id in projectile_grid_candidates(bounds, self.grid)? {
            let target = self.record(id)?;
            if target[0x1a] == participant as u8 || matches!(target[0x14], 14 | 42) {
                continue;
            }
            if !contact_near(pose, [long(target, 0), long(target, 4)]) {
                continue;
            }
            let slot = |r| usize::try_from(self.model_slot(r)?.checked_sub(1)?).ok();
            let a = slot(target)?;
            let b = slot(moving)?;
            let da = self.dimensions.get(a)?;
            let db = self.dimensions.get(b)?;
            if object_pair_overlap(
                target,
                moving,
                [da[0], da[1], db[0], db[1]],
                [*radii.get(a)?, *radii.get(b)?],
                self.directions,
            ) {
                select_contact(&mut contacts, moving[0x14], target[0x14], id, false);
            }
        }
        Some(contacts)
    }

    /// CODE 7 +0x0ad6, restricted to the ordinary type-9 projectile path.
    /// Cannon/Seeker kind 9 and Smart Missile kind 34 share this query path.
    /// Returns 0 for no selected hit and -1 for a discarded hit.
    /// Participant object IDs are the original words at participant +0x56.
    pub fn basic_projectile_hit(
        &self,
        id: i16,
        next_pose: [i32; 3],
        elapsed: i32,
        participant_objects: &[i16; 12],
    ) -> Option<i16> {
        let shot = self.record(id)?;
        if !matches!(shot[0x14], 9 | 34) {
            return None;
        }
        let long = |record: &[u8; 128], offset| {
            i32::from_be_bytes(record[offset..offset + 4].try_into().unwrap())
        };
        let midpoint = std::array::from_fn(|i| {
            if i == 2 {
                long(shot, 8)
            } else {
                ((long(shot, i * 4).wrapping_add(next_pose[i]) as u32) >> 1) as i32
            }
        });
        let owner = shot[0x1a] as i8;
        let excluded = if owner == 0 {
            id
        } else {
            *participant_objects.get(usize::try_from(owner as i16 - 1).ok()?)?
        };
        let dimensions = [0x18000, elapsed.wrapping_mul(0x60000).wrapping_add(0x30000)];
        let hit = self.projectile_hit(midpoint, dimensions, excluded)?;
        if hit == id {
            return Some(-1);
        }
        if hit <= 0 {
            return Some(hit);
        }
        let target = self.record(hit)?;
        Some(if direct_projectile_contact(shot, target)? {
            hit
        } else {
            -1
        })
    }

    fn record(&self, id: i16) -> Option<&[u8; 128]> {
        self.records.get(usize::try_from(id.checked_sub(1)?).ok()?)
    }

    /// CODE 6 +0x18ea, including type-42 linked-object animation selection.
    pub fn model_slot(&self, record: &[u8; 128]) -> Option<i16> {
        let slot = |kind: u8| self.model_slots.get(kind as usize).copied().map(i16::from);
        if record[0x14] != 42 {
            return slot(record[0x14]);
        }
        let linked = i16::from_be_bytes([record[0x44], record[0x45]]);
        if linked <= 0 {
            return Some(-1);
        }
        let kind = self.record(linked)?[0x14];
        let base = slot(kind)?;
        Some(if kind == 10 {
            base
        } else {
            base + record[0x3e] as i8 as i16
        })
    }

    fn owner_state(&self, record: &[u8; 128]) -> Option<Option<u8>> {
        let owner = record[0x1a] as i8;
        if owner <= 0 {
            Some(None)
        } else {
            self.participant_states
                .get(owner as usize - 1)
                .copied()
                .map(Some)
        }
    }

    /// Complete CODE 7 +0x06ba query for a 16-by-16 grid.
    /// Invalid IDs, model slots, or owner indices return None as host guards.
    pub fn projectile_hit(
        &self,
        pose: [i32; 3],
        dimensions: [i32; 2],
        excluded: i16,
    ) -> Option<i16> {
        let read_pose = |record: &[u8; 128]| {
            std::array::from_fn(|i| {
                i32::from_be_bytes(record[i * 4..i * 4 + 4].try_into().unwrap())
            })
        };
        let (extents, bounds) = projectile_grid_bounds(pose, dimensions, 16, self.directions);
        let center = [(pose[0] >> 16) as i16, (pose[1] >> 16) as i16];
        let mut hit = 0;
        for id in projectile_grid_candidates(bounds, self.grid)? {
            let record = self.record(id)?;
            let slot = self.model_slot(record)?;
            let candidate = read_pose(record);
            if !projectile_candidate(
                [id, excluded],
                [candidate[0], candidate[1]],
                center,
                extents,
                record[0x14] as i8,
            ) {
                continue;
            }
            let size = self
                .dimensions
                .get(usize::try_from(slot.checked_sub(1)?).ok()?)?;
            if collision_rectangles_overlap(
                pose,
                candidate,
                [size[0], size[1], dimensions[0], dimensions[1]],
                self.directions,
            ) && (hit == 0
                || prefer_projectile_hit(
                    read_pose(self.record(hit)?)[..2].try_into().unwrap(),
                    [candidate[0], candidate[1]],
                    self.directions[direction_table_index(pose[2])],
                ))
            {
                hit = id;
            }
        }
        let mut kinds = [0; 2];
        let mut states = [None; 2];
        for (i, id) in [hit, excluded].into_iter().enumerate() {
            if id > 0 {
                let record = self.record(id)?;
                kinds[i] = record[0x14] as i8;
                states[i] = self.owner_state(record)?;
            }
        }
        Some(finalize_projectile_hit([hit, excluded], kinds, states))
    }
}

/// CODE 11 +0x3eee: object cell, using signed high words and clamping to 0..15.
pub fn object_grid_cell(position: [i32; 2]) -> [usize; 2] {
    position.map(|v| (((v >> 16) as i16 >> 6).clamp(0, 15)) as usize)
}

/// CODE 30 +0x0f60 grid mutation, for valid cells with destination capacity.
/// Participant bookkeeping and object-pose writes are handled separately.
pub fn move_grid_object(
    grid: &mut [Vec<i16>; 256],
    id: i16,
    old: [i32; 2],
    new: [i32; 2],
) -> Option<bool> {
    let index = |p| {
        let [x, z] = object_grid_cell(p);
        x * 16 + z
    };
    let from = index(old);
    let to = index(new);
    if from == to {
        return Some(false);
    }
    if grid[from].len() > 20 || grid[to].len() >= 20 {
        return None;
    }
    remove_grid_entry(&mut grid[from], id);
    grid[to].push(id);
    Some(true)
}

/// CODE 30 +0x088c..+0x08a4: projectile insertion, capped at 20 cell entries.
pub fn insert_projectile_grid_entry(cell: &mut Vec<i16>, id: i16) -> bool {
    if cell.len() >= 20 {
        return false;
    }
    cell.push(id);
    true
}

/// CODE 25 +0x0ea4..+0x0eda: remove the first matching ID by swapping in the last.
/// This is grid unlinking only; object-array compaction is a separate step.
pub fn remove_grid_entry(cell: &mut Vec<i16>, id: i16) -> bool {
    if let Some(index) = cell.iter().position(|&entry| entry == id) {
        cell.swap_remove(index);
        true
    } else {
        false
    }
}

/// Projectile-only entry point; preserves the narrower contract for existing callers.
pub fn delete_projectile_record(
    records: &mut [[u8; 128]],
    count: &mut usize,
    grid: &mut [Vec<i16>; 256],
    participants: &mut [[u8; 256]; 12],
    id: usize,
) -> Option<()> {
    if id == 0
        || id > *count
        || id > records.len()
        || !matches!(
            records[id - 1][0x14],
            9 | 28 | 29 | 32 | 33 | 34 | 43 | 44 | 46
        )
    {
        return None;
    }
    delete_object_record(records, count, grid, participants, &mut [0; 12], id)
}

/// CODE 25 +0x0e62 with CODE 25 +0x0e2e and CODE 14 +0x2e20 deactivation.
/// Valid participant-backed records require an owner in 1..=12.
pub fn delete_object_record(
    records: &mut [[u8; 128]],
    count: &mut usize,
    grid: &mut [Vec<i16>; 256],
    participants: &mut [[u8; 256]; 12],
    participant_status: &mut [u8; 12],
    id: usize,
) -> Option<()> {
    if id == 0 || id > *count || *count > records.len() || *count > i16::MAX as usize {
        return None;
    }
    let mut fields = GeometryParticipants {
        ids: std::array::from_fn(|i| {
            i16::from_be_bytes([participants[i][0x56], participants[i][0x57]])
        }),
        shots: std::array::from_fn(|i| {
            i16::from_be_bytes([participants[i][0x58], participants[i][0x59]])
        }),
        active: std::array::from_fn(|i| participants[i][0x88]),
        status: *participant_status,
    };
    let mut remaining = *count as i16;
    remove_geometry(records, &mut remaining, id as i16, &mut fields, grid)?;
    for (i, participant) in participants.iter_mut().enumerate() {
        participant[0x56..0x58].copy_from_slice(&fields.ids[i].to_be_bytes());
        participant[0x58..0x5a].copy_from_slice(&fields.shots[i].to_be_bytes());
        participant[0x88] = fields.active[i];
    }
    *participant_status = fields.status;
    *count = remaining as usize;
    Some(())
}

/// Planar table edits in CODE 19. These describe loader operations, not blanket kind rules.
#[derive(Clone, Copy)]
pub enum CollisionDimensionOverride {
    TenUnits,
    Half,
    Double,
    Copy([i32; 2]),
}

pub fn override_collision_dimensions(
    dimensions: [i32; 2],
    rule: CollisionDimensionOverride,
) -> [i32; 2] {
    match rule {
        CollisionDimensionOverride::TenUnits => [10 << 16; 2],
        CollisionDimensionOverride::Half => dimensions.map(|n| n / 2),
        CollisionDimensionOverride::Double => dimensions.map(|n| n.wrapping_mul(2)),
        CollisionDimensionOverride::Copy(source) => source,
    }
}

/// CODE 1 +0x133a: first CODE 19 builder called for a procedural selector.
/// Selector 21 additionally calls +0x4f5c and +0x5178 after this first builder.
pub fn procedural_model_entry(selector: i16) -> Option<u16> {
    match selector {
        1 => Some(0x2454),
        2 => Some(0x312c),
        3 => Some(0x3328),
        4 => Some(0x3664),
        5 => Some(0x1e0e),
        6 => Some(0x37ca),
        8 => Some(0x0b42),
        9 => Some(0x3a7c),
        10 => Some(0x5e24),
        11 => Some(0x5a34),
        12 => Some(0x0fd4),
        13 => Some(0x53a0),
        14 => Some(0x2646),
        15 => Some(0x28d8),
        16 => Some(0x2cd4),
        17 => Some(0x1532),
        18 => Some(0x3fc2),
        19 => Some(0x48bc),
        20 => Some(0x434c),
        21 => Some(0x4d38),
        _ => None,
    }
}

/// CODE 25 +0x0b60..+0x0bc4: placement coordinates and heading into object state.
/// Positive world modulus; CODE 33 +0x0260 initializes it to 1024.
/// Signed remainders preserve negative inputs. This does not define render rotation.
pub fn world_placement(h: i16, v: i16, facing: i16, modulus: i16) -> Option<[i32; 3]> {
    if modulus <= 0 {
        return None;
    }
    Some([
        (h as i32 % modulus as i32) << 16,
        (v as i32 % modulus as i32) << 16,
        (facing as i32 % 360) << 16,
    ])
}

/// CODE 7 +0x1426..0x1464: Flow Field contact displacement.
/// Direction is the original lookup result for wrap(field_heading - 90 degrees).
/// Multiplication wraps before signed division by two. Contact detection and
/// the caller's last-contact clock are separate from this arithmetic block.
pub fn flow_field_displacement(
    position: [i32; 2],
    direction: [i32; 2],
    now: i32,
    previous: i32,
) -> [i32; 2] {
    let elapsed = now.wrapping_sub(previous);
    std::array::from_fn(|i| position[i].wrapping_add(direction[i].wrapping_mul(elapsed) / 2))
}

/// CODE 28 +0x021a: count type-12 records in the leading flag slots.
/// Negative signed-byte limits scan nothing; short host slices are rejected.
pub fn remaining_flags(kinds: &[u8], limit: i8) -> Option<i16> {
    let count = limit.max(0) as usize;
    Some(
        kinds
            .get(..count)?
            .iter()
            .filter(|&&kind| kind == 12)
            .count() as i16,
    )
}

/// CODE 25 +0x1f6c..+0x1fe8: refresh eligible leading flag slots.
/// Only kind-14 records convert to flags. This is not the complete level loader.
pub fn refresh_flag_slots(level: i16, object_count: i16, kinds: &mut [u8]) -> Option<i8> {
    let budget = (level / 2)
        .wrapping_add(3)
        .min(20)
        .min(object_count.wrapping_sub(6)) as i8;
    let slots = kinds.get_mut(..budget.max(0) as usize)?;
    let mut changed = 0_i8;
    for kind in slots {
        if *kind == 14 {
            *kind = 12;
            changed += 1;
        }
    }
    Some(changed)
}

/// CODE 19 +0x14ac..+0x152a. Flag copies type-2 dimensions and radius only
/// when both signed-byte slots exist. Slot order remains a caller input.
pub fn copy_flag_collision(
    destination: i8,
    source: i8,
    dimensions: &mut [[i32; 2]],
    radii: &mut [i16],
) -> Option<()> {
    if destination == -1 || source == -1 {
        return Some(());
    }
    let dst = usize::try_from(destination as i16 - 1).ok()?;
    let src = usize::try_from(source as i16 - 1).ok()?;
    let dims = *dimensions.get(src)?;
    let radius = *radii.get(src)?;
    *dimensions.get_mut(dst)? = dims;
    *radii.get_mut(dst)? = radius;
    Some(())
}

/// CODE 25 +0x1ab6. Difficulty is the original 0..7 options control.
/// Mode bytes are kept as raw inputs until their startup semantics are traced.
pub fn score_resource_id(difficulty: i16, mode_12d2: u8, mode_12ca: u8) -> i16 {
    wave_resource_id(difficulty, mode_12d2, mode_12ca)
}

/// CODE 31 +0x0004. Missing resources return score+1. A host iteration guard
/// rejects pathological repeating resources; it is not original game behavior.
pub fn next_life_threshold(score: i32, rules: Option<&crate::formats::ScoreRules>) -> Option<i32> {
    let mut next = score.wrapping_add(1);
    let Some(rules) = rules else {
        return Some(next);
    };
    let last = rules.thresholds[9];
    if score > last {
        next = last.wrapping_add(rules.repeat_increment as i32);
        for _ in 0..1_000_000 {
            if score <= next {
                return Some(next);
            }
            next = next.wrapping_add(rules.repeat_increment as i32);
        }
        None
    } else {
        for &threshold in rules.thresholds.iter().rev() {
            if score < threshold {
                next = threshold;
            }
        }
        Some(next)
    }
}

/// CODE 31 +0x009e scoring arithmetic and extra-life gate. Audio and HUD calls
/// are effects of the returned award count, not part of this pure function.
pub fn award_score(
    mut score: i32,
    mut lives: i16,
    delta: i16,
    disable_lives: bool,
    rules: Option<&crate::formats::ScoreRules>,
) -> Option<(i32, i16, u16)> {
    if delta < 0 {
        score = score.wrapping_add(delta as i32);
    }
    let mut remaining = delta;
    let mut awards = 0;
    while remaining > 0 {
        let chunk = remaining.min(4999) as i32;
        let next = next_life_threshold(score, rules)?;
        let old = score;
        score = score.wrapping_add(chunk);
        if !disable_lives && old < next && score >= next {
            lives = lives.wrapping_add(1);
            awards += 1;
        }
        remaining = remaining.wrapping_sub(chunk as i16);
    }
    Some((score, lives, awards))
}
/// Local-player fields from CODE 31 +0x1a80, with movies/placement/UI excluded.
/// Health/ammo refill executes CODE 23 +0x0684/+0x06fc. The mode byte suppresses
/// life consumption when nonzero. Returns (health,lives,ammo,game_over,respawn).
pub fn local_respawn(
    active: bool,
    health: i16,
    lives: i16,
    ammo: i16,
    mode: u8,
    shields: i8,
    ammo_setting: i8,
) -> (i16, i16, i16, bool, bool) {
    if !active || health > 0 {
        return (health, lives, ammo, false, false);
    }
    let lives = if mode == 0 {
        lives.wrapping_sub(1)
    } else {
        lives
    };
    if lives <= 0 {
        (health, lives, ammo, true, false)
    } else {
        (
            (shields as i16).min(9),
            lives,
            ammo_setting as i16 * 10,
            false,
            true,
        )
    }
}

/// CODE 31 +0x0caa..+0x0de2: scoring gate after damage and kill detection.
/// Returns the signed-word award and whether the per-type kill count increments.
/// Participant flag bytes remain raw to preserve the original bitwise gate.
pub fn combat_score_award(
    kind: u8,
    score: i32,
    owners: [i8; 2],
    enemy_flags: [u8; 2],
    killed: u8,
    mode: u8,
) -> Option<(i16, bool)> {
    let [attacker, victim] = owners;
    if attacker == 0
        || attacker == victim
        || ((enemy_flags[0] & enemy_flags[1]) ^ 1) == 0
        || killed == 0
        || !matches!(mode, 0 | 1 | 6 | 13 | 14 | 18)
    {
        return None;
    }
    let counted = matches!(
        kind,
        6 | 7 | 8 | 20 | 21 | 22 | 23 | 24 | 26 | 27 | 31 | 35 | 45
    );
    Some((
        if mode != 0 {
            1
        } else if counted {
            score as i16
        } else {
            0
        },
        counted,
    ))
}

/// CODE 21 +0x0004: authored per-type count product and bonus build the par time.
/// The original truncates each count product to a signed word before weighting.
pub fn arena_par_seconds(
    counts: &[i16; 47],
    multipliers: &[i16; 47],
    bonuses: &[i32; 47],
    flag_count: i8,
) -> i16 {
    let weighted = counts.iter().zip(multipliers).zip(bonuses).fold(
        0_i32,
        |sum, ((&count, &multiplier), &bonus)| {
            sum.wrapping_add((count.wrapping_mul(multiplier) as i32).wrapping_mul(bonus))
        },
    );
    weighted.wrapping_add((flag_count as i16).wrapping_mul(10).wrapping_add(10) as i32) as i16
}
/// CODE 21 +0x0ae6.
pub fn rating_bonus(rating: i16) -> i32 {
    match rating {
        1 => 50,
        2 => 100,
        3 => 200,
        4 => 400,
        5 => 800,
        _ => 0,
    }
}
#[derive(Debug, serde::Serialize)]
pub struct CompletionRatings {
    pub ratings: [i16; 3],
    pub bars: [i32; 3],
    pub unrated: [bool; 2],
    pub bonuses: [i32; 3],
}
/// CODE 21 +0x0b40..+0x0d44, stopping before presentation and score calls.
/// Counters: player launches, player hits, other launches, player damage events.
/// Prior ratings matter when a launch counter is nonpositive.
pub fn completion_ratings(
    timestamps: [i32; 2],
    par: i16,
    counters: [i32; 4],
    previous: [i16; 3],
) -> Option<CompletionRatings> {
    let elapsed = timestamps[1].wrapping_sub(timestamps[0]) / 60;
    let par = par as i32;
    let quick =
        (6_i32.wrapping_sub(elapsed.wrapping_mul(100).checked_div(par)? / 20) as i16).clamp(1, 5);
    let quick_bar = 19_i32
        .wrapping_sub(elapsed.wrapping_shl(4).checked_div(par)?)
        .clamp(1, 16);
    let [fired, hits, other_fired, damage_events] = counters;
    let accuracy = if fired > 0 {
        (hits.wrapping_mul(100).checked_div(fired)? / 20) as i16
    } else {
        previous[1]
    }
    .clamp(1, 5);
    let accuracy_bar = if fired > 0 {
        hits.wrapping_shl(4).checked_div(fired)?
    } else {
        1
    }
    .min(16);
    let avoidance = if other_fired > 0 {
        (100_i32.wrapping_sub(damage_events.wrapping_mul(100).checked_div(other_fired)?) / 20)
            as i16
    } else {
        previous[2]
    }
    .clamp(1, 5);
    let avoidance_bar = if other_fired > 0 {
        16_i32.wrapping_sub(damage_events.wrapping_shl(4).checked_div(other_fired)?)
    } else {
        1
    }
    .max(1);
    Some(CompletionRatings {
        ratings: [quick, accuracy, avoidance],
        bars: [quick_bar, accuracy_bar, avoidance_bar],
        unrated: [fired <= 0, other_fired <= 0],
        bonuses: [
            rating_bonus(quick),
            if fired > 0 { rating_bonus(accuracy) } else { 0 },
            if other_fired > 0 {
                rating_bonus(avoidance)
            } else {
                0
            },
        ],
    })
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CombatCounters {
    pub player_shots: i32,
    pub player_hits: i32,
    pub other_shots: i32,
    pub player_damage_events: i32,
}
#[derive(Clone, Copy, serde::Deserialize)]
pub struct DamageCounterEvent {
    pub local: i8,
    pub attacker: i8,
    pub victim: i8,
    pub geometry_id: i16,
    pub mode: u8,
    pub now: i32,
}
impl CombatCounters {
    /// CODE 30 +0x0292, after successful projectile slot allocation.
    pub fn launch(&mut self, local: i8, owner: i8) {
        let count = if owner == local {
            &mut self.player_shots
        } else {
            &mut self.other_shots
        };
        *count = count.wrapping_add(1);
    }
    /// Admission and counter effects from CODE 31 +0x0a84..+0x0b9a.
    /// Returns false for a missing victim or the initial local damage grace period.
    /// Damage amount and type-31 immunity do not affect the admitted counters.
    pub fn damage(&mut self, event: DamageCounterEvent, grace_start: &mut i32) -> bool {
        if event.geometry_id == -1 {
            return false;
        }
        if event.victim == event.local {
            if event.mode == 0 && event.now.wrapping_sub(*grace_start) < 75 {
                return false;
            }
            self.player_damage_events = self.player_damage_events.wrapping_add(1);
        } else {
            *grace_start = 0;
            if event.attacker == event.local {
                self.player_hits = self.player_hits.wrapping_add(1);
            }
        }
        true
    }
}

/// CODE 15 +0xbcc..+0xc28 initializes this table; CODE 30 +0x836..+0x864
/// submits priority one and a channel-one request for the original weapon ID.
/// Invalid native IDs and original sentinel 255 produce no request.
pub fn weapon_fire_sound(weapon: i8) -> Option<u32> {
    const SOUNDS: [u16; 18] = [
        1, 24, 37, 255, 255, 38, 255, 255, 255, 28, 27, 21, 255, 45, 255, 20, 35, 47,
    ];
    let sound = *SOUNDS.get(usize::try_from(weapon).ok()?)?;
    (sound != 255).then_some(0x10000 | u32::from(sound))
}

/// CODE 15 +0x0b42 -> +0x006c -> +0x0870. Only the signed low word
/// selects the sample; positive indices below 10000 gain the resource base.
pub fn sound_request_id(request: u32) -> Option<u16> {
    let id = request as i16;
    (id > 0).then(|| {
        if id < 10000 {
            (id + 10000) as u16
        } else {
            id as u16
        }
    })
}

/// CODE 31 +0x1c40..+0x1cb2: signed-word award when the last enemy retires.
/// Uses the initial count/life tables, not the remaining per-participant lives.
pub fn last_enemy_bonus(level: i16, counts: &[i16; 47], lives: &[i16; 47]) -> i16 {
    let multiplier = if level < 21 {
        1
    } else if level < 27 {
        2
    } else {
        (level / 9).max(1)
    };
    let total = counts
        .iter()
        .zip(lives)
        .fold(0_i16, |sum, (&count, &life)| {
            sum.wrapping_add(count.wrapping_mul(life))
        });
    total.wrapping_mul(50).wrapping_mul(multiplier)
}

/// CODE 31 +0x0194: one proposed spawn coordinate in 16.16 units.
/// The Random trap result is supplied; collision rejection belongs to +0x04b4.
/// Nonpositive divisors are rejected as a host guard.
pub fn spawn_coordinate(enemy: u8, kind: i8, extent: i16, random: i16) -> Option<i32> {
    let sample = random.wrapping_abs();
    let coordinate = if enemy == 0 {
        let divisor = (extent.wrapping_sub(200) / 2).wrapping_add(1);
        if divisor <= 0 {
            return None;
        }
        (sample % divisor).wrapping_mul(2).wrapping_add(100)
    } else if matches!(kind, 35 | 45) {
        let divisor = extent.wrapping_mul(3) / 4;
        if divisor <= 0 {
            return None;
        }
        (sample % divisor).wrapping_add(extent / 8)
    } else {
        if extent <= 0 {
            return None;
        }
        sample % extent
    };
    Some((coordinate as i32).wrapping_mul(65536))
}

/// CODE 31 +0x0332 and +0x0298, using CODE 25 +0x0154 proximity.
/// Geometry IDs are one-based. Negative IDs never block; zero/out-of-range
/// IDs return None as a host guard instead of reading outside geometry storage.
pub fn spawn_participant_blocked(
    participant: usize,
    local: usize,
    mode: i8,
    flags: &[[u8; 2]; 12],
    ids: &[i16; 12],
    positions: &[[i32; 2]],
) -> Option<bool> {
    if participant >= 12 || local >= 12 {
        return None;
    }
    let proximity = |a: i16, b: i16, limit: i16| -> Option<bool> {
        if a < 0 || b < 0 {
            return Some(false);
        }
        let a = positions.get(usize::try_from(a - 1).ok()?)?;
        let b = positions.get(usize::try_from(b - 1).ok()?)?;
        let distance = a[0]
            .wrapping_sub(b[0])
            .wrapping_abs()
            .wrapping_add(a[1].wrapping_sub(b[1]).wrapping_abs());
        Some(((distance >> 16) as i16) < limit)
    };
    let limit = if matches!(mode, 2 | 4 | 11 | 12) {
        60
    } else if mode != 0 {
        120
    } else {
        250
    };
    if participant == local {
        for other in 0..12 {
            if other != local
                && flags[other][0] != 0
                && ids[other] != ids[local]
                && proximity(ids[other], ids[local], limit)?
            {
                return Some(true);
            }
        }
    } else {
        if ids[participant] != ids[local] && proximity(ids[participant], ids[local], limit)? {
            return Some(true);
        }
        for other in 0..12 {
            if other != participant
                && flags[other][0] & flags[other][1] != 0
                && proximity(ids[participant], ids[other], 25)?
            {
                return Some(true);
            }
        }
    }
    Some(false)
}

/// CODE 31 +0x0608..+0x070e: selected modes halve coordinates and reflect
/// them toward the participant's team anchor. The anchor comparison is fixed
/// at 500 units, even when the arena extent differs from 1000.
pub fn spawn_team_coordinates(
    mut pose: [i32; 3],
    mode: i8,
    team: i8,
    anchor: [i32; 2],
    extent: i16,
) -> [i32; 3] {
    if matches!(mode, 2 | 12 | 15) && matches!(team, 0 | 1) {
        for axis in 0..2 {
            pose[axis] /= 2;
            if anchor[axis] > 500 * 65536 {
                pose[axis] = (extent as i32 * 65536).wrapping_sub(pose[axis]);
            }
        }
    }
    pose
}

/// CODE 31 +0x0594..+0x0604: type-20 placement beside the current flag.
/// The caller supplies the original direction lookup output; this helper
/// does not claim to reproduce the OS-generated trigonometry table.
pub fn spawn_beside_flag(flag: [i32; 2], random: i16, direction: [i32; 2]) -> [i32; 3] {
    [
        flag[0].wrapping_add(direction[0].wrapping_mul(30)),
        flag[1].wrapping_add(direction[1].wrapping_mul(30)),
        (random.wrapping_abs() % 360) as i32 * 65536,
    ]
}

/// Normal-mode, non-kind-20 branch of CODE 31 +0x04b4. The caller supplies
/// Random words and collision services. A blocked final attempt is retained
/// after 501 attempts, as in the original; `clear` reports that distinction.
#[derive(Clone, Debug)]
pub struct SpawnSearch {
    pub pose: [i32; 3],
    pub attempts: i16,
    pub clear: bool,
}
pub fn normal_spawn_search(
    enemy: u8,
    kind: i8,
    extent: i16,
    random: &mut impl Iterator<Item = i16>,
    mut contacts: impl FnMut([i32; 3]) -> [u8; 2],
) -> Option<SpawnSearch> {
    if kind == 20 {
        return None;
    }
    spawn_search(
        &SpawnContext {
            enemy,
            kind,
            extent,
            mode: 0,
            team: -1,
            object_id: -1,
        },
        &mut [],
        &mut 1,
        random,
        |_| [0, 0],
        &mut contacts,
    )
}

#[derive(Clone, Copy, serde::Deserialize)]
pub struct SpawnContext {
    pub enemy: u8,
    pub kind: i8,
    pub extent: i16,
    pub mode: i8,
    pub team: i8,
    pub object_id: i16,
}
#[derive(Clone, Copy, serde::Deserialize)]
pub struct SpawnGeometry {
    pub kind: i8,
    pub pose: [i32; 3],
}
/// Full candidate-selection ordering from CODE 31 +0x04b4. Geometry uses
/// original one-based indices. Missing required entries are host errors.
/// Contact and direction services are supplied by the caller.
pub fn spawn_search(
    context: &SpawnContext,
    geometry: &mut [SpawnGeometry],
    flag_cursor: &mut i16,
    random: &mut impl Iterator<Item = i16>,
    mut direction: impl FnMut(i32) -> [i32; 2],
    mut contacts: impl FnMut([i32; 3]) -> [u8; 2],
) -> Option<SpawnSearch> {
    let mut heading = (random.next()?.wrapping_abs() % 360) as i32 * 65536;
    for attempts in 1..=501 {
        let mut pose = [
            spawn_coordinate(context.enemy, context.kind, context.extent, random.next()?)?,
            spawn_coordinate(context.enemy, context.kind, context.extent, random.next()?)?,
            heading,
        ];
        if context.kind == 20 {
            let flag = geometry.get(usize::try_from(flag_cursor.checked_sub(1)?).ok()?)?;
            if flag.kind == 12 {
                let sample = random.next()?;
                heading = (sample.wrapping_abs() % 360) as i32 * 65536;
                pose = spawn_beside_flag([flag.pose[0], flag.pose[1]], sample, direction(heading));
                *flag_cursor = flag_cursor.wrapping_add(1);
            }
        }
        if matches!(context.mode, 2 | 12 | 15) && matches!(context.team, 0 | 1) {
            let anchor = geometry.get(context.team as usize)?;
            pose = spawn_team_coordinates(
                pose,
                context.mode,
                context.team,
                [anchor.pose[0], anchor.pose[1]],
                context.extent,
            );
        }
        if context.object_id >= 0 {
            let index = usize::try_from(context.object_id.checked_sub(1)?).ok()?;
            geometry.get_mut(index)?.pose = pose;
        }
        let [collision, participant] = contacts(pose);
        let blocked = if collision != 0 {
            collision
        } else {
            participant
        };
        let clear = blocked & 1 == 0;
        if attempts > 500 || clear {
            return Some(SpawnSearch {
                pose,
                attempts,
                clear,
            });
        }
    }
    unreachable!()
}

/// CODE 31 +0x1bd0..+0x1c0c. Returns whether the enemy is retired after
/// consuming a life and attempting the original same-kind transfer.
/// Called only after the lifecycle dispatcher has established a dead participant.
pub fn consume_enemy_life(
    recipient: usize,
    flags: &[[u8; 3]; 12],
    lives: &mut [i16; 12],
    mode: u8,
) -> Option<bool> {
    if flags.get(recipient)?[1] == 0 || mode != 0 {
        return Some(false);
    }
    lives[recipient] = lives[recipient].wrapping_sub(1);
    Some(lives[recipient] <= 0 && borrow_enemy_life(recipient, flags, lives).is_none())
}

/// CODE 31 +0x19da: transfer one life from the first eligible same-kind
/// participant. Indices here are zero-based; the original scans slots 1..12.
/// Flags are [active, enemy, kind] bytes, retaining the original bitwise gate.
pub fn borrow_enemy_life(
    recipient: usize,
    flags: &[[u8; 3]; 12],
    lives: &mut [i16; 12],
) -> Option<usize> {
    let kind = flags.get(recipient)?[2];
    for donor in 0..12 {
        if donor != recipient
            && flags[donor][0] & flags[donor][1] != 0
            && lives[donor] > 1
            && flags[donor][2] == kind
        {
            lives[donor] -= 1;
            lives[recipient] = 1;
            return Some(donor);
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct EnemyPlan {
    pub counts: [i16; 47],
    pub lives: [i16; 47],
    pub total: i16,
}
/// CODE 25 +0x1d7e..+0x1ee2, after resource lookup. Random trap outputs are
/// supplied in valid-row order. The host guards invalid ranges and a cap loop
/// that cannot progress because every occupied type already has one instance.
pub fn enemy_plan(
    rows: &[crate::formats::Spawn],
    difficulty: i16,
    reduced_capacity: bool,
    random: &[i16],
) -> Option<EnemyPlan> {
    let n = i16::try_from(rows.len()).ok()?;
    if n <= 0 {
        return None;
    }
    let extra_lives = (difficulty as i32 / n as i32) / 5;
    let mut result = EnemyPlan {
        counts: [0; 47],
        lives: [0; 47],
        total: 0,
    };
    let mut random = random.iter();
    for row in rows {
        let kind = row.kind as i8;
        if !(0..47).contains(&kind) {
            continue;
        }
        let width = row.maximum.wrapping_sub(row.minimum).wrapping_add(1);
        if width <= 0 {
            return None;
        }
        let sample = random.next()?.wrapping_abs();
        let count = (sample % width).wrapping_add(row.minimum);
        result.counts[kind as usize] = count;
        result.lives[kind as usize] = row.rebirths.wrapping_add(extra_lives as i16);
    }
    result.total = result
        .counts
        .iter()
        .fold(0_i16, |sum, &v| sum.wrapping_add(v));
    let cap = if reduced_capacity { 6 } else { 12 };
    while result.total >= cap {
        let mut changed = false;
        for kind in 0..47 {
            if result.total < cap {
                break;
            }
            if result.counts[kind] > 1 {
                result.counts[kind] -= 1;
                result.lives[kind] = result.lives[kind].wrapping_add(1);
                result.total = result.total.wrapping_sub(1);
                changed = true;
            }
        }
        if !changed {
            return None;
        }
    }
    Some(result)
}

/// CODE 12 +0020: three rounded integer-root iterations with word wrapping.
/// None guards a zero divisor in the original instructions.
pub fn ai_integer_root(value: i32) -> Option<u16> {
    if value <= 0 {
        return Some(0);
    }
    let mut reduced = value as u32;
    let mut estimate = 1_u16;
    while reduced > 2 {
        reduced >>= 2;
        estimate = estimate.wrapping_mul(2);
    }
    for _ in 0..3 {
        let dividend = (value as u32).wrapping_add(u32::from(estimate >> 1));
        let quotient = dividend.checked_div(u32::from(estimate))?;
        // A 68000 DIVU overflow leaves the destination register unchanged.
        let word = if quotient > 65535 {
            dividend as u16
        } else {
            quotient as u16
        };
        estimate = word.wrapping_add(estimate) >> 1;
    }
    Some(estimate)
}

/// Complete CODE 20 +0040 target pose calculation, with an explicit unit table.
/// Source/target poses use original geometry coordinates; speed is byte +22.
/// None guards invalid table indices or divisions that fault in the original.
pub fn ai_predict_pose(
    source: [i32; 3],
    target: [i32; 3],
    speed: i8,
    mode: u8,
    table: &[i32; 360],
) -> Option<[i32; 3]> {
    if mode == 0 {
        return Some(target);
    }
    let angle = usize::try_from(target[2] >> 16).ok()?;
    let cosine_index = if angle <= 90 {
        90 - angle
    } else {
        450_usize.checked_sub(angle)?
    };
    let direction = [*table.get(cosine_index)?, *table.get(angle)?];
    let speed = i32::from(speed);
    let delta = [
        target[0].wrapping_sub(source[0]) >> 16,
        target[1].wrapping_sub(source[1]) >> 16,
    ];
    let velocity = direction.map(|v| speed.wrapping_mul(v).wrapping_shl(4) >> 16);
    let scaled_speed = speed.wrapping_mul(16384);
    let square = ((i64::from(scaled_speed) * i64::from(scaled_speed)) / 65536) as i32;
    let denominator = (0x240000_i32.wrapping_sub(square)).wrapping_shl(4) >> 16;
    let dot = delta[0]
        .wrapping_mul(velocity[0])
        .wrapping_add(delta[1].wrapping_mul(velocity[1]));
    let negative_dot = dot.wrapping_neg();
    let negative_distance = delta[0]
        .wrapping_mul(delta[0])
        .wrapping_add(delta[1].wrapping_mul(delta[1]))
        .wrapping_neg();
    let discriminant = ((negative_dot.wrapping_mul(negative_dot) as u32) >> 4) as i32;
    let root =
        ai_integer_root(discriminant.wrapping_sub(denominator.wrapping_mul(negative_distance)))?;
    let numerator = i32::from(root).wrapping_shl(2).wrapping_add(dot);
    let time = numerator.checked_div(denominator)? as i16;
    let travel = speed.wrapping_mul(i32::from(time)) as i16;
    Some([
        target[0].wrapping_add(i32::from(travel).wrapping_mul(direction[0])),
        target[1].wrapping_add(i32::from(travel).wrapping_mul(direction[1])),
        target[2],
    ])
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiAimCache {
    pub pose: [i32; 3],
    pub angle: i16,
    pub timestamp: i32,
}

/// CODE 20 +0x0256: refresh the shared pose/angle cache after five ticks.
/// Prediction and angle conversion are separate services with explicit inputs.
pub fn refresh_ai_aim_cache(
    now: i32,
    owner_flag: u8,
    kind: u8,
    cache: &mut AiAimCache,
    mut predict: impl FnMut(u8) -> [i32; 3],
    mut aim: impl FnMut([i32; 3]) -> i16,
) -> bool {
    if now.wrapping_sub(cache.timestamp) < 5 {
        return false;
    }
    let mode = if owner_flag == 1 {
        u8::from(kind == 8)
    } else {
        (owner_flag ^ 1) & 1
    };
    cache.pose = predict(mode);
    cache.angle = aim(cache.pose);
    cache.timestamp = now;
    true
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiIntentCheck {
    pub now: i32,
    pub last_check: i32,
    pub distance: i32,
    pub attack_range: i32,
    pub kind: u8,
    pub behavior: i8,
    pub movement: u8,
    pub game_mode: u8,
    pub target: i16,
    pub participant: i8,
}

/// CODE 20 +0x23ac..+0x24d4, with the resource range already resolved.
/// Returns the CODE-20 callee offset and optional attack target byte.
/// The caller must supply a valid enemy parameter-table kind (0..47).
pub fn ai_intent_action(input: AiIntentCheck) -> (Option<u16>, Option<i8>) {
    if input.last_check != -1 && input.last_check.wrapping_add(15) >= input.now {
        return (None, None);
    }
    if input.distance <= input.attack_range && !matches!(input.kind, 21..=25 | 35 | 45) {
        if input.movement == 1 {
            return (None, None);
        }
        if input.game_mode == 0 {
            return (Some(0x0e6c), Some(1));
        }
        if input.target != 0 && input.target != input.participant as i16 {
            return (Some(0x0e6c), Some(input.target as i8));
        }
        return (None, None);
    }
    let entry = match input.behavior {
        0 | 8 => Some(0x1240),
        1 => Some(0x153a),
        4 | 9 => Some(0x0ef4),
        5 => Some(0x0fb8),
        6 => Some(0x1140),
        7 => Some(0x1042),
        12 | 13 => Some(0x14be),
        _ => None,
    };
    (entry, None)
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiIntentState {
    pub behavior: u8,
    pub target: i16,
    pub movement: u8,
    pub aim_target: i16,
    pub heading_deadline: i32,
    pub field_26: i16,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiTargetCandidate {
    pub team: u8,
    pub owner: u8,
    pub active: u8,
    pub health: i16,
    pub kind: u8,
    pub pose: [i32; 3],
}

/// CODE 20 +0xa22: initial opponent acquisition depends on the game mode.
/// Modes 4/12 use scored selection; other nonzero modes choose the nearest
/// active, living participant on a different team, without an angular filter.
pub fn ai_acquire_opponent(
    source: i8,
    game_mode: u8,
    participants: &[AiTargetCandidate; 12],
    angle: impl FnMut([i32; 3], [i32; 3]) -> i16,
) -> Option<i8> {
    if game_mode == 0 {
        return Some(1);
    }
    if matches!(game_mode, 4 | 12) {
        return ai_select_target(source, 0, 0, game_mode, participants, angle);
    }
    if !(1..=12).contains(&source) {
        return None;
    }
    let origin = participants[source as usize - 1];
    let mut selected = 0;
    let mut best = 0;
    for (index, candidate) in participants.iter().enumerate() {
        if candidate.active == 0 || candidate.health <= 0 || candidate.team == origin.team {
            continue;
        }
        let distance = ai_planar_distance(
            [candidate.pose[0], candidate.pose[1]],
            [origin.pose[0], origin.pose[1]],
        );
        if selected == 0 || distance < best {
            selected = index as i8 + 1;
            best = distance;
        }
    }
    Some(selected)
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiObjectAssignment {
    pub active: u8,
    pub owner: u8,
    pub behavior: u8,
    pub target: i16,
}

/// CODE 20 +0x346: health and participant identity do not affect this count.
pub fn ai_object_assignment_count(object: i16, participants: &[AiObjectAssignment; 12]) -> i16 {
    participants
        .iter()
        .filter(|p| p.active & p.owner != 0 && p.behavior == 1 && p.target == object)
        .count() as i16
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiObjectCandidate {
    pub kind: u8,
    pub position: [i32; 2],
    pub assignments: i16,
}

/// CODE 20 +0x3b2: object indices are one-based geometry slots. The two
/// signed globals are kept explicit until their complete lifecycle is traced.
pub fn ai_select_object(
    source: [i32; 2],
    player: [i32; 2],
    field_136e: i16,
    field_1372: i16,
    objects: &[AiObjectCandidate],
    mut random: impl FnMut() -> i16,
) -> Option<i16> {
    if objects.len() > i16::MAX as usize {
        return None;
    }
    let allow_kind5 = field_136e >= 25 && field_1372 >= 6;
    let limit = field_1372 / 6 + 1;
    let mut admitted = 0i16;
    let mut selected = 0;
    for (index, object) in objects.iter().enumerate() {
        if (object.kind != 12 && !(allow_kind5 && object.kind == 5))
            || object.assignments >= limit
            || ai_planar_distance(source, object.position)
                >= ai_planar_distance(player, object.position)
        {
            continue;
        }
        admitted += 1;
        // The original word ABS leaves -32768 negative; DIVS uses its signed remainder.
        if random().wrapping_abs() % admitted == 0 {
            selected = index as i16 + 1;
        }
    }
    Some(selected)
}

/// CODE 20 +0x754: ascending participant scan, with original signed scoring.
/// The angle service receives the candidate pose, then the source pose.
pub fn ai_select_target(
    source: i8,
    excluded: i8,
    mode: u8,
    game_mode: u8,
    participants: &[AiTargetCandidate; 12],
    mut angle: impl FnMut([i32; 3], [i32; 3]) -> i16,
) -> Option<i8> {
    if !(1..=12).contains(&source) {
        return None;
    }
    let origin = participants[source as usize - 1];
    let mut selected = 0;
    let mut best = 0;
    for (index, candidate) in participants.iter().enumerate() {
        let participant = index as i8 + 1;
        let admitted = if game_mode == 0 {
            candidate.active & candidate.owner != 0
        } else {
            candidate.team != origin.team && candidate.active != 0
        };
        if !admitted || candidate.health <= 0 || participant == source || participant == excluded {
            continue;
        }
        let mut distance = ai_planar_distance(
            [origin.pose[0], origin.pose[1]],
            [candidate.pose[0], candidate.pose[1]],
        );
        if mode != 0 {
            distance = distance.wrapping_mul(2);
        }
        if distance > 300 << 16 {
            continue;
        }
        let error = angle(candidate.pose, origin.pose).wrapping_abs();
        let score = distance.wrapping_add((error.wrapping_mul(2) as i32) << 16);
        if candidate.kind != 8 && distance > 200 << 16 && error > 45 {
            continue;
        }
        if selected == 0 || score < best {
            selected = participant;
            best = score;
        }
    }
    Some(selected)
}

/// CODE 20 +0x0e6c: enter attack tracking unless behavior is 4 or 9.
pub fn ai_begin_attack(state: &mut AiIntentState, target: i8) {
    if matches!(state.behavior, 4 | 9) {
        return;
    }
    state.behavior = 0;
    state.target = target as i16;
    state.movement = 1;
    state.aim_target = target as i16;
    state.heading_deadline = -1;
    state.field_26 = 1;
}

/// CODE 20 +0x1284..+0x14ae, kind 7 tail only. The earlier retarget branch
/// must run separately. Returns whether the cached target pose was requested.
pub fn ai_type7_tracking_tail(
    state: &mut AiIntentState,
    tracked_pose: &mut [i32; 3],
    game_mode: u8,
    active: u8,
    mut cached_pose: impl FnMut() -> [i32; 3],
) -> bool {
    if state.movement == 3 {
        state.movement = 0;
        state.aim_target = state.target;
    }
    if game_mode != 0 && active == 0 {
        state.behavior = 3;
        state.target = 0;
        return false;
    }
    *tracked_pose = cached_pose();
    state.heading_deadline = -1;
    true
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiTrackingParticipant {
    pub intent: AiIntentState,
    pub tracked_pose: [i32; 3],
    pub counter: i16,
    pub active: u8,
}

/// Complete CODE 20 +0xef4, with the cached-pose service supplied by the caller.
/// Returns whether that service was requested. Other participant fields are preserved.
pub fn ai_attack_tracking(
    state: &mut AiTrackingParticipant,
    game_mode: u8,
    source_pose: [i32; 3],
    effective_difficulty: i16,
    mut cached_pose: impl FnMut() -> [i32; 3],
) -> bool {
    if game_mode != 0 && state.active == 0 {
        state.intent.behavior = 3;
        state.intent.target = 0;
        return false;
    }
    state.intent.movement = 0;
    state.tracked_pose = cached_pose();
    state.intent.heading_deadline = -1;
    let limit = 500_i16.wrapping_sub(effective_difficulty.wrapping_mul(3));
    if ai_planar_distance(
        [source_pose[0], source_pose[1]],
        [state.tracked_pose[0], state.tracked_pose[1]],
    ) > i32::from(limit) * 65536
    {
        state.intent.behavior = 3;
    }
    true
}

/// CODE 20 +0xb6c, kind 7 with initializer mode zero. Object/opponent
/// searches and distance run before this transition in the original.
pub fn ai_initialize_type7(
    state: &mut AiTrackingParticipant,
    last_check: &mut i32,
    opponent: i8,
    object: i16,
    player_distance: i32,
    attack_range: i32,
    mut cached_pose: impl FnMut(&AiIntentState) -> [i32; 3],
) -> bool {
    let mut requested_pose = false;
    if object != 0 && player_distance > attack_range.wrapping_mul(2) {
        state.intent.behavior = 1;
        state.intent.target = object;
    } else if opponent != 0 {
        state.intent.behavior = 0;
        state.intent.target = opponent as i16;
        state.intent.movement = 0;
        state.intent.aim_target = opponent as i16;
        state.tracked_pose = cached_pose(&state.intent);
        state.intent.heading_deadline = -1;
        requested_pose = true;
    } else {
        state.intent.behavior = 2;
    }
    *last_check = -1;
    state.intent.movement = 3;
    requested_pose
}

/// CODE 20 +0x153a: approach a selected world object, or guard it nearby.
pub fn ai_track_object(
    state: &mut AiTrackingParticipant,
    participant: i8,
    source_pose: [i32; 3],
    object_pose: [i32; 3],
) {
    let distance = ai_planar_distance(
        [source_pose[0], source_pose[1]],
        [object_pose[0], object_pose[1]],
    );
    if distance < 40 << 16 && participant != 1 {
        state.intent.movement = 2;
        state.intent.aim_target = 1;
    } else {
        state.intent.movement = 0;
        state.intent.aim_target = state.intent.target;
        state.tracked_pose = object_pose;
        state.intent.heading_deadline = -1;
    }
}

/// Complete CODE 20 +0x1240 kind-7 path. Selection and cached-pose services
/// remain explicit. A replacement target changes the local participant,
/// which can differ from the participant whose tracking handler is running.
pub fn ai_type7_tracking(
    participant: i8,
    local: i8,
    game_mode: u8,
    states: &mut [AiTrackingParticipant; 12],
    mut select: impl FnMut(i8, i8, u8) -> i8,
    mut cached_pose: impl FnMut(i8, &AiIntentState) -> [i32; 3],
) -> Option<bool> {
    if !(1..=12).contains(&participant) || !(1..=12).contains(&local) {
        return None;
    }
    let index = participant as usize - 1;
    if states[index].counter >= 5 {
        let target = select(local, states[index].intent.target as i8, 0);
        if target != 0 {
            ai_begin_attack(&mut states[local as usize - 1].intent, target);
        }
    }
    let state = &mut states[index];
    // The service observes intent after movement-mode and inactive checks.
    if state.intent.movement == 3 {
        state.intent.movement = 0;
        state.intent.aim_target = state.intent.target;
    }
    if game_mode != 0 && state.active == 0 {
        state.intent.behavior = 3;
        state.intent.target = 0;
        return Some(false);
    }
    state.tracked_pose = cached_pose(participant, &state.intent);
    state.intent.heading_deadline = -1;
    Some(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiMovementPolicy {
    None,
    Ordinary,
    Combat,
    TurnOnly,
    Alternate,
}

/// CODE 31 +0x1f3a..+0x1fa0: gate command generation, before the AI dispatcher.
/// The caller's inhibit byte is tested through bit zero, not general nonzero.
pub fn ai_commands_suppressed(
    geometry_kind: u8,
    inhibit: u8,
    remaining_enemies: i16,
    distance: i32,
    limit: i32,
) -> bool {
    geometry_kind == 10 || inhibit & 1 != 0 || (remaining_enemies > 5 && distance > limit)
}

/// CODE 20 +0x2290..+0x22e4: only lower participant indices can trigger yielding.
/// Flags are participant bytes +0x88 and +5; admission uses their bitwise AND.
pub fn ai_nearby_predecessor(
    participant: i8,
    flags: &[[u8; 2]; 12],
    positions: &[[i32; 2]; 12],
) -> Option<bool> {
    if !(1..=12).contains(&participant) {
        return None;
    }
    let current = participant as usize - 1;
    Some((0..current).any(|i| {
        flags[i][0] & flags[i][1] != 0
            && ai_planar_distance(positions[i], positions[current]) < 60 << 16
    }))
}

/// CODE 20 +0x1d7c: replace steering only; the throttle output is untouched.
pub fn ai_turn_only(angle: i16) -> i16 {
    if angle > 1 {
        1
    } else if angle < -1 {
        -1
    } else {
        0
    }
}

/// CODE 20 +0x253a..+0x25ca: movement subroutine dispatch.
pub fn ai_movement_policy(mode: u8, nearby_predecessor: bool, distance: i32) -> AiMovementPolicy {
    match mode {
        0 if !nearby_predecessor => AiMovementPolicy::Ordinary,
        1 if nearby_predecessor || distance > 30 << 16 && distance < 120 << 16 => {
            AiMovementPolicy::TurnOnly
        }
        1 => AiMovementPolicy::Combat,
        2 => AiMovementPolicy::TurnOnly,
        4 => AiMovementPolicy::Alternate,
        _ => AiMovementPolicy::None,
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiMovementState {
    pub heading: i32,
    pub angle_threshold: i32,
    pub reverse: u8,
    pub deadline: i32,
}

/// CODE 20 +0x1672, behaviors 0/1/3, nonlocal participant, mode byte zero.
/// Other behaviors and mode-specific overrides are outside this path.
pub fn ai_standard_movement(
    source: [i32; 3],
    target: [i32; 2],
    now: i32,
    state: &mut AiMovementState,
    mut aim: impl FnMut() -> i16,
    mut probe: impl FnMut(i32, i32, i32) -> bool,
) -> (i16, i16) {
    let distance = ai_planar_distance(target, [source[0], source[1]]).min(80 << 16);
    if state.deadline == -1 || now >= state.deadline {
        let target_heading = wrap_angle(source[2].wrapping_add((aim() as i32) << 16));
        // The caller clears its result slot before the search. An unsuccessful
        // search leaves that zero intact rather than producing a -1 sentinel.
        let desired =
            ai_clear_heading(target_heading, |heading| probe(heading, 40 << 16, distance))
                .unwrap_or(0);
        state.angle_threshold = 40 << 16;
        (state.heading, state.reverse, state.deadline) =
            ai_reverse_state(desired, target_heading, true, 2, 0, now);
    }
    let (_, steering, throttle) = ai_movement_commands(
        state.heading,
        source[2],
        state.angle_threshold,
        state.reverse != 0,
        |heading| probe(heading, 40 << 16, distance),
    );
    (steering, throttle)
}

/// CODE 20 +0x1672, behavior 0 or 3, nonlocal participant, mode one.
/// Predicted target pose and angle come from the separate aiming cache.
/// A close-range path can read an unwritten stack local. Supply that value only
/// for reference execution; None guards that path in native callers.
pub fn ai_combat_movement(
    source: [i32; 3],
    predicted_target: [i32; 2],
    now: i32,
    state: &mut AiMovementState,
    scratch_travel_heading: Option<i32>,
    mut aim: impl FnMut() -> i16,
    mut probe: impl FnMut(i32, i32, i32) -> bool,
) -> Option<(i16, i16)> {
    let distance = ai_planar_distance(predicted_target, [source[0], source[1]]);
    let movement_allowed = distance >= 120 << 16;
    if state.deadline == -1 || now >= state.deadline {
        let target_heading = wrap_angle(source[2].wrapping_add((aim() as i32) << 16));
        let desired = if movement_allowed {
            ai_clear_heading(target_heading, |heading| probe(heading, 40 << 16, 80 << 16))
                .unwrap_or(0)
        } else {
            target_heading
        };
        state.angle_threshold = 40 << 16;
        (state.heading, state.reverse, state.deadline) =
            ai_reverse_state(desired, target_heading, movement_allowed, 2, 1, now);
    }
    let (difference, steering, mut throttle) = ai_movement_commands(
        state.heading,
        source[2],
        state.angle_threshold,
        state.reverse != 0,
        |heading| probe(heading, 40 << 16, 80 << 16),
    );
    if distance < 30 << 16 {
        let travel_heading = if difference.wrapping_abs() < state.angle_threshold {
            if state.reverse != 0 {
                wrap_angle(source[2].wrapping_add(HALF_TURN))
            } else {
                source[2]
            }
        } else {
            scratch_travel_heading?
        };
        if probe(
            wrap_angle(travel_heading.wrapping_add(HALF_TURN)),
            40 << 16,
            80 << 16,
        ) {
            throttle = -throttle;
        } else {
            throttle = 0;
        }
    } else if distance < 120 << 16 {
        throttle = 0;
    }
    if !movement_allowed {
        throttle = 0;
    }
    Some((steering, throttle))
}

/// CODE 20 +0x197e..+0x1a46: reverse selection and heading-refresh deadline.
/// The mode byte retains the original XOR/AND behavior for noncanonical values.
pub fn ai_reverse_state(
    desired: i32,
    reference: i32,
    movement_allowed: bool,
    participant: i8,
    mode: u8,
    now: i32,
) -> (i32, u8, i32) {
    let mut heading = desired;
    let mut reverse = 0;
    if movement_allowed {
        let mut difference = desired.wrapping_sub(reference);
        if difference > HALF_TURN {
            difference = difference.wrapping_sub(FULL_TURN);
        } else if difference <= -HALF_TURN {
            difference = difference.wrapping_add(FULL_TURN);
        }
        let threshold = if participant == 1 { 130 } else { 90 } << 16;
        if difference.wrapping_abs() > threshold {
            reverse = mode ^ 1;
        }
        if reverse != 0 {
            heading = wrap_angle(desired.wrapping_add(HALF_TURN));
        }
    }
    let deadline = now.wrapping_add(if participant == 1 { 10 } else { 20 });
    (heading, reverse, deadline)
}

/// CODE 20 +0x05b0: boundary and collision clearance along a travel heading.
/// Direction contains the two original direction-table outputs, in x/z order.
/// The collision callback captures width, distance, and the excluded geometry ID.
/// Its result is the selected object's kind, or None for no selected object.
pub fn ai_clearance_probe(
    position: [i32; 2],
    heading: i32,
    distance: i32,
    arena_side: i16,
    direction: [i32; 2],
    mut collision: impl FnMut([i32; 3]) -> Option<u8>,
) -> bool {
    let scale = (distance.wrapping_mul(2) >> 16) as i16;
    let displacement = direction.map(|v| (v as i16 as i32) * scale as i32);
    let ahead: [i16; 2] = std::array::from_fn(|i| {
        (position[i].wrapping_add(displacement[i].wrapping_mul(2)) >> 16) as i16
    });
    if ahead.iter().any(|&v| v < 0 || v > arena_side) {
        return false;
    }
    let pose = [
        position[0].wrapping_add(displacement[0]),
        position[1].wrapping_add(displacement[1]),
        heading,
    ];
    matches!(collision(pose), None | Some(32))
}

/// CODE 20 +0x06ea: search 0, +15, -15, ... +180, -180 degrees.
/// None represents the original unwritten result when every probe fails.
pub fn ai_clear_heading(heading: i32, mut probe: impl FnMut(i32) -> bool) -> Option<i32> {
    let mut offset: i32 = 0;
    while offset.abs() <= HALF_TURN {
        let candidate = wrap_angle(heading.wrapping_add(offset));
        if probe(candidate) {
            return Some(candidate);
        }
        offset = if offset > 0 {
            -offset
        } else {
            -offset + (15 << 16)
        };
    }
    None
}

/// CODE 20 +0x1b8e..+0x1c88, nonlocal participant movement decision.
/// Commands must start at zero, as at +0x16d6. Later policy overrides are separate.
/// The probe receives the travel heading; original distance and width are caller data.
pub fn ai_movement_commands(
    desired: i32,
    current: i32,
    movement_angle: i32,
    reverse: bool,
    mut probe: impl FnMut(i32) -> bool,
) -> (i32, i16, i16) {
    let mut difference = desired.wrapping_sub(current);
    if difference > HALF_TURN {
        difference = difference.wrapping_sub(FULL_TURN);
    } else if difference <= -HALF_TURN {
        difference = difference.wrapping_add(FULL_TURN);
    }
    let steering = if difference > 1 << 16 {
        1
    } else if difference < -(1 << 16) {
        -1
    } else {
        0
    };
    let mut throttle = 0;
    if difference.wrapping_abs() < movement_angle {
        throttle = if reverse { -1 } else { 1 };
        let travel_heading = if reverse {
            wrap_angle(current.wrapping_add(HALF_TURN))
        } else {
            current
        };
        if difference.wrapping_abs() > 10 << 16 && !probe(travel_heading) {
            throttle = 0;
        }
    }
    (difference, steering, throttle)
}

/// Historical QuickDraw AngleFromSlope scan, with its reference slope table
/// supplied explicitly. This is not a claim of System 7 ROM equivalence.
pub fn historical_angle_from_slope(slope: i32, positive_slopes: &[i32; 91]) -> Option<i16> {
    let biased = slope.wrapping_abs().wrapping_sub(500);
    let angle = positive_slopes
        .iter()
        .position(|&threshold| biased <= threshold)? as i16;
    Some(if slope < 0 { angle } else { 180 - angle })
}

/// CODE 6 +0x1680: aiming angle around explicit FixRatio/AngleFromSlope services.
/// Callback arguments are numerator and denominator, both signed words.
/// None means coincident integer coordinates or a guarded nonterminating rotation.
pub fn ai_aim_angle(
    target: [i32; 2],
    source: [i32; 3],
    mut slope_angle: impl FnMut(i16, i16) -> i16,
) -> Option<i16> {
    let mut dx = (target[0].wrapping_sub(source[0]) >> 16) as i16;
    let mut dz = (target[1].wrapping_sub(source[1]) >> 16) as i16;
    if dx == 0 && dz == 0 {
        return None;
    }
    let mut quadrant = 0_i16;
    let mut rotations = 0;
    while dx <= 0 || dz > 0 {
        if rotations == 4 {
            return None;
        }
        (dx, dz) = (dz.wrapping_neg(), dx);
        quadrant = quadrant.wrapping_sub(90);
        rotations += 1;
    }
    let base = if ((dx as u16 as u32) >> 8) as i32 > dz.wrapping_abs() as i32 {
        0_i16
    } else {
        90_i16.wrapping_sub(slope_angle(dx, dz.wrapping_neg()))
    };
    let mut angle = base
        .wrapping_add(quadrant)
        .wrapping_sub((source[2] >> 16) as i16);
    while angle > 180 {
        angle = angle.wrapping_sub(360);
    }
    while angle < -179 {
        angle = angle.wrapping_add(360);
    }
    Some(angle)
}

/// CODE 6 +0x1760: planar distance approximation, with wrapping long arithmetic.
pub fn ai_planar_distance(a: [i32; 2], b: [i32; 2]) -> i32 {
    let dx = a[0].wrapping_sub(b[0]).wrapping_abs();
    let dz = a[1].wrapping_sub(b[1]).wrapping_abs();
    ((dx.wrapping_add(dx.max(dz)).wrapping_add(dz) as u32) >> 1) as i32
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Type7FireCheck {
    pub now: i32,
    pub timer: i32,
    pub distance: i32,
    pub interval: i16,
    pub short_range: i32,
    pub firing_arc: i16,
    /// Supplied signed word result from the original angle service.
    pub angle: i16,
    /// Target geometry +0x28: signed 16.16 vertical position; negative means submerged.
    pub target_field_28: i32,
}

/// CODE 20 +212a..+2154: firing uses the cached angle only on this path.
pub fn ai_fire_uses_cached_angle(target: i8, source: i8, movement: u8) -> bool {
    target == 1 && source != 1 && movement == 1
}

/// Type-7 enemy path through CODE 20 +0x1e9a, with a target present.
/// Returns eligibility and the possibly initialized timer, not a fired shot.
pub fn type7_fire_check(input: Type7FireCheck) -> (bool, i32) {
    let mut timer = input.timer;
    if input.distance > 300 << 16 {
        return (false, timer);
    }
    if timer == -1 {
        timer = input.now;
    }
    let due = timer.wrapping_add(input.interval as i32) <= input.now
        || (timer.wrapping_add(20) <= input.now && input.distance <= input.short_range);
    if !due || input.target_field_28 < 0 {
        return (false, timer);
    }
    let angle = input.angle.wrapping_abs();
    let aligned =
        angle <= 4 || (angle as i32 * (input.distance >> 16)) <= input.firing_arc as i32 * 200;
    (aligned, timer)
}

/// Enemy-owned CODE 20 +0x1e9a with a target present. Angle result is supplied.
/// Returns eligibility, timer, geometry byte +0x5b, behavior, and movement mode.
pub fn enemy_fire_check(
    mut input: Type7FireCheck,
    kind: u8,
    source_field_28: i32,
    mut geometry_flag: u8,
    mut behavior: u8,
    mut movement: u8,
) -> (bool, i32, u8, u8, u8) {
    let maximum = if kind == 20 { 600 } else { 300 } << 16;
    if kind == 13 || input.distance > maximum {
        return (false, input.timer, geometry_flag, behavior, movement);
    }
    if matches!(kind, 21 | 25 | 35 | 45) {
        input.timer = input.now.wrapping_add(10);
    } else if kind == 22
        && source_field_28
            .wrapping_sub(input.target_field_28)
            .wrapping_abs()
            < 25 << 16
    {
        input.timer = -1;
    } else if kind == 26 && geometry_flag == 0 {
        input.timer = -1;
        geometry_flag = 1;
    }
    let mut timer = input.timer;
    if timer == -1 {
        timer = input.now;
    }
    let due = timer.wrapping_add(input.interval as i32) <= input.now
        || (timer.wrapping_add(20) <= input.now && input.distance <= input.short_range);
    let angle = input.angle.wrapping_abs();
    let eligible = due
        && input.target_field_28 >= 0
        && (angle <= 4 || angle as i32 * (input.distance >> 16) <= input.firing_arc as i32 * 200);
    if eligible && kind == 26 {
        behavior = 4;
        movement = 0;
        geometry_flag = 0;
    }
    (eligible, timer, geometry_flag, behavior, movement)
}

/// CODE 31 +0x1fd0..+0x212a: expand a firing command into weapon requests.
/// Each result is (signed weapon byte, signed integer height, network route).
/// This does not allocate projectiles or apply weapon-specific effects.
pub fn enemy_weapon_requests(
    command: i16,
    weapon: i16,
    game_mode: u8,
    network_enabled: u8,
    height: i32,
) -> Vec<(i8, i16, bool)> {
    if command == 0 || weapon == -1 {
        return vec![];
    }
    let weapon = weapon as i8;
    let height = (height / 65536) as i16;
    if game_mode != 0 {
        return if network_enabled != 0 {
            vec![(weapon, height, true)]
        } else {
            vec![]
        };
    }
    let weapons = if weapon == 5 {
        vec![5, 3, 4]
    } else if weapon == 12 && command == 1 {
        vec![0]
    } else {
        vec![weapon]
    };
    weapons
        .into_iter()
        .map(|weapon| (weapon, height, false))
        .collect()
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct LocalWeaponAttempt {
    pub now: i32,
    pub last_attempt: i32,
    pub disabled: u8,
    pub weapon: u8,
    pub ammo: i16,
    pub cost: i16,
    pub health: i16,
    pub inhibit: u8,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct LocalWeaponResult {
    pub timestamp: i32,
    pub request: Option<(u8, i16)>,
    pub empty_sound: bool,
}

/// CODE 30 +08b2 with CODE 28 eligibility and single-player CODE 27/26 dispatch.
/// Projectile construction/admission is separate; valid resource slots are 0..17.
pub fn local_weapon_attempt(input: LocalWeaponAttempt) -> Option<LocalWeaponResult> {
    if input.disabled != 0 {
        return Some(LocalWeaponResult {
            timestamp: input.last_attempt,
            request: None,
            empty_sound: false,
        });
    }
    if input.weapon >= 18 {
        return None;
    }
    let eligible = primary_fire_eligible(input.ammo, input.cost, input.health, input.inhibit) != 0;
    Some(LocalWeaponResult {
        timestamp: input.now,
        request: eligible.then_some((input.weapon, (input.height / 65536) as i16)),
        empty_sound: !eligible && input.ammo <= input.cost,
    })
}

/// CODE 31 +187a..+18c2: optional fire shares the primary-attempt timestamp.
/// Scattershot emits three requests after one gate; this branch does not update the timestamp.
pub fn optional_weapon_fire(now: i32, last_attempt: i32, control: i16, selected: u8) -> Vec<i8> {
    if control != 1 || now.wrapping_sub(last_attempt) <= 15 {
        return vec![];
    }
    if selected == 5 {
        vec![5, 3, 4]
    } else {
        vec![selected as i8]
    }
}

/// CODE 31 +18c2..+1902, including the original weapon cycle.
/// Returns selection, last accepted selection tick, and whether sound 0x10022 is requested.
pub fn optional_weapon_selection(
    now: i32,
    last: i32,
    control: i16,
    selected: u8,
    available: &[u8; 18],
) -> Option<(u8, i32, bool)> {
    if control != 1 || now.wrapping_sub(last) <= 10 {
        return Some((selected, last, false));
    }
    let next = cycle_optional_weapon(selected, available)?;
    Some((next, now, next != selected))
}

/// CODE 6 +121e: advance through all eighteen slots, testing availability bit zero.
/// Invalid current slots are guarded instead of reading adjacent original globals.
pub fn cycle_optional_weapon(current: u8, available: &[u8; 18]) -> Option<u8> {
    if current >= 18 {
        return None;
    }
    let mut next = current;
    loop {
        next = if next == 17 { 0 } else { next + 1 };
        if available[next as usize] & 1 != 0 || next == current {
            return Some(next);
        }
    }
}

/// CODE 6 +54ec: a wave hint unlocks weapons and selects newly available ones.
pub fn unlock_optional_weapon(hint: i16, selected: &mut u8, available: &mut [u8; 18]) {
    let one = match hint {
        10 => 1,
        11 => 9,
        12 => 10,
        13 => 2,
        14 => 11,
        15 => 5,
        17 => 17,
        18 => 13,
        16 => {
            for weapon in [10, 2, 9, 5, 1, 17, 13, 11] {
                if available[weapon] == 0 {
                    *selected = weapon as u8;
                }
            }
            for weapon in [10, 2, 9, 5, 1, 17, 13, 11] {
                available[weapon] = 1;
            }
            return;
        }
        _ => return,
    };
    if available[one] == 0 {
        *selected = one as u8;
    }
    available[one] = 1;
}

/// CODE 30 +0382..+03ca composed with original CODE 20 target selection.
/// Selection uses the owner's pose; final correction uses the projectile pose.
pub fn weapon_target_angle(
    owner: i8,
    game_mode: u8,
    projectile_pose: [i32; 3],
    participants: &[AiTargetCandidate; 12],
    mut angle: impl FnMut([i32; 3], [i32; 3]) -> i16,
) -> Option<i16> {
    let selected = ai_select_target(owner, 0, 0, game_mode, participants, &mut angle)?;
    let target = participants.get(usize::try_from(selected - 1).ok()?)?;
    Some(angle(target.pose, projectile_pose))
}

/// CODE 20 +08f8: projectile-based selection, excluding only the owner.
/// Returns zero when no active, living participant meets the distance/angle gates.
pub fn seeker_select_target(
    owner: i8,
    mode: u8,
    projectile: [i32; 3],
    participants: &[AiTargetCandidate; 12],
    mut angle: impl FnMut([i32; 3], [i32; 3]) -> i16,
) -> i8 {
    let mut selected = 0;
    let mut best = 0;
    for (index, candidate) in participants.iter().enumerate() {
        let participant = index as i8 + 1;
        if candidate.active == 0 || candidate.health <= 0 || participant == owner {
            continue;
        }
        let mut distance = ai_planar_distance(
            [projectile[0], projectile[1]],
            [candidate.pose[0], candidate.pose[1]],
        );
        if mode != 0 {
            distance = distance.wrapping_mul(2);
        }
        if distance > 300 << 16 {
            continue;
        }
        let error = angle(candidate.pose, projectile).wrapping_abs();
        let score = distance.wrapping_add((error.wrapping_mul(2) as i32) << 16);
        if candidate.kind != 8 && distance > 200 << 16 && error > 45 {
            continue;
        }
        if selected == 0 || score < best {
            selected = participant;
            best = score;
        }
    }
    selected
}

/// CODE 30 +240e..+248c: Seekers reacquire and turn before planar movement.
/// The signed angle is divided by three before conversion to fixed point.
pub fn seeker_heading(
    owner: i8,
    projectile: [i32; 3],
    participants: &[AiTargetCandidate; 12],
    mut angle: impl FnMut([i32; 3], [i32; 3]) -> i16,
) -> i32 {
    let selected = seeker_select_target(owner, 0, projectile, participants, &mut angle);
    if selected == 0 {
        return projectile[2];
    }
    let correction = angle(participants[selected as usize - 1].pose, projectile) as i32 / 3;
    wrap_angle(projectile[2].wrapping_add(correction << 16))
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct MissileGuidance {
    pub pose: [i32; 3],
    pub height: i32,
    pub vertical_velocity: i32,
}

/// CODE 30 +211c..+216a: kind-6 owners add one-third of elapsed time.
/// None represents owner zero or the original missing-geometry sentinel.
pub fn smart_missile_elapsed(elapsed: i32, owner_geometry_kind: Option<u8>) -> i32 {
    if owner_geometry_kind == Some(6) {
        elapsed.wrapping_add(elapsed / 3)
    } else {
        elapsed
    }
}

/// CODE 30 +216a..+22a6: ascent, reacquisition, steering, and height correction.
/// Elapsed time already includes the owner's kind-6 adjustment in the caller.
pub fn smart_missile_guidance(
    state: &mut MissileGuidance,
    owner: i8,
    elapsed: i32,
    participants: &[AiTargetCandidate; 12],
    heights: &[i32; 12],
    mut angle: impl FnMut([i32; 3], [i32; 3]) -> i16,
) {
    if state.vertical_velocity > 0 {
        if state.height > 100 << 16 {
            state.vertical_velocity = -2 << 16;
        } else {
            let rise = state
                .vertical_velocity
                .wrapping_mul(2)
                .wrapping_mul(elapsed)
                / 5;
            state.height = state.height.wrapping_add(rise);
        }
        return;
    }
    let selected = seeker_select_target(owner, 0, state.pose, participants, &mut angle);
    let step = state
        .vertical_velocity
        .wrapping_mul(2)
        .wrapping_mul(elapsed)
        / 5;
    if selected != 0 {
        let index = selected as usize - 1;
        let correction = angle(participants[index].pose, state.pose) as i32 / 3;
        state.pose[2] = wrap_angle(state.pose[2].wrapping_add(correction << 16));
        if heights[index] > state.height {
            state.height = state.height.wrapping_sub(step);
        } else {
            state.height = state.height.wrapping_add(step);
        }
    } else {
        state.height = state.height.wrapping_add(step);
    }
    state.height = state.height.max(0);
}

/// CODE 30 +22a6..+22ec: horizontal Smart Missile movement after guidance.
/// The table is the original projectile table (six-times-unit), not the unit table.
/// Elapsed time must include the owner's kind-6 adjustment exactly once.
pub fn move_smart_missile(
    state: &mut MissileGuidance,
    elapsed: i32,
    projectile_table: &[i32; 360],
) -> Option<()> {
    let direction = movement_direction(state.pose[2], projectile_table)?;
    for (position, component) in state.pose[..2].iter_mut().zip(direction) {
        let step = component.wrapping_mul(2).wrapping_mul(elapsed) / 5;
        *position = position.wrapping_add(step);
    }
    Some(())
}

/// CODE 30 +0x2ee..+0x3ee: constructor heading preparation, before direction lookup.
/// Weapons 3/4 use one Random sample; weapon 8 requests a target angle.
/// These variants then continue through the weapon-0 construction path.
pub fn weapon_initial_heading(
    weapon: i8,
    mut heading: i32,
    mut random: impl FnMut() -> i16,
    mut target_angle: impl FnMut() -> Option<i16>,
) -> (i8, i32) {
    match weapon {
        3 => {
            let spread = ((random().wrapping_abs() % 8 + 7) as i32) << 16;
            heading = heading.wrapping_add(spread);
            if heading >= 360 << 16 {
                heading = heading.wrapping_sub(360 << 16);
            }
            (0, heading)
        }
        4 => {
            let spread = ((random().wrapping_abs() % 8 + 7) as i32) << 16;
            heading = heading.wrapping_sub(spread);
            if heading < 0 {
                heading = heading.wrapping_add(360 << 16);
            }
            (0, heading)
        }
        8 => {
            if let Some(angle) = target_angle() {
                heading = wrap_angle(heading.wrapping_add((angle as i32) << 16));
            }
            (0, heading)
        }
        _ => (weapon, heading),
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct WeaponRecordInput {
    pub weapon: i8,
    pub now: i32,
    pub lifetime: i16,
    pub owner: i8,
    pub source_kind: u8,
    pub owner_field_5c: i16,
    pub direction: [i32; 2],
}

/// CODE 25 +0x2902..+0x2916. Fresh single-player settings set this flag
/// to one (CODE 24 +0x142a, copied by CODE 25 +0x2608).
pub fn concurrent_shot_limit(session_flag: u8) -> i16 {
    if session_flag != 0 { 20 } else { 5 }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct WeaponAdmission {
    pub owner: i8,
    pub local: i8,
    pub team: u8,
    pub enemy: u8,
    pub active_shots: i16,
    pub shot_limit: i16,
    pub ammo: i16,
    pub enemy_cost: i16,
    pub player_cost: i16,
    pub free_ammo: u8,
}

#[derive(Debug, serde::Serialize)]
pub struct WeaponAdmissionResult {
    pub ammo: i16,
    pub allocation_called: bool,
    /// Raw constructor admission includes index zero; native allocation guards it separately.
    pub slot: Option<i16>,
    pub sounds: Vec<(i16, i32)>,
}

/// CODE 30 +0x140..+0x292, before successful-allocation counters and record writes.
pub fn admit_weapon(
    input: WeaponAdmission,
    mut allocate: impl FnMut() -> i16,
) -> WeaponAdmissionResult {
    let mut result = WeaponAdmissionResult {
        ammo: input.ammo,
        allocation_called: false,
        slot: None,
        sounds: vec![],
    };
    if input.owner != 0 && input.team == 3 {
        return result;
    }
    let cost = if input.owner == 0 || input.enemy != 0 {
        input.enemy_cost
    } else {
        input.player_cost
    };
    if input.owner != 0 {
        if input.active_shots >= input.shot_limit {
            return result;
        }
        if input.enemy == 0 && input.owner == input.local {
            if input.ammo < cost {
                result.sounds.push((1, 0x30006));
                return result;
            }
            if input.free_ammo == 0 {
                let remaining = input.ammo.wrapping_sub(cost);
                if input.ammo > 5 && remaining <= 5 {
                    result.sounds.extend([(1, 0x1fc1c), (1, 0x3000a)]);
                }
                result.ammo = remaining;
            }
        }
    }
    result.allocation_called = true;
    let slot = allocate();
    if slot < 0 {
        if input.owner == input.local {
            result.ammo = result.ammo.wrapping_add(cost);
        }
    } else {
        result.slot = Some(slot);
    }
    result
}

/// Complete CODE 25 +0xe0e. The original checks equality with 302, not >=.
/// This exposes its raw counter semantics for differential verification.
pub fn original_geometry_slot(counter: &mut i16) -> i16 {
    if *counter == 302 {
        0
    } else {
        *counter = counter.wrapping_add(1);
        *counter
    }
}

/// Native allocation guard: geometry uses one-based indices in a 302-record pool.
/// Reject invalid/full counters without mutation, including original return zero.
pub fn allocate_geometry_slot(counter: &mut i16) -> Option<i16> {
    if !(0..302).contains(counter) {
        return None;
    }
    Some(original_geometry_slot(counter))
}

/// Participant fields touched by CODE 25's geometry removal routine.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GeometryParticipants {
    pub ids: [i16; 12],
    pub shots: [i16; 12],
    pub active: [u8; 12],
    /// CODE 14 status array at A5 -0x4c40.
    pub status: [u8; 12],
}

/// CODE 25 +0xe62..+0x1054, including the CODE 14 +0x2e20 status write.
/// Invalid native references return None without changing any state.
pub fn remove_geometry(
    records: &mut [[u8; 128]],
    count: &mut i16,
    removed: i16,
    participants: &mut GeometryParticipants,
    grid: &mut [Vec<i16>; 256],
) -> Option<()> {
    if *count < 1 || *count as usize > records.len() || removed < 1 || removed > *count {
        return None;
    }
    let old = records[removed as usize - 1];
    let moved = records[*count as usize - 1];
    let participant_kind = |kind| matches!(kind, 6 | 7 | 8 | 10 | 13 | 20..=27 | 31 | 35 | 45);
    let shot = matches!(old[0x14], 9 | 28 | 29 | 32 | 33 | 34 | 43 | 44 | 46);
    let owner = old[0x1a] as i8;
    let deactivate = participant_kind(old[0x14]);
    if ((shot && owner != 0) || deactivate) && !(1..=12).contains(&owner) {
        return None;
    }
    let moved_owner = moved[0x1a] as i8;
    if participant_kind(moved[0x14]) && moved_owner != 0 && !(1..=12).contains(&moved_owner) {
        return None;
    }
    let cell = |r: &[u8; 128]| {
        let position =
            [0, 4].map(|offset| i32::from_be_bytes(r[offset..offset + 4].try_into().unwrap()));
        let [x, z] = object_grid_cell(position);
        x * 16 + z
    };
    let old_cell = cell(&old);
    if grid[old_cell].len() > 20 || grid[cell(&moved)].len() > 20 {
        return None;
    }
    remove_grid_entry(&mut grid[old_cell], removed);
    if shot && owner != 0 {
        let shots = &mut participants.shots[owner as usize - 1];
        *shots = shots.wrapping_sub(1);
    } else if deactivate {
        participants.active[owner as usize - 1] = 0;
        participants.status[owner as usize - 1] = 0;
    }
    compact_geometry_tail(records, count, removed, &mut participants.ids, grid)?;
    Some(())
}

/// CODE 25 +0xf4a..+0x104a, after removal-side bookkeeping and grid unlinking.
/// Copy the last geometry record into the removed slot and repair its references.
/// Invalid native indices return None before any mutation.
pub fn compact_geometry_tail(
    records: &mut [[u8; 128]],
    count: &mut i16,
    removed: i16,
    participant_ids: &mut [i16; 12],
    grid: &mut [Vec<i16>; 256],
) -> Option<()> {
    if *count < 1 || *count as usize > records.len() || removed < 1 || removed > *count {
        return None;
    }
    let last = *count as usize - 1;
    let destination = removed as usize - 1;
    let moved = records[last];
    let participant_kind = matches!(moved[0x14], 6 | 7 | 8 | 10 | 13 | 20..=27 | 31 | 35 | 45);
    let owner = moved[0x1a] as i8;
    if participant_kind && owner != 0 && !(1..=12).contains(&owner) {
        return None;
    }
    let position =
        [0, 4].map(|offset| i32::from_be_bytes(moved[offset..offset + 4].try_into().unwrap()));
    let [x, z] = object_grid_cell(position);
    let cell = &mut grid[x * 16 + z];
    if cell.len() > 20 {
        return None;
    }
    records[destination] = moved;
    if participant_kind && owner != 0 {
        participant_ids[owner as usize - 1] = removed;
    }
    for id in cell.iter_mut() {
        if *id == *count {
            *id = removed;
        }
    }
    records[last][0x14] = 14;
    records[last][0x1a] = 0;
    *count -= 1;
    Some(())
}

/// CODE 31 +a22. Preserve the returned byte, including noncanonical enemy flags.
pub fn death_burst_gate(enemy: u8, participant_kind: u8, field_136e: i16) -> u8 {
    let enemy_gate = if field_136e > 27 { enemy } else { 0 };
    if enemy_gate != 0 {
        enemy_gate
    } else {
        u8::from(participant_kind == 8 && field_136e > 19)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct RadialBurstInput {
    pub owner: i8,
    pub local: i8,
    pub field_136e: i16,
    pub game_mode: u8,
    pub network_authority: u8,
    pub owner_enemy: u8,
    pub now: i32,
}

#[derive(Debug, serde::Serialize)]
pub struct RadialBurstRequest {
    pub now: Option<i32>,
    pub weapon: u8,
    pub source_kind: Option<u8>,
    pub network: bool,
    pub pose: [i32; 3],
    pub height: i16,
    pub owner: i8,
}

/// CODE 31 +8c6: capture weapon-0 constructor or network requests without
/// allocating projectiles. The local constructor also receives now and source kind 7.
pub fn radial_burst_requests(
    record: &[u8; 128],
    input: RadialBurstInput,
    mut random: impl FnMut() -> i16,
) -> Vec<RadialBurstRequest> {
    if input.field_136e <= 20 {
        return Vec::new();
    }
    let count = if record[0x1a] as i8 == input.local {
        2
    } else {
        4
    };
    let step = 360 / count;
    let mut heading: i16 = if input.field_136e > 27 {
        random().wrapping_abs() % 360
    } else {
        0
    };
    let mut requests = Vec::new();
    for _ in 0..count {
        heading = heading.wrapping_abs().wrapping_add(step) % 360;
        let allowed = input.owner == input.local
            || (input.network_authority != 0
                && (input.owner == 0 || (input.network_authority & input.owner_enemy & 1) != 0));
        if input.game_mode == 0 || allowed {
            requests.push(RadialBurstRequest {
                now: (input.game_mode == 0).then_some(input.now),
                weapon: 0,
                source_kind: (input.game_mode == 0).then_some(7),
                network: input.game_mode != 0,
                pose: [
                    i32::from_be_bytes(record[0..4].try_into().unwrap()),
                    i32::from_be_bytes(record[4..8].try_into().unwrap()),
                    i32::from(heading) * 65536,
                ],
                height: (i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()) / 65536) as i16,
                owner: input.owner,
            });
        }
    }
    requests
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeathFragmentsInput {
    pub owner: i8,
    pub local: i8,
    pub game_mode: u8,
    pub network_authority: u8,
    pub owner_enemy: u8,
    pub now: i32,
}

/// Complete CODE 31 +776 request producer. Constructor/network services are
/// returned as requests; their own Random consumption is outside this helper.
pub fn death_fragment_requests(
    record: &[u8; 128],
    input: DeathFragmentsInput,
    mut random: impl FnMut() -> i16,
) -> Vec<RadialBurstRequest> {
    let count = random().wrapping_abs() % 3 + 3;
    let initial = random().wrapping_abs() % 360;
    let step = 360 / count;
    let allowed = input.game_mode == 0
        || input.owner == input.local
        || (input.network_authority != 0
            && (input.owner == 0 || (input.network_authority & input.owner_enemy & 1) != 0));
    let mut requests = Vec::new();
    for index in 1..=count {
        let angle = index.wrapping_mul(step).wrapping_add(initial);
        if allowed {
            requests.push(RadialBurstRequest {
                now: (input.game_mode == 0).then_some(input.now),
                weapon: 6,
                source_kind: (input.game_mode == 0).then_some(record[0x15]),
                network: input.game_mode != 0,
                pose: [
                    i32::from_be_bytes(record[0..4].try_into().unwrap()),
                    i32::from_be_bytes(record[4..8].try_into().unwrap()),
                    wrap_angle(i32::from(angle) * 65536),
                ],
                height: (i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap()) / 65536) as i16,
                owner: input.owner,
            });
        }
    }
    requests
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct ParticipantTransitionInput {
    pub now: i32,
    pub deadline: i32,
    pub kind: u8,
    pub game_mode: u8,
    pub local: bool,
    pub authority: u8,
    pub enemy: u8,
    pub active: u8,
    pub changed: u8,
}

/// Complete CODE 31 +1cfe: admits the +1a80 lifecycle handler.
/// This is not a general simulation-readiness predicate. Preserve byte results.
pub fn participant_transition_ready(input: ParticipantTransitionInput) -> u8 {
    let elapsed = input.deadline != 2_000_000_000 && input.deadline <= input.now;
    if input.kind != 10 {
        return u8::from(elapsed);
    }
    if input.game_mode == 0
        || input.local
        || (input.authority & input.enemy & input.active & 1) != 0
    {
        u8::from(elapsed)
    } else {
        input.changed ^ 1
    }
}

/// CODE 31 +1aa0..+1afa after the active-participant check.
/// Health/life handling and spawn services follow this block in the original.
pub fn begin_participant_transition(participant: &mut [u8; 256], record: &mut [u8; 128]) {
    participant[0x54] = 0;
    record[0x16..0x1a].copy_from_slice(&2_000_000_000_i32.to_be_bytes());
    record[0x14] = if participant[5] != 0 {
        participant[8]
    } else {
        6
    };
}

/// CODE 31 +b9a..+bae: multiplayer uses the supplied byte verbatim.
pub fn damage_killed(health: i16, game_mode: u8, supplied: u8) -> u8 {
    if game_mode == 0 {
        u8::from(health <= 0)
    } else {
        supplied
    }
}

/// CODE 31 +bae..+c34. State spans globals A5-1338 through A5-131b.
/// Preserve unnamed bytes until their consumers establish the full transition.
pub fn local_death_state(
    record: &mut [u8; 128],
    state: &mut [u8; 30],
    now: i32,
    local_victim: bool,
    killed: u8,
) {
    if !local_victim || killed == 0 {
        return;
    }
    for offset in [0xc, 0xe, 0x22] {
        record[offset..offset + 2].fill(0);
    }
    state[29] = 1;
    state[16..28].copy_from_slice(&record[..12]);
    let heading = i32::from_be_bytes(record[8..12].try_into().unwrap());
    state[24..28].copy_from_slice(&wrap_angle(heading.wrapping_add(180 << 16)).to_be_bytes());
    state[14..16].copy_from_slice(&200_i16.to_be_bytes());
    state[8..12].copy_from_slice(&360_i32.to_be_bytes());
    state[12..14].copy_from_slice(&750_i16.to_be_bytes());
    state[..4].copy_from_slice(&now.to_be_bytes());
    state[4..8].copy_from_slice(&200_i32.to_be_bytes());
}

#[derive(Debug, serde::Serialize)]
pub struct DisplayRectangleSetup {
    pub size: [i32; 2],
    pub inset: Option<[i16; 2]>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ColorBindings {
    pub vehicles: [[u8; 2]; 4],
    pub kinds: Vec<[u8; 2]>,
    pub effects: [u8; 4],
    /// Original order: A5-3b32, -3b33, -3b31, -3b34 through -3b38.
    /// Entry zero is the ground-point color index.
    pub special: [u8; 8],
}

/// CODE 6 +838..+94a, after selecting cob1/cob4/cob8 resource 999.
/// This block skips the first word and ignores the low bytes of the last 12
/// entries. Later display-depth overrides are outside this helper.
pub fn color_bindings(data: &[u8]) -> Option<ColorBindings> {
    let bytes = data.get(..128)?;
    let pair = |index: usize| [bytes[2 + index * 2], bytes[3 + index * 2]];
    Some(ColorBindings {
        vehicles: std::array::from_fn(pair),
        kinds: (4..51).map(pair).collect(),
        effects: std::array::from_fn(|index| pair(51 + index)[0]),
        special: std::array::from_fn(|index| pair(55 + index)[0]),
    })
}

/// CODE 12 +145e..+14be. Inputs are the two signed low words produced by
/// its direction lookup, in local-variable order (-5c8, -5c4).
pub fn face_light_components(direction: [i16; 2]) -> [i16; 2] {
    let [a, b] = direction.map(i32::from);
    [
        ((a * 858 + b * 2304).wrapping_shl(2) >> 16) as i16,
        ((a * 2304 - b * 858).wrapping_shl(2) >> 16) as i16,
    ]
}

#[derive(Debug, serde::Serialize)]
pub struct FaceFlashState {
    pub deadline: i32,
    pub toggle: u8,
}

/// CODE 12 +18dc..+1924, with caller default initialized at +17b2.
/// `face` is [stored color, face index, flash limit]. Two TickCount results
/// are supplied because the original reads the clock again on a toggle.
pub fn rendered_face_color(
    face: [i16; 3],
    default_color: u8,
    flash_color: u8,
    ticks: [i32; 2],
    state: &mut FaceFlashState,
) -> i16 {
    let [color, index, limit] = face;
    if color == 13 {
        return i16::from(default_color as i8);
    }
    if ticks[0] > state.deadline {
        state.deadline = ticks[1].wrapping_add(13);
        state.toggle ^= 1;
    }
    if state.toggle != 0 && index < limit {
        i16::from(flash_color as i8)
    } else {
        color
    }
}

/// CODE 12 +26c computes a face plane from three packed signed-word vertices.
/// Output order matches its 12-byte record: normal X/Z, plane offset, normal Y,
/// then color. Degenerate faces retain the original [-1, -1, -1] normal.
pub fn face_plane(vertices: [[i16; 3]; 3], color: i16) -> [u8; 12] {
    let a: [i32; 3] =
        std::array::from_fn(|i| i32::from(vertices[1][i].wrapping_sub(vertices[0][i])));
    let b: [i32; 3] =
        std::array::from_fn(|i| i32::from(vertices[2][i].wrapping_sub(vertices[1][i])));
    let mut normal = [
        b[1] * a[2] - a[1] * b[2],
        b[2] * a[0] - a[2] * b[0],
        b[0] * a[1] - a[0] * b[1],
    ];
    while normal.iter().any(|value| value.abs() > 5000) {
        normal = normal.map(|value| value / 4);
    }
    let squared: u32 = normal.iter().map(|value| (value * value) as u32).sum();
    // CODE 12 +20: scaled seed, followed by three rounded integer iterations.
    let length = if squared == 0 {
        0
    } else {
        let mut seed = 1_u32;
        let mut scaled = squared;
        while scaled > 2 {
            scaled >>= 2;
            seed *= 2;
        }
        for _ in 0..3 {
            seed = (seed + (squared + seed / 2) / seed) / 2;
        }
        seed as i32
    };
    normal = if length == 0 {
        [-1; 3]
    } else {
        normal.map(|value| value * 1024 / length)
    };
    let offset = -normal
        .iter()
        .zip(vertices[1])
        .map(|(n, v)| n * i32::from(v))
        .sum::<i32>();
    let mut record = [0; 12];
    record[..2].copy_from_slice(&(normal[0] as i16).to_be_bytes());
    record[2..4].copy_from_slice(&(normal[2] as i16).to_be_bytes());
    record[4..8].copy_from_slice(&offset.to_be_bytes());
    record[8..10].copy_from_slice(&(normal[1] as i16).to_be_bytes());
    record[10..].copy_from_slice(&color.to_be_bytes());
    record
}

/// CODE 17 +d56..+d64 selects a word by scanline parity. The eight-bit
/// solid-span path at +1054 repeats its high/low bytes at aligned addresses.
/// Coordinates are framebuffer coordinates, not offsets within a polygon.
pub fn face_pattern_pixel_8(pattern: u32, x: i32, y: i32) -> u8 {
    let row = if y & 1 == 0 {
        pattern as u16
    } else {
        (pattern >> 16) as u16
    };
    if x & 1 == 0 {
        (row >> 8) as u8
    } else {
        row as u8
    }
}

/// Complete CODE 11 +ca0. The fraction is capped above at 65535, then
/// narrowed to a word. Weights sum to 65535; the result uses the high word.
pub fn interpolate_rgb(base: [u16; 3], target: [u16; 3], fraction: i32) -> [u16; 3] {
    let weight = u32::from(fraction.min(65535) as u16);
    std::array::from_fn(|i| {
        ((u32::from(base[i]) * (65535 - weight) + u32::from(target[i]) * weight) >> 16) as u16
    })
}

/// CODE 11 +1360..+13ec, with 13 active colors and 16 shades per color.
/// Fraction j/16 is exact in fixed point. The caller supplies the fade color.
/// The final base color is reserved and has no ramp in this block.
pub fn rgb_shade_ramps_8(base: &[[u16; 3]; 13], fade: [u16; 3]) -> Vec<[u16; 3]> {
    base[..12]
        .iter()
        .flat_map(|rgb| (0..16).map(move |step| interpolate_rgb(*rgb, fade, step << 12)))
        .collect()
}

/// Complete CODE 11 +21be shade-to-pixel lookup. The table is A5-7526;
/// negative or absent color indices are rejected instead of reading globals.
pub fn shade_pixel_index(
    shade: i16,
    color: u8,
    depth: i16,
    four_bit_color: bool,
    color_map: &[i16],
) -> Option<i16> {
    if color == 3 && depth == 4 && four_bit_color {
        return Some(15_i16.wrapping_sub(shade.min(3)).wrapping_mul(17));
    }
    if shade == 0 {
        return Some(255);
    }
    if depth == 4 && !four_bit_color {
        return Some(15_i16.wrapping_sub(shade).wrapping_mul(17));
    }
    let mapped = *color_map.get(usize::try_from(color as i8).ok()?)?;
    Some(if depth == 4 {
        mapped
            .wrapping_shl(2)
            .wrapping_add(4)
            .wrapping_sub(shade)
            .wrapping_mul(17)
    } else {
        mapped.wrapping_shl(4).wrapping_add(16).wrapping_sub(shade)
    })
}

/// Complete CODE 11 +227a with display depth eight and A5-7bf1 cleared.
/// Returns the packed pattern word, not RGB. Other display modes are separate.
pub fn face_pixel_pattern_8(shade: i16, color: u8, color_map: &[i16]) -> Option<u32> {
    if shade <= 0 || color == 12 {
        return Some(u32::MAX);
    }
    let mut shade = shade.min(64);
    let color = if color == 11 {
        shade = ((shade as u16) >> 1) as i16;
        0
    } else {
        color
    };
    let mut pixels = [0_i16; 4];
    for (offset, pixel) in pixels.iter_mut().enumerate() {
        *pixel = shade_pixel_index((shade + offset as i16) / 4, color, 8, false, color_map)?;
    }
    let [a, b, c, d] = pixels;
    Some(
        u32::from(a as u16)
            .wrapping_shl(8)
            .wrapping_add(i32::from(c) as u32)
            .wrapping_shl(8)
            .wrapping_add(i32::from(d) as u32)
            .wrapping_shl(8)
            .wrapping_add(i32::from(b) as u32),
    )
}

/// CODE 12 +1854..+18dc. Normal components use the original 1024 scale;
/// the two horizontal light components are supplied by the caller.
/// Preserve word narrowing and long overflow even outside normal input ranges.
pub fn face_light_shade(normal: [i16; 3], light: [i16; 2]) -> i16 {
    let x = i32::from(normal[0]) * i32::from(light[0]);
    let y = i32::from(normal[1]) * 1842;
    let z = i32::from(normal[2]) * i32::from(light[1]);
    let first = ((y.wrapping_add(x).wrapping_add(z) >> 16) as i16).max(0);
    let second = ((y.wrapping_sub(x).wrapping_sub(z) >> 16) as i16).max(0);
    first.wrapping_add(second).wrapping_add(24).min(64)
}

/// CODE 12 +1924..+1986. Bright mode takes precedence over distance fading.
/// `depth` is the signed word at offset six of the renderer's view record.
pub fn adjust_face_shade(shade: i16, bright: bool, fade: bool, depth: i16) -> i16 {
    if bright {
        ((u32::from(shade as u16) >> 1) + 32) as i16
    } else if fade {
        let factor = 4800_i16.wrapping_sub(depth).clamp(2400, 4800);
        (i32::from(shade) * i32::from(factor) / 4800) as i16
    } else {
        shade
    }
}

/// CODE 19 +3ea0 resolves a face selector before CODE 12 +708 submission.
/// The original uses a signed low-byte index into A5-2742. Reject reads
/// outside the supplied kind table instead of reading adjacent globals.
/// Encoded values >=1000 remain unchanged; their later meaning is separate.
pub fn face_color_selector(selector: i16, kind_primary: &[u8]) -> Option<i16> {
    if selector == 255 || selector == -1 {
        Some(13)
    } else if selector >= 1000 {
        Some(selector)
    } else {
        let index = usize::try_from(selector as i8).ok()?;
        kind_primary.get(index).map(|color| i16::from(*color as i8))
    }
}

/// CODE 11 +1184..+11c4 copies the first 13 RGB triples by position from a
/// selected clut. Entry value words are ignored here. Later level-color
/// substitutions and display conversion are separate.
pub fn base_palette(data: &[u8]) -> Option<[[u16; 3]; 13]> {
    let bytes = data.get(..112)?;
    Some(std::array::from_fn(|index| {
        std::array::from_fn(|component| {
            let offset = 10 + index * 8 + component * 2;
            u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
        })
    }))
}

/// CODE 11 +d9c (clut 2000) and +e1a (clut 2001). The resource handle is
/// supplied as bytes; None models a missing resource. Clut 2001 is bypassed
/// at display depth four. Malformed present resources are rejected natively.
pub fn level_palette_color(
    level: i16,
    display_depth: i16,
    use_2001: bool,
    data: Option<&[u8]>,
) -> Option<[u16; 3]> {
    let fallback = if use_2001 {
        [0xff00, 0xff00, 0]
    } else {
        [0xa900, 0, 0xa900]
    };
    if use_2001 && display_depth == 4 {
        return Some(fallback);
    }
    let Some(bytes) = data else {
        return Some(fallback);
    };
    let index = (level.wrapping_sub(1) % 30).max(0) as usize;
    let rgb = bytes.get(10 + index * 8..16 + index * 8)?;
    Some(std::array::from_fn(|component| {
        u16::from_be_bytes([rgb[component * 2], rgb[component * 2 + 1]])
    }))
}

/// CODE 11 +1184..+1224 with a present base clut. The last active entry is
/// forced black after level substitutions. Display conversion is excluded.
pub fn compose_resource_palette(
    base: &[u8],
    level: i16,
    display_depth: i16,
    active_colors: usize,
    clut_2000: Option<&[u8]>,
    clut_2001: Option<&[u8]>,
) -> Option<[[u16; 3]; 13]> {
    if !(1..=13).contains(&active_colors) {
        return None;
    }
    let mut palette = base_palette(base)?;
    palette[2] = level_palette_color(level, display_depth, true, clut_2001)?;
    palette[3] = level_palette_color(level, display_depth, false, clut_2000)?;
    palette[active_colors - 1] = [0; 3];
    Some(palette)
}

/// CODE 17 +43c..+4b0, stopping before QuickDraw InsetRect.
/// The byte at A5-7bf0 selects the full display when nonzero.
pub fn display_rectangle_setup(bounds: [i16; 4], full_size: u8) -> DisplayRectangleSetup {
    let width = i32::from(bounds[3].wrapping_sub(bounds[1]));
    let height = i32::from(bounds[2].wrapping_sub(bounds[0]));
    DisplayRectangleSetup {
        size: [width, height],
        inset: (full_size == 0)
            .then_some([((width - 512) / 2) as i16, ((height - 342) / 2) as i16]),
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ViewportMetrics {
    pub size: [i16; 2],
    pub center: [i16; 2],
    pub offset: [i16; 2],
}

/// CODE 33 +746..+7a6 and +7c2..+7e6. Bounds use QuickDraw top/left/bottom/right.
/// Offset is relative to the supplied bounds origin; later OffsetRect calls are separate.
pub fn viewport_metrics(bounds: [i16; 4], limits: [i16; 2]) -> ViewportMetrics {
    let width = bounds[3].wrapping_sub(bounds[1]);
    let height = bounds[2].wrapping_sub(bounds[0]);
    let size = [width.min(limits[0]), height.wrapping_sub(80).min(limits[1])];
    ViewportMetrics {
        size,
        center: [size[0] / 2, size[1] / 2],
        offset: [
            width.wrapping_sub(size[0]) / 2,
            height.wrapping_sub(size[1]).wrapping_add(80) / 2,
        ],
    }
}

#[derive(Debug, serde::Serialize)]
pub struct GroundCameraBasis {
    /// Internal caller order: depth (D5), then horizontal (D4).
    pub origin: [i32; 2],
    /// World-z increments, in the same depth/horizontal order.
    pub column_step: [i32; 2],
    /// World-x increments, in the same depth/horizontal order.
    pub row_step: [i32; 2],
}

/// CODE 6 +3b48..+3bc2 using CODE 11's software fixed multiplication.
/// Direction is [table[heading], table[90-heading]], stored at A6-48/A6-4c.
pub fn ground_camera_basis(
    position: [i32; 2],
    direction: [i32; 2],
    spacing: i32,
) -> GroundCameraBasis {
    let multiply = |a: i32, b: i32| ((i64::from(a) * i64::from(b)) / 65536) as i32;
    let x = position[0].wrapping_neg();
    let z = position[1].wrapping_neg();
    let column_step = [
        multiply(spacing, direction[0]),
        multiply(spacing, direction[1]),
    ];
    GroundCameraBasis {
        origin: [
            multiply(x, direction[1]).wrapping_add(multiply(z, direction[0])),
            multiply(x, direction[0])
                .wrapping_neg()
                .wrapping_add(multiply(z, direction[1])),
        ],
        column_step,
        row_step: [column_step[1], column_step[0].wrapping_neg()],
    }
}

/// CODE 6 +3b10..+3e96, through point generation and before pixel drawing.
/// Uses the recovered 1024-unit arena/64-division defaults. Direction values
/// are supplied in ground_camera_basis order; height uses the renderer global.
pub fn ground_points(
    position: [i32; 2],
    direction: [i32; 2],
    height: i32,
    center: [i16; 2],
    full_display: bool,
    table: &[[i16; 2]],
) -> Vec<[i16; 2]> {
    let (dense_rows, divisions) = ground_grid_defaults();
    let spacing = ground_grid_spacing(1024, divisions).unwrap();
    let multiply = |value: i32| ((i64::from(value) * 0x00d38000) / 65536) as i32;
    let [sine, cosine] = direction;
    let factor = if full_display { 2 } else { 1 };
    let edges = [
        [
            multiply(cosine.wrapping_sub(sine.wrapping_mul(factor))),
            multiply(cosine.wrapping_mul(factor).wrapping_add(sine)),
        ],
        [
            multiply(cosine.wrapping_add(sine.wrapping_mul(factor))),
            multiply(sine.wrapping_sub(cosine.wrapping_mul(factor))),
        ],
    ];
    let Some([first_row, last_row, first_column, last_column]) =
        ground_visible_bounds(position, edges, spacing, divisions)
    else {
        return Vec::new();
    };
    let basis = ground_camera_basis(position, direction, spacing);
    let mut start: [i32; 2] = std::array::from_fn(|axis| {
        basis.origin[axis]
            .wrapping_add(basis.row_step[axis].wrapping_mul(i32::from(first_row)))
            .wrapping_add(basis.column_step[axis].wrapping_mul(i32::from(first_column)))
    });
    let [skip, sparse_count] = ground_sparse_row(first_column, last_column);
    let mut result = Vec::new();
    for row in first_row..=last_row {
        let dense = dense_rows.get(row as usize).copied().unwrap_or(0) != 0;
        let row_start = if dense {
            start
        } else {
            std::array::from_fn(|axis| {
                start[axis].wrapping_add(basis.column_step[axis].wrapping_mul(i32::from(skip)))
            })
        };
        let step = basis
            .column_step
            .map(|value| if dense { value } else { value.wrapping_mul(8) });
        let count = if dense {
            last_column.wrapping_sub(first_column).wrapping_add(1)
        } else {
            sparse_count
        };
        result.extend(project_ground_row(
            [row_start[1], row_start[0]],
            [step[1], step[0]],
            count,
            height.wrapping_shl(3),
            center,
            table,
            full_display,
        ));
        for (value, step) in start.iter_mut().zip(basis.row_step) {
            *value = value.wrapping_add(step);
        }
    }
    result
}

/// Complete CODE 6 +3918 visible-grid bounds routine. Outputs are first/last
/// world-x index, then world-z index. None guards zero division or DIVS overflow.
pub fn ground_visible_bounds(
    origin: [i32; 2],
    edges: [[i32; 2]; 2],
    spacing: i32,
    divisions: i16,
) -> Option<[i16; 4]> {
    let divisor = (spacing as u32 >> 12) as i16;
    if divisor == 0 {
        return None;
    }
    let mut bounds = [0; 4];
    for axis in 0..2 {
        let first = origin[axis].wrapping_add(edges[0][axis]);
        let second = origin[axis].wrapping_add(edges[1][axis]);
        let third = first.wrapping_add(edges[1][axis]);
        let minimum = origin[axis].min(first).min(second).min(third);
        let maximum = origin[axis].max(first).max(second).max(third);
        let index = |coordinate: i32| {
            let numerator = i32::from((coordinate.wrapping_shl(4) >> 16) as i16);
            i16::try_from(numerator / i32::from(divisor)).ok()
        };
        bounds[axis * 2] = index(minimum)?.wrapping_sub(2).max(0);
        bounds[axis * 2 + 1] = index(maximum)?.wrapping_add(2).min(divisions);
    }
    Some(bounds)
}

/// CODE 6 +4c2..+500. Both preference branches select 64 divisions.
pub fn ground_grid_defaults() -> ([u8; 121], i16) {
    (std::array::from_fn(|index| u8::from(index % 8 == 0)), 64)
}

/// CODE 33 +348..+36a. None guards an invalid zero divisor.
pub fn ground_grid_spacing(extent: i16, divisions: i16) -> Option<i32> {
    (divisions != 0).then(|| {
        i32::from(extent)
            .wrapping_mul(65536)
            .wrapping_div(i32::from(divisions))
    })
}

/// CODE 6 +3d08..+3d86: skip from first column and number of sparse samples.
pub fn ground_sparse_row(first: i16, last: i16) -> [i16; 2] {
    let skip = 7_i16.wrapping_sub(first.wrapping_add(7) % 8);
    let count = last
        .wrapping_sub(first)
        .wrapping_add(1)
        .wrapping_sub(skip)
        .wrapping_add(7)
        / 8;
    [skip, count]
}

/// CODE 6 +442..+4c2 after allocation. Entry zero is not initialized here.
pub fn initialize_ground_projection_table(table: &mut [[i16; 2]; 1200]) {
    for (index, pair) in table.iter_mut().enumerate().skip(1) {
        let divisor = index as i32;
        pair[0] = ((if index < 32 { 32768 } else { 1048576 }) / divisor) as i16;
        pair[1] = (4096 / divisor) as i16;
    }
}

/// Single-point CODE 11 +3ce6 ground projection. Table pairs hold the signed
/// reciprocal coefficient and the precomputed offset for height parameter 256.
/// Input height is the renderer height global times eight, not scene units.
pub fn project_ground_point(
    point: [i32; 2],
    height: i32,
    center: [i16; 2],
    table: &[[i16; 2]],
) -> Option<[i16; 2]> {
    project_ground_point_for_display(point, height, center, table, false)
}

/// CODE 11 +3ce6 (compact) and +3de6 (full display) single-point arithmetic.
/// Full display doubles the lateral culling limit, not the projected offsets.
pub fn project_ground_point_for_display(
    point: [i32; 2],
    height: i32,
    center: [i16; 2],
    table: &[[i16; 2]],
    full_display: bool,
) -> Option<[i16; 2]> {
    let [horizontal, depth] = point;
    let magnitude = horizontal.wrapping_abs();
    let lateral_limit = if full_display {
        depth.wrapping_mul(2)
    } else {
        depth
    };
    if depth < 8 << 16 || magnitude > lateral_limit || magnitude.wrapping_add(depth) >= 300 << 16 {
        return None;
    }
    let offset = ((depth.wrapping_shl(2) >> 16) as i16).wrapping_mul(4);
    if offset < 0 {
        return None;
    }
    let pair = table.get(offset as usize / 4)?;
    let lateral = (horizontal >> 10).wrapping_neg() as i16;
    let x = ((i32::from(lateral) * i32::from(pair[0])) >> 16) as i16;
    let y = if height == 256 {
        pair[1]
    } else {
        ((i32::from(height as i16) * i32::from(pair[0])) >> 16) as i16
    };
    Some([x.wrapping_add(center[0]), y.wrapping_add(center[1])])
}

/// Complete ground-row iteration from CODE 11 +3ce6/+3de6.
/// Appends accepted points in traversal order. Native storage replaces the
/// original caller-owned buffer; unavailable table entries are rejected.
pub fn project_ground_row(
    start: [i32; 2],
    step: [i32; 2],
    count: i16,
    height: i32,
    center: [i16; 2],
    table: &[[i16; 2]],
    full_display: bool,
) -> Vec<[i16; 2]> {
    let mut result = Vec::new();
    let mut point = start;
    for _ in 0..count {
        if let Some(projected) =
            project_ground_point_for_display(point, height, center, table, full_display)
        {
            result.push(projected);
        }
        point[0] = point[0].wrapping_add(step[0]);
        point[1] = point[1].wrapping_add(step[1]);
    }
    result
}

/// The ground projector scales planar scene units by 64 and the camera-height
/// global by 8. Therefore one global height unit is one eighth of a scene unit.
pub fn camera_height_in_scene(height: i32) -> f32 {
    height as f32 / 8.
}

#[derive(Debug, serde::Serialize)]
pub struct DeathCamera {
    pub pose: [i32; 3],
    pub height: i32,
}

/// CODE 6 +50c0..+5196, active death-camera branch. Direction lookup is supplied.
/// None guards a zero duration; the original routine assumes nonzero duration.
pub fn death_camera(
    state: &[u8; 30],
    now: i32,
    base_height: i16,
    mut direction: impl FnMut(i32) -> [i32; 2],
) -> Option<DeathCamera> {
    let long = |o| i32::from_be_bytes(state[o..o + 4].try_into().unwrap());
    let word = |o| i16::from_be_bytes(state[o..o + 2].try_into().unwrap());
    let duration = long(4);
    if duration == 0 {
        return None;
    }
    let elapsed = now.wrapping_sub(long(0));
    let angle = wrap_angle(
        elapsed
            .wrapping_mul(long(8))
            .wrapping_div(duration)
            .wrapping_mul(65536)
            .wrapping_add(long(24)),
    );
    let distance = elapsed
        .wrapping_mul(i32::from(word(14)))
        .wrapping_div(duration);
    let rise = elapsed
        .wrapping_mul(i32::from(word(12)))
        .wrapping_div(duration) as i16;
    let direction = direction(angle);
    Some(DeathCamera {
        pose: [
            long(16).wrapping_add(direction[0].wrapping_mul(distance)),
            long(20).wrapping_add(direction[1].wrapping_mul(distance)),
            wrap_angle(angle.wrapping_add(180 << 16)),
        ],
        height: i32::from(rise.wrapping_add(base_height)),
    })
}

/// Complete CODE 20 +2a30. Cache bytes represent A5-3b68 through A5-3b59.
pub fn reset_ai_participant(record: &mut [u8; 256], participant: i8, cache: &mut [u8; 16]) {
    record[9] = 3;
    record[0x10] = 3;
    for offset in [0x44, 0x40, 0x4e] {
        record[offset..offset + 4].copy_from_slice(&(-1_i32).to_be_bytes());
    }
    record[0x4a..0x4e].fill(0);
    record[0x30] = 1;
    record[0x48..0x4a].fill(0);
    record[0x52..0x54].fill(0);
    if participant == 1 {
        cache.fill(255);
    }
}

/// CODE 31 +f3a..+f82: reset notifications after the local participant dies.
pub fn death_ai_reset_targets(active: &[u8; 12], enemy: &[u8; 12], mode_12d2: u8) -> Vec<i8> {
    (0..12)
        .filter(|&i| active[i] != 0 && (enemy[i] != 0 || (active[i] & mode_12d2) != 0))
        .map(|i| (i + 1) as i8)
        .collect()
}

#[derive(Debug, serde::Serialize)]
pub struct DamageSounds {
    pub requests: Vec<(i16, u32)>,
    pub flush: bool,
}

/// Sound branches in CODE 31 +eee..+f3a and +f84..+fc4.
/// AI notifications and other damage-handler side effects are separate.
pub fn damage_sounds(local_victim: bool, killed: u8, lives: i16) -> DamageSounds {
    if local_victim && killed != 0 {
        DamageSounds {
            requests: vec![(1, 0x1fc1c), (1, if lives > 1 { 0x10005 } else { 0x1000d })],
            flush: lives <= 1,
        }
    } else {
        DamageSounds {
            requests: vec![(
                3,
                if local_victim {
                    0x40003
                } else if killed != 0 {
                    0x40005
                } else {
                    0x40004
                },
            )],
            flush: false,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct DamageResponseInput {
    pub now: i32,
    pub local: i8,
    pub victim: i8,
    pub attacker: i8,
    pub killed: u8,
    pub game_mode: u8,
}

/// CODE 31 +0xde2..+0xe7c, after health/counter/score handling.
/// Returns the multiplayer death notification arguments, without executing that
/// external service. Its original call precedes the kind transition.
pub fn damage_response_fields(
    record: &mut [u8; 128],
    participant: &mut [u8; 256],
    input: DamageResponseInput,
) -> Option<(i8, i8)> {
    let delay = if participant[5] != 0 {
        60
    } else if input.victim == input.local {
        260
    } else {
        66
    };
    record[0x16..0x1a].copy_from_slice(&input.now.wrapping_add(delay).to_be_bytes());
    let notification = if input.killed != 0 {
        record[0x15] = record[0x14];
        record[0x14] = 10;
        (input.game_mode != 0).then_some((input.attacker, input.victim))
    } else {
        None
    };
    participant[0x54] = 1;
    notification
}

/// CODE 30 +294a..+29a6: immediate grenade/bomb blast on a living enemy hit.
/// Caller must first establish a selected participant collision and positive health.
/// Timer expiry uses projectile_lifecycle_requests instead and clears ownership.
pub fn projectile_impact_uses_area(
    weapon: i16,
    victim_enemy: u8,
    game_mode: u8,
    authority: u8,
) -> bool {
    victim_enemy != 0 && matches!(weapon, 1 | 17) && (game_mode == 0 || authority & 1 != 0)
}

/// Damage-call arguments emitted by CODE 30 +0x2aa6: damage, mode, time,
/// attacker, victim. The downstream damage handler remains the caller's job.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct AreaDamageRequest {
    pub damage: i16,
    pub mode: i16,
    pub now: i32,
    pub attacker: i8,
    pub victim: i8,
}

/// Complete selection and scaling for CODE 30 +0x2aa6.
/// Invalid referenced geometry/table indices return None without side effects.
pub fn projectile_area_requests(
    records: &[[u8; 128]],
    participants: &[[u8; 256]; 12],
    source_id: i16,
    now: i32,
    enemy_damage: &[i16; 18],
) -> Option<Vec<AreaDamageRequest>> {
    let long = |record: &[u8; 128], i| i32::from_be_bytes(record[i..i + 4].try_into().unwrap());
    let source = records.get(usize::try_from(source_id.checked_sub(1)?).ok()?)?;
    let mut requests = Vec::new();
    for (index, participant) in participants.iter().enumerate() {
        if participant[0x88] & participant[5] == 0 {
            continue;
        }
        let id = i16::from_be_bytes([participant[0x56], participant[0x57]]);
        let target = records.get(usize::try_from(id.checked_sub(1)?).ok()?)?;
        if target[0x14] == 10 {
            continue;
        }
        let planar = ai_planar_distance(
            [long(source, 0), long(source, 4)],
            [long(target, 0), long(target, 4)],
        );
        let vertical = (long(target, 0x28).wrapping_sub(long(source, 0x28)) / 65536)
            .wrapping_abs()
            .wrapping_mul(2);
        let distance = planar.wrapping_add(vertical);
        if distance >= 240 << 16 {
            continue;
        }
        // The original indexes with the TARGET geometry's signed low weapon byte.
        let weapon = usize::try_from(target[0x1d] as i8).ok()?;
        let base = *enemy_damage.get(weapon)?;
        let damage = if distance < 60 << 16 {
            base.wrapping_mul(2)
        } else if distance < 120 << 16 {
            base.wrapping_mul(3) / 2
        } else {
            base
        };
        requests.push(AreaDamageRequest {
            damage,
            mode: 0,
            now,
            attacker: source[0x1a] as i8,
            victim: (index + 1) as i8,
        });
    }
    Some(requests)
}

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub enum ProjectileLifecycleAction {
    None,
    Move,
    Delete,
    Retire,
    AreaEffectThenDelete,
}

/// One record's CODE 30 +0x2bbe expiry dispatch. Movement, deletion, and area
/// effects are returned to the caller; retirement writes execute here.
pub fn projectile_lifecycle(
    record: &mut [u8; 128],
    now: i32,
    owner_shots: &mut [i16; 12],
) -> Option<ProjectileLifecycleAction> {
    use ProjectileLifecycleAction::*;
    let deadline = i32::from_be_bytes(record[0x16..0x1a].try_into().unwrap());
    let weapon = i16::from_be_bytes(record[0x1c..0x1e].try_into().unwrap());
    let special = matches!(weapon, 1 | 17);
    if matches!(record[0x14], 9 | 28 | 29 | 32 | 33 | 34 | 43 | 44 | 46) {
        if deadline > now {
            return Some(Move);
        }
        if !special {
            return Some(Delete);
        }
        let owner = record[0x1a] as i8;
        if owner != 0 && !(1..=12).contains(&owner) {
            return Option::None;
        }
        record[0x15] = record[0x14];
        record[0x14] = 10;
        if owner != 0 {
            let shots = &mut owner_shots[owner as usize - 1];
            *shots = shots.wrapping_sub(1);
        }
        record[0x1a] = 0;
        record[0x16..0x1a].copy_from_slice(&now.wrapping_add(60).to_be_bytes());
        Some(Retire)
    } else if record[0x14] == 10 && record[0x1a] == 0 && deadline.wrapping_sub(now) < 57 {
        record[0x14] = 9;
        Some(if special {
            AreaEffectThenDelete
        } else {
            Delete
        })
    } else {
        Some(None)
    }
}

/// CODE 30 +2788..+2814, including the complete +261e boundary predicate.
/// Called after movement and collision selection for projectile kinds.
/// A positive selected hit takes priority over boundaries and ground contact.
pub fn projectile_environment_contact(
    record: &[u8; 128],
    arena_extent: i32,
    selected_hit: i16,
) -> bool {
    let word = |offset| i32::from_be_bytes(record[offset..offset + 4].try_into().unwrap());
    let outside = [word(0), word(4)]
        .into_iter()
        .any(|position| position < 0 || position > arena_extent);
    let grounded = match record[0x14] {
        34 | 46 => word(0x28) <= 0,
        43 | 44 => word(0x24) <= 0,
        _ => false,
    };
    selected_hit <= 0 && (outside || grounded)
}

/// The next original service selected after a projectile's collision query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ProjectileObjectContact {
    Ignore,
    DamageParticipant,
    Delete,
    RespondToObject,
}

/// CODE 30 +2814..+28ee: selected-object classification and health gate.
/// `target_health` is the participant health associated with the target record.
/// This selects a branch; it does not execute damage or the object response.
pub fn projectile_object_contact(
    projectile_kind: u8,
    selected_hit: i16,
    target_kind: u8,
    target_health: i16,
) -> ProjectileObjectContact {
    use ProjectileObjectContact::*;
    if selected_hit <= 0
        || matches!(
            target_kind,
            5 | 10 | 12 | 14 | 16 | 17 | 18 | 19 | 23 | 24 | 30 | 36 | 37 | 39 | 40 | 41 | 42
        )
    {
        return Ignore;
    }
    let participant = matches!(
        target_kind,
        6 | 7 | 8 | 13 | 20 | 21 | 22 | 26 | 27 | 31 | 35 | 45
    ) || (projectile_kind == 28 && matches!(target_kind, 23..=25));
    if !participant {
        RespondToObject
    } else if target_health <= 0 {
        Delete
    } else {
        DamageParticipant
    }
}

/// CODE 25 +2d04..+2d26: derive contact retirement from SysEnvRec.machineType.
/// CODE 3 +0818 supplies the record at A5-7b96 to the SysEnvirons wrapper.
/// The 512Ke, Plus, SE, and Portable delete immediately instead.
pub fn projectile_contact_retirement_enabled(machine_type: i16) -> bool {
    !matches!(machine_type, 1 | 2 | 3 | 8)
}

/// CODE 30 +2676: delete immediately or rewind and retire after contact.
/// The movement callback executes before the retirement fields change.
pub fn retire_projectile_contact(
    record: &mut [u8; 128],
    elapsed: i32,
    now: i32,
    enabled: u8,
    owner_shots: &mut [i16; 12],
    mut movement: impl FnMut(&mut [u8; 128], i32),
) -> Option<ProjectileLifecycleAction> {
    if enabled == 0 {
        return Some(ProjectileLifecycleAction::Delete);
    }
    let owner = record[0x1a] as i8;
    if owner != 0 && !(1..=12).contains(&owner) {
        return None;
    }
    let rewind = (elapsed as i16).min(2).wrapping_neg() as i32;
    movement(record, rewind);
    record[0x16..0x1a].copy_from_slice(&now.wrapping_add(60).to_be_bytes());
    record[0x15] = record[0x14];
    record[0x14] = 10;
    if owner != 0 {
        owner_shots[owner as usize - 1] = owner_shots[owner as usize - 1].wrapping_sub(1);
    }
    record[0x1a] = 0;
    Some(ProjectileLifecycleAction::Retire)
}

/// Compose one projectile's retirement with the original area-request routine.
/// Movement and deletion remain caller actions; returned damage requests precede deletion.
pub fn projectile_lifecycle_requests(
    records: &mut [[u8; 128]],
    participants: &[[u8; 256]; 12],
    source_id: i16,
    now: i32,
    owner_shots: &mut [i16; 12],
    enemy_damage: &[i16; 18],
) -> Option<(ProjectileLifecycleAction, Vec<AreaDamageRequest>)> {
    let source = usize::try_from(source_id.checked_sub(1)?).ok()?;
    let action = projectile_lifecycle(records.get_mut(source)?, now, owner_shots)?;
    let requests = if action == ProjectileLifecycleAction::AreaEffectThenDelete {
        projectile_area_requests(records, participants, source_id, now, enemy_damage)?
    } else {
        Vec::new()
    };
    Some((action, requests))
}

/// Complete type-29 route through CODE 30 +0x1d54, including direction lookup
/// and the common grid-movement tail. Table generation is the caller's concern.
/// Invalid native inputs or a full destination return None without mutation.
pub fn step_bouncing_projectile(
    record: &mut [u8; 128],
    id: i16,
    elapsed: i32,
    direction_table: &[i32; 360],
    grid: &mut [Vec<i16>; 256],
) -> Option<()> {
    if id <= 0 || record[0x14] != 29 {
        return None;
    }
    if elapsed == 0 {
        return Some(());
    }
    let heading = i32::from_be_bytes(record[8..12].try_into().unwrap());
    let direction = movement_direction(heading, direction_table)?;
    let position =
        |r: &[u8; 128]| [0, 4].map(|i| i32::from_be_bytes(r[i..i + 4].try_into().unwrap()));
    let mut next = *record;
    move_bouncing_projectile(&mut next, direction, elapsed);
    move_grid_object(grid, id, position(record), position(&next))?;
    *record = next;
    Some(())
}

/// CODE 6 +2a10..+2a24: fixed vertical position to signed eighth-unit render word.
/// Division truncates toward zero, including negative fractional heights.
pub fn render_height_word(height: i32) -> i16 {
    (height / 65536) as i16
}

/// CODE 30 +0x1de0..+0x1e60, type-29 motion after direction lookup.
/// The outer dispatcher excludes elapsed zero. Grid and collision updates follow.
pub fn move_bouncing_projectile(record: &mut [u8; 128], direction: [i32; 2], elapsed: i32) {
    fn get(record: &[u8; 128], offset: usize) -> i32 {
        i32::from_be_bytes(record[offset..offset + 4].try_into().unwrap())
    }
    fn put(record: &mut [u8; 128], offset: usize, value: i32) {
        record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    for (offset, component) in [0, 4].into_iter().zip(direction) {
        put(
            record,
            offset,
            get(record, offset).wrapping_add(component.wrapping_mul(elapsed) / 8),
        );
    }
    let height = get(record, 0x28).wrapping_add(get(record, 0x24).wrapping_mul(elapsed) / 2);
    let velocity = get(record, 0x24).wrapping_sub((1_i32 << 16).wrapping_mul(elapsed));
    if height > 0 {
        put(record, 0x28, height);
        put(record, 0x24, velocity);
    } else {
        put(record, 0x28, 0);
        put(record, 0x24, velocity.wrapping_abs() / 2);
    }
}

/// CODE 30 +0x7e8..+0x836, excluding the kind/model load service at +0x7fc.
/// Sound requests and spatial-grid insertion follow this block.
pub fn finalize_weapon_record(
    record: &mut [u8; 128],
    weapon: i8,
    now: i32,
    owner: i8,
    owner_shots: &mut i16,
) {
    let stored_weapon = if weapon == 5 { 0 } else { weapon as i16 };
    record[0x1c..0x1e].copy_from_slice(&stored_weapon.to_be_bytes());
    record[0x10..0x14].copy_from_slice(&now.to_be_bytes());
    record[0x1a] = owner as u8;
    record[0x1b] = 1;
    record[0x5b] = 0;
    record[0xe..0x10].copy_from_slice(&1_i16.to_be_bytes());
    record[0xc..0xe].copy_from_slice(&0_i16.to_be_bytes());
    if owner != 0 {
        *owner_shots = owner_shots.wrapping_add(1);
    }
}

/// CODE 30 +0x408..+0x7e8, after heading preparation and direction lookup.
/// Mutates only the original weapon-specific fields; allocation and common
/// ownership/grid finalization occur outside this block.
pub fn initialize_weapon_record(
    record: &mut [u8; 128],
    input: WeaponRecordInput,
    mut random: impl FnMut() -> i16,
) {
    fn get(r: &[u8; 128], offset: usize) -> i32 {
        i32::from_be_bytes(r[offset..offset + 4].try_into().unwrap())
    }
    fn put(r: &mut [u8; 128], offset: usize, value: i32) {
        r[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    let lifetime = match input.weapon {
        1 | 17 => (input.lifetime / 4).wrapping_mul(3),
        2 => input.lifetime.wrapping_mul(50),
        6 | 10 | 15 | 16 => input.lifetime.wrapping_mul(4),
        7 => input.lifetime.wrapping_mul(24),
        11 => input.lifetime.wrapping_mul(8),
        _ => input.lifetime,
    };
    put(record, 0x16, input.now.wrapping_add(lifetime as i32));
    if input.weapon != 7 {
        let scale = if matches!(input.weapon, 6 | 15 | 16) {
            3
        } else {
            6
        } * if input.owner == 0 { 3 } else { 1 };
        for (axis, direction) in input.direction.iter().enumerate() {
            put(
                record,
                axis * 4,
                get(record, axis * 4).wrapping_add(direction.wrapping_mul(scale)),
            );
        }
    }
    record[0x14] = match input.weapon {
        17 => 46,
        2 => 28,
        6 => 29,
        7 => 32,
        10 => 34,
        15 => 43,
        16 => 44,
        11 => 33,
        _ => 9,
    };
    match input.weapon {
        17 => put(record, 0x24, -65536),
        6 | 15 | 16 => {
            put(record, 0x24, 30 << 16);
            let rise = if input.weapon == 6 {
                random().wrapping_abs() % 5 + 5
            } else {
                5
            };
            put(
                record,
                0x28,
                get(record, 0x28).wrapping_add((rise as i32) << 16),
            );
            record[0x15] = input.source_kind;
        }
        7 => {
            record[0x30..0x32].copy_from_slice(&0_i16.to_be_bytes());
            record[0x32..0x34].copy_from_slice(&input.owner_field_5c.to_be_bytes());
        }
        10 => {
            put(record, 0x24, 15 << 16);
            put(
                record,
                0x28,
                get(record, 0x28).wrapping_add(20 << 16).max(1 << 16),
            );
        }
        11 => {
            put(record, 0x2c, get(record, 8));
            put(record, 0x1e, 0);
        }
        _ => {}
    }
}

/// CODE 25 +0x1ff6..+0x20dc, including CODE 23 +0x0078 setting validation.
/// One upgrade at multiples of eight above level ten, in priority order.
pub fn campaign_upgrade(level: i16, mut settings: [u8; 7]) -> [u8; 7] {
    if level <= 10 || level % 8 != 0 {
        return settings;
    }
    let shield = settings[1] as i8 as i16 + 1;
    let ammo = settings[2] as i8 as i16 + 1;
    let speed = settings[0] as i8 as i16 + 1;
    if (1..=10).contains(&shield) {
        settings[1] = shield as u8;
    } else if (0..=5).contains(&ammo) {
        settings[2] = ammo as u8;
    } else if (1..=8).contains(&speed) {
        settings[0] = speed.min(10) as u8;
    }
    settings
}

/// CODE 25 +0x23b6..+0x2406, using CODE 23 +0x0684/+0x06fc for a local tank.
pub fn wave_refill(shields: i8, ammo_setting: i8) -> (i16, i16) {
    ((shields as i16).min(9), ammo_setting as i16 * 10)
}

/// CODE 1 +0x0846..+0x086c: initial effective difficulty before uptime updates.
pub fn initial_effective_difficulty(difficulty: i16, level: i16) -> i16 {
    difficulty.wrapping_add(level).clamp(0, 127)
}

/// CODE 25 +0x1a3a and +0x1a78: count consecutive resources until a gap.
/// Signed word wrapping and the -1 ID sentinel match the original loops.
pub fn resource_prefix_count(mut exists: impl FnMut(i16) -> bool) -> i16 {
    let mut id = 10000_i16;
    let mut count = 0_i16;
    while id != -1 && exists(id) {
        count = count.wrapping_add(1);
        id = id.wrapping_add(1);
    }
    count
}

#[derive(Debug, serde::Serialize)]
pub struct WaveResources {
    pub world: i16,
    pub enemies: i16,
    pub random_used: usize,
}

/// CODE 25 +0x1cd4..+0x1d46. Counts describe contiguous IDs starting at 10000.
/// Random samples are consumed only for zero IDs with a positive count.
/// The -32768 absolute-value overflow and signed remainder are intentional.
pub fn resolve_wave_resources(
    world: i16,
    enemies: i16,
    counts: [i16; 2],
    alternate_world: u8,
    random: &[i16],
) -> Option<WaveResources> {
    let mut random_used = 0;
    let mut select = |id: i16, count: i16| -> Option<i16> {
        if id != 0 {
            return Some(id);
        }
        if count <= 0 {
            return Some(10000);
        }
        let sample = *random.get(random_used)?;
        random_used += 1;
        Some(10000_i16.wrapping_add(sample.wrapping_abs() % count))
    };
    let mut world = select(world, counts[0])?;
    if alternate_world != 0 {
        world = world.wrapping_add(1000);
    }
    let enemies = select(enemies, counts[1])?;
    Some(WaveResources {
        world,
        enemies,
        random_used,
    })
}

/// CODE 25 +0x1ab6: choose the wave resource from difficulty and mode bytes.
pub fn wave_resource_id(difficulty: i16, mode_12d2: u8, mode_12ca: u8) -> i16 {
    if mode_12d2 != 0 {
        778
    } else if mode_12ca != 0 {
        777
    } else if difficulty < 3 {
        10000
    } else if difficulty <= 4 {
        10001
    } else {
        10002
    }
}

#[derive(Debug, serde::Serialize)]
pub struct WaveSelection {
    pub index: usize,
    pub health_modifier: i16,
    pub wave: crate::formats::Wave,
}

/// CODE 25 +0x1c32..+0x1cd4: one-based levels wrap through the wave list.
/// Zero world/enemy IDs still require the original random-resource selection.
/// Invalid levels/counts are host guards against original out-of-bounds access.
pub fn select_wave(rows: &[crate::formats::Wave], level: i16) -> Option<WaveSelection> {
    if level <= 0 || rows.is_empty() || rows.len() > i16::MAX as usize {
        return None;
    }
    let index = (level as usize - 1) % rows.len();
    let health_modifier = ((level as usize - 1) / rows.len()) as i16;
    let mut wave = rows[index].clone();
    if health_modifier > 0 {
        wave.hint = 0;
        wave.start_movie = 0;
    }
    Some(WaveSelection {
        index,
        health_modifier,
        wave,
    })
}

/// CODE 25 +0x16b8..+0x18ea: apply rows in order to initialized type slots.
/// Type 5 controls separate globals and does not overwrite a parameter slot.
/// The health bonus flag represents the original +50 branch, not a difficulty name.
pub fn enemy_parameters(
    rows: &[crate::formats::Beastie],
    health_modifier: i16,
    extra_health: bool,
) -> [crate::formats::Beastie; 47] {
    let mut slots = std::array::from_fn(|kind| default_enemy_parameters(kind as i16));
    for row in rows {
        let kind = row.kind as i8;
        if !(0..47).contains(&kind) || kind == 5 {
            continue;
        }
        let mut value = row.clone();
        value.kind = kind as i16;
        value.hits_to_kill = value
            .hits_to_kill
            .wrapping_add(health_modifier)
            .wrapping_add(if extra_health { 50 } else { 0 });
        slots[kind as usize] = value;
    }
    slots
}

/// CODE 25 +0x14f6..+0x1604 initializes every type before bval overrides.
/// These fields remain unchanged when the resource omits a type.
pub fn default_enemy_parameters(kind: i16) -> crate::formats::Beastie {
    crate::formats::Beastie {
        kind,
        hits_to_kill: 1,
        collision_damage: 3,
        attack_range: 250,
        short_firing_range: 80,
        firing_arc: 15,
        firing_interval: 120,
        radial_velocity: 5,
        angular_velocity: 1,
        score: 10,
        bonus: 100,
        weapon: 0,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missile_planar_step_preserves_integer_order_and_rejects_invalid_heading() {
        let mut table = [0; 360];
        table[0] = -393217;
        table[90] = 393217;
        let mut state = MissileGuidance {
            pose: [i32::MAX, i32::MIN, 0],
            height: 123,
            vertical_velocity: -65536,
        };
        move_smart_missile(&mut state, 3, &table).unwrap();
        let step = 393217_i32.wrapping_mul(2).wrapping_mul(3) / 5;
        assert_eq!(
            state.pose,
            [i32::MAX.wrapping_add(step), i32::MIN.wrapping_sub(step), 0]
        );
        assert_eq!((state.height, state.vertical_velocity), (123, -65536));
        state.pose[2] = 360 << 16;
        let original = state;
        assert_eq!(move_smart_missile(&mut state, 3, &table), None);
        assert_eq!(state, original);
    }

    #[test]
    fn missile_finishes_ascent_before_tracking() {
        let dormant = AiTargetCandidate {
            team: 0,
            owner: 0,
            active: 0,
            health: 0,
            kind: 0,
            pose: [0; 3],
        };
        let mut missile = MissileGuidance {
            pose: [0; 3],
            height: 100 << 16,
            vertical_velocity: 15 << 16,
        };
        let candidates = [dormant; 12];
        let heights = [0; 12];
        smart_missile_guidance(
            &mut missile,
            1,
            1,
            &candidates,
            &heights,
            |_, _| unreachable!(),
        );
        assert_eq!(missile.height, 106 << 16);
        assert_eq!(missile.vertical_velocity, 15 << 16);
        smart_missile_guidance(
            &mut missile,
            1,
            1,
            &candidates,
            &heights,
            |_, _| unreachable!(),
        );
        assert_eq!(missile.height, 106 << 16);
        assert_eq!(missile.vertical_velocity, -2 << 16);
        smart_missile_guidance(
            &mut missile,
            1,
            1,
            &candidates,
            &heights,
            |_, _| unreachable!(),
        );
        assert_eq!(missile.height, (106 << 16) - (4 << 16) / 5);
        assert_eq!(smart_missile_elapsed(2, Some(6)), 2);
        assert_eq!(smart_missile_elapsed(3, Some(6)), 4);
        assert_eq!(smart_missile_elapsed(63, Some(6)), 84);
        assert_eq!(smart_missile_elapsed(63, None), 63);
    }

    #[test]
    fn seeker_reacquires_without_team_filter_and_truncates_turns() {
        let dormant = AiTargetCandidate {
            team: 2,
            owner: 0,
            active: 0,
            health: 1,
            kind: 7,
            pose: [0; 3],
        };
        let mut participants = [dormant; 12];
        let projectile = [0, 0, 359 << 16];
        assert_eq!(
            seeker_heading(1, projectile, &participants, |_, _| unreachable!()),
            projectile[2]
        );
        participants[0].active = 255;
        participants[1].active = 2;
        participants[1].pose = [100 << 16, 0, 0];
        assert_eq!(
            seeker_select_target(1, 0, projectile, &participants, |_, _| 8),
            2
        );
        assert_eq!(
            seeker_heading(1, projectile, &participants, |_, _| 8),
            1 << 16
        );
        assert_eq!(
            seeker_heading(1, projectile, &participants, |_, _| -8),
            357 << 16
        );
        assert_eq!(
            seeker_heading(1, projectile, &participants, |_, _| 2),
            projectile[2]
        );
        participants[2] = participants[1];
        assert_eq!(
            seeker_select_target(1, 0, projectile, &participants, |_, _| 8),
            2
        );
        participants[1].health = 0;
        assert_eq!(
            seeker_select_target(1, 0, projectile, &participants, |_, _| 8),
            3
        );
    }

    #[test]
    fn prediction_guards_original_faults_without_changing_copy_mode() {
        let table = candidate_unit_table();
        for speed in [-24, 24] {
            assert_eq!(
                ai_predict_pose([0; 3], [100 << 16, 0, 0], speed, 1, &table),
                None
            );
        }
        let invalid = [1, 2, -65536];
        assert_eq!(ai_predict_pose([0; 3], invalid, 5, 1, &table), None);
        assert_eq!(
            ai_predict_pose([0; 3], invalid, 24, 0, &table),
            Some(invalid)
        );
        assert_eq!(
            ai_predict_pose([0; 3], [1, 2, 360 << 16], 5, 1, &table),
            None
        );
    }
    #[test]
    fn pixel_patterns_guard_color_table_reads() {
        assert_eq!(face_pixel_pattern_8(32, 128, &[0; 13]), None);
        assert_eq!(face_pixel_pattern_8(32, 13, &[0; 13]), None);
        assert_eq!(face_pixel_pattern_8(32, 12, &[]), Some(u32::MAX));
        assert_eq!(face_pixel_pattern_8(0, 128, &[]), Some(u32::MAX));
        assert_eq!(shade_pixel_index(1, 128, 8, false, &[0; 13]), None);
    }

    #[test]
    fn face_selector_rejects_adjacent_global_reads() {
        assert_eq!(face_color_selector(-128, &[0; 47]), None);
        assert_eq!(face_color_selector(47, &[0; 47]), None);
        assert_eq!(face_color_selector(255, &[]), Some(13));
        assert_eq!(face_color_selector(-1, &[]), Some(13));
        assert_eq!(face_color_selector(1000, &[]), Some(1000));
        assert_eq!(face_color_selector(256, &[255]), Some(-1));
    }

    #[test]
    fn ground_projection_uses_eighth_unit_camera_height() {
        let table = vec![[1024, 4]; 1200];
        // Equal ten-unit lateral and vertical offsets project equally. The
        // original camera height is 80, and the ground caller passes 80 * 8.
        assert_eq!(
            project_ground_point([-10 << 16, 100 << 16], 80 * 8, [0, 0], &table),
            Some([10, 10])
        );
        assert_eq!(camera_height_in_scene(80), 10.);
        assert_eq!(camera_height_in_scene(32), 4.);
    }

    #[test]
    fn death_response_deadline_admits_transition_once() {
        for (enemy, victim, delay) in [(1, 2, 60), (0, 1, 260), (0, 2, 66)] {
            let mut record = [0; 128];
            record[0x14] = 7;
            let mut participant = [0; 256];
            participant[5] = enemy;
            participant[8] = 7;
            damage_response_fields(
                &mut record,
                &mut participant,
                DamageResponseInput {
                    now: 100,
                    local: 1,
                    victim,
                    attacker: 1,
                    killed: 1,
                    game_mode: 0,
                },
            );
            let mut input = ParticipantTransitionInput {
                now: 100 + delay - 1,
                deadline: i32::from_be_bytes(record[0x16..0x1a].try_into().unwrap()),
                kind: record[0x14],
                game_mode: 0,
                local: victim == 1,
                authority: 0,
                enemy,
                active: 1,
                changed: participant[0x54],
            };
            assert_eq!(participant_transition_ready(input), 0);
            input.now += 1;
            assert_eq!(participant_transition_ready(input), 1);
            begin_participant_transition(&mut participant, &mut record);
            input.kind = record[0x14];
            input.deadline = i32::from_be_bytes(record[0x16..0x1a].try_into().unwrap());
            input.changed = participant[0x54];
            assert_eq!(participant_transition_ready(input), 0);
            input.now = i32::MAX;
            assert_eq!(participant_transition_ready(input), 0);
        }
    }

    #[test]
    fn death_camera_rejects_zero_duration_before_direction_lookup() {
        assert!(
            death_camera(&[0; 30], 0, 4, |_| panic!(
                "zero duration must not query direction"
            ))
            .is_none()
        );
    }

    #[test]
    fn projectile_retirement_rejects_invalid_owner_without_mutation() {
        let mut record = [0; 128];
        record[0x14] = 29;
        record[0x1a] = 127;
        record[0x1c..0x1e].copy_from_slice(&1_i16.to_be_bytes());
        let before = record;
        let mut counts = [20; 12];
        assert_eq!(projectile_lifecycle(&mut record, 0, &mut counts), None);
        assert_eq!(record, before);
        assert_eq!(counts, [20; 12]);
    }

    #[test]
    fn bouncing_step_guards_full_cells_without_partial_motion() {
        let mut record = [0; 128];
        record[0x14] = 29;
        record[0..4].copy_from_slice(&(63_i32 << 16).to_be_bytes());
        record[0x28..0x2c].copy_from_slice(&(1_i32 << 16).to_be_bytes());
        let before = record;
        let mut table = [0; 360];
        table[90] = 8 << 16;
        let mut grid: [Vec<i16>; 256] = std::array::from_fn(|_| vec![]);
        grid[0] = vec![1];
        grid[16] = vec![2; 20];
        let before_grid = grid.clone();
        assert_eq!(
            step_bouncing_projectile(&mut record, 1, 2, &table, &mut grid),
            None
        );
        assert_eq!(record, before);
        assert_eq!(grid, before_grid);
        record[8..12].copy_from_slice(&(-1_i32).to_be_bytes());
        let invalid_heading = record;
        assert_eq!(
            step_bouncing_projectile(&mut record, 1, 1, &table, &mut grid),
            None
        );
        assert_eq!(record, invalid_heading);
        assert_eq!(grid, before_grid);
        assert_eq!(
            step_bouncing_projectile(&mut record, 1, 0, &table, &mut grid),
            Some(())
        );
        assert_eq!(record, invalid_heading);
        assert_eq!(grid, before_grid);
    }

    #[test]
    fn removal_rejects_invalid_references_before_unlinking_or_decrementing() {
        for (kind, owner, moved_owner, entries) in
            [(7, 0, 1, 1), (9, 127, 1, 1), (9, 1, 127, 1), (9, 1, 1, 21)]
        {
            let mut records = [[0u8; 128]; 2];
            records[0][0x14] = kind;
            records[0][0x1a] = owner;
            records[1][0x14] = 7;
            records[1][0x1a] = moved_owner;
            let before_records = records;
            let mut participants = GeometryParticipants {
                ids: [2; 12],
                shots: [5; 12],
                active: [1; 12],
                status: [255; 12],
            };
            let before_participants = participants.clone();
            let mut grid: [Vec<i16>; 256] = std::array::from_fn(|_| vec![]);
            grid[0] = vec![1; entries];
            let before_grid = grid.clone();
            let mut count = 2;
            assert_eq!(
                remove_geometry(&mut records, &mut count, 1, &mut participants, &mut grid),
                None
            );
            assert_eq!(records, before_records);
            assert_eq!(participants, before_participants);
            assert_eq!(grid, before_grid);
            assert_eq!(count, 2);
            let mut raw = [[0xa5; 256]; 12];
            let mut status = [0x5a; 12];
            let mut raw_count = 2;
            assert_eq!(
                delete_object_record(
                    &mut records,
                    &mut raw_count,
                    &mut grid,
                    &mut raw,
                    &mut status,
                    1
                ),
                None
            );
            assert_eq!(records, before_records);
            assert_eq!(grid, before_grid);
            assert_eq!(raw, [[0xa5; 256]; 12]);
            assert_eq!(status, [0x5a; 12]);
            assert_eq!(raw_count, 2);
        }
    }
    #[test]
    fn compaction_rejects_invalid_references_without_partial_mutation() {
        for (count, removed, owner, entries) in [
            (0, 1, 1, 0),
            (2, 1, 1, 0),
            (1, 0, 1, 0),
            (1, 1, 127, 0),
            (1, 1, 1, 21),
        ] {
            let mut records = [[0u8; 128]; 1];
            records[0][0x14] = 7;
            records[0][0x1a] = owner;
            let before = records;
            let mut ids = [1; 12];
            let mut grid: [Vec<i16>; 256] = std::array::from_fn(|_| vec![]);
            grid[0] = vec![1; entries];
            let mut remaining = count;
            assert!(
                super::compact_geometry_tail(
                    &mut records,
                    &mut remaining,
                    removed,
                    &mut ids,
                    &mut grid
                )
                .is_none()
            );
            assert_eq!(records, before);
            assert_eq!(remaining, count);
            assert_eq!(ids, [1; 12]);
            assert_eq!(grid[0], vec![1; entries]);
        }
    }
    use super::*;
    #[test]
    fn combat_movement_guards_unwritten_travel_heading() {
        let initial = AiMovementState {
            heading: 0,
            angle_threshold: 0,
            reverse: 0,
            deadline: -1,
        };
        let mut state = initial;
        let guarded = ai_combat_movement(
            [0, 0, 0],
            [0, 0],
            0,
            &mut state,
            None,
            || 90,
            |_, _, _| panic!("An unwritten heading must not reach the collision probe"),
        );
        assert_eq!(guarded, None);
        assert_eq!(state.heading, 90 << 16);
        assert_eq!(state.deadline, 20);

        let mut calls = Vec::new();
        state = initial;
        let result = ai_combat_movement(
            [0, 0, 0],
            [0, 0],
            0,
            &mut state,
            Some(0),
            || 90,
            |heading, _, _| {
                calls.push(heading);
                true
            },
        );
        assert_eq!(result, Some((1, 0)));
        assert_eq!(calls, vec![180 << 16]);
    }
    #[test]
    fn combined_spawn_advances_flags_before_rejection_and_keeps_latest_heading() {
        let context = SpawnContext {
            enemy: 1,
            kind: 20,
            extent: 1024,
            mode: 0,
            team: 0,
            object_id: 1,
        };
        let mut geometry = [
            SpawnGeometry {
                kind: 20,
                pose: [0; 3],
            },
            SpawnGeometry {
                kind: 12,
                pose: [500 << 16, 500 << 16, 0],
            },
            SpawnGeometry {
                kind: 2,
                pose: [0; 3],
            },
        ];
        let mut cursor = 2;
        let mut samples = [10, 1, 2, 90, 3, 4].into_iter();
        let mut seen = Vec::new();
        let result = spawn_search(
            &context,
            &mut geometry,
            &mut cursor,
            &mut samples,
            |heading| {
                assert_eq!(heading, 90 << 16);
                [65536, 0]
            },
            |pose| {
                seen.push(pose);
                [u8::from(seen.len() == 1), 0]
            },
        )
        .unwrap();
        assert_eq!(
            seen,
            vec![
                [530 << 16, 500 << 16, 90 << 16],
                [3 << 16, 4 << 16, 90 << 16]
            ]
        );
        assert_eq!(cursor, 3);
        assert_eq!(result.pose, geometry[0].pose);
        assert_eq!(result.attempts, 2);
        assert!(samples.next().is_none());
    }

    #[test]
    fn spawn_variants_keep_fixed_anchor_threshold_and_wrapping_offsets() {
        let pose = [101, -101, 90 << 16];
        assert_eq!(
            spawn_team_coordinates(pose, 2, 0, [500 << 16, (500 << 16) + 1], 1024),
            [50, (1024 << 16) + 50, 90 << 16]
        );
        assert_eq!(
            spawn_team_coordinates(pose, 0, 0, [900 << 16; 2], 1024),
            pose
        );
        assert_eq!(
            spawn_team_coordinates(pose, 2, 2, [900 << 16; 2], 1024),
            pose
        );
        assert_eq!(
            spawn_beside_flag([i32::MAX, 0], -90, [1, -65536]),
            [i32::MIN + 29, -30 << 16, 90 << 16]
        );
    }

    #[test]
    fn normal_spawn_retries_keep_heading_and_retain_blocked_final_pose() {
        let mut samples = [90, 0, 1, 2, 3].into_iter();
        let mut seen = Vec::new();
        let result = normal_spawn_search(1, 7, 1024, &mut samples, |pose| {
            seen.push(pose);
            [u8::from(seen.len() == 1), 0]
        })
        .unwrap();
        assert_eq!(
            seen,
            vec![[0, 1 << 16, 90 << 16], [2 << 16, 3 << 16, 90 << 16]]
        );
        assert_eq!(result.attempts, 2);
        assert!(result.clear);
        let mut calls = 0;
        let result = normal_spawn_search(1, 7, 1024, &mut std::iter::repeat(0), |_| {
            calls += 1;
            [1, 0]
        })
        .unwrap();
        assert_eq!(calls, 501);
        assert_eq!(result.attempts, 501);
        assert!(!result.clear);
        assert!(normal_spawn_search(1, 20, 1024, &mut std::iter::repeat(0), |_| [0, 0]).is_none());
    }

    #[test]
    fn spawn_proposals_preserve_ranges_and_random_word_edge() {
        assert_eq!(spawn_coordinate(0, 6, 1024, 0), Some(100 << 16));
        assert_eq!(spawn_coordinate(0, 6, 1024, 412), Some(924 << 16));
        assert_eq!(spawn_coordinate(0, 6, 1024, 413), Some(100 << 16));
        assert_eq!(spawn_coordinate(1, 7, 1024, 1023), Some(1023 << 16));
        assert_eq!(spawn_coordinate(1, 35, 1024, 0), Some(128 << 16));
        assert_eq!(spawn_coordinate(255, 45, 1024, 767), Some(895 << 16));
        assert_eq!(spawn_coordinate(1, 35, 1024, i16::MIN), Some(-384 << 16));
        assert_eq!(spawn_coordinate(0, 6, 100, 0), None);
        assert_eq!(spawn_coordinate(1, 7, 0, 0), None);
    }

    #[test]
    fn enemy_deaths_borrow_in_slot_order_then_retire() {
        let mut flags = [[0; 3]; 12];
        flags[0] = [1, 1, 7];
        flags[1] = [1, 1, 31];
        flags[2] = [1, 1, 7];
        flags[3] = [1, 1, 7];
        let mut lives = [0; 12];
        lives[..4].copy_from_slice(&[1, 9, 2, 2]);
        assert_eq!(consume_enemy_life(0, &flags, &mut lives, 0), Some(false));
        assert_eq!(&lives[..4], &[1, 9, 1, 2]);
        assert_eq!(consume_enemy_life(0, &flags, &mut lives, 0), Some(false));
        assert_eq!(&lives[..4], &[1, 9, 1, 1]);
        assert_eq!(consume_enemy_life(0, &flags, &mut lives, 0), Some(true));
        assert_eq!(&lives[..4], &[0, 9, 1, 1]);
        assert_eq!(consume_enemy_life(0, &flags, &mut lives, 1), Some(false));
        assert_eq!(lives[0], 0);
        assert_eq!(consume_enemy_life(12, &flags, &mut lives, 0), None);
    }

    #[test]
    fn enemy_plan_preserves_word_abs_and_guards_impossible_caps() {
        use crate::formats::Spawn;
        let row = Spawn {
            kind: 7,
            minimum: 2,
            maximum: 8,
            rebirths: 1,
        };
        let plan = enemy_plan(std::slice::from_ref(&row), 10, false, &[i16::MIN]).unwrap();
        // NEG.W leaves -32768 negative; signed remainder must retain that sign.
        assert_eq!(plan.counts[7], 1);
        assert_eq!(plan.lives[7], 3);
        assert!(enemy_plan(std::slice::from_ref(&row), 0, false, &[]).is_none());
        let rows: Vec<_> = (0..6)
            .map(|kind| Spawn {
                kind,
                minimum: 1,
                maximum: 1,
                rebirths: 2,
            })
            .collect();
        assert!(enemy_plan(&rows, 0, true, &[0; 6]).is_none());
        assert_eq!(enemy_plan(&rows, 0, false, &[0; 6]).unwrap().total, 6);
        assert!(enemy_plan(&[], 0, false, &[]).is_none());
    }

    #[test]
    fn score_awards_preserve_penalties_disabled_lives_and_guard_bad_resources() {
        let rules = crate::formats::ScoreRules {
            thresholds: [
                5000, 10000, 20000, 30000, 40000, 60000, 80000, 100000, 120000, 140000,
            ],
            repeat_increment: -25536,
        };
        assert_eq!(
            award_score(4990, 3, 20, false, Some(&rules)),
            Some((5010, 4, 1))
        );
        assert_eq!(
            award_score(4990, 3, 20, true, Some(&rules)),
            Some((5010, 3, 0))
        );
        assert_eq!(
            award_score(5010, 4, -20, false, Some(&rules)),
            Some((4990, 4, 0))
        );
        assert_eq!(
            award_score(4990, 3, 10000, false, Some(&rules)),
            Some((14990, 5, 2))
        );
        assert_eq!(next_life_threshold(140000, Some(&rules)), Some(140001));
        assert_eq!(next_life_threshold(140001, Some(&rules)), Some(2147478624));
        let invalid = crate::formats::ScoreRules {
            repeat_increment: 0,
            ..rules
        };
        assert_eq!(next_life_threshold(140001, Some(&invalid)), None);
    }
    #[test]
    fn boundaries_and_fractional_negative_angles() {
        assert_eq!(wrap_angle(-1), FULL_TURN - 1);
        assert_eq!(wrap_angle(FULL_TURN), 0);
        assert_eq!(wrap_angle(-FULL_TURN), 0);
        assert_eq!(angular_distance(359 << 16, 1 << 16), 2 << 16);
        assert_eq!(angular_distance(0, HALF_TURN), HALF_TURN);
        assert_eq!(direction_table_index(-1), 359);
        assert_eq!(direction_table_index((90 << 16) + 65535), 90);
    }
}

/// Ten-byte CODE 12 edge record, excluding its unwritten final padding byte.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelEdge {
    pub vertices: [i16; 2],
    pub visibility_refs: [i16; 2],
    pub flag: u8,
}

/// CODE 12 +5f2: merge an oppositely submitted edge or append a new record.
/// Arguments retain the stack-word order at +e/+10 and +a/+c respectively.
pub fn insert_model_edge(
    edges: &mut Vec<ModelEdge>,
    vertex_arguments: [i16; 2],
    reference_arguments: [i16; 2],
    flag: u8,
) {
    if let Some(edge) = edges
        .iter_mut()
        .find(|edge| edge.vertices == vertex_arguments)
    {
        edge.visibility_refs[1] = reference_arguments[1];
    } else {
        edges.push(ModelEdge {
            vertices: [vertex_arguments[1], vertex_arguments[0]],
            visibility_refs: [reference_arguments[1], reference_arguments[0]],
            flag,
        });
    }
}

/// CODE 12 +b60..+ba2: selection in the first edge scan path.
/// Preserve bytewise tests, including non-boolean visibility bytes.
pub fn edge_scan_simple(clipped: u8, visibility: [u8; 2]) -> bool {
    let mask = clipped ^ 1;
    mask != 0 && (visibility[0] != 0 || mask & visibility[1] != 0)
}

/// CODE 12 +bde..+cb0: (queued for line drawing, edge classification byte).
/// Screen coordinates are the signed words at projected-record offsets +2/+6.
pub fn edge_scan_partitioned(
    clipped: u8,
    flag: u8,
    visibility: [u8; 2],
    coordinates: [i16; 2],
) -> (bool, u8) {
    if clipped != 0 {
        return (false, 1);
    }
    if (flag != 0 && (visibility[0] != 0 || flag & visibility[1] != 0))
        || (visibility[0] != 0 && visibility[1] != 0)
    {
        return (true, 1);
    }
    match (visibility[0] != 0, visibility[1] != 0) {
        (true, false) => (
            false,
            if coordinates[0] > coordinates[1] {
                2
            } else {
                0
            },
        ),
        (false, true) => (
            false,
            if coordinates[0] > coordinates[1] {
                0
            } else {
                2
            },
        ),
        _ => (false, 1),
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProjectedEdge {
    pub endpoints: [i16; 4],
    pub clipped: u8,
    pub flag: u8,
    pub visibility: [u8; 2],
    pub classification: u8,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EdgeDrawBatch {
    /// A5 drawing-service displacement, pending rasterizer integration.
    pub service: u16,
    pub color: u8,
    pub segments: Vec<[i16; 4]>,
}

/// Complete CODE 12 +b30 dispatcher for a valid contiguous projected-edge range.
/// `first` is its original one-based index. Original storage permits at most 50 edges.
pub fn edge_draw_batches(
    edges: &mut [ProjectedEdge],
    first: i16,
    mode: u8,
    special_order: u8,
    style: u8,
    color: u8,
) -> Vec<EdgeDrawBatch> {
    let mut batches = Vec::new();
    let mut emit = |service, segments: Vec<[i16; 4]>| {
        if !segments.is_empty() {
            batches.push(EdgeDrawBatch {
                service,
                color,
                segments,
            });
        }
    };
    if mode == 0 {
        emit(
            0x6d2,
            edges
                .iter()
                .filter(|e| edge_scan_simple(e.clipped, e.visibility))
                .map(|e| e.endpoints)
                .collect(),
        );
        return batches;
    }
    let mut direct = Vec::new();
    for edge in edges.iter_mut() {
        let (queued, classification) = edge_scan_partitioned(
            edge.clipped,
            edge.flag,
            edge.visibility,
            [edge.endpoints[1], edge.endpoints[3]],
        );
        edge.classification = classification;
        if queued {
            direct.push(edge.endpoints);
        }
    }
    emit(if style != 0 { 0x722 } else { 0x6d2 }, direct);
    let mut second = Vec::new();
    let mut third: Vec<[i16; 4]> = Vec::new();
    for (offset, edge) in edges.iter().enumerate() {
        let index = first + offset as i16;
        if edge.classification == 2 && !(special_order != 0 && [2, 8].contains(&index)) {
            second.push(edge.endpoints);
        }
        if edge.classification == 0 && !(special_order != 0 && [1, 9].contains(&index)) {
            let p = edge.endpoints;
            let delta = [
                p[2].wrapping_sub(p[0]) as i32,
                p[3].wrapping_sub(p[1]) as i32,
            ];
            let position = third
                .iter()
                .position(|q| {
                    let old = [
                        q[2].wrapping_sub(q[0]) as i32,
                        q[3].wrapping_sub(q[1]) as i32,
                    ];
                    (delta[1] * old[0]).abs() <= (delta[0] * old[1]).abs()
                })
                .unwrap_or(third.len());
            third.insert(position, p);
        }
    }
    if special_order != 0 {
        for index in [2_i16, 8] {
            if let Some(edge) = index
                .checked_sub(first)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| edges.get(i))
                && edge.classification == 2
            {
                second.push(edge.endpoints);
            }
        }
        for index in [1_i16, 9] {
            if let Some(edge) = index
                .checked_sub(first)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| edges.get(i))
                && edge.classification == 0
            {
                third.push(edge.endpoints);
            }
        }
    }
    emit(if style != 0 { 0x71a } else { 0x70a }, second);
    emit(if style != 0 { 0x72a } else { 0x712 }, third);
    batches
}

/// CODE 11 +149c..+1592: line-color tables for four/eight-bit display setup.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LinePalette {
    pub high: [i16; 13],
    pub low: [i16; 13],
    pub patterns: [u32; 13],
}

pub fn line_palette(depth: i16, four_bit_color: u8, remapping: [i16; 13]) -> LinePalette {
    let mut result = LinePalette {
        high: [0; 13],
        low: [0; 13],
        patterns: [0; 13],
    };
    for (index, mapped) in remapping.iter().copied().enumerate() {
        let black = mapped == remapping[12];
        let low = if depth == 4 {
            if black { 15 } else { mapped.wrapping_shl(2) }
        } else if black {
            255
        } else {
            mapped.wrapping_shl(4)
        };
        let high = if depth == 4 { low.wrapping_shl(4) } else { low };
        result.low[index] = low;
        result.high[index] = high;
        result.patterns[index] = u32::from((low as u16) | (high as u16)).wrapping_mul(0x01010101);
    }
    if depth == 4 && four_bit_color != 0 {
        result.patterns[5] = 0x48488484;
    }
    result
}

/// CODE 11 +4ee..+658: clip a line and prepare its sixteen-byte raster record.
/// Endpoints are [y0,x0,y1,x1]; viewport and origin are [x,y].
/// Requires positive viewport dimensions and nonzero clipping divisors.
pub fn deferred_line_record(
    endpoints: [i16; 4],
    viewport: [i16; 2],
    origin: [i16; 2],
    stride: i16,
    base: u32,
    depth: i16,
) -> Option<[u8; 16]> {
    let [mut y0, mut x0, mut y1, mut x1] = endpoints;
    let [width, height] = viewport.map(|v| v.wrapping_sub(1));
    // DIVS leaves the destination unchanged when its quotient overflows a word.
    let ratio = |a: i16, b: i16, divisor: i16| -> i16 {
        let product = i32::from(a) * i32::from(b);
        let quotient = product / i32::from(divisor);
        i16::try_from(quotient).unwrap_or(product as i16)
    };
    if x1 < x0 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }
    if x0 < 0 {
        if x1 < 0 {
            return None;
        }
        y0 = y0.wrapping_add(ratio(y1.wrapping_sub(y0), x0, x0.wrapping_sub(x1)));
        x0 = 0;
    }
    if x1 > width {
        if x0 > width {
            return None;
        }
        y1 = y1.wrapping_add(ratio(
            y0.wrapping_sub(y1),
            width.wrapping_sub(x1),
            x0.wrapping_sub(x1),
        ));
        x1 = width;
    }
    if y0 < 0 {
        if y1 < 0 {
            return None;
        }
        x0 = x0.wrapping_add(ratio(x1.wrapping_sub(x0), y0, y0.wrapping_sub(y1)));
        y0 = 0;
    } else if y1 < 0 {
        x1 = x1.wrapping_add(ratio(x0.wrapping_sub(x1), y1, y1.wrapping_sub(y0)));
        y1 = 0;
    }
    if y0 > height {
        if y1 > height {
            return None;
        }
        x0 = x0.wrapping_add(ratio(
            x1.wrapping_sub(x0),
            y0.wrapping_sub(height),
            y0.wrapping_sub(y1),
        ));
        y0 = height;
    }
    if y1 > height {
        x1 = x1.wrapping_add(ratio(
            x0.wrapping_sub(x1),
            y1.wrapping_sub(height),
            y1.wrapping_sub(y0),
        ));
        y1 = height;
    }
    let dx = x1.wrapping_sub(x0);
    let dy = y1.wrapping_sub(y0);
    let step = if y1 < y0 {
        stride.wrapping_neg()
    } else {
        stride
    };
    let dy = if y1 < y0 { dy.wrapping_neg() } else { dy };
    let horizontal = dy <= dx;
    let (minor, major) = if horizontal { (dy, dx) } else { (dx, dy) };
    let x = x0.wrapping_add(origin[0]);
    let y = y0.wrapping_add(origin[1]);
    let (offset, mask) = match depth {
        8 => (x, 0),
        4 => (x >> 1, if x & 1 != 0 { 255 } else { 0 }),
        _ => (x >> 3, 0x7f_u8.rotate_right((x & 7) as u32)),
    };
    let address = base
        .wrapping_add((i32::from(stride) * i32::from(y)) as u32)
        .wrapping_add(i32::from(offset) as u32);
    let mut record = [0; 16];
    for (at, value) in [(0, x0), (2, major), (8, step), (10, minor), (12, major)] {
        record[at..at + 2].copy_from_slice(&value.to_be_bytes());
    }
    record[4..8].copy_from_slice(&address.to_be_bytes());
    record[14] = u8::from(horizontal);
    record[15] = mask;
    Some(record)
}

/// Complete CODE 10 +440..+4b2, eight-bit deferred line rasterization.
/// Returns None if a record addresses outside the supplied framebuffer.
/// Valid records come from deferred_line_record with depth eight.
pub fn raster_deferred_lines_8(
    framebuffer: &mut [u8],
    base: u32,
    records: &[[u8; 16]],
    pattern: u32,
) -> Option<()> {
    let and_mask = (pattern >> 8) as u8;
    let or_mask = pattern as u8;
    for record in records {
        let word = |offset| i16::from_be_bytes([record[offset], record[offset + 1]]);
        let count = word(2) as u16;
        let mut address = u32::from_be_bytes(record[4..8].try_into().ok()?);
        let step = i32::from(word(8)) as u32;
        let minor = word(10);
        let major = word(12);
        let horizontal = record[14] != 0;
        let mut error = major >> 1;
        for _ in 0..=count {
            let pixel = framebuffer.get_mut(address.wrapping_sub(base) as usize)?;
            *pixel = (*pixel & and_mask) | or_mask;
            if horizontal {
                address = address.wrapping_add(1);
            }
            error = error.wrapping_sub(minor);
            if error < 0 {
                error = error.wrapping_add(major);
                address = address.wrapping_add(if horizontal { step } else { 1 });
            }
            if !horizontal {
                address = address.wrapping_add(step);
            }
        }
    }
    Some(())
}

/// Complete CODE 11 +1f88 (four-bit) / +1fdc (eight-bit) mask builders.
pub fn deferred_line_mask(depth: i16, mapped: i16, phase: u8) -> u32 {
    let bytes = if depth == 4 {
        if phase != 0 {
            [
                0x3f,
                mapped.wrapping_shl(6) as u8,
                0xf3,
                mapped.wrapping_shl(2) as u8,
            ]
        } else {
            [0xcf, mapped.wrapping_shl(4) as u8, 0xfc, mapped as u8]
        }
    } else {
        let (mask, value) = if phase != 0 {
            (7, mapped.wrapping_shl(3) as u8)
        } else {
            (0x38, mapped as u8)
        };
        [mask, value, mask, value]
    };
    u32::from_be_bytes(bytes)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeferredLinePalette {
    pub high: [i16; 13],
    pub low: [i16; 13],
    pub phase_zero: [u32; 13],
    pub phase_one: [u32; 13],
}

/// CODE 11 +161a..+1742, selected deferred-palette setup branch.
pub fn deferred_line_palette(depth: i16, remapping: [i16; 13]) -> DeferredLinePalette {
    DeferredLinePalette {
        high: remapping.map(|v| v.wrapping_mul(if depth == 4 { 80 } else { 9 })),
        low: remapping.map(|v| v.wrapping_mul(if depth == 4 { 5 } else { 9 })),
        phase_zero: remapping.map(|v| deferred_line_mask(depth, v, 0)),
        phase_one: remapping.map(|v| deferred_line_mask(depth, v, 1)),
    }
}

/// CODE 11 +1596..+161a: two transposed deferred RGB tables.
/// CODE 11 +1816 selects table zero on even frames and table one on odd frames.
/// The selected deferred display modes supply four or eight base colors.
pub fn deferred_rgb_palettes(base: &[[u16; 3]]) -> [Vec<[u16; 3]>; 2] {
    let mut low_field = Vec::with_capacity(base.len() * base.len());
    let mut high_field = Vec::with_capacity(base.len() * base.len());
    for row in base {
        for column in base {
            low_field.push(*column);
            high_field.push(*row);
        }
    }
    [low_field, high_field]
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DeferredColorBatch {
    pub lines: Vec<[u8; 16]>,
    pub pixels: [Vec<u32>; 2],
}

/// Complete CODE 11 +37a6/+388c eight-bit batch paint/erase paths.
/// Erasure uses mapped color twelve for every group, preserving the other field.
pub fn draw_deferred_frame_8(
    framebuffer: &mut [u8],
    base: u32,
    batches: &[DeferredColorBatch; 13],
    remapping: [i16; 13],
    phase: u8,
    erase: bool,
) -> Option<()> {
    for (index, batch) in batches.iter().enumerate() {
        let mapped = remapping[if erase { 12 } else { index }];
        let mask = deferred_line_mask(8, mapped, phase);
        raster_deferred_lines_8(framebuffer, base, &batch.lines, mask)?;
        for addresses in &batch.pixels {
            for address in addresses {
                let pixel = framebuffer.get_mut(address.wrapping_sub(base) as usize)?;
                *pixel = (*pixel & (mask >> 8) as u8) | mask as u8;
            }
        }
    }
    Some(())
}

#[derive(Clone, Copy, Debug)]
pub struct LineRasterTarget {
    pub viewport: [i16; 2],
    pub origin: [i16; 2],
    pub stride: i16,
    pub base: u32,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DeferredFrameQueue {
    pub dirty: bool,
    pub colors: [DeferredColorBatch; 13],
}

impl DeferredFrameQueue {
    /// CODE 11 +20a4 deferred branch: append accepted lines in their input order.
    /// Native vectors replace Macintosh allocation; original capacity growth is separate.
    pub fn queue_lines(
        &mut self,
        color: u8,
        lines: &[[i16; 4]],
        target: LineRasterTarget,
    ) -> Option<()> {
        let batch = self.colors.get_mut(usize::from(color))?;
        self.dirty = true;
        batch.lines.extend(lines.iter().filter_map(|points| {
            deferred_line_record(
                *points,
                target.viewport,
                target.origin,
                target.stride,
                target.base,
                8,
            )
        }));
        Some(())
    }
}

impl DeferredFrameQueue {
    /// Complete CODE 11 +840: a clean queue is untouched, including its counts.
    pub fn clear_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }
        for color in &mut self.colors {
            color.lines.clear();
            for pixels in &mut color.pixels {
                pixels.clear();
            }
        }
        self.dirty = false;
    }

    /// CODE 11 +896: concatenate all three lists when the source is dirty.
    /// Native vectors replace the original capacity checks and BlockMove calls.
    pub fn append_dirty(&mut self, source: &Self) {
        if !source.dirty {
            return;
        }
        self.dirty = true;
        for (destination, source) in self.colors.iter_mut().zip(&source.colors) {
            destination.lines.extend_from_slice(&source.lines);
            for (destination, source) in destination.pixels.iter_mut().zip(&source.pixels) {
                destination.extend_from_slice(source);
            }
        }
    }
}

/// CODE 11 +3ae8..+3b30; slots correspond to A5 -6be0, -6a8e, -67ea.
pub fn rotate_deferred_queues(queues: &mut [DeferredFrameQueue; 3]) {
    queues.rotate_left(1);
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DeferredFrameCycle {
    pub frame: i32,
    /// A5 -6be0, -6a8e, -67ea, in that order.
    pub queues: [DeferredFrameQueue; 3],
    /// A5 -693c, merged into the newly displayed queue after rotation.
    pub auxiliary: DeferredFrameQueue,
    pub palette_pending: bool,
}

impl DeferredFrameCycle {
    /// Complete CODE 11 +3a78 deferred eight-bit path, after palette readiness.
    /// Returns None while a palette update is pending, or for an invalid raster address.
    /// The platform must acknowledge palette upload before the next presentation.
    pub fn present_8(
        &mut self,
        framebuffer: &mut [u8],
        base: u32,
        remapping: [i16; 13],
    ) -> Option<()> {
        if self.palette_pending {
            return None;
        }
        let phase = (self.frame.wrapping_add(1) as u32 & 1) as u8;
        if self.queues[0].dirty {
            draw_deferred_frame_8(
                framebuffer,
                base,
                &self.queues[0].colors,
                remapping,
                phase,
                true,
            )?;
            self.queues[0].clear_if_dirty();
        }
        draw_deferred_frame_8(
            framebuffer,
            base,
            &self.queues[2].colors,
            remapping,
            phase,
            false,
        )?;
        self.frame = self.frame.wrapping_add(1);
        self.palette_pending = true;
        rotate_deferred_queues(&mut self.queues);
        self.queues[1].append_dirty(&self.auxiliary);
        self.auxiliary.clear_if_dirty();
        Some(())
    }
}

/// Complete CODE 11 +3b7e: project the first three words into the final two.
/// The original sign-branch observes wrapped `depth - 10`, not a signed comparison.
pub fn project_vertex_record(vertex: &mut [i16; 5]) {
    let depth = vertex[2];
    if depth.wrapping_sub(10) < 0 {
        return;
    }
    for axis in 0..2 {
        let numerator = i32::from(vertex[axis]) << 8;
        let quotient = numerator / i32::from(depth);
        // Overflow leaves DIVS's destination unchanged; only its low word is stored.
        vertex[axis + 3] = i16::try_from(quotient).unwrap_or(numerator as i16);
    }
}

/// Complete CODE 11 +3ba8: packed vertex transformation and projection.
/// Output projection words remain unchanged when the original depth branch skips them.
pub fn transform_project_vertices(
    vertices: &[[i16; 3]],
    rows: [[i16; 3]; 3],
    translation: [i16; 3],
    output: &mut [[i16; 5]],
) -> Option<()> {
    if vertices.len() != output.len() {
        return None;
    }
    for (vertex, result) in vertices.iter().zip(output) {
        for axis in 0..3 {
            let sum = rows[axis]
                .iter()
                .zip(vertex)
                .fold(0_i32, |sum, (coefficient, value)| {
                    sum.wrapping_add(i32::from(*coefficient) * i32::from(*value))
                });
            result[axis] = ((sum >> 14) as i16).wrapping_add(translation[axis]);
        }
        project_vertex_record(result);
    }
    Some(())
}

/// CODE 12 +0x1af6..0x1b56, including its original +0x0140 direction lookup.
/// Position is the render record's horizontal, height, and depth words.
/// Camera height is already converted to packed eighth-unit coordinates.
pub fn model_view_transform(
    position: [i16; 3],
    camera_height: i16,
    relative_heading: i32,
    table: &[i32; 360],
) -> ([[i16; 3]; 3], [i16; 3]) {
    let index = direction_table_index(relative_heading);
    let complement = if index <= 90 { 90 - index } else { 450 - index };
    let sine = table[index] as i16;
    let cosine = table[complement] as i16;
    (
        [
            [cosine, 0, sine.wrapping_neg()],
            [0, 16384, 0],
            [sine, 0, cosine],
        ],
        [
            position[0].wrapping_neg(),
            position[1].wrapping_sub(camera_height),
            position[2],
        ],
    )
}

/// CODE 6 +0x1af2..0x1b6a: camera-relative fixed positions to render-record words.
/// Basis contains the signed words at caller frame +0x10 and +0x14.
/// Returns horizontal displacement followed by depth.
pub fn render_record_planar(delta: [i32; 2], basis: [i16; 2]) -> [i16; 2] {
    let packed = delta.map(|value| (value.wrapping_shl(5).wrapping_add(0x8000) >> 16) as i16);
    let product =
        |axis: usize, component: usize| i32::from(packed[axis]) * i32::from(basis[component]);
    let depth = product(0, 0).wrapping_add(product(1, 1));
    let horizontal = product(1, 0).wrapping_sub(product(0, 1));
    [(horizontal >> 16) as i16, (depth >> 16) as i16]
}

/// CODE 11 +0x3b58: signed-word interpolation, including DIVS overflow.
pub fn interpolate_vertex_axis(
    previous: i16,
    current: i16,
    previous_depth: i16,
    current_depth: i16,
    plane: i16,
) -> i16 {
    let denominator = current_depth.wrapping_sub(previous_depth);
    if denominator == 0 {
        return previous;
    }
    let product =
        i32::from(current.wrapping_sub(previous)) * i32::from(plane.wrapping_sub(previous_depth));
    let quotient = product / i32::from(denominator);
    i16::try_from(quotient)
        .unwrap_or(product as i16)
        .wrapping_add(previous)
}

/// CODE 12 +0x12ca: indexed polygon clipping at packed depth 16 and screen placement.
/// Input records are XYZ/projected-XY. Indices are host zero-based; output is screen XY.
/// Empty or invalid index lists are host guards, not original behavior.
pub fn clip_project_face(
    vertices: &[[i16; 5]],
    indices: &[usize],
    origin: [i16; 2],
    offset: [i16; 2],
) -> Option<Vec<[i16; 2]>> {
    Some(
        clip_face_records(vertices, indices)?
            .into_iter()
            .map(|v| {
                [
                    v[3].wrapping_add(origin[0]).wrapping_add(offset[0]),
                    origin[1].wrapping_sub(v[4]).wrapping_add(offset[1]),
                ]
            })
            .collect(),
    )
}

/// Retain original clipped XYZ/projected-XY records for the host depth buffer.
/// The original clipper stores only screen XY; intersection order is identical.
pub fn clip_face_records(vertices: &[[i16; 5]], indices: &[usize]) -> Option<Vec<[i16; 5]>> {
    let mut previous = *vertices.get(*indices.last()?)?;
    let mut result = Vec::with_capacity(indices.len() + 2);
    for &index in indices {
        let current = *vertices.get(index)?;
        if (previous[2] < 16) != (current[2] < 16) {
            let mut cut = previous;
            for axis in 0..2 {
                cut[axis] = interpolate_vertex_axis(
                    previous[axis],
                    current[axis],
                    previous[2],
                    current[2],
                    16,
                );
            }
            cut[2] = 16;
            project_vertex_record(&mut cut);
            result.push(cut);
        }
        if current[2] >= 16 {
            result.push(current);
        }
        previous = current;
    }
    Some(result)
}

#[cfg(test)]
mod packed_face_depth_tests {
    use super::*;

    #[test]
    fn crossing_face_retains_original_intersection_depths_and_projection() {
        let vertices = [
            [-16, 16, 0, 111, 222],
            [16, 16, 32, 128, 128],
            [0, -16, 32, 0, -128],
        ];
        assert_eq!(
            clip_face_records(&vertices, &[0, 1, 2]).unwrap(),
            vec![
                [-8, 0, 16, -128, 0],
                [0, 16, 16, 0, 256],
                vertices[1],
                vertices[2],
            ]
        );
        assert!(clip_face_records(&vertices, &[0]).unwrap().is_empty());
        assert!(clip_face_records(&vertices, &[3]).is_none());
    }
}

/// CODE 6 +0x27fe..0x283c: object-origin horizontal visibility gate.
pub fn solid_origin_visible(horizontal: i16, depth: i16, full_size: bool) -> bool {
    let side = horizontal.wrapping_abs().wrapping_sub(288);
    let forward = depth.wrapping_add(288);
    if full_size {
        i32::from(forward as u16) * 2 >= i32::from(side)
    } else {
        forward >= side
    }
}

/// CODE 12 +0x106c..0x1128: clip a model edge against packed depth 16.
/// Preserve endpoint order and the original first-to-second interpolation order.
pub fn clip_edge_records(mut edge: [[i16; 5]; 2]) -> Option<[[i16; 5]; 2]> {
    let outside = edge.map(|v| v[2] < 16);
    if outside == [true, true] {
        return None;
    }
    if outside[0] != outside[1] {
        let endpoint = usize::from(outside[1]);
        for axis in 0..2 {
            edge[endpoint][axis] =
                interpolate_vertex_axis(edge[0][axis], edge[1][axis], edge[0][2], edge[1][2], 16);
        }
        edge[endpoint][2] = 16;
        project_vertex_record(&mut edge[endpoint]);
    }
    Some(edge)
}

/// CODE 6 +0x283c..0x2880: suppression, depth, and cached squared-distance gates.
/// Returns admission and the updated signed high-word distance cache.
pub fn solid_distance_visible(
    horizontal: i16,
    depth: i16,
    suppressed: u8,
    cached: i16,
) -> (bool, i16) {
    if suppressed != 0 || depth >= 2400 {
        return (false, cached);
    }
    let cached = if cached == -1 {
        let square = |v: i16| i32::from(v) * i32::from(v);
        (square(depth).wrapping_add(square(horizontal)) >> 16) as i16
    } else {
        cached
    };
    (cached < 87, cached)
}

/// CODE 12 +0x0aa4: evaluate a packed face plane at signed-word coordinates.
pub fn evaluate_face_plane(plane: &[u8; 12], point: [i16; 3]) -> i32 {
    let constant = i32::from_be_bytes(plane[4..8].try_into().unwrap());
    [0, 8, 2]
        .into_iter()
        .zip(point)
        .fold(constant, |sum, (offset, coordinate)| {
            let normal = i16::from_be_bytes([plane[offset], plane[offset + 1]]);
            sum.wrapping_add(i32::from(normal) * i32::from(coordinate))
        })
}

/// CODE 12 +0x0afe: equality is visible; test the wrapped signed plane result.
pub fn face_plane_visible(plane: &[u8; 12], point: [i16; 3]) -> bool {
    evaluate_face_plane(plane, point) >= 0
}

/// CODE 12 +0x1c30..0x1c50 and +0x1cec..0x1d56: viewer in object coordinates.
/// Delta is object minus camera in fixed world coordinates; heights are packed words.
pub fn face_viewpoint(
    delta: [i32; 2],
    heading: i32,
    table: &[i32; 360],
    camera_height: i16,
    object_height: i16,
) -> [i16; 3] {
    let negative = delta.map(|v| ((v.wrapping_shl(5) >> 16) as i16).wrapping_neg());
    let index = direction_table_index(heading);
    let complement = if index <= 90 { 90 - index } else { 450 - index };
    let sine = i32::from(table[index] as i16);
    let cosine = i32::from(table[complement] as i16);
    let [x, z] = negative.map(i32::from);
    [
        (x.wrapping_mul(sine).wrapping_sub(z.wrapping_mul(cosine)) >> 16) as i16,
        camera_height.wrapping_sub(object_height),
        (x.wrapping_mul(cosine).wrapping_add(z.wrapping_mul(sine)) >> 16) as i16,
    ]
}

/// CODE 1 +192a..197c: top-view plane count and whether the trailing resource word is consumed.
/// The compatibility branch substitutes -1 without reading the resource word.
pub fn resource_plane_sides(stored: i16, last_line: i16, format: u8, version: u8) -> (i16, bool) {
    let consume = !(format == 2 && version >> 4 == 0);
    let value = if consume { stored } else { -1 };
    (
        if value == -1 {
            last_line.wrapping_add(1)
        } else {
            value
        }
        .min(8),
        consume,
    )
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RenderModelFields {
    pub face_count: i16,
    /// Runtime +25e: boundary between edge groups, not total edge count.
    #[serde(alias = "edge_count")]
    pub edge_split: i16,
    pub mode: u8,
}

impl RenderModelFields {
    /// Read the fields consumed by CODE 12's prepared-record construction.
    pub fn from_runtime(model: &[u8]) -> Option<Self> {
        let fields = model.get(0x25c..0x266)?;
        Some(Self {
            face_count: i16::from_be_bytes([fields[0], fields[1]]),
            edge_split: i16::from_be_bytes([fields[2], fields[3]]),
            mode: fields[9],
        })
    }
}

/// Complete CODE 12 +08ee: configure an edge split, two face references, and mode byte.
/// The caller has already resolved the selected runtime model slot.
pub fn configure_model_edges(
    model: &mut [u8],
    edge_split: i16,
    face_references: [i16; 2],
    mode: u8,
) -> Option<()> {
    let fields = model.get_mut(0x25e..0x265)?;
    fields[0..2].copy_from_slice(&edge_split.to_be_bytes());
    fields[2..4].copy_from_slice(&face_references[0].to_be_bytes());
    fields[4..6].copy_from_slice(&face_references[1].to_be_bytes());
    fields[6] = mode;
    Some(())
}

/// Complete CODE 12 +0926: set the separate prepared-rendering mode byte.
pub fn enable_model_render_mode(model: &mut [u8]) -> Option<()> {
    *model.get_mut(0x265)? = 1;
    Some(())
}

/// CODE 12 +0x1c20..0x1d56: construct one prepared record and update class bounds.
/// Fields not written by the original remain unchanged, including face coordinates
/// when the model has no positive face count.
pub fn prepare_render_record(
    input: &[u8; 64],
    output: &mut [u8; 32],
    model: RenderModelFields,
    interval: [i16; 2],
    classes: &mut [i8; 2],
    table: &[i32; 360],
) {
    let long = |offset| i32::from_be_bytes(input[offset..offset + 4].try_into().unwrap());
    output[0..2].copy_from_slice(&input[6..8]);
    let negative = [long(12), long(16)].map(|v| ((v.wrapping_shl(5) >> 16) as i16).wrapping_neg());
    output[2..4].copy_from_slice(&negative[0].to_be_bytes());
    output[4..6].copy_from_slice(&negative[1].to_be_bytes());
    output[6..8].copy_from_slice(&input[0..2]);
    output[8] = input[4];
    output[9] = u8::from(model.edge_split != 0);
    output[10..12].copy_from_slice(&interval[0].to_be_bytes());
    output[12..14].copy_from_slice(&interval[1].to_be_bytes());
    output[18..22].copy_from_slice(&input[20..24]);
    output[22] = model.mode;
    output[24..26].copy_from_slice(&input[28..30]);
    output[26..28].copy_from_slice(&input[32..34]);
    output[28..30].copy_from_slice(&input[30..32]);
    output[30..32].copy_from_slice(&input[10..12]);
    let class = input[33] as i8;
    classes[0] = classes[0].min(class);
    classes[1] = classes[1].max(class);
    if model.face_count > 0 {
        let point = face_viewpoint([long(12), long(16)], long(20), table, 0, 0);
        output[14..16].copy_from_slice(&point[0].to_be_bytes());
        output[16..18].copy_from_slice(&point[2].to_be_bytes());
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RenderPreparationState {
    pub vertices: Vec<[i16; 5]>,
    pub record: [u8; 32],
    pub interval: [i16; 2],
    pub classes: [i8; 2],
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RenderPreparationConfig {
    pub model: RenderModelFields,
    pub camera_height: i16,
    /// The base model's count can differ from the selected override geometry count.
    pub scan_count: i16,
    pub detail: u8,
    pub filled: u8,
}

/// CODE 12 +0x1af6..0x1d56, including original transform and direction services.
/// Preserve projection words when near-depth projection is skipped, and preserve
/// interval locals when the bytewise wireframe-detail gate is disabled.
pub fn prepare_render_geometry(
    input: &[u8; 64],
    vertices: &[[i16; 3]],
    table: &[i32; 360],
    config: RenderPreparationConfig,
    state: &mut RenderPreparationState,
) -> Option<()> {
    let scan = config.detail & (config.filled ^ 1) != 0;
    let count = config.scan_count.max(0) as usize;
    if vertices.len() > state.vertices.len() || (scan && count > state.vertices.len()) {
        return None;
    }
    let word = |offset| i16::from_be_bytes(input[offset..offset + 2].try_into().unwrap());
    let heading = i32::from_be_bytes(input[24..28].try_into().unwrap());
    let (rows, translation) = model_view_transform(
        [word(8), word(10), word(6)],
        config.camera_height,
        heading,
        table,
    );
    transform_project_vertices(
        vertices,
        rows,
        translation,
        &mut state.vertices[..vertices.len()],
    )?;
    if scan {
        state.interval = [9999, -9999];
        for vertex in state.vertices.iter().take(count) {
            state.interval[0] = state.interval[0].min(vertex[3]);
            state.interval[1] = state.interval[1].max(vertex[3]);
        }
    }
    prepare_render_record(
        input,
        &mut state.record,
        config.model,
        state.interval,
        &mut state.classes,
        table,
    );
    Some(())
}

/// CODE 12 +0x1a5e..0x1af6: stable insertion order for signed object sort keys.
/// The original shifts each 32-byte prepared record and its geometry pointer together.
pub fn render_preparation_order(keys: &[i16]) -> Vec<usize> {
    let mut order: Vec<_> = (0..keys.len()).collect();
    order.sort_by_key(|&index| keys[index]);
    order
}

/// CODE 12 +0x1d56..0x1dc2: mark records whose projected intervals overlap.
/// Preserve the original bytewise mode gate and inclusive signed comparisons.
pub fn render_overlap_flags(
    intervals: &[[i16; 2]],
    flags: &[u8],
    inserted: usize,
    detail: u8,
    filled: u8,
) -> Option<Vec<u8>> {
    if intervals.len() != flags.len() || inserted >= intervals.len() {
        return None;
    }
    let mut result = flags.to_vec();
    if detail & (filled ^ 1) != 0 {
        let [low, high] = intervals[inserted];
        for (index, &[other_low, other_high]) in intervals.iter().enumerate() {
            if index != inserted && other_low <= high && other_high >= low {
                result[index] = 1;
                result[inserted] = 1;
            }
        }
    }
    Some(result)
}

/// CODE 12 +0x1dd4..0x1efe: dispatch order for prepared render records.
/// Input order is the preceding preparation pass, not unsorted world order.
/// Filled rendering visits signed classes ascending and records descending within each class.
/// Wireframe rendering visits records ascending, regardless of class.
pub fn render_dispatch_order(classes: &[i8], filled: bool) -> Vec<usize> {
    let mut order: Vec<_> = (0..classes.len()).collect();
    if filled {
        order.sort_unstable_by_key(|&index| (classes[index], std::cmp::Reverse(index)));
    }
    order
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct HorizonInput {
    pub center: [i16; 2],
    pub half_width: i16,
    pub vertical_offset: i16,
    pub level: i16,
    pub depth: i16,
    pub four_bit: bool,
    pub color: i8,
    pub solid_color: Option<i16>,
}

/// CODE 6 +0x4a1a: horizon draw requests, with OffsetRect expressed as word arithmetic.
/// Output records are top, left, bottom, right, color, shade.
pub fn horizon_bands(input: HorizonInput) -> Vec<[i16; 6]> {
    let (first, step): (i16, i16) = if input.depth == 4 && input.four_bit {
        (4, 6)
    } else {
        (0, 5)
    };
    let bottom = input.center[1]
        .wrapping_add(3)
        .wrapping_add(input.vertical_offset);
    let left = input.center[0].wrapping_sub(input.half_width);
    let right = input.center[0].wrapping_add(input.half_width);
    if let Some(color) = input.solid_color.filter(|color| *color != -1) {
        return vec![[
            bottom.wrapping_sub(step * 16),
            left,
            bottom,
            right,
            color,
            0,
        ]];
    }
    let limit = (input.level / 8).wrapping_add(11).clamp(0, 15);
    if limit < 1 {
        return vec![];
    }
    let mut bottom = bottom.wrapping_sub((15 - limit).wrapping_mul(step));
    let mut bands = Vec::new();
    for index in first..=limit {
        let shade = if input.color == 3 {
            48 - 3 * index
        } else {
            32 - 2 * index
        };
        bands.push([
            bottom.wrapping_sub(step),
            left,
            bottom,
            right,
            i16::from(input.color),
            shade,
        ]);
        bottom = bottom.wrapping_sub(step);
    }
    bands
}

/// Complete CODE 6 +0x415a: three aiming marks around the projection center.
/// The rendering service receives these segments with the special color binding.
pub fn crosshair_lines(center: [i16; 2], hidden: bool) -> Vec<[i16; 4]> {
    if hidden {
        return vec![];
    }
    let [x, y] = center;
    vec![
        [x.wrapping_sub(55), y, x.wrapping_sub(40), y],
        [x.wrapping_add(55), y, x.wrapping_add(40), y],
        [x, y.wrapping_sub(55), x, y.wrapping_sub(40)],
    ]
}

/// CODE 33 +746..+87e, excluding marker initialization at +83a.
/// Rectangles use QuickDraw top/left/bottom/right; radar coordinates are viewport-local.
#[derive(Debug, serde::Serialize)]
pub struct ViewportLayout {
    pub metrics: ViewportMetrics,
    pub world: [i16; 4],
    pub radar_center: [i16; 2],
    pub radar: [i16; 4],
    pub instruments: [i16; 4],
}

pub fn viewport_layout(bounds: [i16; 4], limits: [i16; 2]) -> ViewportLayout {
    let metrics = viewport_metrics(bounds, limits);
    let left = bounds[1].wrapping_add(metrics.offset[0]);
    let top = bounds[0].wrapping_add(metrics.offset[1]);
    let world = [
        top,
        left,
        top.wrapping_add(metrics.size[1]),
        left.wrapping_add(metrics.size[0]),
    ];
    let radar_center = [metrics.center[0].wrapping_add(194), 62];
    let radar = [
        2,
        radar_center[0].wrapping_sub(60),
        122,
        radar_center[0].wrapping_add(60),
    ];
    let middle = left.wrapping_add(metrics.center[0]);
    let instruments = [
        top.wrapping_sub(80),
        middle.wrapping_sub(256),
        top,
        middle.wrapping_add(256),
    ];
    ViewportLayout {
        metrics,
        world,
        radar_center,
        radar,
        instruments,
    }
}

/// CODE 17 +43c..+4d4, including modeled QuickDraw InsetRect arithmetic.
#[derive(Debug, serde::Serialize)]
pub struct DisplayRectangle {
    /// Extents are saved before the 80-pixel top adjustment.
    pub size: [i32; 2],
    /// QuickDraw top/left/bottom/right, after the top adjustment.
    pub bounds: [i16; 4],
}

pub fn display_rectangle(mut bounds: [i16; 4], full_size: u8) -> DisplayRectangle {
    let setup = display_rectangle_setup(bounds, full_size);
    if let Some([dx, dy]) = setup.inset {
        bounds[0] = bounds[0].wrapping_add(dy);
        bounds[1] = bounds[1].wrapping_add(dx);
        bounds[2] = bounds[2].wrapping_sub(dy);
        bounds[3] = bounds[3].wrapping_sub(dx);
    }
    let size = [
        i32::from(bounds[3].wrapping_sub(bounds[1])),
        i32::from(bounds[2].wrapping_sub(bounds[0])),
    ];
    bounds[0] = bounds[0].wrapping_add(80);
    DisplayRectangle { size, bounds }
}

#[cfg(test)]
mod display_layout_tests {
    use super::*;

    #[test]
    fn compact_display_preserves_odd_pixels_and_single_instrument_strip() {
        let compact = display_rectangle([0, 0, 481, 641], 0);
        assert_eq!(compact.size, [513, 343]);
        assert_eq!(compact.bounds, [149, 64, 412, 577]);
        let layout = viewport_layout([0, 0, 481, 641], compact.size.map(|v| v as i16));
        assert_eq!(layout.world, [109, 64, 452, 577]);
        assert_eq!(layout.instruments, [29, 64, 109, 576]);
        let full = display_rectangle([0, 0, 480, 640], 1);
        assert_eq!(full.size, [640, 480]);
        assert_eq!(full.bounds, [80, 0, 480, 640]);
        let layout = viewport_layout([0, 0, 480, 640], full.size.map(|v| v as i16));
        assert_eq!(layout.world, [80, 0, 480, 640]);
        assert_eq!(layout.instruments, [0, 64, 80, 576]);
    }
}

/// Complete CODE 11 +1f22. OffsetRect is modeled with wrapping word arithmetic.
#[derive(Debug, serde::Serialize)]
pub struct DrawingViewport {
    pub bounds: [i16; 4],
    pub size: [i16; 2],
    pub origin: [i16; 2],
}

pub fn drawing_viewport(
    rect: [i16; 4],
    screen_origin: [i16; 2],
    depth: i16,
    previous_origin: [i16; 2],
) -> DrawingViewport {
    let bounds = std::array::from_fn(|i| {
        rect[i].wrapping_sub(screen_origin[if i % 2 == 0 { 1 } else { 0 }])
    });
    DrawingViewport {
        size: [
            bounds[3].wrapping_sub(bounds[1]),
            bounds[2].wrapping_sub(bounds[0]),
        ],
        origin: if depth == 1 {
            previous_origin
        } else {
            [bounds[1], bounds[0]]
        },
        bounds,
    }
}

/// CODE 17 +39c6 axis-aligned branches, before the rectangle painter.
/// Returns no rectangle for suppressed, nonpositive-pen, or unsupported diagonal lines.
/// Endpoints are XY, pen is width/height, output is top/left/bottom/right.
pub fn axis_line_rectangle(
    endpoints: [[i16; 2]; 2],
    pen: [i16; 2],
    suppressed: bool,
) -> Option<[i16; 4]> {
    if suppressed || pen.iter().any(|v| *v <= 0) {
        return None;
    }
    let [[x0, y0], [x1, y1]] = endpoints;
    if y0 == y1 {
        Some([
            y0,
            x0.min(x1),
            y0.wrapping_add(pen[1]),
            x0.max(x1).wrapping_add(pen[0]),
        ])
    } else if x0 == x1 {
        Some([
            y0.min(y1),
            x0,
            y0.max(y1).wrapping_add(pen[1]),
            x0.wrapping_add(pen[0]),
        ])
    } else {
        None
    }
}

/// Immediate surface presentation: CODE 17 +36c2..36da, +3774..3784,
/// +37a0..37ca, and destination row arithmetic at +1c20..1c5a.
/// QuickDraw coordinate conversions use an explicitly supplied port origin.
#[derive(Debug, serde::Deserialize)]
pub struct SurfaceBlitInput {
    pub global_clip: [i16; 4],
    pub surface_origin: [i16; 2],
    pub port_origin: [i16; 2],
    pub pixmap_origin: [i16; 2],
    pub base: u32,
    pub stride: i16,
}

#[derive(Debug, serde::Serialize)]
pub struct SurfaceBlitGeometry {
    pub local_clip: [i16; 4],
    pub destination_offset: [i16; 2],
    /// Address of the first destination row, before its horizontal pixel offset.
    pub row_address: u32,
}

pub fn surface_blit_geometry(input: SurfaceBlitInput) -> SurfaceBlitGeometry {
    let global_origin: [i16; 2] =
        std::array::from_fn(|i| input.surface_origin[i].wrapping_add(input.port_origin[i]));
    let destination_offset =
        std::array::from_fn(|i| global_origin[i].wrapping_sub(input.pixmap_origin[i]));
    let local_clip = std::array::from_fn(|i| {
        input.global_clip[i].wrapping_sub(global_origin[if i % 2 == 0 { 1 } else { 0 }])
    });
    let row = local_clip[0].wrapping_add(destination_offset[1]);
    SurfaceBlitGeometry {
        local_clip,
        destination_offset,
        row_address: input
            .base
            .wrapping_add((i32::from(row) * i32::from(input.stride)) as u32),
    }
}

#[cfg(test)]
mod surface_presentation_tests {
    use super::*;

    #[test]
    fn compact_surface_placement_differs_from_deferred_viewport() {
        let screen = [0, 0, 480, 640];
        let display = display_rectangle(screen, 0);
        let viewport = viewport_layout(screen, display.size.map(|v| v as i16));
        let surface = surface_blit_geometry(SurfaceBlitInput {
            global_clip: display.bounds,
            surface_origin: [display.bounds[1], display.bounds[0]],
            port_origin: [0, 0],
            pixmap_origin: [0, 0],
            base: 0x100000,
            stride: 640,
        });
        assert_eq!(surface.local_clip, [0, 0, 262, 512]);
        assert_eq!(surface.destination_offset, [64, 149]);
        assert_eq!(surface.row_address, 0x100000 + 149 * 640);
        assert_eq!(viewport.metrics.center, [256, 171]);
        assert_eq!(viewport.world[0] + viewport.metrics.center[1], 280);
        assert_eq!(
            surface.destination_offset[1] + viewport.metrics.center[1],
            320
        );
    }
}

/// Complete CODE 6 +342: nonlinear radar projection of packed planar offsets.
/// Distance -1 requests a squared-distance calculation; all coordinates are XY.
pub fn radar_point(
    position: [i16; 2],
    distance: i16,
    center: [i16; 2],
    threshold: i32,
    scale_base: i32,
    falloff: i16,
) -> [i16; 2] {
    let distance = if distance == -1 {
        let [x, y] = position.map(i32::from);
        (x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) >> 16) as i16
    } else {
        distance
    };
    let mut scale = 614_i16;
    if i32::from(distance) > threshold {
        let excess = i32::from(distance).wrapping_sub(threshold) as i16;
        let reduction = ((i32::from(excess) * i32::from(falloff)) as u32 >> 8) as i32;
        let adjusted = scale_base.wrapping_sub(reduction) as i16;
        scale = ((i32::from(scale.wrapping_mul(2)) * i32::from(adjusted)) >> 16) as i16;
    }
    std::array::from_fn(|i| {
        center[i].wrapping_sub(((i32::from(position[i]) * i32::from(scale)) >> 16) as i16)
    })
}

/// Complete CODE 6 +c4: three pixels at each cardinal edge of the radar.
pub fn radar_edge_points(center: [i16; 2]) -> [[i16; 2]; 12] {
    let [x, y] = center;
    std::array::from_fn(|i| {
        let offset = (i % 3) as i16 - 1;
        match i / 3 {
            0 => [x.wrapping_sub(60), y.wrapping_add(offset)],
            1 => [x.wrapping_add(60), y.wrapping_add(offset)],
            2 => [x.wrapping_add(offset), y.wrapping_sub(60)],
            _ => [x.wrapping_add(offset), y.wrapping_add(60)],
        }
    })
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AiCooldown {
    pub kind: i8,
    pub behavior: u8,
    pub movement: u8,
    pub deadline: i32,
    pub now: i32,
    pub distance: i32,
    pub local_dying: u8,
}

/// Complete CODE 20 +fb8 cooldown handler, excluding terminal sound playback.
/// Returns behavior, movement, next deadline, and optional channel-one sound.
pub fn ai_cooldown(input: AiCooldown) -> (u8, u8, i32, Option<u32>) {
    if input.now < input.deadline {
        return (input.behavior, input.movement, input.deadline, None);
    }
    // The original uses XOR 1 and AND with Scc's 0xff, not a boolean cast.
    let sound = if input.kind == 23 && input.local_dying ^ 1 != 0 {
        Some(if input.distance <= 400 << 16 {
            0x10014
        } else {
            0x10013
        })
    } else {
        None
    };
    (0, 0, input.now.wrapping_add(20), sound)
}

/// CODE 20 +12f8..+132a, reached by the kind-23/kind-24 tracking branch.
/// Returns behavior, movement mode, and the shared phase deadline.
pub fn ai_surface_phase(now: i32, behavior: u8, movement: u8, mut deadline: i32) -> (u8, u8, i32) {
    if deadline == -1 {
        deadline = now;
    }
    if now >= deadline {
        (5, 3, now.wrapping_add(50))
    } else {
        (behavior, movement, deadline)
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct AiSurfaceTracking {
    pub participant: i8,
    pub local: i8,
    pub game_mode: u8,
    pub now: i32,
}

/// Complete CODE 20 +1240 path for kind 23 or 24.
/// The common prefix matches kind 7; the surface phase follows a pose request.
pub fn ai_surface_tracking(
    input: AiSurfaceTracking,
    states: &mut [AiTrackingParticipant; 12],
    phase_deadline: &mut i32,
    select: impl FnMut(i8, i8, u8) -> i8,
    cached_pose: impl FnMut(i8, &AiIntentState) -> [i32; 3],
) -> Option<bool> {
    let called = ai_type7_tracking(
        input.participant,
        input.local,
        input.game_mode,
        states,
        select,
        cached_pose,
    )?;
    if called {
        let state = &mut states[input.participant as usize - 1].intent;
        (state.behavior, state.movement, *phase_deadline) =
            ai_surface_phase(input.now, state.behavior, state.movement, *phase_deadline);
    }
    Some(called)
}

/// Complete mode-zero CODE 20 +b6c transition for kinds 23/24.
/// These kinds skip object selection; the acquired opponent is supplied.
pub fn ai_initialize_surface(
    state: &mut AiTrackingParticipant,
    last_check: &mut i32,
    opponent: i8,
    cached_pose: impl FnMut(&AiIntentState) -> [i32; 3],
) -> bool {
    ai_initialize_type7(state, last_check, opponent, 0, 0, 0, cached_pose)
}

#[derive(Debug, serde::Serialize)]
pub struct AcidContact {
    pub deadline: i32,
    pub sound: (i16, u32),
    pub damage: i16,
    pub timestamp: i32,
    pub attacker: i8,
    pub victim: i8,
    pub network: bool,
}

/// Complete CODE 7 +1096 through +1116, up to terminal sound/damage services.
/// The resource damage word and contacted object's owner byte are supplied.
pub fn acid_contact(
    now: i32,
    deadline: i32,
    mode: u8,
    damage: i16,
    attacker: i8,
    victim: i8,
) -> Option<AcidContact> {
    if now <= deadline {
        return None;
    }
    Some(AcidContact {
        deadline: now.wrapping_add(150),
        sound: (1, 0x1001d),
        damage,
        timestamp: now,
        attacker,
        victim,
        network: mode != 0,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct HandlingContact {
    pub adjustment: i8,
    pub deadline: i32,
    pub sound: Option<(i16, u32)>,
}

/// CODE 31 +11e6..121c and +1240..1258 refill blocks, including capacity helpers.
/// This does not select pickup subtypes or perform their later relocation/effects.
pub fn dump_refill(
    health: i16,
    ammo: i16,
    shields: i8,
    ammo_setting: i8,
    full: bool,
) -> (i16, i16) {
    let (max_health, max_ammo) = wave_refill(shields, ammo_setting);
    if full {
        (max_health, max_ammo)
    } else {
        (
            health.wrapping_add(1).min(max_health),
            ammo.wrapping_add(20).min(max_ammo),
        )
    }
}

/// Complete CODE 25 +0224, with the Mac Random word supplied.
pub fn dump_subtype(random: i16, weights: [i8; 3]) -> i16 {
    let roll = random.wrapping_abs() % 10 + 1;
    let first = weights[0] as i16;
    let second = first + weights[1] as i16;
    let third = second + weights[2] as i16;
    if roll <= first {
        0
    } else if roll <= second {
        1
    } else if roll <= third {
        2
    } else {
        3
    }
}

/// CODE 25 +1654..166c defaults and +1876..18ae type-5 resource override.
/// The fourth byte is preserved even though +0224 reads only the first three.
pub fn dump_weights(rows: &[crate::formats::Beastie]) -> [i8; 4] {
    let mut weights = [5, 2, 1, 2];
    for row in rows {
        if row.kind as i8 == 5 {
            weights = [
                row.hits_to_kill as i8,
                row.collision_damage as i8,
                row.attack_range as i8,
                row.short_firing_range as i8,
            ];
        }
    }
    weights
}

/// CODE 31 +1416..14b8 relocation loop. Collision and OS randomness are supplied.
/// A wrapped zero divisor has no valid original result and returns None.
pub fn relocate_dump(
    pose: [i32; 3],
    extent: i16,
    mut random: impl FnMut() -> i16,
    mut clear: impl FnMut([i32; 3]) -> bool,
) -> Option<([i32; 3], u16)> {
    let divisor = extent.wrapping_sub(200) as i32;
    if divisor == 0 {
        return None;
    }
    let mut candidate = pose;
    for attempt in 1..=51 {
        for coordinate in &mut candidate[..2] {
            let numerator = random().wrapping_abs() as i32;
            let quotient = numerator / divisor;
            // DIVS leaves its destination unchanged on quotient overflow.
            // The following SWAP then exposes the original high word.
            let remainder = if i16::try_from(quotient).is_ok() {
                numerator % divisor
            } else {
                numerator >> 16
            };
            *coordinate = (remainder as i16).wrapping_add(100) as i32 * 65536;
        }
        if attempt > 50 || clear(candidate) {
            return Some((candidate, attempt));
        }
    }
    unreachable!()
}

/// Complete CODE 7 +132a/+1360 ice/mud handlers, excluding sound playback.
/// Moving mud dispatches to the same handler as static mud.
pub fn handling_contact(kind: u8, now: i32, deadline: i32, mud: i8) -> Option<HandlingContact> {
    let (adjustment, interval, request) = match kind {
        17 => (-6, 6000, 0x10024),
        18 | 24 => (mud, 60, 0x10023),
        _ => return None,
    };
    let due = now >= deadline;
    Some(HandlingContact {
        adjustment,
        deadline: if due {
            now.wrapping_add(interval)
        } else {
            deadline
        },
        sound: due.then_some((1, request)),
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct DumpEffectInput {
    pub subtype: i16,
    pub health: i16,
    pub ammo: i16,
    pub shields: i8,
    pub ammo_setting: i8,
    pub vertical: i32,
    pub local_energy: i16,
    pub mode: u8,
}

#[derive(Debug, serde::Serialize)]
pub struct DumpEffect {
    pub health: i16,
    pub ammo: i16,
    pub local_energy: i16,
    pub sound: Option<(i16, u32)>,
    /// (network route, height word), passed to the original type-7 effect service.
    pub effect: Option<(bool, i16)>,
}

/// CODE 31 +11d6..13da, excluding terminal sound and effect services.
/// The energy field belongs to the local participant even for a remote collector.
pub fn dump_effect(input: &DumpEffectInput) -> DumpEffect {
    let (health, ammo) = dump_refill(
        input.health,
        input.ammo,
        input.shields,
        input.ammo_setting,
        matches!(input.subtype, 1 | 2),
    );
    let mut result = DumpEffect {
        health,
        ammo,
        local_energy: input.local_energy,
        sound: None,
        effect: None,
    };
    match input.subtype {
        0 => result.sound = Some((1, 0x1000b)),
        1 => result.sound = Some((1, 0x1001e)),
        2 => {
            result.sound = Some((1, 0x1001a));
            result.effect = Some((input.mode != 0, (input.vertical / 65536) as i16));
        }
        _ if input.vertical >= 0 => {
            result.local_energy = input.local_energy.wrapping_add(400).min(2000);
            result.sound = Some((1, 0x10010));
        }
        _ => {}
    }
    result
}

pub struct DumpCreation {
    pub extent: i16,
    pub kind: i8,
    pub now: i32,
    pub weights: [i8; 3],
    pub callback: u32,
}

/// Complete CODE 25 +029c..0404 for nonnegative object counts.
/// None denotes invalid extent or an exhausted host Random source, not success.
/// Unlike later relocation, the original initial placement has no retry limit.
pub fn create_dump_record(
    record: &mut [u8; 128],
    existing: &[[i32; 2]],
    input: &DumpCreation,
    mut random: impl FnMut() -> Option<i16>,
) -> Option<bool> {
    if existing.len() >= 50 {
        return Some(false);
    }
    let divisor = input.extent.wrapping_sub(200) as i32;
    if divisor == 0 {
        return None;
    }
    fn put_word(record: &mut [u8; 128], offset: usize, value: i16) {
        record[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }
    fn put_long(record: &mut [u8; 128], offset: usize, value: i32) {
        record[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    record[0x1a] = 0;
    put_word(record, 0x1c, 0);
    record[0x14] = input.kind as u8;
    loop {
        let mut point = [0; 2];
        for (axis, coordinate) in point.iter_mut().enumerate() {
            let numerator = random()?.wrapping_abs() as i32;
            let quotient = numerator / divisor;
            let remainder = if i16::try_from(quotient).is_ok() {
                numerator % divisor
            } else {
                numerator >> 16
            };
            *coordinate = (remainder as i16).wrapping_add(100) as i32 * 65536;
            put_long(record, axis * 4, *coordinate);
        }
        let near = existing.iter().any(|other| {
            let distance = other[0]
                .wrapping_sub(point[0])
                .wrapping_abs()
                .wrapping_add(other[1].wrapping_sub(point[1]).wrapping_abs());
            ((distance >> 16) as i16) < 40
        });
        if !near {
            break;
        }
    }
    put_long(record, 8, (random()?.wrapping_abs() % 360) as i32 * 65536);
    put_long(record, 0x10, input.now);
    put_word(record, 0xe, 1);
    for offset in [0xc, 0x22, 0x3c] {
        put_word(record, offset, 0);
    }
    for offset in [0x1b, 0x5b] {
        record[offset] = 0;
    }
    put_long(record, 0x16, 2_000_000_000);
    for offset in [0x28, 0x24, 0x2c, 0x34] {
        put_long(record, offset, 0);
    }
    put_long(record, 0x48, input.callback as i32);
    put_long(record, 0x38, input.now);
    if input.kind == 5 {
        put_word(record, 0x1c, dump_subtype(random()?, input.weights));
    }
    Some(true)
}

/// CODE 19 +2276: animated kind-5 packed vertices. The caller supplies now % 60.
/// This routine changes vertices, not cached collision dimensions.
pub fn dump_animation(phase: i16) -> [[i16; 3]; 4] {
    let a = (-72 * phase.wrapping_add(90) as i32 / 220) as i16;
    let b = a.wrapping_neg();
    [[a, 0, a], [a, 0, b], [b, 0, b], [b, 0, a]]
}

/// CODE 30 +154c..1668. Returns hover height, fixed vertical position, and hold byte.
/// A zero speed setting traps in the original active-flight branch.
pub fn flight_height(
    height: i32,
    vertical: i32,
    hold: i8,
    active: bool,
    speed: i8,
    setting: i8,
    clocks: [i32; 2],
) -> Option<(i32, i32, i8)> {
    let [now, elapsed] = clocks;
    if hold != 0 {
        let hold = (hold as i16 - 1).max(0) as i8;
        return Some((
            height,
            vertical.wrapping_add(height.wrapping_mul(65536)),
            hold,
        ));
    }
    let mut next = height;
    if active {
        if setting == 0 {
            return None;
        }
        let target = 200_i32
            .wrapping_sub((speed as i16).abs() as i32 * (160 / setting as i32))
            .wrapping_add((now & 127) - 64);
        if target >= next {
            next = next.wrapping_add(21_i32.wrapping_mul(elapsed) / 7);
        }
    } else if next <= 0 {
        return Some((next, vertical, hold));
    }
    next = next.wrapping_sub(19_i32.wrapping_mul(elapsed) / 13).max(0);
    Some((next, vertical.wrapping_add(next.wrapping_mul(65536)), hold))
}

/// CODE 33 +da..172: local energy clocks and flight-enabled flag.
/// The original truncates intermediate clock results to signed words.
pub fn energy_clock(
    flight: i16,
    hyperspace: i16,
    paused: bool,
    now: i32,
    previous: i32,
) -> (i16, i16, bool) {
    if paused {
        return (flight, hyperspace, flight > 0);
    }
    let elapsed = now.wrapping_sub(previous);
    let hyperspace = (elapsed.wrapping_add(hyperspace as i32) as i16).min(2000);
    let cost = ((elapsed / 4) as i16).max(1);
    let flight = flight.wrapping_sub(cost).max(0);
    (flight, hyperspace, flight > 0)
}

/// Shared vertical state around CODE 30 +145a..1668.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VerticalState {
    pub transition: i32,
    pub camera_scale: i32,
    pub renderer_height: i16,
    pub hover: i32,
    pub hold: i8,
    pub vertical: i32,
    pub active: bool,
}

impl Default for VerticalState {
    fn default() -> Self {
        Self {
            transition: 0,
            camera_scale: 4 * 65536,
            renderer_height: 32,
            hover: 0,
            hold: 0,
            vertical: 0,
            active: false,
        }
    }
}

/// Original transition clamps followed by the recovered hover update.
pub fn update_vertical(
    state: &mut VerticalState,
    energy: i16,
    speed: i8,
    setting: i8,
    clocks: [i32; 2],
) -> Option<()> {
    state.active = energy > 0;
    if state.transition != 0 {
        if state.transition < 0 {
            state.active = false;
            state.hover = 0;
        }
        state.camera_scale = state.camera_scale.wrapping_add(state.transition);
        state.renderer_height = state
            .renderer_height
            .wrapping_add((state.transition.wrapping_shl(3) / 65536) as i16);
        if state.transition > 0 && state.camera_scale > 4 * 65536 {
            state.camera_scale = 4 * 65536;
            state.renderer_height = 32;
            state.transition = 0;
        } else if state.transition < 0 && state.camera_scale < -12 * 65536 {
            state.camera_scale = -12 * 65536;
            state.renderer_height = -96;
            state.transition = 0;
        }
    }
    let base = state.renderer_height.wrapping_sub(32) as i32 * 65536;
    let (hover, vertical, hold) = flight_height(
        state.hover,
        base,
        state.hold,
        state.active,
        speed,
        setting,
        clocks,
    )?;
    state.hover = hover;
    state.vertical = vertical;
    state.hold = hold;
    Some(())
}

#[cfg(test)]
mod flight_state_tests {
    use super::*;

    #[test]
    fn rollback_recomputes_speed_instead_of_restoring_a_snapshot() {
        let mut state = Type6Motion {
            pose: [100 << 16, 100 << 16, 90 << 16],
            speed: 5,
            ramp: 0,
            vertical: VerticalState::default(),
        };
        let before = state.clone();
        let mut input = Type6Step {
            commands: [0, 1],
            limit: 5,
            energy: 0,
            turn_rate: 65536,
            now: 64,
            elapsed: 1,
        };
        let table = [65536; 360];
        type6_step(&mut state, &input, &table).unwrap();
        assert_eq!(state.speed, 5, "forward acceleration reaches the speed cap");
        input.elapsed = -1;
        type6_step(&mut state, &input, &table).unwrap();
        assert_eq!(state.pose, before.pose);
        assert_eq!(
            state.speed, 4,
            "rollback applies negative-time speed update"
        );
    }

    #[test]
    fn energy_and_altitude_sequence_takes_off_then_lands() {
        let mut state = VerticalState {
            transition: 0,
            camera_scale: 4 * 65536,
            renderer_height: 32,
            hover: 0,
            hold: 0,
            vertical: 0,
            active: false,
        };
        let mut energy = 400;
        let mut airborne = false;
        for now in 1..800 {
            let (next, _, _) = energy_clock(energy, 2000, false, now, now - 1);
            energy = next;
            update_vertical(&mut state, energy, 0, 5, [now, 1]).unwrap();
            airborne |= state.vertical > 0;
            assert_eq!(state.vertical, state.hover * 65536);
            assert_eq!(state.renderer_height, 32);
            if now == 399 {
                assert!(state.active && state.hover > 0);
            }
            if now == 400 {
                assert!(
                    !state.active && state.hover > 0,
                    "depletion starts descent without teleporting to ground"
                );
            }
        }
        assert!(airborne);
        assert_eq!(energy, 0);
        assert_eq!(state.vertical, 0);
        assert_eq!(state.hover, 0);
    }
}

/// CODE 7 +1be6..1d1e: solid-contact record changes after movement rollback.
/// Height adjustment is three raw fixed-point bits, not three world units.
pub fn contact_rebound(record: &mut [u8; 128], boundary: bool, other_height: i32) {
    let height = i32::from_be_bytes(record[0x28..0x2c].try_into().unwrap());
    if !boundary && height > 0 {
        record[0x5a] = 30;
        let height = if height > other_height {
            height.wrapping_add(3).max(0)
        } else {
            height.wrapping_sub(3)
        };
        record[0x28..0x2c].copy_from_slice(&height.to_be_bytes());
    }
    for offset in [0xe, 0xc] {
        let value = i16::from_be_bytes(record[offset..offset + 2].try_into().unwrap());
        record[offset..offset + 2].copy_from_slice(&value.wrapping_neg().to_be_bytes());
    }
    record[0x22] = (record[0x22] as i8).wrapping_neg() as u8;
}

/// CODE 7 +1adc..1be6 in single-player mode, before record rebound.
/// Returns rollback elapsed time and the optional local sound request.
/// The caller must run the contact notification service before rollback.
pub fn solid_response_request(
    special: u8,
    contact: i16,
    hold: u8,
    local: bool,
    elapsed: i32,
) -> Option<(i32, Option<(i16, u32)>)> {
    if special != 0 || (contact != -999 && hold != 0) {
        return None;
    }
    Some((
        elapsed.wrapping_neg(),
        local.then_some((1, if contact == -999 { 0x20012 } else { 0x20007 })),
    ))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Type6Motion {
    pub pose: [i32; 3],
    pub speed: i8,
    pub ramp: i8,
    pub vertical: VerticalState,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Type6Step {
    pub commands: [i16; 2],
    pub limit: i8,
    pub energy: i16,
    pub turn_rate: i32,
    pub now: i32,
    pub elapsed: i32,
}

/// CODE 30 +1d54 type-6 dispatch, local single-player, coast/adjustment zero.
/// Spatial-index maintenance is the caller's responsibility.
pub fn type6_step(state: &mut Type6Motion, input: &Type6Step, table: &[i32; 360]) -> Option<()> {
    if input.elapsed == 0 {
        return Some(());
    }
    let delta = (input.commands[0] as i32)
        .wrapping_mul(input.turn_rate)
        .wrapping_mul(input.elapsed);
    if input.elapsed > 0 {
        (state.pose[2], state.ramp) = turn_object(
            state.pose[2],
            state.ramp,
            6,
            input.commands[0],
            input.elapsed,
            delta,
        );
    }
    let direction = movement_direction(state.pose[2], table)?;
    let (position, result) = move_type6_planar(
        [state.pose[0], state.pose[1]],
        direction,
        SpeedUpdate {
            speed: state.speed,
            command: input.commands[1],
            elapsed: input.elapsed,
            limit: input.limit,
            coast: 0,
            adjustment: 0,
            mode: 0,
            random: 0,
        },
    );
    state.speed = result.speed;
    update_vertical(
        &mut state.vertical,
        input.energy,
        state.speed,
        input.limit,
        [input.now, input.elapsed],
    )?;
    state.pose[..2].copy_from_slice(&position);
    if input.elapsed < 0 {
        (state.pose[2], state.ramp) = turn_object(
            state.pose[2],
            state.ramp,
            6,
            input.commands[0],
            input.elapsed,
            delta,
        );
    }
    Some(())
}

#[derive(Clone, Debug)]
pub struct TransitEffect {
    pub transition: i32,
    pub energy: i16,
    pub sound: Option<(i16, u32)>,
}

/// Complete CODE 7 +125c/+12b8 transit handlers, excluding sound playback.
pub fn transit_contact(
    kind: u8,
    transition: i32,
    vertical: i32,
    energy: i16,
) -> Option<TransitEffect> {
    let (transition, energy, sound) = match kind {
        36 if transition != 65536 && vertical < 0 => (65536, energy, Some((1, 0x1000c))),
        37 if transition != -65536 => (-65536, 0, Some((1, 0x10029))),
        36 | 37 => (transition, energy, None),
        _ => return None,
    };
    Some(TransitEffect {
        transition,
        energy,
        sound,
    })
}
