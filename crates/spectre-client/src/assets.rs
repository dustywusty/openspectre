use serde::Deserialize;
use spectre_core::viewer::Viewer;

pub const SHARED: &[u8] = include_bytes!("../../../assets/open/generated/spectre.bin");
const ZONES: [&[u8]; 4] = [
    include_bytes!("../../../assets/open/generated/central.bin"),
    include_bytes!("../../../assets/open/generated/eastern.bin"),
    include_bytes!("../../../assets/open/generated/south.bin"),
    include_bytes!("../../../assets/open/generated/midnight.bin"),
];
pub const ARENAS_PER_ZONE: usize = 4;
pub const ARENA_COUNT: usize = ZONES.len() * ARENAS_PER_ZONE;

#[derive(Deserialize)]
pub struct Pack {
    pub zones: Vec<Zone>,
}

#[derive(Deserialize)]
pub struct Zone {
    pub key: String,
    pub name: String,
    pub arenas: Vec<Arena>,
}

#[derive(Deserialize)]
pub struct Arena {
    pub name: String,
    pub objects: Vec<[i32; 4]>,
}

pub fn pack() -> Result<Pack, String> {
    serde_json::from_str(include_str!("../../../assets/open/pack.json"))
        .map_err(|error| format!("Cannot read the bundled arena list: {error}"))
}

pub fn viewer(arena: usize, camera: u32) -> Result<Viewer, String> {
    let bytes = ZONES
        .get(arena / ARENAS_PER_ZONE)
        .ok_or("Arena is unavailable")?;
    let mut viewer = Viewer::default();
    viewer.load_shared_colors(SHARED)?;
    viewer.load(bytes)?;
    let id = 10000 + (arena % ARENAS_PER_ZONE) as i16;
    let world = viewer
        .entries
        .iter()
        .position(|e| e.kind == "wrld" && e.id == id)
        .ok_or("The bundled arena is missing")?;
    viewer.start_mission(world)?;
    viewer.camera = camera.min(2);
    viewer.draw_game();
    Ok(viewer)
}

pub fn sounds() -> Vec<(u16, &'static [u8])> {
    macro_rules! embedded {
        ($($id:literal),*) => { vec![$(($id, include_bytes!(concat!(
            "../../../assets/open/generated/audio/", stringify!($id), ".wav")) as &[u8])),*] };
    }
    embedded!(
        10001, 10002, 10003, 10004, 10005, 10006, 10007, 10008, 10009, 10010, 10011, 10012, 10013,
        10014, 10015, 10016, 10017, 10018, 10019, 10020, 10021, 10022, 10023, 10024, 10025, 10026,
        10027, 10028, 10029, 10030, 10031, 10032, 10033, 10034, 10035, 10036, 10037, 10038, 10039,
        10040, 10041, 10042, 10043, 10044, 10045, 10046, 10047, 10048
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_arena_starts_with_objectives_enemies_and_pixels() {
        let pack = pack().unwrap();
        assert_eq!(pack.zones.len(), ZONES.len());
        for (zone, metadata) in pack.zones.iter().enumerate() {
            assert_eq!(metadata.arenas.len(), ARENAS_PER_ZONE);
            for (index, arena) in metadata.arenas.iter().enumerate() {
                let mut viewer = viewer(zone * ARENAS_PER_ZONE + index, 1).unwrap();
                let game = viewer.game.as_ref().unwrap();
                assert!(game.encounter.enabled, "{}", arena.name);
                assert!(game.flags.total > 0, "{}", arena.name);
                assert!(!game.targets.is_empty(), "{}", arena.name);
                for camera in 0..3 {
                    viewer.camera = camera;
                    viewer.draw_game();
                    assert_eq!(viewer.frame.rgba.len(), 800 * 600 * 4);
                    assert!(
                        viewer
                            .frame
                            .rgba
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .any(|p| p != &viewer.frame.rgba[..4])
                    );
                }
            }
        }
    }

    #[test]
    fn embedded_audio_is_complete_and_decodes_without_external_files() {
        let sounds = sounds();
        assert_eq!(sounds.len(), 48);
        for id in 10001..=10048 {
            let (_, wav) = sounds.iter().find(|(sound, _)| *sound == id).unwrap();
            assert_eq!(&wav[..4], b"RIFF");
            assert!(wav.len() > 44);
        }
        assert!(viewer(ARENA_COUNT, 0).is_err());
    }
}
