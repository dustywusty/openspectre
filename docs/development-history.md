# Development history and recovery reference

This is the earlier engineering log. Some progress notes describe intermediate implementations. See the [README](../README.md) for current setup.

# OpenSpectre

OpenSpectre is a native Rust tank game for browsers and desktop windows, inspired by Spectre VR.
The shared simulation runs through WebAssembly in the browser. It needs no Mac ROM, System 7 runtime, or CPU interpreter.

The bundled **Signal Frontier** pack works without a disc. It includes:

- Three zones and 12 new arenas.
- 53 model entries per zone, including tanks, scenery, supplies, projectiles, and animation frames.
- New palettes, radar symbols, enemy settings, weapon tables, and campaign sequences.
- 48 synthesized sound effects.
- First-person, chase, and overhead views, single-player combat, and two-player online deathmatch.

Classic mode loads original assets from your own Spectre VR/CD image. Original assets are not included in this repository.
Movement uses recovered routines. Collision and parts of the original behavior remain provisional.
The bundled pack has its own solid renderer. Classic mode retains the original projection research path.

## Play without a disc

Install Rust 1.88 or later, Node.js 22 or later, and Python 3. Then run:

```sh
rustup target add wasm32-unknown-unknown
npm ci
npm run build
npm start
```

Open <http://localhost:5174/#drive>. Click **Drive tank**.
The generated replacement assets are included. `npm run build` regenerates them from source and compiles the WASM module.
Use `npm run build:wasm` to rebuild only the engine.

See [asset pack authoring](asset-packs.md) for model parameters, arena coordinates, sound generation, and reproducibility checks.

## Play with classic disc assets

On macOS, install hfsutils. Then extract your own disc and export its sound:

```sh
brew install hfsutils
python3 scripts/extract-disc.py /absolute/path/to/SpectreVRCD_1_1.iso
npm run build:audio
npm run start:classic
```

Build the WASM module first, as described above. Stop an existing server before changing modes, then refresh the browser.
`npm run start:classic` checks that the required assets exist. It does not silently substitute replacement assets.
`npm start` selects the bundled pack by default. The `SPECTRE_ASSETS=classic` environment setting also selects classic mode.

The extraction script reads a working copy and checks that the source ISO hash stays unchanged.
Extracted resources, sound, and research output remain in the ignored `.local/` directory.
The server serves only the selected pack. A server uses the same pack for every multiplayer participant.

For the original Macintosh executable, use the separate [emulator reference](#original-game-reference).

## Playable encounter

Open <http://localhost:5174/#drive>. Click **Drive tank**.

- Use WASD or the arrow keys to drive and steer.
- Hold Space to fire. Hold Shift to brake.
- Press Q or use **Select weapon** to cycle unlocked optional weapons. Press E or **Fire Scattershot** when Scattershot is selected.
- Press R to reset. Press Escape to pause.
- Click **Fullscreen** or press F while the canvas has focus. Press F again or Escape to leave fullscreen.
- Use the buttons below the canvas for touch controls.

The browser pauses when the driving canvas loses focus or the tab becomes hidden.
Click **Drive tank** to resume. Select an arena to reset into another original layout.

For the same simulation in a desktop window:

```sh
cargo run --locked -p spectre-tools --features desktop -- drive .local/extracted/central.bin
```

Campaign loads now retain the original optional-weapon availability and starting selection in shared Rust state.
Unlocks accumulate from wave hints through the selected level. Reset restores that starting loadout.
Q selects an unlocked optional weapon with the original repeat delay and change sound. E fires Scattershot when selected; other optional weapon effects remain unfinished.
Central level 11 at difficulty 0 starts with Scattershot. A full burst uses three rounds from the owned weapon table.

Flag mission is the browser default. Type-7 and type-8 enemies pursue you and fire aimed shots.
Use scenery for cover. Destroy the escorts to expose protected type-31 targets.
The round ends in victory or hull destruction. Empty ammunition prevents firing but does not end the round. Press R to restart, then click **Drive tank**.
The hull has six points. The balanced preset supplies 40 rounds.
Select **Stationary research** to use the earlier target practice mode and manual type-31 event control.

Enemy AI, hull damage, and encounter objectives are provisional. Moving acid and mud also use recovered phase and pursuit rules. Other enemy types remain stationary.
Type-8 enemies use recovered predictive aiming and skip world-object selection during initialization.
The prediction uses the player’s stored speed and heading; the direction table retains its modeled-SANE limitation.
Invalid headings or original division-fault inputs fall back to the current target pose.
Type-7 firing now uses recovered timer, distance, and arc checks with original resource parameters.
Type-7 translation and turning use recovered arithmetic and original speed parameters with a candidate direction table.
Later enemy slots yield to qualifying earlier enemies within the recovered sixty-unit threshold.
Type-7 steering uses recovered heading search, movement policies, and cached aiming.
Type-7 intent now uses recovered initialization, object selection, guarding, and attack transitions.
Enemies can guard eligible objects and retain combat mode after the player leaves attack range.
With more than five enemies remaining, the recovered scheduler skips commands beyond its default 500-unit distance limit.
Runtime geometry, participant allocation, Random generation, collision adapters, and OS angle conversion remain provisional.
Scenery stops projectiles but does not suppress enemy firing requests.
Enemy fire state retains the resource weapon ID. Unarmed enemies emit no shots; Scattershot emits center and two spread requests.
Spread arithmetic is recovered, but the random stream remains provisional.
Enemy weapon 6 uses recovered launch height, lifetime, and bouncing motion, with height shown in first-person and chase views.
Its collision sampling and damage remain provisional. Other special weapons still use basic preview projectiles.
Models, static placements, enemy types, base health, and sounds come from the original disc.
Handling, projectile motion, and type-31 vulnerability use recovered routines with documented limitations.
Player shots use the recovered collision query where model slots are available; other arenas use preview bounds.
Spawn positions, camera, colors, enemy hit tests, and campaign progression remain provisional.
Static position tests establish decoder consistency; they do not prove complete original map behavior.
See [disassembly notes](disassembly.md) for the original world-loader evidence and remaining uncertainties.

Run `npm run test:encounter` to check combat behavior and native/WASM replay parity.
Player death now runs the recovered 260-tick transition before respawn or final loss. Controls and firing stop while the world continues. First-person and chase views use the recovered death camera. Overhead view spins the tank and arena, with a gradual zoom out. Its full projection remains provisional.
Player death resets active enemy behavior, movement mode, and firing timers through the recovered notification path. Tracking data survives this reset; enemies can return to flag guarding.
The desktop drive command uses the same encounter simulation. Desktop sound output is not implemented.

The single-player **Infinite health** checkbox prevents hull loss from enemy fire and acid. It is off on each page load.
Hits still register. Turning it off restores normal damage.

**Enemy respawn** is on by default in the browser and desktop clients. Destroyed mission enemies return indefinitely and never retire, so the last-enemy bonus is not awarded. Turning it off restores the original per-arena rebirth limits, which can retire enemies. The browser saves the choice in local storage. The desktop client saves it in `profile.json`. WASM hosts call `game_enemy_respawn(0|1)`.

## Multiplayer

Select **Multiplayer**, create a room, and share the invitation link. The second player joins automatically.
Local movement and shots use prediction. The server controls damage and scores.
See [multiplayer setup and rules](multiplayer.md) for public links and connection details.

For the bundled pack in a native window:

```sh
cargo run --locked -p spectre-tools --features desktop -- drive assets/open/generated/central.bin --colors assets/open/generated/spectre.bin
```

## Native desktop viewer

```sh
cargo run --locked -p spectre-tools --features desktop -- view .local/extracted/central.bin
```

Press `[` or `]` to select a resource. Use the arrow keys to rotate a model.
Press `+` or `-` to zoom. Press Space to change the fill mode. Press Escape to close the viewer.
The desktop viewer uses the same Rust decoder and renderer as the browser.
Only macOS has passed a window smoke test so far.

## Inspect and export resources

```sh
cargo run --locked -p spectre-tools -- inspect .local/extracted/central.bin .local/central-inventory.json
cargo run --locked -p spectre-tools -- export .local/extracted/spectre.bin .local/game-resources
cargo run --locked -p spectre-tools -- export .local/extracted/central.bin .local/central-resources
```

Reports include resource IDs, names, byte offsets, sizes, and decoded data.
Export also writes each raw resource and an OBJ file for each decoded model.
Resource type directories use hexadecimal bytes, so resource names cannot create unsafe paths.
Non-ASCII name bytes remain explicit hexadecimal escapes until Mac Roman text conversion is implemented.

## Native validation

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
npm run build:wasm
npm run test:open
npm run test:open-network
```

Classic recovery checks such as `npm run test:assets` require your extracted CD. It renders all 358 model resources and 112 map resources across the three campaigns.
Shared resources count once per campaign. It also compares the native and WASM Tank 1 reference frames pixel by pixel.
The WASM module has no runtime imports.

See [native port research](native-port.md) for verified formats, unresolved behavior, and the next milestones.

## Original-code recovery

See [disassembly research](disassembly.md) for recovered routines, exact code offsets, and reproducible original-machine-code comparisons.
The browser uses recovered type-6 speed, turn ramp, and planar translation with the balanced preset.
Its direction table awaits original-SANE validation. Collision, scheduling, and special movement effects remain unfinished.

## Original game reference

This local launcher runs the original Macintosh game through Infinite Mac and its Basilisk II WebAssembly emulator.
The server reads your CD image without changing it. It downloads emulator and system files from Infinite Mac.

### Start the emulator reference

Use Node.js 22 or later. No npm dependencies are required.

```sh
npm run start:emulator
```

Open <http://localhost:5173>. Click **Start Macintosh**.

The default CD path is `~/SpectreVRCD_1_1.iso`.
For another CD path or port, run:

```sh
SPECTRE_ISO="/absolute/path/to/game.iso" PORT=5173 npm run start:emulator
```

### Open the original game

1. Wait for the System 7.5.3 desktop.
2. Open the **Spectre VR/CD** disk.
3. Open the **Spectre VR CD** folder.
4. Open the **Spectre VR CD** application.
5. Enter your name and the serial number printed on your disk.

The game currently starts from the CD and reaches its registration dialog.
Gameplay, sound output, and multiplayer still require validation after registration.

### Emulator storage

The CD image remains read-only. Saved HD uses browser storage at the localhost origin.
Files on the system disk do not have the same persistence guarantee.
The game’s registration and preferences persistence still require validation.

For persistent game files, copy the game folder to **Saved HD** in the emulated Finder.
Return with the same browser profile and localhost port.
Before you close the tab, quit the game and select **Special → Shut Down** in the Mac.

### Emulator server tests

```sh
npm test
```

The tests use a temporary disk fixture. They check disk bytes, HTTP ranges, read-only access, the CD manifest, and isolation headers.

### Emulator implementation

`server.mjs` binds to loopback. It serves one configured CD image through HTTP byte ranges.
It supplies the CD manifest locally and proxies emulator requests to `https://infinitemac.org`.
The CD contents never go to that upstream server. Local telemetry requests receive an empty response.

`index.html` embeds the emulator and provides pause and full-screen controls.
Cross-origin isolation enables SharedArrayBuffer support. The emulator runs a Quadra 650 with System 7.5.3.

This prototype needs internet access and depends on the hosted Infinite Mac build.
It is not a standalone emulator distribution. A future version can pin and host the emulator assets.
Multiplayer is not configured.

### Emulator credits

The emulator comes from [Infinite Mac](https://github.com/mihaip/infinite-mac) and Basilisk II.
Infinite Mac provides the [embedding interface](https://infinitemac.org/embed-docs).
This repository contains neither the game nor copies of the emulator or operating system.

Original sound effects are decoded locally from the disc with `npm run build:audio`.
Click **Drive tank** to enable browser audio. The Sound controls set volume or mute playback.
The current build plays player and enemy firing, player and enemy hits, explosions, collision, and empty-ammo effects.
The recovered player ammo rule also triggers sample 10010 when a shot reduces ammo from above five to five or fewer.
Player and enemy firing use the original fresh-session limit of twenty active shots per owner. Rejected player launches consume no ammo.
Weapon firing uses the recovered constructor sound table, including silent Scattershot spread requests.
Player damage from enemy shots now selects the recovered nonfatal, respawn-death, or final-death sample without an extra hit sound on death.
Enemy hits use the recovered nonfatal-or-death sample selection in both collision paths. Protected type-31 hits retain nonfatal feedback.
Other combat effects, music, and the original sound-channel scheduler remain pending.
Run `npm run test:audio` after building audio and WASM to check sample preservation and sound events.


The first-person renderer now uses per-pixel reciprocal depth for solid faces, shots, and impact effects.
This removes average-distance sorting errors when surfaces overlap. Concave faces retain their openings.
The renderer remains a modern implementation; the original visibility algorithm is not yet recovered.
Fullscreen keeps the 4:3 view letterboxed and displays the combat HUD.
Browser fullscreen requests can be restricted by embedded browser hosts.

The driving canvas supports three views: first person, diagnostic chase, and overhead map.
Tab or keypad * cycles views; +/− zooms chase and map views. Shift-Tab leaves the browser canvas.
The map uses original top-view lines and authored Plane Sides polygon fills where available, with projected mesh edges as fallback.
Projection, colors, zoom steps, and north-up orientation remain provisional. Desktop uses the same view controls and Rust renderer.

Ice (kind 17) and static Cybermud (kind 18) now affect player handling through the recovered participant contact query.
The HUD reports `ice`, `mud`, `flow`, `none`, or `unavailable`. Ice selects adjustment -6; mud uses the original default adjustment 5.
These surfaces become passable when contact geometry is available. Missing geometry retains the previous bounds behavior.
Contact sampling runs at the start of each preview tick; original scheduling, mud-strength overrides, surface sounds, original solid-response ordering, and moving mud remain unfinished.

Flow Fields (kind 39) now apply recovered extra displacement using the six-times-unit direction table, field heading minus 90 degrees, and division by two.
Contact starts with one tick of push and clears on exit/reset. The preview rejects pushes into solids; original spatial-update ordering remains unrecovered.
Direction-table values still use modeled SANE, and the adapter's heading conversion remains provisional.


The first-person ground pattern now uses the recovered 64-division lattice over the 1024-unit arena.
Every eighth row has points at 16-unit intervals. Other rows contain only every eighth column.
The live renderer also uses recovered visible-row selection, culling, and integer ground projection.
Ground points and solid objects share the modeled original direction table and fixed camera position.
First-person rendering uses the original 256-pixel projection with compact immediate-display framing on a 640-by-480 reference screen.
The host scales that screen uniformly to 800 by 600. Direction-table generation still uses modeled SANE.


The browser loads shared color resources from the extracted executable before loading a campaign.
Campaign clut resources override matching executable resources. Ground dots use the composed palette color.
Palette assembly uses the recovered eight-bit, 13-color path. Authored solid faces use recovered face selectors, packed-vertex normals, lighting arithmetic, 192-color ramps, and screen-aligned dithering. Procedural tetrahedrons, selector-three columns, filled flag faces, and the six-face bodies of selectors 1 and 5 use original packed vertices, verified face winding, and builder-selected kind colors. Other procedural faces, line decorations, and movie/picture materials remain provisional.
The ordinary-object adapter uses kind-primary colors and world rotation. Direction tables and vertex rounding retain their modeled OS limitations. Dynamic flashes and object-specific bright/fade flags are not yet connected.
The RGBA adapter uses each original channel's high byte; original display conversion remains unfinished.

To load the same shared colors in the desktop preview:

```sh
cargo run --locked -p spectre-tools --features desktop -- drive .local/extracted/central.bin --colors .local/extracted/spectre.bin
```

Run `npm run test:palette` after building the native tool and WASM module.
This checks campaign overrides, level colors, reload, reset, native/WASM agreement, and visible red shading on an authored tower and yellow shading on a procedural tetrahedron.

First-person solid object positions now use the recovered camera-relative rounding and planar rotation from CODE 6.
This includes the death camera and the original indexed direction-table layout, with modeled SANE values.
Shapes now retain original packed vertices. First-person meshes with a complete packed vertex list use recovered integer rotation and translation.
Scenery uses its authored heading; moving targets convert their simulation heading back to native degrees. Lighting uses the same native heading.
Packed solid faces now use original near clipping and integer projected coordinates, with clipped depths retained for the depth buffer.
Packed two-point model edges also use original near clipping and integer projection.
Shots, impacts, and unsupported meshes share this framing but retain preview geometry and projection arithmetic.
This is a partial integration of the original projection pipeline, not a complete original renderer.

Scenery collision records, driving footprints, fallback projectile bounds, and spawn/AI obstacle poses now use authored native headings.
They no longer apply the participant heading conversion to scenery. Flow Fields therefore push according to their authored direction.
Collision still uses the documented mixture of recovered queries and provisional footprint responses; this fixes orientation, not all collision fidelity.

Exterior and overhead views now use the same native scenery orientation as first-person rendering and collision.
Their camera placement, colors, and projection remain diagnostic.

First-person solid visibility now applies the original forward-depth and squared-distance limits after the horizontal origin gate.
The adapter recomputes distance per object; original type-specific suppression and cached-state scheduling remain unfinished.

First-person faces with recovered plane records now use the original model-space visibility test before clipping and rasterization.
Unsupported faces retain diagnostic behavior. Blue pickups now enable player flight using recovered energy and altitude rules. Enemy object heights and several special vertical states remain unfinished.

The first-person horizon now uses recovered shaded-band drawing requests and the original horizon color binding.
Its color follows the campaign palette, including the later green horizons seen in the reference video.
The live adapter selects ordinary eight-bit shading and the original default horizon level 32; alternate horizon states remain pending.

First-person aiming marks now use the original three-segment geometry and bound palette color.
The horizontal marks lie on the horizon. Compact surface clipping also applies to geometry, ground points, and horizon bands.
Full-display mode selection and original instrument-panel rendering remain pending.
The video differs by one source pixel under the assumed screen settings; no correction is inserted for that unresolved difference.

The shared executable-resource loader also decodes the original 41-symbol radar atlas from PICT 1000.
Desktop and WASM retain those symbols across campaign loads and resets.
First-person radar now shows the player marker and contacts for ten recovered enemy types, using original glyphs, colors, and distance scaling.
Type-31 contacts appear only at the final animation frame. Remaining object branches, some visibility gates, and exterior radar remain unfinished.

First-person radar also uses recovered markers for teleporters, underground entrances/exits, and seven surface or supply types.
The original branches suppress these markers in overhead view and for radar color 13.

Single-player flag contacts now use original glyph 40 and the recovered 25-tick flashing deadline.
Collected flags lose their marker. The fixed-step loop drives flashing; original display scheduling and multiplayer flag rules remain pending.

Radar pixels now follow the original ascending color-queue order, including overlapping contacts.
Complete original glyph emission matches Rust, including timed random disturbance. Low-health hits now activate disturbance in first-person radar.

Recovered radar disturbance activation: accepted local hits at health 2 or less start a selected-difficulty-dependent deadline.
The live damage adapter activates the recovered display state and plays warning sample 10009.

Radar disturbance uses a provisional deterministic display-only random stream. It does not consume movement or spawn randomness.
The first emitted contact at the deadline still receives jitter; subsequent contacts are stable. Respawn and arena reset restore stability.
Original shared Random behavior, complete sound queue controls, exterior radar, and object emission order remain unfinished.

Recovered the moving acid/mud phase transition and complete cooldown handler, including distance-dependent acid sounds.
These verified AI helpers are not yet connected to live surface enemies; tracking, movement, and contact damage remain pending.

Surface-enemy mode-zero initialization and complete target tracking now match original instruction runs.
Their movement shares the recovered type-7 arithmetic branch; live activation and surface contact responses remain pending.

Moving acid (kind 23) and mud (kind 24) now initialize, track, pause, and pursue in the live Rust simulation.
Their original per-kind parameters drive shared movement arithmetic. Acid phase sounds and moving A/M radar contacts are connected.
Authored WASM checks exercise Central levels 12 and 13. Moving mud slows the player; static and moving acid apply recovered contact damage requests.

Moving mud uses the original static-mud slowdown handler and is passable when recovered contact geometry is available.
The complete acid-contact request handler is recovered, including its strict 150-tick deadline and source ownership.
Live acid damage uses the wave's kind-23 collision-damage parameter and the recovered strict contact cooldown.

Static and moving acid now share player damage counters, low-health radar disturbance, sounds, and the normal death transition.
Contact detection still depends on available recovered geometry. Original contact ordering, event parameter overrides, and network damage remain unfinished.

Ice and static/moving mud now emit their original contact samples with recovered cooldown rules.
Ordinary movement resets both sound clocks through the original speed-update condition. Mud-strength event overrides remain pending.


Blue flight pickups now lift the player automatically. Slowing down raises the target altitude; accelerating lowers it.
Energy depletion causes gradual descent. The browser status line shows energy and altitude.
Cockpit and chase views, player shots, movement contacts, pickup effects, and respawn state share the player altitude.
Run `npm run test:flight` to drive to a blue pickup, take off, fire, land, and reset in WASM.
Original update scheduling, complete landing response, special flight weapons, and complete submergence interactions remain unfinished.

Ordinary solid collisions now use recovered negative-time movement and rebound when model geometry is available and coast/surface adjustment are zero.
Airborne contacts can hold altitude for 30 updates; the hold suppresses repeated ordinary rollback and sound.
Special-contact handlers, unsupported geometry, and other handling states retain the previous adapter behavior.

Original down/up transit devices now submerge the tank and return it to the surface where recovered contact geometry is available.
Down transits clear flight energy. Up transits activate below ground.
Run `npm run test:transits` for a driven WASM check using original Central 10110 placements.


Seekers are now playable with E or the optional-fire touch button when slot 9 is selected.
Central campaign level 13 at difficulty 0 unlocks them. Q cycles available weapons.
Each shot uses the resource ammunition cost, two rounds in Central, and sound 10028.
During flight, Seekers use recovered projectile-based target selection and one-third integer-angle steering.
The participant adapter uses the player plus eleven stable enemy slots. Dead slots keep their identity.
Angle conversion, direction tables, collision sampling, and scheduling retain the existing prototype limits.
`npm run test:seekers` checks campaign firing, ammunition, sound, reset, and native/WASM replay parity.


Smart Missiles are now playable with E or the optional-fire touch button when slot 10 is selected.
Central campaign level 7 at difficulty 0 unlocks them. Q cycles available weapons.
The owned Central weapon table specifies six ammunition units and three damage points per missile.
They use sound 10027, the recovered launch initializer, and a 300-tick lifetime with the current 75-tick base.
Missiles climb before homing. Guidance, vertical movement, horizontal movement, and collision filtering use recovered routines.
Enemy target heights remain zero in the current participant adapter. Full flying-enemy height support remains unfinished.
The shared renderer uses original shape 10001 for missiles in all three views, with the old marker as a missing-model fallback.
`npm run test:missiles` checks campaign firing, resource costs, launch state, sound, reset, and native/WASM parity.


Grenades are playable with E or touch when slot 1 is selected.
Central level 3 at difficulty 0 unlocks them. Each launch costs ten rounds and uses sound 10024.
Their current base lifetime produces 54 flight ticks. Timer expiry retires the shot, then applies an ownerless blast four ticks later.
A living-enemy impact instead applies an immediate blast with launch ownership intact.
Blast damage uses the recovered target-based table lookup and range calculation. Type-31 protection remains active.
Timed blasts do not add player-hit or kill-score credit. Impact blasts can award that credit.
Blast appearance, scenery-contact response, complete death effects, and scheduling remain provisional.
Cyberbombs are still unavailable. `npm run test:grenades` checks Central firing and native/WASM parity.
