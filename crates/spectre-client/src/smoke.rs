//! Opt-in desktop verification. Uses an isolated profile and writes screenshots plus a report.
use crate::{act, ui::Action};
use macroquad::prelude::*;
use spectre_client::session::{Screen, Session};
use spectre_core::game::Input;
use std::path::PathBuf;

fn resize(width: f32, height: f32) {
    // Miniquad's macOS backend takes window points here; Macroquad otherwise multiplies by DPI twice.
    if cfg!(target_os = "macos") {
        miniquad::window::set_window_size(width as u32, height as u32);
    } else {
        request_new_screen_size(width, height);
    }
}

pub struct Smoke {
    pub directory: PathBuf,
    paused_ticks: u64,
    screens: Vec<String>,
}

impl Smoke {
    pub fn from_args() -> Result<Option<Self>, String> {
        let mut args = std::env::args().skip(1);
        match args.next().as_deref() {
            None => Ok(None),
            Some("--smoke-test") => {
                let directory = PathBuf::from(
                    args.next()
                        .ok_or("--smoke-test requires an output directory")?,
                );
                if args.next().is_some() {
                    return Err("Unexpected extra argument".into());
                }
                std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                // Do not reuse a previous smoke run's settings or scores.
                spectre_client::settings::Profile::default()
                    .save(&directory.join("profile.json"))?;
                Ok(Some(Self {
                    directory,
                    paused_ticks: 0,
                    screens: Vec::new(),
                }))
            }
            Some(_) => Err("Usage: spectre-client [--smoke-test OUTPUT_DIRECTORY]".into()),
        }
    }

    pub fn before_frame(&mut self, frame: u64, session: &mut Session) -> Result<(), String> {
        let action = match frame {
            20 => Action::Arenas,
            40 => Action::Launch,
            90 => Action::Camera,
            110 => {
                session.pause();
                self.paused_ticks = session.viewer.game.as_ref().unwrap().tank.ticks;
                Action::None
            }
            130 => Action::Settings,
            145 => Action::Volume(0.1),
            150 => Action::Back,
            160 => {
                if session.screen != Screen::Paused
                    || session.viewer.game.as_ref().unwrap().tank.ticks != self.paused_ticks
                {
                    return Err(format!(
                        "Pause/settings check failed: screen {:?}, ticks {} (expected Paused, ticks {})",
                        session.screen,
                        session.viewer.game.as_ref().unwrap().tank.ticks,
                        self.paused_ticks,
                    ));
                }
                Action::Resume
            }
            270 => Action::Launch,
            280 => {
                let game = session.viewer.game.as_ref().unwrap();
                if game.mission.score != 0 || game.flags.collected != 0 || game.mission.lives != 3 {
                    return Err("Replay did not reset the match".into());
                }
                Action::None
            }
            // A loss fixture verifies presentation; the core tests verify death and life consumption.
            310 => {
                session.viewer.game.as_mut().unwrap().encounter.status = "lost";
                Action::None
            }
            330 => Action::Arenas,
            340 => Action::Select(spectre_client::assets::ARENA_COUNT - 1),
            350 => Action::Launch,
            370 => {
                session.pause();
                resize(960., 600.);
                Action::None
            }
            400 => {
                resize(1280., 800.);
                Action::Title
            }
            410 | 460 => Action::Fullscreen,
            _ => Action::None,
        };
        act(action, session)?;
        // Move beacons onto the player to exercise real contact, scoring, and win detection.
        if (220..240).contains(&frame) && session.screen == Screen::Playing {
            let game = session.viewer.game.as_mut().unwrap();
            game.infinite_health = true;
            for item in &mut game.items {
                if item.kind == 12 {
                    item.x = game.tank.x;
                    item.z = game.tank.z;
                }
            }
        }
        Ok(())
    }

    pub fn input(&self, frame: u64, _input: Input) -> Input {
        Input {
            throttle: if (42..85).contains(&frame) { 1. } else { 0. },
            fire: (45..85).contains(&frame),
            ..Input::default()
        }
    }

    pub fn after_frame(
        &mut self,
        frame: u64,
        session: &Session,
        sounds_played: u64,
    ) -> Result<bool, String> {
        let capture = match frame {
            10 => Some("title"),
            30 => Some("arenas"),
            85 => Some("playing"),
            100 => Some("map"),
            120 => Some("paused"),
            140 => Some("settings"),
            250 => {
                if session.screen != Screen::Results
                    || session.viewer.game.as_ref().unwrap().encounter.status != "won"
                {
                    return Err("Beacon contacts did not reach the results screen".into());
                }
                if session.profile.records.get(&0).map(|r| r.wins) != Some(1) {
                    return Err("The completed arena was not recorded exactly once".into());
                }
                Some("won")
            }
            320 => {
                if session.screen != Screen::Results {
                    return Err("Loss did not reach results".into());
                }
                Some("lost")
            }
            390 => {
                if screen_width() > 1000. || screen_height() > 650. {
                    return Err("The small-window resize did not take effect".into());
                }
                Some("small-window")
            }
            440 => Some("fullscreen"),
            _ => None,
        };
        if let Some(name) = capture {
            let path = self.directory.join(format!("{name}.png"));
            get_screen_data()
                .export_png(path.to_str().ok_or("Screenshot path is not valid UTF-8")?);
            self.screens.push(name.into());
        }
        if frame == 490 {
            if sounds_played == 0 {
                return Err("No native sound events played".into());
            }
            let saved =
                spectre_client::settings::Profile::load(&self.directory.join("profile.json"))?;
            if saved.records.get(&0).map(|r| r.wins) != Some(1)
                || (saved.settings.volume - 0.45).abs() > 0.051
            {
                return Err("Settings and results did not persist".into());
            }
            let report = serde_json::json!({
                "passed": true, "screenshots": self.screens, "sounds_played": sounds_played,
                "pause_frozen": true, "beacon_contact_win": true, "replay_reset": true,
                "loss_fixture": true, "settings_persisted": true, "fullscreen_requested": true,
                "last_arena": saved.last_arena,
            });
            std::fs::write(
                self.directory.join("report.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            println!("Desktop smoke test passed: {}", self.directory.display());
            return Ok(true);
        }
        Ok(false)
    }
}
