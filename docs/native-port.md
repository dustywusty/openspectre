# Native port research

The target is a Rust implementation of Spectre VR/CD 1.1 for WASM and current desktop operating systems.
The emulator remains a behavioral reference. The native renderer does not call the emulator.

## Evidence

The supplied image has SHA-256:

```text
f5104eb04344a9c81e8f24ec9e3eaba1a3f8d3389d0104925a3df6a9ed63ed56
```

The HFS volume is `Spectre VR/CD`. The application has creator code `Spe2` and 654 resources.
Its 37 `CODE` resources include the segment jump table in `CODE 0`.
The disk also contains VRchitect, three campaign matrices, textures, and movies.

The extraction script records source and output hashes in `.local/extracted/provenance.json`.
The original disk is unchanged. Generated data and code resources are excluded from Git.

## Decoded campaign data

| File | Models (`shap`) | Layouts (`wrld`) | Enemy tables (`enem`) | Parameter tables (`bval`) | Campaign sequences (`wave`) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Central Systems Matrix | 119 | 30 | 46 | 6 | 3 |
| Eastern Edge Matrix | 120 | 52 | 61 | 10 | 3 |
| South Central Matrix | 119 | 30 | 46 | 6 | 3 |

These are resource counts, not unique models or playable level counts. Campaigns share data and can reuse layouts across waves.
The current decoders consume every byte of each supported resource. None of these resources failed decoding.

## Verified layouts

The campaign files include ResEdit `TMPL` resources. These templates describe the field layouts directly.

| Type | Verified layout |
| --- | --- |
| `shap` | Big-endian signed 16.16 XYZ vertices, polygon indices and color values, radar vertices and lines, final `Plane Sides` word |
| `wrld` | Count, then four signed words per placement: object type, vertical position, horizontal position, facing |
| `wave` | Count, then eight signed words: world ID, enemy-table ID, hint index, parameter-table ID, two movie indices, song ID, colors ID |
| `enem` | Count, then four signed words: enemy type, minimum, maximum, rebirth count |
| `bval` | Count, then 28 bytes per entry: type, health, collision damage, ranges, arc, firing interval, velocities, scores, weapon |
| `wval` | Count, then three signed words: weapon type, damage, ammunition cost |
| `TMPL` | Repeated Pascal-string labels and four-byte field types |

The shape lists use `ZCNT`, which stores count minus one. A value of `0xffff` represents an empty list.
Polygon and radar indices are one-based. Other supported campaign tables use direct counts (`OCNT`).
An older `wrld` template also exists. The CD layouts match the newer direct-count format.

Tank 1 is `shap 10010`. It contains 14 vertices and 13 faces.
The final shape word has values including `0`, `-1`, `3`, `5`, and `8`.
Its rendering semantics remain unresolved. The inspection renderer retains this value but does not use it.

## Boundaries of current work

The viewer draws the original vertices and polygon topology. Its camera, fill order, colors, and shading are new diagnostic code.
Map markers represent the original placement fields. They do not resolve every object type to its final 3D form.
The parameter names come from the templates. Their units and runtime effects still require code analysis and behavioral tests.

The native executable presented frames in a macOS window. The same Tank 1 frame matched WASM output pixel for pixel.
The corpus check rendered all 470 campaign model and layout resources without a trap.
The WASM binary has zero imports and uses neither an interpreter nor a compatibility layer.

## Driving prototype

The shared Rust simulation now runs at 60 ticks per second on desktop and WASM.
The hosts supply elapsed time and input. An accumulator advances physics in fixed steps.
A frame accepts at most 100 milliseconds. Extra time after a stall is discarded.
The browser pauses on focus loss and clears held controls.

The prototype preserves original world placement coordinates and Tank 1 vertices.
Visible placements use original selected models where supported. Collision uses their oriented bounds.
Zero-shape placements are omitted. Unsupported models retain explicit diagnostic markers.
A circular tank collider has a 22-unit radius. Arena bounds are 0–1024.
Spawn selection finds a clear point near the center.

The tank now uses recovered type-6 speed updates, turning ramps, and planar translation.
Movement state uses fixed-point coordinates and heading, plus signed speed and ramp bytes.
The balanced preset sets speed limit 5 and Coast 0. Both directions reach 75 displayed units per second.
Releasing movement stops it immediately. Shift forces the movement command to zero as a preview convenience.
The renderer converts original heading 90 to its own forward heading zero.

The direction table is a candidate generated from modeled SANE services.
The preview explicitly selects the full turning rate and advances in single-tick steps.
Original machine-dependent halving, grouped catch-up steps, and special movement effects are not reproduced here.
Collision still uses model bounds and resets speed on contact. That response remains provisional.

Primary fire attempts require more than ten ticks since the previous attempt.
At one tick per update, held fire attempts a shot every eleven ticks.
An empty-ammo attempt also resets this timer. Reset restores the initial eleven-tick delay.
Projectiles use the recovered six-unit launch offset and six-unit-per-tick direction table.
The lifetime is 75 ticks. Expiry uses the recovered timestamp comparison before movement.
A new shot has zero elapsed time at its launch timestamp. Collision remains provisional.
One-unit collision substeps stop shots at model bounds and arena boundaries.
Scenery impacts remain visible for 18 ticks. Target impacts remain visible for 36 ticks.
Targets use original wave types, minimum counts, and base health. Preview positions remain provisional.
Basic shots use weapon-0 resource damage and recovered health subtraction. Destroyed targets disappear and reset restores them.
Type-31 targets use the recovered shared animation transition and final-frame damage guard.
**Replay enemy unlock event** manually activates the transition. Automatic movie scheduling remains unfinished.
Other animated models show their original first frame. Enemy AI and remaining special damage rules remain unfinished.

Fourteen Rust tests pass, including frame-rate independence, braking, collision, and firing.
All 112 recovered layouts pass spawn and render checks.
Desktop and WASM replays agree within 0.001 coordinate units after 240 driving and firing ticks.
Floating-point trigonometry can differ across targets. This is not a network lockstep guarantee.
The macOS driving window passed a three-frame presentation test.

## Disassembly progress

The jump table, four mathematical or heading routines, speed update, and normal health subtraction are recovered.
See [original executable recovery](disassembly.md) for 21,705 differential cases and unresolved movement paths.
Health subtraction and recovered movement components run in the preview. Direction-table identity, collision, and full scheduling remain unfinished.

## Next milestones

1. Map the `CODE 0` jump table and trace the original simulation entry points.
2. Resolve `shps`, `mshp`, `rshp`, and `ashp` references, including procedural shapes and animation rules.
3. Recover camera projection, palette rules, clipping, and the original world coordinate conventions.
4. Build one navigable original level with verified player movement and collision behavior.
5. Implement shooting, enemy behavior, damage, pickups, progression, and saves against recorded reference behavior.
6. Decode sounds, music, textures, and movie assets needed by the CD edition.
7. Add modern multiplayer after the single-player simulation passes repeatable tests.

A deterministic simulation with explicit input ticks is the intended basis for multiplayer.
No network protocol or lockstep behavior is implemented yet.
The original currently stops at its serial-number dialog, so gameplay reference recordings remain unavailable.
Static analysis and asset work can continue without those recordings.

## Sources

The principal evidence is the supplied CD and its embedded templates.
The [resource_dasm Spectre decoder](https://github.com/fuzziqersoftware/resource_dasm/blob/master/src/SpriteDecoders/Spectre-shap.cc) corroborates the vertex and polygon layout.
The CD template additionally describes the trailing `Plane Sides` word, which our decoder retains.
The Rust implementation uses the format facts and does not include that project's C++ renderer.

The [2019 reverse-engineering discussion](https://www.virtual-boy.com/forums/t/spectre-vr-dev-thread/) identified the embedded templates as useful format evidence.
The [Macintosh resource-fork format notes](https://ciderpress2.com/formatdoc/ResourceFork-notes.html) describe the resource map and reference lists.
The [Rust WASM target documentation](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html) describes the browser compilation target.
The desktop window uses [minifb](https://github.com/emoon/rust_minifb).
