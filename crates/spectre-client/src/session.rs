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
        self.profile.last_arena = self.selected;
        self.profile_dirty = true;
        self.screen = Screen::Playing;
        Ok(())
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
