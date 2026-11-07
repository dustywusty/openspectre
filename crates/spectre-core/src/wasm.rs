//! Small explicit ABI. JavaScript only handles files, controls, and pixel display.
use crate::viewer::Viewer;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    input: Vec<u8>,
    output: Vec<u8>,
    viewer: Viewer,
    deathmatch: Option<crate::game::deathmatch::Match>,
    controls: [crate::game::Input; 2],
    prediction: Option<crate::game::Game>,
    prediction_alive: bool,
}
thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }

#[unsafe(no_mangle)]
pub extern "C" fn input_buffer(len: u32) -> *mut u8 {
    if len > 32 * 1024 * 1024 {
        return std::ptr::null_mut();
    }
    STATE.with_borrow_mut(|s| {
        s.input.resize(len as usize, 0);
        s.input.as_mut_ptr()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn load_shared_colors() -> u32 {
    STATE.with_borrow_mut(|s| match s.viewer.load_shared_colors(&s.input) {
        Ok(()) => {
            s.output = b"Shared colors loaded".to_vec();
            1
        }
        Err(e) => {
            s.output = e.into_bytes();
            0
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn load_archive() -> u32 {
    STATE.with_borrow_mut(|s| match s.viewer.load(&s.input) {
        Ok(()) => {
            s.output = serde_json::to_vec(&s.viewer.entries).unwrap();
            1
        }
        Err(e) => {
            s.output = e.into_bytes();
            0
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    STATE.with_borrow(|s| s.output.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn output_len() -> u32 {
    STATE.with_borrow(|s| s.output.len() as u32)
}
#[unsafe(no_mangle)]
pub extern "C" fn render(index: u32, yaw: f32, pitch: f32, zoom: f32, filled: u32) -> *const u8 {
    STATE.with_borrow_mut(|s| {
        s.viewer.draw(index as usize, yaw, pitch, zoom, filled != 0);
        s.viewer.frame.rgba.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn game_start(index: u32) -> u32 {
    start_arena(index, false)
}
#[unsafe(no_mangle)]
pub extern "C" fn game_start_mission(index: u32) -> u32 {
    start_arena(index, true)
}
#[unsafe(no_mangle)]
pub extern "C" fn game_start_campaign_level(level: i32, difficulty: i32) -> u32 {
    STATE.with_borrow_mut(|s| {
        let result = match (i16::try_from(level), i16::try_from(difficulty)) {
            (Ok(level), Ok(difficulty)) => s.viewer.start_campaign_level(level, difficulty),
            _ => Err("Campaign input exceeds signed word range".into()),
        };
        match result {
            Ok(()) => {
                let game = s.viewer.game.as_ref().unwrap();
                s.output = serde_json::to_vec(&serde_json::json!({
                    "campaign":s.viewer.campaign,"flags":game.flags,"scenario":game.scenario,
                    "targets":game.targets,"par_seconds":game.par_seconds,"dump_weights":game.dump_weights,"optional_weapons":game.optional_weapons,"pickups":game.pickups,"pickups_collected":game.pickups_collected,"flight_energy":game.flight_energy,"vertical":game.vertical,"pending_shields":game.pending_shields,"radar_glyph_count":game.radar_glyphs.as_ref().map_or(0, Vec::len),"palette":game.palette,"ground_color_index":game.ground_color_index,
                }))
                .unwrap();
                1
            }
            Err(e) => {
                s.output = e.into_bytes();
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn game_next_campaign_level() -> u32 {
    STATE.with_borrow_mut(|s| match s.viewer.next_campaign_level() {
        Ok(()) => {
            let game = s.viewer.game.as_ref().unwrap();
            s.output = serde_json::to_vec(&serde_json::json!({
                "campaign":s.viewer.campaign,"scenario":game.scenario,"targets":game.targets,
            }))
            .unwrap();
            1
        }
        Err(e) => {
            s.output = e.into_bytes();
            0
        }
    })
}
fn start_arena(index: u32, flags: bool) -> u32 {
    STATE.with_borrow_mut(|s| {
        match if flags {
            s.viewer.start_mission(index as usize)
        } else {
            s.viewer.start_game(index as usize)
        } {
            Ok(()) => {
                let game = s.viewer.game.as_ref().unwrap();
                s.output = serde_json::to_vec(&serde_json::json!({
                    "items": game.items,
                    "scenario":game.scenario,
                    "primary_damage":game.primary_damage,
                    "targets":game.targets,
                    "resolved": game.items.iter().filter(|i| i.shape.is_some()).count(),
                    "unresolved": game.items.iter().filter(|i| i.shape.is_none()).count()
                }))
                .unwrap();
                1
            }
            Err(e) => {
                s.output = e.into_bytes();
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn game_replay_type31_event() -> u32 {
    STATE.with_borrow_mut(|s| {
        s.viewer
            .game
            .as_mut()
            .is_some_and(|game| game.replay_type31_event()) as u32
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn game_reset() {
    STATE.with_borrow_mut(|s| {
        if let Some(game) = &mut s.viewer.game {
            game.reset();
        }
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn game_frame(
    seconds: f64,
    throttle: f32,
    steer: f32,
    brake: u32,
    fire: u32,
) -> *const u8 {
    game_frame_controls(seconds, throttle, steer, brake, fire, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn game_frame_controls(
    seconds: f64,
    throttle: f32,
    steer: f32,
    brake: u32,
    fire: u32,
    select_weapon: u32,
) -> *const u8 {
    game_frame_weapons(seconds, throttle, steer, brake, fire, select_weapon, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn game_frame_weapons(
    seconds: f64,
    throttle: f32,
    steer: f32,
    brake: u32,
    fire: u32,
    select_weapon: u32,
    fire_optional: u32,
) -> *const u8 {
    STATE.with_borrow_mut(|s| {
        s.viewer.drive(
            seconds,
            crate::game::Input {
                throttle,
                steer,
                brake: brake != 0,
                fire: fire != 0,
                select_weapon: select_weapon != 0,
                fire_optional: fire_optional != 0,
            },
        );
        s.viewer.draw_game();
        if let Some(game) = &s.viewer.game {
            s.output = serde_json::to_vec(&serde_json::json!({"campaign":s.viewer.campaign,"vehicle_settings":game.vehicle_settings,"mission":game.mission,"flags":game.flags,"tank":game.tank,"shots":game.shots,"impacts":game.impacts,"fired":game.fired,"combat_counters":game.combat_counters,"par_seconds":game.par_seconds,"dump_weights":game.dump_weights,"optional_weapons":game.optional_weapons,"pickups":game.pickups,"pickups_collected":game.pickups_collected,"flight_energy":game.flight_energy,"vertical":game.vertical,"pending_shields":game.pending_shields,"completion":game.completion,"ammo":game.ammo,"targets":game.targets,"destroyed":game.destroyed,"enemy_clear_award":game.enemy_clear_award,"collision_mode":game.collision_mode,"audio_events":game.audio_events,"encounter":game.encounter,"death_transition":game.death_transition,"radar_glyph_count":game.radar_glyphs.as_ref().map_or(0, Vec::len),"palette":game.palette,"ground_color_index":game.ground_color_index,"enemy_shots":game.enemy_shots})).unwrap();
        }
        s.viewer.frame.rgba.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn game_infinite_health(enabled: u32) {
    STATE.with_borrow_mut(|s| {
        if let Some(game) = &mut s.viewer.game {
            game.infinite_health = enabled != 0;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn game_camera(mode: u32) {
    STATE.with_borrow_mut(|s| s.viewer.camera = mode.min(2));
}

#[unsafe(no_mangle)]
pub extern "C" fn game_zoom(level: i32) {
    STATE.with_borrow_mut(|s| s.viewer.camera_zoom = level.clamp(-2, 3));
}

#[unsafe(no_mangle)]
pub extern "C" fn game_encounter(enabled: u32) {
    STATE.with_borrow_mut(|s| {
        if let Some(game) = &mut s.viewer.game {
            game.set_encounter(enabled != 0);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dm_start() -> u32 {
    STATE.with_borrow_mut(|s| {
        let mut build = || -> crate::Result<crate::game::deathmatch::Match> {
            s.viewer.start_deathmatch_arena()?;
            let a = s.viewer.game.take().unwrap();
            s.viewer.start_deathmatch_arena()?;
            let b = s.viewer.game.take().unwrap();
            Ok(crate::game::deathmatch::Match::new([a, b]))
        };
        match build() {
            Ok(game) => {
                s.deathmatch = Some(game);
                s.controls = [Default::default(); 2];
                1
            }
            Err(e) => {
                s.output = e.into_bytes();
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_input(slot: u32, throttle: i32, steer: i32, brake: u32, fire: u32) {
    STATE.with_borrow_mut(|s| {
        if let Some(input) = s.controls.get_mut(slot as usize) {
            *input = crate::game::Input {
                throttle: throttle.clamp(-1, 1) as f32,
                steer: steer.clamp(-1, 1) as f32,
                brake: brake != 0,
                fire: fire != 0,
                ..Default::default()
            };
        }
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_step() {
    STATE.with_borrow_mut(|s| {
        if let Some(game) = &mut s.deathmatch {
            game.step(s.controls);
        }
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_snapshot() {
    STATE.with_borrow_mut(|s| {
        if let Some(game) = &mut s.deathmatch {
            s.output = serde_json::to_vec(&game.snapshot()).unwrap();
        }
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_view_start() -> u32 {
    STATE.with_borrow_mut(|s| match s.viewer.start_deathmatch_arena() {
        Ok(()) => 1,
        Err(e) => {
            s.output = e.into_bytes();
            0
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_render(slot: u32) -> *const u8 {
    STATE.with_borrow_mut(|s| {
        let result = serde_json::from_slice::<crate::game::deathmatch::Snapshot>(&s.input)
            .map_err(|e| e.to_string())
            .and_then(|snapshot| s.viewer.draw_deathmatch(&snapshot, slot as usize));
        match result {
            Ok(()) => s.viewer.frame.rgba.as_ptr(),
            Err(e) => {
                s.output = e.into_bytes();
                std::ptr::null()
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dm_prediction_start() -> u32 {
    STATE.with_borrow_mut(|s| {
        let render = s.viewer.game.take();
        let result = s.viewer.start_deathmatch_arena();
        s.prediction = s.viewer.game.take();
        s.viewer.game = render;
        match result {
            Ok(()) => 1,
            Err(e) => {
                s.output = e.into_bytes();
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_predict_restore() -> u32 {
    STATE.with_borrow_mut(|s| {
        let Ok(player) = serde_json::from_slice::<crate::game::deathmatch::Player>(&s.input) else {
            return 0;
        };
        let Some(g) = &mut s.prediction else { return 0 };
        player.motion.restore(g);
        s.prediction_alive = player.health > 0 && player.respawn == 0;
        s.output = serde_json::to_vec(&crate::game::deathmatch::prediction_state(g)).unwrap();
        1
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn dm_predict_step(throttle: i32, steer: i32, brake: u32, fire: u32) {
    STATE.with_borrow_mut(|s| {
        if let Some(g) = &mut s.prediction {
            g.audio_events.clear();
            if s.prediction_alive {
                g.ammo = 999;
                g.step(crate::game::Input {
                    throttle: throttle.clamp(-1, 1) as f32,
                    steer: steer.clamp(-1, 1) as f32,
                    brake: brake != 0,
                    fire: fire != 0,
                    ..Default::default()
                });
            }
            s.output = serde_json::to_vec(&crate::game::deathmatch::prediction_state(g)).unwrap();
        }
    });
}
