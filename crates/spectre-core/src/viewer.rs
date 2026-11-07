use crate::{
    Result,
    formats::{Shape, World},
    render::Frame,
    resource::Archive,
};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Entry {
    pub kind: String,
    pub id: i16,
    pub name: String,
    pub vertices: usize,
    pub faces: usize,
    pub objects: usize,
}
pub enum Content {
    Shape(Shape),
    World(World),
}
#[derive(Clone, Debug, Serialize)]
pub struct CampaignLevel {
    pub level: i16,
    pub difficulty: i16,
    pub wave_resource: i16,
    pub wave_index: usize,
    pub world: i16,
    pub health_modifier: i16,
}
#[derive(Default)]
pub struct Viewer {
    pub entries: Vec<Entry>,
    pub content: Vec<Content>,
    pub frame: Frame,
    pub game: Option<crate::game::Game>,
    /// 0 cockpit, 1 exterior, 2 overhead. Projection remains provisional.
    pub camera: u32,
    pub campaign: Option<CampaignLevel>,
    pub camera_zoom: i32,
    tank_index: usize,
    catalog: crate::scene::Catalog,
    shared_colors: Vec<crate::resource::Resource>,
}
impl Viewer {
    /// Load owned executable colors and radar glyphs before opening a campaign.
    pub fn load_shared_colors(&mut self, bytes: &[u8]) -> Result<()> {
        if !self.entries.is_empty() {
            return Err("Load shared colors before the campaign".into());
        }
        let archive = Archive::parse(bytes)?;
        let colors: Vec<_> = archive
            .resources
            .into_iter()
            .filter(|r| r.kind == "clut" || r.kind == "cob8" || (r.kind == "PICT" && r.id == 1000))
            .collect();
        if !colors.iter().any(|r| {
            r.kind == "cob8" && r.id == 999 && crate::recovered::color_bindings(&r.data).is_some()
        }) {
            return Err("Executable color bindings are missing or truncated".into());
        }
        self.shared_colors = colors;
        Ok(())
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<()> {
        let archive = Archive::parse(bytes)?;
        let mut entries = Vec::new();
        let mut content = Vec::new();
        let mut resources: Vec<_> = archive
            .resources
            .iter()
            .filter(|r| r.kind == "shap" || r.kind == "wrld")
            .collect();
        resources.sort_by_key(|r| (r.kind.clone(), r.id));
        for r in resources {
            let mut e = Entry {
                kind: r.kind.clone(),
                id: r.id,
                name: r.name.clone(),
                vertices: 0,
                faces: 0,
                objects: 0,
            };
            let c = if r.kind == "shap" {
                let shape = Shape::parse(&r.data).map_err(|e| format!("shap {}: {e}", r.id))?;
                e.vertices = shape.vertices.len();
                e.faces = shape.faces.len();
                Content::Shape(shape)
            } else {
                let world = World::parse(&r.data).map_err(|e| format!("wrld {}: {e}", r.id))?;
                e.objects = world.obstacles.len();
                Content::World(world)
            };
            entries.push(e);
            content.push(c);
        }
        if entries.is_empty() {
            return Err("No shap or wrld resources in this file. Open a campaign matrix.".into());
        }
        let mut merged = self.shared_colors.clone();
        // Catalog insertion order gives campaign resources precedence.
        merged.extend(archive.resources.iter().cloned());
        let catalog = crate::scene::Catalog::load(&Archive { resources: merged })?;
        self.catalog = catalog;
        self.game = None;
        self.campaign = None;
        self.entries = entries;
        self.content = content;
        Ok(())
    }
    pub fn start_game(&mut self, world_index: usize) -> Result<()> {
        self.start_arena(world_index, false, None)
    }
    pub fn start_mission(&mut self, world_index: usize) -> Result<()> {
        self.start_arena(world_index, true, None)
    }
    pub fn next_campaign_level(&mut self) -> Result<()> {
        let current = self
            .campaign
            .as_ref()
            .ok_or("Start a campaign level first")?;
        let game = self.game.as_ref().ok_or("No active game")?;
        if game.encounter.status != "won" || !game.flags.enabled || game.completion.is_none() {
            return Err("Collect every flag before advancing".into());
        }
        if game.mission.lives <= 0 {
            return Err("No lives remaining".into());
        }
        let level = current
            .level
            .checked_add(1)
            .ok_or("Campaign level exceeds supported range")?;
        let difficulty = current.difficulty;
        let progress = game.mission.clone();
        let settings = crate::recovered::campaign_upgrade(level, game.vehicle_settings);
        self.start_campaign_level(level, difficulty)?;
        self.game
            .as_mut()
            .unwrap()
            .enter_with_progress(progress, settings);
        Ok(())
    }
    pub fn start_campaign_level(&mut self, level: i16, difficulty: i16) -> Result<()> {
        if !(0..=7).contains(&difficulty) {
            return Err("Difficulty must be 0 through 7".into());
        }
        let wave_resource = crate::recovered::wave_resource_id(difficulty, 0, 0);
        let rows = self
            .catalog
            .waves
            .get(&wave_resource)
            .ok_or("Campaign wave resource missing")?;
        let selected =
            crate::recovered::select_wave(rows, level).ok_or("Invalid campaign level")?;
        if selected.wave.world == 0 || selected.wave.enemies == 0 {
            return Err("This wave requires supplied random resource selections".into());
        }
        let world_index = self
            .entries
            .iter()
            .position(|entry| entry.kind == "wrld" && entry.id == selected.wave.world)
            .ok_or("Campaign world missing")?;
        self.start_arena(
            world_index,
            true,
            Some(CampaignLevel {
                level,
                difficulty,
                wave_resource,
                wave_index: selected.index,
                world: selected.wave.world,
                health_modifier: selected.health_modifier,
            }),
        )
    }
    fn start_arena(
        &mut self,
        world_index: usize,
        flags: bool,
        campaign: Option<CampaignLevel>,
    ) -> Result<()> {
        let Some(Content::World(world)) = self.content.get(world_index) else {
            return Err("Select a world layout".into());
        };
        let tank_index = self
            .entries
            .iter()
            .position(|e| e.kind == "shap" && e.id == 10010)
            .ok_or("Tank 1 model missing")?;
        let world = if flags {
            world.with_level_flags(campaign.as_ref().map_or(1, |c| c.level))?
        } else {
            world.clone()
        };
        let mut game =
            crate::game::Game::with_items(world.clone(), self.catalog.instantiate(&world))?;
        if let Some(c) = &campaign {
            game.configure_optional_weapons(&self.catalog.waves[&c.wave_resource], c.level);
            let wave = &self.catalog.waves[&c.wave_resource][c.wave_index];
            self.catalog.populate_wave_targets(
                wave,
                &mut game,
                &crate::scene::TargetScenario {
                    resource: c.wave_resource,
                    index: c.wave_index,
                    level: c.level,
                    difficulty: c.difficulty,
                    health_modifier: c.health_modifier,
                    mission: true,
                },
            )?;
            game.scenario = format!(
                "Level {} · Difficulty {} · {}",
                c.level, c.difficulty, game.scenario
            );
        } else {
            self.catalog
                .populate_targets(self.entries[world_index].id, &mut game, flags)?;
        }
        game.configure_collision(&self.catalog);
        game.selected_difficulty = campaign.as_ref().map_or(0, |c| c.difficulty);
        if flags {
            // Standalone arenas use difficulty 0; campaign levels use their selected tier.
            let score_id = crate::recovered::score_resource_id(
                campaign.as_ref().map_or(0, |c| c.difficulty),
                0,
                0,
            );
            game.score_rules = self.catalog.score_rules.get(&score_id).cloned();
            if game.score_rules.is_none() {
                return Err("Original score table missing".into());
            }
            let template_world = crate::formats::World {
                obstacles: vec![crate::formats::Obstacle {
                    kind: 5,
                    h: 0,
                    v: 0,
                    facing: 0,
                }],
            };
            let template = self
                .catalog
                .instantiate(&template_world)
                .pop()
                .ok_or("Pickup model selection missing")?;
            let mut existing: Vec<_> = game
                .items
                .iter()
                .map(|item| [(item.x * 65536.) as i32, (item.z * 65536.) as i32])
                .collect();
            // Adapter order/initial player pose remain provisional; enemies do not
            // belong to the original scenery count used by this allocator.
            existing.push([(game.tank.x * 65536.) as i32, (game.tank.z * 65536.) as i32]);
            game.pickups = crate::pickups::create(
                &template,
                existing,
                game.dump_weights,
                &mut game.pickup_random,
            )?;
            game.configure_collision(&self.catalog);
            game.enable_flag_objective();
        }
        self.tank_index = tank_index;
        self.game = Some(game);
        self.campaign = campaign;
        Ok(())
    }
    pub fn drive(&mut self, seconds: f64, input: crate::game::Input) {
        if let Some(game) = &mut self.game {
            game.advance(seconds, input);
        }
    }
    pub fn draw_game(&mut self) {
        if let (Some(game), Some(Content::Shape(shape))) =
            (&self.game, self.content.get(self.tank_index))
        {
            let zoom = 2.0_f32.powf(self.camera_zoom.clamp(-2, 3) as f32 * 0.5);
            match self.camera {
                1 => self.frame.driving(game, shape, zoom),
                2 => self.frame.overhead(game, shape, zoom),
                _ => self.frame.first_person(game),
            }
        }
    }
    pub fn draw(&mut self, index: usize, yaw: f32, pitch: f32, zoom: f32, filled: bool) {
        if !yaw.is_finite() || !pitch.is_finite() || !zoom.is_finite() {
            return;
        }
        match self.content.get(index) {
            Some(Content::Shape(s)) => self.frame.model(s, yaw, pitch, zoom, filled),
            Some(Content::World(w)) => self.frame.world(w, usize::MAX),
            None => self.frame.clear([7, 12, 20]),
        }
    }
}

impl Viewer {
    pub fn start_deathmatch_arena(&mut self) -> Result<()> {
        let index = self
            .entries
            .iter()
            .position(|e| e.kind == "wrld" && e.id == 10000)
            .ok_or("Deathmatch arena 10000 missing")?;
        self.start_game(index)
    }
    pub fn draw_deathmatch(
        &mut self,
        snapshot: &crate::game::deathmatch::Snapshot,
        slot: usize,
    ) -> Result<()> {
        if slot > 1 {
            return Err("Invalid player slot".into());
        }
        let Some(Content::Shape(shape)) = self.content.get(self.tank_index) else {
            return Err("Tank model missing".into());
        };
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &shape.vertices {
            for (axis, n) in [v.x, v.y, v.z].into_iter().enumerate() {
                min[axis] = min[axis].min(n);
                max[axis] = max[axis].max(n);
            }
        }
        let item = crate::scene::Item {
            kind: 7,
            shape_id: 10010,
            x: 0.,
            z: 0.,
            heading: 0.,
            source: "Remote player".into(),
            min,
            max,
            shape: Some(shape.clone()),
        };
        let game = self
            .game
            .as_mut()
            .ok_or("Deathmatch view not initialized")?;
        snapshot.apply(game, slot, item);
        self.draw_game();
        Ok(())
    }
}
