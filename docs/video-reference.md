# Gameplay video reference: 4:00–5:00

See [full recording review](video-full-review.md) for the expanded 21:34 reference and timestamped observations.

Source: [user-supplied gameplay video](https://www.youtube.com/watch?v=b0X74Oe80tg&t=240s).
Inspected on 2026-11-07. The video edition and settings are not confirmed against the local CD.

The local clip is `.local/reference-video/spectre-4m-5m.mp4`.
It contains the requested minute at 640×480 and 30 frames per second.
Twelve snapshots cover 4:00 through 4:55 at five-second intervals.
`frame-01.png` corresponds to 4:00; `frame-12.png` corresponds to 4:55.
`contact-sheet.jpg` contains these snapshots in row order.
`combat-sheet.jpg` samples 4:25–4:33 at two frames per second.
The local `index.html` provides the clip and snapshot gallery.
These reference files stay in the ignored `.local` directory.

## Visible evidence

| Time | Observation | Port implication |
| --- | --- | --- |
| 4:00–4:05 | Low first-person view, centered white aiming marks, and no visible player tank body. | The current elevated chase camera is a diagnostic view. Add an original-style first-person view. |
| 4:00, 4:25 | Black ground with sparse white dots, a magenta horizon band, and solid colored geometry. | Recover projection, polygon fills, colors, and background drawing. Wire outlines alone cannot match this presentation. |
| 4:10 | A results screen shows QUICKNESS, ACCURACY, and AVOIDANCE ratings. | Level completion includes a separate results state. Rating formulas remain unknown. |
| 4:15–4:20 | LEVEL 5 appears with SPINNERS KEEP ENEMIES BUSY. | Level introduction and hints belong to the original flow. This text does not establish spinner behavior. |
| 4:25 | A red column, blue flag, and orange ground polygons occupy the same scene. | Preserve object colors, flags, and ground features in addition to recovered meshes. |
| 4:25–4:33 | Scenery crosses the screen during turns. Ammo decreases during combat; damage and score also change. | Combat must retain aiming feedback and live HUD counters. Inputs and exact physics cannot be derived from this footage alone. |
| Around 4:30–4:31 | A nearby green polygonal object is followed by orange fragments and a higher score. | This suggests a destruction effect. Object identity and the precise damage rule require code evidence. |
| 4:55 | Large orange triangular forms appear above the horizon. | Effects or objects can occupy substantial screen space. Their identity remains unconfirmed. |

The HUD includes lives, damage, ammo, score, level, time, two vertical meters, and a weapon icon.
A small cluster of colored dots appears above the right side of the horizon, consistent with a radar display.
Its mapping, range, and orientation remain unconfirmed.
The horizon stays approximately level during the sampled combat sequence.

## Next rendering targets

1. Add a first-person camera while retaining the diagnostic camera for development.
2. Recover camera height, projection scale, viewport limits, and clipping from the executable.
3. Recover solid face colors and the horizon and ground drawing routines.
4. Add HUD state, radar, and destruction effects with their original update rules.

Video supplies visual comparisons, not recovered constants.
This minute does not establish random coasting, exact acceleration, collision sizes, or type-31 vulnerability timing.
Those claims still require instruction traces or controlled original-game observations.

## First-person prototype implemented

The default driving view now uses a level first-person camera and solid recovered meshes.
The View selector also provides the diagnostic chase camera.
Both views use the same Rust simulation and work on desktop and WASM.
The browser exposes the selector; native callers can set `Viewer::chase_camera`.

Camera height is four world units, focal length is 500 pixels, and the horizon is at pixel 360 of 600.
These are provisional presentation choices, not recovered constants.
The renderer clips faces at the near plane and now uses per-pixel reciprocal depth.
The earlier average-depth sorting was replaced after reports of see-through geometry.
Colors, ground dots, horizon gradient, and impact marks remain provisional.
HUD reconstruction, radar, and original visibility rules remain pending.

All 112 arenas pass the render and gameplay checks.
The checks also compare simulation state before and after camera changes.
Desktop and WASM 240-tick replays still match within 0.001 units.
