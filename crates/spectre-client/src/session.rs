use crate::{assets, settings::Profile};
use spectre_core::{game::Input, viewer::Viewer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Arenas,
    Playing,
    Paused,
    Settings,
    Controls,
    Results,
    ConfirmLeave,
}

pub struct Session {
    pub viewer: Viewer,
    pub profile: Profile,
    pub screen: Screen,
    pub return_to: Screen,
    pub selected: usize,
    pub notice: Option<String>,
    pub profile_dirty: bool,
}

impl Session {
    pub fn new(profile: Profile) -> Result<Self, String> {
        let selected = profile.last_arena;
        Ok(Self {
            viewer: assets::viewer(selected, 1)?,
            profile,
            screen: Screen::Title,
            return_to: Screen::Title,
            selected,
            notice: None,
            profile_dirty: false,
        })
    }

    pub fn select(&mut self, arena: usize) -> Result<(), String> {
        let viewer = assets::viewer(arena, 1)?;
        self.viewer = viewer;
        self.selected = arena;
        Ok(())
    }

    pub fn launch(&mut self) -> Result<(), String> {
        self.viewer = assets::viewer(self.selected, self.profile.settings.camera)?;
        self.apply_game_settings();
        self.profile.last_arena = self.selected;
        self.profile_dirty = true;
        self.screen = Screen::Playing;
        Ok(())
    }

    /// Push gameplay options from the saved profile into the running arena.
    pub fn apply_game_settings(&mut self) {
        if let Some(game) = &mut self.viewer.game {
            game.enemy_respawn = self.profile.settings.enemy_respawn;
        }
    }

    /// Only this state advances the core. Pauses never accumulate catch-up time.
    pub fn update(&mut self, dt: f64, input: Input) -> Vec<u16> {
        if self.screen != Screen::Playing {
            return Vec::new();
        }
        self.viewer.drive(dt, input);
        let game = self
            .viewer
            .game
            .as_mut()
            .expect("A session always has an arena");
        let events = std::mem::take(&mut game.audio_events);
        match game.encounter.status {
            "won" => {
                self.profile
                    .record_win(self.selected, game.mission.score, game.tank.ticks);
                self.profile_dirty = true;
                self.screen = Screen::Results;
            }
            "lost" | "no_spawn" => self.screen = Screen::Results,
            _ => {}
        }
        events
    }

    pub fn pause(&mut self) {
        if self.screen == Screen::Playing {
            self.screen = Screen::Paused;
        }
    }

    pub fn open(&mut self, screen: Screen) {
        self.return_to = self.screen;
        self.screen = screen;
    }

    pub fn back(&mut self) {
        self.screen = match self.screen {
            Screen::Playing => Screen::Paused,
            Screen::Paused => Screen::Playing,
            Screen::Settings | Screen::Controls => self.return_to,
            Screen::ConfirmLeave => Screen::Paused,
            Screen::Arenas | Screen::Results => Screen::Title,
            Screen::Title => Screen::Title,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_settings_and_focus_loss_do_not_advance_or_leak_audio() {
        let mut session = Session::new(Profile::default()).unwrap();
        session.launch().unwrap();
        let input = Input {
            throttle: 1.,
            fire: true,
            ..Input::default()
        };
        session.update(1. / 60., input);
        session.pause();
        let ticks = session.viewer.game.as_ref().unwrap().tank.ticks;
        assert!(session.update(10., input).is_empty());
        session.open(Screen::Settings);
        assert!(session.update(10., input).is_empty());
        session.back();
        assert_eq!(session.screen, Screen::Paused);
        assert_eq!(session.viewer.game.as_ref().unwrap().tank.ticks, ticks);
        session.back();
        session.update(1. / 60., Input::default());
        assert_eq!(session.viewer.game.as_ref().unwrap().tank.ticks, ticks + 1);
    }

    #[test]
    fn results_are_recorded_once_and_replay_resets_the_match() {
        let mut session = Session::new(Profile::default()).unwrap();
        session.launch().unwrap();
        let game = session.viewer.game.as_mut().unwrap();
        game.encounter.status = "won";
        game.mission.score = 1234;
        game.tank.ticks = 600;
        session.update(1. / 60., Input::default());
        assert_eq!(session.screen, Screen::Results);
        session.update(1., Input::default());
        assert_eq!(session.profile.records[&0].wins, 1);
        session.launch().unwrap();
        let game = session.viewer.game.as_ref().unwrap();
        assert_eq!(game.mission.score, 0);
        assert_eq!(game.mission.lives, 3);
        assert_eq!(game.flags.collected, 0);
        assert_eq!(game.tank.ticks, 0);
        session.viewer.game.as_mut().unwrap().encounter.status = "lost";
        session.update(1. / 60., Input::default());
        assert_eq!(session.screen, Screen::Results);
        assert_eq!(session.profile.records[&0].wins, 1);
        session.select(assets::ARENA_COUNT - 1).unwrap();
        session.launch().unwrap();
        assert_eq!(session.profile.last_arena, assets::ARENA_COUNT - 1);
    }
}

#[cfg(test)]
mod enemy_respawn_tests {
    use super::*;
    use spectre_core::game::Game;

    /// Launch the bundled opening arena with the given saved preference.
    fn arena(enemy_respawn: bool) -> Session {
        let mut profile = Profile::default();
        profile.settings.enemy_respawn = enemy_respawn;
        let mut session = Session::new(profile).unwrap();
        session.launch().unwrap();
        let game = session.viewer.game.as_mut().unwrap();
        assert_eq!(
            game.enemy_respawn, enemy_respawn,
            "launch applies the setting"
        );
        // Survive long enough to fight; enemy damage is not under test here.
        game.infinite_health = true;
        session
    }

    fn game(session: &Session) -> &Game {
        session.viewer.game.as_ref().unwrap()
    }

    /// Drive and shoot at the nearest live enemy with real inputs until
    /// `done` holds or the tick budget runs out. Returns the ticks used.
    fn hunt(session: &mut Session, budget: u32, done: impl Fn(&Game) -> bool) -> u32 {
        for tick in 0..budget {
            let g = game(session);
            if done(g) {
                return tick;
            }
            let tank = &g.tank;
            let Some((dx, dz)) = g
                .targets
                .iter()
                .filter(|t| t.health > 0 && !t.retired)
                .map(|t| (t.item.x - tank.x, t.item.z - tank.z))
                .min_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)))
            else {
                session.update(1. / 60., Input::default());
                continue;
            };
            let aim = dx.atan2(dz);
            let delta = (aim - tank.heading).sin().atan2((aim - tank.heading).cos());
            session.update(
                1. / 60.,
                Input {
                    throttle: if delta.abs() < 0.2 && dx.hypot(dz) > 150. {
                        1.
                    } else {
                        0.
                    },
                    steer: if delta.abs() > 0.03 {
                        delta.signum()
                    } else {
                        0.
                    },
                    fire: delta.abs() < 0.12,
                    ..Input::default()
                },
            );
        }
        budget
    }

    #[test]
    fn eliminated_enemies_respawn_when_enabled() {
        let mut session = arena(true);
        let enemies = game(&session).targets.len();
        let lives: Vec<_> = game(&session).targets.iter().map(|t| t.lives).collect();
        assert!(enemies > 0);
        // Kill every enemy, then kill one again to prove it came back and fights.
        let used = hunt(&mut session, 60 * 300, |g| {
            g.targets.iter().all(|t| t.deaths > 0) && g.destroyed as usize > enemies
        });
        let g = game(&session);
        println!("respawn on: {} kills in {used} ticks", g.destroyed);
        assert!(
            g.targets.iter().all(|t| t.deaths > 0),
            "every enemy was eliminated at least once: {:?}",
            g.targets.iter().map(|t| t.deaths).collect::<Vec<_>>()
        );
        assert!(
            g.targets.iter().any(|t| t.deaths >= 2),
            "a respawned enemy was killed again"
        );
        assert!(
            g.targets.iter().all(|t| !t.retired && t.health > 0),
            "all enemies respawned"
        );
        assert_eq!(
            g.targets.iter().map(|t| t.lives).collect::<Vec<_>>(),
            lives,
            "endless respawn does not spend rebirth lives"
        );
        assert_eq!(g.encounter.status, "playing");
    }

    #[test]
    fn eliminated_enemies_stay_dead_when_disabled() {
        let mut session = arena(false);
        let enemies = game(&session).targets.len();
        let used = hunt(&mut session, 60 * 300, |g| {
            g.targets.iter().all(|t| t.retired)
        });
        let g = game(&session);
        println!("respawn off: {} kills in {used} ticks", g.destroyed);
        assert!(
            g.targets.iter().all(|t| t.retired && t.health == 0),
            "every enemy retired"
        );
        assert_eq!(
            g.destroyed as usize, enemies,
            "each enemy died exactly once"
        );
        // Keep playing: nothing comes back.
        for _ in 0..600 {
            session.update(
                1. / 60.,
                Input {
                    fire: true,
                    ..Input::default()
                },
            );
        }
        let g = game(&session);
        assert!(
            g.targets.iter().all(|t| t.retired && t.health == 0),
            "no enemy respawned"
        );
        assert_eq!(g.destroyed as usize, enemies);
    }

    #[test]
    fn toggling_the_setting_reaches_the_running_arena() {
        let mut session = arena(true);
        session.profile.settings.enemy_respawn = false;
        session.apply_game_settings();
        assert!(!game(&session).enemy_respawn);
    }
}
