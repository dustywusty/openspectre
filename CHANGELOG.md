# Changelog

## Client cleanup

The client opens directly to the game. Asset tools and diagnostics are optional panels.
Progress cards and the polling activity feed are removed from the client.

## Recent work

- Added the Signal Frontier replacement pack and kept classic disc loading.
- Added CI playthrough recordings and README screenshots.
- Added a two-browser multiplayer recording with controlled latency and jitter.

## Recovery history

These entries preserve the earlier progress feed. They describe the implementation at each stage, including provisional behavior.

### 2026-11-07 — Standalone replacement pack

Added reproducible asset generation, solid full-frame views, no-disc startup, and a separate classic mode. Verified combat, campaign advancement, and multiplayer without a .local directory.

### 2026-11-07 — Recover and integrate bouncing-round impact retirement

1,024 original composed calls verify kind-29 reverse movement, vertical integration, bounce behavior, and retirement. Enemy weapon-6 wall/boundary contacts now release shot capacity and delete after four ticks. Regression checks height, velocity, position, and timing. 114 Rust tests, Clippy, and WASM build pass; collision sampling remains provisional.

### 2026-11-07 — Retire enemy cannon and Scattershot rounds

Enemy weapons 0, 3, 4, and 5 now rewind and retire on walls, immediately releasing per-owner shot capacity before fourth-tick deletion. Enemy scenery checks apply original ignored-kind rules. Regression covers full shot limits, Flow Fields, wall positions, ownership, and player health. Enemy hit geometry remains provisional.

### 2026-11-07 — Retire Smart Missiles with original reverse guidance and altitude

1,024 original retirement/movement compositions match Rust, including ascent/descent and owner-dependent elapsed adjustment. Live wall/ground contacts now rewind, retire, and delete after four ticks without area damage. Regression covers both collision paths and first-person model suppression. Original angle/table generation and compact-pool integration remain pending.

### 2026-11-07 — Retire Seekers through original reacquisition and reverse movement

1,024 complete original contact/movement compositions verify target reacquisition before reverse translation, zero-elapsed suppression, owner counts, and grid request poses. Live Seeker scenery/boundary impacts now use this sequence. Rust, Clippy, WASM, and gameplay parity pass; angle/table generation and compact-pool integration remain modeled.

### 2026-11-07 — Use original impact retirement for cannon and Scattershot

Player weapons 0, 3, 4, and 5 now rewind and retire on scenery/boundary contact, then delete four ticks later without area damage. Both collision paths verify timing and surface exclusions. 113 Rust tests, Clippy, WASM, 112-arena replays, and gameplay parity pass. Guided and enemy-shot retirement remain pending.

### 2026-11-07 — Restore retired-projectile radar markers

512 original request cases verify glyph 39, signed view admission, and the distinction between radar admission color and body drawing color. Shared radar rendering now includes retired shots. Pixel checks cover color, suppression, and removal; original compact-pool ordering remains pending.

### 2026-11-07 — Match original first-person retirement visibility

512 original kind-10 render traces reach the world-model exclusion while preserving optional radar requests. First-person retired grenade markers are now hidden. Pixel checks distinguish active markers from retired/absent shots; delayed blasts still pass. Other camera views and radar integration remain pending.

### 2026-11-07 — Retire grenades after wall impacts with original reverse movement

1,024 composed original contact/movement cases match Rust with direction-table input and grid relocation captured. Live grenade scenery/boundary contacts rewind, clear ownership, then blast four ticks later. Both collision paths pass delayed-damage tests. 113 Rust tests, Clippy, WASM build, and gameplay parity pass; original retirement visuals remain pending.

### 2026-11-07 — Preserve live shots through ignored surface contacts

Player shots now apply original object exclusions in both collision paths. Moving acid, mud, and Flow Fields preserve shots and health; walls and tanks retain their responses. 113 Rust tests, strict Clippy, WASM build, and five gameplay parity suites pass. Complete impact retirement remains pending. Hosting stays stopped.

### 2026-11-07 — Recover projectile object routing and disabled scenery damage

11,520 original routing cases match Rust across all target-kind bytes. Another 6,126 complete object-response calls confirm the shipped entry skips its damage body. Participant health gates and ignored surface kinds are preserved. Live movement/retirement composition remains pending.

### 2026-11-07 — Recover projectile boundary and ground priority

2,048 original executions match Rust with the complete boundary routine and no service stubs. Positive object hits override boundary/ground contact; ground checks use type-specific height fields. The retirement flag is now traced to SysEnvirons hardware identity. Full caller composition and live retirement remain pending.

### 2026-11-07 — Recover conditional projectile contact retirement

1,024 original mode-flag and contact-response cases match Rust. The enabled path requests reverse movement before retirement, owner-count decrement, and ownership clearing. Mode meaning and live scenery integration remain pending.

### 2026-11-07 — Make grenades playable

E and touch fire grenades in Central level 3. Timed blasts wait four ticks after retirement and lose attribution; enemy impacts blast immediately with launch credit. Target-based damage and protection tests pass, along with native/WASM parity. Blast visuals, scenery response, and complete death effects remain provisional.

### 2026-11-07 — Distinguish impact blasts from timed blasts

512 original enemy-impact cases confirm immediate grenade/bomb area damage preserves launch ownership; timed expiry clears it. Recovered the single-player/multiplayer-authority dispatch gate. Live integration remains pending.

### 2026-11-07 — Verify downstream ownerless blast damage

1,024 complete original single-player enemy damage calls match composed Rust helpers. Ownerless hits leave score and combat counters unchanged, preserve type-31 protection, and request normal death effects. Terminal effects are captured; live blast integration is still pending.

### 2026-11-07 — Verify grenade and bomb retirement through blast requests

512 multi-tick original-code sequences match Rust composition. Retirement clears ownership; the later blast uses attacker zero and target-indexed damage. Shot counters, request order, deletion order, and record preservation checked. Live weapon integration remains pending.

### 2026-11-07 — Render the original Smart Missile model

All three views now use owned shape 10001 at the missile position and height, with a missing-model marker fallback. Offline camera checks preserve projectile state; first-person frame inspected. Rust tests, Clippy, WASM build, and gameplay parity pass.

### 2026-11-07 — Make Smart Missiles playable

E and touch fire slot-10 missiles with recovered launch, climb, homing, descent, and collision filtering. Central level 7 uses six ammo, three damage, and sound 10027. Native/WASM parity and flight/damage tests pass. Flying-enemy heights and missile presentation remain provisional.

### 2026-11-07 — Recover Smart Missile collision filtering

The collision adapter now supports missile kind 34. Its post-query height and object exclusions match 2,048 original-code cases. Exact height-boundary regression, workspace tests, and strict Clippy pass. Playable missile integration remains pending.

### 2026-11-07 — Verify composed Smart Missile flight

Combined guidance and planar movement match 2,048 executions of the original code, including direction lookup over a synthetic table and signed arithmetic. Gameplay integration and collision remain pending.

### 2026-11-07 — Recover Smart Missile ascent and guidance

Rust matches 2,048 original-code executions for ascent, owner-dependent timing, target selection, turning, and height correction. Angle-service results remain supplied. Smart Missile gameplay integration is still pending.

### 2026-11-07 — Make Seekers playable

E and touch fire Seekers when selected. Projectiles use recovered target selection and steering, resource ammo cost, and original firing sound. Central level 13 native/WASM parity and live acquisition/loss tests pass. Participant mapping, OS math, scheduling, and collision remain provisional.

### 2026-11-07 — Recover original Seeker steering

Recovered weapon-9 projectile-based target selection and one-third angle correction. All 1,024 composed original-code comparisons pass; Rust tests and strict Clippy pass. Playable Seeker firing is not connected yet.

### 2026-11-07 — Make original Scattershot playable with E and touch

Scattershot now fires three original spread requests with per-shot resource ammo charges, sound, basic-projectile damage, and partial-burst limits. Central level 11/difficulty 0 WASM bursts match native replay at ticks 16/32. All 106 Rust tests, Clippy, build, encounter parity, and selection checks pass. Other optional effects and original Random remain unfinished; hosting stays stopped.

### 2026-11-07 — Recover complete local firing dispatch and share it with primary fire

Verified 2,048 original paths through eligibility, local request packing, weapon dispatch, and signed height conversion. Primary fire now uses the composed attempt result and original empty-ammo sound condition. All 104 Rust tests, Clippy, WASM build, encounter parity, weapon controls, and flight/firing checks pass. Optional firing and weapon-specific effects remain pending.

### 2026-11-07 — Connect optional-weapon selection to Q and touch controls

Fixed-step Rust selection now uses the original repeat delay and change sound. Browser Q/touch and desktop Q cycle unlocked weapons; browser status names the selection and notes that optional firing remains unavailable. WASM tick/release/redraw/audio/reset checks, 104 Rust tests, Clippy, build, encounter parity, and 476 campaign checks pass. Hosted keyboard/touch interaction remains unverified; hosting stays stopped.

### 2026-11-07 — Connect original weapon unlocks to campaign loadouts

Campaign levels now populate shared Rust optional-weapon availability and starting selection from cumulative original wave hints. Reset restores the initial loadout; earlier levels clear later unlocks. All 476 campaign cases, 18 native/WASM setups, encounter parity, 103 Rust tests, Clippy, and WASM build pass. Selection and special-fire controls remain pending.

### 2026-11-07 — Recover special-weapon control timing and Scattershot requests

Added original selection repeat timing, change-only sound, and optional-fire timing against the shared firing timestamp. Scattershot requests [5, 3, 4] after one timing gate. The selection suite now passes 8,192 original comparisons. All 102 Rust tests, strict Clippy, and WASM build pass. Live control integration remains pending.

### 2026-11-07 — Recover optional-weapon cycling and wave unlocks

Added original eighteen-slot cycling and hint-driven unlock rules, verified in 4,096 complete original calls. Repeated unlocks preserve selection; the all-weapons hint has an explicit selection order. Traced cumulative wave-hint loading and the Mac selection control timing. Browser selection/firing integration remains pending. All 102 Rust tests, Clippy, and WASM build pass.

### 2026-11-07 — Verify complete targeted-shot selection and launch correction

Added the composed weapon-8 target-angle helper and 1,024 original constructor/selector comparisons. Selection uses the owner pose; final correction uses the projectile pose, then construction continues as weapon 0. Single-player ownership filtering prevents assuming every enemy shot targets the player. Live weapon controls and hit routing remain pending. All 102 Rust tests, Clippy, and WASM build pass.

### 2026-11-07 — Restore the original cached-angle firing branch

Original CODE 20 uses cached aim for movement mode 1 when an enemy targets player slot 1. The port now preserves that branch, including type-8 predictive aim. Added 720 original routing comparisons and a live firing regression for kinds 7 and 8. All 102 Rust tests, strict Clippy, WASM build, authored type-8 movement/reset, and death transition checks pass.

### 2026-11-07 — Recover predictive aiming and activate type-8 pursuit

The complete original target predictor matches Rust in 2,048 separate comparisons, with original integer math and a synthetic direction table. Type-8 enemies now lead moving players and pursue without object selection. Central level 4 WASM movement/reset, 101 Rust tests, strict Clippy, build, and encounter replay parity pass. Modeled direction tables and other documented adapters remain provisional.

### 2026-11-07 — Verify type-8 pursuit and movement against original code

Added 2,800 original-code comparisons for type-8 movement, initialization, and tracking. It shares ordinary tank movement but skips the world-object search. All 241,213 main vectors pass. Live activation awaits its distinct predictive aiming path; hosting remains stopped.

### 2026-11-07 — Recover and connect behavior 4/9 pursuit refresh

Ported complete CODE 20 +0ef4 and connected the encounter dispatcher. It refreshes tracking and requests reinitialization beyond the original difficulty-dependent distance. All 1,024 original handler cases pass; the full main oracle totals 238,413 matches. All 99 Rust tests, Clippy, and WASM build pass. This handler alone does not enable further stationary enemy types.

### 2026-11-07 — Respect submerged player height in enemy firing

Removed the old constant-zero target altitude from live enemy firing checks. Enemies now obey the recovered below-ground exclusion, retain their timer, and can resume after emergence. Positive flight height remains eligible. All 98 Rust tests, Clippy, WASM build, encounter native/WASM parity, transit driving, and flight/reset checks pass. Other stationary enemy types and complete vertical combat remain unfinished.

### 2026-11-07 — Use original primary colors in overhead view

Overhead scenery, enemies, flags, and player now use recovered primary kind bindings and the current level palette; pickups retain subtype colors. A regression distinguishes primary from radar colors and checks palette changes and fallback. All 97 Rust tests, strict Clippy, WASM build, 112 arenas, and flight/view/reset checks pass. Captured map frame inspected. Flashing and multiplayer overrides remain unfinished.

### 2026-11-07 — Fill overhead silhouettes from original Plane Sides data

Traced the resource tail through the original top-view drawing path. Overhead rendering now fills the authored vertex prefix and draws remaining lines. The loader rule matches 1,024 original executions, bringing the focused suite to 6,426. All 97 Rust tests, strict Clippy, WASM build, 112 arenas, and flight/view/reset checks pass. Offline map frame inspected; projection, colors, and exact rasterization remain provisional.

### 2026-11-07 — Recover runtime model metadata setters

Verified original model edge-split, face-reference, and mode setters across 1,024 complete calls. Corrected the provisional edge_count field name: +25e divides edge ranges; +25a is the total count. The focused suite now passes 5,402 vectors, with complete model-byte preservation and selected-slot lookup checks. All 96 Rust tests and strict Clippy pass. Loader-to-renderer metadata propagation remains pending.

### 2026-11-07 — Verify the connected geometry preparation path

The Rust path now connects original transform setup, projection, interval scanning, and record construction. It preserves skipped projection coordinates, interval locals, and retained buffer entries scanned beyond alternate geometry. All 512 integrated original-code cases pass; the focused suite totals 4,378 matches. All 96 Rust tests, strict Clippy, and WASM build pass. Live rendering still needs complete source metadata and buffer lifecycle integration.

### 2026-11-07 — Recover complete prepared rendering record construction

Ported CODE 12 +1c20..1d56, including original direction lookup, wrapping coordinate conversion, metadata copies, signed class bounds, and untouched fields. All 1,024 new construction cases match the executable; the combined focused suite passes 3,866 vectors. All 96 Rust tests, strict Clippy, and WASM build pass. Existing transform helpers are already recovered; complete stage composition and record lifecycle remain pending before live integration.

### 2026-11-07 — Recover object insertion order and wireframe overlap marking

Rust now reproduces stable signed-key insertion and the original bytewise overlap gate. The focused oracle verifies 262 insertion sequences with full record/pointer pairing and 1,024 overlap blocks, alongside 1,556 drawing-order cases: 2,842 total matches. All 96 Rust tests and strict Clippy pass. Live use still needs the intervening transform and complete prepared-record construction.

### 2026-11-07 — Recover original prepared-object drawing order

Traced the submerged object byte into CODE 12: it selects a drawing class, not a hidden flag. Filled mode draws signed classes ascending and prepared records descending within each class; wireframe visits records ascending. A Rust helper matches 1,556 focused original-code executions, with geometry pointers, record preservation, and stack checked. Live integration awaits recovery of the preceding preparation pass.

### 2026-11-07 — Use original horizon height during submergence

Horizon bands now use the recovered vertical-transition height instead of constant 32. The diagnostic palette fallback shares the same band geometry. WASM pixel checks verify disappearance below ground and return after emergence. All 96 Rust tests, strict Clippy, WASM build, transit driving, and flight checks pass. Submerged object visibility and full rendering state remain unfinished.

### 2026-11-07 — Drive through original submergence and emergence devices

Kinds 37 and 36 now trigger recovered descent/ascent and original sounds; descent clears flight energy. All 2,048 original handler cases pass (237,389 total). WASM drives through original Central 10110 placements, submerges, resurfaces, and resets without teleporting. 96 Rust tests, Clippy, WASM, flight, 112 arenas, and 476 campaign checks pass. Special vertical interactions and complete original rendering/scheduling remain unfinished.

### 2026-11-07 — Use recovered rollback and rebound in ordinary collisions

Supported ordinary movement now runs the complete type-6 step, recovered contact gate, negative-time rollback, speed reversal, and airborne height hold. Boundary requests use the original distinct sound. Special handlers and unsupported handling states keep the prior adapter. Live rebound/hold tests, 95 Rust tests, Clippy, WASM, pickup driving, flight, 112 arenas, 476 campaign checks, replay parity, encounters, and death transitions pass.

### 2026-11-07 — Verify complete local tank movement and rollback

512 complete original type-6 dispatcher calls match composed Rust movement for forward, zero, and negative elapsed time. Original turn, speed, altitude, and participant turn-rate getter execute; only spatial-index maintenance is captured. Scope: local single-player, coast and surface adjustment zero. Total: 235,341 differential vectors. 94 Rust tests, Clippy, and WASM build pass. Live rollback/rebound integration remains next.

### 2026-11-07 — Verify the collision response gate and service order

2,048 original single-player response cases verify special-handler suppression, the height-hold gate, boundary bypass, local collision sounds, and ordered notification/sound/rollback requests. Fixed a harness translation-cache issue and reran all 234,829 matching vectors. 93 Rust tests, Clippy, and WASM build pass. The actual rollback service and live response integration remain unfinished.

### 2026-11-07 — Recover solid-contact rebound state

2,048 original-code cases verify movement-word reversal, speed reversal, a 30-update airborne hold, boundary bypass, and raw fixed-point height correction. Total: 232,781 matching differential vectors. 93 Rust tests, Clippy, and WASM build pass. Live response integration awaits the preceding hold gate, movement rollback, and special-contact handlers.

### 2026-11-07 — Fly after collecting a blue pickup

Live flight now connects recovered energy and vertical state to movement/surface contacts, cockpit and chase cameras, player shots, enemy hit-height checks, pickup effects, and respawn/reset. A WASM tank drives to a blue pickup, takes off, fires aloft, consumes energy, lands, and resets. Offline flight frames inspected. 93 Rust tests, Clippy, WASM, 112 arenas, 476 campaign checks, encounter and death checks pass. Exact scheduling, landing response, submergence triggers, and special flight weapons remain unfinished.

### 2026-11-07 — Use original collision geometry for tank movement

Supported movement checks now use recovered participant contacts, model dimensions, radii, orientation, and vertical overlap gates. Passable-surface filtering and rejection remain adapter policy; unsupported geometry keeps the footprint fallback. Height-aware query checks, 92 Rust tests, Clippy, WASM, pickup driving, 112 arenas, 476 campaign checks, native/WASM parity, encounters, and death transitions pass. Flight still awaits live shared-altitude integration.

### 2026-11-07 — Recover shared vertical transition state

Recovered original camera-scale and renderer-height transitions together with hover and collision height. 2,048 new original-code cases pass (230,733 total). A sequence test verifies takeoff, energy depletion, and gradual landing. All 92 Rust tests, Clippy, and WASM build pass. Live flight remains pending vertical movement collision and renderer/projectile integration.

### 2026-11-07 — Recover flight and hyperspace energy clocks

2,048 original-code cases verify energy consumption, hyperspace recharge, pause behavior, flight-enable transitions, and integer overflow across all twelve participant slots. Total: 228,685 matching differential vectors. Live flight still requires shared vertical state in collision, camera, shots, pickups, and lifecycle handling.

### 2026-11-07 — Recover original hover altitude calculation

2,048 original-code altitude cases match Rust, including speed-dependent target height, clock-driven oscillation, descent, temporary hold, integer overflow, and record preservation. Total: 226,637 differential vectors. Flight remains unconnected pending vertical collision, projectile, camera, and reset integration; energy consumption is traced but not yet oracle-verified.

### 2026-11-07 — Recover pulsing pickup geometry

The complete original animation matches Rust in 1,024 new differential cases (224,589 total). Live pickups expand and reset every 60 simulation ticks. Contact records and cached bounds stay unchanged. Exact dispatcher scheduling, shield protection, and flight controls remain unfinished.

### 2026-11-07 — Show original pickup colors and radar glyphs

Recovered subtype presentation: shared body/radar color and glyph 38, with original view/color gates. 2,048 new original-code cases pass (223,565 total). Connected resource colors and moving radar markers. A WASM pixel test caught and fixed missing pickups in the separate overhead renderer. All 90 Rust tests, Clippy, WASM build, and driven refill/reset checks pass. Pulsing geometry, shield protection, and flight controls remain pending.

### 2026-11-07 — Collect and relocate live ammo dumps

Recovered contact queries now collect dumps, apply original refills/sounds, select a new subtype, and relocate the same record. WASM fires and drives to a white dump, restores 40 rounds, and verifies relocation. All four subtypes pass Rust contact/reset tests; blue stores flight energy and orange retains a pending shield request. 90 tests, Clippy, WASM, 476 campaign checks, 18 native/WASM setups, encounter and death checks pass. Shield protection, flight controls, radar, and subtype visuals remain unfinished.

### 2026-11-07 — Create pickup objects in live arenas

Campaign/mission startup now creates up to six dumps with recovered placement arithmetic and a provisional independent Random stream. Records, subtypes, rendering, serialization, and reset are connected. Added traced procedural selector-6 mesh after offline inspection exposed missing geometry. 476 campaign checks, 18 native/WASM setups, pickup view/reset checks, 89 Rust tests, Clippy, and WASM build pass. Collection, radar, subtype colors, and animation remain pending.

### 2026-11-07 — Recover complete initial dump creation

Ported original initial placement, Manhattan-distance rejection, heading, subtype, and record initialization. 512 complete original calls match with only OS Random supplied; all record bytes, capacity, repeated attempts, and random consumption checked. All 221,517 vectors, 89 Rust tests, Clippy, and WASM build pass. Live scene/collision/radar integration and special-effect objects remain pending.

### 2026-11-07 — Load original campaign pickup weights

Arena state now reads type-5 bval weights instead of assuming a universal pickup mix. Expanded all 122 original loader checks to compare these fields; 221,005 vectors pass. All 476 campaign load/reset checks and 18 native/WASM setups pass with independent raw-resource weight checks. Traced six-dump creation request and shared 50-object cap; live creation remains pending.

### 2026-11-07 — Recover all pickup effect branches

All four original pickup effects now have a verified Rust representation: gradual/full refill, weapon-7 shield request, and conditional flight-energy refill. 2,048 original dispatch blocks match, including terminal sound/effect packets and local/network routing. All 221,005 vectors, 89 Rust tests, Clippy, and WASM build pass. Live creation/contact, shield objects, flight state, and resource weights remain pending.

### 2026-11-07 — Recover dump selection and relocation

Ported complete subtype selection and the 51-candidate relocation loop. 2,048 subtype cases and 256 relocation runs match original instructions with supplied OS randomness/collision results, including division overflow. All 218,957 saved vectors match Rust; 89 tests, strict Clippy, and WASM build pass. Live pickups, special effects, and resource-driven weights remain pending.

### 2026-11-07 — Recover ammo dump refill arithmetic

Located kind-5 pickup handler and subtype dispatch. Normal refill adds 20 rounds and one shield, capped at original capacities; full refill restores both. 1,024 original refill blocks match Rust, including signed overflow and participant preservation (216,653 total vectors). All 89 Rust tests, strict Clippy, and WASM build pass. Subtype effects, relocation, and live pickups remain pending.

### 2026-11-07 — Restore ice and mud contact sounds

Complete original ice/mud handlers now drive speed adjustment and sound cooldowns. Ordinary movement resets their clocks through the recovered speed-update condition. All 215,629 original comparisons, 89 Rust tests, Clippy, builds, 112 arenas, encounter/death/surface checks and 105-sample audio validation pass. Hosting stays stopped.

### 2026-11-07 — Connect static and moving acid damage

Acid now uses the active wave kind-23 damage parameter, original strict contact cooldown, player damage counters, warning sounds, radar disturbance, and normal death handling. Both contact kinds pass repeated-hit and death regressions; 89 Rust tests, Clippy, builds, surface-enemy, encounter, death and palette checks pass. Original contact scheduling and event overrides remain pending. Hosting stays stopped.

### 2026-11-07 — Connect moving-mud slowdown and recover acid contact requests

Moving mud now slows the tank and remains passable with recovered geometry; leaving contact restores acceleration. Complete acid handler matches 2,048 new original runs, including strict cooldown and damage routing. 213,581 vectors, 89 Rust tests, Clippy, builds, 112 arenas, encounter/death and surface-enemy checks pass. Live acid damage remains pending. Hosting stays stopped.

### 2026-11-07 — Activate moving acid and mud pursuit

Kinds 23/24 now use recovered initialization, tracking, phase timing, and shared movement arithmetic. Acid sounds and moving radar markers are connected. Authored WASM tests verify Central levels 12/13; 89 Rust tests, Clippy, 112 arenas, 476 campaign checks, encounter parity, death and palette checks pass. Moving surface contact damage remains pending. Hosting stays stopped.

### 2026-11-07 — Recover complete moving-surface target tracking

Recovered mode-zero initialization and complete kind-23/24 tracking, including local retarget writes and phase timing. All 2,848 new original comparisons and 211,533 total vectors pass, plus 88 Rust tests, Clippy, and WASM. Confirmed shared movement arithmetic with type 7; live activation and surface contacts remain pending. Hosting stays stopped.

### 2026-11-07 — Recover moving acid and mud AI phases

Recovered the kind-23/24 phase timer and complete behavior-five cooldown handler, including acid sound selection. All 3,072 new original comparisons and 208,685 total vectors pass, plus 88 Rust tests, Clippy, and WASM. Live surface-enemy tracking, movement, and damage remain pending. Hosting stays stopped.

### 2026-11-07 — Connect low-health radar disturbance to gameplay

Accepted low-health hits now jitter first-person contacts and play warning sample 10009. Selected difficulty controls the deadline; redraw expiry, reset and respawn are tested. All 88 Rust tests, Clippy, native/WASM builds, palette and death checks pass. Display RNG remains provisional; original shared randomness and sound queue controls are unfinished. Hosting stays stopped.

### 2026-11-07 — Recover the low-health radar disturbance trigger

Original damage code activates disturbance for local health at most 2. Recovered selected-difficulty timing, signed narrowing, and warning requests. 2,048 new original-block cases bring the passing total to 205,613; 87 Rust tests, Clippy, and WASM pass. Live activation, reset scheduling, and sound queue behavior remain pending. Hosting stays stopped.

### 2026-11-07 — Recover radar glyph emission and overlap order

Executed the complete original glyph routine in 2,048 new cases, including supplied Random results for disturbance. Live radar now uses recovered point expansion and original ascending color order. All 203,565 comparisons, 87 Rust tests, Clippy, WASM, and 16 palette checks pass. Disturbance activation remains pending. Hosting stays stopped.

### 2026-11-07 — Restore flashing flag radar contacts

Recovered single-player glyph selection and 25-tick flash deadline from CODE 6. Connected current flags and collection removal. All 201,517 original comparisons, 87 Rust tests, Clippy, WASM, palette and death checks pass. Display scheduling and multiplayer visibility remain pending. Hosting stays stopped.

### 2026-11-07 — Recover surface and teleporter radar contacts

Connected ten original object marker rules to first-person radar. All 17,920 new original-code comparisons and 197,421 total vectors pass, plus 87 Rust tests, Clippy, WASM, and 16 palette scenarios. Flags and remaining visibility gates are pending. Hosting stays stopped.

### 2026-11-07 — Connect original radar scaling and enemy contacts

Recovered arena-dependent scale initialization and connected first-person radar markers for the player and ten supported enemy types. Original glyphs, palette bindings, projection, and type-31 phase gating are live. All 179,501 vectors, 87 Rust tests, Clippy, WASM, arena/replay/death/palette checks pass; offline frame inspected. Other contacts and earlier visibility gates remain pending; hosting stays stopped.

### 2026-11-07 — Recover enemy radar symbols and type-31 visibility

Verified the original dispatch set and marker selection for ten enemy types. Type 31 appears only at its final animation frame; types 13, 20, and 22 use distinct symbols. All 2,048 new original comparisons and 178,477 total vectors pass, with 86 Rust tests, Clippy, and WASM. Earlier visibility gates and live radar integration remain pending; hosting stays stopped.

### 2026-11-07 — Load original radar symbols into desktop and WASM games

The shared executable-resource path now carries PICT 1000 through the catalog into Game. Sixteen campaign/level scenarios verify all 41 symbols, reset preservation, and desktop/WASM parity. All 86 Rust tests, Clippy, WASM, 112 arenas, 476 campaign checks, and 18 native/WASM setups pass. Original object-selection rules and live radar drawing remain pending; hosting stays stopped.

### 2026-11-07 — Decode the original radar glyph atlas

Added a bounded decoder for the owned PICT 1000 layout and original column-first glyph sampling. All 41 glyphs match the original extraction loop; 256 synthetic atlases and every truncated resource prefix are also checked. GetPixel is modeled. All 176,429 vectors, 86 Rust tests, Clippy, and WASM pass. Catalog loading and live radar drawing remain pending; hosting stays stopped.

### 2026-11-07 — Recover nonlinear radar projection and edge markers

Ported complete radar projection and twelve edge pixels. All 2,048 projection and 1,024 marker comparisons execute original instructions without fixtures. Located 41 original radar glyphs in PICT 1000 for the next integration step. All 176,172 vectors, 86 Rust tests, Clippy, and WASM pass. Live radar selection, glyph loading, and colors remain pending; hosting stays stopped.

### 2026-11-07 — Apply recovered compact framing to the playable view

First-person geometry, ground, horizon, and aiming marks now share original compact surface bounds and 256-pixel projection, uniformly scaled from 640x480 to 800x600. Added clipping and margin regression coverage. All 86 Rust tests, Clippy, WASM, 112 arenas, movement parity, death, and palette checks pass; offline frame inspected. Full-display selection and original instruments remain pending. Hosting stays stopped.

### 2026-11-07 — Trace immediate surface placement into framebuffer rows

Recovered surface/global/pixmap coordinate conversion and destination row addressing. Under 640x480 compact assumptions, the immediate path explains 40 of the 41 pixels separating the video from deferred-layout predictions. Added a two-path regression. All 173,100 vectors, 85 Rust tests, Clippy, and WASM pass. OS translations are modeled; the remaining pixel and live framing integration are pending. Hosting stays stopped.

### 2026-11-07 — Verify complete immediate aiming-mark call chain

Recovered axis-line rectangles with 1,024 original calls and checked 256 complete crosshair chains through the original dispatch and line wrappers. Only final rectangle painting is captured. The immediate surface setup differs from deferred drawing origins and needs further placement tracing. All 172,076 vectors, 84 Rust tests, Clippy, and WASM pass. Hosting stays stopped.

### 2026-11-07 — Verify drawing origins and cross-check video aiming marks

Ported complete original viewport origin setup with 1,024 comparisons, including monochrome origin preservation. Video mark endpoints exactly match recovered geometry, but expose an unresolved 41-pixel vertical placement difference under assumed screen bounds. All 170,796 vectors, 84 Rust tests, Clippy, and WASM pass. Live framing stays provisional; hosting stays stopped.

### 2026-11-07 — Recover complete compact and full display rectangle setup

Ported inset completion, extent recalculation, and the 80-pixel top adjustment. Added an odd-dimension and instrument-strip regression. All 1,024 new original comparisons, 169,772 total vectors, 84 Rust tests, Clippy, and WASM pass. InsetRect is modeled. Live framing integration remains pending; hosting stays stopped.

### 2026-11-07 — Recover original world, radar, and instrument rectangles

Ported original viewport placement through CODE 33 +0x87e. All 1,024 new instruction comparisons and 168,748 total vectors pass, with 83 Rust tests, Clippy, and WASM. OffsetRect is modeled and marker initialization is skipped. Live layout integration remains pending; hosting stays stopped.

### 2026-11-07 — Draw original aiming marks with the bound color

Recovered all three segments and the hidden branch from complete original executions. First-person rendering now uses their geometry and palette binding. All 167,724 differential vectors, 83 Rust tests, Clippy, WASM, arena, replay, death, and palette checks pass. Viewport scaling remains provisional; hosting stays stopped.

### 2026-11-07 — Draw original horizon bands with level colors

Recovered complete horizon rectangle requests and connected shaded bands to the original horizon palette binding. The playable horizon now changes with campaign colors. 1,024 original runs use modeled OffsetRect and captured paint requests; all 166,700 vectors, 82 Rust tests, Clippy, WASM, arena/death/palette checks pass. Alternate horizon states and viewport sizing remain pending; hosting stays stopped.

### 2026-11-07 — Cull faces using original viewer coordinates and planes

Recovered the caller’s model-space viewer conversion and retained full authored/procedural plane records. First-person faces now use original visibility before clipping. All 1,024 new original comparisons and 165,676 total vectors pass, with a rendered front/back regression, 81 Rust tests, Clippy, WASM, arena/death/palette checks. Object heights and unsupported faces remain pending; hosting stays stopped.

### 2026-11-07 — Recover original face-plane visibility

Ported complete packed-plane evaluation and visibility wrapper, including wrapped products/sums and visible equality. All 2,048 paired original comparisons and 164,652 total vectors pass, with 80 Rust tests, Clippy, and WASM. Caller-coordinate recovery and live face-culling integration remain pending; hosting stays stopped.

### 2026-11-07 — Apply original solid depth and distance limits

Recovered suppression/depth/cached-distance admission and connected distance limits to first-person drawing. All 2,048 new original comparisons and 162,604 total vectors pass, with 80 Rust tests, Clippy, WASM, 112 arenas, death camera, and palette checks. Type-specific suppression and cache scheduling remain pending; hosting stays stopped.

### 2026-11-07 — Clip packed model edges with original arithmetic

Recovered and integrated the original two-endpoint near clipper with preserved interpolation order, integer projection, and depth-buffer inputs. All 2,048 new original comparisons and 160,556 total vectors pass, with 80 Rust tests, Clippy, WASM, 112 arenas, death camera, and palette checks. Edge visibility/color dispatch, viewport scaling, and effect projection remain pending; hosting stays stopped.

### 2026-11-07 — Use original clipping and projection for solid faces

Packed first-person polygons now use original integer intersections and projected coordinates with retained depth-buffer inputs. Recovered the required horizontal origin visibility gate after an offscreen projection-overflow regression. All 158,508 original comparisons, 80 Rust tests, Clippy, WASM, 112 arenas, death camera, and palette checks pass; offline frame inspected. Viewport scaling, lines, and remaining visibility gates remain pending. Hosting stays stopped.

### 2026-11-07 — Recover original polygon near clipping

Ported the complete indexed face clipper, signed-word intersection interpolation, and screen placement. The original plane is packed depth 16 (two scene units). All 2,048 new original comparisons and 157,484 total vectors pass, with 79 Rust tests, Clippy, and WASM. Depth-buffer integration remains pending; hosting stays stopped.

### 2026-11-07 — Keep scenery orientation consistent across cameras

Exterior, overhead, and first-person fallback meshes now share the native-to-scene heading conversion used by collision. The asymmetric orientation regression checks actual pixels in all three views at two headings. 79 Rust tests, Clippy, WASM, 112 arena view checks, death camera, and 16 palette scenarios pass. Original exterior/map camera parameters remain pending; hosting stays stopped.

### 2026-11-07 — Align scenery collision with native model headings

Fixed participant heading conversion incorrectly applied to scenery in collision records, driving footprints, projectile fallback, and spawn/AI obstacle poses. Flow Fields now use authored native headings. An asymmetric obstacle regression checks collision and clear space. 79 Rust tests, Clippy, WASM, 112 arenas, campaign, encounter, death, and palette checks pass. Collision response remains partly provisional; hosting stays stopped.

### 2026-11-07 — Render packed meshes with original rotation

First-person meshes retain packed vertices and use recovered integer matrix rotation/translation. Scenery and dynamic target heading conventions are reconciled for rendering and lighting. An asymmetric orientation regression, 78 Rust tests, Clippy, WASM, palette/death/112-arena checks, and 476 campaign checks pass. Near clipping, viewport placement, object height, and collision orientation audit remain pending. Hosting stays stopped.

### 2026-11-07 — Use original object-position projection in play

First-person solid origins now use original camera-relative rounding and rotation, including the death camera. 77 Rust tests, strict Clippy, WASM, 16 palette scenarios, 112 arenas, death transition, and 476 campaign checks pass. Offline frame inspected. Local mesh rotation, clipping, viewport scale, and shot projection remain provisional. Hosting stays stopped.

### 2026-11-07 — Recover camera-relative solid coordinates

Ported original planar rounding and camera rotation into render-record horizontal/depth words. All 2,048 new original comparisons and 155,436 total vectors pass, with 77 Rust tests, strict Clippy, and WASM. Traced caller basis to the original direction lookup. Coordinate/unit integration and viewport placement remain pending; hosting stays stopped.

### 2026-11-07 — Recover model rotation and translation

Ported the original render-record matrix setup and complete direction lookup. All 1,024 new original comparisons and 153,388 total vectors pass, with 77 Rust tests, strict Clippy, and WASM. Upstream world-to-render-record conversion, viewport placement, and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Recover original packed vertex transformation and projection

Translated complete original matrix-transform and single-vertex projection routines, including wrapped sums/depth gates, DIVS overflow, and preserved screen words. All 2,560 new original comparisons and 152,364 total vectors pass, along with 77 Rust tests, strict Clippy, and WASM. Camera/model matrices, viewport placement, and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Recover complete deferred eight-bit frame cycle

Composed original erase, paint, counter update, palette publication, queue rotation, auxiliary merge, and reset into one native frame cycle. All 128 complete original cycles match state and pixels, including counter wraparound; BlockMove is modeled and palette readiness supplied. All 149,804 vectors, 77 Rust tests, strict Clippy, and WASM pass. Platform upload/timing and live game integration remain unfinished; hosting stays stopped.

### 2026-11-07 — Recover deferred queue reset, merge, and rotation

Native queues now preserve the original dirty-gated reset/merge rules and three-header rotation order. All 256 original merge/reset sequences and 256 header rotations match; BlockMove is explicitly modeled for merges. All 149,676 vectors, 77 Rust tests, strict Clippy, and WASM pass. Allocation growth, full frame scheduling, and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Connect recovered line queues to paint and erase

Added the native deferred frame queue with original clipping, rejection compaction, ordering, and dirty-state behavior. All 256 original queue/paint/erase sequences match counts, records, and framebuffer bytes across eight appends each. All 149,164 vectors, 77 Rust tests, strict Clippy, and WASM pass. Original capacity growth, queue rotation, scheduling, and live renderer integration remain pending; hosting stays stopped.

### 2026-11-07 — Recover complete deferred frame painting and erasure

Recovered both complete eight-bit paint/erase paths across thirteen color groups, line records, and both direct-pixel lists. All 256 original sequences match every framebuffer byte after painting and erasure. All 148,908 vectors, 77 Rust tests, strict Clippy, and WASM pass. Queue allocation/rotation, display scheduling, and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Verify alternating frame colors through original pixels

Recovered both deferred RGB tables and frame-parity selection. All 256 table/selection cases and 128 composed two-phase sequences match original indexed pixels and RGB words, including masks, clipping, and rasterization. All 148,652 vectors, 77 Rust tests, strict Clippy, and WASM pass. Queue management, old-frame erasure, display upload, and live integration remain unfinished; hosting stays stopped.

### 2026-11-07 — Recover deferred frame-phase color masks

Translated both original four/eight-bit mask builders and the thirteen-color deferred setup loop. All 1,024 complete constructor cases and 512 complete setup cases match both phase tables and supporting words. All 148,268 vectors, 77 Rust tests, strict Clippy, and WASM pass. Phase RGB composition, queue management, and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Recover original eight-bit line pixel writing

Rust now matches complete original clipping and eight-bit deferred line rasterization across 512 batches. Checks compare all framebuffer bytes, generated records, and unchanged source records, including clipped/overlapping lines and single pixels. All 146,732 vectors, 77 Rust tests, strict Clippy, and WASM pass. Frame-phase masks, queue handling, other pixel depths, and live integration remain unfinished; hosting stays stopped.

### 2026-11-07 — Recover deferred line clipping and raster records

Translated the complete original viewport clipper and sixteen-byte line constructor. All 2,048 original executions match, including rejection, endpoint swaps, major-axis selection, row direction, framebuffer offsets, and one/four/eight-bit starting masks. All 146,220 vectors, 77 Rust tests, strict Clippy, and WASM pass. Pixel writing and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Recover original line palette patterns

Recovered all thirteen line patterns and both supporting word tables, including black aliases and four-bit color pattern five. All 512 original setup-block comparisons and 144,172 total vectors pass, along with 77 Rust tests, strict Clippy, and WASM. Immediate/deferred line services and live integration remain unfinished. Hosting stays stopped.

### 2026-11-07 — Recover complete projected-edge drawing dispatch

Translated the full original dispatcher, including three ordered batches, slope insertion sorting, special-edge ordering, and six drawing-service choices. All 1,152 complete original executions match, including 128 cases at 50-edge capacity. All 143,660 vectors, 77 Rust tests, strict Clippy, and WASM pass. Drawing services are capture fixtures; live renderer integration remains pending. Hosting stays stopped.

### 2026-11-07 — Recover projected-edge selection and classification

Both initial edge scan paths now have Rust translations verified against 2,048 original instruction blocks. Checks cover clipping, both visibility references, signed screen-coordinate ordering, classification bytes, and queued endpoints. All 142,508 vectors, 77 Rust tests, strict Clippy, and WASM pass. Later drawing passes and live integration remain pending; hosting stays stopped.

### 2026-11-07 — Recover original edge merging and visibility references

Recovered complete edge insertion and opposite-direction merging into a Rust record with both visibility references. Original callers and drawing code show these words are visibility indices, not palette colors. All 1,536 new original executions and 140,460 total vectors pass, along with 77 Rust tests, strict Clippy, and WASM. Renderer integration and line-color recovery remain pending; hosting stays stopped.

### 2026-11-07 — Restore filled-flag colors

Both filled flag faces now use the original kind-twelve primary color and packed normals in first-person shading. Captured 256 original submission blocks across all color bytes. All 138,924 vectors, 77 Rust tests, strict Clippy, WASM, 16 palette scenarios, and 112-arena replay checks pass. Flag pole materials, decorative letters, and object lighting flags remain unfinished; hosting stays stopped.

### 2026-11-07 — Restore procedural column colors

Selector-three columns now use original kind-two colors and packed normals in first-person shading. All 256 original face submissions agree with the earlier cap/wall geometry captures. All 138,668 vectors, 77 Rust tests, strict Clippy, WASM, 16 palette scenarios, and 112-arena replay checks pass. Object lighting flags and remaining special materials are unfinished; hosting stays stopped.

### 2026-11-07 — Restore procedural box-body colors

Selectors 1 and 5 now shade their six-face bodies using original builder-selected kind colors and packed normals. 512 original caller/builder executions verify all face arguments. All 138,412 vectors, 77 Rust tests, WASM, palette, arena, and campaign checks pass. Line decorations, columns, and flags remain pending; hosting stays stopped.

### 2026-11-07 — Restore procedural tetrahedron colors and face winding

Tetrahedrons now use packed geometry, verified original side-face winding, kind-one level colors, and recovered shading. All 256 original face-submission cases and 137,900 total vectors pass. Offline yellow scenery inspected; 77 Rust tests, native/WASM palettes, arenas, campaign, and death checks pass. Other procedural colors remain pending; hosting stays stopped.

### 2026-11-07 — Render authored faces with recovered colors and dithering

First-person authored solid faces now use disc color bindings, packed integer normals, recovered light/shade arithmetic, RGB ramps, and screen-aligned patterns. Offline frame inspected. All 77 Rust tests, palette, 112-arena replay, 476 campaign/boundary, and death checks pass. Procedural colors, special materials, lighting flags, and full projection remain unfinished; hosting stays stopped.

### 2026-11-07 — Recover lighting setup and face-color flashes

2,048 original blocks verify horizontal lighting components, inherited face colors, flash limits, and shared timer behavior. All 137,644 differential vectors and 76 Rust tests pass, plus Clippy and WASM. Caller-state and procedural-face color integration remain; hosting stays stopped.

### 2026-11-07 — Recover complete original face planes

2,048 complete original constructors verify integer normals, winding, cross-product scaling, degenerate faces, plane offsets, and colors. All 135,596 differential vectors and 76 Rust tests pass, plus Clippy and WASM. Live packed-geometry and lighting/color integration remain; hosting stays stopped.

### 2026-11-07 — Verify original dither placement

1,024 original row selections and complete eight-bit solid-span writes verify framebuffer-aligned dither, partial spans, and untouched neighboring pixels. All 133,548 vectors and 76 Rust tests pass, plus Clippy and WASM. Face normals and caller color integration remain; hosting stays stopped.

### 2026-11-07 — Recover original RGB shade ramps

1,024 complete RGB interpolations and 64 complete 192-color ramp blocks match Rust. All 132,524 differential vectors pass. Exact sixteenth-step FixRatio results are supplied in the ramp fixture. Pixel-pattern placement remains before live integration; hosting stays stopped.

### 2026-11-07 — Recover eight-bit shaded pixel patterns

4,096 complete original lookup/pattern calls match Rust; 131,436 differential vectors pass. Recovered special colors, shade offsets, and packed pattern ordering. RGB shade ramps and pixel placement remain before live integration; hosting stays stopped.

### 2026-11-07 — Recover original face lighting

2,048 original lighting and brightness/fading blocks match Rust. All 127,340 differential vectors and 75 Rust tests pass, plus Clippy and WASM. Traced the next dependency to display-specific pixel patterns; live shading integration remains pending.

### 2026-11-07 — Recover original face color selector

1,024 original wrapper calls match Rust; all 125,292 differential vectors and 75 Rust tests pass. Traced encoded selectors through a separate resource-record loader, which remains unfinished. Rendering integration is pending; hosting remains stopped.

### 2026-11-07 — Load shared palettes and render original ground color

Browser and desktop can load executable color resources while preserving campaign palette overrides. Ground dots use composed original RGB; object/horizon colors remain provisional. 16 native/WASM palette scenarios, 476 campaign checks, 112-arena replay checks, and shared-color death checks pass. Offline frame inspected; hosting remains stopped.

### 2026-11-07 — Verify composed resource palettes

Matched 512 original palette assembly blocks, including base RGB, both level substitutions, and final black assignment. Identified Central’s authored clut 1000 override and the need for shared executable resources. All 124,268 differential vectors and 74 Rust tests pass. Live palette loading remains pending.

### 2026-11-07 — Recover the original level-color cycle

Matched 1,024 complete level-color calls with supplied resource handles, including 30-entry cycling, missing-resource fallbacks, and the four-bit yellow override. Disc colors corroborate yellow/magenta opening and purple/green later footage. All 123,756 differential vectors and 74 Rust tests pass. Live palette composition remains pending.

### 2026-11-07 — Recover original color bindings and base palette

Matched 515 color-binding loader blocks and 516 base-palette copies, including all three cob tables and four disc cluts. Ground dots use special color index zero; RGB depends on palette selection. All 122,732 differential vectors and 74 Rust tests pass. Level substitutions and live palette integration remain pending.

### 2026-11-07 — Use recovered ground projection during play

First-person and death views now run original ground bounds, sparse sampling, culling, and integer projection. Ground and solids share fixed camera position and modeled direction values. An explicit 500/256 adapter preserves current framing. 74 Rust tests, WASM death checks, and 112-arena replay checks pass; offline driving/death frames inspected.

### 2026-11-07 — Verify the composed original ground-point pipeline

Matched 256 original caller paths through region construction, bounds, sparse sampling, and projection. Corrected direction and depth/horizontal ordering at helper interfaces. All 121,701 differential vectors and 74 Rust tests pass. Live projection integration still needs consistent solid-geometry framing; pixel drawing and direction generation remain separate.

### 2026-11-07 — Recover ground visibility bounds and camera basis

Matched 1,024 complete original visible-grid bounds calls and 1,024 camera-basis/row-advance cases. Original min/max and software multiply run directly; direction values are supplied. All 121,445 differential vectors and 74 Rust tests pass. Visible-region construction and live projection integration remain pending.

### 2026-11-07 — Render the recovered sparse ground lattice

Connected original 64-division spacing and every-eighth-row/column sampling to first-person and death views. Verified 1,540 additional original-code cases; all 119,397 differential vectors and 74 Rust tests pass. WASM death checks and 112-arena replay checks pass. Projection and visible-row selection remain provisional.

### 2026-11-07 — Recover both original ground-row projection modes

Matched 1,024 complete row calls, including culling, coordinate increments, height branches, and preserved output prefixes. Full-display mode doubles lateral acceptance without changing projection scale. All 117,857 differential vectors and 74 Rust tests pass. World-to-row setup and live ground integration remain pending.

### 2026-11-07 — Recover display extents and viewport placement

Matched 1,024 original viewport metric blocks and 512 display extent/inset blocks. Traced full-display and nominal 512×342 modes, the 80-pixel upper band, and local projection center. All 116,833 differential vectors and 74 Rust tests pass. QuickDraw placement and live framing integration remain pending.

### 2026-11-07 — Recover original ground projection lookup table

Matched four complete 4,800-byte table initializations and 512 original projections using the generated table. Original division and word writes execute without fixtures. All 115,297 differential vectors and 74 Rust tests pass. Live ground projection still needs viewport and dispatch recovery.

### 2026-11-07 — Correct death camera height from original projection

Recovered 1,024 original ground projection calls and established eighth-unit camera heights. Fixed the eightfold height error; the midpoint frame now shows the arena and ground. 114,781 differential vectors, 74 Rust tests, and WASM death checks pass. Full projection and palettes remain provisional.

### 2026-11-07 — Play the recovered death transition before respawn

Live deaths now hold controls for 260 ticks while the world updates, render the recovered camera in all views, then consume one life or enter final loss. Native and WASM timing/control tests pass; 73 Rust tests and campaign suites pass. Offline frames expose remaining camera projection calibration; death fragments remain pending.

### 2026-11-07 — Recover original death-to-respawn deadline gate

Verified 2,048 complete lifecycle gates and 512 transition entry blocks. Local death becomes eligible at its 260-tick deadline; the transition resets the deadline sentinel before restoring object kind. All 113,757 differential vectors and 72 Rust tests pass. Live scheduling remains pending.

### 2026-11-07 — Recover original death camera calculation

Verified death-flag selection, local death state, and active camera arithmetic against 2,560 original-code cases. Camera turns around the saved position with increasing radius and height. All 111,197 differential vectors and 71 Rust tests pass. Rendering and respawn timing integration remain pending.

### 2026-11-07 — Connect original enemy hit sound selection

Both collision paths now choose nonfatal or death feedback, removing the duplicate hit sound on enemy kills. Tests cover protected type-31 hits, multi-hit destruction, and a WASM one-hit kill. 70 Rust tests, audio, encounter parity, and lives checks pass.

### 2026-11-07 — Connect original enemy reset after player death

Verified 768 original AI resets and 512 death notification scans. Live active enemies reset behavior, movement mode, and firing timer while preserving tracking. Authored-arena respawn and two open-field respawns pass. 108,637 differential vectors and 70 Rust tests pass; remaining raw AI fields are not yet represented.

### 2026-11-07 — Use original player damage sound selection

Recovered 512 original sound branches and connected enemy projectile hits to nonfatal, respawn-death, and final-death samples. Removed the extra hit sound on fatal player damage; preloaded sample 10013. 107,357 differential vectors and 69 Rust tests pass. Original channel scheduling remains pending.

### 2026-11-07 — Recover original death fragment requests

Verified 1,600 original fragment routines: randomized count and angle, weapon 6, previous-kind propagation, and network gates. Corrected earlier radial labels to weapon 0/source kind 7. All 106,845 differential vectors pass. Live integration remains pending.

### 2026-11-07 — Recover original death-burst requests

Verified 1,024 original death gates and 1,600 radial burst routines, including two/four directions, conditional Random consumption, height conversion, and network authority gates. All 105,245 differential vectors pass. Live integration remains pending.

### 2026-11-07 — Recover original damage response state

Verified 1,024 original damage-response blocks: participant deadlines, changed flag, killed-kind transition, and multiplayer notification order. All 102,621 differential vectors pass. Live handler integration remains pending.

### 2026-11-07 — Recover area-effect target selection and damage requests

1,200 complete original calls verify participant filtering, distance arithmetic, target-indexed damage, range bands, and request order. The native helper emits matching damage requests; live application remains pending.

### 2026-11-07 — Recover projectile retirement and expiry dispatch

3,098 original scans verify movement, removal, owner-count release, type-10 retirement, and delayed area-effect requests. Weapon 6 expires by removal; weapons 1 and 17 have a separate retirement path. Area-effect integration remains pending.

### 2026-11-07 — Connect bouncing projectiles to live enemy fire and rendering

Weapon 6 now uses recovered launch fields, 300-tick lifetime, and bounce motion. First-person and chase markers follow height. Trajectory and render regressions pass; original hit effects, collision scheduling, and compact-pool integration remain pending.

### 2026-11-07 — Verify complete bouncing-projectile movement

720 complete original type-29 calls match native direction lookup, bounce motion, and grid movement across all headings. Atomic capacity guards pass. Live rendering and hit-effect integration remain pending.

### 2026-11-07 — Recover bouncing projectile motion

Type-29 horizontal motion, height, gravity, and ground bounce match 1,200 original-code executions, including arithmetic boundaries. Live height/rendering, collision, and hit-effect integration remain pending.

### 2026-11-07 — Recover weapon-specific constructor sounds

Executed the original sound-table initializer and all eighteen weapon sound routes. Live launch sounds now follow that table, including one Scattershot sound and silent spread requests. Original sound-channel scheduling remains pending.

### 2026-11-07 — Use resource weapons for enemy firing requests

Enemy fire state retains the original weapon ID. Unarmed enemies emit no shots, while Scattershot emits center and two recovered spread requests with per-projectile admission. Special projectile effects and original Random remain pending.

### 2026-11-07 — Apply enemy shot limits per owner

Enemy projectiles retain their target owner. Each enemy gets its own twenty-shot limit; expiry and impacts release that owner’s slots. A two-enemy regression verifies isolation, timing, sounds, and resumed firing. Compact-pool integration remains pending.

### 2026-11-07 — Recover and enforce the fresh-session player shot limit

Traced fresh single-player settings to the original twenty-shot limit. All 256 flag values match original code. Live player firing rejects full-slot attempts without ammo loss and resumes after removal. Enemy accounting and compact-pool capacity remain pending.

### 2026-11-07 — Connect recovered low-ammo warning to live firing

Player firing uses recovered ammo admission and queues original sample 10010 when ammo crosses down to five. Browser preload and WASM audio checks pass. Pool allocation, concurrent-shot limits, and original sound-channel control remain unfinished.

### 2026-11-07 — Unify removal and execute the original status callback

Traced CODE 14 +2e20 to its status-byte write. Consolidated the earlier raw-record helper with typed removal and extended all 1,024 removal comparisons to execute this service. All 95,105 differential cases and 60 Rust tests pass.

### 2026-11-07 — Recover geometry removal bookkeeping

Original grid unlinking, projectile shot-count decrement, participant deactivation, and compaction match 1,024 complete machine-code executions. External participant service effects and live pool integration remain pending.

### 2026-11-07 — Recover geometry compaction and reference repair

768 original compaction tails match Rust geometry bytes, participant indices, and grid references. Invalid native inputs are rejected without partial mutation. Full removal integration remains unfinished.

### 2026-11-07 — Recover weapon admission and ammunition handling

1,600 original admission blocks match Rust: owner/team checks, active-shot limits, ammo charging, warning sounds, and allocation-failure refunds. Full constructor integration remains unfinished.

### 2026-11-07 — Recover geometry allocation and projectile finalization

612 original allocator calls and 768 common finalization blocks match Rust. Native allocation rejects the original zero-index capacity mismatch. Full projectile lifecycle integration remains unfinished.

### 2026-11-07 — Recover weapon-specific projectile record construction

1,080 original construction blocks match all 128 record bytes in Rust: kinds, lifetimes, offsets, height changes, and preserved fields. Allocation and common finalization remain unfinished.

### 2026-11-07 — Recover spread and aimed-shot heading preparation

1,000 original constructor blocks match Rust. Weapons 3/4 apply opposite 7–14 degree spreads; weapon 8 optionally aims at a selected target. Projectile construction and live integration remain unfinished.

### 2026-11-07 — Recover enemy weapon-request dispatch

1,200 original dispatch blocks match Rust, including weapon-5 triple requests, weapon-12 remapping, integer heights, and network routing. Projectile constructors remain unfinished.

### 2026-11-07 — Recover enemy-kind firing rules

1,410 complete original predicates match Rust across 47 kinds, including range limits, timer overrides, and kind-26 behavior changes. Additional enemy types remain inactive pending their weapon and movement rules.

### 2026-11-07 — Recover mode-specific opponent acquisition

1,024 complete original calls across all game-mode bytes match Rust. Single-player, scored-target, and nearest-opponent paths remain distinct. Multiplayer runtime and networking are not implemented.

### 2026-11-07 — Preserve original cached-pose pursuit

Ordinary pursuit now steers toward the stored target pose instead of the live player position. 1,200 original movement cases and a live cache-delay regression pass; native/WASM encounter and flag replays agree.

### 2026-11-07 — Restore original distant-enemy command gate

Original gate matches 1,024 executions. Live type-7 enemies skip commands beyond 500 units while more than five enemies remain. 57 Rust tests and encounter/campaign/lives/flags suites pass.

### 2026-11-07 — Integrate original guarding and attack intent

Live type-7 enemies now use recovered object selection, approach/guard states, intent dispatch, and persistent combat mode. 56 Rust tests and native/WASM mission replays pass. Runtime records and Random generation remain provisional.

### 2026-11-07 — Recover original guard-object selection

800 complete original object selections match Rust, including assignment limits, relative distance, and 2,267 Random calls. Original assignment counts also match across 13,587 scans. Runtime integration remains unfinished.

### 2026-11-07 — Recover enemy object guarding and intent initialization

800 original initializer cases and 600 complete object-tracking cases match Rust. Enemies can approach world objects and guard them within 40 units. Original object selection still needs recovery before live integration.

### 2026-11-07 — Recover complete type-7 tracking handler

1,200 complete original handler executions match Rust state writes and service calls. Local-participant retargeting and inactive-state ordering are preserved. Live caller integration remains unfinished.

### 2026-11-07 — Recover original participant target selection

Complete target selector matches 1,000 original-code executions, including flags, teams, range, angle scoring, and tie order. Final angle service is supplied; caller integration remains unfinished.

### 2026-11-07 — Recover intent dispatch and tracking state

Original dispatch gate and type-7 tracking tail match 7,328 original-code cases. Combat mode persists across range exit. Earlier retarget selection remains unfinished; live intent switching is still provisional.

### 2026-11-07 — Drive enemies with recovered steering policies

Live type-7 enemies now use recovered heading search, ordinary/combat commands, turn-only selection, and cached aiming. The fixed 135-unit stop and alternating-side steering are removed. Target selection, intent transitions, and OS angle conversion remain provisional. 55 Rust tests and native/WASM gameplay suites pass.

### 2026-11-07 — Use recovered nearby-enemy yielding

800 original priority scans match Rust. Live later-slot type-7 tanks pause translation near qualifying earlier enemies and resume after separation grows. Participant activation/order remain preview mappings; pursuit choices remain provisional. 54 Rust tests and native/WASM encounter, lives, and campaign checks pass.

### 2026-11-07 — Use original enemy motion parameters

Live type-7 tanks now use recovered turn/translation arithmetic and original radial/angular speeds. 1,400 new original-code comparisons pass; pursuit and obstacle-choice logic remain provisional. All 53 Rust tests and native/WASM gameplay suites pass.

### 2026-11-07 — Use recovered firing rules in the playable encounter

Live type-7 enemies now use original resource firing intervals, short range, arc limits, and recovered timer/distance rules. Scenery stops their projectiles. Steering and OS angle conversion remain provisional. 52 Rust tests and native/WASM encounter, campaign, lives, flags, game, and audio checks pass.

### 2026-11-07 — Recover the shared AI aiming cache

1,000 paired original pose/angle getter cases match Rust. Cache refresh occurs at five elapsed ticks. 200 cases execute the actual type-7 target-copy path: this enemy mode does not extrapolate its target. OS angle fidelity, command scheduling, and live integration remain pending.

### 2026-11-07 — Recover nonlocal combat movement

700 complete original combat-mode calls match Rust for type-7 behavior 0/3. Tests supply prediction, aim, clearance, and prior scratch-local values. Native callers guard an unwritten-local edge case. Prediction-cache recovery and live integration remain pending.

### 2026-11-07 — Recover attack tracking and movement dispatch

1,024 attack transitions, 3,072 movement dispatches, and 509 turn-only calls match original execution. Attack tracking selects a distinct combat-mode branch; ordinary movement alone does not reproduce it. Combat-mode recovery and live integration remain pending.

### 2026-11-07 — Recover a complete ordinary AI movement policy

Rust matches 400 complete original behavior-3 movement calls, plus 1,004 reverse-state cases. Tests cover heading refresh, ordered clearance search, reverse selection, and output commands. Aim and clearance services are explicit fixtures; other behaviors and live integration remain pending.

### 2026-11-07 — Recover the complete AI clearance probe

Rust matches 1,000 original probes and 400 complete probe/collision scenes. The composed scenes supply no service results and produce 223 blocked and 177 clear outcomes. Direction-table initialization remains modeled; live movement-policy integration is pending. All 61,158 comparison and guard cases pass.

### 2026-11-07 — Recover AI steering and heading search

684 original movement-decision blocks and 216 complete heading searches match Rust. The search tests alternating 15-degree offsets. Clearance-probe results are supplied; full policy integration remains pending. All 59,758 game comparison and guard cases pass.

### 2026-11-07 — Check historical QuickDraw aiming conversion

Rust matches 1,551 executions of Apple’s historical QuickDraw angle routine, including table boundaries. Reference source and tables stay local. This is not System 7 ROM verification; FixRatio and live AI integration remain pending.

### 2026-11-07 — Recover AI aiming preparation

Ported integer-coordinate rounding, quadrant selection, near-horizontal bypass, heading subtraction, and asymmetric angle normalization. 800 original helper calls match with explicit OS conversion results; three nonterminating original boundary cases have host guards. Located the firing timer reset to -1. Exact Mac OS angle conversion and live AI integration remain pending.

### 2026-11-07 — Recover type-7 firing eligibility

Ported the original type-7 range, interval, close-range timing, target-state, and firing-arc predicate. 800 complete original calls match Rust with supplied angle-service results. Another 300 original calls verify its planar distance approximation. Live AI integration awaits angle and caller-timer recovery; existing firing behavior is unchanged.

### 2026-11-07 — Earn original campaign vehicle upgrades

Entering every eighth level after level ten now attempts the original shield/ammo/speed upgrade. Verified 572 full original routine executions with original validators. Settings persist through advancement, death, and restart; movement and refill use them. WASM level 15→16 upgrades hull to seven. Direct level selection still starts with fresh Tank 1 settings. 49 Rust tests pass.

### 2026-11-07 — Continue campaigns with score and lives

Added Next level after campaign completion. The shared Rust transition preserves score/lives, refills hull/ammo, and clears round state. Restart restores the carried level-entry baseline. WASM fixtures verify advancement, anti-skip checks, rollback on failed loads, and restart. 200 original refill blocks match Rust; 48 Rust tests pass. Original movies, transition timing, upgrades, and powerups remain unfinished.

### 2026-11-07 — Load playable campaign levels

Added Campaign level mode with level and difficulty controls. The shared Rust loader uses the selected wave roster, cycle health modifier, flag conversion, score rules, and level bonus. All 467 shipped entries plus nine repeat boundaries load/reset; 18 native/WASM setups match. Each selection starts fresh; score/life carryover and automatic advancement remain pending. Browser interaction remains unverified while hosting is stopped.

### 2026-11-07 — Resolve wave resources and audit shipped campaigns

Recovered consecutive resource counting and zero-ID random resolution, including signed wrapping and exact random consumption. 600 original resolution blocks and 32 full enumeration calls pass. All 467 shipped wave entries have explicit arena/roster IDs and available world, enemy, and bval resources. Playable campaign state and transitions remain pending.

### 2026-11-07 — Recover campaign wave selection

Ported difficulty-based wave selection, level wrapping, cycle health modifiers, and repeat-cycle hint/opening-movie suppression. Closing movies remain selected. Verified 1,042 original selection blocks across all nine disc wave resources and four synthetic lists. Added an offline campaign-wave inspection command. Random resource resolution and playable campaign transitions remain pending.

### 2026-11-07 — Recover enemy parameter overrides

The shared loader now preserves low-byte type IDs, last-row overrides, type-5 parameter exclusions, and original defaults. Verified health modifier arithmetic against 122 full original loader calls, including all 22 disc parameter tables. 54,303 differential cases pass. Traced the normal health modifier to campaign cycles; campaign progression remains pending.

### 2026-11-07 — Restore omitted enemy types from original defaults

Verified all 47 default parameter records by executing the original loader initialization. Restored five missing type-45 enemies in each of Central/South layout 10100. All 77 linked layouts now have completion target times; 35 unlinked layouts remain unrated. 54,181 differential cases pass. Type-specific AI remains incomplete.

### 2026-11-07 — Completion ratings and bonuses

Final flags now award recovered Quickness, Accuracy, and Avoidance bonuses. Browser results show each category and total score. 75 arenas have target-time data; 35 unlinked layouts and two incomplete enemy plans explicitly omit ratings. 47 Rust tests, strict Clippy, 112 mission checks, native/WASM parity, and completion/reset fixtures pass. Campaign transitions and browser visual checks remain pending.

### 2026-11-07 — Record original completion statistics

Player/other launches, protected-target hits, and admitted player damage events now feed shared Rust combat counters. Counters survive respawns and reset with the mission. 800 new original-code comparisons verify counter and grace-period behavior; completion results integration is next. All 54,134 differential cases, 46 Rust tests, strict Clippy, all 112 mission starts, and native/WASM replay checks pass. Hosting stays stopped.

### 2026-11-07 — Connect original flag and bonus sound cues

Original request routing now selects flagPull, OneMoreFlag, newMan, and allRobotsKilled from the disc. Browser audio preloads all four; gameplay emits them on flag pickups, earned lives, and final enemy retirement. Original mixing and desktop audio remain pending. 512 original-code routing comparisons pass (53,334 total). All 105 samples preserve original PCM; WASM verifies flag and one-time enemy-clear events. Hosting stays stopped.

### 2026-11-07 — Award the original last-enemy bonus

Permanent retirement of the last mission enemy awards the recovered bonus once, using initial enemy counts and life budgets. Level-one previews award 50 points per initial enemy life. The flag objective remains active; original bonus presentation/audio remains pending. 500 new original-code comparisons pass (52,822 total). Actual WASM verifies kill points, the 50-point single-life bonus, no duplication, and reset. 46 Rust tests and strict Clippy pass. Hosting stays stopped.

### 2026-11-07 — Preserve flag placement slots after pickup

Collected flags leave kind-14 placement placeholders. Type-20 respawns keep their original cursor index even after visible flags disappear. Duplicate-position flags and reset are covered by a regression test; broader geometry lifecycle recovery remains ongoing. 46 Rust tests, strict Clippy, all 112 mission starts, respawn checks, and native/WASM replays pass. Hosting stays stopped.

### 2026-11-07 — Play enemy lives, respawns, and retirement

Flag missions now use recovered enemy caps/life budgets, same-type life sharing, and respawn search. Kills score once each; the HUD separates kills from permanent retirement. Count selection uses explicit Random=0; random generation, geometry lifecycle, and AI remain provisional. 45 Rust tests, strict Clippy, actual WASM enemy respawn/scoring, all 112 mission starts, and native/WASM replay checks pass. Hosting stays stopped.

### 2026-11-07 — Play with recovered player respawn placement

Mission respawns now use original coordinate, heading, retry, and participant-clearance rules. Supported models use recovered contact queries; unsupported geometry uses the preview fallback. Random generation remains explicitly provisional. Initial placement and enemy respawning remain pending. 44 Rust tests, strict Clippy, all 112 mission starts, native/WASM replays, two authored-arena respawns, and a single-enemy open-field final-death fixture pass. Hosting stays stopped.

### 2026-11-07 — Combine original placement branches

240 full original calls verify flag cursor advancement, retry ordering, changing team anchors, and Random consumption. These tests corrected the flag-relative caller to type 20 (hex $14), not type 14. Runtime collision and placement integration remain pending. All 52,322 differential cases, 43 Rust tests, and strict Clippy pass. Hosting stays stopped.

### 2026-11-07 — Recover flag-relative and team spawn transforms

1,000 new original-code comparisons verify type-20 flag-relative placement arithmetic and selected-mode team transforms. Flag cursor initialization, trig-table fidelity, and solid-contact integration remain pending. All 52,082 differential cases, 42 Rust tests, and strict Clippy pass. Hosting stays stopped.

### 2026-11-07 — Recover spawn retries and participant clearance

760 new original-code comparisons verify the normal spawn retry loop and participant distance scans. The original keeps its final candidate after 501 attempts. Clearance uses Manhattan distance. Solid-contact wiring and special placement branches remain before live integration. All 51,082 differential cases, 41 Rust tests, and strict Clippy pass. Hosting stays stopped.

### 2026-11-07 — Recover original spawn coordinate ranges

768 original-code comparisons verify player inset coordinates, full-arena enemy coordinates, and central-area placement for types 35/45. Original collision retries, special placement branches, and Random generation remain before live integration. All 50,322 differential cases and 40 Rust tests pass; strict Clippy passes. Hosting stays stopped.

### 2026-11-07 — Recover shared enemy lives

Original enemies can borrow a life from the first eligible same-type participant before retiring. 1,200 new original-code comparisons verify transfers and the death gate. Placement, respawn integration, and last-enemy awards remain pending. All 49,554 differential cases and 39 Rust tests pass; strict Clippy passes. Hosting stays stopped.

### 2026-11-07 — Recover enemy counts and life budgets

353 original-code fixtures verify resource ranges, difficulty-based lives, and count-to-life cap balancing. Four supplied settings stall the original loop; Rust guards them. Gameplay integration and enemy respawning remain pending. All 48,354 differential cases, 38 Rust tests, and strict Clippy pass. Hosting stays stopped.

### 2026-11-07 — Recover completion ratings and bonuses

800 new original-code comparisons verify par time, Quickness/Accuracy/Avoidance ratings, bars, and bonuses. The first video results screen matches exactly: 200+200+400 points explains score 135 becoming 935. Runtime counter/table initialization and playable results remain pending.

### 2026-11-07 — Earn original enemy kill points

Mission kills now award original bval points through recovered attribution/type gates and scoring. Protected/nonfatal hits award nothing; a kill can earn an extra life. 812 new original-code comparisons, resource-byte checks across 112 missions, and WASM combat checks pass. Completion bonuses and multiplayer ownership remain unfinished.

### 2026-11-07 — Play with lives, flag score, and respawn

Flag missions now start with three lives. Death restores health/ammo while preserving collected flags, score, enemies, and the tick clock; the last death ends play. Original score rules can award extra lives. 400 lifecycle comparisons and actual WASM death/respawn checks pass. Placement, death presentation, kill points, and campaign transitions remain unfinished.

### 2026-11-07 — Identify difficulty-dependent score tables

Traced the original Difficulty control through its setter, getter, and score-resource selector. 192 original executions verify normal thresholds starting at 5,000/10,000/20,000 and mode overrides. Death/respawn entry points are identified; playable lives remain pending.

### 2026-11-07 — Recover score awards and extra-life thresholds

560 new original-code comparisons verify threshold lookup, 4,999-point award chunks, penalties, and the extra-life gate. Original score resources are decoded; the first Central table begins at 5,000 points. Playable score integration, table selection, and death/respawn remain pending.

### 2026-11-07 — Recover visible flags and original pickup geometry

Recovered the flag mesh and conditional copy of type-2 collision dimensions/radius. 302 new original-code comparisons pass. WASM now visibly renders flags and drives to an authored pickup without teleporting. All 3,439 raw-layout placements resolve. Load order, unsupported-arena pickup bounds, flag lettering, and campaign progression remain provisional or unfinished.

### 2026-11-07 — Collect authored flags to clear an arena

Flag mission is now the default playable mode. Recovered level-one layout conversion supplies two or three flags; drive over them, earn 20 flag points each, and collect all to finish. Restart restores flags and combat. Pickup geometry, AI, and round scheduling remain provisional; campaign transitions and bonuses are pending.

### 2026-11-07 — Expanded video reference through level 14

Indexed the full 21:34 recording with timestamped overview and event sheets. Confirmed custom vehicle settings, same-level replenishment, lives-based respawn, changing palettes and weapons, and high-score entry. Removed the incorrect zero-ammo loss condition: the video clearly shows continued play with empty ammunition. Original AI call paths are now identified for further recovery.

### 2026-11-07 — Depth-tested geometry and fullscreen controls

Replaced average-face sorting with per-pixel depth in first person. Walls now hide farther faces, shots, and impacts; concave openings remain visible. Added Fullscreen button and F shortcut with a letterboxed view and HUD. All 112 arena checks pass. Live browser fullscreen verification is pending; hosting remains stopped.

### 2026-11-07 — Map coordinates now checked against original instructions

Executed the original placement conversion for all 4,347 disc records and 400 signed/boundary cases. All match Rust. Scene tests now use these instruction results for 3,439 visible placements. Original initialization confirms the 1024-unit world modulus. Render orientation, spawns, and campaign sequencing remain open.

### 2026-11-07 — Enemies fight back; rounds can end

Added provisional type-7 pursuit and aimed fire, six-point player hull, cover, win/loss states, and restart. Enemy firing and player hits use original samples. Clearing escorts starts the recovered type-31 transition. Native/WASM encounter replays match. Map notes now distinguish disc data from unverified runtime behavior.

### 2026-11-07 — Original disc sound effects are playable

Rust decodes all 105 standard PCM sound resources. Browser gameplay now plays original firing, enemy-hit, explosion, collision, and empty-ammo samples with volume and mute controls. Sample bytes are preserved and redraws do not replay events. Trigger mapping remains provisional; music and original sound scheduling are pending.

### 2026-11-07 — Model slots now assemble actual collision geometry

Connected recovered dispatch to packed resources, animation frames, four procedural builders, aliases, and dimension overrides. 123 assembled resource geometries match original-instruction fixtures across three campaigns. Unsupported builders remain explicit. Startup ordering and live projectile-query integration are still pending.

### 2026-11-07 — Resource collision radius now retained

Recovered fixed-point radius iteration and original slot finalization. 825 new instruction comparisons pass; 364 resource pipelines now include radius instead of bypassing it. Packed models retain candidate radii. FixDiv and FixRound remain modeled OS operations; live collision integration remains pending. All 112 arenas still pass gameplay checks.

### 2026-11-07 — Animation model slot sequencing recovered

264 original-instruction comparisons verify animation destination slots and return counts, including all nine campaign animation resources. Frames use base slot plus frame index, even after a builder reports zero slots. Inner builders remain supplied fixtures; collision integration is still pending.

### 2026-11-07 — Closer clearance and higher aiming marks

Replaced the oversized circular tank collider with Tank 1’s ground footprint and obstacle convex outlines. Rotation also checks clearance. Raised the aiming stem and horizontal marks to better match the reference video. This remains preview collision; original fixed-point collision integration is pending.

### 2026-11-07 — Selected-model loader matches original dispatch

600 complete original-instruction comparisons verify reservation, three-column selection, five inner-loader ranges, and completion. Missing resources retain reservations; already-loaded requests still write zero counts. Resource Manager services and inner-builder results are supplied fixtures. Live collision integration remains pending.

### 2026-11-07 — First-person driving with solid scenery

The playable Rust/WASM view now uses a low first-person camera, filled recovered meshes, ground dots, aiming marks, and visible shots. Diagnostic chase remains selectable. All 112 arenas pass render and gameplay checks; camera switching preserves simulation state. Camera constants, colors, face ordering, and background are provisional visual matches, not recovered original rendering.

### 2026-11-07 — Original gameplay reference captured

Inspected 4:00–5:00 of the supplied video, with 12 overview snapshots and a denser combat sequence. First-person camera, solid polygons, magenta horizon, dotted ground, HUD, and level transitions give concrete visual targets. Video does not establish exact physics or hidden gameplay rules.

### 2026-11-07 — Original octagonal columns are playable

Recovered selector-3 packed vertices and polygon indices now replace 215 unresolved map markers. Across 112 maps, 3,342 placements resolve and 97 remain unresolved. All scene and desktop/WASM gameplay checks pass. Original lighting and collision integration remain unfinished.

### 2026-11-07 — Runtime resource geometry now loads in the catalog

All 358 campaign shape entries plus six capacity cases match original vertex packing and dimension instructions. Catalog loading now retains packed coordinates and base collision sizes. FixRound is modeled; radius, procedural builders, and slot integration remain pending.

### 2026-11-07 — Complete outer model loader matches original dispatch

1,128 comparisons cover all 47 types, dependency order, slot state, size halving, counters, and extra resources. Geometry loaders are explicit fixtures. Resource geometry, procedural builders, and startup order remain before live collision integration.

### 2026-11-07 — Recovered special-type model aliases

500 comparisons execute original alias branches with a supplied model-loader fixture. Types 23–26 share types 16, 18, 3, and 7; current and first slots copy, but frame counts do not. Geometry builders and remaining loader branches are still pending.

### 2026-11-07 — Recovered model-slot reservation behavior

1,046 original-code comparisons verify reservation, the slot-100 capacity boundary, and completion counts. Empty loads preserve their assigned slot. Geometry builders, loading order, and special aliases remain before live collision integration.

### 2026-11-07 — Recovered complete basic-shot collision caller

800 original calls match Rust, including the nested query: 247 hits, 336 misses, and 217 rejected hits. Recovered midpoint sweep, owner exclusion, height rejection, and target-type rules. Model-slot construction remains before live integration.

### 2026-11-07 — Recovered separate collision direction tables

Original initialization uses direct angles and extended math, unlike movement. All 360 Rust pairs match modeled SANE. Another 800 complete collision queries match original execution with those inputs. Original OS math and live model-slot integration remain unfinished.

### 2026-11-07 — Complete collision query matches original execution

800 complete original queries match Rust: 592 hits and 208 no-hit results. Model lookup, grid traversal, overlap, ordering, and final rejection execute together. Runtime direction tables, model data, and the projectile caller remain before live integration.

### 2026-11-07 — Recovered final collision rejection rules

3,816 original-code comparisons verify type exclusions and participant state checks after hit selection. The meaning of participant state 3 remains untraced. Complete-query validation and live collision integration remain pending.

### 2026-11-07 — Verified procedural loader routing

1,004 original-dispatch comparisons map selectors to builders. Campaign type 18 uses resource 10095 and bypasses the procedural halving path. Types 4, 9, 12, and 28 select the identified override builders. Live collision integration remains pending.

### 2026-11-07 — Verified collision-dimension override arithmetic

600 comparisons cover six original loader blocks: ten-unit dimensions, halving, doubling, and copying another slot. These edits depend on loader paths and slot aliases; they are not unconditional object-type rules. Live collision integration remains pending.

### 2026-11-07 — Recovered participant-backed object deletion

141 additional comparisons cover all object kinds 0–46 and execute the original deactivation call. Participant active/status bytes, object compaction, and grid repair match Rust. Death effects, respawn, and live storage integration remain unfinished.

### 2026-11-07 — Recovered complete projectile deletion

200 complete original-routine comparisons verify object compaction, owner shot counts, participant index repair, grid references, and tail-slot state. Tank/enemy deactivation and live object-storage integration remain unfinished.

### 2026-11-07 — Recovered projectile insertion and grid removal

968 original-code comparisons verify the 20-entry insertion cap, missing-ID behavior, and removal by swapping in the last entry. Full object deletion also compacts the object array and repairs indices; that work remains before live integration.

### 2026-11-07 — Recovered object movement between grid cells

508 comparisons verify coordinate clamping and grid mutation. Movement replaces the old entry with the last entry, then appends to the destination; this can change collision tie order. Initial insertion, deletion, participant bookkeeping, and model overrides remain before live integration.

### 2026-11-07 — Verified projectile grid traversal

75 original-loop comparisons confirm ascending X/Z cell traversal and stored object order, including duplicate IDs and empty ranges. Hit testing is bypassed only for these traversal checks. Grid population, movement updates, and collision-size overrides remain before live integration.

### 2026-11-07 — Recovered projectile grid bounds

The complete query-bound calculation matches 540 original-code cases. It preserves dimension rounding, the short-query branch, 64-unit cells, and original edge-wrap behavior. Grid traversal and model overrides remain before live collision integration.

### 2026-11-07 — Recovered projectile filtering and hit preference

Verified object exclusion, nine excluded types, integer distance checks, and selection between overlapping hits. 2,538 new original-code comparisons pass. Equal projections preserve traversal order. Live collision still needs grid traversal and model dimension overrides.

### 2026-11-07 — Traced resource vertex conversion

Recovered scale, fixed multiply, and packing into eighth-unit coordinates. 1,368 conditional comparisons pass, including 1,157 unique resource vertices. FixRound uses the documented Apple rule in the harness; the original OS trap remains unverified. Collision overrides and candidate filtering are next.

### 2026-11-07 — Recovered model collision dimensions

Original rectangles use twice the largest absolute planar vertex coordinates, centered on the model origin. 205 comparisons pass for packed runtime vertices. Radius was bypassed and remains unverified. Resource conversion and dimension overrides still need recovery before live integration.

### 2026-11-07 — Recovered original rectangle collision test

The complete oriented rectangle test matches 1,000 original-code cases, including signed arithmetic. Traced projectile midpoint sweeps and model-indexed dimensions. This helper is not live yet: dimension overrides, candidate filters, and hit selection still need recovery.

### 2026-11-07 — Recovered projectile expiry and elapsed time

Basic shots expire at their deadline before movement. Movement uses the recovered elapsed-time clamp; a shot has zero elapsed time at launch. 666 new original-code comparisons pass. Collision and the overall scheduler remain provisional.

### 2026-11-07 — Recovered primary-fire timer is playable

Held fire attempts a basic shot every eleven ticks with one-tick updates. Empty-ammo attempts reset the timer too. 774 original-code comparisons cover timing, overflow, and the disable flag. All 112 arena checks and desktop/WASM replays pass. Original update ordering remains unfinished.

### 2026-11-07 — Recovered projectile launch and motion are playable

Basic shots start six units ahead and use recovered fixed-point movement with candidate direction values. 660 original-code comparisons pass. Lifetime is 75 ticks; collision, firing cadence, and update ordering remain provisional.

### 2026-11-07 — Recovered ammunition rules are playable

The balanced preset starts with 40 rounds. Basic shots consume original weapon cost and stop when ammo is insufficient. The complete original eligibility routine matches 572 new cases. Reset refills ammo; special modes, projectile capacity, and firing timing still need recovery.

### 2026-11-07 — Basic shots use original weapon damage

Recovered the 18-entry weapon table loader, including defaults and byte indexing. 106 comparisons include all three campaign resources. The playable tank now reads weapon-0 damage from wval 10000; ammo, modifiers, projectile motion, and firing cadence remain unfinished.

### 2026-11-07 — Recovered enemy animation and protection are playable

Type-31 targets resist shots until their final original model frame. Replay enemy unlock event manually starts the recovered transition; its original trigger is movie completion. Browser checks show Protected 2 becoming 0, and reset restoring 2. All 112 arenas and desktop/WASM replays pass.

### 2026-11-07 — Enemy damage links to model animation

Original initialization links enemy objects to participant health. Type 31 permits damage only at its final model slot. Recovered the shared animation transition: 504 original-code comparisons and 141 enemy linkage checks pass. Event activation remains to be traced before gameplay integration.

### 2026-11-07 — Participant damage guard recovered

1,648 new comparisons execute the original type guard and its helper. Participant records differ from arena objects, so the guard is not assigned to enemy types without tracing their damage route. Preview enemy damage remains provisional.

### 2026-11-07 — Recovered handling is playable

Replaced guessed acceleration and steering with recovered routines, balanced preset, and fixed-point state. Direction-table math remains a labeled candidate. All 112 arenas and desktop/WASM replay checks pass; targets can still be destroyed and reset.

### 2026-11-07 — Direction-table initialization reconstructed

Traced the stored angle step and single-precision rounding. All 360 Rust candidate values match original initialization instructions with modeled SANE math services. Original SANE validation and browser integration remain pending; these are separate from the verified-code count.

### 2026-11-07 — Local movement checked; machine-dependent turning identified

Expanded movement comparisons to local-player ground-state paths. Recovered the machine-type selector that halves turning rate on specific original Macs. Direction-table values and additional movement effects remain pending.

### 2026-11-07 — Directional command priority recovered

Recovered the original keyboard command mixer. All 256 directional combinations and 100 noncanonical input cases match original execution. Traced the participant turn-rate initialization and its optional halving branch.

### 2026-11-07 — Original Coast control and vehicle presets identified

The original settings dialog labels Coast and permits 0–7. All three built-in vehicle presets use Coast 0; random coasting applies to values 1–5. Recovered preset selection passes 88 original-code comparisons.

### 2026-11-07 — Original planar movement and scheduler traced

Verified 1,080 translation cases across all 360 integer headings using explicit test tables. Traced the clock caller and catch-up steps of up to four ticks. Original direction values and the complete local-player path remain pending.

### 2026-11-07 — Original speed update recovered

Ported the signed-byte speed update, limits, and random coasting branches. All 1,200 new cases match original instruction execution. Browser handling remains provisional while input mapping, direction tables, and timing are recovered.

### 2026-11-07 — Original enemy models and verified damage arithmetic

Four targets in the first arena use original wave types and base health. Shooting destroys targets; reset restores them. Verified 256 damage cases and 90 animation-frame selections against original instructions. Animated models show their first frame while timing is traced.

### 2026-11-07 — Original map items replace collision boxes

Traced the three-column shape selector and checked 423 selections against original instructions. Across 112 maps, 3,127 placements resolve and 312 remain marked as unresolved. The default arena resolves all 20 visible placements. Damage and destruction are still being traced.

### 2026-11-07 — Original machine code → verified Rust routines

Mapped 599 jump-table entries. Recovered angle wrapping, angular distance, direction indexing, and an object heading-update routine. All 1,333 test cases match isolated original 68K execution. Browser movement is still provisional while player input and timing are traced.

### 2026-11-07 — First driving and firing loop

Drive Tank 1 through recovered layouts and fire at collision boxes. Ten Rust tests pass. All 112 arenas spawn and render; 240-tick desktop/WASM driving and firing replays agree within 0.001 coordinate units.

### 2026-11-07 — A window into the rebuild

Added this interactive progress page, original model thumbnails, and an automatically refreshed milestone log.

### 2026-11-07 — One renderer, two platforms

Verified desktop and browser reference frames pixel-for-pixel. The WebAssembly module has no imports. Six Rust unit tests and strict Clippy checks passed.

### 2026-11-07 — Original geometry recovered

Decoded shape, world, wave, enemy, and combat-value resources from the three campaign archives. Model colors and shading remain diagnostic.

