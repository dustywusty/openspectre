//! Shared inspection and playable rendering with incrementally recovered original routines.
use crate::formats::{Shape, World};

pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 600;
pub struct Frame {
    pub rgba: Vec<u8>,
    inverse_depth: Vec<f32>,
    clip: Option<[i32; 4]>,
    ground_table: [[i16; 2]; 1200],
    light_directions: [i32; 360],
    camera_directions: [[i16; 2]; 360],
}
impl Default for Frame {
    fn default() -> Self {
        let mut ground_table = [[0; 2]; 1200];
        crate::recovered::initialize_ground_projection_table(&mut ground_table);
        Self {
            rgba: vec![0; WIDTH * HEIGHT * 4],
            inverse_depth: vec![0.; WIDTH * HEIGHT],
            clip: None,
            ground_table,
            light_directions: crate::recovered::candidate_movement_table(),
            camera_directions: crate::recovered::candidate_collision_table(),
        }
    }
}
impl Frame {
    pub fn clear(&mut self, color: [u8; 3]) {
        self.clip = None;
        self.inverse_depth.fill(0.);
        for p in self.rgba.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[color[0], color[1], color[2], 255]);
        }
    }
    fn pixel(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if self.clip.is_some_and(|[top, left, bottom, right]| {
            x < left || x >= right || y < top || y >= bottom
        }) {
            return;
        }
        if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
            let i = (y as usize * WIDTH + x as usize) * 4;
            self.rgba[i..i + 3].copy_from_slice(&c);
        }
    }
    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], c: [u8; 3]) {
        if a.iter().chain(b.iter()).any(|v| !v.is_finite()) {
            return;
        }
        // Liang-Barsky clipping bounds raster work even for off-screen endpoints.
        let d = [b[0] - a[0], b[1] - a[1]];
        let mut lo: f32 = 0.;
        let mut hi: f32 = 1.;
        for (p, q) in [
            (-d[0], a[0]),
            (d[0], WIDTH as f32 - 1. - a[0]),
            (-d[1], a[1]),
            (d[1], HEIGHT as f32 - 1. - a[1]),
        ] {
            if p == 0. {
                if q < 0. {
                    return;
                }
            } else if p < 0. {
                lo = lo.max(q / p);
            } else {
                hi = hi.min(q / p);
            }
        }
        if lo > hi {
            return;
        }
        let mut x = (a[0] + lo * d[0]).round() as i32;
        let mut y = (a[1] + lo * d[1]).round() as i32;
        let x1 = (a[0] + hi * d[0]).round() as i32;
        let y1 = (a[1] + hi * d[1]).round() as i32;
        let dx = (x1 - x).abs();
        let dy = -(y1 - y).abs();
        let sx = if x < x1 { 1 } else { -1 };
        let sy = if y < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.pixel(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e = 2 * err;
            if e >= dy {
                err += dy;
                x += sx;
            }
            if e <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    fn polygon(&mut self, points: &[[f32; 2]], color: [u8; 3]) {
        if points.len() < 3 {
            return;
        }
        let top = points
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as usize;
        let bottom = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(HEIGHT as f32 - 1.) as usize;
        for y in top..=bottom {
            let scan = y as f32 + 0.5;
            let mut hits = Vec::new();
            for i in 0..points.len() {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                if (a[1] <= scan && b[1] > scan) || (b[1] <= scan && a[1] > scan) {
                    hits.push(a[0] + (scan - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
            hits.sort_by(f32::total_cmp);
            for pair in hits.as_chunks::<2>().0 {
                let x0 = pair[0].ceil().max(0.) as i32;
                let x1 = pair[1].floor().min(WIDTH as f32 - 1.) as i32;
                for x in x0..=x1 {
                    self.pixel(x, y as i32, color);
                }
            }
        }
    }
    pub fn model(&mut self, shape: &Shape, yaw: f32, pitch: f32, zoom: f32, filled: bool) {
        self.clear([7, 12, 20]);
        for x in (0..WIDTH).step_by(40) {
            self.line([x as f32, 0.], [x as f32, HEIGHT as f32], [15, 27, 37]);
        }
        for y in (0..HEIGHT).step_by(40) {
            self.line([0., y as f32], [WIDTH as f32, y as f32], [15, 27, 37]);
        }
        if shape.vertices.is_empty() {
            return;
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &shape.vertices {
            for (i, n) in [v.x, v.y, v.z].iter().enumerate() {
                min[i] = min[i].min(*n);
                max[i] = max[i].max(*n);
            }
        }
        let center = std::array::from_fn::<_, 3, _>(|i| (min[i] + max[i]) * 0.5);
        let radius = shape
            .vertices
            .iter()
            .map(|v| {
                ((v.x - center[0]).powi(2) + (v.y - center[1]).powi(2) + (v.z - center[2]).powi(2))
                    .sqrt()
            })
            .fold(0., f32::max)
            .max(0.01);
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let transformed: Vec<_> = shape
            .vertices
            .iter()
            .map(|v| {
                let x = (v.x - center[0]) / radius;
                let y = (v.y - center[1]) / radius;
                let z = (v.z - center[2]) / radius;
                let rx = x * cy + z * sy;
                let rz = z * cy - x * sy;
                [rx, y * cp - rz * sp, rz * cp + y * sp]
            })
            .collect();
        let points: Vec<_> = transformed
            .iter()
            .map(|v| {
                let scale = 760. * zoom.clamp(0.3, 2.) / (4. + v[2]);
                [400. + v[0] * scale, 300. - v[1] * scale]
            })
            .collect();
        let mut faces: Vec<_> = shape
            .faces
            .iter()
            .filter(|f| !f.vertices.is_empty())
            .collect();
        faces.sort_by(|a, b| {
            let depth = |f: &crate::formats::Face| {
                f.vertices.iter().map(|i| transformed[*i][2]).sum::<f32>() / f.vertices.len() as f32
            };
            depth(b).total_cmp(&depth(a))
        });
        for (index, face) in faces.iter().enumerate() {
            let polygon: Vec<_> = face.vertices.iter().map(|&i| points[i]).collect();
            if filled {
                let shade = 28 + (index * 13 % 45) as u8;
                self.polygon(&polygon, [shade / 2, shade, shade + 10]);
            }
            for i in 0..polygon.len() {
                self.line(
                    polygon[i],
                    polygon[(i + 1) % polygon.len()],
                    [111, 244, 185],
                );
            }
        }
        // Radar geometry is decoded separately from the 3D faces.
        self.line([635., 435.], [780., 435.], [48, 77, 84]);
        for line in &shape.radar_lines {
            let point = |i: usize| {
                let v = &shape.radar_vertices[i];
                [
                    707. + (v.x - center[0]) / radius * 65.,
                    515. + (v.z - center[2]) / radius * 65.,
                ]
            };
            self.line(point(line[0]), point(line[1]), [239, 180, 91]);
        }
    }
    pub fn world(&mut self, world: &World, selected: usize) {
        self.clear([7, 12, 20]);
        let map = |h: f32, v: f32| [150. + h * 500. / 1024., 50. + v * 500. / 1024.];
        for n in (0..=1024).step_by(128) {
            self.line(map(n as f32, 0.), map(n as f32, 1024.), [29, 46, 59]);
            self.line(map(0., n as f32), map(1024., n as f32), [29, 46, 59]);
        }
        for (index, o) in world.obstacles.iter().enumerate() {
            let p = map(o.h as f32, o.v as f32);
            // Diagnostic colors; object semantics and final palette are not inferred.
            let color = if index == selected {
                [255, 255, 255]
            } else if o.kind == 14 {
                [239, 180, 91]
            } else {
                [111, 244, 185]
            };
            let radius = if index == selected { 6. } else { 3. };
            self.line(
                [p[0] - radius, p[1] - radius],
                [p[0] + radius, p[1] - radius],
                color,
            );
            self.line(
                [p[0] + radius, p[1] - radius],
                [p[0] + radius, p[1] + radius],
                color,
            );
            self.line(
                [p[0] + radius, p[1] + radius],
                [p[0] - radius, p[1] + radius],
                color,
            );
            self.line(
                [p[0] - radius, p[1] + radius],
                [p[0] - radius, p[1] - radius],
                color,
            );
            let angle = (o.facing as f32).to_radians();
            self.line(
                p,
                [p[0] + angle.sin() * 13., p[1] - angle.cos() * 13.],
                color,
            );
        }
    }
}

impl Frame {
    /// Playable map view. Uses original top-view lines where present; north-up
    /// centering, scale, colors, and fallback projections are provisional.
    pub fn overhead(&mut self, game: &crate::game::Game, tank_shape: &Shape, zoom: f32) {
        let projectile_models = game.projectile_models(true);
        self.clear([7, 12, 20]);
        // Overhead presentation: slowly rotate the arena with the tank's death
        // spin and ease out to a wider view. This does not alter simulation poses.
        let (rotation, pullback) = game.death_transition.as_ref().map_or((0., 1.), |death| {
            let elapsed = (game.tank.ticks as i32).wrapping_sub(death.started).max(0) as f32;
            let progress = (elapsed / 120.).min(1.);
            let ease = progress * progress * (3. - 2. * progress);
            (
                -std::f32::consts::TAU * elapsed / 200. * 0.35,
                1. - 0.22 * ease,
            )
        });
        let scale = 0.8 * zoom * pullback;
        let (s, c) = rotation.sin_cos();
        let map = |x: f32, z: f32| {
            let x = x - game.tank.x;
            let z = z - game.tank.z;
            [
                400. + (x * c + z * s) * scale,
                300. - (z * c - x * s) * scale,
            ]
        };
        for n in (0..=1024).step_by(64) {
            self.line(map(n as f32, 0.), map(n as f32, 1024.), [24, 43, 51]);
            self.line(map(0., n as f32), map(1024., n as f32), [24, 43, 51]);
        }
        for (a, b) in [
            ([0., 0.], [1024., 0.]),
            ([1024., 0.], [1024., 1024.]),
            ([1024., 1024.], [0., 1024.]),
            ([0., 1024.], [0., 0.]),
        ] {
            self.line(map(a[0], a[1]), map(b[0], b[1]), [145, 139, 81]);
        }
        // CODE 6 +2cfe..2d10 selects the primary kind color for top-view drawing.
        // Participant flashing and multiplayer color overrides remain separate.
        let kind_color = |kind: i16, fallback| {
            game.color_bindings
                .as_ref()
                .and_then(|bindings| bindings.kinds.get(usize::try_from(kind).ok()?))
                .and_then(|pair| game.palette.as_ref()?.get(usize::from(pair[0])))
                .map(|rgb| rgb.map(|v| (v >> 8) as u8))
                .unwrap_or(fallback)
        };
        let mut draw_shape = |shape: &Shape, x: f32, z: f32, heading: f32, color| {
            let (s, c) = heading.sin_cos();
            let point =
                |v: &crate::formats::Vertex| map(x + v.x * c + v.z * s, z - v.x * s + v.z * c);
            if !shape.radar_lines.is_empty() {
                let count = shape.top_view_fill_count();
                for [a, b] in shape.radar_lines.iter().skip(count) {
                    self.line(
                        point(&shape.radar_vertices[*a]),
                        point(&shape.radar_vertices[*b]),
                        color,
                    );
                }
                let polygon: Vec<_> = shape.radar_vertices.iter().take(count).map(point).collect();
                self.polygon(&polygon, color);
            } else {
                // Procedural models may lack top-view data. Project their actual
                // mesh edges rather than inventing an original map symbol.
                for face in &shape.faces {
                    for i in 0..face.vertices.len() {
                        self.line(
                            point(&shape.vertices[face.vertices[i]]),
                            point(&shape.vertices[face.vertices[(i + 1) % face.vertices.len()]]),
                            color,
                        );
                    }
                }
            }
        };
        for (item, participant, color) in game
            .items
            .iter()
            .chain(game.pickups.iter().map(|p| &p.item))
            .map(|i| {
                (
                    i,
                    false,
                    if let Some(rgb) = game.pickup_color(i).and_then(|index| {
                        game.palette
                            .as_ref()?
                            .get(index as usize)
                            .map(|rgb| rgb.map(|v| (v >> 8) as u8))
                    }) {
                        rgb
                    } else {
                        kind_color(
                            i.kind,
                            if i.kind == 12 {
                                [255, 240, 40]
                            } else {
                                [222, 177, 100]
                            },
                        )
                    },
                )
            })
            .chain(
                game.targets
                    .iter()
                    .filter(|t| t.health > 0)
                    .map(|t| (&t.item, true, kind_color(t.item.kind, [255, 105, 105]))),
            )
            .chain(
                projectile_models
                    .iter()
                    .map(|(item, _)| (item, true, kind_color(34, [255, 245, 155]))),
            )
        {
            if let Some(shape) = &item.shape {
                draw_shape(
                    shape,
                    item.x,
                    item.z,
                    item.scene_heading(participant),
                    color,
                );
            }
        }
        draw_shape(
            tank_shape,
            game.tank.x,
            game.tank.z,
            // Apply the recovered death-camera spin to the tank symbol too.
            game.death_camera().map_or(game.tank.heading, |camera| {
                (90. - camera.pose[2] as f32 / 65536.).to_radians()
            }),
            kind_color(6, [145, 255, 191]),
        );
        for shot in game.shots.iter().chain(&game.enemy_shots) {
            if shot.missile.is_some() && game.missile_model.is_some() {
                continue;
            }
            self.line(
                map(shot.x, shot.z),
                map(
                    shot.x - shot.heading.sin() * 10.,
                    shot.z - shot.heading.cos() * 10.,
                ),
                [255, 245, 155],
            );
        }
        for impact in &game.impacts {
            let r = (37 - impact.life.min(36)) as f32 * 0.5;
            self.line(
                map(impact.x - r, impact.z),
                map(impact.x + r, impact.z),
                [255, 145, 70],
            );
            self.line(
                map(impact.x, impact.z - r),
                map(impact.x, impact.z + r),
                [255, 145, 70],
            );
        }
        // The manual explicitly omits the separate radar in map view.
    }
}

// Chase camera for the driving prototype. Wireframe geometry and collision boxes
// are deliberate diagnostic visuals, not the original game's projection.
impl Frame {
    pub fn driving(&mut self, game: &crate::game::Game, shape: &Shape, zoom: f32) {
        let projectile_models = game.projectile_models(true);
        if game.death_transition.is_some() {
            self.first_person(game);
            return;
        }
        let t = &game.tank;
        self.clear([7, 12, 20]);
        let (s, c) = t.heading.sin_cos();
        let altitude = game.vertical.vertical as f32 / 524288.;
        let camera = [t.x - s * 100., 65. + altitude, t.z - c * 100.];
        let transform = |p: [f32; 3]| {
            let x = p[0] - camera[0];
            let y = p[1] - camera[1];
            let z = p[2] - camera[2];
            let forward = x * s + z * c;
            [
                x * c - z * s,
                y * 0.94 + forward * 0.342,
                forward * 0.94 - y * 0.342,
            ]
        };
        let mut edge = |a: [f32; 3], b: [f32; 3], color| {
            let mut a = transform(a);
            let mut b = transform(b);
            if a[2] < 2. && b[2] < 2. {
                return;
            }
            if a[2] < 2. || b[2] < 2. {
                let ratio = (2. - a[2]) / (b[2] - a[2]);
                let cut = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * ratio);
                if a[2] < 2. {
                    a = cut;
                } else {
                    b = cut;
                }
            }
            let project = |p: [f32; 3]| {
                [
                    400. + p[0] * 500. * zoom / p[2],
                    285. - p[1] * 500. * zoom / p[2],
                ]
            };
            self.line(project(a), project(b), color);
        };
        for n in (0..=1024).step_by(64) {
            edge([n as f32, 0., 0.], [n as f32, 0., 1024.], [24, 43, 51]);
            edge([0., 0., n as f32], [1024., 0., n as f32], [24, 43, 51]);
        }
        for (a, b) in [
            ([0., 0., 0.], [1024., 0., 0.]),
            ([1024., 0., 0.], [1024., 0., 1024.]),
            ([1024., 0., 1024.], [0., 0., 1024.]),
            ([0., 0., 1024.], [0., 0., 0.]),
        ] {
            edge(a, b, [145, 139, 81]);
        }
        for (item, enemy, raw_height) in game
            .items
            .iter()
            .chain(game.pickups.iter().map(|p| &p.item))
            .map(|i| (i, false, 0))
            .chain(
                game.targets
                    .iter()
                    .filter(|t| t.health > 0)
                    .map(|t| (&t.item, true, 0)),
            )
            .chain(
                projectile_models
                    .iter()
                    .map(|(item, height)| (item, true, *height)),
            )
        {
            let pickup_rgb = game.pickup_color(item).and_then(|index| {
                game.palette
                    .as_ref()?
                    .get(index as usize)
                    .map(|rgb| rgb.map(|v| (v >> 8) as u8))
            });
            let color = if let Some(rgb) = pickup_rgb {
                rgb
            } else if enemy {
                [255, 105, 105]
            } else if item.kind == 12 {
                [255, 240, 40]
            } else {
                [222, 177, 100]
            };
            let (sin, cos) = item.scene_heading(enemy).sin_cos();
            let point = |x: f32, y: f32, z: f32| {
                [
                    item.x + x * cos + z * sin,
                    y + raw_height as f32 / 524288.,
                    item.z - x * sin + z * cos,
                ]
            };
            if let Some(shape) = &item.shape {
                for face in &shape.faces {
                    for i in 0..face.vertices.len() {
                        let a = &shape.vertices[face.vertices[i]];
                        let b = &shape.vertices[face.vertices[(i + 1) % face.vertices.len()]];
                        edge(point(a.x, a.y, a.z), point(b.x, b.y, b.z), color);
                    }
                }
            } else {
                // Magenta cross: unresolved generated geometry, never a fake original model.
                edge(point(-12., 0., -12.), point(12., 24., 12.), [224, 109, 191]);
                edge(point(-12., 24., 12.), point(12., 0., -12.), [224, 109, 191]);
            }
        }
        for shot in game.shots.iter().chain(&game.enemy_shots) {
            if shot.missile.is_some() && game.missile_model.is_some() {
                continue;
            }
            edge(
                [shot.x, shot.height, shot.z],
                [
                    shot.x - shot.heading.sin() * 10.,
                    shot.height,
                    shot.z - shot.heading.cos() * 10.,
                ],
                [255, 245, 155],
            );
        }
        for impact in &game.impacts {
            let r = (37 - impact.life.min(36)) as f32 * 0.5;
            edge(
                [impact.x - r, 4., impact.z],
                [impact.x + r, 4., impact.z],
                [255, 145, 70],
            );
            edge(
                [impact.x, 4. - r, impact.z],
                [impact.x, 4. + r, impact.z],
                [255, 145, 70],
            );
            edge(
                [impact.x, 4., impact.z - r],
                [impact.x, 4., impact.z + r],
                [255, 145, 70],
            );
        }
        let point = |i: usize| {
            let v = &shape.vertices[i];
            [
                t.x + v.x * c + v.z * s,
                v.y + altitude,
                t.z - v.x * s + v.z * c,
            ]
        };
        for face in &shape.faces {
            for i in 0..face.vertices.len() {
                edge(
                    point(face.vertices[i]),
                    point(face.vertices[(i + 1) % face.vertices.len()]),
                    [145, 255, 191],
                );
            }
        }
        // Fixed world minimap: same coordinates as the simulation, north is -Z.
        let map = |x: f32, z: f32| [630. + x * 0.14, 425. + z * 0.14];
        self.polygon(
            &[[620., 415.], [785., 415.], [785., 580.], [620., 580.]],
            [9, 22, 27],
        );
        for o in game
            .items
            .iter()
            .chain(game.pickups.iter().map(|p| &p.item))
        {
            let p = map(o.x, o.z);
            self.line([p[0] - 2., p[1]], [p[0] + 2., p[1]], [222, 177, 100]);
        }
        for target in game.targets.iter().filter(|t| t.health > 0) {
            let p = map(target.item.x, target.item.z);
            self.line([p[0] - 3., p[1]], [p[0] + 3., p[1]], [255, 105, 105]);
            self.line([p[0], p[1] - 3.], [p[0], p[1] + 3.], [255, 105, 105]);
        }
        let p = map(t.x, t.z);
        self.line([p[0] - 3., p[1]], [p[0] + 3., p[1]], [145, 255, 191]);
        self.line([p[0], p[1] - 3.], [p[0], p[1] + 3.], [145, 255, 191]);
        self.line(p, [p[0] + s * 12., p[1] + c * 12.], [145, 255, 191]);
    }
}

// Unsupported preview geometry retains floating clipping. Packed original
// geometry uses recovered projection; depth rasterization remains host-native.
fn near_clip(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut result = Vec::new();
    if points.is_empty() {
        return result;
    }
    let mut a = points[points.len() - 1];
    for &b in points {
        if (a[2] >= 1.) != (b[2] >= 1.) {
            let t = (1. - a[2]) / (b[2] - a[2]);
            result.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), 1.]);
        }
        if b[2] >= 1. {
            result.push(b);
        }
        a = b;
    }
    result
}

impl Frame {
    // Screen x/y and reciprocal camera depth. Reciprocal depth is linear
    // across a projected planar face, unlike camera-space Z.
    fn depth_pixel(&mut self, x: i32, y: i32, inverse: f32, color: [u8; 3]) {
        if self.clip.is_some_and(|[top, left, bottom, right]| {
            x < left || x >= right || y < top || y >= bottom
        }) {
            return;
        }
        if x < 0
            || y < 0
            || x >= WIDTH as i32
            || y >= HEIGHT as i32
            || !inverse.is_finite()
            || inverse <= 0.
        {
            return;
        }
        let index = y as usize * WIDTH + x as usize;
        if inverse > self.inverse_depth[index] {
            self.inverse_depth[index] = inverse;
            self.pixel(x, y, color);
        }
    }
    fn depth_polygon(&mut self, points: &[[f32; 3]], color: [u8; 3]) {
        self.depth_polygon_paint(points, |_, _| color);
    }
    fn depth_polygon_paint(&mut self, points: &[[f32; 3]], paint: impl Fn(i32, i32) -> [u8; 3]) {
        if points.len() < 3 || points.iter().flatten().any(|v| !v.is_finite()) {
            return;
        }
        let top = points
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as i32;
        let bottom = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(HEIGHT as f32 - 1.) as i32;
        let mut hits = Vec::with_capacity(points.len());
        for y in top..=bottom {
            hits.clear();
            let scan = y as f32 + 0.5;
            for i in 0..points.len() {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                if (a[1] <= scan && b[1] > scan) || (b[1] <= scan && a[1] > scan) {
                    let t = (scan - a[1]) / (b[1] - a[1]);
                    hits.push([a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2])]);
                }
            }
            hits.sort_by(|a, b| a[0].total_cmp(&b[0]));
            // Even-odd spans preserve concave faces without a convex triangle fan.
            for pair in hits.as_chunks::<2>().0 {
                let [a, b] = *pair;
                if b[0] <= a[0] {
                    continue;
                }
                let left = (a[0] - 0.5).ceil().max(0.) as i32;
                let right = (b[0] - 0.5).ceil().min(WIDTH as f32) as i32;
                for x in left..right {
                    let t = (x as f32 + 0.5 - a[0]) / (b[0] - a[0]);
                    self.depth_pixel(x, y, a[1] + t * (b[1] - a[1]), paint(x, y));
                }
            }
        }
    }
    fn depth_line(&mut self, a: [f32; 3], b: [f32; 3], color: [u8; 3]) {
        if a.iter().chain(&b).any(|v| !v.is_finite()) {
            return;
        }
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let (mut lo, mut hi) = (0_f32, 1_f32);
        for (p, q) in [
            (-d[0], a[0]),
            (d[0], WIDTH as f32 - 1. - a[0]),
            (-d[1], a[1]),
            (d[1], HEIGHT as f32 - 1. - a[1]),
        ] {
            if p == 0. {
                if q < 0. {
                    return;
                }
            } else if p < 0. {
                lo = lo.max(q / p);
            } else {
                hi = hi.min(q / p);
            }
        }
        if lo > hi {
            return;
        }
        let steps = ((hi - lo) * d[0].abs().max(d[1].abs())).ceil() as usize;
        for i in 0..=steps {
            let t = lo + (hi - lo) * i as f32 / steps.max(1) as f32;
            self.depth_pixel(
                (a[0] + t * d[0]).round() as i32,
                (a[1] + t * d[1]).round() as i32,
                a[2] + t * d[2],
                color,
            );
        }
    }
    fn radar_overlay(
        &mut self,
        game: &crate::game::Game,
        layout: &crate::recovered::ViewportLayout,
        display: &crate::recovered::DisplayRectangle,
        scale: f32,
    ) {
        let (Some(glyphs), Some(bindings), Some(palette), Some(settings)) = (
            &game.radar_glyphs,
            &game.color_bindings,
            &game.palette,
            crate::radar::scale_for_arena(1024),
        ) else {
            return;
        };
        let center = layout.radar_center;
        let screen = |p: [i16; 2]| {
            [
                (f32::from(p[0].wrapping_add(display.bounds[1])) * scale).round() as i32,
                (f32::from(p[1].wrapping_add(display.bounds[0])) * scale).round() as i32,
            ]
        };
        let color = |index: u8| {
            palette
                .get(usize::from(index))
                .map(|rgb| rgb.map(|v| (v >> 8) as u8))
        };
        // CODE 6 +9b6 flushes the thirteen color queues in ascending order.
        let mut queues: [Vec<[i16; 2]>; 13] = std::array::from_fn(|_| Vec::new());
        if let Some(queue) = queues.get_mut(usize::from(bindings.special[1])) {
            queue.extend(crate::recovered::radar_edge_points(center));
        }
        let (position, _, _) = game.first_person_camera();
        let basis = self.camera_directions
            [crate::recovered::direction_table_index(game.first_person_heading())];
        let mut display_state = game.radar_display.borrow_mut();
        let mut glyph = |id: i16, point: [i16; 2], index: u8| {
            let Some(points) = id.checked_sub(1).and_then(|i| glyphs.get(i as usize)) else {
                return;
            };
            if let Some(queue) = queues.get_mut(usize::from(index)) {
                queue.extend(display_state.emit(point, points, game.tank.ticks as i32));
            }
        };
        let enemies = game
            .targets
            .iter()
            .filter(|t| t.health > 0 && !t.retired)
            .filter_map(|target| {
                let pair = bindings.kinds.get(target.item.kind as usize)?;
                let id = crate::radar::enemy_glyph(
                    target.item.kind as i8,
                    0,
                    pair[1],
                    target.vulnerable,
                )
                .or_else(|| crate::radar::object_glyph(target.item.kind as i8, 0, pair[1]))?;
                Some((&target.item, id, pair[1]))
            });
        let objects = game.items.iter().filter_map(|item| {
            let pair = bindings.kinds.get(item.kind as usize)?;
            let id = if item.kind == 12 {
                crate::radar::single_player_flag_glyph(0, pair[1], game.radar_flash)
            } else {
                crate::radar::object_glyph(item.kind as i8, 0, pair[1])
            }?;
            Some((item, id, pair[1]))
        });
        let pickups = game.pickups.iter().filter_map(|pickup| {
            let (id, color) = crate::radar::pickup_glyph(pickup.subtype, 0, &bindings.effects)?;
            Some((&pickup.item, id as i16, color))
        });
        for (item, id, radar_color) in objects.chain(enemies).chain(pickups) {
            let delta = [
                ((item.x * 65536.) as i32).wrapping_sub(position[0]),
                ((item.z * 65536.) as i32).wrapping_sub(position[1]),
            ];
            let [horizontal, depth] = crate::recovered::render_record_planar(delta, basis);
            let point = crate::recovered::radar_point(
                [horizontal, depth],
                -1,
                center,
                settings.threshold,
                settings.scale_base,
                settings.falloff as i16,
            );
            glyph(id, point, radar_color);
        }
        if let Some(pair) = bindings.kinds.get(10)
            && let Some((id, radar_color)) = crate::radar::retired_glyph(0, pair[0], pair[1])
        {
            for shot in game
                .shots
                .iter()
                .chain(&game.enemy_shots)
                .filter(|s| s.retiring)
            {
                let delta = [
                    ((shot.x * 65536.) as i32).wrapping_sub(position[0]),
                    ((shot.z * 65536.) as i32).wrapping_sub(position[1]),
                ];
                let [horizontal, depth] = crate::recovered::render_record_planar(delta, basis);
                let point = crate::recovered::radar_point(
                    [horizontal, depth],
                    -1,
                    center,
                    settings.threshold,
                    settings.scale_base,
                    settings.falloff as i16,
                );
                glyph(id, point, radar_color);
            }
        }
        if game.death_camera().is_none()
            && let Some(pair) = bindings.kinds.get(6)
        {
            glyph(41, center, pair[0]);
        }
        for (index, points) in queues.iter().enumerate() {
            if let Some(rgb) = color(index as u8) {
                for &point in points {
                    let [x, y] = screen(point);
                    self.pixel(x, y, rgb);
                }
            }
        }
    }

    pub fn first_person(&mut self, game: &crate::game::Game) {
        let projectile_models = game.projectile_models(false);
        self.clear([0, 0, 0]);
        // Original compact immediate-display layout on a 640x480 screen.
        // The host scales that screen uniformly to its 800x600 framebuffer.
        let screen = [0, 0, 480, 640];
        let display = crate::recovered::display_rectangle(screen, 0);
        let layout = crate::recovered::viewport_layout(screen, display.size.map(|v| v as i16));
        let scale = WIDTH as f32 / 640.;
        let center_x = f32::from(display.bounds[1] + layout.metrics.center[0]) * scale;
        let horizon = f32::from(display.bounds[0] + layout.metrics.center[1]) * scale;
        let focal = 256. * scale;
        self.clip = Some(display.bounds.map(|v| (f32::from(v) * scale).ceil() as i32));
        let project = |p: [f32; 3]| {
            [
                center_x + focal * p[0] / p[2],
                horizon - focal * p[1] / p[2],
            ]
        };
        let death = game.death_camera();
        let (position, direction, renderer_height) = game.first_person_camera();
        let camera_basis = self.camera_directions
            [crate::recovered::direction_table_index(game.first_person_heading())];
        let [x, z] = position.map(|value| value as f32 / 65536.);
        let height = crate::recovered::camera_height_in_scene(renderer_height);
        // The original heading convention is rotated relative to scene heading.
        let c = direction[0] as f32 / 65536.;
        let s = direction[1] as f32 / 65536.;
        let transform = |p: [f32; 3]| {
            let dx = p[0] - x;
            let dz = p[2] - z;
            [dx * c - dz * s, p[1] - height, dx * s + dz * c]
        };
        let ramps = game
            .palette
            .as_ref()
            .map(|base| crate::recovered::rgb_shade_ramps_8(base, [0; 3]));
        let color_map: [i16; 13] = std::array::from_fn(|i| i as i16);
        let horizon_color = game
            .color_bindings
            .as_ref()
            .map_or(3, |b| b.special[6] as i8);
        let bands = crate::recovered::horizon_bands(crate::recovered::HorizonInput {
            center: [0, 0],
            half_width: 256,
            vertical_offset: 0,
            level: game.vertical.renderer_height,
            depth: 8,
            four_bit: false,
            color: horizon_color,
            solid_color: None,
        });
        for [top, _, bottom, _, color, shade] in bands {
            let Some(pattern) =
                crate::recovered::face_pixel_pattern_8(shade, color as u8, &color_map)
            else {
                continue;
            };
            let top = (horizon + f32::from(top) * scale).floor() as i32;
            let bottom = (horizon + f32::from(bottom) * scale).floor() as i32;
            for y in top.max(0)..bottom.min(HEIGHT as i32) {
                for x in 0..WIDTH as i32 {
                    let index = crate::recovered::face_pattern_pixel_8(pattern, x, y);
                    let color = ramps
                        .as_ref()
                        .filter(|_| game.color_bindings.is_some())
                        .and_then(|ramps| ramps.get(usize::from(index)))
                        .map(|rgb| rgb.map(|v| (v >> 8) as u8))
                        // Diagnostic color only; band placement still follows CODE 6.
                        .unwrap_or_else(|| {
                            let value = (48 - shade).clamp(0, 48) as u8 * 2;
                            [value, 0, value / 2]
                        });
                    self.pixel(x, y, color);
                }
            }
        }
        // Ground and solid geometry share the original 256-pixel projection.
        let ground_color = game
            .palette
            .as_ref()
            .and_then(|palette| {
                game.ground_color_index
                    .and_then(|index| palette.get(usize::from(index)))
            })
            .map(|rgb| rgb.map(|channel| (channel >> 8) as u8))
            .unwrap_or([95, 110, 100]);
        for point in crate::recovered::ground_points(
            position,
            direction,
            renderer_height,
            [0, 0],
            false,
            &self.ground_table,
        ) {
            self.pixel(
                (center_x + f32::from(point[0]) * scale) as i32,
                (horizon + f32::from(point[1]) * scale) as i32,
                ground_color,
            );
        }
        let kind_colors = game.color_bindings.as_ref().map(|bindings| {
            bindings
                .kinds
                .iter()
                .map(|pair| pair[0])
                .collect::<Vec<_>>()
        });
        for (item, enemy, raw_height) in game
            .items
            .iter()
            .chain(game.pickups.iter().map(|p| &p.item))
            .map(|i| (i, false, 0))
            .chain(
                game.targets
                    .iter()
                    .filter(|t| t.health > 0)
                    .map(|t| (&t.item, true, 0)),
            )
            .chain(
                projectile_models
                    .iter()
                    .map(|(item, height)| (item, true, *height)),
            )
        {
            let Some(shape) = &item.shape else {
                continue;
            };
            let (sin, cos) = item.scene_heading(enemy).sin_cos();
            // Scene scenery retains native authored degrees. Moving targets use
            // the simulation adapter's 90-degree-minus-native convention.
            let native_heading = item.native_heading(enemy);
            let delta = [
                ((item.x * 65536.) as i32).wrapping_sub(position[0]),
                ((item.z * 65536.) as i32).wrapping_sub(position[1]),
            ];
            let [horizontal, depth] = crate::recovered::render_record_planar(delta, camera_basis);
            if !crate::recovered::solid_origin_visible(horizontal, depth, false)
                || !crate::recovered::solid_distance_visible(horizontal, depth, 0, -1).0
            {
                continue;
            }

            let mut packed_records = None;
            let vertices: Vec<_> = if shape.packed_vertices.len() == shape.vertices.len() {
                let (rows, translation) = crate::recovered::model_view_transform(
                    [horizontal, (raw_height >> 16) as i16, depth],
                    renderer_height as i16,
                    native_heading.wrapping_sub(game.first_person_heading()),
                    &self.light_directions,
                );
                let mut packed = vec![[0; 5]; shape.packed_vertices.len()];
                crate::recovered::transform_project_vertices(
                    &shape.packed_vertices,
                    rows,
                    translation,
                    &mut packed,
                )
                .expect("matching packed vertex counts");
                packed_records = Some(packed);
                packed_records
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        [
                            f32::from(v[0]) / 8.,
                            f32::from(v[1]) / 8.,
                            f32::from(v[2]) / 8.,
                        ]
                    })
                    .collect()
            } else {
                // Unsupported preview meshes retain their diagnostic geometry.
                let origin = [-f32::from(horizontal) / 8., f32::from(depth) / 8.];
                shape
                    .vertices
                    .iter()
                    .map(|v| {
                        let dx = v.x * cos + v.z * sin;
                        let dz = -v.x * sin + v.z * cos;
                        [
                            origin[0] + dx * c - dz * s,
                            v.y + raw_height as f32 / 524288. - height,
                            origin[1] + dx * s + dz * c,
                        ]
                    })
                    .collect()
            };
            let viewpoint = crate::recovered::face_viewpoint(
                delta,
                native_heading,
                &self.light_directions,
                renderer_height as i16,
                (raw_height >> 16) as i16,
            );
            let angle = crate::recovered::direction_table_index(native_heading);
            // CODE 12 +140 returns cosine first, sine second for these locals.
            let light = crate::recovered::face_light_components([
                self.light_directions[(450 - angle) % 360] as i16,
                self.light_directions[angle] as i16,
            ]);
            for (index, face) in shape.faces.iter().enumerate() {
                if face
                    .plane
                    .as_ref()
                    .is_some_and(|plane| !crate::recovered::face_plane_visible(plane, viewpoint))
                {
                    continue;
                }

                let projected: Vec<_> = if let Some(records) =
                    packed_records.as_ref().filter(|_| face.vertices.len() >= 2)
                {
                    let clipped = if face.vertices.len() == 2 {
                        crate::recovered::clip_edge_records([
                            records[face.vertices[0]],
                            records[face.vertices[1]],
                        ])
                        .map(Vec::from)
                        .unwrap_or_default()
                    } else {
                        crate::recovered::clip_face_records(records, &face.vertices)
                            .unwrap_or_default()
                    };
                    clipped
                        .iter()
                        .map(|v| {
                            [
                                center_x + f32::from(v[3]) * scale,
                                horizon - f32::from(v[4]) * scale,
                                8. / f32::from(v[2]),
                            ]
                        })
                        .collect()
                } else {
                    // Unsupported preview geometry retains the floating projection.
                    let points: Vec<_> = face.vertices.iter().map(|&i| vertices[i]).collect();
                    near_clip(&points)
                        .iter()
                        .map(|&p| {
                            let q = project(p);
                            [q[0], q[1], 1. / p[2]]
                        })
                        .collect()
                };
                if projected.len() < 2 || (face.vertices.len() >= 3 && projected.len() < 3) {
                    continue;
                }
                let shade = 160 + (index % 4) as u8 * 25;
                let color = if enemy {
                    [shade, 30, 20]
                } else if item.kind == 12 {
                    [shade, shade, 20]
                } else {
                    [20, shade / 2, shade]
                };
                if projected.len() == 2 {
                    self.depth_line(projected[0], projected[1], color);
                } else {
                    let original = (|| {
                        let normal = face.normal?;
                        if normal == [-1; 3] {
                            return None;
                        }
                        let kinds = kind_colors.as_ref()?;
                        let selected =
                            crate::recovered::face_color_selector(face.color as i16, kinds)?;
                        let selected = if selected == 13 {
                            i16::from(
                                game.pickup_color(item)
                                    .unwrap_or(*kinds.get(usize::try_from(item.kind).ok()?)?),
                            )
                        } else {
                            selected
                        };
                        if !(0..13).contains(&selected) {
                            return None;
                        }
                        let shade = crate::recovered::face_light_shade(normal, light);
                        let pattern = crate::recovered::face_pixel_pattern_8(
                            shade,
                            selected as u8,
                            &color_map,
                        )?;
                        let ramp = ramps.as_ref()?;
                        let mut pixels = [[0; 3]; 4];
                        for (i, rgb) in pixels.iter_mut().enumerate() {
                            let index = crate::recovered::face_pattern_pixel_8(
                                pattern,
                                (i % 2) as i32,
                                (i / 2) as i32,
                            );
                            // Index 255 is the display's black entry; RGB ramp entries occupy 0..192.
                            *rgb = if index == 255 {
                                [0; 3]
                            } else {
                                ramp.get(usize::from(index))?.map(|v| (v >> 8) as u8)
                            };
                        }
                        Some(pixels)
                    })();
                    if let Some(pixels) = original {
                        self.depth_polygon_paint(&projected, |x, y| {
                            pixels[((y & 1) * 2 + (x & 1)) as usize]
                        });
                    } else {
                        self.depth_polygon(&projected, color);
                    }
                }
            }
        }
        for shot in game.shots.iter().chain(&game.enemy_shots) {
            // CODE 6 +288c excludes retired kind-10 records from world drawing.
            if shot.retiring || (shot.missile.is_some() && game.missile_model.is_some()) {
                continue;
            }
            let p = transform([shot.x, shot.height, shot.z]);
            if p[2] >= 1. {
                let q = project(p);
                self.depth_line(
                    [q[0] - 3., q[1], 1. / p[2]],
                    [q[0] + 3., q[1], 1. / p[2]],
                    [255, 245, 155],
                );
            }
        }
        for impact in &game.impacts {
            let p = transform([impact.x, 4., impact.z]);
            if p[2] >= 1. {
                let q = project(p);
                let radius = ((37 - impact.life.min(36)) as f32 * 100. / p[2]).min(60.);
                self.depth_line(
                    [q[0] - radius, q[1] - radius, 1. / p[2]],
                    [q[0] + radius, q[1] + radius, 1. / p[2]],
                    [255, 145, 70],
                );
                self.depth_line(
                    [q[0] - radius, q[1] + radius, 1. / p[2]],
                    [q[0] + radius, q[1] - radius, 1. / p[2]],
                    [255, 145, 70],
                );
            }
        }
        let crosshair_color = game
            .color_bindings
            .as_ref()
            .and_then(|bindings| game.palette.as_ref()?.get(usize::from(bindings.special[1])))
            .map(|rgb| rgb.map(|v| (v >> 8) as u8))
            .unwrap_or([230, 235, 230]);
        for [x0, y0, x1, y1] in crate::recovered::crosshair_lines([0, 0], death.is_some()) {
            let point = |x: i16, y: i16| {
                [
                    center_x + f32::from(x) * scale,
                    horizon + f32::from(y) * scale,
                ]
            };
            self.line(point(x0, y0), point(x1, y1), crosshair_color);
        }
        self.radar_overlay(game, &layout, &display, scale);
        self.clip = None;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn patterned_faces_keep_phase_across_polygon_boundaries() {
        let mut whole = Frame::default();
        let mut split = Frame::default();
        whole.clear([1, 2, 3]);
        split.clear([1, 2, 3]);
        let paint = |x: i32, y: i32| [((x & 1) * 80 + 20) as u8, ((y & 1) * 90 + 30) as u8, 0];
        whole.depth_polygon_paint(
            &[
                [20., 20., 1.],
                [60., 20., 1.],
                [60., 60., 1.],
                [20., 60., 1.],
            ],
            paint,
        );
        for (left, right) in [(20., 41.), (41., 60.)] {
            split.depth_polygon_paint(
                &[
                    [left, 20., 1.],
                    [right, 20., 1.],
                    [right, 60., 1.],
                    [left, 60., 1.],
                ],
                paint,
            );
        }
        let near = [
            [30., 30., 2.],
            [50., 30., 2.],
            [50., 50., 2.],
            [30., 50., 2.],
        ];
        whole.depth_polygon(&near, [255, 255, 255]);
        split.depth_polygon(&near, [255, 255, 255]);
        assert_eq!(whole.rgba, split.rgba);
        let outside = (10 * WIDTH + 10) * 4;
        assert_eq!(&whole.rgba[outside..outside + 3], &[1, 2, 3]);
        let inside = (21 * WIDTH + 21) * 4;
        assert_eq!(&whole.rgba[inside..inside + 3], &[100, 120, 0]);
    }

    use super::*;
    fn rgb(frame: &Frame, x: usize, y: usize) -> &[u8] {
        &frame.rgba[(y * WIDTH + x) * 4..(y * WIDTH + x) * 4 + 3]
    }
    #[test]
    fn crossing_faces_occlude_per_pixel_in_either_submission_order() {
        // The red plane is nearer on the left, farther on the right.
        // A single average-depth sort cannot produce both visible halves.
        let sloped = [
            [100., 100., 0.1],
            [300., 100., 0.01],
            [300., 300., 0.01],
            [100., 300., 0.1],
        ];
        let flat = [
            [100., 100., 0.05],
            [300., 100., 0.05],
            [300., 300., 0.05],
            [100., 300., 0.05],
        ];
        let mut a = Frame::default();
        let mut b = Frame::default();
        a.clear([0; 3]);
        b.clear([0; 3]);
        a.depth_polygon(&sloped, [255, 0, 0]);
        a.depth_polygon(&flat, [0, 0, 255]);
        b.depth_polygon(&flat, [0, 0, 255]);
        b.depth_polygon(&sloped, [255, 0, 0]);
        assert_eq!(a.rgba, b.rgba);
        assert_eq!(rgb(&a, 120, 200), [255, 0, 0]);
        assert_eq!(rgb(&a, 280, 200), [0, 0, 255]);
    }
    #[test]
    fn concave_faces_keep_their_notches_and_effects_obey_depth() {
        let mut f = Frame::default();
        f.clear([0; 3]);
        f.depth_polygon(
            &[
                [10., 10., 0.1],
                [90., 10., 0.1],
                [90., 90., 0.1],
                [60., 90., 0.1],
                [60., 40., 0.1],
                [40., 40., 0.1],
                [40., 90., 0.1],
                [10., 90., 0.1],
            ],
            [10, 20, 30],
        );
        assert_eq!(rgb(&f, 50, 60), [0, 0, 0]);
        assert_eq!(rgb(&f, 20, 60), [10, 20, 30]);
        f.depth_line([0., 60., 0.05], [100., 60., 0.05], [255, 255, 0]);
        assert_eq!(rgb(&f, 20, 60), [10, 20, 30]);
        assert_eq!(rgb(&f, 50, 60), [255, 255, 0]);
        f.depth_line([0., 60., 0.2], [100., 60., 0.2], [255, 0, 255]);
        assert_eq!(rgb(&f, 20, 60), [255, 0, 255]);
        f.clear([0; 3]);
        f.depth_polygon(
            &[
                [0., 0., 0.01],
                [100., 0., 0.01],
                [100., 100., 0.01],
                [0., 100., 0.01],
            ],
            [1, 2, 3],
        );
        assert_eq!(rgb(&f, 20, 60), [1, 2, 3], "depth clears between frames");
    }
    #[test]
    fn depth_raster_clips_large_spans_and_rejects_nonfinite_values() {
        let mut f = Frame::default();
        f.clear([0; 3]);
        f.depth_polygon(
            &[
                [-10000., -10000., 0.01],
                [10000., -10000., 0.01],
                [10000., 10000., 0.01],
                [-10000., 10000., 0.01],
            ],
            [4, 5, 6],
        );
        assert_eq!(rgb(&f, 799, 599), [4, 5, 6]);
        f.depth_line([-1e6, 300., 0.1], [1e6, 300., 0.1], [1, 2, 3]);
        assert_eq!(rgb(&f, 400, 300), [1, 2, 3]);
        f.depth_polygon(
            &[[0., 0., f32::NAN], [100., 0., 1.], [0., 100., 1.]],
            [255; 3],
        );
        assert_eq!(rgb(&f, 10, 10), [4, 5, 6]);
    }
    #[test]
    fn clips_faces_crossing_camera_without_projecting_behind_it() {
        let clipped = near_clip(&[[-2., 0., -1.], [2., 0., 3.], [0., 2., 3.]]);
        assert_eq!(clipped.len(), 4);
        assert!(clipped.iter().all(|p| p[2] >= 1.));
        assert!(clipped.contains(&[0., 0., 1.]));
        assert!(clipped.contains(&[-1., 1., 1.]));
        assert!(near_clip(&[[0., 0., -2.], [1., 0., 0.], [0., 1., 0.]]).is_empty());
        let mut frame = Frame::default();
        frame.clear([0, 0, 0]);
        frame.polygon(
            &[[-10000., -10000.], [10000., -10000.], [0., 10000.]],
            [12, 34, 56],
        );
        assert_eq!(
            &frame.rgba[(300 * WIDTH + 400) * 4..(300 * WIDTH + 400) * 4 + 4],
            &[12, 34, 56, 255]
        );
    }
    #[test]
    fn clips_lines_and_preserves_alpha() {
        let mut f = Frame::default();
        f.clear([0, 0, 0]);
        f.line([-1e6, 300.], [1e6, 300.], [1, 2, 3]);
        assert_eq!(
            &f.rgba[300 * WIDTH * 4..300 * WIDTH * 4 + 4],
            &[1, 2, 3, 255]
        );
        assert_eq!(&f.rgba[0..4], &[0, 0, 0, 255]);
        f.line([f32::NAN, 0.], [0., 0.], [255, 0, 0]);
    }
}
