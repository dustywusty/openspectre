#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod audio;
mod smoke;
mod ui;

use macroquad::prelude::*;
use spectre_client::{
    assets,
    session::{Screen, Session},
    settings::{Profile, profile_path},
};
use spectre_core::{
    game::Input,
    render::{HEIGHT, WIDTH},
};
use ui::{Action, Ui};

fn window_conf() -> Conf {
    Conf {
        window_title: "OpenSpectre · Signal Frontier".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        window_resizable: true,
        platform: miniquad::conf::Platform {
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[derive(Default)]
struct Focus {
    lost: bool,
}
impl miniquad::EventHandler for Focus {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn window_minimized_event(&mut self) {
        self.lost = true;
    }
}

fn input() -> Input {
    let axis = |positive: &[KeyCode], negative: &[KeyCode]| {
        f32::from(positive.iter().any(|key| is_key_down(*key)))
            - f32::from(negative.iter().any(|key| is_key_down(*key)))
    };
    Input {
        throttle: axis(&[KeyCode::W, KeyCode::Up], &[KeyCode::S, KeyCode::Down]),
        steer: axis(&[KeyCode::D, KeyCode::Right], &[KeyCode::A, KeyCode::Left]),
        brake: is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift),
        fire: is_key_down(KeyCode::Space),
        select_weapon: is_key_pressed(KeyCode::Q),
        fire_optional: is_key_down(KeyCode::E),
    }
}

fn act(action: Action, session: &mut Session) -> Result<bool, String> {
    match action {
        Action::None => {}
        Action::Arenas => session.screen = Screen::Arenas,
        Action::Select(index) => session.select(index)?,
        Action::Launch => session.launch()?,
        Action::Next => {
            session.select((session.selected + 1) % assets::ARENA_COUNT)?;
            session.launch()?;
        }
        Action::Resume => {
            session.screen = if session.screen == Screen::Playing {
                Screen::Paused
            } else {
                Screen::Playing
            };
        }
        Action::Settings => session.open(Screen::Settings),
        Action::Controls => session.open(Screen::Controls),
        Action::Back => session.back(),
        Action::Leave => session.screen = Screen::ConfirmLeave,
        Action::Title => {
            session.select(session.selected)?;
            session.screen = Screen::Title;
        }
        Action::Quit => return Ok(true),
        Action::Volume(delta) => {
            session.profile.settings.volume = ((session.profile.settings.volume + delta) * 100.)
                .round()
                .clamp(0., 100.)
                / 100.;
            session.profile_dirty = true;
        }
        Action::Mute => {
            session.profile.settings.muted = !session.profile.settings.muted;
            session.profile_dirty = true;
        }
        Action::Fullscreen => {
            session.profile.settings.fullscreen = !session.profile.settings.fullscreen;
            set_fullscreen(session.profile.settings.fullscreen);
            session.profile_dirty = true;
        }
        Action::Camera => {
            session.profile.settings.camera = (session.profile.settings.camera + 1) % 3;
            session.viewer.camera = session.profile.settings.camera;
            session.profile_dirty = true;
        }
        Action::Pixels => {
            session.profile.settings.smooth_pixels = !session.profile.settings.smooth_pixels;
            session.profile_dirty = true;
        }
        Action::Hints => {
            session.profile.settings.control_hints = !session.profile.settings.control_hints;
            session.profile_dirty = true;
        }
    }
    Ok(false)
}

#[macroquad::main(window_conf)]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("OpenSpectre: {error}");
        if std::env::args().any(|arg| arg == "--smoke-test") {
            std::process::exit(1);
        }
        loop {
            clear_background(color_u8!(8, 17, 26, 255));
            draw_text("OpenSpectre could not start", 40., 90., 36., WHITE);
            draw_text(&error, 40., 140., 20., ORANGE);
            draw_text("Press Escape to close.", 40., 200., 24., WHITE);
            if is_key_pressed(KeyCode::Escape) || is_quit_requested() {
                break;
            }
            next_frame().await;
        }
    }
}

async fn run() -> Result<(), String> {
    prevent_quit();
    let mut smoke = smoke::Smoke::from_args()?;
    let path = smoke
        .as_ref()
        .map(|s| s.directory.join("profile.json"))
        .unwrap_or_else(profile_path);
    let (profile, warning) = match Profile::load(&path) {
        Ok(profile) => (profile, None),
        Err(error) => (Profile::default(), Some(error)),
    };
    let pack = assets::pack()?;
    let mut session = Session::new(profile)?;
    session.notice = warning;
    let mut audio = audio::Audio::load().await?;
    let texture = Texture2D::from_rgba8(WIDTH as u16, HEIGHT as u16, &session.viewer.frame.rgba);
    let font = load_ttf_font_from_bytes(include_bytes!("../assets/SpaceGrotesk.ttf"))
        .map_err(|error| format!("Cannot load the interface font: {error}"))?;
    let mut ui = Ui::new(font);
    let subscriber = macroquad::input::utils::register_input_subscriber();
    let mut focus = Focus::default();
    let mut previous_screen = session.screen;
    let mut frame: u64 = 0;
    let mut suppress_fire = false;
    let mut audio_settings = (
        session.profile.settings.volume,
        session.profile.settings.muted,
    );
    if session.profile.settings.fullscreen {
        set_fullscreen(true);
    }
    loop {
        let frame_start = std::time::Instant::now();
        macroquad::input::utils::repeat_all_miniquad_input(&mut focus, subscriber);
        if focus.lost {
            if smoke.is_none() {
                session.pause();
                audio.stop();
            }
            focus.lost = false;
        }
        if is_quit_requested() {
            break;
        }
        // Smoke runs use scripted actions, independent of activity on the shared desktop.
        if smoke.is_none() && is_key_pressed(KeyCode::Escape) {
            session.back();
            audio.stop();
        }
        if smoke.is_none() && (is_key_pressed(KeyCode::F) || is_key_pressed(KeyCode::F11)) {
            act(Action::Fullscreen, &mut session)?;
        }
        if smoke.is_none() && is_key_pressed(KeyCode::M) {
            act(Action::Mute, &mut session)?;
        }
        if smoke.is_none() && session.screen == Screen::Playing {
            if is_key_pressed(KeyCode::Tab) {
                act(Action::Camera, &mut session)?;
            }
            if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
                session.viewer.camera_zoom = (session.viewer.camera_zoom + 1).min(3);
            }
            if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
                session.viewer.camera_zoom = (session.viewer.camera_zoom - 1).max(-2);
            }
        }
        if let Some(smoke) = &mut smoke {
            smoke.before_frame(frame, &mut session)?;
        }
        if previous_screen != session.screen {
            suppress_fire = true;
        }
        if !is_key_down(KeyCode::Space) {
            suppress_fire = false;
        }
        let mut controls = input();
        if suppress_fire {
            controls.fire = false;
        }
        if let Some(smoke) = &smoke {
            controls = smoke.input(frame, controls);
        }
        let dt = if smoke.is_some() {
            1. / 60.
        } else {
            f64::from(get_frame_time()).min(0.1)
        };
        let events = session.update(dt, controls);
        let current_audio_settings = (
            session.profile.settings.volume,
            session.profile.settings.muted,
        );
        if current_audio_settings != audio_settings {
            audio.apply(&session.profile.settings);
            audio_settings = current_audio_settings;
        }
        audio.play(&events, &session.profile.settings, get_time());
        if session.screen == Screen::Title {
            if let Some(game) = &mut session.viewer.game {
                game.tank.heading = 0.65 + (get_time() as f32 * 0.12).sin() * 0.45;
            }
            session.viewer.camera = 1;
        }
        // The simulation keeps its fixed 60 Hz clock. Static menus reuse the last game frame.
        if matches!(
            session.screen,
            Screen::Playing | Screen::Title | Screen::Results
        ) || previous_screen != session.screen
        {
            session.viewer.draw_game();
            texture.update_from_bytes(WIDTH as u32, HEIGHT as u32, &session.viewer.frame.rgba);
        }
        texture.set_filter(if session.profile.settings.smooth_pixels {
            FilterMode::Linear
        } else {
            FilterMode::Nearest
        });
        ui.begin(session.screen);
        ui.draw(&session, &pack, &texture);
        ui.end();
        previous_screen = session.screen;
        if smoke.is_none() && ui.action != Action::None {
            match act(ui.action, &mut session) {
                Ok(true) => break,
                Ok(false) => {}
                Err(error) => session.notice = Some(error),
            }
            if previous_screen != session.screen {
                audio.stop();
            }
            audio.play(&[10020], &session.profile.settings, get_time());
        }
        if session.profile_dirty {
            match session.profile.save(&path) {
                Ok(()) => session.profile_dirty = false,
                Err(error) => {
                    session.notice = Some(error);
                    session.profile_dirty = false;
                }
            }
        }
        if let Some(smoke) = &mut smoke
            && smoke.after_frame(frame, &session, audio.played)?
        {
            break;
        }
        frame += 1;
        next_frame().await;
        // Some desktops ignore swap_interval. Cap the loop to avoid a busy idle window.
        let remaining =
            std::time::Duration::from_secs_f64(1. / 60.).saturating_sub(frame_start.elapsed());
        if !remaining.is_zero() {
            std::thread::sleep(remaining);
        }
    }
    audio.stop();
    session.profile.save(&path)?;
    Ok(())
}
