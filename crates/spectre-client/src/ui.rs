use macroquad::prelude::*;
use spectre_client::{
    assets::Pack,
    session::{Screen, Session},
};

const BG: Color = color_u8!(8, 17, 26, 255);
const PANEL: Color = color_u8!(15, 29, 40, 245);
const INK: Color = color_u8!(232, 243, 237, 255);
const MUTED: Color = color_u8!(138, 166, 174, 255);
const ACCENT: Color = color_u8!(104, 236, 204, 255);
const LINE: Color = color_u8!(42, 66, 76, 255);
const AMBER: Color = color_u8!(255, 194, 114, 255);
const DANGER: Color = color_u8!(255, 120, 120, 255);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    Arenas,
    Select(usize),
    Launch,
    Next,
    Resume,
    Settings,
    Controls,
    Back,
    Leave,
    Title,
    Quit,
    Volume(f32),
    Mute,
    Fullscreen,
    Camera,
    Pixels,
    Hints,
}

pub struct Ui {
    font: Font,
    scale: f32,
    offset: Vec2,
    mouse: Vec2,
    previous_mouse: Vec2,
    mouse_moved: bool,
    focus: usize,
    count: usize,
    previous_count: usize,
    screen: Screen,
    activate: bool,
    pub action: Action,
}

impl Ui {
    pub fn new(font: Font) -> Self {
        Self {
            font,
            scale: 1.,
            offset: Vec2::ZERO,
            mouse: Vec2::ZERO,
            previous_mouse: vec2(-1., -1.),
            mouse_moved: false,
            focus: 0,
            count: 0,
            previous_count: 0,
            screen: Screen::Title,
            activate: false,
            action: Action::None,
        }
    }

    pub fn begin(&mut self, screen: Screen) {
        self.scale = (screen_width() / 1280.)
            .min(screen_height() / 800.)
            .max(0.1);
        self.offset = vec2(
            (screen_width() - 1280. * self.scale) / 2.,
            (screen_height() - 800. * self.scale) / 2.,
        );
        let (x, y) = mouse_position();
        self.mouse = (vec2(x, y) - self.offset) / self.scale;
        self.mouse_moved = (self.mouse - self.previous_mouse).length() > 1.;
        self.previous_mouse = self.mouse;
        if self.screen != screen {
            self.focus = 0;
            self.previous_count = 0;
            self.screen = screen;
        }
        if self.previous_count > 0 {
            let grid = screen == Screen::Arenas && self.focus < 12;
            let mut delta = 0;
            if is_key_pressed(KeyCode::Down) {
                delta = if grid { 4 } else { 1 };
            }
            if is_key_pressed(KeyCode::Up) {
                delta = if grid { -4 } else { -1 };
            }
            if is_key_pressed(KeyCode::Right) {
                delta = 1;
            }
            if is_key_pressed(KeyCode::Left) {
                delta = -1;
            }
            if is_key_pressed(KeyCode::Tab) {
                delta = if is_key_down(KeyCode::LeftShift) {
                    -1
                } else {
                    1
                };
            }
            self.focus =
                (self.focus as i32 + delta).rem_euclid(self.previous_count as i32) as usize;
        }
        self.activate = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        self.count = 0;
        self.action = Action::None;
        clear_background(BG);
    }

    pub fn end(&mut self) {
        self.previous_count = self.count;
    }

    fn rect(&self, r: Rect, color: Color) {
        draw_rectangle(
            self.offset.x + r.x * self.scale,
            self.offset.y + r.y * self.scale,
            r.w * self.scale,
            r.h * self.scale,
            color,
        );
    }
    fn border(&self, r: Rect, color: Color) {
        draw_rectangle_lines(
            self.offset.x + r.x * self.scale,
            self.offset.y + r.y * self.scale,
            r.w * self.scale,
            r.h * self.scale,
            self.scale,
            color,
        );
    }
    fn line(&self, a: Vec2, b: Vec2, color: Color) {
        let a = self.offset + a * self.scale;
        let b = self.offset + b * self.scale;
        draw_line(a.x, a.y, b.x, b.y, self.scale, color);
    }
    fn text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        draw_text_ex(
            text,
            self.offset.x + x * self.scale,
            self.offset.y + y * self.scale,
            TextParams {
                font: Some(&self.font),
                font_size: (size * self.scale).round().max(1.) as u16,
                color,
                ..Default::default()
            },
        );
    }
    fn texture(&self, texture: &Texture2D, r: Rect, color: Color) {
        draw_texture_ex(
            texture,
            self.offset.x + r.x * self.scale,
            self.offset.y + r.y * self.scale,
            color,
            DrawTextureParams {
                dest_size: Some(vec2(r.w, r.h) * self.scale),
                ..Default::default()
            },
        );
    }
    fn button(&mut self, r: Rect, label: &str, action: Action, primary: bool) {
        let id = self.count;
        self.count += 1;
        let hover = r.contains(self.mouse);
        if hover && self.mouse_moved {
            self.focus = id;
        }
        let focused = self.focus == id;
        let fill = if primary {
            ACCENT
        } else if focused {
            color_u8!(27, 51, 60, 255)
        } else {
            PANEL
        };
        self.rect(r, fill);
        self.border(
            r,
            if focused {
                INK
            } else if primary {
                ACCENT
            } else {
                LINE
            },
        );
        if focused {
            self.rect(
                Rect::new(r.x, r.y, 3., r.h),
                if primary { BG } else { ACCENT },
            );
        }
        self.text(
            label,
            r.x + 20.,
            r.y + r.h / 2. + 7.,
            23.,
            if primary { BG } else { INK },
        );
        if (hover && is_mouse_button_pressed(MouseButton::Left)) || (focused && self.activate) {
            self.action = action;
        }
    }
    fn footer(&self, right: &str) {
        self.line(vec2(64., 742.), vec2(1216., 742.), LINE);
        self.text("OPENSPECTRE  /  SIGNAL FRONTIER", 64., 772., 16., MUTED);
        self.text(right, 760., 772., 16., MUTED);
    }
    fn header(&self, eyebrow: &str, heading: &str, subtitle: &str) {
        self.text(eyebrow, 64., 76., 17., ACCENT);
        self.text(heading, 62., 132., 48., INK);
        self.text(subtitle, 64., 169., 21., MUTED);
    }
    fn backdrop(&self, texture: &Texture2D) {
        // Preserve the renderer's 4:3 projection; the title art extends behind the menu.
        self.texture(texture, Rect::new(360., 40., 920., 690.), WHITE);
        for i in 0..64 {
            let alpha = (1. - i as f32 / 64.).powf(0.35);
            self.rect(
                Rect::new(i as f32 * 20., 0., 20., 800.),
                Color::new(BG.r, BG.g, BG.b, alpha),
            );
        }
        self.rect(
            Rect::new(0., 0., 1280., 800.),
            Color::new(BG.r, BG.g, BG.b, 0.22),
        );
    }
    fn modal(&self, title: &str, subtitle: &str) {
        self.rect(Rect::new(0., 0., 1280., 800.), color_u8!(3, 9, 16, 210));
        self.rect(Rect::new(362., 96., 556., 606.), PANEL);
        self.border(Rect::new(362., 96., 556., 606.), LINE);
        self.rect(Rect::new(362., 96., 556., 3.), ACCENT);
        self.text(title, 406., 164., 40., INK);
        self.text(subtitle, 406., 201., 20., MUTED);
    }

    pub fn draw(&mut self, session: &Session, pack: &Pack, texture: &Texture2D) {
        let arena = &pack.zones[session.selected / 4].arenas[session.selected % 4];
        let settings = &session.profile.settings;
        match session.screen {
            Screen::Title => {
                self.backdrop(texture);
                self.text("A SIGNAL IN THE STATIC", 64., 116., 18., ACCENT);
                self.text("OPEN", 60., 216., 100., INK);
                self.text("SPECTRE", 60., 306., 100., INK);
                self.text("S I G N A L   F R O N T I E R", 66., 352., 24., ACCENT);
                self.text(
                    "Find the beacons. Survive the crossfire.",
                    66.,
                    398.,
                    23.,
                    MUTED,
                );
                self.button(
                    Rect::new(64., 448., 340., 62.),
                    "Single player                 >",
                    Action::Arenas,
                    true,
                );
                self.button(
                    Rect::new(64., 524., 164., 52.),
                    "Settings",
                    Action::Settings,
                    false,
                );
                self.button(
                    Rect::new(240., 524., 164., 52.),
                    "How to play",
                    Action::Controls,
                    false,
                );
                self.button(Rect::new(64., 590., 340., 52.), "Quit", Action::Quit, false);
                self.text("12 ARENAS  /  3 REGIONS", 836., 654., 18., ACCENT);
                self.text(&arena.name, 836., 687., 26., INK);
                self.footer("ARROWS select    ENTER confirm    F fullscreen");
            }
            Screen::Arenas => {
                self.header(
                    "SINGLE PLAYER",
                    "Choose your arena",
                    "Collect every beacon to win. You have three lives. Keep moving.",
                );
                for (zone_index, zone) in pack.zones.iter().enumerate() {
                    let y = 212. + zone_index as f32 * 142.;
                    self.text(&zone.name.to_uppercase(), 64., y, 16., ACCENT);
                    for (arena_index, arena) in zone.arenas.iter().enumerate() {
                        let index = zone_index * 4 + arena_index;
                        let x = 64. + arena_index as f32 * 292.;
                        let r = Rect::new(x, y + 14., 276., 102.);
                        self.button(r, "", Action::Select(index), false);
                        if session.selected == index {
                            self.border(Rect::new(r.x + 3., r.y + 3., r.w - 6., r.h - 6.), ACCENT);
                        }
                        self.text(&format!("{:02}", index + 1), x + 16., y + 42., 15., MUTED);
                        self.text(&arena.name, x + 16., y + 72., 23., INK);
                        let record = session.profile.records.get(&index);
                        let detail = record
                            .filter(|r| r.wins > 0)
                            .map(|r| format!("CLEARED  /  BEST {}", r.score))
                            .unwrap_or_else(|| "BEACON RUN".into());
                        self.text(
                            &detail,
                            x + 16.,
                            y + 97.,
                            13.,
                            if record.is_some() { ACCENT } else { MUTED },
                        );
                        for object in &arena.objects {
                            self.rect(
                                Rect::new(
                                    x + 218. + object[1] as f32 / 1024. * 40.,
                                    y + 28. + object[2] as f32 / 1024. * 40.,
                                    3.,
                                    3.,
                                ),
                                MUTED,
                            );
                        }
                    }
                }
                self.button(
                    Rect::new(64., 666., 180., 52.),
                    "<  Back",
                    Action::Back,
                    false,
                );
                self.text(&format!("Selected: {}", arena.name), 278., 699., 23., INK);
                self.button(
                    Rect::new(884., 666., 332., 52.),
                    "Deploy                 SPACE",
                    Action::Launch,
                    true,
                );
                if is_key_pressed(KeyCode::Space) {
                    self.action = Action::Launch;
                }
                self.footer("ARROWS navigate    ENTER select    ESC back");
            }
            Screen::Playing | Screen::Paused | Screen::ConfirmLeave | Screen::Results => {
                self.game(session, pack, texture);
                match session.screen {
                    Screen::Paused => {
                        self.modal("On standby", "Your arena is paused.");
                        self.button(
                            Rect::new(406., 248., 468., 58.),
                            "Resume                          ESC",
                            Action::Resume,
                            true,
                        );
                        self.button(
                            Rect::new(406., 324., 468., 54.),
                            "Settings",
                            Action::Settings,
                            false,
                        );
                        self.button(
                            Rect::new(406., 394., 468., 54.),
                            "How to play",
                            Action::Controls,
                            false,
                        );
                        self.button(
                            Rect::new(406., 464., 468., 54.),
                            "Leave arena",
                            Action::Leave,
                            false,
                        );
                        self.text(
                            "Take your time. The frontier can wait.",
                            406.,
                            642.,
                            20.,
                            MUTED,
                        );
                    }
                    Screen::ConfirmLeave => {
                        self.modal("Leave this arena?", "Your current run will end.");
                        self.button(
                            Rect::new(406., 292., 468., 58.),
                            "Keep playing",
                            Action::Resume,
                            true,
                        );
                        self.button(
                            Rect::new(406., 370., 468., 54.),
                            "Return to title",
                            Action::Title,
                            false,
                        );
                    }
                    Screen::Results => self.results(session),
                    _ => {}
                }
            }
            Screen::Settings => {
                self.header(
                    "MAKE IT YOURS",
                    "Settings",
                    "Changes save automatically. Escape returns to your previous screen.",
                );
                let volume = format!(
                    "Effects volume                         {}%",
                    (settings.volume * 100.).round() as u32
                );
                self.text(&volume, 84., 236., 25., INK);
                self.button(
                    Rect::new(840., 204., 170., 52.),
                    "-  Quieter",
                    Action::Volume(-0.1),
                    false,
                );
                self.button(
                    Rect::new(1026., 204., 170., 52.),
                    "+  Louder",
                    Action::Volume(0.1),
                    false,
                );
                self.rect(Rect::new(84., 257., 700., 4.), LINE);
                self.rect(Rect::new(84., 257., 700. * settings.volume, 4.), ACCENT);
                for (i, (label, value, action)) in [
                    (
                        "Sound",
                        if settings.muted { "Muted" } else { "On" },
                        Action::Mute,
                    ),
                    (
                        "Display",
                        if settings.fullscreen {
                            "Fullscreen"
                        } else {
                            "Windowed"
                        },
                        Action::Fullscreen,
                    ),
                    (
                        "Starting camera",
                        camera_name(settings.camera),
                        Action::Camera,
                    ),
                    (
                        "Picture",
                        if settings.smooth_pixels {
                            "Smooth"
                        } else {
                            "Crisp pixels"
                        },
                        Action::Pixels,
                    ),
                    (
                        "Control hints",
                        if settings.control_hints { "On" } else { "Off" },
                        Action::Hints,
                    ),
                ]
                .iter()
                .enumerate()
                {
                    let y = 286. + i as f32 * 65.;
                    self.text(label, 84., y + 33., 24., INK);
                    self.button(Rect::new(840., y, 356., 52.), value, *action, false);
                    self.line(vec2(84., y + 57.), vec2(790., y + 57.), LINE);
                }
                self.button(
                    Rect::new(64., 666., 240., 52.),
                    "<  Done",
                    Action::Back,
                    true,
                );
                self.footer("F fullscreen    M mute    ESC done");
            }
            Screen::Controls => {
                self.header("FIELD MANUAL", "Own the arena", "Drive through the glowing beacons. Collect them all before you run out of lives.");
                let rows = [
                    ("W A S D  /  ARROWS", "Drive and turn your tank"),
                    ("SPACE", "Fire the cannon"),
                    ("SHIFT", "Brake for a tight turn"),
                    ("TAB", "Switch cockpit, chase, and map views"),
                    ("+  /  -", "Zoom the chase or map view"),
                    ("ESC", "Pause or resume"),
                    ("F  /  F11", "Toggle fullscreen"),
                    ("M", "Mute or restore sound"),
                ];
                for (i, (keys, description)) in rows.iter().enumerate() {
                    let y = 235. + i as f32 * 48.;
                    self.text(keys, 84., y, 21., ACCENT);
                    self.text(description, 410., y, 23., INK);
                    self.line(vec2(84., y + 17.), vec2(1196., y + 17.), LINE);
                }
                self.text("Use the radar to find beacons. Supply prisms restore resources. Enemies guard the route.", 84., 636., 20., MUTED);
                self.button(
                    Rect::new(64., 666., 240., 52.),
                    "<  Got it",
                    Action::Back,
                    true,
                );
                self.footer("ARROWS select    ENTER confirm    ESC back");
            }
        }
        if let Some(notice) = &session.notice {
            self.rect(Rect::new(48., 8., 1184., 34.), PANEL);
            self.text(notice, 64., 32., 17., AMBER);
        }
    }

    fn game(&mut self, session: &Session, pack: &Pack, texture: &Texture2D) {
        let game = session.viewer.game.as_ref().unwrap();
        let zone = &pack.zones[session.selected / 4];
        let arena = &zone.arenas[session.selected % 4];
        self.text("SIGNAL FRONTIER", 32., 34., 16., ACCENT);
        self.text(&arena.name, 32., 71., 32., INK);
        self.text(
            &format!("{} / 12", session.selected + 1),
            310.,
            68.,
            18.,
            MUTED,
        );
        self.text("BEACONS", 490., 32., 15., MUTED);
        self.text(
            &format!("{} / {}", game.flags.collected, game.flags.total),
            490.,
            69.,
            32.,
            ACCENT,
        );
        self.text("SCORE", 666., 32., 15., MUTED);
        self.text(
            &format!("{:06}", game.mission.score.max(0)),
            666.,
            69.,
            32.,
            INK,
        );
        self.text("TIME", 880., 32., 15., MUTED);
        self.text(&clock(game.tank.ticks), 880., 69., 32., INK);
        if session.screen == Screen::Playing {
            self.button(
                Rect::new(1080., 24., 168., 50.),
                "Pause  ESC",
                Action::Resume,
                false,
            );
        } else {
            self.text("PAUSED", 1110., 57., 20., MUTED);
        }
        self.line(vec2(32., 90.), vec2(1248., 90.), LINE);

        // Fit the complete 800x600 core frame, including its radar, without stretching or cropping.
        self.texture(texture, Rect::new(32., 108., 800., 600.), WHITE);
        self.border(Rect::new(31., 107., 802., 602.), LINE);
        self.rect(Rect::new(856., 108., 392., 600.), PANEL);
        self.text("TACTICAL RADAR", 884., 140., 16., ACCENT);
        self.radar(game);
        self.text("HULL", 884., 318., 16., MUTED);
        let max_health = i16::from(game.vehicle_settings[1]).clamp(1, 9);
        let health = game.encounter.health.clamp(0, max_health);
        self.text(
            &format!("{} / {}", health, max_health),
            1110.,
            318.,
            24.,
            if health <= 2 { DANGER } else { INK },
        );
        for i in 0..max_health {
            self.rect(
                Rect::new(
                    884. + i as f32 * (336. / max_health as f32),
                    338.,
                    336. / max_health as f32 - 5.,
                    12.,
                ),
                if i < health {
                    if health <= 2 { DANGER } else { ACCENT }
                } else {
                    LINE
                },
            );
        }
        self.text("AMMO", 884., 393., 16., MUTED);
        self.text(
            &game.ammo.max(0).to_string(),
            884.,
            435.,
            38.,
            if game.ammo < 10 { AMBER } else { INK },
        );
        self.text("LIVES", 1080., 393., 16., MUTED);
        self.text(
            &game.mission.lives.max(0).to_string(),
            1080.,
            435.,
            38.,
            INK,
        );
        self.line(vec2(884., 461.), vec2(1220., 461.), LINE);
        self.text(camera_name(session.viewer.camera), 884., 500., 23., ACCENT);
        if session.profile.settings.control_hints {
            self.text("WASD / arrows   Drive", 884., 540., 20., MUTED);
            self.text("SPACE   Fire cannon", 884., 573., 20., MUTED);
            self.text("SHIFT   Brake", 884., 606., 20., MUTED);
            self.text("TAB   Change view", 884., 639., 20., MUTED);
        }
        self.text(&zone.name, 884., 683., 17., MUTED);
        self.text(
            "DRIVE THROUGH THE BEACONS TO COLLECT THEM",
            32.,
            752.,
            18.,
            MUTED,
        );
        self.text(
            if session.profile.settings.muted {
                "SOUND OFF  /  M"
            } else {
                "SOUND ON  /  M"
            },
            1030.,
            752.,
            17.,
            MUTED,
        );
        if game.death_transition.is_some() {
            self.rect(Rect::new(144., 348., 576., 102.), color_u8!(9, 17, 26, 235));
            self.text("TANK DESTROYED", 200., 389., 34., DANGER);
            self.text(
                if game.mission.lives > 1 {
                    "Stand by for redeployment..."
                } else {
                    "Signal lost..."
                },
                200.,
                424.,
                23.,
                INK,
            );
        } else if game.encounter.hit_flash > 0 {
            self.border(Rect::new(34., 110., 796., 596.), DANGER);
        }
    }

    fn radar(&self, game: &spectre_core::game::Game) {
        let r = Rect::new(884., 156., 136., 136.);
        self.rect(r, BG);
        self.border(r, LINE);
        for i in 1..4 {
            let d = i as f32 * 34.;
            self.line(vec2(r.x + d, r.y), vec2(r.x + d, r.y + r.h), LINE);
            self.line(vec2(r.x, r.y + d), vec2(r.x + r.w, r.y + d), LINE);
        }
        let point = |x: f32, z: f32| {
            vec2(
                r.x + 4. + x.clamp(0., 1024.) / 8.,
                r.y + 4. + z.clamp(0., 1024.) / 8.,
            )
        };
        for item in &game.items {
            if item.kind == 12 {
                let p = point(item.x, item.z);
                self.rect(Rect::new(p.x - 3., p.y - 3., 6., 6.), AMBER);
            }
        }
        for target in &game.targets {
            if target.health > 0 && !target.retired {
                let p = point(target.item.x, target.item.z);
                self.rect(Rect::new(p.x - 2., p.y - 2., 4., 4.), DANGER);
            }
        }
        let p = point(game.tank.x, game.tank.z);
        let direction = vec2(game.tank.heading.sin(), game.tank.heading.cos());
        self.line(p - direction * 3., p + direction * 8., ACCENT);
        self.rect(Rect::new(p.x - 2., p.y - 2., 4., 4.), ACCENT);
        for (i, (label, color)) in [("You", ACCENT), ("Beacons", AMBER), ("Enemies", DANGER)]
            .iter()
            .enumerate()
        {
            let y = 178. + i as f32 * 34.;
            self.rect(Rect::new(1042., y - 8., 6., 6.), *color);
            self.text(label, 1062., y, 19., INK);
        }
        self.text("Collect them all.", 1042., 285., 17., MUTED);
    }

    fn results(&mut self, session: &Session) {
        let game = session.viewer.game.as_ref().unwrap();
        let won = game.encounter.status == "won";
        let last = session.selected == 11;
        self.modal(
            if won {
                "Signal recovered"
            } else {
                "Signal lost"
            },
            if won {
                "Every beacon secured. Arena complete."
            } else if game.encounter.status == "no_spawn" {
                "No safe respawn position. Try another run."
            } else {
                "No lives left. The next run is yours."
            },
        );
        self.text("SCORE", 406., 253., 16., MUTED);
        self.text(
            &format!("{:06}", game.mission.score.max(0)),
            406.,
            299.,
            42.,
            if won { ACCENT } else { INK },
        );
        self.text("TIME", 686., 253., 16., MUTED);
        self.text(&clock(game.tank.ticks), 686., 299., 42., INK);
        self.text(
            &format!(
                "{} / {} beacons     {} enemies destroyed",
                game.flags.collected, game.flags.total, game.destroyed
            ),
            406.,
            344.,
            20.,
            MUTED,
        );
        if let Some(record) = session.profile.records.get(&session.selected) {
            self.text(
                &format!("PERSONAL BEST  {}", record.score),
                406.,
                379.,
                17.,
                ACCENT,
            );
        }
        self.button(
            Rect::new(406., 416., 468., 58.),
            if won && !last {
                "Next arena                       >"
            } else {
                "Play again"
            },
            if won && !last {
                Action::Next
            } else {
                Action::Launch
            },
            true,
        );
        self.button(
            Rect::new(406., 490., 468., 52.),
            if won && !last {
                "Replay this arena"
            } else {
                "Choose arena"
            },
            if won && !last {
                Action::Launch
            } else {
                Action::Arenas
            },
            false,
        );
        self.button(
            Rect::new(406., 558., 468., 52.),
            "Return to title",
            Action::Title,
            false,
        );
        if won && last {
            self.text(
                "End of the frontier. Find your next best time.",
                406.,
                658.,
                19.,
                MUTED,
            );
        }
    }
}

pub fn camera_name(camera: u32) -> &'static str {
    match camera {
        0 => "Cockpit",
        1 => "Chase",
        _ => "Tactical map",
    }
}
pub fn clock(ticks: u64) -> String {
    format!("{:02}:{:02}", ticks / 3600, ticks / 60 % 60)
}
