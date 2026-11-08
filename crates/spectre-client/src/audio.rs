use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};
use spectre_client::{assets, settings::Settings};
use std::collections::HashMap;

pub struct Audio {
    sounds: HashMap<u16, Sound>,
    recent: HashMap<u16, f64>,
    pub played: u64,
}

impl Audio {
    pub async fn load() -> Result<Self, String> {
        let mut sounds = HashMap::new();
        for (id, bytes) in assets::sounds() {
            let sound = load_sound_from_bytes(bytes)
                .await
                .map_err(|error| format!("Cannot load sound {id}: {error}"))?;
            sounds.insert(id, sound);
        }
        Ok(Self {
            sounds,
            recent: HashMap::new(),
            played: 0,
        })
    }

    pub fn play(&mut self, events: &[u16], settings: &Settings, now: f64) {
        if settings.muted || settings.volume <= 0. {
            return;
        }
        for id in events.iter().take(12) {
            // A stalled frame can contain several fixed ticks. Do not stack identical cues.
            if self.recent.get(id).is_some_and(|last| now - last < 0.045) {
                continue;
            }
            if let Some(sound) = self.sounds.get(id) {
                stop_sound(sound);
                play_sound(
                    sound,
                    PlaySoundParams {
                        looped: false,
                        volume: settings.volume,
                    },
                );
                self.recent.insert(*id, now);
                self.played += 1;
            }
        }
    }

    pub fn apply(&self, settings: &Settings) {
        for sound in self.sounds.values() {
            set_sound_volume(sound, if settings.muted { 0. } else { settings.volume });
        }
    }

    pub fn stop(&mut self) {
        for sound in self.sounds.values() {
            stop_sound(sound);
        }
        self.recent.clear();
    }
}
