use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub volume: f32,
    pub muted: bool,
    pub fullscreen: bool,
    pub camera: u32,
    pub smooth_pixels: bool,
    pub control_hints: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.35,
            muted: false,
            fullscreen: false,
            camera: 1,
            smooth_pixels: false,
            control_hints: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    pub score: i32,
    pub wins: u32,
    pub fastest_ticks: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub settings: Settings,
    pub records: BTreeMap<usize, Record>,
    pub last_arena: usize,
}

impl Profile {
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(bytes) => {
                let mut profile: Self = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("Cannot read saved settings: {error}"))?;
                if !profile.settings.volume.is_finite() {
                    profile.settings.volume = 0.35;
                }
                profile.settings.volume = profile.settings.volume.clamp(0., 1.);
                profile.settings.camera = profile.settings.camera.min(2);
                profile.last_arena = profile.last_arena.min(11);
                Ok(profile)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("Cannot read saved settings: {error}")),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let write = || -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let temporary = path.with_extension("json.tmp");
            fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
            fs::rename(temporary, path)?;
            Ok(())
        };
        write().map_err(|error| format!("Cannot save settings and scores: {error}"))
    }

    pub fn record_win(&mut self, arena: usize, score: i32, ticks: u64) {
        let record = self.records.entry(arena).or_default();
        record.score = record.score.max(score);
        record.wins = record.wins.saturating_add(1);
        record.fastest_ticks = Some(record.fastest_ticks.map_or(ticks, |old| old.min(ticks)));
    }
}

pub fn profile_path() -> PathBuf {
    if let Some(path) = std::env::var_os("SPECTRE_DATA_DIR") {
        return PathBuf::from(path).join("profile.json");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let directory = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/OpenSpectre")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("OpenSpectre")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("openspectre")
    };
    directory.join("profile.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_survives_restart_and_normalizes_old_settings() {
        let dir = std::env::temp_dir().join(format!("spectre-profile-test-{}", std::process::id()));
        let path = dir.join("profile.json");
        let mut profile = Profile::default();
        profile.settings.volume = 0.7;
        profile.settings.fullscreen = true;
        profile.record_win(3, 150, 600);
        profile.record_win(3, 100, 500);
        profile.save(&path).unwrap();
        let read = Profile::load(&path).unwrap();
        assert_eq!(read.settings.volume, 0.7);
        assert!(read.settings.fullscreen);
        assert_eq!(read.records[&3].score, 150);
        assert_eq!(read.records[&3].wins, 2);
        assert_eq!(read.records[&3].fastest_ticks, Some(500));
        fs::write(
            &path,
            r#"{"settings":{"volume":9,"camera":99},"last_arena":999}"#,
        )
        .unwrap();
        let read = Profile::load(&path).unwrap();
        assert_eq!(read.settings.volume, 1.);
        assert_eq!(read.settings.camera, 2);
        assert_eq!(read.last_arena, 11);
        assert!(read.settings.control_hints);
        fs::write(&path, b"broken").unwrap();
        assert!(Profile::load(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
