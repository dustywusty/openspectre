use spectre_core::{formats, resource::Archive, viewer::Viewer};
use std::{env, fs, path::Path};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn load_optional_colors(viewer: &mut Viewer) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if let Some(index) = args.iter().position(|arg| arg == "--colors") {
        let path = args
            .get(index + 1)
            .ok_or("--colors requires an executable resource file")?;
        viewer.load_shared_colors(&fs::read(path)?)?;
    }
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("");
    let input = args
        .get(2)
        .ok_or("Usage: spectre-tools <inspect|export|render|view|drive> FILE [OUTPUT]")?;
    let bytes = fs::read(input)?;
    match command {
        "replay-scattershot" | "replay-seeker" | "replay-missile" | "replay-grenade" => {
            let mut viewer = Viewer::default();
            viewer.load(&bytes)?;
            viewer.start_campaign_level(
                match command {
                    "replay-seeker" => 13,
                    "replay-missile" => 7,
                    "replay-grenade" => 3,
                    _ => 11,
                },
                0,
            )?;
            for _ in 0..32 {
                viewer.drive(
                    spectre_core::game::STEP,
                    spectre_core::game::Input {
                        fire_optional: true,
                        ..Default::default()
                    },
                );
            }
            let game = viewer.game.as_ref().unwrap();
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "ammo":game.ammo,"fired":game.fired,"shots":game.shots,"tank":game.tank,
                    "audio_events":game.audio_events,"combat_counters":game.combat_counters,
                    "optional_weapons":game.optional_weapons,
                }))?
            );
        }
        "render-missile-model" => {
            let mut viewer = Viewer::default();
            viewer.load(&bytes)?;
            viewer.start_campaign_level(7, 0)?;
            let game = viewer.game.as_mut().unwrap();
            let model = game.missile_model.as_ref().ok_or("Missing missile model")?;
            if model.shape_id != 10001 || model.source != "original resource" {
                return Err("Missile model is not original shape 10001".into());
            }
            // Isolated render fixture avoids scenery occlusion and enemy damage.
            game.items.clear();
            game.targets.clear();
            game.encounter.enabled = false;
            game.flags.enabled = false;
            for _ in 0..24 {
                game.step(spectre_core::game::Input {
                    fire_optional: true,
                    ..Default::default()
                });
            }
            let before = serde_json::to_value(&game.shots)?;
            let directory = args.get(3).ok_or("Render output directory required")?;
            fs::create_dir_all(directory)?;
            for camera in 0..3 {
                viewer.camera = camera;
                viewer.draw_game();
                let pixels = viewer.frame.rgba.clone();
                let model = viewer.game.as_mut().unwrap().missile_model.take();
                viewer.draw_game();
                if pixels == viewer.frame.rgba {
                    return Err(format!("Missile model did not affect camera {camera}").into());
                }
                viewer.game.as_mut().unwrap().missile_model = model;
                let mut ppm = format!(
                    "P6\n{} {}\n255\n",
                    spectre_core::render::WIDTH,
                    spectre_core::render::HEIGHT
                )
                .into_bytes();
                for pixel in pixels.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&pixel[..3]);
                }
                fs::write(
                    Path::new(directory).join(format!("missile-{camera}.ppm")),
                    ppm,
                )?;
            }
            if before != serde_json::to_value(&viewer.game.as_ref().unwrap().shots)? {
                return Err("Rendering mutated projectile state".into());
            }
            println!(
                "Original missile model rendered in all three views; fallback differs; projectile state preserved"
            );
        }
        "campaign-level" => {
            let level: i16 = args.get(3).map(String::as_str).unwrap_or("1").parse()?;
            let difficulty: i16 = args.get(4).map(String::as_str).unwrap_or("0").parse()?;
            let mut viewer = Viewer::default();
            load_optional_colors(&mut viewer)?;
            viewer.load(&bytes)?;
            viewer.start_campaign_level(level, difficulty)?;
            let game = viewer.game.as_ref().unwrap();
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "campaign":viewer.campaign,"flags":game.flags,"scenario":game.scenario,
                    "targets":game.targets,"par_seconds":game.par_seconds,"dump_weights":game.dump_weights,"optional_weapons":game.optional_weapons,"pickups":game.pickups,"pickups_collected":game.pickups_collected,"flight_energy":game.flight_energy,"vertical":game.vertical,"pending_shields":game.pending_shields,"radar_glyph_count":game.radar_glyphs.as_ref().map_or(0, Vec::len),"palette":game.palette,"ground_color_index":game.ground_color_index,
                }))?
            );
        }
        "campaign-wave" => {
            let level: i16 = args.get(3).map(String::as_str).unwrap_or("1").parse()?;
            let difficulty: i16 = args.get(4).map(String::as_str).unwrap_or("0").parse()?;
            let id = spectre_core::recovered::wave_resource_id(difficulty, 0, 0);
            let archive = Archive::parse(&bytes)?;
            let resource = archive
                .resources
                .iter()
                .find(|r| r.kind == "wave" && r.id == id)
                .ok_or("Selected wave resource is missing")?;
            let rows = formats::waves(&resource.data)?;
            let selection = spectre_core::recovered::select_wave(&rows, level)
                .ok_or("Invalid level or wave count")?;
            let prefix_count = |kind: &str| {
                spectre_core::recovered::resource_prefix_count(|id| {
                    archive
                        .resources
                        .iter()
                        .any(|r| r.kind == kind && r.id == id)
                })
            };
            let counts = [prefix_count("wrld"), prefix_count("enem")];
            let random = args
                .iter()
                .skip(5)
                .map(|value| value.parse::<i16>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let resolved = spectre_core::recovered::resolve_wave_resources(
                selection.wave.world,
                selection.wave.enemies,
                counts,
                0,
                &random,
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "resource":id,"level":level,"difficulty":difficulty,"selection":selection,
                    "contiguous_counts":counts,"resolved":resolved,"random_samples":random,
                    "random_world_requested":selection.wave.world==0,
                    "random_enemies_requested":selection.wave.enemies==0,
                    "needs_random_samples":resolved.is_none()
                }))?
            );
        }
        "score-rules" => {
            let difficulty: i16 = args.get(3).map(String::as_str).unwrap_or("0").parse()?;
            let selected = spectre_core::recovered::score_resource_id(difficulty, 0, 0);
            let archive = Archive::parse(&bytes)?;
            let mut result = Vec::new();
            for resource in archive.resources.iter().filter(|r| r.kind == "sstf") {
                result.push(serde_json::json!({"id":resource.id,"selected":resource.id==selected,"difficulty":difficulty,"rules":formats::ScoreRules::parse(&resource.data)?}));
            }
            println!("{}", serde_json::to_string_pretty(&result)?);
        }

        "flag-layout" => {
            let level: i16 = args.get(3).map(String::as_str).unwrap_or("1").parse()?;
            let archive = Archive::parse(&bytes)?;
            let mut result = Vec::new();
            for resource in archive.resources.iter().filter(|r| r.kind == "wrld") {
                let original = formats::World::parse(&resource.data)?;
                let world = original.with_level_flags(level)?;
                let flags: Vec<_> = world
                    .obstacles
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.kind == 12)
                    .map(|(i, o)| serde_json::json!({"slot":i+1,"h":o.h,"v":o.v,"facing":o.facing}))
                    .collect();
                result.push(serde_json::json!({"world":resource.id,"level":level,"flags":flags,"objects":world.obstacles.len()}));
            }
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "export-sounds" => {
            let archive = Archive::parse(&bytes)?;
            let out = Path::new(args.get(3).ok_or("Supply sound output directory")?);
            fs::create_dir_all(out)?;
            let mut manifest = Vec::new();
            for resource in archive.resources.iter().filter(|r| r.kind == "snd ") {
                let sound = spectre_core::sound::Sound::parse(&resource.data)?;
                fs::write(out.join(format!("{}.wav", resource.id)), sound.wav())?;
                manifest.push(serde_json::json!({"id":resource.id,"name":resource.name,"rate":sound.rate,"samples":sound.samples.len()}));
            }
            fs::write(
                out.join("manifest.json"),
                serde_json::to_vec_pretty(&manifest)?,
            )?;
            println!("Exported {} original PCM sounds", manifest.len());
        }

        "model-slots" => {
            let catalog = spectre_core::scene::Catalog::load(&Archive::parse(&bytes)?)?;
            let mut result = Vec::new();
            for kind in 0..47 {
                let mut slots = spectre_core::model_slots::ModelSlots::default();
                let supported = slots.load_type(&catalog, kind).is_some();
                let geometry: Vec<_> = slots.geometry.iter().map(|(slot,shape)| serde_json::json!({"slot":slot,"vertices":shape.vertices,"dimensions":shape.dimensions,"radius":shape.candidate_radius})).collect();
                result.push(serde_json::json!({"kind":kind,"supported":supported,"next":slots.state.next,"current":slots.state.current.as_slice(),"counts":slots.state.counts.as_slice(),"geometry":geometry}));
            }
            println!("{}", serde_json::to_string(&result)?);
        }
        "verify-handling" => {
            let frames: serde_json::Value = serde_json::from_slice(&bytes)?;
            let frames = frames.as_array().ok_or("Expected handling frames")?;
            let mut game = spectre_core::game::Game::new(formats::World { obstacles: vec![] })?;
            for (tick, frame) in frames.iter().enumerate() {
                game.step(spectre_core::game::Input {
                    throttle: frame["throttle"].as_i64().unwrap() as f32,
                    steer: frame["steer"].as_i64().unwrap() as f32,
                    ..Default::default()
                });
                let actual = serde_json::json!(game.handling_snapshot());
                if actual != frame["expected"] {
                    return Err(format!(
                        "Handling mismatch at tick {tick}: {actual} != {}",
                        frame["expected"]
                    )
                    .into());
                }
            }
            println!(
                "{} integrated handling ticks match original movement instructions with the candidate direction table.",
                frames.len()
            );
        }
        "verify-direction-table" => {
            let report: serde_json::Value = serde_json::from_slice(&bytes)?;
            let collision: Vec<[i16; 2]> = serde_json::from_value(report["collision"].clone())?;
            if collision.as_slice() != spectre_core::recovered::candidate_collision_table() {
                return Err(
                    "Candidate collision table differs from modeled-SANE initializer".into(),
                );
            }
            let expected: Vec<i32> = serde_json::from_value(report["movement"].clone())?;
            let actual = spectre_core::recovered::candidate_movement_table();
            let base: Vec<i32> = serde_json::from_value(report["base"].clone())?;
            if base.as_slice() != spectre_core::recovered::candidate_unit_table() {
                return Err("Candidate unit table differs from modeled-SANE initializer".into());
            }
            if expected.as_slice() != actual {
                return Err(
                    "Candidate direction table differs from modeled-SANE initializer".into(),
                );
            }
            println!(
                "360 movement values and 360 collision pairs match modeled-SANE initializers (original SANE still unverified)."
            );
        }
        "verify-recovered" => {
            let vectors: serde_json::Value = serde_json::from_slice(&bytes)?;
            let cases = vectors["cases"].as_array().ok_or("Expected cases")?;
            for case in cases {
                let a = case["a"].as_i64().ok_or("Missing a")? as i32;
                let b = case["b"].as_i64().ok_or("Missing b")? as i32;
                let expected = case["expected"].as_i64().ok_or("Missing expected")? as i32;
                let actual = match case["routine"].as_str() {
                    Some("machine_turn_rate") => i32::from(
                        spectre_core::recovered::half_turn_rate_for_machine(a as i16, b as i16),
                    ),
                    Some("keyboard") => {
                        let keys = serde_json::from_value(case["keys"].clone())?;
                        let (throttle, turn) = spectre_core::recovered::keyboard_commands(&keys);
                        if turn as i64 != case["turn"].as_i64().unwrap() {
                            return Err(
                                format!("Keyboard turn mismatch: {case}, actual {turn}").into()
                            );
                        }
                        throttle as i32
                    }
                    Some("vehicle_settings") => {
                        let current = serde_json::from_value(case["current"].clone())?;
                        let saved = serde_json::from_value(case["saved"].clone())?;
                        let after = spectre_core::recovered::select_vehicle_settings(
                            a as i16, current, saved,
                        );
                        if serde_json::json!(after) != case["after"] {
                            return Err(format!(
                                "Vehicle settings mismatch: {case}, actual {after:?}"
                            )
                            .into());
                        }
                        after[3] as i32
                    }
                    Some("translation") => {
                        let table: [i32; 360] = serde_json::from_value::<Vec<i32>>(
                            vectors["movement_marker_table"].clone(),
                        )?
                        .try_into()
                        .map_err(|_| "Expected 360 table entries")?;
                        let direction = spectre_core::recovered::movement_direction(a, &table)
                            .ok_or("Invalid heading")?;
                        let position: [i32; 2] = serde_json::from_value(case["position"].clone())?;
                        let (after, result) = spectre_core::recovered::move_type6_planar(
                            position,
                            direction,
                            spectre_core::recovered::SpeedUpdate {
                                speed: b as i8,
                                command: case["command"].as_i64().unwrap() as i16,
                                elapsed: case["elapsed"].as_i64().unwrap() as i32,
                                limit: case["limit"].as_i64().unwrap() as i8,
                                coast: case["coast"].as_i64().unwrap() as i8,
                                adjustment: case["adjustment"].as_i64().unwrap() as i8,
                                mode: case["mode"].as_i64().unwrap() as i8,
                                random: case["random"].as_i64().unwrap() as i16,
                            },
                        );
                        if serde_json::json!(after) != case["after"]
                            || result.used_random != case["used_random"].as_bool().unwrap()
                        {
                            return Err(
                                format!("Translation mismatch: {case}, actual {after:?}").into()
                            );
                        }
                        result.speed as i32
                    }
                    Some("speed") => {
                        let result = spectre_core::recovered::update_speed(
                            spectre_core::recovered::SpeedUpdate {
                                speed: a as i8,
                                command: b as i16,
                                elapsed: case["elapsed"].as_i64().unwrap() as i32,
                                limit: case["limit"].as_i64().unwrap() as i8,
                                coast: case["coast"].as_i64().unwrap() as i8,
                                adjustment: case["adjustment"].as_i64().unwrap() as i8,
                                mode: case["mode"].as_i64().unwrap() as i8,
                                random: case["random"].as_i64().unwrap() as i16,
                            },
                        );
                        if result.reset_timestamps != case["reset_timestamps"].as_bool().unwrap()
                            || result.used_random != case["used_random"].as_bool().unwrap()
                        {
                            return Err(format!("Speed side-effect mismatch: {case}").into());
                        }
                        result.speed as i32
                    }
                    Some("turn") => {
                        let (heading, ramp) = spectre_core::recovered::turn_object(
                            a,
                            b as i8,
                            case["kind"].as_i64().unwrap() as i8,
                            case["command"].as_i64().unwrap() as i16,
                            case["elapsed"].as_i64().unwrap() as i32,
                            case["delta"].as_i64().unwrap() as i32,
                        );
                        if ramp as i64 != case["ramp"].as_i64().unwrap() {
                            return Err(format!("Turn ramp mismatch: {case}, actual {ramp}").into());
                        }
                        heading
                    }
                    Some("remaining_flags") => {
                        let kinds: Vec<u8> = case["kinds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap() as u8)
                            .collect();
                        spectre_core::recovered::remaining_flags(&kinds, a as i8)
                            .ok_or("Short flag fixture")? as i32
                    }
                    Some("refresh_flags") => {
                        let mut kinds: Vec<u8> = case["kinds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap() as u8)
                            .collect();
                        let count = spectre_core::recovered::refresh_flag_slots(
                            a as i16, b as i16, &mut kinds,
                        )
                        .ok_or("Short refresh fixture")?;
                        if serde_json::json!(kinds) != case["after"] {
                            return Err(format!("Flag refresh mismatch: {case}").into());
                        }
                        count as i32
                    }
                    Some("contact_near") => {
                        let pos =
                            |key| std::array::from_fn(|i| case[key][i].as_i64().unwrap() as i32);
                        i32::from(spectre_core::recovered::contact_near(
                            pos("first"),
                            pos("second"),
                        ))
                    }
                    Some("contact_select") => {
                        let mut contacts: [Option<i16>; 2] =
                            std::array::from_fn(|i| case["before"][i].as_i64().map(|x| x as i16));
                        spectre_core::recovered::select_contact(
                            &mut contacts,
                            a as u8,
                            b as u8,
                            37,
                            case["same_owner"].as_bool().unwrap(),
                        );
                        if serde_json::json!(contacts) != case["after"] {
                            return Err(format!(
                                "Contact selection mismatch: {case}, actual {contacts:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("object_pair_overlap") => {
                        let record = |n: usize| {
                            std::array::from_fn(|i| case["records"][n][i].as_u64().unwrap() as u8)
                        };
                        let dimensions =
                            std::array::from_fn(|i| case["dimensions"][i].as_i64().unwrap() as i32);
                        let radii =
                            std::array::from_fn(|i| case["radii"][i].as_i64().unwrap() as i16);
                        i32::from(spectre_core::recovered::object_pair_overlap(
                            &record(0),
                            &record(1),
                            dimensions,
                            radii,
                            &spectre_core::recovered::candidate_collision_table(),
                        ))
                    }
                    Some("flow_displacement") => {
                        let position =
                            std::array::from_fn(|i| case["position"][i].as_i64().unwrap() as i32);
                        let direction =
                            std::array::from_fn(|i| case["direction"][i].as_i64().unwrap() as i32);
                        let after = spectre_core::recovered::flow_field_displacement(
                            position, direction, a, b,
                        );
                        if serde_json::json!(after) != case["after"] {
                            return Err(format!("Flow Field displacement mismatch: {case}").into());
                        }
                        0
                    }
                    Some("damage") => {
                        spectre_core::recovered::subtract_health(a as i16, b as i16) as i32
                    }
                    Some("shot_launch") | Some("shot_motion") => {
                        let position =
                            std::array::from_fn(|i| case["position"][i].as_i64().unwrap() as i32);
                        let direction =
                            std::array::from_fn(|i| case["direction"][i].as_i64().unwrap() as i32);
                        let (after, deadline) = if case["routine"] == "shot_launch" {
                            spectre_core::recovered::launch_basic_shot(
                                position, direction, a, b as i16,
                            )
                        } else {
                            (
                                spectre_core::recovered::move_basic_shot(position, direction, a),
                                0,
                            )
                        };
                        if serde_json::json!(after) != case["after"] {
                            return Err(format!("Projectile position mismatch: {case}").into());
                        }
                        deadline
                    }
                    Some("projectile_elapsed") => spectre_core::recovered::projectile_elapsed(a, b),
                    Some("procedural_entry") => {
                        spectre_core::recovered::procedural_model_entry(a as i16)
                            .map(i32::from)
                            .unwrap_or(-1)
                    }
                    Some("dimension_override") => {
                        use spectre_core::recovered::CollisionDimensionOverride;
                        let before =
                            std::array::from_fn(|i| case["before"][i].as_i64().unwrap() as i32);
                        let source =
                            std::array::from_fn(|i| case["source"][i].as_i64().unwrap() as i32);
                        let rule = match a {
                            0 => CollisionDimensionOverride::TenUnits,
                            1 => CollisionDimensionOverride::Half,
                            2 => CollisionDimensionOverride::Double,
                            3 => CollisionDimensionOverride::Copy(source),
                            _ => return Err("Invalid override".into()),
                        };
                        if serde_json::json!(
                            spectre_core::recovered::override_collision_dimensions(before, rule)
                        ) != case["after"]
                        {
                            return Err(format!("Dimension override mismatch: {case}").into());
                        }
                        0
                    }
                    Some("delete_projectile") | Some("delete_object") => {
                        let mut records: Vec<[u8; 128]> = case["records"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|r| std::array::from_fn(|i| r[i].as_u64().unwrap() as u8))
                            .collect();
                        let mut count = records.len();
                        let mut participants = std::array::from_fn(|p| {
                            std::array::from_fn(|i| {
                                case["participants"][p][i].as_u64().unwrap() as u8
                            })
                        });
                        let mut grid = std::array::from_fn(|i| {
                            case["grid"][i]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|n| n.as_i64().unwrap() as i16)
                                .collect()
                        });
                        let mut status =
                            std::array::from_fn(|i| case["status"][i].as_u64().unwrap() as u8);
                        if case["routine"] == "delete_projectile" {
                            spectre_core::recovered::delete_projectile_record(
                                &mut records,
                                &mut count,
                                &mut grid,
                                &mut participants,
                                a as usize,
                            )
                            .ok_or("Invalid deletion fixture")?;
                        } else {
                            spectre_core::recovered::delete_object_record(
                                &mut records,
                                &mut count,
                                &mut grid,
                                &mut participants,
                                &mut status,
                                a as usize,
                            )
                            .ok_or("Invalid deletion fixture")?;
                        }
                        if serde_json::json!(status) != case["after_status"] {
                            return Err("Participant status mismatch".into());
                        }
                        let actual_records: Vec<_> = records.iter().map(|r| r.as_slice()).collect();
                        let actual_participants: Vec<_> =
                            participants.iter().map(|r| r.as_slice()).collect();
                        if serde_json::json!(actual_records) != case["after_records"]
                            || serde_json::json!(actual_participants) != case["after_participants"]
                            || serde_json::json!(grid.as_slice()) != case["after_grid"]
                            || count != case["after_count"].as_u64().unwrap() as usize
                        {
                            return Err(format!("Projectile deletion mismatch, id {a}").into());
                        }
                        0
                    }
                    Some("grid_insert") | Some("grid_unlink") => {
                        let mut cell: Vec<i16> = case["cell"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|n| n.as_i64().unwrap() as i16)
                            .collect();
                        let changed = if case["routine"] == "grid_insert" {
                            spectre_core::recovered::insert_projectile_grid_entry(
                                &mut cell, a as i16,
                            )
                        } else {
                            spectre_core::recovered::remove_grid_entry(&mut cell, a as i16)
                        };
                        if serde_json::json!(cell) != case["after"] {
                            return Err(format!("Grid cell mutation mismatch: {case}").into());
                        }
                        changed as i32
                    }
                    Some("grid_move") => {
                        let old = std::array::from_fn(|i| case["old"][i].as_i64().unwrap() as i32);
                        let new = std::array::from_fn(|i| case["new"][i].as_i64().unwrap() as i32);
                        let mut grid = std::array::from_fn(|i| {
                            case["grid"][i]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|n| n.as_i64().unwrap() as i16)
                                .collect()
                        });
                        spectre_core::recovered::move_grid_object(&mut grid, 1, old, new)
                            .ok_or("Invalid grid move fixture")?;
                        if serde_json::json!(grid.as_slice()) != case["after"] {
                            return Err(format!("Grid move mismatch: {old:?} -> {new:?}").into());
                        }
                        0
                    }
                    Some("object_cell") => {
                        let cell = spectre_core::recovered::object_grid_cell([a, b]);
                        if serde_json::json!(cell) != case["cell"] {
                            return Err(format!("Cell mismatch: {case}").into());
                        }
                        0
                    }
                    Some("grid_traversal") => {
                        let bounds =
                            std::array::from_fn(|i| case["bounds"][i].as_i64().unwrap() as i16);
                        let grid = std::array::from_fn(|i| {
                            case["grid"][i]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|n| n.as_i64().unwrap() as i16)
                                .collect()
                        });
                        let actual =
                            spectre_core::recovered::projectile_grid_candidates(bounds, &grid)
                                .ok_or("Invalid traversal fixture")?;
                        if serde_json::json!(actual) != case["visited"] {
                            return Err(format!("Grid traversal mismatch: {bounds:?}").into());
                        }
                        0
                    }
                    Some("projectile_grid") => {
                        let pose =
                            std::array::from_fn(|i| case["pose"][i].as_i64().unwrap() as i32);
                        let dimensions =
                            std::array::from_fn(|i| case["dimensions"][i].as_i64().unwrap() as i32);
                        let table = std::array::from_fn(|i| {
                            [
                                ((i * 193) % 65536) as u16 as i16,
                                ((i * 317 + 1000) % 65536) as u16 as i16,
                            ]
                        });
                        let (extents, bounds) = spectre_core::recovered::projectile_grid_bounds(
                            pose, dimensions, a as i16, &table,
                        );
                        if serde_json::json!(extents) != case["extents"]
                            || serde_json::json!(bounds) != case["bounds"]
                        {
                            return Err(format!(
                                "Projectile grid mismatch: {case}, actual {extents:?} {bounds:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("procedural_box") | Some("fixed_obstacle") => {
                        let (shape, faces) = if case["routine"] == "procedural_box" {
                            let arguments = std::array::from_fn(|i| {
                                case["arguments"][i].as_i64().unwrap() as i32
                            });
                            (
                                spectre_core::model::PackedShape::tapered_box(arguments),
                                spectre_core::model::PackedShape::box_faces(),
                            )
                        } else {
                            spectre_core::model::PackedShape::fixed_obstacle(a as i16, b != 0)
                                .unwrap()
                        };
                        if serde_json::json!(shape.vertices) != case["packed"]
                            || serde_json::json!(faces) != case["faces"]
                        {
                            return Err(format!(
                                "Procedural obstacle mismatch: {case}, actual {:?} {:?}",
                                shape.vertices, faces
                            )
                            .into());
                        }
                        0
                    }
                    Some("spawn_team_coordinates" | "spawn_beside_flag") => {
                        let result = if case["routine"] == "spawn_team_coordinates" {
                            spectre_core::recovered::spawn_team_coordinates(
                                serde_json::from_value(case["pose"].clone())?,
                                a as i8,
                                b as i8,
                                serde_json::from_value(case["anchor"].clone())?,
                                case["extent"].as_i64().unwrap() as i16,
                            )
                        } else {
                            spectre_core::recovered::spawn_beside_flag(
                                serde_json::from_value(case["flag"].clone())?,
                                a as i16,
                                serde_json::from_value(case["direction"].clone())?,
                            )
                        };
                        if serde_json::json!(result) != case["after"] {
                            return Err(format!(
                                "Spawn variant mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("spawn_participant_blocked") => {
                        let flags = serde_json::from_value(case["flags"].clone())?;
                        let ids = serde_json::from_value(case["ids"].clone())?;
                        let positions: Vec<[i32; 2]> =
                            serde_json::from_value(case["positions"].clone())?;
                        i32::from(
                            spectre_core::recovered::spawn_participant_blocked(
                                a as usize,
                                b as usize,
                                case["mode"].as_i64().unwrap() as i8,
                                &flags,
                                &ids,
                                &positions,
                            )
                            .ok_or("Invalid spawn geometry")?,
                        )
                    }
                    Some("combat_damage_counter" | "combat_launch_counter") => {
                        let mut counters: spectre_core::recovered::CombatCounters =
                            serde_json::from_value(case["before"].clone())?;
                        let result = if case["routine"] == "combat_damage_counter" {
                            let mut grace = a;
                            let admitted = counters
                                .damage(serde_json::from_value(case["event"].clone())?, &mut grace);
                            if grace != b {
                                return Err(format!("Damage grace mismatch: {case}").into());
                            }
                            i32::from(admitted)
                        } else {
                            counters.launch(a as i8, b as i8);
                            0
                        };
                        if serde_json::to_value(counters)? != case["after"] {
                            return Err(format!("Counter mismatch: {case}").into());
                        }
                        result
                    }
                    Some("sound_request_id") => {
                        spectre_core::recovered::sound_request_id(a as u32).map_or(-1, i32::from)
                    }
                    Some("last_enemy_bonus") => spectre_core::recovered::last_enemy_bonus(
                        a as i16,
                        &std::array::from_fn(|i| case["counts"][i].as_i64().unwrap() as i16),
                        &std::array::from_fn(|i| case["lives"][i].as_i64().unwrap() as i16),
                    ) as i32,
                    Some("spawn_search") => {
                        let context = serde_json::from_value(case["context"].clone())?;
                        let mut geometry: Vec<spectre_core::recovered::SpawnGeometry> =
                            serde_json::from_value(case["geometry"].clone())?;
                        let direction: [i32; 2] =
                            serde_json::from_value(case["direction"].clone())?;
                        let samples: Vec<i16> = serde_json::from_value(case["random"].clone())?;
                        let mut samples = samples.into_iter();
                        let contacts: Vec<[u8; 2]> =
                            serde_json::from_value(case["contacts"].clone())?;
                        let mut index = 0;
                        let mut cursor = a as i16;
                        let result = spectre_core::recovered::spawn_search(
                            &context,
                            &mut geometry,
                            &mut cursor,
                            &mut samples,
                            |_| direction,
                            |_| {
                                let v = contacts[index];
                                index += 1;
                                v
                            },
                        )
                        .ok_or_else(|| format!("Invalid combined spawn: {case}"))?;
                        if serde_json::json!(result.pose) != case["pose"]
                            || serde_json::json!(cursor) != case["cursor"]
                            || samples.next().is_some()
                        {
                            return Err(format!("Combined spawn mismatch: {case}, actual {result:?}, cursor {cursor}").into());
                        }
                        result.attempts as i32
                    }
                    Some("normal_spawn_search") => {
                        let samples: Vec<i16> = serde_json::from_value(case["random"].clone())?;
                        let contacts: Vec<[u8; 2]> =
                            serde_json::from_value(case["contacts"].clone())?;
                        let mut index = 0;
                        let result = spectre_core::recovered::normal_spawn_search(
                            b as u8,
                            case["kind"].as_i64().unwrap() as i8,
                            a as i16,
                            &mut samples.into_iter(),
                            |_| {
                                let value = contacts[index];
                                index += 1;
                                value
                            },
                        )
                        .ok_or("Invalid spawn search")?;
                        if serde_json::json!(result.pose) != case["pose"] {
                            return Err(format!(
                                "Spawn search mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        result.attempts as i32
                    }
                    Some("spawn_coordinate") => spectre_core::recovered::spawn_coordinate(
                        case["enemy"].as_u64().unwrap() as u8,
                        case["kind"].as_i64().unwrap() as i8,
                        a as i16,
                        b as i16,
                    )
                    .ok_or("Invalid spawn extent")?,
                    Some("borrow_enemy_life" | "consume_enemy_life") => {
                        let flags: [[u8; 3]; 12] = serde_json::from_value(case["flags"].clone())?;
                        let mut lives: [i16; 12] = serde_json::from_value(case["lives"].clone())?;
                        let result = if case["routine"] == "consume_enemy_life" {
                            spectre_core::recovered::consume_enemy_life(
                                a as usize, &flags, &mut lives, b as u8,
                            )
                            .ok_or("Invalid participant")?
                        } else {
                            spectre_core::recovered::borrow_enemy_life(
                                a as usize, &flags, &mut lives,
                            )
                            .is_some()
                        };
                        if serde_json::json!(lives) != case["after"] {
                            return Err(format!(
                                "Life transfer mismatch: {case}, actual {lives:?}"
                            )
                            .into());
                        }
                        i32::from(result)
                    }
                    Some("remove_geometry") => {
                        let rows: Vec<Vec<u8>> = serde_json::from_value(case["before"].clone())?;
                        let mut records: Vec<[u8; 128]> = rows
                            .into_iter()
                            .map(|row| row.try_into().unwrap())
                            .collect();
                        let mut participants =
                            serde_json::from_value(case["participants"].clone())?;
                        let cells: Vec<Vec<i16>> = serde_json::from_value(case["grid"].clone())?;
                        let mut grid: [Vec<i16>; 256] =
                            cells.try_into().map_err(|_| "Invalid grid")?;
                        let mut count = a as i16;
                        let result = spectre_core::recovered::remove_geometry(
                            &mut records,
                            &mut count,
                            b as i16,
                            &mut participants,
                            &mut grid,
                        );
                        let rows: Vec<Vec<u8>> = records.iter().map(|row| row.to_vec()).collect();
                        if result.is_none()
                            || count != a as i16 - 1
                            || serde_json::json!(rows) != case["after"]
                            || serde_json::json!(participants) != case["after_participants"]
                            || serde_json::json!(grid.as_slice()) != case["after_grid"]
                        {
                            return Err(format!("Geometry removal mismatch: {case}").into());
                        }
                        0
                    }
                    Some("compact_geometry_tail") => {
                        let rows: Vec<Vec<u8>> = serde_json::from_value(case["before"].clone())?;
                        let mut records: Vec<[u8; 128]> = rows
                            .into_iter()
                            .map(|row| row.try_into().unwrap())
                            .collect();
                        let mut ids = serde_json::from_value(case["ids"].clone())?;
                        let cells: Vec<Vec<i16>> = serde_json::from_value(case["grid"].clone())?;
                        let mut grid: [Vec<i16>; 256] =
                            cells.try_into().map_err(|_| "Invalid grid")?;
                        let mut count = a as i16;
                        let result = spectre_core::recovered::compact_geometry_tail(
                            &mut records,
                            &mut count,
                            b as i16,
                            &mut ids,
                            &mut grid,
                        );
                        let rows: Vec<Vec<u8>> = records.iter().map(|row| row.to_vec()).collect();
                        if result.is_none()
                            || count != a as i16 - 1
                            || serde_json::json!(rows) != case["after"]
                            || serde_json::json!(ids) != case["after_ids"]
                            || serde_json::json!(grid.as_slice()) != case["after_grid"]
                        {
                            return Err(format!("Geometry compaction mismatch: {case}").into());
                        }
                        0
                    }
                    Some("death_burst_gate") => spectre_core::recovered::death_burst_gate(
                        a as u8,
                        b as u8,
                        case["field"].as_i64().unwrap() as i16,
                    ) as i32,
                    Some("radial_burst_requests") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let record: [u8; 128] = bytes.try_into().map_err(|_| "Invalid record")?;
                        let input = serde_json::from_value(case["input"].clone())?;
                        let mut calls = 0;
                        let result =
                            spectre_core::recovered::radial_burst_requests(&record, input, || {
                                calls += 1;
                                case["sample"].as_i64().unwrap() as i16
                            });
                        if serde_json::json!(result) != case["requests"]
                            || serde_json::json!(calls) != case["random_calls"]
                        {
                            return Err(format!("Radial burst mismatch: {case}").into());
                        }
                        0
                    }
                    Some("death_fragment_requests") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let record: [u8; 128] = bytes.try_into().map_err(|_| "Invalid record")?;
                        let input = serde_json::from_value(case["input"].clone())?;
                        let samples: Vec<i16> = serde_json::from_value(case["samples"].clone())?;
                        let mut calls = 0;
                        let result = spectre_core::recovered::death_fragment_requests(
                            &record,
                            input,
                            || {
                                let value = samples[calls];
                                calls += 1;
                                value
                            },
                        );
                        if serde_json::json!(result) != case["requests"] || calls != 2 {
                            return Err(format!("Death fragments mismatch: {case}").into());
                        }
                        0
                    }
                    Some("reset_ai_participant") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 256] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let mut cache = serde_json::from_value(case["cache"].clone())?;
                        spectre_core::recovered::reset_ai_participant(
                            &mut record,
                            a as i8,
                            &mut cache,
                        );
                        if serde_json::json!(record.as_slice()) != case["after"]
                            || serde_json::json!(cache) != case["after_cache"]
                        {
                            return Err(format!("AI reset mismatch: {case}").into());
                        }
                        0
                    }
                    Some("death_ai_reset_targets") => {
                        let active = serde_json::from_value(case["active"].clone())?;
                        let enemy = serde_json::from_value(case["enemy"].clone())?;
                        if serde_json::json!(spectre_core::recovered::death_ai_reset_targets(
                            &active, &enemy, a as u8
                        )) != case["targets"]
                        {
                            return Err(format!("Death AI reset targets mismatch: {case}").into());
                        }
                        0
                    }
                    Some("damage_killed") => spectre_core::recovered::damage_killed(
                        a as i16,
                        b as u8,
                        case["supplied"].as_u64().unwrap() as u8,
                    ) as i32,
                    Some("local_death_state") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let mut state = serde_json::from_value(case["state"].clone())?;
                        spectre_core::recovered::local_death_state(
                            &mut record,
                            &mut state,
                            a,
                            case["local"].as_bool().unwrap(),
                            b as u8,
                        );
                        if serde_json::json!(record.as_slice()) != case["after"]
                            || serde_json::json!(state) != case["after_state"]
                        {
                            return Err(format!("Local death state mismatch: {case}").into());
                        }
                        0
                    }
                    Some("death_camera") => {
                        let state = serde_json::from_value(case["state"].clone())?;
                        let direction = serde_json::from_value(case["direction"].clone())?;
                        let mut angles = Vec::new();
                        let result =
                            spectre_core::recovered::death_camera(&state, a, b as i16, |angle| {
                                angles.push(angle);
                                direction
                            });
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(angles) != case["angles"]
                        {
                            return Err(format!("Death camera mismatch: {case}").into());
                        }
                        0
                    }
                    Some("participant_transition_ready") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        spectre_core::recovered::participant_transition_ready(input) as i32
                    }
                    Some("begin_participant_transition") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let bytes: Vec<u8> = serde_json::from_value(case["participant"].clone())?;
                        let mut participant: [u8; 256] =
                            bytes.try_into().map_err(|_| "Invalid participant")?;
                        spectre_core::recovered::begin_participant_transition(
                            &mut participant,
                            &mut record,
                        );
                        if serde_json::json!(record.as_slice()) != case["after_record"]
                            || serde_json::json!(participant.as_slice())
                                != case["after_participant"]
                        {
                            return Err(
                                format!("Begin participant transition mismatch: {case}").into()
                            );
                        }
                        0
                    }
                    Some("display_rectangle") => {
                        let bounds = serde_json::from_value(case["bounds"].clone())?;
                        let full_size = case["full_size"].as_u64().unwrap() as u8;
                        if serde_json::json!(spectre_core::recovered::display_rectangle(
                            bounds, full_size
                        )) != case["result"]
                        {
                            return Err(format!("Display rectangle mismatch: {case}").into());
                        }
                        0
                    }
                    Some("display_rectangle_setup") => {
                        let bounds = serde_json::from_value(case["bounds"].clone())?;
                        let full_size =
                            case["full_size"].as_u64().ok_or("Missing full_size")? as u8;
                        if serde_json::json!(spectre_core::recovered::display_rectangle_setup(
                            bounds, full_size
                        )) != case["result"]
                        {
                            return Err(format!("Display rectangle setup mismatch: {case}").into());
                        }
                        0
                    }
                    Some("immediate_crosshair_rectangles") => {
                        let center = serde_json::from_value(case["center"].clone())?;
                        let hidden = case["hidden"].as_bool().unwrap();
                        let result: Vec<_> =
                            spectre_core::recovered::crosshair_lines(center, hidden)
                                .into_iter()
                                .filter_map(|[x0, y0, x1, y1]| {
                                    spectre_core::recovered::axis_line_rectangle(
                                        [[x0, y0], [x1, y1]],
                                        [1, 1],
                                        false,
                                    )
                                })
                                .collect();
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Immediate crosshair mismatch: {case}").into());
                        }
                        0
                    }
                    Some("axis_line_rectangle") => {
                        let endpoints = serde_json::from_value(case["endpoints"].clone())?;
                        let pen = serde_json::from_value(case["pen"].clone())?;
                        if serde_json::json!(spectre_core::recovered::axis_line_rectangle(
                            endpoints,
                            pen,
                            case["suppressed"].as_bool().unwrap()
                        )) != case["result"]
                        {
                            return Err(format!("Axis line rectangle mismatch: {case}").into());
                        }
                        0
                    }
                    Some("surface_blit_geometry") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        if serde_json::json!(spectre_core::recovered::surface_blit_geometry(input))
                            != case["result"]
                        {
                            return Err(format!("Surface blit mismatch: {case}").into());
                        }
                        0
                    }
                    Some("radar_scale") => {
                        if serde_json::json!(spectre_core::radar::scale_for_arena(a as i16))
                            != case["result"]
                        {
                            return Err(format!("Radar scale mismatch: {case}").into());
                        }
                        0
                    }
                    Some("radar_damage_disturbance") => {
                        let result = spectre_core::radar::damage_disturbance(
                            a as i8,
                            b as i8,
                            case["health"].as_i64().unwrap() as i16,
                            case["difficulty"].as_i64().unwrap() as i16,
                            case["now"].as_i64().unwrap() as i32,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(
                                format!("Radar disturbance trigger mismatch: {case}").into()
                            );
                        }
                        0
                    }
                    Some("radar_glyph_points") => {
                        let point = spectre_core::recovered::radar_point(
                            serde_json::from_value(case["position"].clone())?,
                            a as i16,
                            serde_json::from_value(case["center"].clone())?,
                            156,
                            32767,
                            1688,
                        );
                        let offsets: Vec<[i8; 2]> =
                            serde_json::from_value(case["offsets"].clone())?;
                        let result = spectre_core::radar::glyph_points(
                            point,
                            &offsets,
                            b as u8,
                            case["now"].as_i64().unwrap() as i32,
                            case["deadline"].as_i64().unwrap() as i32,
                            serde_json::from_value(case["random"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Radar glyph points mismatch: {case}").into());
                        }
                        0
                    }
                    Some("flag_radar_glyph") => {
                        let glyph = spectre_core::radar::single_player_flag_glyph(
                            a as i16,
                            case["color"].as_u64().unwrap() as u8,
                            b as u8,
                        );
                        if serde_json::json!(glyph) != case["result"] {
                            return Err(format!("Flag radar mismatch: {case}").into());
                        }
                        0
                    }
                    Some("radar_flash") => {
                        let result = spectre_core::radar::advance_flash(
                            a,
                            b,
                            case["flash"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Radar flash mismatch: {case}").into());
                        }
                        0
                    }
                    Some("object_radar_glyph") => {
                        let color = case["color"].as_u64().unwrap() as u8;
                        let glyph = spectre_core::radar::object_glyph(a as i8, b as i16, color);
                        if serde_json::json!(glyph) != case["result"] {
                            return Err(format!("Object radar glyph mismatch: {case}").into());
                        }
                        0
                    }
                    Some("render_height_word") => {
                        i32::from(spectre_core::recovered::render_height_word(a))
                    }
                    Some("retired_radar_glyph") => {
                        let result = spectre_core::radar::retired_glyph(
                            a as i16,
                            b as u8,
                            case["color"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Retired radar mismatch: {case}").into());
                        }
                        0
                    }
                    Some("enemy_radar_glyph") => {
                        let color = case["color"].as_u64().unwrap() as u8;
                        let final_frame = case["final_frame"].as_bool().unwrap();
                        let glyph =
                            spectre_core::radar::enemy_glyph(a as i8, b as i16, color, final_frame);
                        if serde_json::json!(glyph) != case["result"] {
                            return Err(format!("Enemy radar glyph mismatch: {case}").into());
                        }
                        0
                    }
                    Some("radar_glyphs") => {
                        let bitmap: Vec<u8> = serde_json::from_value(case["bitmap"].clone())?;
                        let result = spectre_core::radar::glyphs_from_bitmap(&bitmap);
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Radar glyph mismatch: {case}").into());
                        }
                        if let Some(pict) = case.get("pict") {
                            let pict: Vec<u8> = serde_json::from_value(pict.clone())?;
                            if spectre_core::radar::glyphs_from_pict(&pict) != result {
                                return Err("PICT radar glyph decode mismatch".into());
                            }
                            for length in 0..pict.len() {
                                if spectre_core::radar::glyphs_from_pict(&pict[..length]).is_some()
                                {
                                    return Err("Truncated radar PICT accepted".into());
                                }
                            }
                        }
                        0
                    }
                    Some("radar_point") => {
                        let position = serde_json::from_value(case["position"].clone())?;
                        let center = serde_json::from_value(case["center"].clone())?;
                        let result = spectre_core::recovered::radar_point(
                            position,
                            a as i16,
                            center,
                            case["threshold"].as_i64().unwrap() as i32,
                            case["scale_base"].as_i64().unwrap() as i32,
                            b as i16,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Radar point mismatch: {case}").into());
                        }
                        0
                    }
                    Some("radar_edge_points") => {
                        let center = serde_json::from_value(case["center"].clone())?;
                        if serde_json::json!(spectre_core::recovered::radar_edge_points(center))
                            != case["result"]
                        {
                            return Err(format!("Radar edge mismatch: {case}").into());
                        }
                        0
                    }
                    Some("drawing_viewport") => {
                        let rect = serde_json::from_value(case["rect"].clone())?;
                        let origin = serde_json::from_value(case["screen_origin"].clone())?;
                        let previous = serde_json::from_value(case["previous_origin"].clone())?;
                        if serde_json::json!(spectre_core::recovered::drawing_viewport(
                            rect, origin, a as i16, previous
                        )) != case["result"]
                        {
                            return Err(format!("Drawing viewport mismatch: {case}").into());
                        }
                        0
                    }
                    Some("viewport_layout") => {
                        let bounds = serde_json::from_value(case["bounds"].clone())?;
                        let limits = serde_json::from_value(case["limits"].clone())?;
                        if serde_json::json!(spectre_core::recovered::viewport_layout(
                            bounds, limits
                        )) != case["result"]
                        {
                            return Err(format!("Viewport layout mismatch: {case}").into());
                        }
                        0
                    }
                    Some("viewport_metrics") => {
                        let bounds = serde_json::from_value(case["bounds"].clone())?;
                        let limits = serde_json::from_value(case["limits"].clone())?;
                        if serde_json::json!(spectre_core::recovered::viewport_metrics(
                            bounds, limits
                        )) != case["result"]
                        {
                            return Err(format!("Viewport metrics mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ground_projection_table") => {
                        let table: Vec<[i16; 2]> = serde_json::from_value(case["before"].clone())?;
                        let mut table: [[i16; 2]; 1200] =
                            table.try_into().map_err(|_| "Invalid table")?;
                        spectre_core::recovered::initialize_ground_projection_table(&mut table);
                        if serde_json::json!(table.as_slice()) != case["after"] {
                            return Err("Ground projection table mismatch".into());
                        }
                        0
                    }
                    Some("compose_resource_palette") => {
                        let base: Vec<u8> = serde_json::from_value(case["base"].clone())?;
                        let clut_2000: Option<Vec<u8>> =
                            serde_json::from_value(case["clut_2000"].clone())?;
                        let clut_2001: Option<Vec<u8>> =
                            serde_json::from_value(case["clut_2001"].clone())?;
                        let active = case["active_colors"]
                            .as_u64()
                            .ok_or("Missing color count")?
                            as usize;
                        if serde_json::json!(spectre_core::recovered::compose_resource_palette(
                            &base,
                            a as i16,
                            b as i16,
                            active,
                            clut_2000.as_deref(),
                            clut_2001.as_deref()
                        )) != case["result"]
                        {
                            return Err("Composed palette mismatch".into());
                        }
                        0
                    }
                    Some("level_palette_color") => {
                        let data: Option<Vec<u8>> = serde_json::from_value(case["data"].clone())?;
                        let use_2001 = case["use_2001"]
                            .as_bool()
                            .ok_or("Missing palette variant")?;
                        if serde_json::json!(spectre_core::recovered::level_palette_color(
                            a as i16,
                            b as i16,
                            use_2001,
                            data.as_deref()
                        )) != case["result"]
                        {
                            return Err(format!("Level palette color mismatch: {case}").into());
                        }
                        0
                    }
                    Some("clip_edge_records") => {
                        let result = spectre_core::recovered::clip_edge_records(
                            serde_json::from_value(case["edge"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Clipped edge mismatch".into());
                        }
                        0
                    }
                    Some("solid_distance_visible") => {
                        let result = spectre_core::recovered::solid_distance_visible(
                            case["a"].as_i64().unwrap() as i16,
                            case["b"].as_i64().unwrap() as i16,
                            case["suppressed"].as_u64().unwrap() as u8,
                            case["cached"].as_i64().unwrap() as i16,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Solid distance visibility mismatch".into());
                        }
                        0
                    }
                    Some("crosshair_lines") => {
                        let result = spectre_core::recovered::crosshair_lines(
                            serde_json::from_value(case["center"].clone())?,
                            case["hidden"].as_bool().unwrap(),
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Crosshair lines mismatch".into());
                        }
                        0
                    }
                    Some("resource_plane_sides") => {
                        let result = spectre_core::recovered::resource_plane_sides(
                            a as i16,
                            b as i16,
                            case["format"].as_u64().unwrap() as u8,
                            case["version"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Resource plane sides mismatch".into());
                        }
                        0
                    }
                    Some("model_render_metadata") => {
                        let mut model: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        if case["configure"].as_bool().unwrap() {
                            spectre_core::recovered::configure_model_edges(
                                &mut model,
                                a as i16,
                                serde_json::from_value(case["references"].clone())?,
                                b as u8,
                            )
                            .ok_or("Runtime model too short")?;
                        }
                        if case["enable"].as_bool().unwrap() {
                            spectre_core::recovered::enable_model_render_mode(&mut model)
                                .ok_or("Runtime model too short")?;
                        }
                        let fields =
                            spectre_core::recovered::RenderModelFields::from_runtime(&model);
                        if serde_json::json!(model) != case["result"]
                            || serde_json::to_value(fields)? != case["fields"]
                        {
                            return Err("Model render metadata mismatch".into());
                        }
                        0
                    }
                    Some("prepare_render_geometry") => {
                        let input: Vec<u8> = serde_json::from_value(case["input"].clone())?;
                        let input: [u8; 64] =
                            input.try_into().map_err(|_| "Render input length")?;
                        let vertices: Vec<[i16; 3]> =
                            serde_json::from_value(case["vertices"].clone())?;
                        let table: Vec<i32> = serde_json::from_value(case["table"].clone())?;
                        let table: [i32; 360] =
                            table.try_into().map_err(|_| "Render table length")?;
                        let mut state = serde_json::from_value(case["state"].clone())?;
                        spectre_core::recovered::prepare_render_geometry(
                            &input,
                            &vertices,
                            &table,
                            serde_json::from_value(case["config"].clone())?,
                            &mut state,
                        )
                        .ok_or("Invalid rendering geometry fixture")?;
                        if serde_json::to_value(&state)? != case["result"] {
                            return Err(format!(
                                "Render geometry mismatch: {case}, actual {}",
                                serde_json::to_string(&state)?
                            )
                            .into());
                        }
                        0
                    }
                    Some("prepare_render_record") => {
                        let input: Vec<u8> = serde_json::from_value(case["input"].clone())?;
                        let input: [u8; 64] =
                            input.try_into().map_err(|_| "Render input length")?;
                        let mut output: [u8; 32] = serde_json::from_value(case["before"].clone())?;
                        let mut classes: [i8; 2] = serde_json::from_value(case["classes"].clone())?;
                        let table: Vec<i32> = serde_json::from_value(case["table"].clone())?;
                        let table: [i32; 360] =
                            table.try_into().map_err(|_| "Render table length")?;
                        spectre_core::recovered::prepare_render_record(
                            &input,
                            &mut output,
                            serde_json::from_value(case["model"].clone())?,
                            serde_json::from_value(case["interval"].clone())?,
                            &mut classes,
                            &table,
                        );
                        if serde_json::json!([
                            serde_json::json!(output),
                            serde_json::json!(classes)
                        ]) != case["result"]
                        {
                            return Err(format!(
                                "Prepared record mismatch: {case}, actual {output:?} {classes:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("render_preparation_order") => {
                        let keys: Vec<i16> = serde_json::from_value(case["keys"].clone())?;
                        let result = spectre_core::recovered::render_preparation_order(&keys);
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Render preparation mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("render_overlap_flags") => {
                        let intervals: Vec<[i16; 2]> =
                            serde_json::from_value(case["intervals"].clone())?;
                        let flags: Vec<u8> = serde_json::from_value(case["flags"].clone())?;
                        let result = spectre_core::recovered::render_overlap_flags(
                            &intervals,
                            &flags,
                            a as usize,
                            b as u8,
                            case["filled"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Render overlap mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("render_dispatch_order") => {
                        let classes: Vec<i8> = serde_json::from_value(case["classes"].clone())?;
                        let result = spectre_core::recovered::render_dispatch_order(
                            &classes,
                            case["filled"].as_bool().unwrap(),
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Render dispatch mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("horizon_bands") => {
                        let result = spectre_core::recovered::horizon_bands(
                            serde_json::from_value(case["input"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Horizon bands mismatch".into());
                        }
                        0
                    }
                    Some("face_viewpoint") => {
                        let table: Vec<i32> = serde_json::from_value(case["table"].clone())?;
                        let table: [i32; 360] =
                            table.try_into().map_err(|_| "Direction table length")?;
                        let result = spectre_core::recovered::face_viewpoint(
                            serde_json::from_value(case["delta"].clone())?,
                            case["heading"].as_i64().unwrap() as i32,
                            &table,
                            case["camera_height"].as_i64().unwrap() as i16,
                            case["object_height"].as_i64().unwrap() as i16,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Face viewpoint mismatch".into());
                        }
                        0
                    }
                    Some("face_plane_visibility") => {
                        let plane = serde_json::from_value(case["plane"].clone())?;
                        let point = serde_json::from_value(case["point"].clone())?;
                        let result = spectre_core::recovered::evaluate_face_plane(&plane, point);
                        let visible = spectre_core::recovered::face_plane_visible(&plane, point);
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(visible) != case["visible"]
                        {
                            return Err("Face plane visibility mismatch".into());
                        }
                        0
                    }
                    Some("solid_origin_visible") => {
                        let result = spectre_core::recovered::solid_origin_visible(
                            case["a"].as_i64().unwrap() as i16,
                            case["b"].as_i64().unwrap() as i16,
                            case["full_size"].as_bool().unwrap(),
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Solid visibility mismatch".into());
                        }
                        0
                    }
                    Some("clip_project_face") => {
                        let vertices: Vec<[i16; 5]> =
                            serde_json::from_value(case["vertices"].clone())?;
                        let indices: Vec<usize> = serde_json::from_value(case["indices"].clone())?;
                        let result = spectre_core::recovered::clip_project_face(
                            &vertices,
                            &indices,
                            serde_json::from_value(case["origin"].clone())?,
                            serde_json::from_value(case["offset"].clone())?,
                        )
                        .ok_or("Invalid face indices")?;
                        if serde_json::json!(result) != case["result"] {
                            return Err("Clipped face mismatch".into());
                        }
                        0
                    }
                    Some("render_record_planar") => {
                        let result = spectre_core::recovered::render_record_planar(
                            serde_json::from_value(case["delta"].clone())?,
                            serde_json::from_value(case["basis"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Render record planar mismatch".into());
                        }
                        0
                    }
                    Some("model_view_transform") => {
                        let table: Vec<i32> = serde_json::from_value(case["table"].clone())?;
                        let table: [i32; 360] =
                            table.try_into().map_err(|_| "Direction table length")?;
                        let (rows, translation) = spectre_core::recovered::model_view_transform(
                            serde_json::from_value(case["position"].clone())?,
                            case["camera_height"].as_i64().ok_or("Camera height")? as i16,
                            case["heading"].as_i64().ok_or("Heading")? as i32,
                            &table,
                        );
                        if serde_json::json!(rows) != case["rows"]
                            || serde_json::json!(translation) != case["translation"]
                        {
                            return Err("Model view transform mismatch".into());
                        }
                        0
                    }
                    Some("project_vertex_record") => {
                        let mut vertex = serde_json::from_value(case["initial"].clone())?;
                        spectre_core::recovered::project_vertex_record(&mut vertex);
                        if serde_json::json!(vertex) != case["result"] {
                            return Err("Vertex projection mismatch".into());
                        }
                        0
                    }
                    Some("transform_project_vertices") => {
                        let vertices: Vec<[i16; 3]> =
                            serde_json::from_value(case["vertices"].clone())?;
                        let mut output: Vec<[i16; 5]> =
                            serde_json::from_value(case["initial"].clone())?;
                        spectre_core::recovered::transform_project_vertices(
                            &vertices,
                            serde_json::from_value(case["rows"].clone())?,
                            serde_json::from_value(case["translation"].clone())?,
                            &mut output,
                        )
                        .ok_or("Vertex count mismatch")?;
                        if serde_json::json!(output) != case["result"] {
                            return Err("Transformed vertices mismatch".into());
                        }
                        0
                    }
                    Some("deferred_frame_cycle") => {
                        let mut state: spectre_core::recovered::DeferredFrameCycle =
                            serde_json::from_value(case["initial_state"].clone())?;
                        let mut pixels: Vec<u8> =
                            serde_json::from_value(case["initial_pixels"].clone())?;
                        state
                            .present_8(
                                &mut pixels,
                                0x45000,
                                serde_json::from_value(case["remapping"].clone())?,
                            )
                            .ok_or("Frame cycle not ready or outside framebuffer")?;
                        if serde_json::json!(state) != case["result_state"]
                            || serde_json::json!(pixels) != case["result_pixels"]
                        {
                            return Err("Complete frame cycle mismatch".into());
                        }
                        0
                    }
                    Some("deferred_queue_merge_clear") => {
                        let mut destination: spectre_core::recovered::DeferredFrameQueue =
                            serde_json::from_value(case["destination"].clone())?;
                        let source = serde_json::from_value(case["source_queue"].clone())?;
                        destination.append_dirty(&source);
                        if serde_json::json!(destination) != case["merged"] {
                            return Err("Queue merge mismatch".into());
                        }
                        destination.clear_if_dirty();
                        if serde_json::json!(destination) != case["cleared"] {
                            return Err("Queue reset mismatch".into());
                        }
                        0
                    }
                    Some("deferred_queue_rotation") => {
                        let before: [Vec<u8>; 3] = serde_json::from_value(case["before"].clone())?;
                        let mut queues: [spectre_core::recovered::DeferredFrameQueue; 3] =
                            std::array::from_fn(|i| {
                                let mut queue =
                                    spectre_core::recovered::DeferredFrameQueue::default();
                                // Keep each opaque header byte as a marker in a logical queue.
                                queue.colors[0].pixels[0] =
                                    before[i].iter().map(|v| u32::from(*v)).collect();
                                queue
                            });
                        spectre_core::recovered::rotate_deferred_queues(&mut queues);
                        let after: Vec<_> = queues
                            .iter()
                            .map(|q| q.colors[0].pixels[0].clone())
                            .collect();
                        if serde_json::json!(after) != case["after"] {
                            return Err("Queue rotation mismatch".into());
                        }
                        0
                    }
                    Some("deferred_queue_8") => {
                        let mut queue = spectre_core::recovered::DeferredFrameQueue::default();
                        let requests: Vec<(u8, Vec<[i16; 4]>)> =
                            serde_json::from_value(case["requests"].clone())?;
                        let mut snapshots = Vec::new();
                        for (color, lines) in requests {
                            queue
                                .queue_lines(
                                    color,
                                    &lines,
                                    spectre_core::recovered::LineRasterTarget {
                                        viewport: [16, 12],
                                        origin: [0, 0],
                                        stride: 16,
                                        base: 0x45000,
                                    },
                                )
                                .ok_or("Invalid queue color")?;
                            snapshots.push(
                                queue
                                    .colors
                                    .iter()
                                    .map(|c| c.lines.len())
                                    .collect::<Vec<_>>(),
                            );
                        }
                        if serde_json::json!(snapshots) != case["counts"]
                            || serde_json::json!(queue) != case["queue"]
                        {
                            return Err("Deferred queue mismatch".into());
                        }
                        let remapping = serde_json::from_value(case["remapping"].clone())?;
                        let mut pixels = vec![63_u8; 16 * 12];
                        spectre_core::recovered::draw_deferred_frame_8(
                            &mut pixels,
                            0x45000,
                            &queue.colors,
                            remapping,
                            a as u8,
                            false,
                        )
                        .ok_or("Queued draw outside framebuffer")?;
                        if serde_json::json!(pixels) != case["drawn"] {
                            return Err("Queued framebuffer mismatch".into());
                        }
                        spectre_core::recovered::draw_deferred_frame_8(
                            &mut pixels,
                            0x45000,
                            &queue.colors,
                            remapping,
                            a as u8,
                            true,
                        )
                        .ok_or("Queued erase outside framebuffer")?;
                        if serde_json::json!(pixels) != case["erased"] {
                            return Err("Queued erasure mismatch".into());
                        }
                        0
                    }
                    Some("deferred_frame_8") => {
                        let batches = serde_json::from_value(case["batches"].clone())?;
                        let remapping = serde_json::from_value(case["remapping"].clone())?;
                        let mut framebuffer: Vec<u8> =
                            serde_json::from_value(case["initial"].clone())?;
                        spectre_core::recovered::draw_deferred_frame_8(
                            &mut framebuffer,
                            0x45000,
                            &batches,
                            remapping,
                            a as u8,
                            false,
                        )
                        .ok_or("Draw batch outside framebuffer")?;
                        if serde_json::json!(framebuffer) != case["drawn"] {
                            return Err("Deferred frame draw mismatch".into());
                        }
                        spectre_core::recovered::draw_deferred_frame_8(
                            &mut framebuffer,
                            0x45000,
                            &batches,
                            remapping,
                            a as u8,
                            true,
                        )
                        .ok_or("Erase batch outside framebuffer")?;
                        if serde_json::json!(framebuffer) != case["erased"] {
                            return Err("Deferred frame erase mismatch".into());
                        }
                        0
                    }
                    Some("deferred_rgb_palettes") => {
                        let base: Vec<[u16; 3]> = serde_json::from_value(case["base"].clone())?;
                        let result = spectre_core::recovered::deferred_rgb_palettes(&base);
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(result[(a as u32 & 1) as usize])
                                != case["selected"]
                        {
                            return Err("Deferred RGB palette mismatch".into());
                        }
                        0
                    }
                    Some("deferred_phase_pixels") => {
                        let base: Vec<[u16; 3]> = serde_json::from_value(case["base"].clone())?;
                        let palettes = spectre_core::recovered::deferred_rgb_palettes(&base);
                        let phases: Vec<Vec<([i16; 4], i16)>> =
                            serde_json::from_value(case["phases"].clone())?;
                        let mut framebuffer = vec![63_u8; 16 * 12];
                        let mut results = Vec::new();
                        for (phase, lines) in phases.iter().enumerate() {
                            for (points, mapped) in lines {
                                if let Some(record) = spectre_core::recovered::deferred_line_record(
                                    *points,
                                    [16, 12],
                                    [0, 0],
                                    16,
                                    0x45000,
                                    8,
                                ) {
                                    spectre_core::recovered::raster_deferred_lines_8(
                                        &mut framebuffer,
                                        0x45000,
                                        &[record],
                                        spectre_core::recovered::deferred_line_mask(
                                            8,
                                            *mapped,
                                            phase as u8,
                                        ),
                                    )
                                    .ok_or("Phase line outside framebuffer")?;
                                }
                            }
                            results.push(serde_json::json!({"pixels":framebuffer,"rgb":framebuffer.iter().map(|p|palettes[phase][*p as usize]).collect::<Vec<_>>()}));
                        }
                        if serde_json::json!(results) != case["results"] {
                            return Err("Composed phase pixels mismatch".into());
                        }
                        0
                    }
                    Some("deferred_line_palette") => {
                        let result = spectre_core::recovered::deferred_line_palette(
                            a as i16,
                            serde_json::from_value(case["remapping"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Deferred line palette mismatch".into());
                        }
                        0
                    }
                    Some("deferred_line_mask") => {
                        let result = spectre_core::recovered::deferred_line_mask(
                            a as i16,
                            b as i16,
                            case["phase"].as_u64().ok_or("Missing phase")? as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Deferred line mask mismatch".into());
                        }
                        0
                    }
                    Some("deferred_lines_8") => {
                        let endpoints: Vec<[i16; 4]> =
                            serde_json::from_value(case["endpoints"].clone())?;
                        let records: Vec<_> = endpoints
                            .into_iter()
                            .filter_map(|points| {
                                spectre_core::recovered::deferred_line_record(
                                    points,
                                    [64, 48],
                                    [0, 0],
                                    64,
                                    0x45000,
                                    8,
                                )
                            })
                            .collect();
                        let mut framebuffer: Vec<u8> =
                            serde_json::from_value(case["initial"].clone())?;
                        if serde_json::json!(records) != case["records"] {
                            return Err("Integrated line clipping mismatch".into());
                        }
                        spectre_core::recovered::raster_deferred_lines_8(
                            &mut framebuffer,
                            0x45000,
                            &records,
                            a as u32,
                        )
                        .ok_or("Line raster address outside framebuffer")?;
                        if serde_json::json!(framebuffer) != case["result"] {
                            return Err(format!(
                                "Deferred line pixels mismatch: {}",
                                case["endpoints"]
                            )
                            .into());
                        }
                        0
                    }
                    Some("deferred_line_record") => {
                        let result = spectre_core::recovered::deferred_line_record(
                            serde_json::from_value(case["endpoints"].clone())?,
                            serde_json::from_value(case["viewport"].clone())?,
                            serde_json::from_value(case["origin"].clone())?,
                            a as i16,
                            case["base"].as_u64().ok_or("Missing raster base")? as u32,
                            b as i16,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Deferred line record mismatch: {case}").into());
                        }
                        0
                    }
                    Some("line_palette") => {
                        let result = spectre_core::recovered::line_palette(
                            a as i16,
                            b as u8,
                            serde_json::from_value(case["remapping"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Line palette mismatch: {case}").into());
                        }
                        0
                    }
                    Some("edge_draw_batches") => {
                        let mut edges: Vec<spectre_core::recovered::ProjectedEdge> =
                            serde_json::from_value(case["edges"].clone())?;
                        let flags: [u8; 4] = serde_json::from_value(case["flags"].clone())?;
                        let batches = spectre_core::recovered::edge_draw_batches(
                            &mut edges, a as i16, flags[0], flags[1], flags[2], flags[3],
                        );
                        if serde_json::json!(batches) != case["batches"]
                            || serde_json::json!(
                                edges.iter().map(|e| e.classification).collect::<Vec<_>>()
                            ) != case["classifications"]
                        {
                            return Err(format!("Edge drawing dispatcher mismatch: {case}").into());
                        }
                        0
                    }
                    Some("edge_scan") => {
                        let visibility = serde_json::from_value(case["visibility"].clone())?;
                        let result = if case["partitioned"]
                            .as_bool()
                            .ok_or("Missing edge scan mode")?
                        {
                            spectre_core::recovered::edge_scan_partitioned(
                                a as u8,
                                b as u8,
                                visibility,
                                serde_json::from_value(case["coordinates"].clone())?,
                            )
                        } else {
                            (
                                spectre_core::recovered::edge_scan_simple(a as u8, visibility),
                                0xa5,
                            )
                        };
                        if serde_json::json!(result.0) != case["queued"]
                            || serde_json::json!(result.1) != case["state"]
                        {
                            return Err(format!("Edge scan mismatch: {case}").into());
                        }
                        0
                    }
                    Some("insert_model_edge") => {
                        let mut edges = serde_json::from_value(case["initial"].clone())?;
                        spectre_core::recovered::insert_model_edge(
                            &mut edges,
                            serde_json::from_value(case["vertices"].clone())?,
                            serde_json::from_value(case["visibility_refs"].clone())?,
                            a as u8,
                        );
                        if serde_json::json!(edges) != case["result"] {
                            return Err(format!("Model edge insertion mismatch: {case}").into());
                        }
                        0
                    }
                    Some("flag_faces") => {
                        let (_, faces) = spectre_core::model::PackedShape::flag(true);
                        if serde_json::json!(&faces[1..]) != case["faces"]
                            || case["colors"] != serde_json::json!(([a as i8 as i16; 2]))
                        {
                            return Err(format!("Flag face submission mismatch: {case}").into());
                        }
                        0
                    }
                    Some("column_faces") => {
                        if serde_json::json!(spectre_core::model::PackedShape::column_faces())
                            != case["faces"]
                            || case["colors"] != serde_json::json!(([a as i8 as i16; 10]))
                        {
                            return Err(format!("Column face submission mismatch: {case}").into());
                        }
                        0
                    }
                    Some("fixed_obstacle_faces") => {
                        let expected =
                            spectre_core::model::PackedShape::fixed_obstacle_color(a as i16);
                        if serde_json::json!(spectre_core::model::PackedShape::box_faces())
                            != case["faces"]
                            || serde_json::json!(expected) != case["kind"]
                            || case["colors"] != serde_json::json!(([b as i8 as i16; 6]))
                        {
                            return Err(format!("Fixed obstacle faces mismatch: {case}").into());
                        }
                        0
                    }
                    Some("tetrahedron_faces") => {
                        let faces = spectre_core::model::PackedShape::tetrahedron_faces();
                        if serde_json::json!(faces) != case["faces"]
                            || case["colors"] != serde_json::json!(([a as i8 as i16; 4]))
                        {
                            return Err("Tetrahedron face submission mismatch".into());
                        }
                        0
                    }
                    Some("face_light_components") => {
                        let result = spectre_core::recovered::face_light_components(
                            serde_json::from_value(case["direction"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("Face light components mismatch".into());
                        }
                        0
                    }
                    Some("rendered_face_color") => {
                        let mut state = spectre_core::recovered::FaceFlashState {
                            deadline: case["deadline"].as_i64().unwrap() as i32,
                            toggle: case["toggle"].as_u64().unwrap() as u8,
                        };
                        let color = spectre_core::recovered::rendered_face_color(
                            serde_json::from_value(case["face"].clone())?,
                            a as u8,
                            b as u8,
                            serde_json::from_value(case["ticks"].clone())?,
                            &mut state,
                        );
                        if serde_json::json!([
                            i64::from(color),
                            i64::from(state.deadline),
                            i64::from(state.toggle)
                        ]) != case["result"]
                        {
                            return Err(format!("Rendered face color mismatch: {case}").into());
                        }
                        0
                    }
                    Some("face_plane") => {
                        let result = spectre_core::recovered::face_plane(
                            serde_json::from_value(case["vertices"].clone())?,
                            a as i16,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Face plane mismatch: {case}").into());
                        }
                        0
                    }
                    Some("pattern_span_8") => {
                        let pattern = case["pattern"].as_u64().unwrap() as u32;
                        let mut pixels: Vec<u8> = serde_json::from_value(case["initial"].clone())?;
                        let start = case["start"].as_u64().unwrap() as usize;
                        let count = case["count"].as_u64().unwrap() as usize;
                        for (x, pixel) in pixels.iter_mut().enumerate().skip(start).take(count) {
                            *pixel =
                                spectre_core::recovered::face_pattern_pixel_8(pattern, x as i32, a);
                        }
                        if serde_json::json!(pixels) != case["result"] {
                            return Err(format!("Pattern span mismatch: {case}").into());
                        }
                        0
                    }
                    Some("interpolate_rgb") => {
                        let result = spectre_core::recovered::interpolate_rgb(
                            serde_json::from_value(case["base"].clone())?,
                            serde_json::from_value(case["target"].clone())?,
                            a,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("RGB interpolation mismatch: {case}").into());
                        }
                        0
                    }
                    Some("rgb_shade_ramps_8") => {
                        let result = spectre_core::recovered::rgb_shade_ramps_8(
                            &serde_json::from_value(case["base"].clone())?,
                            serde_json::from_value(case["target"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err("RGB shade ramp mismatch".into());
                        }
                        0
                    }
                    Some("shade_pixel_index") => {
                        let map: Vec<i16> = serde_json::from_value(case["map"].clone())?;
                        if serde_json::json!(spectre_core::recovered::shade_pixel_index(
                            a as i16,
                            b as u8,
                            case["depth"].as_i64().unwrap() as i16,
                            case["four_bit_color"].as_bool().unwrap(),
                            &map
                        )) != case["result"]
                        {
                            return Err(format!("Shade pixel index mismatch: {case}").into());
                        }
                        0
                    }
                    Some("face_pixel_pattern_8") => {
                        let map: Vec<i16> = serde_json::from_value(case["map"].clone())?;
                        if serde_json::json!(spectre_core::recovered::face_pixel_pattern_8(
                            a as i16, b as u8, &map
                        )) != case["result"]
                        {
                            return Err(format!("Face pixel pattern mismatch: {case}").into());
                        }
                        0
                    }
                    Some("face_light_shade") => {
                        i32::from(spectre_core::recovered::face_light_shade(
                            serde_json::from_value(case["normal"].clone())?,
                            serde_json::from_value(case["light"].clone())?,
                        ))
                    }
                    Some("adjust_face_shade") => {
                        i32::from(spectre_core::recovered::adjust_face_shade(
                            a as i16,
                            case["bright"].as_bool().unwrap(),
                            case["fade"].as_bool().unwrap(),
                            b as i16,
                        ))
                    }
                    Some("face_color_selector") => {
                        let data: Vec<u8> = serde_json::from_value(case["data"].clone())?;
                        if serde_json::json!(spectre_core::recovered::face_color_selector(
                            a as i16, &data
                        )) != case["result"]
                        {
                            return Err(format!("Face color selector mismatch: {case}").into());
                        }
                        0
                    }
                    Some("base_palette") => {
                        let data: Vec<u8> = serde_json::from_value(case["data"].clone())?;
                        if serde_json::json!(spectre_core::recovered::base_palette(&data))
                            != case["result"]
                        {
                            return Err("Base palette mismatch".into());
                        }
                        0
                    }
                    Some("color_bindings") => {
                        let data: Vec<u8> = serde_json::from_value(case["data"].clone())?;
                        if serde_json::json!(spectre_core::recovered::color_bindings(&data))
                            != case["result"]
                        {
                            return Err("Color bindings mismatch".into());
                        }
                        0
                    }
                    Some("ground_points") => {
                        let position = serde_json::from_value(case["position"].clone())?;
                        let direction = serde_json::from_value(case["direction"].clone())?;
                        let center = serde_json::from_value(case["center"].clone())?;
                        let table: Vec<[i16; 2]> = serde_json::from_value(case["table"].clone())?;
                        let full_display = case["full_display"].as_bool().ok_or("Missing mode")?;
                        if serde_json::json!(spectre_core::recovered::ground_points(
                            position,
                            direction,
                            a,
                            center,
                            full_display,
                            &table
                        )) != case["result"]
                        {
                            return Err(format!("Ground pipeline mismatch: position={position:?}, direction={direction:?}, height={a}, full={full_display}").into());
                        }
                        0
                    }
                    Some("ground_camera_basis") => {
                        let position = serde_json::from_value(case["position"].clone())?;
                        let direction = serde_json::from_value(case["direction"].clone())?;
                        if serde_json::json!(spectre_core::recovered::ground_camera_basis(
                            position, direction, a
                        )) != case["result"]
                        {
                            return Err(format!("Ground camera basis mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ground_visible_bounds") => {
                        let origin = serde_json::from_value(case["origin"].clone())?;
                        let edges = serde_json::from_value(case["edges"].clone())?;
                        if serde_json::json!(spectre_core::recovered::ground_visible_bounds(
                            origin, edges, a, b as i16
                        )) != case["result"]
                        {
                            return Err(format!("Ground visible bounds mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ground_grid_defaults") => {
                        let (mask, divisions) = spectre_core::recovered::ground_grid_defaults();
                        if serde_json::json!(mask.as_slice()) != case["mask"]
                            || i64::from(divisions) != case["divisions"].as_i64().unwrap()
                        {
                            return Err("Ground grid defaults mismatch".into());
                        }
                        0
                    }
                    Some("ground_grid_spacing") => {
                        if serde_json::json!(spectre_core::recovered::ground_grid_spacing(
                            a as i16, b as i16
                        )) != case["result"]
                        {
                            return Err("Ground grid spacing mismatch".into());
                        }
                        0
                    }
                    Some("ground_sparse_row") => {
                        if serde_json::json!(spectre_core::recovered::ground_sparse_row(
                            a as i16, b as i16
                        )) != case["result"]
                        {
                            return Err("Ground sparse row mismatch".into());
                        }
                        0
                    }
                    Some("project_ground_row") => {
                        let start = serde_json::from_value(case["start"].clone())?;
                        let step = serde_json::from_value(case["step"].clone())?;
                        let center = serde_json::from_value(case["center"].clone())?;
                        let table: Vec<[i16; 2]> = serde_json::from_value(case["table"].clone())?;
                        let count = case["count"].as_i64().ok_or("Missing count")? as i16;
                        let full_display = case["full_display"]
                            .as_bool()
                            .ok_or("Missing display mode")?;
                        if serde_json::json!(spectre_core::recovered::project_ground_row(
                            start,
                            step,
                            count,
                            a,
                            center,
                            &table,
                            full_display
                        )) != case["result"]
                        {
                            return Err("Ground row mismatch".into());
                        }
                        0
                    }
                    Some("project_ground_point") => {
                        let point = serde_json::from_value(case["point"].clone())?;
                        let center = serde_json::from_value(case["center"].clone())?;
                        let table: Vec<[i16; 2]> = serde_json::from_value(case["table"].clone())?;
                        if serde_json::json!(spectre_core::recovered::project_ground_point(
                            point, a, center, &table
                        )) != case["result"]
                        {
                            return Err(format!("Ground point mismatch: {case}").into());
                        }
                        0
                    }
                    Some("damage_sounds") => {
                        let result = spectre_core::recovered::damage_sounds(
                            a != 0,
                            b as u8,
                            case["lives"].as_i64().unwrap() as i16,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Damage sounds mismatch: {case}").into());
                        }
                        0
                    }
                    Some("damage_response_fields") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let bytes: Vec<u8> = serde_json::from_value(case["participant"].clone())?;
                        let mut participant: [u8; 256] =
                            bytes.try_into().map_err(|_| "Invalid participant")?;
                        let input = serde_json::from_value(case["input"].clone())?;
                        let result = spectre_core::recovered::damage_response_fields(
                            &mut record,
                            &mut participant,
                            input,
                        );
                        if serde_json::json!(result) != case["notification"]
                            || serde_json::json!(record.as_slice()) != case["after_record"]
                            || serde_json::json!(participant.as_slice())
                                != case["after_participant"]
                        {
                            return Err(format!("Damage response fields mismatch: {case}").into());
                        }
                        0
                    }
                    Some("projectile_object_contact") => {
                        let result = spectre_core::recovered::projectile_object_contact(
                            a as u8,
                            b as i16,
                            case["target_kind"].as_u64().unwrap() as u8,
                            case["health"].as_i64().unwrap() as i16,
                        );
                        if serde_json::json!(result) != case["action"] {
                            return Err(format!("Projectile object route mismatch: {case}").into());
                        }
                        0
                    }
                    Some("projectile_environment_contact") => {
                        let row: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let record: [u8; 128] = row.try_into().unwrap();
                        i32::from(spectre_core::recovered::projectile_environment_contact(
                            &record, a, b as i16,
                        ))
                    }
                    Some("projectile_contact_retirement") => {
                        let row: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 128] = row.try_into().unwrap();
                        let mut counts = serde_json::from_value(case["counts"].clone())?;
                        let mut calls = Vec::new();
                        let enabled =
                            spectre_core::recovered::projectile_contact_retirement_enabled(
                                case["mode"].as_i64().unwrap() as i16,
                            );
                        let result = spectre_core::recovered::retire_projectile_contact(
                            &mut record,
                            a,
                            b,
                            u8::from(enabled),
                            &mut counts,
                            |r, elapsed| {
                                calls.push(serde_json::json!([elapsed, r[0x14], r[0x1a]]));
                                if let Some(direction) = case.get("direction") {
                                    let direction =
                                        serde_json::from_value(direction.clone()).unwrap();
                                    if r[0x14] == 29 {
                                        if elapsed != 0 {
                                            spectre_core::recovered::move_bouncing_projectile(
                                                r, direction, elapsed,
                                            );
                                        }
                                        return;
                                    }
                                    let position = std::array::from_fn(|i| {
                                        i32::from_be_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
                                    });
                                    let moved = spectre_core::recovered::move_basic_shot(
                                        position, direction, elapsed,
                                    );
                                    for (i, value) in moved.iter().enumerate() {
                                        r[i * 4..i * 4 + 4].copy_from_slice(&value.to_be_bytes());
                                    }
                                } else {
                                    let x = i32::from_be_bytes(r[..4].try_into().unwrap());
                                    r[..4].copy_from_slice(&x.wrapping_add(elapsed).to_be_bytes());
                                }
                            },
                        );
                        if serde_json::json!(result) != case["action"]
                            || serde_json::json!(calls) != case["calls"]
                            || serde_json::json!(record.to_vec()) != case["after"]
                            || serde_json::json!(counts) != case["after_counts"]
                        {
                            return Err(
                                format!("Projectile contact retirement mismatch: {case}").into()
                            );
                        }
                        0
                    }
                    Some("ownerless_enemy_damage") => {
                        let record: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 128] = record.try_into().unwrap();
                        let participant: Vec<u8> =
                            serde_json::from_value(case["participant"].clone())?;
                        let mut participant: [u8; 256] = participant.try_into().unwrap();
                        let phase = serde_json::from_value(case["phase"].clone())?;
                        let health = spectre_core::recovered::participant_health_after_guard(
                            i16::from_be_bytes([participant[0x5a], participant[0x5b]]),
                            a as i16,
                            participant[8] as i8,
                            phase,
                        );
                        participant[0x5a..0x5c].copy_from_slice(&health.to_be_bytes());
                        let killed = spectre_core::recovered::damage_killed(health, 0, 0);
                        spectre_core::recovered::damage_response_fields(
                            &mut record,
                            &mut participant,
                            spectre_core::recovered::DamageResponseInput {
                                now: b,
                                local: 1,
                                victim: 2,
                                attacker: 0,
                                killed,
                                game_mode: 0,
                            },
                        );
                        let mut calls = Vec::new();
                        if killed
                            & spectre_core::recovered::death_burst_gate(
                                participant[5],
                                participant[8],
                                case["level"].as_i64().unwrap() as i16,
                            )
                            != 0
                        {
                            calls.push(serde_json::json!(["burst", 0, 1]));
                        }
                        if killed != 0 {
                            calls.push(serde_json::json!(["fragments", 0, 1]));
                        }
                        for (channel, request) in
                            spectre_core::recovered::damage_sounds(false, killed, 1).requests
                        {
                            calls.push(serde_json::json!(["sound", channel, request]));
                        }
                        if serde_json::json!(record.to_vec()) != case["after_record"]
                            || serde_json::json!(participant.to_vec()) != case["after_participant"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!(
                                "Ownerless enemy damage mismatch: {case}; calls {calls:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("projectile_lifecycle_sequence") => {
                        let rows: Vec<Vec<u8>> = serde_json::from_value(case["records"].clone())?;
                        let mut records: Vec<[u8; 128]> =
                            rows.into_iter().map(|r| r.try_into().unwrap()).collect();
                        let rows: Vec<Vec<u8>> =
                            serde_json::from_value(case["participants"].clone())?;
                        let participants: [[u8; 256]; 12] = rows
                            .into_iter()
                            .map(|r| r.try_into().unwrap())
                            .collect::<Vec<_>>()
                            .try_into()
                            .map_err(|_| "Invalid participants")?;
                        let damage = serde_json::from_value(case["damage"].clone())?;
                        let mut counts = serde_json::from_value(case["counts"].clone())?;
                        for step in case["steps"].as_array().unwrap() {
                            let (action, requests) =
                                spectre_core::recovered::projectile_lifecycle_requests(
                                    &mut records,
                                    &participants,
                                    1,
                                    step["now"].as_i64().unwrap() as i32,
                                    &mut counts,
                                    &damage,
                                )
                                .ok_or("Invalid lifecycle input")?;
                            if serde_json::json!(action) != step["action"]
                                || serde_json::json!(requests) != step["requests"]
                                || serde_json::json!(counts) != step["counts"]
                                || serde_json::json!(records[0].to_vec()) != step["source"]
                            {
                                return Err(format!("Lifecycle sequence mismatch: {step}").into());
                            }
                        }
                        0
                    }
                    Some("projectile_area_requests" | "projectile_impact_area_requests") => {
                        let rows: Vec<Vec<u8>> = serde_json::from_value(case["records"].clone())?;
                        let records: Vec<[u8; 128]> = rows
                            .into_iter()
                            .map(|row| row.try_into().unwrap())
                            .collect();
                        let rows: Vec<Vec<u8>> =
                            serde_json::from_value(case["participants"].clone())?;
                        let participants: Vec<[u8; 256]> = rows
                            .into_iter()
                            .map(|row| row.try_into().unwrap())
                            .collect();
                        let participants: [[u8; 256]; 12] = participants
                            .try_into()
                            .map_err(|_| "Invalid participants")?;
                        let damage = serde_json::from_value(case["damage"].clone())?;
                        let admitted = case["routine"] != "projectile_impact_area_requests"
                            || spectre_core::recovered::projectile_impact_uses_area(
                                i16::from_be_bytes([records[0][0x1c], records[0][0x1d]]),
                                1,
                                case["game_mode"].as_u64().unwrap() as u8,
                                case["authority"].as_u64().unwrap() as u8,
                            );
                        let result = if admitted {
                            spectre_core::recovered::projectile_area_requests(
                                &records,
                                &participants,
                                b as i16,
                                a,
                                &damage,
                            )
                        } else {
                            Some(Vec::new())
                        };
                        if serde_json::json!(result) != case["requests"] {
                            return Err(format!("Area effect mismatch: {case}").into());
                        }
                        0
                    }
                    Some("projectile_lifecycle") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let mut counts = serde_json::from_value(case["counts"].clone())?;
                        let result = spectre_core::recovered::projectile_lifecycle(
                            &mut record,
                            a,
                            &mut counts,
                        );
                        if serde_json::json!(result) != case["action"]
                            || serde_json::json!(record.as_slice()) != case["after"]
                            || serde_json::json!(counts) != case["after_counts"]
                        {
                            return Err(format!("Projectile lifecycle mismatch: {case}").into());
                        }
                        0
                    }
                    Some("step_bouncing_projectile") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let values: Vec<i32> = serde_json::from_value(case["table"].clone())?;
                        let table: [i32; 360] = values.try_into().map_err(|_| "Invalid table")?;
                        let cells: Vec<Vec<i16>> = serde_json::from_value(case["grid"].clone())?;
                        let mut grid: [Vec<i16>; 256] =
                            cells.try_into().map_err(|_| "Invalid grid")?;
                        let result = spectre_core::recovered::step_bouncing_projectile(
                            &mut record,
                            b as i16,
                            a,
                            &table,
                            &mut grid,
                        );
                        if result.is_none()
                            || serde_json::json!(record.as_slice()) != case["after"]
                            || serde_json::json!(grid.as_slice()) != case["after_grid"]
                        {
                            return Err(format!("Bouncing projectile step mismatch: {case}").into());
                        }
                        0
                    }
                    Some("move_bouncing_projectile") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid record")?;
                        let direction = serde_json::from_value(case["direction"].clone())?;
                        spectre_core::recovered::move_bouncing_projectile(
                            &mut record,
                            direction,
                            a,
                        );
                        if serde_json::json!(record.as_slice()) != case["after"] {
                            return Err(format!("Bouncing projectile mismatch: {case}").into());
                        }
                        0
                    }
                    Some("weapon_fire_sound") => {
                        let result = spectre_core::recovered::weapon_fire_sound(a as i8);
                        if serde_json::json!(result) != case["request"] {
                            return Err(format!("Weapon sound mismatch: {case}").into());
                        }
                        0
                    }
                    Some("concurrent_shot_limit") => {
                        i32::from(spectre_core::recovered::concurrent_shot_limit(a as u8))
                    }
                    Some("admit_weapon") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        let result = spectre_core::recovered::admit_weapon(input, || a as i16);
                        if serde_json::to_value(result)? != case["result"] {
                            return Err(format!("Weapon admission mismatch: {case}").into());
                        }
                        0
                    }
                    Some("finalize_weapon_record") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid weapon record")?;
                        let mut count = case["count"].as_i64().unwrap() as i16;
                        spectre_core::recovered::finalize_weapon_record(
                            &mut record,
                            a as i8,
                            b,
                            case["owner"].as_i64().unwrap() as i8,
                            &mut count,
                        );
                        if serde_json::json!(record.to_vec()) != case["after"]
                            || count as i64 != case["after_count"].as_i64().unwrap()
                        {
                            return Err(format!("Weapon finalization mismatch: {case}").into());
                        }
                        0
                    }
                    Some("original_geometry_slot") => {
                        let mut counter = a as i16;
                        let result = spectre_core::recovered::original_geometry_slot(&mut counter);
                        if counter as i32 != b {
                            return Err(format!("Geometry counter mismatch: {case}").into());
                        }
                        let mut guarded = a as i16;
                        let admitted =
                            spectre_core::recovered::allocate_geometry_slot(&mut guarded);
                        if (0..302).contains(&a) {
                            if admitted != Some(result) || guarded != counter {
                                return Err(
                                    format!("Geometry allocation guard mismatch: {case}").into()
                                );
                            }
                        } else if admitted.is_some() || guarded != a as i16 {
                            return Err(format!(
                                "Geometry allocation guard mutated invalid state: {case}"
                            )
                            .into());
                        }
                        result as i32
                    }
                    Some("initialize_weapon_record") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid weapon record")?;
                        let input = serde_json::from_value(case["input"].clone())?;
                        let mut calls = 0;
                        spectre_core::recovered::initialize_weapon_record(
                            &mut record,
                            input,
                            || {
                                calls += 1;
                                case["sample"].as_i64().unwrap() as i16
                            },
                        );
                        if serde_json::json!(record.to_vec()) != case["after"]
                            || calls != case["calls"].as_u64().unwrap()
                        {
                            return Err(format!("Weapon record mismatch: {case}").into());
                        }
                        0
                    }
                    Some("weapon_initial_heading") => {
                        let mut random_calls = 0;
                        let mut target_calls = 0;
                        let angle: Option<i16> = serde_json::from_value(case["angle"].clone())?;
                        let result = spectre_core::recovered::weapon_initial_heading(
                            a as i8,
                            b,
                            || {
                                random_calls += 1;
                                case["sample"].as_i64().unwrap() as i16
                            },
                            || {
                                target_calls += 1;
                                angle
                            },
                        );
                        if serde_json::json!(result) != case["result"]
                            || random_calls != case["random_calls"].as_u64().unwrap()
                            || target_calls != case["target_calls"].as_u64().unwrap()
                        {
                            return Err(format!(
                                "Weapon heading mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("enemy_weapon_requests") => {
                        let result = spectre_core::recovered::enemy_weapon_requests(
                            a as i16,
                            b as i16,
                            case["game_mode"].as_u64().unwrap() as u8,
                            case["network"].as_u64().unwrap() as u8,
                            case["height"].as_i64().unwrap() as i32,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Enemy weapon request mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("enemy_fire_check") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        let result = spectre_core::recovered::enemy_fire_check(
                            input,
                            a as u8,
                            b,
                            case["flag"].as_u64().unwrap() as u8,
                            case["behavior"].as_u64().unwrap() as u8,
                            case["movement"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Enemy firing mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_commands_suppressed") => {
                        i32::from(spectre_core::recovered::ai_commands_suppressed(
                            a as u8,
                            b as u8,
                            case["remaining"].as_i64().unwrap() as i16,
                            case["distance"].as_i64().unwrap() as i32,
                            case["limit"].as_i64().unwrap() as i32,
                        ))
                    }
                    Some("ai_select_object") => {
                        let objects: Vec<spectre_core::recovered::AiObjectCandidate> =
                            serde_json::from_value(case["objects"].clone())?;
                        let samples: Vec<i16> = serde_json::from_value(case["samples"].clone())?;
                        let mut consumed = 0;
                        let result = spectre_core::recovered::ai_select_object(
                            serde_json::from_value(case["source"].clone())?,
                            serde_json::from_value(case["player"].clone())?,
                            a as i16,
                            b as i16,
                            &objects,
                            || {
                                let sample = samples[consumed];
                                consumed += 1;
                                sample
                            },
                        );
                        if consumed != case["consumed"].as_u64().unwrap() as usize {
                            return Err(format!("AI object random count mismatch: {case}").into());
                        }
                        let participants = serde_json::from_value(case["participants"].clone())?;
                        for (index, object) in objects.iter().enumerate() {
                            if spectre_core::recovered::ai_object_assignment_count(
                                index as i16 + 1,
                                &participants,
                            ) != object.assignments
                            {
                                return Err(format!("AI object assignment mismatch: {case}").into());
                            }
                        }
                        result.unwrap_or(-1) as i32
                    }
                    Some("ai_attack_tracking") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        let supplied = serde_json::from_value(case["supplied"].clone())?;
                        let called = spectre_core::recovered::ai_attack_tracking(
                            &mut state,
                            a as u8,
                            serde_json::from_value(case["source"].clone())?,
                            b as i16,
                            || supplied,
                        );
                        if serde_json::to_value(state)? != case["after"]
                            || called != case["called"].as_bool().unwrap()
                        {
                            return Err("Attack tracking mismatch".into());
                        }
                        0
                    }
                    Some("ai_track_object") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        spectre_core::recovered::ai_track_object(
                            &mut state,
                            a as i8,
                            serde_json::from_value(case["source_pose"].clone())?,
                            serde_json::from_value(case["object_pose"].clone())?,
                        );
                        if serde_json::json!(state) != case["after"] {
                            return Err(format!("AI object tracking mismatch: {case}").into());
                        }
                        0
                    }
                    Some("local_weapon_attempt") => {
                        let result = spectre_core::recovered::local_weapon_attempt(
                            serde_json::from_value(case["input"].clone())?,
                        );
                        if serde_json::json!(result.map(|r| (
                            r.timestamp,
                            r.request,
                            r.empty_sound
                        ))) != case["result"]
                        {
                            return Err(format!(
                                "Local weapon attempt mismatch: {case}; got {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("optional_weapon_fire") => {
                        let result = spectre_core::recovered::optional_weapon_fire(
                            a,
                            b,
                            case["control"].as_i64().unwrap() as i16,
                            case["selected"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Optional fire request mismatch: {case}").into());
                        }
                        0
                    }
                    Some("optional_weapon_selection") => {
                        let available: [u8; 18] =
                            serde_json::from_value(case["available"].clone())?;
                        let result = spectre_core::recovered::optional_weapon_selection(
                            a,
                            b,
                            case["control"].as_i64().unwrap() as i16,
                            case["selected"].as_u64().unwrap() as u8,
                            &available,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Optional selection timing mismatch: {case}; got {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("cycle_optional_weapon") => {
                        let available: [u8; 18] =
                            serde_json::from_value(case["available"].clone())?;
                        spectre_core::recovered::cycle_optional_weapon(a as u8, &available).unwrap()
                            as i32
                    }
                    Some("unlock_optional_weapon") => {
                        let mut available: [u8; 18] =
                            serde_json::from_value(case["available"].clone())?;
                        let mut selected = b as u8;
                        spectre_core::recovered::unlock_optional_weapon(
                            a as i16,
                            &mut selected,
                            &mut available,
                        );
                        if serde_json::json!(available) != case["after"] {
                            return Err(format!("Weapon unlock mismatch: {case}").into());
                        }
                        selected as i32
                    }
                    Some("direct_projectile_contact") => {
                        let shot: Vec<u8> = serde_json::from_value(case["shot"].clone())?;
                        let target: Vec<u8> = serde_json::from_value(case["target"].clone())?;
                        let shot: [u8; 128] = shot.try_into().map_err(|_| "Invalid shot record")?;
                        let target: [u8; 128] =
                            target.try_into().map_err(|_| "Invalid target record")?;
                        i32::from(
                            spectre_core::recovered::direct_projectile_contact(&shot, &target)
                                .ok_or("Unsupported projectile kind")?,
                        )
                    }
                    Some("smart_missile_guidance" | "smart_missile_flight") => {
                        let participants: [spectre_core::recovered::AiTargetCandidate; 12] =
                            serde_json::from_value(case["participants"].clone())?;
                        let heights: [i32; 12] = serde_json::from_value(case["heights"].clone())?;
                        let mut state: spectre_core::recovered::MissileGuidance =
                            serde_json::from_value(case["state"].clone())?;
                        let mut calls = Vec::new();
                        let elapsed = spectre_core::recovered::smart_missile_elapsed(
                            b,
                            serde_json::from_value(case["owner_kind"].clone())?,
                        );
                        spectre_core::recovered::smart_missile_guidance(
                            &mut state,
                            a as i8,
                            elapsed,
                            &participants,
                            &heights,
                            |target, source| {
                                calls.push([target, source]);
                                let index =
                                    participants.iter().position(|p| p.pose == target).unwrap();
                                case["angles"][index].as_i64().unwrap() as i16
                            },
                        );
                        if case["routine"] == "smart_missile_flight" {
                            let table: Vec<i32> =
                                serde_json::from_value(vectors["missile_direction_table"].clone())?;
                            let table: [i32; 360] = table
                                .try_into()
                                .map_err(|_| "Invalid missile direction table")?;
                            spectre_core::recovered::move_smart_missile(
                                &mut state, elapsed, &table,
                            )
                            .ok_or("Invalid missile heading")?;
                        }
                        if serde_json::json!(state) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!(
                                "Missile guidance mismatch: {case}; got {state:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("seeker_contact_retirement" | "missile_contact_retirement") => {
                        let participants: [spectre_core::recovered::AiTargetCandidate; 12] =
                            serde_json::from_value(case["participants"].clone())?;
                        let row: Vec<u8> = serde_json::from_value(case["record"].clone())?;
                        let mut record: [u8; 128] = row.try_into().unwrap();
                        let mut counts = serde_json::from_value(case["counts"].clone())?;
                        let table: Vec<i32> =
                            serde_json::from_value(vectors["movement_marker_table"].clone())?;
                        let table: [i32; 360] = table.try_into().unwrap();
                        let mut calls = Vec::new();
                        spectre_core::recovered::retire_projectile_contact(
                            &mut record,
                            a,
                            b,
                            1,
                            &mut counts,
                            |r, elapsed| {
                                if elapsed == 0 {
                                    return;
                                }
                                let pose = std::array::from_fn(|i| {
                                    i32::from_be_bytes(r[i * 4..i * 4 + 4].try_into().unwrap())
                                });
                                if case["routine"] == "missile_contact_retirement" {
                                    let mut state = spectre_core::recovered::MissileGuidance {
                                        pose,
                                        height: i32::from_be_bytes(
                                            r[0x28..0x2c].try_into().unwrap(),
                                        ),
                                        vertical_velocity: i32::from_be_bytes(
                                            r[0x24..0x28].try_into().unwrap(),
                                        ),
                                    };
                                    let adjusted = spectre_core::recovered::smart_missile_elapsed(
                                        elapsed,
                                        case["owner_kind"].as_u64().map(|v| v as u8),
                                    );
                                    let heights =
                                        serde_json::from_value(case["heights"].clone()).unwrap();
                                    spectre_core::recovered::smart_missile_guidance(
                                        &mut state,
                                        r[0x1a] as i8,
                                        adjusted,
                                        &participants,
                                        &heights,
                                        |target, source| {
                                            calls.push([target, source]);
                                            let index = participants
                                                .iter()
                                                .position(|p| p.pose == target)
                                                .unwrap();
                                            case["angles"][index].as_i64().unwrap() as i16
                                        },
                                    );
                                    spectre_core::recovered::move_smart_missile(
                                        &mut state, adjusted, &table,
                                    )
                                    .unwrap();
                                    for (offset, value) in [
                                        (0, state.pose[0]),
                                        (4, state.pose[1]),
                                        (8, state.pose[2]),
                                        (0x24, state.vertical_velocity),
                                        (0x28, state.height),
                                    ] {
                                        r[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                                    }
                                    return;
                                }
                                let heading = spectre_core::recovered::seeker_heading(
                                    r[0x1a] as i8,
                                    pose,
                                    &participants,
                                    |target, source| {
                                        calls.push([target, source]);
                                        let index = participants
                                            .iter()
                                            .position(|p| p.pose == target)
                                            .unwrap();
                                        case["angles"][index].as_i64().unwrap() as i16
                                    },
                                );
                                let direction =
                                    spectre_core::recovered::movement_direction(heading, &table)
                                        .unwrap();
                                let moved = spectre_core::recovered::move_basic_shot(
                                    [pose[0], pose[1]],
                                    direction,
                                    elapsed,
                                );
                                for (i, value) in [moved[0], moved[1], heading].iter().enumerate() {
                                    r[i * 4..i * 4 + 4].copy_from_slice(&value.to_be_bytes());
                                }
                            },
                        )
                        .unwrap();
                        if serde_json::json!(record.as_slice()) != case["after"]
                            || serde_json::json!(counts) != case["after_counts"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!("Seeker retirement mismatch: {case}").into());
                        }
                        0
                    }
                    Some("seeker_heading") => {
                        let participants: [spectre_core::recovered::AiTargetCandidate; 12] =
                            serde_json::from_value(case["participants"].clone())?;
                        let projectile: [i32; 3] =
                            serde_json::from_value(case["projectile"].clone())?;
                        let mut calls = Vec::new();
                        let result = spectre_core::recovered::seeker_heading(
                            a as i8,
                            projectile,
                            &participants,
                            |target, source| {
                                calls.push([target, source]);
                                let index =
                                    participants.iter().position(|p| p.pose == target).unwrap();
                                case["angles"][index].as_i64().unwrap() as i16
                            },
                        );
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!("Seeker mismatch: {case}; got {result}").into());
                        }
                        0
                    }
                    Some("weapon_target_heading") => {
                        let participants: [spectre_core::recovered::AiTargetCandidate; 12] =
                            serde_json::from_value(case["participants"].clone())?;
                        let projectile: [i32; 3] =
                            serde_json::from_value(case["projectile"].clone())?;
                        let mut calls = Vec::new();
                        let correction = spectre_core::recovered::weapon_target_angle(
                            a as i8,
                            b as u8,
                            projectile,
                            &participants,
                            |target, source| {
                                calls.push([target, source]);
                                let index =
                                    participants.iter().position(|p| p.pose == target).unwrap();
                                case[if source == projectile {
                                    "final_angles"
                                } else {
                                    "angles"
                                }][index]
                                    .as_i64()
                                    .unwrap() as i16
                            },
                        );
                        let result = spectre_core::recovered::weapon_initial_heading(
                            8,
                            projectile[2],
                            || panic!("targeted shot must not request Random"),
                            || correction,
                        );
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!(
                                "Weapon target/heading mismatch: {case}; got {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_fire_uses_cached_angle") => {
                        i32::from(spectre_core::recovered::ai_fire_uses_cached_angle(
                            a as i8,
                            b as i8,
                            case["movement"].as_u64().unwrap() as u8,
                        ))
                    }
                    Some("ai_predict_pose") => {
                        let result = spectre_core::recovered::ai_predict_pose(
                            serde_json::from_value(case["source_pose"].clone())?,
                            serde_json::from_value(case["target_pose"].clone())?,
                            a as i8,
                            b as u8,
                            &serde_json::from_value::<Vec<i32>>(case["table"].clone())?
                                .try_into()
                                .unwrap(),
                        );
                        if serde_json::json!(result) != case["after"] {
                            return Err(
                                format!("AI prediction mismatch: {case}; got {result:?}").into()
                            );
                        }
                        0
                    }
                    Some("ai_initialize_surface") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        let mut timer = case["timer"].as_i64().unwrap() as i32;
                        let supplied = serde_json::from_value(case["supplied"].clone())?;
                        let mut poses = Vec::new();
                        let called = spectre_core::recovered::ai_initialize_surface(
                            &mut state,
                            &mut timer,
                            a as i8,
                            |intent| {
                                poses.push(*intent);
                                supplied
                            },
                        );
                        if serde_json::json!(state) != case["after"]
                            || timer != -1
                            || serde_json::json!(poses) != case["poses"]
                            || serde_json::json!(called) != case["called"]
                        {
                            return Err(format!("AI initializer mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_initialize_type7") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        let mut timer = case["timer"].as_i64().unwrap() as i32;
                        let supplied = serde_json::from_value(case["supplied"].clone())?;
                        let mut poses = Vec::new();
                        let called = spectre_core::recovered::ai_initialize_type7(
                            &mut state,
                            &mut timer,
                            a as i8,
                            b as i16,
                            case["distance"].as_i64().unwrap() as i32,
                            case["range"].as_i64().unwrap() as i32,
                            |intent| {
                                poses.push(*intent);
                                supplied
                            },
                        );
                        if serde_json::json!(state) != case["after"]
                            || timer != -1
                            || serde_json::json!(poses) != case["poses"]
                            || serde_json::json!(called) != case["called"]
                        {
                            return Err(format!("AI initializer mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_surface_tracking") => {
                        let mut states = serde_json::from_value(case["before"].clone())?;
                        let supplied = serde_json::from_value(case["supplied"].clone())?;
                        let selected = case["selected"].as_i64().unwrap() as i8;
                        let mut selections = Vec::new();
                        let mut poses = Vec::new();
                        let mut deadline = case["deadline"].as_i64().unwrap() as i32;
                        let called = spectre_core::recovered::ai_surface_tracking(
                            spectre_core::recovered::AiSurfaceTracking {
                                participant: a as i8,
                                local: b as i8,
                                game_mode: case["game_mode"].as_u64().unwrap() as u8,
                                now: case["now"].as_i64().unwrap() as i32,
                            },
                            &mut states,
                            &mut deadline,
                            |source, excluded, mode| {
                                selections.push((source, excluded, mode));
                                selected
                            },
                            |participant, intent| {
                                poses.push((participant, *intent));
                                supplied
                            },
                        );
                        if serde_json::json!(deadline) != case["deadline_after"]
                            || serde_json::json!(states) != case["after"]
                            || serde_json::json!(selections) != case["selections"]
                            || serde_json::json!(poses) != case["poses"]
                            || serde_json::json!(called) != case["called"]
                        {
                            return Err(format!("Complete AI tracking mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_type7_tracking") => {
                        let mut states = serde_json::from_value(case["before"].clone())?;
                        let supplied = serde_json::from_value(case["supplied"].clone())?;
                        let selected = case["selected"].as_i64().unwrap() as i8;
                        let mut selections = Vec::new();
                        let mut poses = Vec::new();
                        let called = spectre_core::recovered::ai_type7_tracking(
                            a as i8,
                            b as i8,
                            case["game_mode"].as_u64().unwrap() as u8,
                            &mut states,
                            |source, excluded, mode| {
                                selections.push((source, excluded, mode));
                                selected
                            },
                            |participant, intent| {
                                poses.push((participant, *intent));
                                supplied
                            },
                        );
                        if serde_json::json!(states) != case["after"]
                            || serde_json::json!(selections) != case["selections"]
                            || serde_json::json!(poses) != case["poses"]
                            || serde_json::json!(called) != case["called"]
                        {
                            return Err(format!("Complete AI tracking mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_select_target") | Some("ai_acquire_opponent") => {
                        let participants: [spectre_core::recovered::AiTargetCandidate; 12] =
                            serde_json::from_value(case["participants"].clone())?;
                        let angles: [i16; 12] = serde_json::from_value(case["angles"].clone())?;
                        let mut calls = Vec::new();
                        let angle = |candidate, source| {
                            let index = participants
                                .iter()
                                .position(|p| p.pose == candidate)
                                .unwrap();
                            calls.push((candidate, source));
                            angles[index]
                        };
                        let result = if case["routine"] == "ai_acquire_opponent" {
                            spectre_core::recovered::ai_acquire_opponent(
                                a as i8,
                                case["game_mode"].as_u64().unwrap() as u8,
                                &participants,
                                angle,
                            )
                        } else {
                            spectre_core::recovered::ai_select_target(
                                a as i8,
                                b as i8,
                                case["mode"].as_u64().unwrap() as u8,
                                case["game_mode"].as_u64().unwrap() as u8,
                                &participants,
                                angle,
                            )
                        };
                        if serde_json::json!(calls) != case["calls"] {
                            return Err(format!("AI target angle calls mismatch: {case}").into());
                        }
                        result.unwrap_or(-1) as i32
                    }
                    Some("ai_type7_tracking_tail") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        let mut pose = serde_json::from_value(case["pose_before"].clone())?;
                        let supplied = serde_json::from_value(case["supplied"].clone())?;
                        let called = spectre_core::recovered::ai_type7_tracking_tail(
                            &mut state,
                            &mut pose,
                            a as u8,
                            b as u8,
                            || supplied,
                        );
                        if serde_json::json!(state) != case["after"]
                            || serde_json::json!(pose) != case["pose_after"]
                            || serde_json::json!(called) != case["called"]
                        {
                            return Err(format!("AI tracking tail mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_surface_phase") => {
                        let result = spectre_core::recovered::ai_surface_phase(
                            a,
                            case["behavior"].as_u64().unwrap() as u8,
                            case["movement"].as_u64().unwrap() as u8,
                            b,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("AI surface phase mismatch: {case}").into());
                        }
                        0
                    }
                    Some("create_dump_record") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] = bytes.try_into().unwrap();
                        let existing: Vec<[i32; 2]> =
                            serde_json::from_value(case["existing"].clone())?;
                        let words: Vec<i16> = serde_json::from_value(case["words"].clone())?;
                        let mut words = words.into_iter();
                        let input = spectre_core::recovered::DumpCreation {
                            extent: a as i16,
                            kind: b as i8,
                            now: case["now"].as_i64().unwrap() as i32,
                            weights: serde_json::from_value(case["weights"].clone())?,
                            callback: 0x60532,
                        };
                        let result = spectre_core::recovered::create_dump_record(
                            &mut record,
                            &existing,
                            &input,
                            || words.next(),
                        );
                        if result != Some(case["created"].as_bool().unwrap())
                            || serde_json::json!(record.as_slice()) != case["after"]
                            || words.next().is_some()
                        {
                            return Err(format!("Dump creation mismatch: {case}").into());
                        }
                        0
                    }
                    Some("pickup_presentation") => {
                        let colors: [u8; 4] = serde_json::from_value(case["colors"].clone())?;
                        if serde_json::json!(spectre_core::radar::pickup_glyph(
                            a as i16, b as i16, &colors
                        )) != case["result"]
                            || serde_json::json!(spectre_core::radar::pickup_color(
                                a as i16, &colors
                            )) != case["solid"]
                        {
                            return Err(format!("Pickup presentation mismatch: {case}").into());
                        }
                        0
                    }
                    Some("transit_contact") => {
                        let result = spectre_core::recovered::transit_contact(
                            a as u8,
                            b,
                            case["vertical"].as_i64().unwrap() as i32,
                            case["energy"].as_i64().unwrap() as i16,
                        );
                        if serde_json::json!(result.map(|v| (v.transition, v.energy, v.sound)))
                            != case["result"]
                        {
                            return Err(format!("Transit contact mismatch: {case}").into());
                        }
                        0
                    }
                    Some("type6_step") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        let input = serde_json::from_value(case["input"].clone())?;
                        let table: [i32; 360] =
                            serde_json::from_value::<Vec<i32>>(case["table"].clone())?
                                .try_into()
                                .map_err(|_| "Direction table size")?;
                        spectre_core::recovered::type6_step(&mut state, &input, &table)
                            .ok_or("Invalid type6 step")?;
                        if serde_json::json!(state) != case["result"] {
                            return Err(
                                format!("Type6 step mismatch: {case}, actual {state:?}").into()
                            );
                        }
                        0
                    }
                    Some("solid_response_request") => {
                        let result = spectre_core::recovered::solid_response_request(
                            a as u8,
                            b as i16,
                            case["hold"].as_u64().unwrap() as u8,
                            case["local"].as_bool().unwrap(),
                            case["elapsed"].as_i64().unwrap() as i32,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Solid response request mismatch: {case}").into());
                        }
                        0
                    }
                    Some("contact_rebound") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["before"].clone())?;
                        let mut record: [u8; 128] =
                            bytes.try_into().map_err(|_| "Invalid rebound record")?;
                        spectre_core::recovered::contact_rebound(&mut record, a != 0, b);
                        if serde_json::json!(record.to_vec()) != case["after"] {
                            return Err(format!("Contact rebound mismatch: {case}").into());
                        }
                        0
                    }
                    Some("update_vertical") => {
                        let mut state = serde_json::from_value(case["before"].clone())?;
                        spectre_core::recovered::update_vertical(
                            &mut state,
                            a as i16,
                            b as i8,
                            case["setting"].as_i64().unwrap() as i8,
                            serde_json::from_value(case["clocks"].clone())?,
                        )
                        .ok_or("Invalid vertical setting")?;
                        if serde_json::json!(state) != case["result"] {
                            return Err(format!("Vertical state mismatch: {case}").into());
                        }
                        0
                    }
                    Some("energy_clock") => {
                        let result = spectre_core::recovered::energy_clock(
                            a as i16,
                            b as i16,
                            case["paused"].as_bool().unwrap(),
                            case["now"].as_i64().unwrap() as i32,
                            case["previous"].as_i64().unwrap() as i32,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Energy clock mismatch: {case}").into());
                        }
                        0
                    }
                    Some("flight_height") => {
                        let result = spectre_core::recovered::flight_height(
                            a,
                            b,
                            case["hold"].as_i64().unwrap() as i8,
                            case["active"].as_bool().unwrap(),
                            case["speed"].as_i64().unwrap() as i8,
                            case["setting"].as_i64().unwrap() as i8,
                            serde_json::from_value(case["clocks"].clone())?,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Flight height mismatch: {case}").into());
                        }
                        0
                    }
                    Some("dump_animation") => {
                        if serde_json::json!(spectre_core::recovered::dump_animation(a as i16))
                            != case["result"]
                        {
                            return Err(format!("Dump animation mismatch: {case}").into());
                        }
                        0
                    }
                    Some("dump_effect") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        let result = spectre_core::recovered::dump_effect(&input);
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Dump effect mismatch: {case}").into());
                        }
                        0
                    }
                    Some("dump_subtype") => {
                        let weights: [i8; 3] = serde_json::from_value(case["weights"].clone())?;
                        spectre_core::recovered::dump_subtype(a as i16, weights) as i32
                    }
                    Some("relocate_dump") => {
                        let pose: [i32; 3] = serde_json::from_value(case["pose"].clone())?;
                        let words: Vec<i16> = serde_json::from_value(case["words"].clone())?;
                        let mut words = words.into_iter();
                        let mut queries = Vec::new();
                        let accept = case["accept"].as_u64().unwrap() as usize;
                        let result = spectre_core::recovered::relocate_dump(
                            pose,
                            a as i16,
                            || words.next().unwrap(),
                            |point| {
                                queries.push(point);
                                queries.len() == accept
                            },
                        );
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(queries) != case["queries"]
                            || words.next().is_some()
                        {
                            return Err(format!("Dump relocation mismatch: {case}").into());
                        }
                        0
                    }
                    Some("dump_refill") => {
                        let result = spectre_core::recovered::dump_refill(
                            a as i16,
                            b as i16,
                            case["shields"].as_i64().unwrap() as i8,
                            case["ammo_setting"].as_i64().unwrap() as i8,
                            case["full"].as_bool().unwrap(),
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Dump refill mismatch: {case}").into());
                        }
                        0
                    }
                    Some("handling_contact") => {
                        let result = spectre_core::recovered::handling_contact(
                            case["kind"].as_u64().unwrap() as u8,
                            a,
                            b,
                            case["mud"].as_i64().unwrap() as i8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Handling contact mismatch: {case}").into());
                        }
                        0
                    }
                    Some("acid_contact") => {
                        let result = spectre_core::recovered::acid_contact(
                            a,
                            b,
                            case["mode"].as_u64().unwrap() as u8,
                            case["damage"].as_i64().unwrap() as i16,
                            case["attacker"].as_i64().unwrap() as i8,
                            case["victim"].as_i64().unwrap() as i8,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!("Acid contact mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_cooldown") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        if serde_json::json!(spectre_core::recovered::ai_cooldown(input))
                            != case["result"]
                        {
                            return Err(format!("AI cooldown mismatch: {case}").into());
                        }
                        0
                    }
                    Some("ai_intent_action") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        let result = spectre_core::recovered::ai_intent_action(input);
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "AI intent action mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_nearby_predecessor") => {
                        let flags = serde_json::from_value(case["flags"].clone())?;
                        let positions = serde_json::from_value(case["positions"].clone())?;
                        i32::from(
                            spectre_core::recovered::ai_nearby_predecessor(
                                a as i8, &flags, &positions,
                            )
                            .ok_or("Invalid nearby-participant fixture")?,
                        )
                    }
                    Some("participant_angular_rate") => {
                        spectre_core::recovered::participant_angular_rate(
                            case["enemy"].as_u64().unwrap() as u8,
                            case["half"].as_u64().unwrap() as u8,
                            a,
                            b,
                        )
                    }
                    Some("type7_motion") => {
                        let table: Vec<i32> =
                            serde_json::from_value(vectors["movement_marker_table"].clone())?;
                        let table: [i32; 360] =
                            table.try_into().map_err(|_| "Expected 360 directions")?;
                        let pose = serde_json::from_value(case["pose"].clone())?;
                        let commands = serde_json::from_value(case["commands"].clone())?;
                        let result = spectre_core::recovered::move_type7(
                            pose,
                            commands,
                            case["radial"].as_i64().unwrap() as i16,
                            a,
                            b,
                            &table,
                        );
                        if serde_json::json!(result) != case["after"] {
                            return Err(format!(
                                "Type-7 motion mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_aim_cache") => {
                        let mut cache = serde_json::from_value(case["before"].clone())?;
                        let mut modes = Vec::new();
                        let mut poses = Vec::new();
                        let predicted = serde_json::from_value(case["predicted"].clone())?;
                        for _ in 0..2 {
                            spectre_core::recovered::refresh_ai_aim_cache(
                                a,
                                case["owner"].as_u64().unwrap() as u8,
                                case["kind"].as_u64().unwrap() as u8,
                                &mut cache,
                                |mode| {
                                    modes.push(mode);
                                    predicted
                                },
                                |pose| {
                                    poses.push(pose);
                                    b as i16
                                },
                            );
                        }
                        if serde_json::json!(cache) != case["after"]
                            || serde_json::json!(modes) != case["modes"]
                            || serde_json::json!(poses) != case["poses"]
                        {
                            return Err(format!("AI aim cache mismatch: {case}, actual {cache:?}, {modes:?}, {poses:?}").into());
                        }
                        0
                    }
                    Some("ai_turn_only") => spectre_core::recovered::ai_turn_only(a as i16) as i32,
                    Some("ai_begin_attack") => {
                        let mut state = serde_json::from_value(case["state"].clone())?;
                        spectre_core::recovered::ai_begin_attack(&mut state, a as i8);
                        if serde_json::json!(state) != case["after"] {
                            return Err(format!(
                                "AI attack transition mismatch: {case}, actual {state:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_movement_policy") => {
                        use spectre_core::recovered::AiMovementPolicy;
                        match spectre_core::recovered::ai_movement_policy(
                            a as u8,
                            case["nearby"].as_bool().unwrap(),
                            b,
                        ) {
                            AiMovementPolicy::None => 0,
                            AiMovementPolicy::Ordinary => 1,
                            AiMovementPolicy::Combat => 2,
                            AiMovementPolicy::TurnOnly => 3,
                            AiMovementPolicy::Alternate => 4,
                        }
                    }
                    Some("ai_standard_movement") => {
                        let mut state = serde_json::from_value(case["state"].clone())?;
                        let mut calls = Vec::new();
                        let mut aim_calls = 0;
                        let mask = case["mask"].as_u64().unwrap();
                        let result = spectre_core::recovered::ai_standard_movement(
                            serde_json::from_value(case["source"].clone())?,
                            serde_json::from_value(case["target"].clone())?,
                            a,
                            &mut state,
                            || {
                                aim_calls += 1;
                                b as i16
                            },
                            |heading, width, distance| {
                                calls.push([heading, width, distance]);
                                mask & (1 << (calls.len() - 1)) != 0
                            },
                        );
                        if serde_json::json!(state) != case["after"]
                            || serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                            || serde_json::json!(aim_calls) != case["aim_calls"]
                        {
                            return Err(format!("AI standard policy mismatch: {case}, actual {state:?}, {result:?}, {calls:?}, {aim_calls}").into());
                        }
                        0
                    }
                    Some("ai_combat_movement") => {
                        let mut state = serde_json::from_value(case["state"].clone())?;
                        let mut calls = Vec::new();
                        let mut aim_calls = 0;
                        let mask = case["mask"].as_u64().unwrap();
                        let result = spectre_core::recovered::ai_combat_movement(
                            serde_json::from_value(case["source"].clone())?,
                            serde_json::from_value(case["target"].clone())?,
                            a,
                            &mut state,
                            case["scratch"].as_i64().map(|v| v as i32),
                            || {
                                aim_calls += 1;
                                b as i16
                            },
                            |heading, width, distance| {
                                calls.push([heading, width, distance]);
                                mask & (1 << (calls.len() - 1)) != 0
                            },
                        );
                        if serde_json::json!(state) != case["after"]
                            || serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                            || serde_json::json!(aim_calls) != case["aim_calls"]
                        {
                            return Err(format!("AI combat policy mismatch: {case}, actual {state:?}, {result:?}, {calls:?}, {aim_calls}").into());
                        }
                        0
                    }
                    Some("ai_reverse_state") => {
                        let result = spectre_core::recovered::ai_reverse_state(
                            a,
                            b,
                            case["allowed"].as_bool().unwrap(),
                            case["participant"].as_i64().unwrap() as i8,
                            case["mode"].as_u64().unwrap() as u8,
                            case["now"].as_i64().unwrap() as i32,
                        );
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "AI reverse-state mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_clearance_probe") => {
                        let mut calls = Vec::new();
                        let result = spectre_core::recovered::ai_clearance_probe(
                            serde_json::from_value(case["position"].clone())?,
                            a,
                            b,
                            case["side"].as_i64().unwrap() as i16,
                            serde_json::from_value(case["direction"].clone())?,
                            |pose| {
                                calls.push(pose);
                                case["hit_kind"].as_u64().map(|v| v as u8)
                            },
                        );
                        if serde_json::json!(calls) != case["calls"] {
                            return Err(format!(
                                "AI clearance calls mismatch: {case}, actual {calls:?}"
                            )
                            .into());
                        }
                        i32::from(result)
                    }
                    Some("ai_clear_heading") => {
                        let mut calls = Vec::new();
                        let result = spectre_core::recovered::ai_clear_heading(a, |heading| {
                            calls.push(heading);
                            calls.len() as i32 == b
                        });
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!("AI heading search mismatch: {case}, result {result:?}, calls {calls:?}").into());
                        }
                        0
                    }
                    Some("ai_movement_commands") => {
                        let mut calls = Vec::new();
                        let result = spectre_core::recovered::ai_movement_commands(
                            a,
                            b,
                            case["threshold"].as_i64().unwrap() as i32,
                            case["reverse"].as_bool().unwrap(),
                            |heading| {
                                calls.push(heading);
                                case["probe"].as_bool().unwrap()
                            },
                        );
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!(
                                "AI movement mismatch: {case}, result {result:?}, calls {calls:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("historical_angle_from_slope") => {
                        let slopes: Vec<i32> =
                            serde_json::from_value(vectors["positive_slopes"].clone())?;
                        let slopes: [i32; 91] =
                            slopes.try_into().map_err(|_| "Expected 91 slope values")?;
                        spectre_core::recovered::historical_angle_from_slope(a, &slopes)
                            .ok_or("Slope table has no matching threshold")?
                            as i32
                    }
                    Some("ai_aim_angle") => {
                        let target = serde_json::from_value(case["target"].clone())?;
                        let source = serde_json::from_value(case["source"].clone())?;
                        let mut calls = Vec::new();
                        let result = spectre_core::recovered::ai_aim_angle(
                            target,
                            source,
                            |numerator, denominator| {
                                calls.push([numerator, denominator]);
                                a as i16
                            },
                        );
                        if serde_json::json!(result) != case["result"]
                            || serde_json::json!(calls) != case["calls"]
                        {
                            return Err(format!(
                                "Aim angle mismatch: {case}, actual {result:?}, calls {calls:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("ai_planar_distance") => {
                        let first = serde_json::from_value(case["first"].clone())?;
                        let second = serde_json::from_value(case["second"].clone())?;
                        spectre_core::recovered::ai_planar_distance(first, second)
                    }
                    Some("type7_fire_check") => {
                        let input = serde_json::from_value(case["input"].clone())?;
                        let result = spectre_core::recovered::type7_fire_check(input);
                        if serde_json::json!(result) != case["result"] {
                            return Err(format!(
                                "Type-7 firing mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("campaign_upgrade") => {
                        let settings: [u8; 7] = serde_json::from_value(case["settings"].clone())?;
                        let result = spectre_core::recovered::campaign_upgrade(a as i16, settings);
                        if serde_json::json!(result) != case["after"] {
                            return Err(format!(
                                "Campaign upgrade mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("wave_refill") => {
                        let result = spectre_core::recovered::wave_refill(a as i8, b as i8);
                        if serde_json::json!(result) != case["refill"] {
                            return Err(
                                format!("Wave refill mismatch: {case}, actual {result:?}").into()
                            );
                        }
                        0
                    }
                    Some("initial_effective_difficulty") => {
                        spectre_core::recovered::initial_effective_difficulty(a as i16, b as i16)
                            as i32
                    }
                    Some("resource_prefix_count") => {
                        let ids: Option<Vec<i16>> = serde_json::from_value(case["ids"].clone())?;
                        let mut queries = Vec::new();
                        let result = spectre_core::recovered::resource_prefix_count(|id| {
                            queries.push(id);
                            ids.as_ref().map_or(id != a as i16, |ids| ids.contains(&id))
                        });
                        if serde_json::json!(queries) != case["queries"] {
                            return Err(format!("Resource enumeration mismatch: {case}").into());
                        }
                        result as i32
                    }
                    Some("resolve_wave_resources") => {
                        let random: Vec<i16> = serde_json::from_value(case["random"].clone())?;
                        let counts: [i16; 2] = serde_json::from_value(case["counts"].clone())?;
                        let result = spectre_core::recovered::resolve_wave_resources(
                            a as i16,
                            b as i16,
                            counts,
                            case["alternate"].as_u64().unwrap() as u8,
                            &random,
                        );
                        if serde_json::to_value(&result)? != case["result"] {
                            return Err(format!(
                                "Wave resource mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("wave_resource_id") => spectre_core::recovered::wave_resource_id(
                        a as i16,
                        b as u8,
                        case["mode"].as_u64().unwrap() as u8,
                    ) as i32,
                    Some("select_wave") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["resource"].clone())?;
                        let rows = formats::waves(&bytes)?;
                        let result = spectre_core::recovered::select_wave(&rows, a as i16);
                        if serde_json::to_value(&result)? != case["selection"] {
                            return Err(format!(
                                "Wave selection mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("enemy_parameters") => {
                        let bytes: Vec<u8> = serde_json::from_value(case["resource"].clone())?;
                        let rows = formats::beasties(&bytes)?;
                        if serde_json::json!(spectre_core::recovered::dump_weights(&rows))
                            != case["dump_weights"]
                        {
                            return Err(format!("Dump resource weights mismatch: {case}").into());
                        }
                        let result =
                            spectre_core::recovered::enemy_parameters(&rows, a as i16, b != 0);
                        if serde_json::to_value(result.as_slice())? != case["parameters"] {
                            return Err(format!(
                                "Parameter table mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("default_enemy_parameters") => {
                        let result = spectre_core::recovered::default_enemy_parameters(a as i16);
                        if serde_json::to_value(&result)? != case["parameters"] {
                            return Err(format!(
                                "Default parameter mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("enemy_plan") => {
                        let rows: Vec<_> = case["rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|row| formats::Spawn {
                                kind: row[0].as_i64().unwrap() as i16,
                                minimum: row[1].as_i64().unwrap() as i16,
                                maximum: row[2].as_i64().unwrap() as i16,
                                rebirths: row[3].as_i64().unwrap() as i16,
                            })
                            .collect();
                        let random: Vec<i16> = serde_json::from_value(case["random"].clone())?;
                        let result =
                            spectre_core::recovered::enemy_plan(&rows, a as i16, b != 0, &random);
                        if case["guarded"].as_bool() == Some(true) {
                            if result.is_some() {
                                return Err("Expected host guard for stalled enemy plan".into());
                            }
                            continue;
                        }
                        let result = result.ok_or("Invalid enemy plan")?;
                        if serde_json::json!(result.counts.as_slice()) != case["counts"]
                            || serde_json::json!(result.lives.as_slice()) != case["lives"]
                        {
                            return Err(
                                format!("Enemy plan mismatch: {case}, actual {result:?}").into()
                            );
                        }
                        result.total as i32
                    }
                    Some("arena_par") => spectre_core::recovered::arena_par_seconds(
                        &std::array::from_fn(|i| case["counts"][i].as_i64().unwrap() as i16),
                        &std::array::from_fn(|i| case["multipliers"][i].as_i64().unwrap() as i16),
                        &std::array::from_fn(|i| case["bonuses"][i].as_i64().unwrap() as i32),
                        a as i8,
                    ) as i32,
                    Some("completion_ratings") => {
                        let result = spectre_core::recovered::completion_ratings(
                            std::array::from_fn(|i| case["timestamps"][i].as_i64().unwrap() as i32),
                            a as i16,
                            std::array::from_fn(|i| case["counters"][i].as_i64().unwrap() as i32),
                            std::array::from_fn(|i| case["previous"][i].as_i64().unwrap() as i16),
                        )
                        .ok_or("Invalid rating division")?;
                        if serde_json::to_value(&result)? != case["result"] {
                            return Err(format!(
                                "Completion ratings mismatch: {case}, actual {result:?}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("combat_score") => {
                        let award = spectre_core::recovered::combat_score_award(
                            a as u8,
                            b,
                            std::array::from_fn(|i| case["owners"][i].as_i64().unwrap() as i8),
                            std::array::from_fn(|i| case["flags"][i].as_u64().unwrap() as u8),
                            case["killed"].as_u64().unwrap() as u8,
                            case["mode"].as_u64().unwrap() as u8,
                        );
                        if serde_json::json!(award.map(|v| v.0)) != case["award"]
                            || award.is_some_and(|v| v.1) != case["counted"].as_bool().unwrap()
                        {
                            return Err(
                                format!("Combat score mismatch: {case}, actual {award:?}").into()
                            );
                        }
                        0
                    }
                    Some("local_respawn") => {
                        let (health, lives, ammo, game_over, respawn) =
                            spectre_core::recovered::local_respawn(
                                case["active"].as_bool().unwrap(),
                                a as i16,
                                b as i16,
                                case["ammo"].as_i64().unwrap() as i16,
                                case["mode"].as_u64().unwrap() as u8,
                                case["shields"].as_i64().unwrap() as i8,
                                case["ammo_setting"].as_i64().unwrap() as i8,
                            );
                        if serde_json::json!([health, lives, ammo]) != case["after"]
                            || game_over != case["game_over"].as_bool().unwrap()
                            || respawn != case["respawn"].as_bool().unwrap()
                        {
                            return Err(format!("Lifecycle mismatch: {case}, actual {health} {lives} {ammo} {game_over} {respawn}").into());
                        }
                        0
                    }
                    Some("score_resource") => spectre_core::recovered::score_resource_id(
                        a as i16,
                        b as u8,
                        case["mode"].as_u64().unwrap() as u8,
                    ) as i32,
                    Some("next_life") | Some("award_score") => {
                        let rules = if case["payload"].is_null() {
                            None
                        } else {
                            let data: Vec<u8> = serde_json::from_value(case["payload"].clone())?;
                            Some(spectre_core::formats::ScoreRules::parse(&data)?)
                        };
                        if case["routine"] == "next_life" {
                            spectre_core::recovered::next_life_threshold(a, rules.as_ref())
                                .ok_or("Threshold iteration guard")?
                        } else {
                            let (score, lives, awards) = spectre_core::recovered::award_score(
                                a,
                                case["lives"].as_i64().unwrap() as i16,
                                b as i16,
                                case["disabled"].as_bool().unwrap(),
                                rules.as_ref(),
                            )
                            .ok_or("Score iteration guard")?;
                            if lives as i64 != case["after_lives"].as_i64().unwrap()
                                || awards as u64 != case["awards"].as_u64().unwrap()
                            {
                                return Err(format!(
                                    "Score effects mismatch: {case}, {lives}, {awards}"
                                )
                                .into());
                            }
                            score
                        }
                    }
                    Some("flag_mesh") => {
                        let (shape, faces) = spectre_core::model::PackedShape::flag(a != 0);
                        if serde_json::json!(shape.vertices) != case["packed"]
                            || serde_json::json!(faces) != case["faces"]
                            || serde_json::json!(shape.dimensions) != case["dimensions"]
                            || serde_json::json!(shape.candidate_radius) != case["radius"]
                        {
                            return Err(format!(
                                "Flag mesh mismatch: {case}, actual {:?} {:?}",
                                shape.vertices, faces
                            )
                            .into());
                        }
                        0
                    }
                    Some("flag_collision_copy") => {
                        let mut dimensions: Vec<[i32; 2]> =
                            serde_json::from_value(case["dimensions"].clone())?;
                        let mut radii: Vec<i16> = serde_json::from_value(case["radii"].clone())?;
                        spectre_core::recovered::copy_flag_collision(
                            a as i8,
                            b as i8,
                            &mut dimensions,
                            &mut radii,
                        )
                        .ok_or("Invalid flag slots")?;
                        if serde_json::json!(dimensions) != case["after"]
                            || serde_json::json!(radii) != case["after_radii"]
                        {
                            return Err("Flag collision copy mismatch".into());
                        }
                        0
                    }
                    Some("procedural_column") => {
                        let table =
                            std::array::from_fn(|i| case["table"][i].as_i64().unwrap() as i32);
                        let shape = spectre_core::model::PackedShape::octagonal_column(&table);
                        if serde_json::json!(shape.vertices) != case["packed"]
                            || serde_json::json!(spectre_core::model::PackedShape::column_faces())
                                != case["faces"]
                        {
                            return Err("Procedural column mismatch".into());
                        }
                        0
                    }
                    Some("world_placement") => {
                        let pose = spectre_core::recovered::world_placement(
                            a as i16,
                            b as i16,
                            case["facing"].as_i64().unwrap() as i16,
                            case["modulus"].as_i64().unwrap() as i16,
                        )
                        .unwrap();
                        if serde_json::json!(pose) != case["pose"] {
                            return Err(format!("World placement mismatch: {case}").into());
                        }
                        0
                    }
                    Some("resource_vertices") => {
                        let payload: Vec<u8> = case["payload"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap() as u8)
                            .collect();
                        let shape =
                            spectre_core::model::PackedShape::from_resource_vertices(&payload)?;
                        if serde_json::json!(shape.vertices) != case["packed"]
                            || serde_json::json!(shape.dimensions) != case["dimensions"]
                            || serde_json::json!(shape.candidate_radius) != case["radius"]
                        {
                            return Err(format!(
                                "Resource runtime geometry mismatch: {}",
                                case["source"]
                            )
                            .into());
                        }
                        0
                    }
                    Some("model_dispatch") => {
                        use spectre_core::recovered::{ModelLoadRequest, ModelSlotState};
                        let array = |name: &str| {
                            std::array::from_fn(|i| case[name][i].as_i64().unwrap() as i8)
                        };
                        let mut state = ModelSlotState {
                            next: b as i16,
                            current: array("current"),
                            first: array("first"),
                            counts: array("counts"),
                            type19_requests: case["requests"].as_i64().unwrap() as i16,
                            builder_flag: case["flag"].as_i64().unwrap() as i16,
                        };
                        let mut sizes: Vec<[i32; 2]> = case["sizes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|r| std::array::from_fn(|i| r[i].as_i64().unwrap() as i32))
                            .collect();
                        let mut radii: Vec<i16> = case["radii"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|r| r.as_i64().unwrap() as i16)
                            .collect();
                        let mut calls = Vec::new();
                        state.load_type(a as usize, &mut |state, request| {
                            match request {
                                ModelLoadRequest::Selected(kind) => {
                                    calls.push(serde_json::json!(["selected",kind,state.next]));
                                    let loaded = if state.begin(kind)? { case["builder_counts"][kind].as_i64().unwrap() as i16 } else { 0 };
                                    state.finish(kind,loaded)?;
                                    Some(0)
                                }
                                ModelLoadRequest::Resource {kind,id,slot} => {
                                    calls.push(serde_json::json!(["resource",kind,id,slot]));
                                    Some(case["resource_counts"][(id-10099) as usize].as_i64().unwrap() as i16)
                                }
                                ModelLoadRequest::Halve {slot} => {
                                    let index = usize::try_from(slot as i16-1).ok()?;
                                    sizes[index] = spectre_core::recovered::override_collision_dimensions(sizes[index], spectre_core::recovered::CollisionDimensionOverride::Half);
                                    radii[index] /= 2;
                                    Some(0)
                                }
                            }
                        }).ok_or("Invalid model dispatch fixture")?;
                        let actual = serde_json::json!({"next":state.next,"current":state.current.as_slice(),"first":state.first.as_slice(),
                            "counts":state.counts.as_slice(),"requests":state.type19_requests,"flag":state.builder_flag,
                            "sizes":sizes,"radii":radii,"calls":calls});
                        if actual != case["after"] {
                            return Err(format!(
                                "Model dispatch mismatch: {case}, actual {actual}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("model_animation") => {
                        let frames: Vec<i16> = case["frames"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_i64().unwrap() as i16)
                            .collect();
                        let mut calls = Vec::new();
                        let mut index = 0;
                        let total = spectre_core::recovered::load_animation_models(
                            if case["present"].as_bool().unwrap() {
                                Some(&frames)
                            } else {
                                None
                            },
                            a as i16,
                            case["kind"].as_u64().unwrap() as u8,
                            b as i16,
                            &mut |entry, id, slot, kind| {
                                calls.push(serde_json::json!([entry, id, slot, kind]));
                                let count = case["returns"][index].as_i64().unwrap() as i16;
                                index += 1;
                                Some(count)
                            },
                        )
                        .ok_or("Invalid animation loader input")?;
                        if serde_json::json!(calls) != case["calls"] {
                            return Err(format!(
                                "Animation loader calls mismatch: {case}, actual {calls:?}"
                            )
                            .into());
                        }
                        total as i32
                    }
                    Some("model_selected") => {
                        let array = |name: &str| {
                            std::array::from_fn(|i| case[name][i].as_i64().unwrap() as i8)
                        };
                        let mut state = spectre_core::recovered::ModelSlotState {
                            next: b as i16,
                            current: array("current"),
                            first: array("first"),
                            counts: array("counts"),
                            ..Default::default()
                        };
                        let rows: Vec<[i16; 3]> = case["rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|r| std::array::from_fn(|i| r[i].as_i64().unwrap() as i16))
                            .collect();
                        let mut calls = Vec::new();
                        state
                            .load_selected(
                                a as usize,
                                if case["present"].as_bool().unwrap() {
                                    Some(&rows)
                                } else {
                                    None
                                },
                                case["display"].as_bool().unwrap(),
                                case["alternate"].as_bool().unwrap(),
                                &mut |entry, id, slot, kind| {
                                    calls.push(serde_json::json!([entry, id, slot, kind]));
                                    Some(case["loaded"].as_i64().unwrap() as i16)
                                },
                            )
                            .ok_or("Invalid selected loader fixture")?;
                        let actual = serde_json::json!({"next":state.next,"current":state.current.as_slice(),"first":state.first.as_slice(),"counts":state.counts.as_slice(),"calls":calls});
                        if actual != case["after"] {
                            return Err(format!(
                                "Selected model mismatch: {case}, actual {actual}"
                            )
                            .into());
                        }
                        0
                    }
                    Some("model_alias") => {
                        let array = |name: &str| {
                            std::array::from_fn(|i| case[name][i].as_i64().unwrap() as i8)
                        };
                        let mut state = spectre_core::recovered::ModelSlotState {
                            next: b as i16,
                            current: array("current"),
                            first: array("first"),
                            counts: array("counts"),
                            ..Default::default()
                        };
                        let mut calls = Vec::new();
                        state
                            .load_alias_type(a as usize, &mut |state, kind| {
                                calls.push(kind);
                                let loaded = if state.begin(kind)? {
                                    case["builder_counts"][kind].as_i64().unwrap() as i16
                                } else {
                                    0
                                };
                                state.finish(kind, loaded)
                            })
                            .ok_or("Unsupported alias fixture")?;
                        let actual = serde_json::json!({"next":state.next,"current":state.current.as_slice(),
                            "first":state.first.as_slice(),"counts":state.counts.as_slice(),"calls":calls});
                        if actual != case["after"] {
                            return Err(
                                format!("Model alias mismatch: {case}, actual {actual}").into()
                            );
                        }
                        0
                    }
                    Some("model_reservation") => {
                        let kind = a as usize;
                        let mut state = spectre_core::recovered::ModelSlotState {
                            next: b as i16,
                            ..Default::default()
                        };
                        state.current[kind] = case["current"].as_i64().unwrap() as i8;
                        state.first[kind] = case["first"].as_i64().unwrap() as i8;
                        let begin = state.begin(kind).unwrap();
                        state
                            .finish(kind, case["loaded"].as_i64().unwrap() as i16)
                            .unwrap();
                        if serde_json::json!([
                            state.next as i32,
                            state.current[kind] as i32,
                            state.first[kind] as i32,
                            state.counts[kind] as i32
                        ]) != case["after"]
                        {
                            return Err(format!("Model slot state mismatch: {case}").into());
                        }
                        begin as i32
                    }
                    Some("collision_query")
                    | Some("ai_clearance_scene")
                    | Some("basic_collision")
                    | Some("participant_contacts") => {
                        let records: Vec<[u8; 128]> = case["records"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|r| std::array::from_fn(|i| r[i].as_u64().unwrap() as u8))
                            .collect();
                        let states =
                            std::array::from_fn(|i| case["states"][i].as_u64().unwrap() as u8);
                        let slots =
                            std::array::from_fn(|i| case["slots"][i].as_i64().unwrap() as i8);
                        let sizes: Vec<[i32; 2]> = case["sizes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|r| std::array::from_fn(|i| r[i].as_i64().unwrap() as i32))
                            .collect();
                        let grid = std::array::from_fn(|i| {
                            case["grid"][i]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_i64().unwrap() as i16)
                                .collect()
                        });
                        let table = std::array::from_fn(|i| {
                            [
                                ((i * 193) % 65536) as u16 as i16,
                                ((i * 317 + 1000) % 65536) as u16 as i16,
                            ]
                        });
                        let table = if case["candidate_table"].as_bool().unwrap_or(false) {
                            spectre_core::recovered::candidate_collision_table()
                        } else {
                            table
                        };
                        let world = spectre_core::recovered::CollisionQueryWorld {
                            records: &records,
                            participant_states: &states,
                            grid: &grid,
                            model_slots: &slots,
                            dimensions: &sizes,
                            directions: &table,
                        };
                        let pose =
                            std::array::from_fn(|i| case["pose"][i].as_i64().unwrap() as i32);
                        let dimensions =
                            std::array::from_fn(|i| case["dimensions"][i].as_i64().unwrap() as i32);
                        if case["routine"] == "ai_clearance_scene" {
                            i32::from(
                                world
                                    .ai_clearance(a as i16, pose[2], dimensions, 1024)
                                    .ok_or("Invalid AI clearance scene")?,
                            )
                        } else if case["routine"] == "participant_contacts" {
                            let radii: Vec<i16> = case["radii"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_i64().unwrap() as i16)
                                .collect();
                            let cell = std::array::from_fn(|i| {
                                case["cached_cell"][i].as_i64().unwrap() as i16
                            });
                            let result = world
                                .participant_contacts(a as i8, b as i16, cell, &radii, 1024)
                                .ok_or("Invalid participant contact fixture")?;
                            if serde_json::json!(result) != case["contacts"] {
                                return Err(format!(
                                    "Participant contacts mismatch: {case}, actual {result:?}"
                                )
                                .into());
                            }
                            0
                        } else if case["routine"] == "basic_collision" {
                            let objects = std::array::from_fn(|i| {
                                case["participant_objects"][i].as_i64().unwrap() as i16
                            });
                            world
                                .basic_projectile_hit(a as i16, pose, b, &objects)
                                .ok_or("Invalid basic collision fixture")?
                                as i32
                        } else {
                            world
                                .projectile_hit(pose, dimensions, a as i16)
                                .ok_or("Invalid collision query fixture")?
                                as i32
                        }
                    }
                    Some("finalize_hit") => {
                        let ids = std::array::from_fn(|i| case["ids"][i].as_i64().unwrap() as i16);
                        let kinds =
                            std::array::from_fn(|i| case["kinds"][i].as_i64().unwrap() as i8);
                        let states =
                            std::array::from_fn(|i| case["states"][i].as_u64().map(|v| v as u8));
                        spectre_core::recovered::finalize_projectile_hit(ids, kinds, states) as i32
                    }
                    Some("prefer_hit") => {
                        let current =
                            std::array::from_fn(|i| case["current"][i].as_i64().unwrap() as i32);
                        let candidate =
                            std::array::from_fn(|i| case["candidate"][i].as_i64().unwrap() as i32);
                        let direction =
                            std::array::from_fn(|i| case["direction"][i].as_i64().unwrap() as i16);
                        spectre_core::recovered::prefer_projectile_hit(
                            current, candidate, direction,
                        ) as i32
                    }
                    Some("projectile_candidate") => {
                        let ids = std::array::from_fn(|i| case["ids"][i].as_i64().unwrap() as i16);
                        let position =
                            std::array::from_fn(|i| case["position"][i].as_i64().unwrap() as i32);
                        let center =
                            std::array::from_fn(|i| case["center"][i].as_i64().unwrap() as i16);
                        let extents =
                            std::array::from_fn(|i| case["extents"][i].as_i64().unwrap() as i16);
                        spectre_core::recovered::projectile_candidate(
                            ids, position, center, extents, a as i8,
                        ) as i32
                    }
                    Some("runtime_vertex") => {
                        let vertex =
                            std::array::from_fn(|i| case["vertex"][i].as_i64().unwrap() as i32);
                        let actual = spectre_core::recovered::candidate_runtime_vertex(vertex, a);
                        if serde_json::json!(actual) != case["packed"] {
                            return Err(format!("Runtime vertex mismatch: {case}").into());
                        }
                        0
                    }
                    Some("collision_radius") => {
                        let vertices: Vec<[i16; 3]> = case["vertices"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| std::array::from_fn(|i| v[i].as_i64().unwrap() as i16))
                            .collect();
                        let dimensions =
                            spectre_core::recovered::runtime_collision_dimensions(&vertices);
                        if serde_json::json!(dimensions) != case["dimensions"] {
                            return Err("Radius finalizer dimensions differ".into());
                        }
                        spectre_core::recovered::runtime_collision_radius(
                            &vertices,
                            &mut spectre_core::recovered::candidate_fix_div,
                        ) as i32
                    }
                    Some("runtime_sqrt") => spectre_core::recovered::runtime_sqrt(
                        a,
                        &mut spectre_core::recovered::candidate_fix_div,
                    ),
                    Some("collision_dimensions") => {
                        let vertices: Vec<[i16; 3]> = case["vertices"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| std::array::from_fn(|i| v[i].as_i64().unwrap() as i16))
                            .collect();
                        let dimensions =
                            spectre_core::recovered::runtime_collision_dimensions(&vertices);
                        if serde_json::json!(dimensions) != case["dimensions"] {
                            return Err(format!("Collision dimensions mismatch: {case}").into());
                        }
                        0
                    }
                    Some("rect_overlap") => {
                        let first =
                            std::array::from_fn(|i| case["first"][i].as_i64().unwrap() as i32);
                        let second =
                            std::array::from_fn(|i| case["second"][i].as_i64().unwrap() as i32);
                        let dimensions =
                            std::array::from_fn(|i| case["dimensions"][i].as_i64().unwrap() as i32);
                        let table = std::array::from_fn(|i| {
                            [
                                ((i * 193) % 65536) as u16 as i16,
                                ((i * 317 + 1000) % 65536) as u16 as i16,
                            ]
                        });
                        spectre_core::recovered::collision_rectangles_overlap(
                            first, second, dimensions, &table,
                        ) as i32
                    }
                    Some("shot_expired") => {
                        spectre_core::recovered::basic_shot_expired(a, b) as i32
                    }
                    Some("fire_due") => spectre_core::recovered::primary_fire_due(
                        a,
                        b,
                        case["pressed"].as_bool().unwrap(),
                    ) as i32,
                    Some("attempt_timestamp") => {
                        spectre_core::recovered::primary_attempt_timestamp(
                            a,
                            b,
                            case["disabled"].as_u64().unwrap() as u8,
                        )
                    }
                    Some("fire_eligible") => spectre_core::recovered::primary_fire_eligible(
                        a as i16,
                        b as i16,
                        case["health"].as_i64().unwrap() as i16,
                        case["inhibit"].as_u64().unwrap() as u8,
                    ) as i32,
                    Some("weapon_tables") => {
                        let entries: Vec<_> = case["entries"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|row| spectre_core::formats::Weapon {
                                kind: row[0].as_i64().unwrap() as i16,
                                damage: row[1].as_i64().unwrap() as i16,
                                ammo_cost: row[2].as_i64().unwrap() as i16,
                            })
                            .collect();
                        let (damage, ammo) = spectre_core::recovered::weapon_tables(&entries);
                        if serde_json::json!([damage, damage, ammo, ammo]) != case["banks"] {
                            return Err(format!("Weapon table mismatch: {case}").into());
                        }
                        0
                    }
                    Some("type31_model") => {
                        let (current, timer) = spectre_core::recovered::advance_type31_model(
                            a as i8,
                            case["first"].as_i64().unwrap() as i8,
                            case["count"].as_i64().unwrap() as i8,
                            b,
                            case["now"].as_i64().unwrap() as i32,
                            case["interval"].as_i64().unwrap() as i16,
                        );
                        if timer as i64 != case["timer_after"].as_i64().unwrap() {
                            return Err(
                                format!("Type-31 timer mismatch: {case}, actual {timer}").into()
                            );
                        }
                        current as i32
                    }
                    Some("guarded_participant_damage") => {
                        let phase =
                            std::array::from_fn(|i| case["phase"][i].as_i64().unwrap() as i8);
                        spectre_core::recovered::participant_health_after_guard(
                            a as i16,
                            b as i16,
                            case["kind"].as_i64().unwrap() as i8,
                            phase,
                        ) as i32
                    }
                    Some("wrap") => spectre_core::recovered::wrap_angle(a),
                    Some("distance") => spectre_core::recovered::angular_distance(a, b),
                    Some("index") => spectre_core::recovered::direction_table_index(a) as i32,
                    _ => return Err("Unknown recovered routine".into()),
                };
                if actual != expected {
                    return Err(
                        format!("Recovered routine mismatch: {case}, actual {actual}").into(),
                    );
                }
            }
            let modeled = cases
                .iter()
                .filter(|c| {
                    matches!(
                        c["conditional"].as_str(),
                        Some("modeled FixRound" | "modeled FixDiv" | "modeled FixRound and FixDiv")
                    )
                })
                .count();
            let loader_fixtures = cases
                .iter()
                .filter(|c| c["conditional"] == "supplied selected-model loader")
                .count();
            println!("{loader_fixtures} loader comparisons use explicit geometry-loader fixtures.");
            let inner_fixtures = cases
                .iter()
                .filter(|c| c["conditional"] == "supplied inner model builders")
                .count();
            println!(
                "{inner_fixtures} selected/animation-loader comparisons supply inner builder results."
            );
            println!(
                "{} differential vectors match Rust ({} use an explicitly modeled OS trap).",
                cases.len(),
                modeled
            );
        }

        "replay" => {
            let mut viewer = Viewer::default();
            load_optional_colors(&mut viewer)?;
            viewer.load(&bytes)?;
            let world = viewer
                .entries
                .iter()
                .position(|e| e.kind == "wrld" && e.id == 10000)
                .ok_or("World 10000 missing")?;
            if args.get(3).is_some_and(|v| v == "mission") {
                viewer.start_mission(world)?;
            } else {
                viewer.start_game(world)?;
            }
            if args.get(3).is_some_and(|v| v == "encounter") {
                viewer.game.as_mut().unwrap().set_encounter(true);
            }
            for _ in 0..240 {
                viewer.drive(
                    spectre_core::game::STEP,
                    spectre_core::game::Input {
                        throttle: 1.,
                        steer: 0.4,
                        fire: true,
                        brake: false,
                        select_weapon: false,
                        fire_optional: false,
                    },
                );
            }
            let game = viewer.game.as_ref().unwrap();
            println!(
                "{}",
                serde_json::json!({"campaign":viewer.campaign,"vehicle_settings":game.vehicle_settings,"mission":game.mission,"flags":game.flags,"tank":game.tank,"shots":game.shots,"impacts":game.impacts,"fired":game.fired,"combat_counters":game.combat_counters,"par_seconds":game.par_seconds,"dump_weights":game.dump_weights,"optional_weapons":game.optional_weapons,"pickups":game.pickups,"pickups_collected":game.pickups_collected,"flight_energy":game.flight_energy,"vertical":game.vertical,"pending_shields":game.pending_shields,"completion":game.completion,"ammo":game.ammo,"targets":game.targets,"destroyed":game.destroyed,"enemy_clear_award":game.enemy_clear_award,"collision_mode":game.collision_mode,"audio_events":game.audio_events,"encounter":game.encounter,"death_transition":game.death_transition,"radar_glyph_count":game.radar_glyphs.as_ref().map_or(0, Vec::len),"palette":game.palette,"ground_color_index":game.ground_color_index,"enemy_shots":game.enemy_shots})
            );
        }

        "inspect" | "export" => {
            let a = Archive::parse(&bytes)?;
            let mut decoded = Vec::new();
            let mut errors = Vec::new();
            let output = args.get(3).map(Path::new);
            if command == "export" {
                fs::create_dir_all(output.ok_or("Export requires an output directory")?)?;
            }
            for r in &a.resources {
                let value: Result<Option<serde_json::Value>, String> = (|| {
                    Ok(match r.kind.as_str() {
                        "shap" => {
                            Some(serde_json::to_value(formats::Shape::parse(&r.data)?).unwrap())
                        }
                        "wrld" => {
                            Some(serde_json::to_value(formats::World::parse(&r.data)?).unwrap())
                        }
                        "wave" => Some(serde_json::to_value(formats::waves(&r.data)?).unwrap()),
                        "enem" => Some(serde_json::to_value(formats::enemies(&r.data)?).unwrap()),
                        "bval" => Some(serde_json::to_value(formats::beasties(&r.data)?).unwrap()),
                        "wval" => Some(serde_json::to_value(formats::weapons(&r.data)?).unwrap()),
                        "TMPL" => Some(serde_json::to_value(formats::template(&r.data)?).unwrap()),
                        _ => None,
                    })
                })();
                match value {
                    Ok(Some(value)) => decoded.push(
                        serde_json::json!({"kind":r.kind,"id":r.id,"name":r.name,"value":value}),
                    ),
                    Ok(None) => {}
                    Err(error) => {
                        errors.push(serde_json::json!({"kind":r.kind,"id":r.id,"error":error}))
                    }
                }
                if command == "export" {
                    let safe_kind: String = r.kind.bytes().map(|b| format!("{b:02x}")).collect();
                    let dir = output.unwrap().join(safe_kind);
                    fs::create_dir_all(&dir)?;
                    fs::write(dir.join(format!("{}.bin", r.id)), &r.data)?;
                    if r.kind == "shap"
                        && let Ok(shape) = formats::Shape::parse(&r.data)
                    {
                        fs::write(dir.join(format!("{}.obj", r.id)), shape.obj())?;
                    }
                }
            }
            let report = serde_json::json!({"source":input,"resources":a.resources,"decoded":decoded,"errors":errors});
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(out) = output {
                let out = if command == "export" {
                    out.join("inventory.json")
                } else {
                    out.to_path_buf()
                };
                fs::write(&out, json)?;
                println!(
                    "{} resources, {} decoded, {} errors → {}",
                    a.resources.len(),
                    decoded.len(),
                    errors.len(),
                    out.display()
                );
            } else {
                println!("{json}");
            }
            if !errors.is_empty() {
                return Err("Some resource decoders failed; see report".into());
            }
        }
        "render" => {
            let mut v = Viewer::default();
            v.load(&bytes)?;
            let selected = v
                .entries
                .iter()
                .position(|e| e.kind == "shap" && e.id == 10010)
                .unwrap_or(0);
            v.draw(selected, 0.65, 0.3, 1., true);
            let mut ppm = format!(
                "P6\n{} {}\n255\n",
                spectre_core::render::WIDTH,
                spectre_core::render::HEIGHT
            )
            .into_bytes();
            for p in v.frame.rgba.as_chunks::<4>().0 {
                ppm.extend_from_slice(&p[..3]);
            }
            let output = args.get(3).ok_or("Render requires an output PPM path")?;
            fs::write(output, ppm)?;
            println!(
                "Rendered {} {}: {}",
                v.entries[selected].kind, v.entries[selected].id, v.entries[selected].name
            );
        }
        "view" | "drive" => view(
            &bytes,
            args.iter().any(|a| a == "--smoke-test"),
            command == "drive",
        )?,
        _ => return Err("Unknown command. Use inspect, export, render, view, or drive.".into()),
    }
    Ok(())
}

#[cfg(not(feature = "desktop"))]
fn view(_: &[u8], _: bool, _: bool) -> Result<(), Box<dyn std::error::Error>> {
    Err("Build with --features desktop for the native window".into())
}
#[cfg(feature = "desktop")]
fn view(bytes: &[u8], smoke_test: bool, driving: bool) -> Result<(), Box<dyn std::error::Error>> {
    use minifb::{Key, KeyRepeat, Window, WindowOptions};
    use spectre_core::render::{HEIGHT, WIDTH};
    let mut viewer = Viewer::default();
    load_optional_colors(&mut viewer)?;
    viewer.load(bytes)?;
    let mut index = viewer
        .entries
        .iter()
        .position(|e| e.kind == "shap" && e.id == 10010)
        .unwrap_or(0);
    if driving {
        let world = viewer
            .entries
            .iter()
            .position(|e| e.kind == "wrld" && e.id == 10000)
            .or_else(|| viewer.entries.iter().position(|e| e.kind == "wrld"))
            .ok_or("No world layout")?;
        viewer.start_mission(world)?;
    }
    let mut window = Window::new(
        "Spectre Rust — original asset viewer",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;
    window.set_target_fps(60);
    let mut pixels = vec![0u32; WIDTH * HEIGHT];
    let mut yaw = 0.65;
    let mut pitch: f32 = 0.3;
    let mut zoom: f32 = 1.;
    let mut filled = true;
    let mut last = std::time::Instant::now();
    let mut frames = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = std::time::Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;
        if window.is_key_pressed(Key::RightBracket, KeyRepeat::Yes) {
            index = (index + 1) % viewer.entries.len();
        }
        if window.is_key_pressed(Key::LeftBracket, KeyRepeat::Yes) {
            index = (index + viewer.entries.len() - 1) % viewer.entries.len();
        }
        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            filled = !filled;
        }
        if window.is_key_down(Key::Left) {
            yaw -= dt;
        }
        if window.is_key_down(Key::Right) {
            yaw += dt;
        }
        if window.is_key_down(Key::Up) {
            pitch += dt;
        }
        if window.is_key_down(Key::Down) {
            pitch -= dt;
        }
        if window.is_key_down(Key::Equal) {
            zoom += dt;
        }
        if window.is_key_down(Key::Minus) {
            zoom -= dt;
        }
        pitch = pitch.clamp(-1.5, 1.5);
        zoom = zoom.clamp(0.3, 2.);
        if driving {
            if window.is_key_pressed(Key::Tab, KeyRepeat::No)
                || window.is_key_pressed(Key::NumPadAsterisk, KeyRepeat::No)
            {
                viewer.camera = (viewer.camera + 1) % 3;
            }
            if viewer.camera != 0 {
                if window.is_key_pressed(Key::Equal, KeyRepeat::Yes)
                    || window.is_key_pressed(Key::NumPadPlus, KeyRepeat::Yes)
                {
                    viewer.camera_zoom = (viewer.camera_zoom + 1).min(3);
                }
                if window.is_key_pressed(Key::Minus, KeyRepeat::Yes)
                    || window.is_key_pressed(Key::NumPadMinus, KeyRepeat::Yes)
                {
                    viewer.camera_zoom = (viewer.camera_zoom - 1).max(-2);
                }
            }
            if window.is_key_pressed(Key::R, KeyRepeat::No) {
                viewer.game.as_mut().unwrap().reset();
            }
            let axis = |a: Key, b: Key| {
                f32::from(window.is_key_down(a)) - f32::from(window.is_key_down(b))
            };
            let input = if window.is_active() {
                spectre_core::game::Input {
                    throttle: axis(Key::W, Key::S) + axis(Key::Up, Key::Down),
                    steer: axis(Key::D, Key::A) + axis(Key::Right, Key::Left),
                    brake: window.is_key_down(Key::LeftShift),
                    fire: window.is_key_down(Key::Space),
                    select_weapon: window.is_key_down(Key::Q),
                    fire_optional: window.is_key_down(Key::E),
                }
            } else {
                spectre_core::game::Input::default()
            };
            if window.is_active() {
                viewer.drive(dt as f64, input);
            }
            viewer.draw_game();
            let game = viewer.game.as_ref().unwrap();
            window.set_title(&format!(
                "Spectre flag mission | Lives {} | Score {} | Flags {}/{} | Hull {}/6 | Ammo {} | {} | R restart | Tab view | +/- zoom",
                game.mission.lives, game.mission.score, game.flags.collected, game.flags.total, game.encounter.health, game.ammo, game.encounter.status
            ));
        } else {
            viewer.draw(index, yaw, pitch, zoom, filled);
        }
        for (p, c) in pixels.iter_mut().zip(viewer.frame.rgba.as_chunks::<4>().0) {
            *p = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
        }
        let e = &viewer.entries[index];
        if !driving {
            window.set_title(&format!(
                "Spectre Rust · {} {} {} · [ ] select · arrows rotate · +/- zoom · Space fill",
                e.kind, e.id, e.name
            ));
        }
        window.update_with_buffer(&pixels, WIDTH, HEIGHT)?;
        frames += 1;
        if smoke_test && frames == 3 {
            println!("Native window smoke test: presented {frames} frames.");
            break;
        }
    }
    Ok(())
}
