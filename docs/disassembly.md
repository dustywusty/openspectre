# Original executable recovery

This work analyzes the supplied Spectre VR/CD 1.1 executable.
The browser uses recovered movement components with a candidate direction table and provisional collision.
The sections below distinguish instruction-verified routines from modeled services and preview choices.

## Reproduce the analysis

Export the executable resources:

```sh
cargo run --locked -p spectre-tools -- export .local/extracted/spectre.bin .local/game-resources
python3 -m venv .local/re-tools
.local/re-tools/bin/pip install -r scripts/research-requirements.txt
.local/re-tools/bin/python scripts/disassemble.py
.local/re-tools/bin/python scripts/verify-recovered.py
```

The first script writes listings and an index to `.local/disassembly/`.
The second executes original instructions with Unicorn, then compares their results with the Rust implementations.
Only the research tests use CPU emulation. The browser runtime has no emulator dependency.

Original bytes, listings, and generated test vectors stay in ignored local directories.
The extraction provenance records the source disc hash.

## Executable map

CODE 0 contains the jump table. Its four header fields are:

| Field | Value |
| --- | ---: |
| Bytes above A5 | 4824 |
| Bytes below A5 | 31752 |
| Jump-table bytes | 4792 |
| Jump-table offset from A5 | 32 |

The table contains 599 entries across CODE 1 through CODE 36.
Each entry has the expected `3f3c` and `a9f0` words.
Each segment header agrees with its jump-table range and entry count.

All offsets below include the four-byte CODE segment header.
For an unloaded jump entry, add four to its routine offset to obtain the resource-relative offset.
The callable A5 displacement is the table base plus the entry index times eight, plus two.

The index records 3,002 candidate cross-segment calls.
These are linear-sweep results, not 3,002 proven reachable calls.
Embedded tables and strings can decode as instructions. Indirect calls are not fully resolved.
Known jump-table entry boundaries constrain the sweep.

## Recovered routines

The implementations are in `crates/spectre-core/src/recovered.rs`.
Names below are our descriptive names, not recovered source symbols.

| Routine | Original location | A5 entry | Evidence |
| --- | --- | --- | --- |
| Angle wrap | CODE 7 +0x0004 | +0x0522 | Adds or subtracts 0x01680000 until the signed value is within one turn |
| Angular distance | CODE 7 +0x0036 | +0x052a | Takes the absolute difference; above 0x00b40000, subtracts it from one turn |
| Direction-table index | CODE 18 +0x0dcc | +0x0bca | Reads the signed high word, takes remainder modulo 360, and corrects negative results |
| Object heading update | CODE 30 +0x10b8 | Local routine | Updates object heading, applies a type-6 ramp branch, and wraps the result |
| Speed update | CODE 30 +0x11d6 | Local routine | Updates signed speed, limits it, and conditionally applies coasting |
| Normal health subtraction | CODE 31 +0x0b2e–0x0b40 | Block within +0x0a84 | Subtracts a signed damage word with wrapping, then clamps negative health to zero |

One turn is `360 << 16`. Half a turn is `180 << 16`.
The direction lookup ignores fractional degrees.
Its two tables begin at A5-0x3312 and A5-0x38b2.
The test uses distinct marker tables to verify both output pointers and index selection.
It does not recover the original table values.

Angular distance expects normalized angles for its usual meaning.
The Rust implementation also preserves wrapping subtraction and negation outside that domain.
It does not silently normalize its inputs.

### Heading update

CODE 30 +0x10b8 selects a 128-byte object record through A5-0x1346.
It updates the heading at object offset +0x08.
For types other than 6, it adds the supplied heading delta directly.

Type 6 uses a signed byte at +0x23 as a turning ramp.
For a positive elapsed argument, it increases that byte by at most four.
A zero command word at +0x0c clears the ramp.
It caps the ramp at eight and scales the heading delta by ramp divided by eight.
For negative elapsed arguments, the ramp adjustment occurs after the heading change.

The multiply wraps to 32 bits before signed division.
The routine wraps the resulting heading into one turn.
The source of the heading delta, elapsed units, and identity of type 6 remain unresolved.
The Rust function preserves instruction behavior without claiming these are player-control constants.

## Differential validation

The harness executes original routines on an isolated emulated M68000.
It supplies a stack, synthetic object records, and marker tables.
For the heading routine, A5 jump stubs link to the original min, max, multiply, and divide helpers.
Execution must return within a 10,000-instruction budget.

Results:

- 261 angle-wrap cases, including signed integer limits.
- 511 angular-distance cases, including normalized and wide signed inputs.
- 261 direction-index cases, checking both table writes.
- 300 heading-update cases across types 0, 6, and 7.
- 256 normal health-subtraction cases, including signed limits.
- 1,200 speed-update cases, including overflow, reverse motion, limits, and random coasting branches.
- 4,320 type-6 planar translation cases covering all 360 integer headings, three elapsed values, and four mode/locality contexts.
- 88 vehicle-setting cases covering preset, custom, and unrecognized selectors.
- 356 directional command cases, including every combination of the eight directional slots.
- 75 machine-type and environment-error combinations for the half-turn-rate flag.

An additional 1,648 cases cover the participant damage guard and its original CODE 6 helper.
Another 504 cases cover the complete type-31 model transition routine.
Another 106 cases cover the complete weapon-table loader with supplied Resource Manager services.
Another 572 cases cover the complete local firing eligibility predicate.
All 30,980 cases match Rust.
Separately, 141 original initialization checks verify enemy type and health-table linkage across 47 types and three object indices.
The heading cases cover positive, negative, and zero elapsed arguments; commands -1, 0, and 1; and ramp values 0 through 8.
This is strong evidence for those tested domains, not proof of the full game loop.
Fourteen Rust unit tests and strict Clippy checks also pass.

## Movement and collision leads

These are static-analysis leads, not complete recovered systems.

| Location | Observation | Remaining question |
| --- | --- | --- |
| CODE 30 +0x2700 | Subtracts a stored timestamp, takes the signed low word, clamps it to 0–63, then calls +0x1d54 | Timestamp source and time unit |
| CODE 30 +0x1d54 | Dispatches by object type; calls heading update +0x10b8 and movement candidate +0x1370 | Player type, speed sources, and per-type branches |
| CODE 30 +0x1370 | Uses direction routines and updates position-related state; type 6 has a distinct branch | Exact translation, acceleration, and input mapping |
| CODE 7 +0x0d90 | Reads a 128-byte object record, checks two coordinates against a global boundary, then searches a spatial grid | Object collision shapes and complete response |
| CODE 7 +0x16a0 | Calls +0x0d90 and branches on its outputs | Damage, boundary, and collision-response semantics |
| CODE 23 +0x05b8 | Selects a per-participant value or a table value, with an optional divide-by-two branch | Meaning of the mode flags and selected parameter |

The object records use A5-0x1346 as their base pointer, with 128-byte stride.
A second record table uses A5-0x1342 with 256-byte stride.
Semantic field names remain hypotheses until readers and writers agree.

## Next investigation

1. Trace the caller of CODE 30 +0x2700 to establish the timestamp source.
2. Connect player input to the command and type fields.
3. Execute isolated type-6 movement cases with controlled original tables.
4. Recover the collision query and its object-shape mapping.
5. Replace prototype systems only after complete paths pass differential tests.

## Tool references

The analysis uses [Capstone's M68K disassembler](https://github.com/capstone-engine/capstone).
The test harness uses [Unicorn's CPU emulator](https://github.com/unicorn-engine/unicorn).
Classic segment-loader structure is described in Apple's [Inside Macintosh, Volume II](https://bitsavers.computerhistory.org/pdf/apple/mac/Inside_Macintosh_Vol_2_1984.pdf).
The executable bytes and the differential tests are the primary evidence for the recovered behavior.

## Map shape selection

CODE 1 +0x1206 loads `shps 10000`. The count precedes rows of three signed words, indexed by object kind.
A5-0x7be2 and CODE 19 +0x3f3c select a column. Their full display-mode semantics remain unresolved.
The scene uses column three, matching the A5-0x7be2-zero branch at CODE 1 +0x1278.
It supports direct `shap` references and procedural selector 2.
Zero selections have no model and are omitted from prototype collision. Their original gameplay semantics remain unresolved.

| Selector range | Original loader |
| --- | --- |
| 1–999 | Procedural code at CODE 1 +0x133a |
| 1000–1999 | `rshp`, random alternatives |
| 2000–2999 | `mshp`, multipart |
| 3000–3999 | `ashp`, animation |
| 10000–10999 | `shap`, direct geometry |

Procedural selector 2 dispatches at CODE 1 +0x1396 to CODE 19 +0x312c.
Its four vertex arguments come from +0x3144 through +0x3180.
The diagnostic renderer connects the six tetrahedron edges.
Palette, original polygon metadata, and other procedural generators remain unfinished.

### Scene checks

```sh
.local/re-tools/bin/python scripts/verify-shape-selection.py
npm run test:scenes
```

The first check executes all three original column-selection branches for each campaign row.
All 423 selections match the decoded table.
The second compares Rust/WASM scene items against those machine-code outputs and original placement records.

All 112 maps pass: 3,439 retained placements, 3,127 resolved models, and 312 unresolved markers.
Central World 10000 has 26 records: 20 resolved visible models and six zero-shape records.
Unknown shapes appear as magenta crosses. The renderer does not invent their geometry.

Collision currently uses oriented model bounds. It remains provisional and can block empty space within a model's bounds.
The preview applies recovered health subtraction to enemy targets. Static scenery remains indestructible in this preview.
Campaign `wave` resources select `enem` spawn tables and `bval` parameters. These are needed in addition to static `wrld` placements.


## Damage and enemy preview

CODE 31 +0x0a84 contains the damage path. Its normal block starts at +0x0b2e.
It subtracts the damage word at frame +0x08 from the health word at participant record +0x5a.
The participant table pointer is A5-0x1342, with 256-byte records indexed from one.
Arena objects use A5-0x1346 and 128-byte records. These are separate structures.
The subtraction wraps to 16 bits. A negative result becomes zero.
The differential harness executes through +0x0b40 with synthetic records and damage values.
These tests cover the arithmetic block, not the complete damage routine.

The recovered block at +0x0b0c checks participant byte +8 for value 31.
For that value, CODE 6 +0x197e decides whether subtraction occurs.
The helper compares three signed-byte globals:

```text
A5[-0x25cf] == A5[-0x25ff] + A5[-0x262f] - 1
```

The addition uses signed words. It does not wrap to a byte before comparison.
If the comparison fails, subtraction is skipped. Negative health still becomes zero.
`participant_health_after_guard` preserves this block, including the helper predicate.
All 1,648 new cases execute the original helper and guard instructions without a substituted predicate.
They cover both participant kinds, matching and mismatching globals, signed boundaries, and health overflow.

These globals are entry 31 in three model tables:

| Global | Table base | Meaning |
| --- | --- | --- |
| A5-0x25cf | A5-0x25ee | Current model slot |
| A5-0x25ff | A5-0x261e | First model slot |
| A5-0x262f | A5-0x264e | Loaded model count |

CODE 19 +0x2358 initializes the current and first slots from the model allocation cursor.
CODE 1 +0x1328 passes the loaded model count to CODE 19 +0x23c6.
For type 31, the guard permits subtraction at the final loaded model slot.
The original type-31 animation resource has ten frames in the first Central arena.

CODE 19 +0x4b6c advances this shared model slot. `advance_type31_model` transcribes the complete routine.
Timer -1 means idle. Timer 0 selects first slot plus one and stores the current time.
Otherwise, the routine advances one slot only when `timer + interval < now` under signed, wrapping 32-bit arithmetic.
It sets the timer to -1 upon entry to the final slot. It does not catch up multiple frames per call.
CODE 19 +0x602a supplies interval 6. Equality at six elapsed ticks does not advance the slot.
All 504 differential cases match, including signed boundaries, wrapping, idle state, and the final-slot transition.

CODE 25 +0x2ea2 starts the transition by clearing its timer when the current slot is not -1.
CODE 16 +0x1bc8 dispatches action 24 to that routine through its switch table.
The caller at CODE 16 +0x0a7e reads the action from a 64-byte record at +0x3c.
CODE 16 +0x0a34 calls QuickTime `IsMovieDone` with selector 0x00dd and trap 0xaaaa.
Apple's [Universal Interfaces 2.2 Movies.h](https://github.com/elliotnunn/UniversalInterfaces/blob/master/2.2/Interfaces/CIncludes/Movies.h) confirms this opcode sequence.
When the movie finishes and record byte +0x3b is set, the code clears that byte and dispatches the action once.
Movie resource 1997 selects entries from `STR# 1997` in sequence, skipping empty and ineligible entries.
Entry 24 names `:VIV:VIV1.qt More robots go` and supplies action 24.
CODE 19 +0x5fac rejects that entry when the type-31 current slot is -1.
The movie scheduling and playback path remains outside the native simulation.
Earlier eligibility checks and later death effects remain outside the tested block.
The original entry also checks participant word +0x56 for -1 and a local-player timer threshold of 75 ticks.
The preview does not reproduce those checks.

CODE 25 +0x2352 copies participant byte +8 to the linked arena object's byte +0x14.
It also initializes participant health from the combat parameter table at +0xbe, indexed by type.
All 141 initialization checks pass with distinct marker values and multiple linked object indices.
CODE 30 +0x28c2 reads the hit object's participant index at +0x1a and resolves that participant's health.
The hit-type set includes both 7 and 31. The single-player path calls CODE 31 +0x0a84 at +0x29d4.
Thus the participant guard applies to these linked enemies too.
The preview applies the final-frame guard when the complete type-31 model list is available.
The explicit **Replay enemy unlock event** control starts the recovered transition through a shared type-31 timer.
This control substitutes manual activation for movie completion. It does not claim automatic event scheduling.
Runtime model slots are rebased to zero. Each simulation step advances the recovered routine and updates target geometry and vulnerability.
The host uses its provisional 60 Hz scheduler. Model bounds still drive provisional collision.
Reset restores the initial frame, timer, health, and immunity. Other animated types remain frozen.
The game test verifies protected hits, final-frame vulnerability, destruction, and reset.
The WASM test verifies both type-31 targets advance to frame 9 and regain immunity after reset.
CODE 25 +0x16e0 adds modifiers to the base health from `bval`.
Those modifiers remain unresolved. Preview health uses the base value, with a provisional minimum of one.

Each preview selects the earliest resource and sequence entry that references its map.
It uses the `enem` minimum counts and matching `bval` types.
Missing parameter entries are skipped and reported in the scenario details.
The first Central arena has two type-7 targets with one health and two type-31 targets with three health.
Basic-shot damage comes from the original weapon table. Spawn positions, collision, and effects remain provisional. Enemy AI, rebirth, and campaign progression are unfinished.
The WASM integration test destroys a target through normal fire input and verifies that reset restores all targets.

## Animation model lists

CODE 1 +0x15ba loads `ashp` resources.
The block at +0x15de reads a signed count word. The block at +0x15e8 reads frame IDs at two-byte intervals.
IDs 10000–10999 use the direct shape loader at +0x165e. IDs 1–999 use the procedural loader at +0x133a.
The loader stores successive frames at successive destination slots.
This establishes resource order, but does not establish runtime frame timing.

The shape-selection harness compares all 90 campaign frame IDs against the original instruction blocks.
It writes `.local/disassembly/animation-selection.json` for the scene test.
The asset viewer and static scene preview show the first original frame and label its source as frozen.
Live type-31 targets can advance through the recovered event transition described in Damage and enemy preview.
The scene test compares model bounds with the original frame selected by those instructions.


## Type-6 speed update

CODE 30 +0x1370 dispatches type 6 to +0x13f2.
This branch calls +0x11d6 before direction scaling for positive elapsed values, and after scaling for negative values.
The recovered `update_speed` function reproduces +0x11d6 through +0x136e.
Its names describe observed effects. Player-control mapping and elapsed units remain unresolved.

| Input | Original field |
| --- | --- |
| Current speed | Signed byte at object +0x22 |
| Acceleration command | Signed word at object +0x0e |
| Elapsed argument | Signed long at frame +0x08 |
| Speed limit | Signed byte at participant parameters +0x20 |
| Coasting parameter | Signed byte at participant parameters +0x23 |
| Temporary adjustment | Signed byte at A5-0x3d32 |
| Mode | Signed byte at A5-0x12ca |

The routine adds command times elapsed to speed, then stores the low byte.
A positive temporary adjustment moves speed toward zero, with a minimum magnitude of one in that branch.
It then limits speed in both directions. Test limits cover 0 through 127.
A negative adjustment temporarily replaces the coasting parameter with its absolute byte value.
The routine clears the adjustment global before returning.

With a zero coasting parameter and zero command, speed becomes zero.
With coasting values 1 through 5 and zero command, the routine calls the Macintosh Random trap.
When the signed random value is divisible by twice the coasting parameter, speed moves toward zero by the elapsed argument.
That update retains byte overflow and can cross zero. It does not clamp at zero.
Other coasting values bypass this random-decay branch.

When adjustment and mode are both zero, the routine copies A5-0x134c to A5-0x3d20 and A5-0x12f8.
The Rust result exposes this timestamp-write condition and whether the random branch requests a draw.
The caller must provide the random result. The original RNG algorithm is not recovered here.

The harness executes the complete routine and supplies only the Random trap result.
It compares speed, both timestamp writes, adjustment clearing, and the random-call condition.
The 1,200 cases include 189 random calls, 65 decay-triggering draws, and 103 timestamp resets.
The browser integrates this speed update with balanced Coast-0 settings. Full movement fidelity remains unfinished.

### Direction-table initialization lead

CODE 6 +0x0520 and +0x0530 request two tables with scale argument `0x4000`.
CODE 12 +0x01a6 allocates 360 long entries and scales the table at A5-0x6242 through CODE 11 +0x3c5a.
CODE 12 +0x1fa8 through +0x2062 constructs that base table through floating-point operations.
Those operations and their rounding still require recovery.
CODE 18 +0x0d24 reads the type-6 table through A5-0x38be.
Its first index is the heading high word. The second is 90 minus that word, or 450 minus it above 90.
Unlike +0x0dcc, this routine does not normalize its input.


## Type-6 planar translation

The Rust functions `movement_direction` and `move_type6_planar` reproduce the non-local planar path through CODE 30 +0x1370.
The test covers four (mode, local-participant) pairs: (-1, 2), (0, 1), (0, 2), and (1, 1).
The moving object belongs to participant 1. Local tests use ordinary ground state with no active extra displacement.
These conditions are explicit test constraints. They do not cover every local-player effect.

CODE 18 +0x0d24 selects two values from the table at A5-0x38be.
CODE 30 +0x1412 through +0x143a multiplies each by the signed speed byte.
CODE 30 +0x1d24 through +0x1d3e multiplies each result by elapsed time and adds it to the position.
Both multiplications and the position addition wrap at 32 bits. There is no additional fixed-point shift in these blocks.
The direction table itself supplies the scale.

Positive elapsed values update speed before displacement. Negative elapsed values update speed after the direction scaling.
Zero elapsed leaves speed and position unchanged and does not consume a random draw.
Collision response belongs to a later caller and is outside this recovered path.

The harness executes the complete original +0x1370 routine under these constraints.
It supplies a 360-entry marker table and compares both coordinates, final speed, and random-call conditions.
The 4,320 cases cover every integer heading with random fractional parts and elapsed values -4, 0, and 4.
Original trigonometric values, local-view effects, and additional mode branches remain unfinished.

## Clock and scheduler trace

CODE 33 +0x0058 calls trap `0xa975`, then adds A5-0x4c22 to the returned long at +0x005c.
At +0x0188, it stores that value in A5-0x134c.
At +0x01d0, it passes that timestamp to CODE 30 +0x2d96.
The pause branch at CODE 33 +0x0178 adjusts the offset instead of advancing that path.

CODE 30 +0x2d96 compares the timestamp with A5-0x3d14.
For each catch-up iteration, it advances by the smaller of four and the remaining signed-word difference.
It calls +0x2d2a with each intermediate timestamp, then stores the final timestamp.
That routine dispatches participant objects through +0x2700 and other objects through +0x2bbe.
The +0x2700 path takes a signed-word timestamp difference and clamps it to 0–63.
Thus the original scheduler can group elapsed ticks into steps of up to four.
The browser's fixed single-tick stepping does not yet reproduce that grouping.

Apple documents TickCount as approximately 60 ticks per second, with no exact timing guarantee.
See [Apple's TickCount reference, mirrored](https://dev.os9.ca/techpubs/mac/Toolbox/Toolbox-80.html).
Long stalls, clock wraparound, pause offsets, and network modes still require dynamic verification.


## Coast setting and original vehicle presets

The disc provides direct evidence for the name Coast.
`DITL 2300`, used by the Custom Spectre Settings dialog, contains the label `Coast:`.
Item 17 references `CNTL 8100`. Its resource declares minimum 0 and maximum 7.
This is a control range, not a recovered menu of named options.

CODE 23 +0x0432 selects item 17 and initializes its value from settings byte +3.
Its caller at +0x04cc passes participant settings +0x20.
The dialog event path at +0x0368 reads the control value and writes participant byte +0x23 at +0x038c.
CODE 26 +0x0ac2 copies the participant's 40-byte settings record into the runtime table at A5-0x3f8e.
CODE 30 +0x12d6 reads runtime byte +0x23 for the speed-update branch.
This connects the original UI label with the recovered coasting behavior.

CODE 22 +0x0132 initializes the vehicle settings. The recovered `select_vehicle_settings` preserves its byte operations.
The following values come directly from its immediate operands and agree with the vehicle-selection dialog text:

| Selector | Speed | Shields | Ammo | Coast |
| --- | ---: | ---: | ---: | ---: |
| 3 | 5 | 6 | 4 | 0 |
| 4 | 8 | 4 | 3 | 0 |
| 5 | 3 | 8 | 4 | 0 |
| 6 | Saved custom | Saved custom | Saved custom | Saved custom |

CODE 22 +0x0004 initializes saved custom speed, shields, and ammo to 5, and Coast to 0.
Later saved preferences can replace these defaults.
Selectors 3–5 always choose Coast 0. Thus their unmodified settings do not enter the random coasting branch.
Coast 0 clears speed when the acceleration command is zero.
Coast 1–5 permits random decay. Coast 6–7 bypasses this decay branch.
Other effects can still modify speed, including the temporary adjustment global.

The harness executes the complete settings-selection routine for 11 selectors and all eight Coast values.
All 88 results match Rust, including the seven output bytes and the preserved fields for unknown selectors.
The browser now selects preset 3 for its recovered-handling preview. Other presets are not exposed yet.


## Keyboard command path

CODE 30 +0x0cc6 samples the key bitmap and fills thirteen six-byte records at A5-0x6698.
Each record has a key-code byte at +2 and a pressed word at +4.
The sampling loop writes exactly 0 or 1 to each pressed word.
CODE 31 +0x15d0 can also update these words from mouse movement.
The recovered `keyboard_commands` covers +0x1764 through +0x181c, after those updates.

The following indices are zero-based within the thirteen-record array:

| Result | Pressed slots | Object field |
| --- | --- | --- |
| Positive throttle | 10, 11, or 12 | Word +0x0e becomes 1 |
| Negative throttle | 5, 6, or 7, without positive throttle | Word +0x0e becomes -1 |
| Negative turn | 7, 9, or 12 | Word +0x0c becomes -1 |
| Positive turn | 5, 8, or 10, without negative turn | Word +0x0c becomes 1 |

With no matching slot, the corresponding command becomes zero.
Only values equal to 1 count as pressed. Other nonzero words do not count.
This establishes command priority, but does not yet map every slot to its default physical key.
Earlier branches handle stop input, disabled objects, and temporary movement effects.
Later branches handle firing and other controls. Those branches are outside this recovered block.

The harness checks all 256 directional combinations and 100 cases with noncanonical pressed words.
Both command words match original instruction execution in all 356 cases.

### Turn-rate caller

CODE 30 +0x257c reads the turn command and calls CODE 23 +0x05b8 for the participant's rate.
At +0x2594 and +0x259a, it multiplies command, rate, and elapsed time using wrapping long multiplication.
That result becomes the heading delta passed to +0x10b8.
Positive elapsed values turn before translation. Nonpositive values translate before turning.

For the participant-rate path, CODE 23 reads participant long +0x6c, with an optional division by two.
CODE 25 +0x1322 through +0x1342 initializes this field to `(8 * 0x4000 + 0x10000) / 3`, or `0x10000`.
That is one fixed-point degree per elapsed unit before the turn ramp or optional halving.
The machine-type flag controlling halving is recovered in the next section. Other rate sources still need caller-level validation.


## Machine-dependent turning rate

CODE 25 +0x265a calls the system-environment wrapper at CODE 1 +0x2994 with requested version 1.
The wrapper calls trap `0xa090` when available, or uses a compatibility fallback.
The caller reads the environment record word at +2, which is `machineType` in Apple's `SysEnvRec` layout.
See [Apple's SDK OSUtils.h, mirrored](https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX10.6.sdk/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/CarbonCore.framework/Versions/A/Headers/OSUtils.h).

CODE 25 +0x2668 through +0x269a sets A5-0x3cf8.
A successful environment query sets this flag for machine types 1, 2, 3, 8, and 15.
Other types clear it. A failed query sets it regardless of machine type.
CODE 23 +0x05b8 uses this flag to halve the selected turning rate.
The rationale for these machine choices is unknown. No claim about performance compensation is established.

The recovered `half_turn_rate_for_machine` matches 75 original-instruction cases, including unknown types and environment errors.
The native port must choose an explicit compatibility target when it connects the recovered handling.
Host CPU speed must not silently select the original machine-dependent branch.

### Local-player movement coverage

The expanded translation harness executes CODE 30 +0x1370 with participant 1 selected as the local player in modes 0 and 1.
It initializes the view-height state to ground values and clears participant state and extra displacement.
The same planar Rust calculation matches both coordinates and final speed in these contexts.
The harness does not compare view-height side effects. Jumping, shield-related visual motion, and active extra displacement remain outside its coverage.


## Candidate direction-table generator

The original initializer's conversion sequence is now transcribed in `candidate_movement_table`.
This candidate is separate from the original-machine-code comparisons counted earlier.
The browser now uses this candidate, with its validation limitation stated in the preview.

CODE 12 +0x2088 stores extended constant `3ff98efa2d286f138459`.
Its value is approximately 0.01745327777777778, corresponding to the truncated pi value 3.14159 divided by 180.
The loop begins at zero. Each iteration calculates sine, stores it as single precision, and advances the angle.
The angle increment uses the extended constant, followed by another single-precision conversion.
The sine result is multiplied by 65536 and converted to a signed integer with rounding toward zero.
CODE 11 +0x3c5a then scales that integer by fixed-point 0x4000, again truncating the magnitude.

The research harness executes the original CODE 12 loop, CODE 35 conversion wrapper, and CODE 11 scaling instructions.
Only the requested SANE services are modeled:

- Extended binary arithmetic with explicit 64-bit significand rounding.
- Single-precision storage through binary32 conversion.
- Sine evaluated with an 80-decimal-digit Taylor calculation.
- Toward-zero integer conversion and the environment operations used by the wrapper.

The model covers this bounded initialization workload. It is not a general SANE implementation.
It assumes the initial floating-point environment uses nearest rounding.
It does not reproduce or verify the original SANE transcendental implementation.
Python float intermediates are also part of its single-precision conversion model.

Reproduce the candidate comparison:

```sh
.local/re-tools/bin/python scripts/recover-direction-table.py
cargo run --quiet --locked -p spectre-tools -- verify-direction-table .local/disassembly/direction-table-candidate.json
```

All 360 Rust candidate values match this modeled-service run.
The report records source hashes, trap counts, base values, movement values, and its validation limitation.
An independent original-SANE run or runtime memory capture remains necessary before claiming original table identity.
The next integration must retain that distinction if it uses this candidate before that validation.

The SANE operation selectors are documented in Apple's [Inside Macintosh SANE appendix, mirrored](https://fr.scribd.com/document/188420555/Inside-Macintosh-Promotional-Edition-1985).


## Playable recovered-handling integration

The shared `Game` uses the recovered speed update, heading ramp, and planar translation each tick.
It keeps position and heading in fixed-point integers between frames.
Preset 3 supplies the speed limit and Coast 0. The preview explicitly selects the full machine-dependent turn rate.
Host axes become signed digital commands. Shift forces zero throttle and is a preview convenience.
Original keyboard conflict priority, complete mouse handling, and stop-key side effects are not yet connected.

The world-coordinate renderer uses sine for X and cosine for Z.
The adapter maps its heading to the original table convention with `original = 90 degrees - rendered`.
Shots use recovered launch and motion arithmetic with candidate direction values. Their scheduling remains provisional. Static and enemy collision still uses oriented model bounds.
On contact, the preview rejects translation and clears speed. Original collision response remains unfinished.

The integrated replay checks 210 ticks against original heading and translation instructions.
It covers acceleration, turning while moving, stop, reverse, and turning in place in an empty arena.
Both runs use the candidate direction table, balanced settings, and ordinary local-player ground state.
All fixed-point coordinates, headings, speed bytes, and ramp bytes match exactly.
This is conditional integration evidence, not an original-SANE table validation or full-game equivalence claim.

```sh
.local/re-tools/bin/python scripts/recover-direction-table.py
.local/re-tools/bin/python scripts/verify-handling.py
```

All 112 arenas still pass browser-module spawn and render checks.
The 240-tick native/WASM gameplay replays agree within 0.001 units, and the firing/destruction/reset checks pass.


## Weapon damage tables

CODE 25 +0x1902 initializes four 18-word banks in the combat parameter record at A5-0x1350.
The damage banks at +0x75a and +0x77e start at one. Ammo banks at +0x7a2 and +0x7c6 start at zero.
It loads `wval 10000` and copies each entry into both corresponding banks.
The index is the signed low byte of the resource's type word. Indices outside 0–17 are skipped.
Later duplicate entries replace earlier entries. A missing resource retains the defaults.
`weapon_tables` preserves these rules. It returns one initial damage bank and one initial ammo bank because both copies initially match.

The verifier executes the complete original routine and supplies only resource lookup, handle locking, unlocking, and release services.
Its 106 cases include all three original campaign resources, missing resources, duplicates, signed boundaries, and low-byte aliases.
All four original memory banks match Rust.

CODE 31 +0x1874 passes weapon 0 to the firing routine for the primary fire path.
CODE 30 +0x290a–0x294a selects damage using the projectile's weapon byte at +0x1d and its owner's participant byte +5.
An absent owner selects bank +0x75a. A positive owner with byte +5 equal to zero selects bank +0x77e.
Both banks initially contain the resource damage, but later modifiers can make them differ.
CODE 25 +0x2f32, for example, multiplies weapon-0 damage in the second bank by four.
The preview uses unmodified weapon-0 damage from the campaign resource. The preview consumes weapon-0 ammo through the ordinary local-player path described next.

The primary-fire caller also tests signed elapsed time greater than 10 before requesting a shot.
That caller does not establish the complete firing cadence: CODE 30 +0x08b2 still contains additional eligibility and ammunition checks.
The preview uses recovered primary-fire timing. Collision and update ordering remain provisional.


## Local firing eligibility and ammunition

CODE 28 +0x0004 reads the weapon cost from bank +0x7c6 and the local participant from A5-0x133d.
The participant byte argument is unused in this routine. The local index selects its record.
The routine permits firing when ammo +0x5e is at least the cost, health +0x5a is positive, and the inhibit byte permits it.
The inhibit byte is A5-0x2d70. The routine returns that byte XOR 1 after the two numeric checks succeed.
The Rust helper preserves noncanonical byte results. Tests compare raw bytes rather than normalizing them to booleans.
The original signed condition follows subtraction flags, including overflow. Comparing the wrapped difference with zero would be incorrect.
All 572 cases match complete original execution, including signed extremes and noncanonical flags.

CODE 30 +0x08b2 first checks A5-0x12d2. If clear, it updates the last-attempt timestamp before calling the eligibility predicate.
The actual emission path at +0x01c0 also checks the participant's active-shot count against A5-0x133c.
For an ordinary local participant with byte +5 zero, insufficient ammo aborts emission.
When A5-0x12c5 is zero, +0x025c subtracts the cost with signed-word wrapping.
Allocation failure can refund the cost at +0x028a. Those capacity, allocation, and special-mode paths remain outside the preview.

CODE 25 +0x13f8 initializes ammo from the signed ammo-setting byte multiplied by ten.
The balanced preset's ammo setting is four, giving 40 initial rounds.
The preview starts and resets with 40 rounds, consumes the resource cost, and stops firing when ammo is insufficient.
The player is assumed alive and firing inhibition is clear. Incoming player damage remains unfinished.
Rust and WASM gameplay tests verify exhaustion at 40 basic shots and refilling on reset.
Firing cadence remains the provisional 12-tick interval. This is not yet the complete original firing path.


## Basic projectile launch and motion

CODE 30 +0x0794–0x07e8 initializes an ordinary player-owned weapon-0 shot.
It places the shot six units ahead, using the unit direction table.
The expiry timestamp is launch time plus the signed lifetime word.
CODE 25 +0x273a initializes that lifetime to 75 through CODE 23 +0x12.

CODE 6 +0x0500–0x052c builds separate unit, projectile, and movement direction tables.
Their scales are 1, 6, and 1/4. Scaling the movement table by 24 loses precision.
CODE 30 +0x24fc–0x252a adds projectile direction times elapsed ticks to position.

The differential harness checks 300 launch cases and 360 movement cases against original instructions.
Movement cases enter the type-9 dispatcher and stop before the spatial-grid update.
The launch cases cover signed overflow and lifetime boundaries.
All 660 cases match Rust. The total is 30,980 cases, plus 141 enemy linkage checks.

The playable build uses these launch and movement calculations with candidate direction values.
The base table matches the modeled-SANE initializer. Original SANE remains unverified.
Collision uses provisional substeps between fixed-point endpoints.
The preview uses the recovered expiry and elapsed-time rules described below.
The full original scheduler and collision dispatch remain unfinished.


## Primary-fire attempt timer

CODE 31 +0x1862–0x1874 subtracts the last attempt timestamp from the current timestamp.
The subtraction wraps at 32 bits. A signed comparison requires the result to exceed ten.
The held-fire input must also be active. The ordinary weapon-0 call follows at +0x1876.

CODE 30 +0x08be–0x08ce checks the global disable byte at A5-0x12d2.
A nonzero byte skips the attempt without changing the timestamp.
Otherwise, the routine copies current time into A5-0x1376 before it checks ammunition.
Thus, an empty-ammo attempt delays the next attempt too.
CODE 25 +0x2720 initializes the last attempt timestamp to current time.

The harness executes 518 timer cases and 256 disable-byte cases against original instructions.
Cases cover the ten/eleven-tick boundary, signed overflow, released input, and every disable-byte value.
All 774 cases match Rust. The total is 30,980 cases, plus 141 enemy linkage checks.

The preview uses this rule for basic fire, with the global disable flag clear.
Reset initializes both relative timestamps to zero.
The first attempt occurs at tick eleven. Held fire then attempts every eleven ticks.
An integration test verifies the delay after an empty-ammo attempt and a subsequent refill.
WASM checks also verify the initial boundary. Special-weapon timing and original update ordering remain unfinished.


## Basic projectile expiry and elapsed time

CODE 30 +0x2bbe scans the object table. Type 9 enters the expiry branch at +0x2c10.
The branch compares the signed deadline directly with current time at A5-0x134c.
A deadline greater than current time selects movement at +0x2c94.
Otherwise, weapon 0 selects removal at +0x2c8c. Equality expires the shot before movement.
This comparison differs from a wrapping elapsed-time comparison near signed overflow.

The harness executes this dispatcher through the movement or removal branch for 349 timestamp pairs.
The original Pascal set helpers also execute. The removal and collision routines do not execute in this test.
All cases match Rust, including equality and signed timestamp boundaries.

CODE 30 +0x271a–0x273a calculates movement time from the supplied timestamp and object field +0x10.
It takes the low signed word of the difference, then clamps it between zero and 63.
The harness executes this block and its original min/max helpers for 317 cases. All match Rust.
CODE 30 +0x2a72 stores the supplied timestamp after the update.
Launch initializes the same field at +0x0804, so movement at that timestamp has zero elapsed time.

The playable loop uses these rules. It retains the launch deadline and last-update timestamp for each shot.
A native integration test follows a shot through its deadline and verifies removal before a possible boundary impact.
The full suite contains 30,980 differential cases, plus 141 enemy linkage checks.
Collision substeps and the overall scheduler remain provisional.


## Projectile collision query and rectangle overlap

CODE 30 +0x2782 calls CODE 7 +0x0ad6 after projectile movement.
The caller supplies the old pose, elapsed ticks, and object index.
CODE 7 +0x0b00–0x0b1a builds a pose at the midpoint of old and new positions.
The midpoint uses wrapping addition and a logical right shift. Heading comes from the current object.

At +0x0b3e–0x0b64, the query receives dimensions of `6 * elapsed + 3` and `1.5`, in fixed-point units.
The query excludes the owner participant's object, or the current object when owner is zero.
CODE 7 +0x06ba searches the spatial grid, then tests rectangle overlap at +0x0946.
Grid cells occupy 42 bytes: a count word followed by object indices. Row stride is 672 bytes.
The scan also filters object types and selects between multiple hits at +0x0952–0x0976.
The filter and pairwise hit preference are recovered below. Grid traversal is not integrated yet.

Candidate dimensions come from A5-0x23e2 and A5-0x2572, indexed by the selected model slot minus one.
CODE 6 +0x18ea resolves that slot from the object type, with special handling for type 42.
CODE 18 +0x102a calls CODE 12 +0x0944 to populate the dimension tables.
Several CODE 19 paths override table entries. Live collision must account for those overrides.

The recovered `collision_rectangles_overlap` function transcribes the complete CODE 7 +0x0250 routine.
It calls the separating-axis routine at +0x00a0 for each rectangle.
The transcription preserves quarter-unit position truncation, signed table words, dimension rounding, and wrapping arithmetic.
It accepts direction tables explicitly. It does not assume the movement table is the collision table.

The differential harness executes both original routines and the original direction lookup for 1,000 cases.
Distinct signed marker tables test indexing independently from trigonometric accuracy.
Cases include coincident poses, different headings, fractional dimensions, and wide signed arithmetic.
All match Rust. The full suite contains 30,980 cases, plus 141 enemy linkage checks.

This rectangle helper is not connected to live collision yet.
The next work is model dimension recovery, candidate filtering, and multiple-hit selection.
The playable build retains its documented provisional collision checks.


## Model collision dimensions

CODE 12 +0x0944 reads the selected runtime vertex list through A5-0x6490 and A5-0x648e.
The list starts with a signed vertex count. Each vertex occupies ten bytes.
The dimension calculation reads signed words at vertex offsets zero and four.
Each word is converted to 16.16 fixed-point and divided by eight.
The routine accumulates twice the largest absolute coordinate on each planar axis.
Thus, the rectangle is centered on the model origin. It is not a min-to-max bounding box.

`runtime_collision_dimensions` transcribes the two planar outputs.
The harness executes the original dimension loop, division helpers, multiplication helper, and maximum helpers.
It bypasses the independent radius square-root routine at CODE 12 +0x00c4 with a zero result.
The radius output is not compared or claimed as recovered.
205 cases cover empty lists, asymmetric vertices, signed-word limits, and randomized lists.
All planar outputs match Rust. The original routine also clears the selected-list counter after processing.

Resource vertices still require the original conversion into packed runtime coordinates.
CODE 12 +0x0532 packs resource coordinates. CODE 19 +0x029c belongs to a separate procedural path and shifts by seven bits.
The trap rounding behavior and model-specific dimension overrides require further verification.
The recovered dimension helper is not connected to live collision yet.


## Resource vertex conversion

CODE 1 +0x16c8–0x1716 reads resource coordinates and passes each vertex to CODE 12 +0x0532.
CODE 19 +0x3f4a selects scale 1 for this path.
The vertex routine uses the software fixed multiply, shifts each result left by three, then calls trap A86C.
It stores the rounded coordinates as three signed words in the runtime vertex record.
The runtime list accepts up to 60 vertices. The new helper covers conversion only, not list allocation or capacity.

Apple's ToolUtils interface identifies A86C as FixRound.
The Apple reference specifies nearest-integer rounding, with half-way values rounded toward the higher integer.
For example, negative 3.5 becomes negative 3.
Sources: [ToolUtils interface](https://github.com/elliotnunn/UniversalInterfaces/blob/master/2.2/Interfaces/CIncludes/ToolUtils.h)
and [Apple mathematical utilities reference, page 36](https://leopard-adc.pepas.com/documentation/Carbon/Reference/Mathematical_al_Utilities/math_logic_utils.pdf).

`candidate_runtime_vertex` uses this documented rounding rule.
The harness executes the original vertex routine and software fixed multiply, but models the FixRound trap.
1,368 conditional cases pass, including 1,157 unique vertices from the three campaigns' shape resources.
These are not original-OS trap comparisons. Extreme FixRound overflow behavior still needs runtime verification.
The suite reports 30,980 differential vectors, of which 1,368 explicitly use the modeled FixRound trap.
Other existing qualifications, including supplied Random results and the bypassed radius calculation, still apply.

The conversion helper remains separate from live collision pending dimension overrides and candidate filtering.


## Projectile candidate filter and hit preference

CODE 7 +0x08b4–0x0914 filters candidates before rectangle overlap.
It rejects the excluded object index, then compares integer coordinate distances with the scan extents.
Coordinate subtraction and absolute value use signed words, including wrapping at the word boundaries.
The embedded set before +0x0ad6 excludes types 5, 9, 10, 12, 28, 29, 30, 33, and 41.
The supplied executable's set bytes are `02 02 70 00 16 20`.
This is a bit set, not a jump table.

The `projectile_candidate` helper matches 2,038 executions of the original filter.
Cases cover all 256 type bytes, self-exclusion, coordinate boundaries, and signed-word overflow.
The harness stops at either the rectangle-test path or the next-candidate path.
It does not replace the rectangle test with an assumed hit.

CODE 7 +0x0618 compares two possible hits along the query direction.
It projects each object's integer coordinates onto signed direction-table words.
Multiplication results accumulate with wrapping 32-bit addition.
A candidate replaces the existing hit only when its projection is strictly smaller.
Equal projections retain the existing hit, so grid traversal order matters for ties.

The `prefer_projectile_hit` helper matches 500 executions of the complete original comparison routine.
These include 50 equal-position cases and signed marker directions.
Both helpers remain separate from live collision pending grid traversal and dimension overrides.
The complete suite contains 30,980 differential vectors, including 1,368 modeled-FixRound cases.


## Projectile grid bounds

`projectile_grid_bounds` transcribes CODE 7 +0x06ba–0x0828.
The query rounds each fixed-point dimension upward to a signed word, preserving overflow.
For a summed dimension of 35 or less, both scan extents equal that sum plus 36.
Larger queries project the dimensions through the direction tables, then add 36 to each extent.

Cell coordinates use 64-unit divisions. The upper limits are capped at the grid dimension minus one.
The lower calculation first wraps coordinate-minus-extent to an unsigned word, then shifts right by six.
Its subsequent signed maximum with zero cannot undo that wrap.
Thus, a lower coordinate near zero can produce a minimum greater than the maximum.
Such a range is empty in the original loop and must remain empty in the port.

The harness executes the complete bound-calculation prefix, including original lookup and min/max routines.
540 cases cover the short/long query boundary, headings, arena edges, signed words, and overflow.
All match Rust. The complete suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.

Grid traversal proceeds through ascending first-axis cells, then ascending second-axis cells, then stored object entries.
Cells use a 672-byte first-axis stride and a 42-byte second-axis stride.
This traversal order is directly compared in the following section.


## Projectile grid traversal

`projectile_grid_candidates` preserves CODE 7 +0x0828–0x09ae traversal for valid 16-by-16 grids.
It visits ascending X cells, ascending Z cells, then stored entries in each cell.
Duplicate object IDs remain in the output. Empty coordinate ranges produce no candidates.
Each cell has room for 20 object IDs after its count word.
The Rust helper rejects malformed nonempty ranges and oversized cells instead of reading outside the supplied grid.
This rejection is a host-memory safeguard, not a recovered game behavior.

75 comparisons execute the original loop instructions and object-index loads.
A hook records each loaded index and skips candidate hit testing.
The tests isolate traversal; they do not establish a complete collision result.
Fixtures include the full grid, single corner cells, duplicate IDs, full cells, and empty ranges.
All output sequences match Rust. The full suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.

Live integration still requires grid population and movement updates, plus collision-dimension overrides.
Relevant grid mutation paths include CODE 25 +0x043a, +0x0e8a, and CODE 30 +0x0ffe.


## Object cells and movement between cells

CODE 11 +0x3eee converts an object's planar position into cell coordinates.
It takes each signed high word, shifts it arithmetically by six, and clamps it to 0–15.
Unlike the projectile query-bound calculation, negative object positions clamp to zero.
The Rust `object_grid_cell` helper matches 308 original-routine cases.

CODE 30 +0x0f60 compares old and new cells. Equal cells leave grid entries unchanged.
For different cells, it finds the first matching object ID in the old cell.
It replaces that entry with the cell's last entry, then reduces the count.
It appends the object ID to the new cell even when the old cell had no matching ID.
Later duplicate entries in the old cell remain.
This operation can change collision tie order after movement.

`move_grid_object` matches 200 executions of the complete type-9 movement routine.
The tests compare every grid cell after execution, including duplicates and missing old entries.
They exercise destinations with available capacity. The original move routine does not check the 20-entry limit here.
The Rust helper rejects a full destination before mutation to protect host memory.
That capacity rejection is a host safeguard, not an original gameplay rule.

Participant cell fields and pose writes are outside this helper's contract.
Initial insertion and object deletion still need separate recovery and comparison.
The complete suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.


## Projectile insertion and grid unlinking

CODE 30 +0x088c–0x08a4 inserts a newly emitted projectile into its cell.
It increments the count and appends the object ID when the new count is at most 20.
For an already full cell, it restores count 20 and leaves all stored entries unchanged.
This differs from the unchecked append in the moving-object path.
`insert_projectile_grid_entry` reproduces this behavior for valid cells.

CODE 25 +0x0ea4–0x0eda unlinks the first matching ID from a cell.
It replaces the matching entry with the last entry and decrements the count.
A missing ID leaves the cell unchanged. Later duplicate IDs remain.
The movement helper now shares this verified `remove_grid_entry` operation.

968 comparisons execute these original blocks for all valid cell sizes, full cells, missing IDs, and duplicates.
All active entry lists and counts match Rust. Inactive trailing storage is not part of the Rust vector contract.
The complete suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.

These checks cover cell mutation, not complete object deletion.
CODE 25 +0x0f4a copies the last object into the deleted slot.
The subsequent grid scan replaces references to the old last-object index with the deleted index.
It marks the old tail slot inactive and decrements the object count.
That compaction and participant bookkeeping still require comparison before live integration.


## Complete projectile deletion

`delete_projectile_record` transcribes CODE 25 +0x0e62 for projectile kinds 9, 28, 29, 32, 33, 34, 43, 44, and 46.
It unlinks the deleted ID from its cell and decrements its owner's signed active-shot count.
The decrement wraps at 16 bits. Owner zero skips participant bookkeeping.
It then copies the complete 128-byte tail record into the deleted slot.

For a copied participant-backed kind with a nonzero owner, it repairs participant field +0x56.
The moved object's cell replaces every reference to the former tail index with the deleted index.
The old tail slot becomes type 14 with owner zero. Other tail bytes remain unchanged.
Finally, the global object count decreases by one.
Deleting the tail itself follows the same instruction path.

200 comparisons execute the complete original routine without service stubs.
Each comparison checks all object bytes, all twelve participant records, every active grid entry, and the final count.
Fixtures include tail deletion, duplicate grid entries, multiple projectile kinds, and different copied tail kinds.
All match Rust. The suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.

The Rust helper rejects unsupported non-projectile deletion before mutation.
That path calls participant deactivation through CODE 25 +0x0e2e and still requires recovery.
The recovered deletion helper is not connected to the preview's object storage yet.


## Participant-backed object deletion

The generalized `delete_object_record` includes the deactivation branch of CODE 25 +0x0e62.
Participant-backed kinds are 6, 7, 8, 10, 13, 20–27, 31, 35, and 45.
For these kinds, CODE 25 +0x0e2e clears participant byte +0x88.
It then calls CODE 14 +0x2e20, which copies that zero into A5-0x4c40 indexed by owner minus one.
This call has no further dependencies.
Other non-projectile kinds skip both participant deactivation and active-shot decrement.

The harness now executes CODE 14 directly. No deactivation service is stubbed.
141 additional cases cover each object kind from zero through 46 with three deletion fixtures.
They compare the full object array, all participant records, the status-byte table, every cell, and object count.
All pass alongside the 200 projectile-deletion cases.
The generalized Rust entry point requires valid participant owners for participant-backed records.

This verifies object removal, not the entire enemy-death sequence.
Death effects, respawn, and the scheduler's handling of a moved tail object still require tracing.
Live integration still needs the recovered object storage and collision-dimension overrides.
The full suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.


## Collision-dimension override audit

The direct references to the two planar dimension tables identify six CODE 19 override blocks.
The Rust `CollisionDimensionOverride` operations preserve their arithmetic.
They describe loader edits, not unconditional object-kind rules.

| Block | Selected slot | Planar operation |
| --- | --- | --- |
| +0x220e–0x223a | Current type-4 slot | Set both dimensions to 10 units |
| +0x2c4a–0x2ca2 | Current type-18 slot | Signed division by two, toward zero |
| +0x3cbe–0x3d0a | Current type-9 slot | Double with 32-bit wrapping |
| +0x42d4–0x4320 | Current type-28 slot | Double with 32-bit wrapping |
| +0x5b42–0x5b9a | Current type-16 slot | Signed division by two, toward zero |
| +0x14c4–0x1508 | Current type-12 slot | Copy both dimensions from the type-2 slot |

600 comparisons execute all six blocks with randomized slot indices and signed dimensions.
The division cases execute the original CODE 36 helper. No arithmetic service is replaced.
All results match Rust. Radius-table edits beside these blocks are outside this planar comparison.
The complete suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.

The model loader must select these operations through the original branch conditions.
A resource replacement does not necessarily follow the corresponding procedural loader path.
Live integration must preserve that distinction and slot aliases.


## Procedural selector routing and campaign overrides

`procedural_model_entry` transcribes the dispatcher at CODE 1 +0x133a.
It returns the first CODE 19 builder entry, rather than treating a selector as an object type.
1,004 comparisons execute the original dispatch and stop at the first builder call.
They cover selectors 1–999 and signed boundary values. All match Rust.
Selector 7 and unsupported selectors make no builder call here.
Selector 21 starts at +0x4d38, then calls +0x4f5c and +0x5178; the helper reports only the first entry.

The third shape-table column is identical for the relevant types in all three campaigns:

| Object type | Selector | Relevant path |
| --- | --- | --- |
| 2 | 3 | Builder +0x3328 |
| 4 | 5 | Builder +0x1e0e sets ten-unit dimensions |
| 9 | 9 | Builder +0x3a7c doubles dimensions |
| 12 | 12 | Builder +0x0fd4 copies dimensions from type 2 when slots exist |
| 16 | 10024 | Resource path; the outer type-16 loader has a separate halving edit |
| 18 | 10095 | Resource path, bypassing procedural builder +0x28d8 |
| 28 | 18 | Builder +0x3fc2 doubles dimensions |

Thus, the type-18 halving block does not apply merely because an object has type 18.
It belongs to procedural selector 15. The campaign's resource selector bypasses that builder.
The outer loader at CODE 19 +0x5a80 also skips already-loaded slots.
Its type-16 path loads the selected resource and then halves its dimensions.
Slot aliases and initialization order must remain explicit during live integration.
The full suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.


## Final projectile hit rejection

CODE 7 +0x09ae–0x0ab2 can discard the selected hit after grid traversal.
The Rust helper `finalize_projectile_hit` preserves these rules:

- With positive selected and excluded IDs, a type-6 hit is rejected for excluded types 21, 23, 24, 35, and 45.
- With positive selected and excluded IDs, excluded types 35 and 45 reject every selected type.
- A positive selected ID is rejected when its positive owner references participant byte +4 equal to 3.
- A positive excluded ID also clears the result when its positive owner references participant byte +4 equal to 3.

The last rule runs even when the selected ID is zero or negative.
Signed owner bytes at zero or below bypass participant lookup.
The gameplay meaning of participant state 3 remains untraced.
The type sets are packed constants ending at +0x0ad0 and +0x0aca, respectively.
They are not jump tables.

The harness executes the original tail with 3,816 input combinations.
These cover all type pairs from 0 through 47, all 256 participant state bytes, and randomized signed IDs and owners.
All results match Rust. The suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.
This helper remains outside live gameplay until the complete collision query and its model data are ready.


## Complete projectile collision query

`CollisionQueryWorld::projectile_hit` composes the full CODE 7 +0x06ba routine for the original 16-by-16 grid.
It preserves candidate order, duplicate IDs, dimension lookup, rectangle overlap, hit preference, and final rejection.
Object IDs and model slots remain one-based.
Invalid references return `None` as host guards; the original routine assumes valid runtime data.

The query calls CODE 6 +0x18ea to select each model slot.
Ordinary objects use their type-indexed slot byte at A5-0x25ee.
Type 42 uses the object ID at record +0x44.
If the linked object has type 10, the selector returns its base slot.
Otherwise, it adds the signed animation byte at record +0x3e to the linked object's base slot.
A nonpositive linked ID returns -1.

The harness compares 800 complete original queries with Rust.
Each fixture supplies 20 object records, model slots, dimensions, participant states, and a populated grid.
Fixtures include duplicate IDs, shuffled cell order, type-42 links, fractional query width, and arena edges.
The original routine returns 592 hits and 208 no-hit results. Every result matches Rust.
No instructions are bypassed, and these cases require no OS trap models.
Direction tables use synthetic markers. Model dimensions and state bytes are supplied inputs.
The complete suite contains 30,980 vectors, including 1,368 modeled-FixRound cases elsewhere in the suite.

The query remains separate from live gameplay.
Original direction-table initialization, model-slot construction, and the projectile caller still need integration.


## Independent collision direction tables

CODE 6 +0x0550–0x061a builds the inline collision tables independently of the movement table.
The stored extended constant at +0x064c rounds to `0.017453` in f64.
Each iteration converts the integer degree to extended precision and multiplies it by that constant.
Separate sine and cosine calls retain extended results.
The loop multiplies each result by 65,536, truncates it to a signed integer, and divides by four through CODE 36.
It stores cosine at A5-0x38b2 and sine at A5-0x3312.
This path has no accumulated angle or single-precision intermediate values.

`candidate_collision_table` matches all 360 pairs from the original initializer with modeled SANE services.
The harness now models cosine and signed-word conversion in addition to its previous services.
Original CODE 35 wrappers and CODE 36 division execute normally.
The original OS transcendental implementation and floating environment remain unverified.
The collision sine column differs from the candidate movement table at 151 indices.
Reusing the movement table would lose this observed distinction.

The complete query now has 800 additional comparisons using these modeled-SANE table values.
The original query returns 629 hits and 171 no-hit results for these fixtures. Every result matches Rust.
Together with the original marker-table fixtures, the query has 1,600 complete comparisons.
The suite contains 30,980 vectors. Of these, 1,368 execute an explicitly modeled FixRound trap.
The additional query cases consume candidate table inputs; they do not independently validate original SANE.
Live integration still needs model-slot construction and the original projectile caller.


## Basic projectile collision caller

`CollisionQueryWorld::basic_projectile_hit` ports CODE 7 +0x0ad6 for ordinary type-9 projectiles.
CODE 30 +0x2782 calls this routine with elapsed time and the proposed position.
The query center uses the wrapping sum of old and proposed coordinates, followed by a logical right shift.
Its heading comes from the existing projectile record.
The query width is 1.5 units. Its length is six times elapsed ticks plus three units.
A zero owner excludes the projectile itself. A positive owner excludes the object ID at participant +0x56.

After query selection, the caller rejects the following hits with result -1:

- The projectile itself
- A signed, wrapping absolute height difference above 60 units
- Target types 29 and 42
- Target type 32 with the same owner byte

A query result of zero remains zero. A surviving positive result identifies the target object.
The original caller does not retry selection after rejecting a hit.
Special projectile paths outside type 9 remain unported by this helper.
Negative owner indices return `None` as a host guard.

The harness executes 800 complete original calls, including the nested collision query and model lookup.
All results match Rust: 247 accepted hits, 336 zero results, and 217 rejected hits.
The harness also verifies that these calls leave all object records unchanged.
These fixtures use candidate collision tables from modeled SANE and supplied model dimensions.
The complete suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.
Model-slot construction remains necessary before this caller replaces live projectile collision.


## Model-slot reservation and completion

`ModelSlotState` preserves the loader counter and three type-indexed byte tables.
The default counter is one. Current and first slots start at -1; loaded counts start at zero.
CODE 19 +0x0004 resets entries for types 0 through 46.
CODE 19 +0x5d0a sets the next-slot counter to one.

CODE 19 +0x2358 reserves a slot before model lookup:

- An existing current slot prevents another reservation.
- A counter above 100 assigns current slot 1, preserves the first slot, and declines loading.
- Otherwise, the counter's low byte becomes both current and first slot, and loading proceeds.

CODE 1 +0x1322 adds the builder's returned count to the signed-word counter.
CODE 19 +0x23c6 stores the low byte of that count.
Completion also runs after declined reservations; the normal loader supplies zero in that case.
A failed or empty model load does not roll back its prior slot assignment.
These rules can create aliases that a fresh-slot-per-model implementation would miss.

The harness compares 1,046 reservation and completion cases against the original instructions.
Cases cover all 47 types, the capacity boundary, existing assignments, zero counts, and signed arithmetic.
Builder counts are supplied inputs. Some arithmetic fixtures deliberately supply counts outside normal loader behavior.
All results match Rust. The suite contains 30,980 vectors, including 1,368 modeled-FixRound cases.
Geometry builders, loader order, and special-type aliases remain separate integration work.


## Special-type model aliases

`ModelSlotState::load_alias_type` ports five branches of CODE 19 +0x5a80.
An existing current slot skips the branch entirely.
Type 14 assigns current and first slots to one.
The other supported branches use these dependencies:

| Requested type | Additional dependency | Shared model type |
| --- | --- | --- |
| 23 | 43 | 16 |
| 24 | 44 | 18 |
| 25 | None | 3 |
| 26 | None | 7 |

Missing dependencies load before the shared model.
After loading, the alias copies the source's current and first slots only.
Its count byte remains unchanged. A source still at -1 prevents the copy.

Type 26 has a distinct missing-source path.
It reserves type 7, increments the next-slot counter, and calls CODE 19 +0x1ce4 with zero.
That routine calls the selected-model loader for type 7.
The existing reservation prevents another reservation, and completion stores count zero.
This path is preserved even though normal startup can load type 7 earlier.

The harness executes the original outer routine for 500 cases across all five supported types.
The selected-model loader is an explicit service fixture using independently verified reservation and completion rules.
Comparisons include dependency-call order, all 47 current slots, first slots, count bytes, and the next-slot counter.
All results match Rust. These checks do not execute geometry builders.
The suite contains 30,980 vectors, including 500 loader-fixture cases and 1,368 modeled-FixRound cases.
Other special-type branches and geometry construction remain unfinished.


## Complete outer model-loader dispatch

`ModelSlotState::load_type` ports CODE 19 +0x5a80 with explicit geometry-loader services.
It supports all 47 initialized types and includes the previously recovered alias branches.
Type 19 increments its request counter before checking its existing slot.
An existing slot skips further dispatch. Otherwise, the word at A5-0x3cee becomes one.

Type 16 calls the selected-model loader, then halves both dimensions and the signed-word radius of its assigned slot.
Type 20 loads missing dependency 34 before loading itself.
Type 21 loads selected models 21 and 38, then raw shape resources 10099 and 10100 with type argument 14.
Each raw load receives the current next-slot counter. Its returned count advances that counter with signed-word wrapping.
Ordinary types call the selected-model loader directly.

The service contract separates selected loading, raw resource loading, and slot-size halving.
Selected loading includes reservation and completion. Raw loading returns its actual slot count.
This contract lets geometry builders populate the same model storage used by collision queries.

The harness executes 1,128 complete original outer-loader calls across all 47 types.
It compares call order, current and first slots, counts, counters, flags, all 100 dimension pairs, and all 100 radii.
All results match Rust.
Selected and raw geometry loaders are supplied fixtures; the original outer routine and its arithmetic execute normally.
The suite contains 30,980 vectors, including 1,628 loader-fixture cases and 1,368 modeled-FixRound cases.
Resource geometry, procedural builders, and startup loading order remain before live collision integration.


## Resource geometry in the runtime catalog

`PackedShape::from_resource_vertices` reads signed 16.16 coordinates directly from each shape resource.
It preserves integer precision before the modeled FixRound conversion.
Runtime coordinates use signed eighth-unit words. Insertion retains the first 60 vertices, matching CODE 12 +0x0532.
The helper computes base planar dimensions from those packed vertices.
Faces, radius, and slot-specific overrides remain separate stages.
The full shape parser validates the resource before the catalog stores this vertex data.

The catalog now stores packed geometry beside the decoded display shapes for every campaign resource.
This gives the future slot loader actual resource-derived vertices and dimensions instead of supplied test values.
The live collision query still uses preview bounds while slot construction and procedural builders remain unfinished.

The harness executes CODE 1 +0x16c0–0x1724, original vertex insertion, and the original dimension loop together.
All 358 campaign shape entries and six capacity fixtures match Rust.
Capacity fixtures cover 0, 1, 59, 60, 61, and 65 source vertices.
Allocation is supplied, FixRound is modeled, and the radius subroutine is explicitly bypassed.
The tests do not claim full resource loading, face conversion, or original OS-trap execution.
The suite now contains 30,980 vectors. Of these, 1,732 use modeled FixRound and 1,628 use geometry-loader fixtures.


## Procedural octagonal column is playable

Selector 3 dispatches to CODE 19 +0x3328 and builds the type-2 column.
Its vertex loop creates two eight-vertex rings at heights zero and 27.
The planar radius is six units. Each ring uses 45-degree steps from the unit direction table.
The runtime insertion helper packs these coordinates into eighth-unit words.
The face calls specify two octagonal caps and eight side quads.

`PackedShape::octagonal_column` now supplies this geometry to scene instantiation.
The preview uses the recovered polygon indices with diagnostic colors.
Original face normals, lighting, radar construction, and radius calculation remain separate work.
Base collision dimensions are available, but the live collision algorithm remains provisional.

The harness compares 21 original vertex-loop executions, including the candidate unit table and 20 synthetic tables.
It also captures the ten original polygon-call index lists for each case.
Initialization and radar blocks are skipped. Vertex insertion executes with modeled FixRound.
Face construction is intercepted to capture arguments; original normals are not claimed.
All packed vertices and polygon indices match Rust.
The suite contains 30,980 vectors, including 1,753 modeled-FixRound cases and 1,628 geometry-loader fixtures.

Across 112 maps, this model resolves 215 additional placements.
The preview now resolves 3,342 of 3,439 placements, leaving 97 explicit unresolved markers.
All map-selection checks and desktop/WASM driving and firing replays pass after the WASM rebuild.


## Selected-model loader dispatch recovered

`ModelSlotState::load_selected` implements CODE 1 +0x1206 through +0x1338.
It reserves a model slot before requesting `shps` resource 10000.
The routine selects a table column from two global bytes:

- A5-0x7be2 is zero: column 2
- A5-0x7be2 is nonzero and A5-0x7be5 is zero: column 0
- Both bytes are nonzero: column 1

CODE 19 +0x3f3c returns the second byte directly. Its meaning remains unconfirmed.

The selected ID chooses an inner loader:

| ID range | CODE 1 entry |
| --- | --- |
| 1–999 | +0x133a |
| 1000–1999 | +0x1434 |
| 2000–2999 | +0x150c |
| 3000–3999 | +0x15ba |
| 10000–10999 | +0x165e |

Other IDs load zero slots. A missing selection resource also loads zero slots.
Completion always writes the count, including attempts that skip loading because the type already has a slot.
A failed resource lookup does not undo the earlier slot reservation.
The next-slot counter uses signed-word wrapping addition.

The harness executes the complete selected loader, reservation helper, display-byte getter, and completion helper.
It supplies Resource Manager services and inner-builder return counts.
It compares all slot arrays, the next-slot counter, and the inner call arguments.
The 600 cases cover all 47 types, range boundaries, missing resources, capacity limits, existing slots, and count overflow.
These tests establish dispatch and bookkeeping, not inner geometry construction or live collision integration.


## Animation model slots recovered

`load_animation_models` implements CODE 1 +0x15ba through +0x165c.
The routine requests an `ashp` resource and reads its signed frame count.
For each frame, the destination is the base slot plus its zero-based frame index.
The destination does not depend on earlier builder return counts.

IDs 10000–10999 call the resource shape loader at +0x165e.
IDs 1–999 call the procedural dispatcher at +0x133a.
Other IDs skip the call but retain D5, the previous builder return count.
The total adds D5 after every frame, with signed-word wrapping arithmetic.
The selected-model caller enters with D5 equal to zero.
The Rust function exposes incoming D5 so isolated comparisons also preserve this behavior.
A missing resource returns zero.

The harness executes the complete original animation loader.
Resource Manager lookup and inner builder results are explicit fixtures.
The 264 cases include all nine campaign animation resources, empty and missing resources, invalid IDs, zero builder counts, and arithmetic overflow.
Every destination slot, call argument, and returned total matches Rust.
The original routine also preserves its caller's D5 register.

The combined suite contains 32,049 comparisons.
Of these, 864 selected/animation-loader comparisons supply inner builder results.
Another 1,628 outer-loader comparisons supply geometry-loader fixtures, and 1,958 comparisons use modeled FixRound.
These checks establish allocation sequencing, not complete geometry construction or live collision integration.


## Radius calculation and resource slot finalization

`runtime_sqrt` implements CODE 12 +0x00c4. It normalizes an estimate, then performs three fixed-point Newton iterations.
Nonpositive inputs return zero. Addition and shifts preserve the original 32-bit behavior.
The caller supplies division semantics.

`runtime_collision_radius` implements the radius path in CODE 12 +0x0944 and the word conversion in CODE 19 +0x3f64.
It uses packed X/Z vertices, software fixed-point squares, wrapping addition, and the maximum resulting radius.
The final conversion rounds upward to an integer word.
The complete original finalizer also writes planar dimensions and clears the active-model slot.

`candidate_fix_div` uses signed truncation with saturation as an explicit OS-trap model.
The original Macintosh FixDiv trap is not executed or verified by these comparisons.
Consequently, `PackedShape::candidate_radius` is conditional on that model.
The live tank footprint and projectile collision paths do not use this radius yet.

The harness adds 262 square-root cases and 563 complete slot-finalization cases.
The latter include all 358 campaign model entries and 205 synthetic packed-vertex lists.
Checks also verify that unrelated collision-table entries remain unchanged.

The 364 resource vertex-pipeline cases now execute the original slot finalizer instead of bypassing radius calculation.
Those cases cover resource packing, planar dimensions, and integer radius together.
Allocation remains supplied; FixRound and FixDiv remain modeled.
Face construction, radar construction, and model-specific overrides remain separate stages.

The suite contains 32,874 comparisons, including 2,783 cases that use modeled OS traps.


## Model-slot geometry library

`model_slots::ModelSlots` connects recovered outer dispatch, selected loading, animation loading, and packed geometry.
Callers provide an explicit type order. Original startup ordering remains unrecovered.
The library uses the third campaign selection column.

Resource loads install packed geometry in one-based slots. Missing resources return zero without invented geometry.
Animation loads preserve each frame index, including duplicate resource IDs.
Selectors 1, 2, 3, and 5 provide recovered procedural geometry on their matching type paths.
Other procedural builders and composite resources return an unsupported result.
An unsupported reconstruction path leaves the library unchanged, so retrying cannot mistake a partial reservation for a completed load.
That rollback is a host safeguard. Original missing-resource bookkeeping remains intact.

The outer type-16 path halves planar dimensions and the candidate integer radius.
Alias 23 loads dependency 43, then selected type 16 without applying the outer type-16 reduction.
Type 20 loads dependency 34 before its own resource.
Type 21 also loads type 38 and resources 10099 and 10100.

Selector 5 overrides both dimensions to ten units.
CODE 19 +0x223a–+0x2268 separately converts fixed value 0x00038666 upward and stores radius four.
Thus, the candidate radius from its full visible geometry is not its final collision-table radius.

`npm run test:model-slots` compares 123 assembled resource geometries with original-instruction fixtures across all three campaigns.
It checks animation offsets, duplicate frames, alias dependencies, type-specific reductions, and unsupported-path rollback.
Packed coordinates and dimensions match. Radius comparisons retain the modeled FixRound/FixDiv limitations.
The outer and inner dispatch routines also retain their separate differential tests.

The `model-slots` tool reports each type from a fresh library.
A successful dispatch does not imply that an alias has geometry without its prerequisite load order.
The library now supplies live projectile queries where all required model builders are available.
The current arena checks use recovered queries in 77 maps and explicit preview bounds in 35 maps.
Object records, ownership, loading order, and spawning remain preview inputs.


## First original sound effects

The application resource fork contains 105 `snd ` resources supported by the Rust decoder.
The decoder reads format-1 or format-2 resources with one inline standard sample command.
It preserves unsigned 8-bit PCM bytes and exports mono WAV files.
WAV sample rates use the nearest integer to the original unsigned 16.16 rate.
Compressed headers, external pointers, and unsupported command sequences return errors.
The format reference is [Apple Inside Macintosh: Sound, Sound Header Records](https://leopard-adc.pepas.com/documentation/QuickTime/INMAC/SOUND/imsoundmgr.43.htm).

The first preview event mapping uses the resource names:

| Event | Resource | Name |
| --- | --- | --- |
| Successful player shot | 10001 | playerShot |
| Enemy hit | 10004 | opponentHit |
| Enemy destruction | 10005 | explosion |
| New tank contact | 10007 | collision |
| Empty firing attempt | 10006 | outOfAmmo |

These mappings are provisional. Original sound-call sites, priorities, channels, and music scheduling remain untraced.
Rust emits events during simulation updates. Browser redraws and resets clear pending events.
The browser resumes Web Audio from the Drive tank click and caps concurrent sources at twelve.
Pause, mute, reset, and focus loss stop current sources. Volume starts at 35 percent.

`npm run test:audio` compares all 105 WAV payloads with original resource sample bytes.
A 600-tick WASM firing check produces 40 shot events, a hit and explosion, and subsequent empty-ammo events.
Redraws and resets produce no duplicate sound events.
Browser inspection confirmed source playback for firing, enemy hits, and destruction with decoded buffers.


## Playable encounter mode

Encounter mode adds provisional type-7 pursuit, local obstacle avoidance, aimed firing, and player damage.
Its steering speed, firing interval, sight checks, six-point hull, and victory conditions are new rules.
They are not translations of original AI instructions. Other enemy types remain stationary.
Enemy projectiles reuse recovered launch, motion, and expiry arithmetic with candidate direction values.
Their player and scenery hit tests use preview bounds, separate from the recovered player-shot query.
Original sounds 10002 and 10003 accompany enemy firing and player damage through provisional event mappings.

Clearing non-type-31 enemies starts the recovered type-31 animation transition.
This trigger replaces the original movie scheduler for the encounter; it is not recovered behavior.
The round ends when all targets die or the hull reaches zero.
Empty ammunition prevents firing but does not end the round, consistent with the full video reference.
Reset restores enemies, ammunition, hull, animation state, and timers. Stationary research mode retains the previous test behavior.
Native desktop driving and browser encounter mode share the Rust simulation.
Browser playback supplies audio; native desktop audio output remains unimplemented.

`npm run test:encounter` checks native/WASM replay parity and the actual Central arena.
It checks enemy movement, firing, player damage, sound events, reset, and switching to research mode.
Rust tests cover cover-blocked fire, victory, defeat, ammunition exhaustion, and frame-rate independence.

## Map confidence and original loader evidence

The 112 decoded layouts come from disc `wrld` resources, not hand-built replacements.
However, the scene position tests compare two decoders of the same data.
They establish internal consistency, not independent original-game equivalence.
Shape selection has separate original-instruction fixtures.

Static inspection locates the original world loader at CODE 25 +0x0a70.
It requests `wrld` at +0x0a7a and reads eight-byte placement records.
At +0x0b38, it reads each type byte and validates the range 0–46.
At +0x0b64 and +0x0b84, it reads the two coordinates and takes signed remainders using A5-0x137e.
At +0x0ba6, it reads the heading and takes its signed remainder modulo 360.
The results are converted to fixed-point values and stored in object fields +0, +4, and +8.

These instructions support the decoded record structure. This loader has not yet been replayed against the Rust scene builder.
Heading-to-render orientation, optional objects, and world-selection sequence still need independent checks.
Player and enemy spawn locations remain provisional. The first wave referencing a map is a preview selection rule.
Thus, disc-derived static layouts are established; a fully faithful playable arena is not yet established.


### Original placement conversion replay

CODE 33 +0x0260 explicitly initializes the world modulus at A5-0x137e to 1024.
The verification harness now executes CODE 25 +0x0b60–+0x0bc4 with the original CODE 36 integer multiplication helper.
It supplies each placement record and captures the three fixed-point object fields.
No modeled OS trap or substituted arithmetic is involved in this block.

All 4,347 disc placements across the 112 world resources match the Rust `world_placement` function.
Another 400 cases cover signed coordinates, signed headings, and positive world moduli.
All disc coordinates and headings already satisfy their normal ranges, so this change does not move existing scenery.
Negative synthetic inputs retain signed remainders rather than wrapping into positive coordinates.

Scene construction now uses this recovered conversion.
`npm run test:scenes` compares the 3,439 visible placements directly against original-instruction results.
This replaces its earlier coordinate comparison against a second decoder.
The complete differential suite now contains 37,621 cases, plus 141 enemy type/health linkage checks.

This establishes placement arithmetic and stored headings, not the final model-to-world rendering transform.
It also does not establish the full loader, optional objects, spawn selection, or campaign scheduling.


## First-person visibility correction

The preview previously sorted each face by average camera depth.
This could draw a distant surface over a nearer one, especially where faces intersect or span large distances.
Shots and impact effects also bypassed geometry visibility.

First-person rendering now stores reciprocal depth per pixel.
Scanline intersections interpolate reciprocal depth across planar faces and preserve concave boundaries with even-odd spans.
Near-plane clipping still occurs before projection. Raster work is bounded by the viewport.
Shots, two-vertex faces, and impact effects use depth-tested lines. Crosshair marks remain overlays.
This is a modern renderer correction, not a recovered original rendering routine.

Tests cover crossing surfaces in both submission orders, concave openings, hidden effects, depth reset, and oversized or non-finite input.
All 112 arena render checks and native/WASM encounter replays pass with this renderer.
Fullscreen controls and letterboxed layout are implemented; live browser fullscreen verification remains pending while hosting is stopped.


## AI tracing checkpoint

The participant update at CODE 31 +0x1fb6–+0x1fc6 calls CODE 20 +0x2228 for command selection.
The callee receives three output pointers, including steering, throttle, and the firing request.
Its branch at +0x253a selects movement routines from a participant behavior field, not directly from the object type alone.
CODE 20 +0x1672 contains one movement policy; +0x1d7c and +0x15fa are alternate policies.

Within the +0x1672 policy, +0x1b8e–+0x1bd6 compares desired and current fixed-point headings.
It folds the difference into the signed half-turn interval and applies a one-degree steering dead zone.
Subsequent branches use a movement-angle threshold, reverse state, and calls to +0x05b0 before issuing throttle commands.
The complete policy and its probe dependencies still need instruction replay before replacing the provisional encounter AI.
CODE 20 +0x0754 and +0x08f8 separately rank participant targets using distance and angular criteria.
These static traces identify recovery targets; they are not claims that original AI is implemented.

## Ground-effect contact dispatch and Flow Field displacement

The original `STR#` 1020 names align with zero-based object kinds (string index = kind + 1):
16 Acid, 17 Traction Limiting Zone, 18 CyberMud, 23 Enhanced Acid,
24 Enhanced CyberMud, 39 Flow Field, and 40 Force Field.
These labels agree with the manual's descriptions; ice is the traction-limiting surface.

CODE 7 +0x1756 reads the contacted object's type. The local-player switch subtracts 13,
indexes signed words at +0x1776, and adds each offset to +0x1774.
Decode those words as data, not the misleading linear-sweep instructions in that interval.

| Kind | Branch | Handler |
| --- | --- | --- |
| 16, 23 | +0x17e2 | +0x1096 (acid) |
| 17 | +0x180c | +0x132a (ice) |
| 18, 24 | +0x17f0 | +0x1360 (mud) |
| 39 | +0x17fe | +0x1394 (flow) |
| 40 | +0x19de | Default continuation; not proof that force fields have no collision |

### Flow Field

CODE 7 +0x1394 rejects nonpositive participants and participants other than the local player.
It sets A5-0x3d31 (contact this update). If A5-0x3d28 is -1, it sets that clock to now minus one.
It computes `wrap(field.heading - 90 degrees)` and calls CODE 18 +0x0cd0 for the direction pair.
The participant's object receives this displacement on each planar axis:

`position += wrapping_i32(direction * wrapping_i32(now - previous_contact)) / 2`

Division is signed and truncates toward zero. The product wraps before division.
The handler then calls CODE 30 +0x0f60 with old/new position records and updates the contact clock.
CODE 33 +0x001e clears the contact flag before the update; +0x0248 resets the clock to -1 when no contact occurred.
This is direct extra displacement, not an increase to the tank's speed setting.

`recovered::flow_field_displacement` transcribes +0x1426..+0x1464.
400 differential cases execute that original block with original CODE 36 multiply/divide helpers.
They supply direction and contact clocks explicitly, including zero elapsed, odd negative products, clock wrap, and wide integer inputs.
All resulting positions match Rust; bytes after the two position fields remain unchanged.
This verifies the arithmetic block only. Contact geometry, direction-table selection, spatial updates, and scheduling are not covered by these cases.
The helper is not yet connected to the playable simulation.

### Ice and Cybermud

The ice handler at +0x132a writes byte 0xfa (signed -6) to A5-0x3d32.
It schedules sound request 0x00010024 with a 6000-tick cooldown.
The mud handler at +0x1360 copies A5-0x3fcb into the same adjustment byte.
It schedules sound request 0x00010023 with a 60-tick cooldown.
CODE 25 +0x14e4 initializes the mud adjustment to 5; +0x2fcc changes it to 8 and +0x2fda clears it.
The conditions invoking those later overrides still need tracing; do not assume a universal mud strength.

The already recovered speed routine CODE 30 +0x11d6 consumes this adjustment.
A positive adjustment reduces speed toward a nonzero signed minimum; a negative adjustment selects its magnitude as the coasting parameter.
Thus ice selects coast 6. The Random-dependent branch applies only to coast values 1–5, so ice skips it.
An earlier progress report incorrectly linked ice to that branch; the speed implementation already used the correct range.
CODE 31 +0x16f4 also checks the negative adjustment and, when object +0x0e is nonzero, clears the turn command at +0x0c.
Full control ordering and surface contact still need recovery before enabling these effects in gameplay.

## Complete object-pair overlap used by surface contacts

`recovered::object_pair_overlap` transcribes CODE 7 +0x02ce..+0x047c.
The surrounding contact query at +0x0d90 calls it at +0x0f2c after a separate broad-phase helper.
This pair routine copies both 128-byte records and applies these tests:

- Signed word Manhattan distance from the high words of coordinate differences must be below 108.
- Height separation normally must not exceed 60 fixed-point units. Types 5 and 36 bypass that restriction.
- Type 12 has a directional height override with a strict 140-unit boundary.
- The sum of the resolved model radii is compared with planar distance using wrapping signed-word squares.
- The final test uses the already recovered rotated-rectangle separating-axis routine.

The Rust helper receives resolved model dimensions and radii. Dimensions are grouped by the first and second input records.
It reverses those groups when calling the rectangle routine to preserve the original stack argument order.
Slot lookup remains separate in Rust; the differential harness executes the original CODE 6 +0x18ea lookup.

600 comparisons execute the complete original pair routine, model-slot lookup, direction lookup, and rectangle tests without service stubs.
Inputs cover terrain/vehicle type combinations, rotated models, contact-distance and height boundaries, and signed overflow cases.
The direction table uses the existing candidate values from modeled SANE initialization.
122 original cases report overlap; all 600 results match Rust. Type-42 linked animation selection is outside these new cases.
The complete corpus now contains 38,621 differential vectors.

This is not yet the complete terrain-contact query. Remaining work includes the CODE 11 +0x3cba prefilter,
participant exclusions, spatial-cell traversal, selected-contact ordering, and the collision response caller.
No surface behavior was enabled in the playable loop by this change.

## Contact prefilter and post-overlap selection

`contact_near` transcribes the complete CODE 11 +0x3cba helper.
It compares a signed, wrapping fixed-point Manhattan distance with 108 units, inclusive.
Unlike the later object-pair gate, it retains fractional coordinates and uses long arithmetic.
500 original executions cover the exact threshold, neighboring fractional values, and integer overflow.

`select_contact` transcribes CODE 7 +0x0f3a..+0x104e, after candidate overlap has succeeded.
The query maintains two separate found flags and object IDs:

- Ordinary contact: retain the first accepted object. Ignore kinds 10 and 14. A kind-6 candidate is also ignored when the moving kind is 13, 21, 23, 24, 35, or 45.
- Projectile contact: overwrite the previous accepted ID. The candidate set is 9, 28, 29, 32, 33, 34, 43, 44, and 46. Kinds 29 and 33 are excluded. Moving kinds 23–25 accept only candidate 28 from this set. Matching ownership excludes projectile contact.

The type sets come from the bytes preceding CODE 7 +0x1086, +0x108a, +0x1090, and +0x1096.
Those bytes are data; linear disassembly in this range is misleading.

4,614 comparisons execute the original selection block, covering every pair of normal kinds 0–47, three out-of-range pairs, populated/empty results, and varying ownership.
All match Rust. No instructions or called services are replaced in either new test group.
The corpus now has 43,735 differential vectors.

The block's ordinary-contact path assumes earlier exclusions have already run.
CODE 7 +0x0eb6..+0x0ede rejects matching ownership and kinds 14/42 before the prefilter and pair overlap.
Do not call this selector directly as a complete contact query.
Spatial traversal, cached participant cell coordinates, and caller response remain to be composed and verified.
No playable surface integration changed in this pass.

## Composed participant contact query

`CollisionQueryWorld::participant_contacts` transcribes CODE 7 +0x0d90 for the participant-backed moving types:
6, 7, 8, 13, 20–27, 31, 35, and 45. Other moving types currently return `None` as an unsupported host path.
It composes the original grid bounds, fixed-point prefilter, object-pair overlap, and contact selection.

The query reads the participant's cached cell at offsets +0x60/+0x62 through CODE 7 +0x0484.
Each cell spans 64 world units. The bound expansion uses strict 72-unit comparisons against that cached cell's edges.
It does not simply recompute the cell from the current position. The port preserves the supplied cached coordinates.
Bounds clamp to the 16-by-16 grid, which is traversed by first coordinate, then second coordinate, then stored object order.
Duplicate grid entries remain in the scan.

Participant state 3 returns no contacts. An integer position coordinate at or beyond either arena edge selects ordinary contact -999.
The scan continues after that boundary result, but ordinary first-contact retention prevents replacement.
Matching owner IDs and candidate kinds 14/42 are excluded before overlap.

400 comparisons execute the complete original CODE 7 +0x0d90 routine, including original bounds, integer arithmetic,
model-slot selection, prefilter, and rotated-rectangle overlap. No services or instructions are replaced in these new cases.
The supplied direction table remains the candidate produced with modeled SANE initialization.
All 15 supported moving kinds occur in the fixtures. Cases vary positions, headings, heights, owners, grid order,
duplicate entries, cached cells, participant state, model sizes, and radii. Twelve fixtures explicitly exercise arena edges.
Results: 239 ordinary-contact hits and 95 projectile-contact hits. Both result slots match Rust in every case.
The full corpus now contains 44,135 differential vectors.

The host method takes a resolved moving-object ID; the original retrieves it from participant +0x56.
It guards invalid participants, records, slots, and grid indices with `None`.
Non-participant moving types and full caller responses remain outside this implementation.
The playable loop still needs record construction, cached-cell updates, and ground-effect response integration.
`collision_preview::Query` is the existing bridge for projectile records and is the next integration point to inspect.


## Playable ice and static Cybermud integration

`collision_preview::Query` now carries model radii and exposes the ordinary contact kind from the recovered participant scan.
The adapter still uses preview entity order, owner assignments, and current-position cell updates.
It is not a reconstruction of original object allocation or cached-cell scheduling.

At each preview tick, `Game::step` samples that query before turning and movement.
Kind 17 supplies speed adjustment -6; kind 18 supplies default mud adjustment 5.
The negative adjustment clears the turn command when throttle is nonzero, following the CODE 31 guard.
Ice coast 6 requests no Random call. The existing speed routine handles both adjustments.
Static ice/mud footprints are passable while the query is available. Other solid-response behavior remains provisional.
The tank's serialized `surface` status is exposed in browser telemetry and resets before resampling.

Missing model geometry, invalid slots, or unsupported query inputs yield `unavailable`, without inventing a surface effect.
The existing fallback collision path remains active in those situations.
This does not implement enhanced moving mud, original mud-strength overrides, surface sounds, or Flow Field displacement in the live loop.

A synthetic end-to-end Rust fixture exercises record construction, grid lookup, overlap, selection, movement adjustment,
passability, steering suppression, leaving ice, and reset. It confirms released-throttle motion on ice and forward slowdown on mud.
All 30 Rust tests pass. The actual WASM build passes all 112 arena checks and desktop/WASM replays, plus encounter checks.
Those tests do not establish original scheduling fidelity or exhaustive driving routes across every terrain patch.
Hosting remains stopped and no live browser interaction was performed.


## Playable Flow Field integration

CODE 6 +0x0512 passes scale 0x00060000 to the table initializer, then stores that table pointer at A5-0x38ba (+0x051c).
CODE 18 +0x0cd0 reads that pointer. This is the table called by the Flow Field handler at CODE 7 +0x13f6.
It is distinct from the movement table at A5-0x38be, initialized with scale 0x00004000.
Consequently the Flow Field's divide-by-two produces approximately three world units of displacement per elapsed tick.

The preview adapter now returns both contact kind and heading. Kind 39 is passable when contact geometry is available.
The live loop wraps field heading minus 90 degrees, reads the candidate unit table, scales its pair by six,
and calls the instruction-verified `flow_field_displacement` helper.
The first contact uses previous time = now - 1. Continuous contacts share the clock; a non-flow tick or reset clears it.
Propulsion speed remains independent of this extra displacement. The HUD reports `flow`.

The synthetic surface integration test now covers all four cardinal field orientations, two consecutive pushes,
zero propulsion speed while being pushed, exit, and reset. All 30 Rust tests, Clippy, the WASM build,
112 arena/view checks, desktop/WASM replays, and encounter checks pass.

Limits: contact is sampled at the preview tick's start and push is applied after normal movement.
Solid-footprint checks can reject that push. This guard does not reproduce original CODE 30 +0x0f60 or full response ordering.
Direction-table values use modeled SANE, and adapter heading conversion/record construction remain provisional.
No live browser check was performed; hosting remains stopped.

## Single-player flags: collection, count, and refresh

CODE 7 +0x1a88..+0x1ad8 routes a local player's uncollected type-12 contact to CODE 28 +0x0268.
The caller checks the participant's per-object collected byte before dispatch.
For game mode zero, CODE 28 takes its +0x0558 branch. It accepts object IDs in the leading flag-slot range and requires the local participant.
The accepted pickup block at +0x059c:

- Sets the participant's collected byte at participant +0x73 + object ID.
- Changes the object type at +0x14 from the flag type to 14.
- Stores the current tick at A5-0x3d00.
- Calls the score routine CODE 31 +0x009e with 20 points.
- Issues sound requests, then counts remaining flags. A count of one selects request 0x00010017; other counts select 0x00010011.

These are traced instructions, not yet a complete differential test of the pickup handler or sound mapping.
Score handling includes extra-life behavior and must not be replaced by a guessed independent scoring system.

`remaining_flags` transcribes the complete CODE 28 +0x021a routine.
It counts type-12 records among the first A5-0x12cf slots (a signed byte). Negative limits scan no records.
400 original executions match Rust, covering all 256 signed-byte limits and mixed record kinds.

`refresh_flag_slots` transcribes CODE 25 +0x1f6c..+0x1fe8, before the final display notification.
For ordinary positive inputs, the slot budget is `min(level / 2 + 3, 20, object_count - 6)`.
The implementation also preserves signed-word arithmetic, byte conversion, and negative-limit behavior.
Only type-14 entries within the leading budget become type 12. The global flag-slot byte is then replaced by the number converted.
400 original executions match both the mutated object kinds and resulting count, including signed extremes.
The original min helper executes in these comparisons; no services are replaced.

CODE 25 +0x21c2 loads/reset the world through +0x0ce4, then +0x21c6 calls this refresh routine.
The precise placeholder preparation and complete loader ordering still need verification before applying it to decoded arenas.
In particular, do not assume that every first-N raw obstacle is already an eligible type-14 entry.
The editor manual independently explains that leading layout entries are reserved/converted for flags.

The full corpus now contains 44,935 differential vectors. The playable encounter still uses its provisional elimination objective.
Flag pickup, score, lives, level-end bonus, and campaign transition have not yet been integrated.

## Authored flag placeholders and level-layout preparation

CODE 25 +0x0aa4..+0x0af4 initializes object records to kind 14 before loading the authored world.
At +0x0b38 it reads the low signed byte of each authored kind. Values outside 0–46 become 14; valid kinds are retained.
The subsequent position conversion preserves the authored coordinates through the already verified remainder/fixed-point path.

The actual disc layouts contain leading kind-14 entries. Central world 10000 starts with six.
Applying the recovered refresh at level 1 activates slots 1–3 at H/V coordinates (16,16), (1008,1008), and (624,240).
These are authored placeholder positions, not newly chosen spawn coordinates.
Across the 112 layouts, only Eastern world 10017 has fewer than three leading placeholders: it has two.

`World::with_level_flags` now provides a reusable, non-mutating preparation step for authored layouts with 6–50 obstacles and positive level numbers.
It uses the loader's low-byte kind interpretation and the verified refresh helper; coordinates, headings, and order are preserved.
It deliberately rejects the unsupported procedural/small-world path instead of presenting it as recovered.
This helper excludes the loader's optional extra object and automatic campaign/world selection.

Generate a reviewable manifest without running a server:

```sh
cargo run --quiet --locked -p spectre-tools -- flag-layout .local/extracted/central.bin 1
```

Manifests were generated for all three campaigns at level 1:
`.local/central-level1-flags.json`, `.local/eastern-level1-flags.json`, `.local/south-level1-flags.json`.
111 layouts produce three flags; Eastern 10017 produces two. This tests arbitrary level-1 preparation, not campaign progression order.
A regression test covers unchanged source data/poses, ineligible leading scenery, the object-count limit, and invalid levels.
All 31 Rust tests and Clippy pass. The playable encounter has not yet switched to these prepared layouts.

## Playable authored flag missions

The browser default and desktop driving mode now prepare the selected arena with level-one flags.
Research and elimination modes retain their raw-layout behavior.
This is a selectable arena mission, not recovered campaign progression.

The shared Rust simulation collects flags on contact, tracks progress, and awards 20 flag points per pickup.
The value comes from CODE 28's score-handler call; full score handling and extra lives remain pending.
Collecting every flag ends the preview mission. Enemy elimination alone does not end it.
Restart restores the initial items, counters, enemies, and vehicle state.
Flags are passable and do not block either side's shots or enemy sight.
All three views draw flags in diagnostic yellow.

The flag procedural collision builder remains unsupported.
Its missing geometry no longer disables recovered projectile queries for the whole map.
The contact adapter exposes the selected object ID when original contact geometry is available.
If the participant query cannot resolve geometry, flag pickup uses overlap of the tank and flag footprints.
This fallback is provisional. It does not claim original pickup dimensions or collision timing.
The preview removes collected items; the original changes their type to 14 and tracks participant collection bytes.
Record identity, multiplayer collection semantics, flag animation, pickup sounds, and campaign transitions remain pending.

Validation:

- 32 Rust tests pass, including driving into flags, collecting only once, winning, stopping, and restarting.
- The pickup test covers recovered contact selection with supplied geometry and the footprint fallback.
- Strict Clippy and the WASM release build pass.
- All 112 authored missions start and reset through the WASM API.
- All three campaign mission replays agree between native Rust and WASM.
- Existing 112-arena view/zoom checks and encounter replays still pass.
- Projectile geometry remains available on 77 maps; 35 retain the previous explicit fallback.
- Browser interaction was not tested. Hosting remains stopped.

## Flag geometry and collision bounds

CODE 1's procedural selector 12 calls CODE 19 +0x0fd4.
The base flag mesh is constructed at +0x1014..+0x10f0:
five vertices, a pole edge, and either four outline edges or two opposing cloth faces.
Packed coordinates are (0,0,0), (0,112,0), (56,112,0), (56,160,0), and (0,160,0).
Runtime coordinates use eighth-unit increments.

Two new comparisons execute the original vertex insertion, face/edge argument lists,
and slot finalizer. FixRound and FixDiv remain explicit modeled OS services.
The builder also creates six lettered variants at reserved slots; their glyphs and radar data remain pending.
Their added vertices stay within the base flag's planar extrema.

CODE 19 +0x14ac..+0x152a checks the signed-byte current slots for types 12 and 2.
If both exist, it copies BOTH planar dimensions and the radius from type 2 to the flag slot.
It does not copy the visible vertices.
The full guarded copy matches 300 executions, including missing slots and source/destination aliasing.
This extends the earlier dimension-only block check to include the guard and radius.

The collision loader now supports the base flag and conditional bounds copy.
Its preview load order is still not original startup ordering.
Loading type 2 after a flag does not retroactively change the flag.
Unsupported arenas still use the explicitly provisional footprint pickup path.

The renderer now supplies real procedural flag geometry.
The preceding milestone had checked selector IDs but had not checked resolved geometry,
so its claim that all flags were visibly rendered was too strong.
The WASM mission check now requires a recovered geometry source and visible flag pixels.
Flat flag geometry also exposed the footprint fallback's rejection of line-shaped footprints;
polygon-versus-segment overlap now works without adding a padding radius.

Validation: 45,237 original-code comparisons pass (302 new).
Of the total, 2,785 explicitly model an OS trap; this is not full executable equivalence.
All 34 Rust tests, strict Clippy, WASM build, 112 mission starts/resets,
three native/WASM mission replays, existing gameplay/encounter checks, model-slot checks,
and scene checks pass. All 3,439 visible raw-layout placements now resolve.
An actual WASM driving sequence collects the authored Central flag at (624,240),
earns 20 points, continues playing, and resets. No coordinates are injected.
An offline overhead frame was inspected at .local/flag-mission.png.
Hosting remains stopped; live browser interaction was not tested.

## Score arithmetic and extra-life thresholds

CODE 31 +0x0004 reads an sstf resource selected through CODE 25 +0x1ab6.
The disc TMPL labels ten signed-long fields, starting at byte 14, as scores for new lives.
The final word at byte 54 is the repeat increment after the tenth threshold.
ScoreRules parses these fields and rejects truncated resources.

Central sstf 10000 starts at 5,000 points, consistent with the supplied video's extra life near 4:00–4:30.
Resources 10001 and 10002 start at 10,000 and 20,000.
This supports the observation but does not prove which resource the recording selected.
CODE 25's selector depends on the value returned by CODE 1 +0x0890 and two mode flags.
The runtime meaning and startup choice of that value remain to be traced.

The complete CODE 31 +0x009e handler:

- Adds negative signed-word awards directly to the signed-long score, with wrapping arithmetic.
- Splits positive awards into chunks of at most 4,999 points.
- Computes the next threshold before each chunk.
- Adds a life only when that chunk crosses the threshold and A5-0x12ca is clear.
- Wraps the signed-word life count.
- Requests sound 0x0001002a at priority 1 for each awarded life.
- Returns an award flag and conditionally notifies the HUD.

Rust next_life_threshold and award_score preserve the arithmetic and life gate.
The pure function returns the number of awards; it does not call audio or HUD services.
It has a host-only iteration guard for pathological repeat increments.

A significant edge case: the executable sign-extends the repeat word.
The supplied words 40,000, 50,000, and 60,000 therefore become negative.
With unmodified Central 10000, score 140,000 yields next threshold 140,001;
score 140,001 yields 2,147,478,624 after the original wrapping loop.
Do not silently replace this with an unsigned 40,000-point interval.
These are verified isolated routine results, not evidence that ordinary gameplay reaches that state without other intervention.

Validation adds 60 complete threshold executions and 500 complete score-handler executions.
Fixtures supply Resource Manager access, selected resource bytes, audio, and HUD services.
The original threshold routine, chunk loop, min helper, score updates, and life gate execute.
Cases include missing resources, all three Central tables, a positive-repeat synthetic table,
threshold boundaries, signed penalties, large awards, disabled extra lives, and life-count wrap.
All 45,797 differential vectors pass. The 2,785 modeled-OS-trap count does not include every supplied service.
All 36 Rust tests and strict Clippy pass.

Inspect original resources with:
cargo run --quiet --locked -p spectre-tools -- score-rules .local/extracted/central.bin

The playable loop still has its explicit flag-only points counter.
Do not expose these recovered extra lives as usable lives until death/respawn and resource selection are connected.
No browser behavior changed in this recovery step, and hosting remains stopped.

## Difficulty selects the score resource

The previously unnamed selector input is the Difficulty option:

- DITL 3000 item 15 is a control; item 16 labels it "Difficulty:".
- Item 15 references CNTL 8100. Its stored value/min/max are 0/0/7.
- CODE 2 +0x126a..+0x1292 reads control 15, stores A5-0x7bfa, and calls CODE 1 +0x07fa.
- That setter stores the value at A5-0x4258. CODE 1 +0x0890 returns it.
- CODE 18 +0x0014 initializes the option to zero; preference loading can replace it.
- The manual's printed page 24 describes Basic, Cool, and Killer difficulty.

The full CODE 25 +0x1ab6 selector and actual CODE 1 getter now match 192 original executions.
Cases include all normal control values, signed boundaries, and noncanonical nonzero mode bytes.

Normal-mode mapping:
difficulty below 3 selects sstf 10000;
difficulty 3 or 4 selects 10001;
difficulty above 4 selects 10002.
Their first Central thresholds are respectively 5,000, 10,000, and 20,000.

A5-0x12d2 takes precedence and selects resource 778 when nonzero.
Otherwise a nonzero A5-0x12ca selects resource 777.
These raw mode flags still need their startup meanings traced.
The new Rust score_resource_id keeps both explicit.

The score-rules tool accepts a difficulty argument and marks the selected resource:
cargo run --quiet --locked -p spectre-tools -- score-rules .local/extracted/central.bin 4
The result is saved at .local/central-difficulty4-score-rules.json.
This tool does not claim the port already reproduces difficulty-dependent enemy strength or world generation.

All 45,989 differential vectors, 36 Rust tests, and strict Clippy pass.
Gameplay behavior is unchanged by this recovery step.

### Death/respawn trace to continue

CODE 31 +0x1a80 is a participant lifecycle handler.
The local-player path at +0x1b02..+0x1b32 decrements lives when A5-0x12ca is clear,
then checks whether lives remain. Exhaustion calls CODE 33 +0x09d6.
The surviving path can play a movie, resets local controls through CODE 25 +0x0004,
and invokes CODE 31 +0x04b4 for placement.
The common tail at +0x1cb8 reloads health and ammo via CODE 23 +0x0684/+0x06fc.
The routine also has separate remote/enemy paths, life-sharing logic, and a final-enemy bonus.

These are static trace findings, not newly verified complete routines.
Before gameplay integration, preserve participant score, collected flags, level state, and surviving enemies.
The supplied video around 14:53 supports preservation of score/level while damage and ammo reset.
Do not replace this with Game::reset, which would restore the whole arena.

## Playable mission lives and respawn

The complete CODE 31 +0x1a80 lifecycle routine was executed for 400 local-player cases.
The actual CODE 23 health and ammunition helpers ran.
Fixtures supplied movie availability (false), presentation, control-reset, and placement services.
These results verify the local field decisions, not the original placement or movie sequence.

For an active local participant with health at or below zero:
normal mode decrements the signed-word life count with wrapping arithmetic.
A nonzero A5-0x12ca suppresses consumption.
A nonpositive resulting life count selects game over.
Otherwise health becomes min(signed shield setting, 9), and ammo becomes signed ammo setting times ten.
Positive health or an inactive participant leaves these fields unchanged.
Score bytes and the tested collection bytes remain unchanged.
Cases include signed boundaries, noncanonical mode bytes, all 12 participant indices,
and shield/ammo setting boundaries.
CODE 25 +0x1436 independently initializes a participant with three lives.

Flag missions now use three initial lives and the original default difficulty-zero score resource.
Flag awards call the recovered score handler, so threshold-crossing awards can add usable lives.
The UI explicitly labels the current score as flag-only: kill points and completion bonuses are not connected.

On death, the shared simulation applies the verified life/refill decision.
Respawn preserves score, flags, current items, enemies, destroyed count, shots, firing count, and tick time.
It clears movement state and restores balanced health/ammo to 6/40.
Position selection is a deterministic safe search near the preview spawn.
This is not the original randomized placement routine.
If no safe position exists, the preview stops with an explicit no_spawn state.
Original movies, respawn presentation, timers, and extra-life sound routing remain pending.
The old elimination/research modes retain their prior behavior.

Restart remains a separate full reset: three lives, zero score/deaths, restored flags and enemies.
The original initial-life value is corroborated by the supplied video.
The video's retained score/level on death supports preserving mission progress.

Validation:
- 46,389 original-code comparisons pass, including 400 new lifecycle cases.
- 37 Rust tests and strict Clippy across all targets/features pass.
- An integration test crosses the 5,000-point threshold, consumes the earned life,
  preserves one collected flag and a destroyed enemy, reaches final death, and restarts.
- The actual WASM combat test consumes three lives, confirms two refills/respawns,
  stops simulation on final death, and restores initial state on restart.
- All 112 authored mission starts/resets and three native/WASM mission replays pass.
- Existing 112-arena gameplay/camera checks and encounter parity checks pass.
- WASM release build and JavaScript syntax check pass.
- Hosting remains stopped. No live browser interaction was claimed.

## Combat kill awards are playable

CODE 31 +0x0b9a computes the normal-mode kill flag from remaining health.
The post-damage gate at +0x0caa..+0x0de2 checks attacker attribution,
participant flags, self-hits, the kill flag, and the mode mask.
The admitted mode values are 0, 1, 6, 13, 14, and 18.
Raw flag bytes are retained because the original gate is bitwise.

For types 6, 7, 8, 20, 21, 22, 23, 24, 26, 27, 31, 35, and 45,
the routine increments the per-type kill count and reads a signed word at
runtime table +0x04ca + 4*type.
CODE 25 +0x183c..+0x1852 loads the resource's signed-long score into +0x04c8.
Thus the award uses the signed low word of the resource score.
Other object types yield a zero-point call in normal mode.
Nonzero admitted modes replace the points with one; their upstream kill-flag meanings remain outside this integration.

812 new comparisons execute the whole post-damage gate through the original score-call boundary.
They cover all 256 object bytes, all 256 mode bytes, and 300 attribution/flag cases.
The score handler is captured as a service here and is separately verified in the existing corpus.
The fixtures check the recipient, signed award, and per-type count mutation.

Each preview target now carries its original bval score.
Mission projectile kills feed the recovered award into the recovered score handler.
Both the original-query path and the footprint fallback enqueue each kill once.
Protected hits and nonfatal damage do not award kill points.
Separate kills remain separate score calls, retaining threshold semantics.
Preview player/enemy attribution remains explicit; this does not recover multiplayer ownership.
The score HUD now includes flag and kill points. Completion bonuses remain pending.

All 47,201 differential vectors pass.
37 Rust tests and strict Clippy pass.
The type-31 integration test checks protected hits, one-time signed-low-word awards,
an extra-life threshold crossing, and reset.
Actual WASM shots destroy an enemy and award the sum of its recovered score values.
Across all 112 mission starts, target scores are compared directly with original bval bytes.
The 112 mission/gameplay checks, native/WASM replays, two-respawn/final-death test,
and legacy encounter checks pass. WASM build and JavaScript syntax check pass.
Hosting remains stopped.

## Completion ratings and bonuses

CODE 21 +0x0004 computes the arena par time from two 47-entry signed-word
per-type tables at world state +0x0002/+0x0060, weighted by the signed-long
bonus table at +0x0584, then adds 10 times the signed flag count plus 10.
Each count product is truncated to a signed word before its bonus multiplication.
The returned total is also a signed word.
The original world loader can modify these tables for difficulty and participant limits;
their complete initialization is not yet connected to the playable mission.

CODE 21 +0x0b40..+0x0d44 computes Quickness, Accuracy, and Avoidance.
The prefix runs the original par-time routine and bonus helper, then stops before presentation.
Elapsed seconds are the signed wrapping difference between A5-0x12e2 and A5-0x12e6, divided by 60.
The counters come from world state +0x07fc, +0x0800, +0x0804, and +0x0808:
player launches, player-attributed hits, non-player launches, and player damage-handler events.
CODE 30 +0x0292..+0x02a6 and CODE 31 +0x0b40..+0x0b96 identify their update sites.
These count events, not accumulated damage or the number of destroyed enemies.

For ordinary nonnegative inputs:
- Quickness rating is 6 minus elapsed percentage divided by 20, clamped to 1..5.
- Accuracy rating is hit percentage divided by 20, clamped to 1..5.
- Avoidance rating is (100 minus received-hit percentage) divided by 20, clamped to 1..5.

The port preserves the exact sequence of signed division, wrapping multiplication,
word truncation, and clamps rather than replacing it with floating point percentages.
The 16-step bar formulas are separate from the five rating bands.
Some bars have only one clamp; hostile negative inputs retain original asymmetry.

A nonpositive player-launch count marks Accuracy unrated and grants no Accuracy bonus.
A nonpositive non-player-launch count does the same for Avoidance.
The original keeps and clamps the prior rating word in these cases, but zeros its bonus.
A zero par or unrepresentable host division returns None in Rust as an explicit host guard.

CODE 21 +0x0ae6 maps rating 1..5 to 50, 100, 200, 400, and 800 points.
The results presenter calls the score handler separately for each category,
using the signed low word of each bonus. Do not merge the three awards:
separate score calls can affect extra-life threshold behavior.

Disc STR# 1010 supplies the names:
1 Get Help; 2 Marginal; 3 Not Bad; 4 Radical; 5 Perfect.

Video corroboration:
the inspected .local/reference-video/first-transition.jpg shows
Not Bad / Not Bad / Radical and score 935 after gameplay score 135.
The recovered mapping gives 200+200+400=800, exactly matching that change.
This validates the category awards, not the exact underlying hit/launch counters.

Validation adds 400 original par-time executions and 400 complete computational prefixes.
The original long multiply/divide helpers, par-time routine, and bonus helper execute without mocked arithmetic.
Cases include ordinary counters, missing shots, prior rating values, signed table boundaries,
wrapping count products, negative par values, and out-of-range hit ratios.
All 48,001 differential vectors pass. Existing 37 Rust tests and strict Clippy pass.
No gameplay award was added yet: exact runtime table initialization and counter lifecycle
must be connected before the port claims original completion results.
Hosting remains stopped.


## Enemy counts and life budgets — CODE 25 +0x1d7e..+0x1ee2

The original initialization now has a Rust counterpart, `recovered::enemy_plan`.
This helper is verified in isolation. Gameplay still uses minimum counts and does not respawn enemies.

Each valid resource row selects a count from its minimum/maximum range.
The original takes a signed Random word, applies NEG.W when negative, then uses signed division remainder.
The -32768 input remains negative after NEG.W; Rust preserves this edge case.
Kind selection uses the signed low byte. Repeated kinds overwrite earlier rows.
The life budget is the resource rebirth word plus `(effective difficulty / row count) / 5`.
Both divisions truncate toward zero. CODE 1 +0x0882 supplies effective difficulty.

After summing the count table, the code limits simultaneous participants.
The threshold is 12, or 6 when A5-0x7be6 is nonzero.
It scans kinds 0..46 repeatedly. Counts above one lose one instance and gain one life.
The loop stops when the total is strictly below the threshold: at most 11 or 5 participants.
This is not a simple clamp, and it changes the tables used by completion par time.
The meaning and valid callers of the reduced-capacity flag need further tracing.

The differential harness executes the original instruction block with supplied Random outputs.
Resource lock/release and model loading are explicit service fixtures.
It tests 153 nonempty disc resources and 200 synthetic inputs, including repeated kinds.
Four supplied resource/settings combinations stall after every occupied kind reaches count one.
The harness verifies their loop location, total, and counts after bounded execution.
Rust returns None for these cases instead of hanging. This does not establish a reachable original-game bug.
Rust also guards empty rows, invalid ranges, and missing Random samples.

All 48,354 differential cases pass, including the four host-guard cases.
All 38 Rust tests and strict Clippy pass. Hosting remains stopped.
The Random generator, enemy placement, and enemy lifecycle are not recovered by this helper.


## Enemy life sharing and retirement — CODE 31 +0x19da and +0x1bd0

CODE 31 +0x19da scans participant slots 1..12 in order.
It finds the first different participant with matching kind, more than one life,
and a nonzero bitwise intersection of its active and enemy flag bytes.
It subtracts one life from that donor and sets the recipient's lives to one.
It returns false without changes if no donor qualifies.
The donor's health is not tested. The recipient's current life count is not tested within this helper.

The caller at +0x1bd0 runs after the lifecycle dispatcher establishes a dead participant.
A nonzero enemy flag and zero A5-0x12ca mode byte enable life consumption.
It decrements the signed life word, preserving wrapping behavior.
When the result is nonpositive, it calls the transfer routine.
Only a failed transfer reaches the retirement branch at +0x1c0c.
Other outcomes proceed to health/ammo refill at +0x1cb8.

The retirement branch sets participant byte +6, removes geometry,
sets the geometry index to -1, changes kind to 14, and decrements A5-0x1372.
Its last-enemy bonus path remains to be recovered and verified.
Placement happens earlier through the lifecycle call at +0x1bba; its behavior remains a separate research target.
Do not infer an independent per-enemy respawn counter or delay from these routines.

Rust helpers `borrow_enemy_life` and `consume_enemy_life` preserve this table behavior.
The harness executes 600 complete original transfers and 600 original caller gates.
These cases use no service fixtures or OS traps. They cover all recipient slots,
raw flag bytes, signed life boundaries, absent donors, and mode suppression.
Transfer fixtures also verify that all other participant bytes remain unchanged.
All 49,554 differential cases, 39 Rust tests, and strict Clippy pass.
Gameplay integration remains pending; hosting remains stopped.


## Spawn coordinate proposals — CODE 31 +0x0194

The complete helper now has a verified Rust implementation, `spawn_coordinate`.
It returns one 16.16 coordinate from a supplied signed Random word.
The placement routine calls it independently for both ground coordinates.
This helper proposes a location; it does not establish that the location is safe.

For an arena extent E and ordinary nonnegative Random magnitude R:
- Non-enemies use `100 + 2 * (R % (((E - 200) / 2) + 1))`.
- Enemy types 35 and 45 use `E / 8 + R % ((E * 3) / 4)`.
- Other enemy types use `R % E`.

The type mask is the six bytes at CODE 31 +0x0292: 20 08 00 00 00 00.
Its reverse-byte bit indexing selects exactly types 35 and 45.
For E=1024, the ranges are 100..924 in steps of two, 128..895, and 0..1023.
Those ranges assume ordinary Random magnitudes. NEG.W leaves -32768 negative,
so that exceptional input can propose coordinates outside those ranges.
The Rust helper preserves word wrapping, signed remainder, and the fixed-point conversion.
Nonpositive divisors return None as an explicit host guard.

The broader placement routine starts at CODE 31 +0x04b4.
It proposes a random heading once, then loops over candidate coordinates.
The loop clears geometry fields +0x24 and +0x28, applies special placement branches,
updates geometry through CODE 30 +0x0f60, and calls collision/participant checks.
Its counter exits after exceeding 500 attempts; the exact acceptance path needs further verification.
Kind 20 has a separate flag-relative placement branch. Selected modes also alter coordinates.
These paths and the original Random generator are not covered by the coordinate helper.

Validation executes 768 complete original helper calls with supplied Random trap results.
It covers every signed kind byte, enemy flags 0/1/255, all participant slots,
five arena extents, negative Random values, and -32768.
The original multiplication helper executes unchanged.
All 50,322 differential cases, 40 Rust tests, and strict Clippy pass.
Live placement still uses its documented preview search. Hosting remains stopped.


## Normal spawn retry loop and participant clearance

`normal_spawn_search` recovers CODE 31 +0x04b4 for mode zero and kinds other than 20.
It consumes one Random word for heading, then two words for each proposed position.
Heading stays fixed across retries. Geometry/contact services remain external callbacks.
The loop exits on a clear candidate or after attempt 501, retaining that final candidate even when blocked.
The Rust result exposes this distinction through `clear`; it does not label every returned position safe.
The original checks the low bit after XOR 1. Nonboolean callback bytes preserve that behavior.

The harness executes 160 complete original placement calls.
Original coordinate proposals, heading arithmetic, and loop control execute unchanged.
Geometry commit, solid-contact query, and participant clearance are explicit fixtures in this test group.
Cases include immediate acceptance, repeated rejections, exhaustion, and nonboolean contact bytes.
These tests do not verify special placement branches or the whole original collision system.

`spawn_participant_blocked` separately recovers CODE 31 +0x0332 and +0x0298,
including the actual CODE 25 +0x0154 proximity helper.
Proximity uses wrapping absolute X and Y differences, adds them,
then compares the signed high word against a strict distance threshold.
For ordinary arena coordinates, this is Manhattan distance with fractional bits discarded.
It is not a radius or a collision-footprint test.

Normal mode uses threshold 250 between the local player and other participants.
Modes 2, 4, 11, and 12 use 60; other nonzero modes use 120.
The local-player path scans other active participants with different geometry IDs.
The nonlocal path first checks the local player, then scans other active enemy participants at threshold 25.
That second scan uses the bitwise intersection of active and enemy bytes.
Negative geometry IDs never block. Zero or unavailable geometry IDs are guarded in Rust.
The original only guards negative IDs before reading geometry memory.

600 complete clearance executions use original scan and proximity instructions without service fixtures.
They cover local/nonlocal slots, duplicate and negative geometry IDs, modes, raw flags,
and signed coordinate overflow. A unit test verifies retry ordering and blocked final-candidate retention.
All 51,082 differential cases, 41 Rust tests, and strict Clippy pass.
Live placement still uses the preview search. Solid-contact wiring, special modes, and kind-20 placement remain pending.
Hosting remains stopped.


## Special placement transforms — CODE 31 +0x0594 and +0x0608

`spawn_team_coordinates` recovers the coordinate transform at +0x0608..+0x070e.
The two-byte mask at +0x0774 is 90 04, selecting modes 2, 12, and 15.
Only participant team bytes 0 and 1 enter this branch.
Both ground coordinates are divided by two with signed truncation toward zero.
For each axis, the associated team anchor is compared strictly against 500 in 16.16 units.
If the anchor exceeds that boundary, the halved coordinate becomes `extent - coordinate`.
The boundary stays 500 even for extents other than 1000. Heading is unchanged.
Other modes and team bytes leave the proposal unchanged.

`spawn_beside_flag` recovers the arithmetic at +0x0594..+0x0604.
The preceding gate requires participant kind 20 and kind 12 at the current geometry cursor.
The branch copies that flag's position, chooses a new random heading,
and adds 30 times each output of the direction lookup.
It then increments the geometry cursor at A5-0x1300.
The Rust arithmetic helper accepts the direction lookup values explicitly.
It preserves the exceptional -32768 Random behavior and wrapping coordinate additions.
The cursor's initialization and bounds still need tracing before this branch can be integrated safely.
This evidence does not identify every gameplay role of kind 20.

Validation adds 600 original team-transform executions with original arithmetic helpers.
They cover selected and unselected modes, valid and invalid team bytes,
four extents, signed coordinates, and either side of the exact 500-unit boundary.
Another 400 original flag-relative branches use supplied Random and direction lookup services.
Those fixtures verify final position, heading, and cursor advancement.
The full trig table is not verified by these fixtures; its existing SANE modeling caveat remains.
All 52,082 differential cases, 42 Rust tests, and strict Clippy pass.
Live placement remains unchanged until the recovered branches and solid-contact query are connected.
Hosting remains stopped.


## Combined placement ordering and flag cursor correction

The combined `spawn_search` now joins coordinate proposals, flag-relative placement,
team transforms, geometry position updates, and retry decisions.
`normal_spawn_search` delegates to it for the normal branch.
Geometry/contact services and the direction lookup remain explicit caller inputs.
This is not yet the full runtime placement system.

CODE 25 +0x21ca initializes A5-0x1300 to geometry entry 1 during wave preparation.
The placement branch checks that exact entry for kind 12; it does not search for another flag.
It advances the cursor before collision acceptance. A rejected candidate can therefore consume a flag entry.
Each attempt still consumes two coordinate Random words before a flag override.
A new flag heading persists into subsequent attempts, including attempts after the flag run ends.
Team anchors are read from current geometry, so retries can observe an earlier candidate's committed position.
The Rust routine updates its geometry positions accordingly.
Missing required geometry entries return None as a host guard; this is not an original bounds check.

Correction: the flag-relative caller is type 20, not type 14.
CODE 31 +0x0576 loads hexadecimal $14, which equals decimal 20.
The isolated arithmetic tests began after this gate and could not detect the earlier labeling error.
The full-call comparisons exposed it. Earlier placement notes and the progress page are corrected.
Other uses of decimal type 14, including flag-slot conversion and retirement, remain distinct and unchanged.

240 complete original placement calls now compare final poses, attempts, cursor changes,
and exact Random word consumption across combined branches and retries.
Geometry commit, collision queries, and direction lookup remain supplied service fixtures.
The harness invalidates translated CODE 31 blocks before these calls so newly installed service hooks take effect consistently.
All 52,322 differential cases, 43 Rust tests, and strict Clippy pass.
The current gameplay placement is still provisional. Hosting remains stopped.


## Player respawn placement connected to gameplay

Mission player respawns now call the recovered normal spawn search.
Each death gets a new heading and proposed coordinates, with original retry ordering and the 501-attempt cap.
The original normal-mode participant clearance checks the first eleven preview target slots.
Dead targets are inactive. Initial wave placement and enemy respawning remain unchanged.
The final candidate is retained on exhaustion, with `tank.blocked` exposing that outcome.

The collision adapter can query a proposed player pose through the recovered participant contact scan.
It recomputes the candidate grid cell and excludes the player's old grid entry by ownership.
This uses the existing preview record adapter and modeled-SANE direction table.
Unsupported models or query inputs fall back to the existing footprint collision check.
Original record lifecycle and grid scheduling are not claimed by this integration.

Random words come from a deterministic xorshift host stream, seeded with 0x53565231 on mission reset.
This is explicitly provisional; it does not reproduce the Mac OS Random generator or original seed lifecycle.
The stream persists across player deaths. Restart restores it for reproducible native/WASM behavior.
Health, ammo, flags, score, enemies, shots, and simulation-clock handling retain the existing lifecycle rules.

Validation: 44 Rust tests and strict Clippy pass; the WASM build succeeds.
All 112 mission starts and resets, three native/WASM mission replays, game replays, and encounter checks pass.
The authored Central arena verifies two randomized WASM respawns, preserved progress, and restart.
The previous idle-until-final-death test became unreliable because provisional enemies can get stuck after relocation.
A separate in-memory open-field, single-shooter fixture verifies three deaths and the final simulation freeze.
That fixture edits no original files and is not evidence of authored-arena navigation fidelity.
Initial placement, enemy life integration, original Random generation, and enemy navigation remain pending.
Hosting remains stopped.


## Enemy lives and respawns connected to missions

Flag missions now populate targets from the recovered enemy plan with effective difficulty 1
(default difficulty zero plus preview level one). Random outputs are explicitly zero,
selecting resource minimum counts before original cap balancing.
Types are populated in ascending order, matching the original cumulative count scan.
Duplicate resource types use the plan's overwritten values.
Research and elimination encounters retain their one-life setup.

Each target tracks remaining lives, deaths, and permanent retirement separately.
A successful kill increments deaths and awards its resource score once.
The mission lifecycle proposes placement before consuming the life, preserving that original ordering.
It then applies the recovered same-type transfer scan and retirement gate.
An enemy with lives left returns at the proposed pose. A retired target stays dead.
Refill uses the stored resource health capped at nine, following CODE 23 +0x0684.
The HUD displays Kills and Retired separately; reset restores initial lives and counters.

Enemy placement uses the combined recovered search and participant clearance.
The collision adapter now supports a proposed enemy record as well as the player.
Supported geometry uses the recovered contact query; unsupported inputs use the preview footprint check.
The current entity order, ownership adapter, and geometry lifecycle remain preview conventions.
In particular, removal of collected flag items does not yet preserve original geometry slot identity.
Type-20 placement uses this preview geometry list; exact cursor-to-slot behavior after collection remains pending.
Random generation and direction tables retain their documented provisional sources.
AI scheduling, enemy ammunition, last-enemy awards, and campaign progression remain unfinished.

Validation: 45 Rust tests and strict Clippy pass. The WASM build succeeds.
A new lifecycle test verifies repeated respawns, borrowing from a same-type donor,
permanent retirement, no repeated processing after retirement, and full reset.
Actual WASM shooting verifies a killed enemy returns with health, cumulative kill scoring,
and agreement between per-target deaths and the kill counter.
All 112 mission starts/resets, native/WASM mission and game replays,
player lifecycle checks, and encounter checks pass. Hosting remains stopped.


## Preserve placement slots when collecting flags

Collected flags now become kind-14 placeholders in a separate placement item list.
This follows the object-kind write in CODE 28 +0x059c.
The visible/query item list can still remove a collected flag without shifting placement slots.
An explicit index map connects current items to their original placement slots.
It does not infer identity from coordinates, which may be identical for different flags.

Enemy placement now uses the retained list and its length when assigning participant geometry indices.
The contact adapter keeps its own compact record indices; these are passed separately.
The flag cursor continues to check its current slot without scanning forward.
Thus a collected flag cannot cause a type-20 respawn to use the next flag by accident.
Reset restores both lists, the index map, and the initial cursor.
This replaces the flag-removal limitation recorded in the preceding integration entry.
Original model-load ordering and the broader geometry lifecycle remain unverified.

A regression test collects duplicate-position flags separately, respawns type 20,
checks that the cursor stays at the collected slot, then verifies full reset.
All 46 Rust tests, strict Clippy, and the WASM build pass.
All 112 mission starts/resets, actual enemy and player respawn checks,
native/WASM game and mission replays, and encounter checks pass.
Hosting remains stopped.


## Last-enemy retirement bonus

CODE 31 +0x1c26 decrements the remaining-enemy word after permanent retirement.
When that word becomes nonpositive, the caller requests presentation and computes a score award.
`last_enemy_bonus` recovers +0x1c40..+0x1cb2 through the score-call boundary.
It sums initial count times initial life budget over all 47 types.
The sum and subsequent multiplication by 50 use signed-word truncation/wrapping.
The level multiplier is 1 below level 21, 2 for levels 21..26,
and the greater of 1 or level divided by 9 thereafter.
The score handler receives the signed low word of the final product.

Mission setup stores this award from the initial recovered enemy plan, using preview level 1.
It is enabled only when all planned participant slots were populated.
The last permanent retirement awards it once through the recovered score handler.
Kills and intermediate respawns do not trigger it. Restart clears its awarded state.
The flag objective remains active after enemy clearance.
Frame output and the HUD expose the awarded amount separately from cumulative score.
The associated original presentation/audio request is not implemented in this change.

500 original computational executions cover multiplier boundaries, negative levels,
large counts/life budgets, and signed-word overflow. Original arithmetic helpers execute unchanged.
All 52,822 differential cases pass. All 46 Rust tests and strict Clippy pass.
The extended lifecycle test checks that a surviving enemy prevents the award and repeated ticks cannot duplicate it.
An actual WASM open-field, single-life-enemy fixture verifies a 50-point bonus after retirement,
ordinary kill points, continued flag play, one-time award behavior, and reset.
All 112 mission starts/resets, player/enemy respawn checks, native/WASM replays, and encounter checks pass.
Hosting remains stopped.


## Original event sound IDs connected to browser audio

CODE 15 +0x0b42 calls +0x006c, which passes the request's low word to +0x0870.
That routine ignores nonpositive signed IDs. Positive IDs below 10000 gain 10000;
IDs at or above 10000 pass through unchanged.
It stores the selected ID at A5-0x42f8 and calls the sound backend with that ID and zero.
The upper request word and the wrapper priority do not affect this path's ID selection.

512 full wrapper executions verify this routing through the captured backend boundary.
They cover zero, signed boundaries, the 10000 transition, actual event requests,
random upper words, and random priority words. The Mac sound backend is not emulated.
The original sample manifest independently confirms these names:
- Request 0x00010011 -> 10017, flagPull.
- Request 0x00010017 -> 10023, OneMoreFlag.
- Request 0x0001002a -> 10042, newMan.
- Request 0x0001002c -> 10044, allRobotsKilled.

Flag collection emits OneMoreFlag when one flag remains, otherwise flagPull.
Each earned extra life emits newMan. Permanent retirement of the last enemy emits allRobotsKilled once.
The browser preloads these four samples alongside its existing seven combat samples.
The prior resource-name-based combat routing remains provisional.
Original mixing, interruption, scheduling, and desktop audio are not implemented by this change.

All 53,334 differential cases, 46 Rust tests, and strict Clippy pass.
All 105 exported samples still match original PCM bytes.
WASM verifies authored flag pickup audio and exactly one enemy-clear cue, with no repeats afterward.
The Rust lifecycle test verifies extra-life audio precedes the one-more-flag cue when the pickup crosses a score threshold.
The WASM build, 112 mission start/reset checks, native/WASM mission replays, and lifecycle checks pass.
No audible browser listening check was performed; hosting remains stopped.


## Arena combat counters connected to gameplay

`CombatCounters` records player launches, player-attributed hits, other launches,
and player damage-handler events. These are the four inputs to the recovered completion ratings.
CODE 30 +0x0292 increments launches only after successful projectile slot allocation.
An empty-ammo attempt does not count as a launch.

The damage prefix at CODE 31 +0x0a84..+0x0b9a rejects victim geometry ID -1.
In normal mode, local-player damage is rejected while the signed elapsed uptime is below 75 ticks.
Nonlocal damage clears the grace timestamp. The only initialization found is wave preparation at CODE 25 +0x21a4.
The prefix then applies type-31 protection and health subtraction before updating counters.
A local victim increments damage events. Otherwise, a local attacker increments player hits.
Zero damage and a protected type-31 target still count once the handler admits the event.
Self-inflicted local damage counts as a damage event, not a player hit.

600 complete original damage prefixes verify admission, counter writes, and grace timestamp changes.
TickCount and the type-31 phase lookup are supplied services; the original energy/minimum helper executes unchanged.
Cases include missing victims, the 74/75-tick boundary, modes, ownership, protected targets,
zero damage, and counter wrapping. Another 200 original blocks verify launch counting.
All 54,134 differential cases pass.

Both gameplay projectile collision paths now record admitted hits, including protected hits.
Enemy fire records launches; admitted player hits record damage events before health loss.
Counters persist across player and enemy respawns, and reset at mission restart.
Frame JSON and native replay output expose the same named counter fields.
The preview normalizes damage uptime to begin at 75, with the grace timestamp also 75.
This preserves a fresh arena's 75-tick grace while a cleared timestamp means expired.
It does not emulate the original system uptime or the complete damage-response scheduler.
Current ownership, collision records, AI, and projectile capacity remain preview conventions.

All 46 Rust tests and strict Clippy pass. The protected-target test verifies accuracy credit without score credit.
Empty-ammo tests exclude unsuccessful launches. WASM checks launch totals and fatal-hit counters across respawn and reset.
The WASM build, all 112 mission starts/resets, native/WASM replays, and encounter checks pass.
Completion result presentation and bonus integration remain pending. Hosting remains stopped.

## Completion ratings connected to gameplay

The final flag now freezes combat counters and applies the recovered completion ratings.
Each category bonus passes separately through the original score rules. Restart clears the results.
Target times use the recovered enemy plan, bonus table, and flag count.
The preview still selects level 1 and minimum enemy counts.
75 arenas have the required data. 35 unlinked layouts and two incomplete enemy plans omit ratings explicitly.

The browser shows Quickness, Accuracy, Avoidance, category bonuses, elapsed time, and total score.
The final sound cue can finish after gameplay stops. Campaign transitions remain pending.
The desktop shares the scoring simulation, but has no equivalent results panel yet.

A controlled Rust fixture reproduces the video's 200 + 200 + 400 bonus combination.
This does not establish the unseen counters in that recording.
A WASM fixture verifies final-flag awards, unrated categories, frozen counters, and restart.
All 47 Rust tests, strict Clippy, and the WASM build pass.
All 112 arena starts and resets pass, with native/WASM replay parity for three campaigns.
Browser visual and audible checks remain pending. Hosting remains stopped.

## Original defaults for omitted enemy parameter rows

CODE 25 +0x14dc initializes all 47 parameter slots before resource loading at +0x166c.
The loop at +0x14f6..+0x1604 writes these defaults:

| Parameter | Default |
| --- | --- |
| Hits to kill | 1 |
| Collision damage | 3 |
| Attack range | 250 |
| Short firing range | 80 |
| Firing arc | 15 |
| Firing interval | 120 |
| Radial velocity | 5 |
| Angular velocity | 1 |
| Kill score | 10 |
| Completion target-time weight | 100 |
| Weapon | 0 |

Resource entries overwrite their corresponding slots. An omitted type retains its defaults.
The resource lookup falls back to bval 777 only when the entire requested resource is absent.
That fallback does not apply to individual missing rows.

The differential harness executes the original initialization with poisoned destination memory.
All 47 resulting records match the Rust defaults. All 54,181 differential cases pass.
The playable loader now retains enemies whose types lack a bval row.
Central and South layout 10100 each recover five type-45 enemies, with one hit point and ten kill points each.
Their completion target time is 150 seconds under the preview's level-1 minimum-count plan.
Type-45-specific AI remains pending. Existing resource rows still use the preview's parameter handling.

77 linked layouts now have completion ratings. 35 unlinked layouts still omit them.
All 112 mission starts, resets, and render checks pass. Three campaign desktop/WASM replays agree.
All 47 Rust tests, strict Clippy, and the WASM completion fixture pass. Hosting remains stopped.

## Parameter row overrides and health modifiers

The Rust parameter table now applies CODE 25 +0x16b8..+0x18ea row semantics.
Type selection uses the signed low byte of each resource type word.
Only types 0 through 46 are accepted. Later rows replace earlier rows for the same type.
Type 5 updates separate globals and leaves its initialized enemy parameter slot unchanged.
The port preserves that slot behavior. It does not yet apply those separate globals.

For each ordinary row, health receives the caller's signed word modifier with wrapping arithmetic.
The loader adds another 50 when A5-0x7be4 is negative or A5-0x7be8 differs from 42.
These global conditions are retained as an explicit input, without assigning an unverified gameplay name.
Omitted rows keep initialized health; neither modifier applies to their defaults.

The normal wave caller establishes the first modifier at CODE 25 +0x1c32..+0x1c44.
It starts from the level word and repeatedly subtracts the wave count until the index fits.
Each subtraction increments the modifier. The caller passes this count at +0x1d50.
Thus repeated campaign cycles increase the health of types with resource rows.
The existing arena preview uses the first cycle and disables the separate 50-point addition.
Campaign progression and selection of the special global conditions remain pending.

The differential harness executes 122 complete original loader calls: 22 disc tables and 100 synthetic resources.
Resource Manager operations are supplied services. Original arithmetic helpers execute unchanged.
Fixtures cover duplicate rows, low-byte aliases, rejected types, type 5, signed health overflow, and both modifier branches.
Comparisons cover all 47 enemy parameter slots. Separate type-5 globals are not yet compared or ported.
All 54,303 differential cases pass.

Gameplay uses the recovered table for enemy health, kill scores, and completion target-time weights.
The existing minimum-health clamp and first-cycle scenario choices remain preview behavior.
All 47 Rust tests, strict Clippy, WASM compilation, 112 mission checks, and completion fixtures pass.
Three native/WASM mission replays agree. Hosting remains stopped.

## Campaign wave selection

CODE 25 +0x1ab6 selects wave resources with the same routine used for score thresholds.
The existing 192 original selector executions cover this shared choice.
The Rust wave selector is now the common implementation for both callers.

| Difficulty | Wave resource |
| --- | --- |
| Less than 3 | 10000 |
| 3 or 4 | 10001 |
| More than 4 | 10002 |

A nonzero A5-0x12d2 selects 778 before other conditions. Otherwise, nonzero A5-0x12ca selects 777.
Their mode meanings remain separate research targets.

CODE 25 +0x1c32..+0x1c44 wraps the one-based level through the selected resource's entries.
Each wrap increments the signed health modifier.
The branch at +0x1cc8..+0x1cd4 clears hints and opening movies after the first cycle.
Closing movies remain selected. This differs from disabling all movies on repeat cycles.

Central and South contain 73, 66, and 57 entries for the three difficulty tiers.
Eastern contains 25 entries in each tier.
For example, Central level 74 at difficulty 0 returns to entry 1 with health modifier 1.
Its opening movie and hint are cleared, while closing movie 1023 remains.

The new select_wave routine preserves zero world and enemy IDs as unresolved random selections.
It does not replace those fields with arbitrary available resources.
The host rejects nonpositive levels and empty or oversized lists before original-style indexing can access invalid data.

The harness executes 1,042 original selection blocks across all nine disc wave resources and four synthetic lists.
Every entry is covered through two cycles, together with the next boundary and level 32767.
Three additional fixtures verify explicit host guards. All 55,348 comparison and guard cases pass.
The blocks cover index wrapping, health modifiers, selected fields, and repeat-cycle suppression.
They do not include random resource resolution, song clamping, world loading, or transition scheduling.

Inspect a selection without a server:

```sh
cargo run --quiet --locked -p spectre-tools -- campaign-wave .local/extracted/central.bin 74 0
```

The final arguments are level and difficulty. The JSON marks unresolved random world and enemy selections.
The browser still runs standalone arenas. Campaign state, resource resolution, and transitions remain pending.
All 47 Rust tests and strict Clippy pass. Hosting remains stopped.

## Wave resource resolution and resource counts

CODE 25 +0x1cd4..+0x1d46 resolves zero world and enemy-table IDs.
The original consumes Random words only for zero IDs with positive resource counts.
It resolves the world before the enemy table. Nonpositive counts select resource 10000 without consuming Random.
The signed word absolute value preserves the -32768 overflow. Signed remainder can consequently select IDs below 10000.
A nonzero A5-0x7be6 adds 1000 to the world ID, including explicitly selected worlds.
Its startup meaning remains unresolved. The inspection tool uses zero for this byte.

CODE 25 +0x1a3a and +0x1a78 count resources consecutively from ID 10000.
They stop at the first missing resource, so later IDs do not expand the random selection range.
Both the ID and count use wrapping signed words. ID -1 terminates scanning without a lookup.
The shared Rust implementation preserves these rules. Central has nine consecutive worlds and fifteen consecutive enemy tables.

600 original resolution blocks verify IDs and random consumption, with Random supplied as a service.
Three host guards reject insufficient supplied samples. They do not invent replacement random values.
32 complete original enumeration calls verify gaps, signed wrapping, and the -1 sentinel.
All 55,983 comparison and guard cases pass. This does not recover the original Random generator or seed.

The campaign-wave inspection command accepts signed Random words after level and difficulty.
Its JSON reports contiguous counts, consumed samples, resolved IDs, and whether more samples are required.
The counts use the recovered enumeration function.

The disc audit covers all 467 entries across nine campaign wave tables.
Every entry explicitly selects its world and enemy table; none requires random resource resolution.
Every referenced world, enemy table, and bval resource exists in its campaign archive.
The local audit is saved in .local/disassembly/campaign-reference-audit.json.
Thus this fallback is supported, but is not a blocker for loading the shipped campaigns.
Random enemy quantities and spawn placement remain separate questions.

All 47 Rust tests and strict Clippy pass. Playable campaign state and transition handling remain pending.
Hosting remains stopped.

## Load playable campaign levels

Viewer::start_campaign_level now builds a game from the selected wave, level, and difficulty.
The shared target loader accepts that wave directly. It no longer substitutes the first wave referencing the same world.
Standalone arena loading retains its previous level-one behavior.

Campaign level setup applies these recovered inputs:

- The wave's world, enemy roster, and combat parameter table
- The level-specific flag conversion
- The cycle health modifier
- The selected difficulty's extra-life score table
- The level-specific last-enemy bonus
- The loaded enemy plan and flag count for completion target time

Enemy quantities still use explicit zero Random samples. Placement still uses the existing preview initialization.
The special 50-point health override remains disabled.
Initial effective difficulty uses CODE 1 +0x0846..+0x086c: wrapping difficulty-plus-level, clamped to 0 through 127.
200 original executions verify this arithmetic. Elapsed-time increases and original initialization ordering remain unverified in the integrated scheduler.
All 56,183 comparison and guard cases pass.

The browser has a Campaign level mode with level and difficulty inputs.
Each selection starts a fresh round. It does not carry scores or lives across levels or advance automatically.
The WASM entry point is game_start_campaign_level(level, difficulty).
Failed loads preserve the active core game. Frame JSON and native replay output include campaign metadata.

Inspect a playable setup offline:

```sh
cargo run --quiet --locked -p spectre-tools -- campaign-level .local/extracted/central.bin 74 0
npm run test:campaign
```

The campaign test loads all 467 shipped entries and nine first-repeat boundaries.
All 476 setups reset to their initial state. Eighteen native/WASM setups agree exactly.
Invalid levels and difficulties preserve the existing game.
All 112 standalone mission checks, completion fixtures, and native/WASM gameplay replays pass.
All 47 Rust tests, strict Clippy, the WASM build, and JavaScript syntax checks pass.
Browser visual interaction remains unverified while hosting is stopped.

## Continue completed campaign rounds

The shared Viewer now advances a completed campaign round to the next wave.
It requires a flag victory, computed completion results, and at least one remaining life.
The new game retains score, lives, and the port's diagnostic death count.
Round counters, flags, enemy state, projectiles, and completion results start fresh.
A failed next-level load leaves the completed arena and its awards intact.

CODE 25 +0x23b6..+0x2406 refills the local participant's health and ammunition.
The original CODE 23 helpers cap signed shield capacity at nine and multiply signed ammunition capacity by ten.
The differential harness executes 200 original refill blocks, including those helpers.
It verifies every participant byte remains unchanged except health and ammunition.
All 56,383 comparison and guard cases pass.
The selected Tank 1 settings currently refill six health and forty rounds.
Later vehicle upgrades, powerups, original transition scheduling, and movie playback remain pending.
The refill block does not establish the behavior of those other transition phases.

The reviewed first transition shows the awarded score continuing from 135 to 935.
The browser now offers Next level after campaign completion and resumes driving after a successful load.
This is an explicit continuation action; it does not reproduce the original results-screen timing.
Selecting a level manually starts a fresh run.

Restart restores the current level's entry progress, including carried score and lives.
Awards and deaths earned during that level are rolled back with its state.
This prevents repeated restarts from accumulating completion bonuses.

An explicit WASM fixture completes level one, advances to level two, and verifies carryover, refill, and restart.
It rejects unfinished rounds, duplicate advancement, and missing next-world resources without losing the current game.
A Rust test verifies restart after score and life changes. All 48 Rust tests and strict Clippy pass.
The browser controls pass syntax checks; visible interaction remains unverified while hosting is stopped.

## Earned campaign vehicle upgrades

CODE 25 +0x1ff6..+0x20dc attempts one upgrade when the incoming level exceeds ten and is divisible by eight.
It calls the original CODE 23 +0x0078 validator before changing a setting.
The priority is shields, ammunition capacity, then speed.
An accepted shield increment must be 1 through 10. Ammunition must be 0 through 5. Speed must be 1 through 8.
These limits apply to signed-byte settings plus one. Invalid settings retain the original fallback behavior.
All other settings remain unchanged.

For an unchanged Tank 1 preset, earned upgrades follow this sequence:

| Incoming level | New setting |
| --- | --- |
| 16 | Shields 7 |
| 24 | Shields 8 |
| 32 | Shields 9 |
| 40 | Shields 10 |
| 48 | Ammunition capacity 5, or 50 rounds |
| 56 | Speed 6 |
| 64 | Speed 7 |
| 72 | Speed 8 |

The original refill helper caps hull at nine even when the shield setting is ten.
The supplied video ends at level 14, before this routine's first upgrade. This finding comes from executable evidence.

572 fixtures execute the complete original upgrade routine with its original validator and minimum helper.
Cases include valid presets, exhausted upgrade paths, negative levels, signed-byte edge cases, and unrelated settings.
All 56,955 comparison and guard cases pass.

Next level applies the upgrade once before refilling the new tank.
Vehicle settings persist across death, advancement, and round restart.
Movement uses the current speed limit. Respawn uses the current hull and ammunition settings.
The browser hull denominator follows the capped current shield setting.
Direct level selection remains a fresh Tank 1 run without historical upgrades.
The original startup path's conditional replay of earlier upgrades is not yet reproduced.

A WASM fixture completes synthetic level 15 and verifies the level-16 shield upgrade, refill, and restart.
A Rust lifecycle test verifies upgraded speed, ammunition, hull, death, and restart.
All 49 Rust tests and strict Clippy pass. Browser interaction remains unverified; hosting remains stopped.

## Original type-7 firing predicate and planar distance

CODE 20 +0x1e9a..+0x2220 evaluates firing eligibility.
The new Rust type7_fire_check covers its type-7 enemy path with a target present.
It returns eligibility and the updated timer. It does not allocate a projectile.

The type-7 path rejects supplied fixed-point distance above 300 units before initializing its timer.
A timer of -1 becomes the current tick.
The normal path becomes eligible when timer plus the signed resource interval is at most now.
An earlier path becomes eligible after twenty ticks when distance is within the resource's short firing range.
All timestamp additions wrap as signed longs.
Negative target geometry field +0x28 rejects firing. Its gameplay meaning remains unnamed here.

The angle service supplies a signed word. Its wrapping absolute value must be at most four,
or angle times the signed integer distance must be at most firing_arc times 200.
The -32768 absolute-value overflow remains part of the recovered arithmetic.
The caller at CODE 20 +0x26dc writes the firing timer after accepting the command.
That command is not proof that the weapon allocator successfully created a projectile.

The differential harness executes 800 complete original type-7 predicate calls.
Only the angle-service result is supplied. Original range, time, table lookup, target-state, and arc branches execute.
Fixtures select the ordinary angle-service path; the special predictive-angle branch remains unverified.

CODE 20 +0x0004 obtains distance through CODE 6 +0x17c6 and +0x1760.
For ordinary coordinates, it is max(abs(dx), abs(dz)) plus half the smaller separation.
The implementation preserves wrapping coordinate subtraction, signed absolute-value overflow, and the final logical right shift.
300 full original distance-helper executions verify ordinary and extreme coordinate inputs.
All 58,055 comparison and guard cases pass.

Live encounter AI still uses provisional steering, firing cadence, distance, and aiming checks.
The recovered predicate is not yet integrated: angle-service semantics and caller timer initialization still require work.
This prevents an unverified input conversion from disguising itself as original behavior.
Hosting remains stopped.

## Game-side aiming angle and AI timer initialization

CODE 6 +0x1680 computes the aiming error used by the ordinary angle-service path.
It subtracts fixed-point coordinates with wrapping arithmetic, then uses their signed high words.
Fractional differences are discarded before quadrant selection.
The routine rotates until the first difference is positive and the second is nonpositive.
Each rotation subtracts ninety degrees from the quadrant correction.
A near-horizontal branch bypasses the OS conversion when the first difference divided by 256 exceeds the second difference's absolute value.

Other directions call FixRatio and AngleFromSlope. The callback receives the first difference as numerator and the negated second difference as denominator.
The helper then applies its quadrant correction and subtracts the source heading's signed high word.
Normalization produces -179 through +180 degrees. Coincident integer coordinates leave the caller's result slot unchanged.
The Rust interface returns None for that unwritten result.

Apple's [QuickDraw Reference, page 50](https://leopard-adc.pepas.com/documentation/Carbon/Reference/QuickDraw_Ref/QuickDraw_Ref.pdf) describes AngleFromSlope as accurate within one degree.
It does not promise nearest-degree rounding. Thus a rounded atan2 replacement is not established as identical.
The Rust helper exposes the combined OS conversion as a supplied callback. Its exact implementation remains pending.

800 complete original helper calls verify argument preparation, quadrant rotation, bypass behavior, heading subtraction, and normalization.
The harness supplies explicit OS results and compares both service arguments and final outputs.
Three signed-coordinate boundary cases cycle indefinitely in bounded original execution.
Rust stops after four rotations and returns None for these guarded inputs.
All 58,858 comparison and guard cases pass.

CODE 20 +0x2a30 resets participant AI state. At +0x2a58 it writes -1 to the firing timer at participant offset +0x44.
CODE 25 calls this reset during wave setup at +0x2224 and +0x24dc.
The firing predicate initializes that sentinel when it admits the distance check.
The accepted firing-command path at CODE 20 +0x26dc writes the current tick.
This establishes timer locations and the reset value; full AI scheduling and predictive aiming remain unfinished.

Live AI remains unchanged. All 49 Rust tests, strict Clippy, and WASM compilation pass. Hosting remains stopped.

## Historical QuickDraw angle conversion

The Computer History Museum publishes [Apple's 1984 QuickDraw source](https://computerhistory.org/blog/macpaint-and-quickdraw-source-code/).
Its [Angles.a implementation](https://github.com/computerhistory/Historical-Source-Code-Quick-Draw-Repository) provides a reference for AngleFromSlope.
This is historical source evidence, not verification against the System 7 ROM used by Spectre.

The routine takes a wrapping absolute value of the signed fixed-point slope, then subtracts 500.
It scans the positive slope table until that biased value is at most the current entry.
For negative input, it returns the table index. Otherwise, it returns 180 minus that index.
The table includes values that differ from freshly calculated tangent values. Nearest-degree atan rounding does not establish equivalence.

Rust historical_angle_from_slope implements this scan with an explicit 91-entry table argument.
No historical table or upstream source is embedded in the port.
The reference checkout, assembled binary, and generated fixtures remain in ignored .local storage.

scripts/verify-quickdraw-angle.py adapts assembler syntax and assembles the historical routine with vasm.
It executes the reference with Unicorn M68000 and compares its outputs with Rust.
All 1,551 cases pass, including signed limits, zero, seeded random values, and both sides of every table boundary.
The report records source and binary hashes in .local/disassembly/quickdraw-angle-vectors.json.
These cases are separate from the 58,858 game executable comparisons and guards.

FixRatio behavior and System 7 AngleFromSlope equivalence remain unresolved.
Live AI still uses its provisional aiming conversion; this reference routine is not connected to gameplay yet.

## AI steering commands and clearance search

CODE 20 +0x1b8e..+0x1c88 contains a movement decision inside the +0x1672 policy.
Rust ai_movement_commands covers this block for nonlocal participants with initially cleared steering and throttle commands.
The policy clears both commands at +0x16d6. Local-participant branches and later overrides remain separate.

The block subtracts current heading from desired heading with signed long wrapping.
It subtracts 360 degrees above +180, or adds 360 at or below -180.
This is one correction, not general modulo normalization.
Errors above +1 degree request steering +1. Errors below -1 degree request steering -1.
Both exact one-degree boundaries remain inside the dead zone.

Movement requires the wrapping absolute heading error to be strictly below the stored movement-angle threshold.
The reverse flag selects throttle -1 and a travel heading opposite the current heading.
Otherwise, throttle is +1 and the travel heading stays unchanged.
An error above ten degrees also requires a successful clearance probe. A failed probe clears throttle.
The probe receives width 40 and the policy's stored probe distance.
The differential fixtures use distance 80 and compare the exact probe headings and command outputs.

CODE 20 +0x06ea searches for a clear heading through the same probe.
Its offset order is 0, +15, -15, +30, -30, through +180 and -180 degrees.
Each candidate uses the original angle wrapper. The first successful probe supplies the result.
Both half-turn candidates execute, although they usually represent the same direction.
If all probes fail, the original leaves its result slot unwritten. Rust ai_clear_heading returns None.

The next dependency is CODE 20 +0x05b0, the clearance probe itself.
Static tracing shows a projected boundary check followed by CODE 7 +0x06ba collision selection.
No collision or a selected type-32 object permits movement. This probe still needs execution fixtures.
The complete policy also includes target selection, reverse-state updates, and later throttle overrides.
The live encounter retains its provisional steering until these dependencies are connected.

684 original movement-decision blocks and 216 complete heading searches match Rust, with clearance-probe results supplied.
All 59,758 game comparison and guard cases pass.
All 49 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.

## Complete AI clearance probe

CODE 20 +0x05b0 obtains the participant's geometry pose and replaces its heading with the proposed travel heading.
CODE 18 +0x0dcc selects direction-table entries from the heading's signed integer word.
The probe uses only the signed low word of each direction value.
It multiplies these words by the signed high word of twice the fixed-point probe distance.
This preserves truncation and wrapping at the original instruction boundaries.

The boundary check projects the current position by twice that displacement.
It rejects either signed coordinate word below zero or above the arena side length.
Exact equality with either boundary passes this check.
The collision center uses one displacement, halfway to the boundary-test point.

The probe calls CODE 7 +0x06ba with that center, the proposed heading, width, distance, and excluded geometry ID.
No selected collision permits movement. A selected type-32 object also permits movement.
Any other selected object blocks movement.
This uses the selector's result, not a separate scan that ignores all type-32 objects.

Rust ai_clearance_probe covers the arithmetic and selected-kind decision.
CollisionQueryWorld::ai_clearance composes it with the recovered collision selector and preserves invalid-input errors.
1,000 complete original probe executions match Rust with collision results supplied.
These fixtures compare collision arguments and cover signed limits, table truncation, arena boundaries, and selected object kinds.

Another 400 complete original probe executions include the real collision selector and synthetic arena geometry.
No service results are supplied in these composed cases. They produce 223 blocked and 177 clear outcomes.
Both implementations receive the same candidate direction table, whose original SANE initialization remains unverified.
All 61,158 game comparison and guard cases pass, along with 49 Rust tests, strict Clippy, and the WASM build.

Live AI integration still requires the complete movement policy and its reverse-state transitions.
The preview geometry adapter also remains distinct from the original runtime record loader.
Hosting remains stopped.

## Reverse selection and complete ordinary movement policy

CODE 20 +0x197e..+0x1a46 updates the reverse byte, desired heading, and heading-refresh deadline.
The reverse decision compares the selected clear heading with the target heading, not the current vehicle heading.
The signed difference receives one half-turn correction, as in the steering block.
For nonlocal participants, an absolute difference strictly above ninety degrees selects reverse in mode zero.
Participant 1 uses a 130-degree threshold. Exact threshold values do not select reverse.

The mode byte is XORed with one and ANDed with the comparison result.
Rust preserves this byte behavior for noncanonical inputs, rather than treating every nonzero mode as true.
If reverse is nonzero, the desired heading rotates by 180 degrees through the original angle wrapper.
If movement is disallowed, reverse clears and the desired heading stays unchanged.
The refresh deadline becomes now plus twenty ticks, or ten ticks for participant 1, with signed long wrapping.
1,004 original block executions verify these outputs and preservation of unrelated participant bytes.

Rust ai_standard_movement covers the complete CODE 20 +0x1672 path for behavior 3, nonlocal participants, and mode zero.
It uses the recovered planar distance, capped at eighty units, as the clearance-probe distance.
A deadline of -1 or a reached deadline triggers a new target angle and clear-heading search.
The search uses width forty. The movement-angle threshold becomes forty degrees.
The reverse decision then updates the heading and deadline before steering and throttle selection.
Before that deadline, the stored heading, threshold, and reverse state remain in use.

The caller clears the search result slot before calling CODE 20 +0x06ea.
If all clearance probes fail, the search leaves that zero unchanged.
Thus this path uses heading zero, despite its later comparison against a -1 failure sentinel.
The Rust composition preserves the caller's zero fallback.

400 complete original policy calls verify state updates, output commands, aim-call counts, and ordered clearance arguments.
These comparisons supply the aim result and clearance responses explicitly.
They include reached and future deadlines, blocked searches, late successful probes, reverse state, and signed tick limits.
Other behaviors, mode-specific overrides, and target selection remain outside this recovered path.
Live encounter steering remains provisional until its state and geometry adapters use this policy.

All 62,562 comparison and guard cases pass. All 49 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Attack transition and movement-policy dispatch

The AI has separate behavior and movement-mode bytes at participant offsets +9 and +16.
The behavior-3 path recovered above does not establish which movement mode the outer dispatcher selects.
CODE 20 +0x253a reads the movement-mode byte and selects the movement subroutine.

| Movement mode | Condition | Selected path |
| --- | --- | --- |
| 0 | No qualifying nearby predecessor | +0x1672 with mode argument zero |
| 0 | Nearby predecessor | No movement call |
| 1 | Nearby predecessor, or distance strictly between 30 and 120 | +0x1d7c, turn only |
| 1 | Otherwise | +0x1672 with mode argument one |
| 2 | Always | +0x1d7c, turn only |
| 4 | Always | +0x15fa, alternate policy |
| Other | Always | No movement call |

The distance boundaries are strict. Exactly thirty or 120 units selects the mode-one movement path when no nearby predecessor exists.
The earlier scan at +0x2290 checks lower participant indices and a sixty-unit distance threshold.
Its state flags and complete scan remain separate recovery work.
3,072 original dispatch executions verify every byte mode, both nearby states, and both sides of each distance boundary.
The harness captures actual callee addresses and call arguments.

CODE 20 +0x0e6c enters attack tracking unless the current behavior is 4 or 9.
It sets behavior zero, movement mode one, and both target fields to the signed target byte.
It resets the heading-refresh deadline to -1 and writes one to participant word +0x2e.
That word remains named field_26 relative to the AI subrecord until its meaning is established.
Other participant fields remain unchanged.
1,024 complete original calls cover every behavior byte and signed target boundaries.

CODE 20 +0x1d7c obtains an aiming angle and writes steering +1 above one degree, or -1 below minus one degree.
Otherwise, it writes steering zero. It leaves the supplied throttle output unchanged.
The outer command dispatcher clears throttle before this call, so that path turns in place.
509 complete original calls verify steering and throttle preservation with explicit aim-service results.

Attack tracking selects movement mode one, not the already recovered ordinary mode-zero path.
The combat-mode branch, predictive target cache, and complete command scheduling still need recovery before faithful live integration.

All 67,167 comparison and guard cases pass. All 49 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Nonlocal combat movement policy

Rust ai_combat_movement covers CODE 20 +0x1672 for nonlocal type-7 participants, behavior 0 or 3, and mode one.
The original first copies the cached predicted target pose into the participant's target-pose fields.
It computes distance from that predicted position with the recovered planar-distance helper.
Movement becomes eligible at 120 units. The clearance-probe distance stays eighty units.

When the heading deadline is reached, the cached aiming angle supplies the target heading.
Eligible movement runs the recovered clear-heading search. Otherwise, the target heading is used directly.
The movement-angle threshold becomes forty degrees, reverse clears, and the heading deadline advances twenty ticks.
Before the deadline, stored heading and reverse state remain in use.

The common steering block produces commands and can make another clearance probe.
Below thirty units, a later branch probes the opposite travel direction and can negate throttle.
Between thirty and 120 units, throttle clears directly.
The final movement-eligibility gate clears throttle below 120 units in this nonlocal path.
Thus the earlier reverse request does not survive that gate.

The below-thirty branch can read stack local -0x24 without a preceding write.
This occurs when the steering error fails the movement-angle threshold.
The reference fixtures supply the prior stack value explicitly and compare the resulting probe heading.
Native callers can supply None instead. Rust then returns None before making a probe with an undefined heading.
State updates before that guard remain visible. A unit test covers this partial-update contract and the explicit-value path.

700 complete original calls match Rust state, steering, throttle, aim-call counts, and ordered clearance arguments.
The tests supply predicted poses, cached angles, clearance responses, and prior scratch-local values.
They also verify the target-pose copy and preservation of unrelated participant fields.
The prediction cache, its angle conversion, and other behavior branches remain separate dependencies for live integration.

All 67,867 comparison and guard cases pass. All 50 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Shared AI pose and aiming-angle cache

CODE 20 +0x0256 updates cached pose, aiming angle, and timestamp at participant offsets +0x32, +0x3e, and +0x40.
Refresh requires signed wrapping elapsed time of at least five ticks.
The timestamp value -1 is not a special immediate-refresh sentinel here.
For example, now zero and timestamp -1 leave the existing cache unchanged.
Exactly five elapsed ticks permits refresh.

The updater chooses the mode passed to CODE 20 +0x0040 from participant flag +5 and object type +8.
When the flag is one, type 8 selects mode one and other types select mode zero.
Otherwise, the mode is the low bit of the flag XOR one.
Rust preserves this arithmetic for noncanonical flag bytes.

The targeting routine receives the signed low byte of the aim-target field and the source participant index.
Its returned pose replaces all three cached pose components before CODE 20 +0x0214 computes the angle.
That angle helper uses the new cached pose and the source participant's current geometry pose.
The timestamp updates last.

The pose getter at +0x02e4 and angle getter at +0x031c both call the same updater.
Two getters at the same tick cause at most one refresh.
800 paired original getter executions match Rust cache state, mode selection, and angle-service inputs.
These cases supply targeting and angle results and cover signed tick limits and the five-tick boundary.
They also verify getter outputs and preservation of unrelated participant bytes.

For a normal type-7 enemy with flag +5 equal to one, the targeting routine uses mode zero.
That path copies the target's current geometry pose without extrapolation.
Another 200 paired getter cases execute this original target-copy path, with only the final angle result supplied.
Predictive-intercept arithmetic is thus not a dependency of this specific type-7 cache path.
Exact OS angle conversion, outer command scheduling, and live state integration remain unfinished.

All 68,867 comparison and guard cases pass. All 50 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Type-7 firing integrated into the playable encounter

The live type-7 encounter now calls type7_fire_check instead of the preview's global two-second cadence and 480-unit range.
Each target stores its firing interval, short firing range, firing arc, and timer.
Catalog population copies these values from the resolved original parameter table, including recovered defaults for missing rows.
Short firing range converts from a signed integer word to fixed point.
Timers start at -1 and initialize only after the original 300-unit range check admits the target.
The accepted firing-command path writes the current tick before projectile creation.

The original planar-distance helper supplies range. The original game-side angle helper supplies quadrant and heading arithmetic.
The remaining OS conversion is explicitly a candidate: signed ratio division and an atan estimate of the biased slope-table scan.
It does not embed the historical QuickDraw table and is not verified against System 7 traps.
Coincident integer coordinates retain the original caller's cleared angle result.
The native nontermination guard also falls back to zero in this adapter.

The grounded-player geometry field +0x28 remains zero, as in the existing preview record adapter.
The complete command scheduler's inhibition flags, special weapon requests, and other enemy types are not integrated here.
Steering remains provisional, and the existing basic enemy projectile path remains in use.
This change does not establish complete original enemy AI.

The previous line-of-sight veto is removed from firing eligibility.
Scenery still intercepts enemy projectiles. The cover test verifies that enemies can fire without damaging the protected player.
Enemy respawn resets its firing timer, and round restart restores the captured initial state and original parameters.
The enemy-respawn timer reset remains an adapter lifecycle choice until its original call sequence is verified.

New integration tests cover range-entry timing, accepted-command timestamps, restart, short-range cadence, and resource firing arcs.
All 52 Rust tests, strict Clippy, and the WASM build pass.
Native/WASM encounter replay and reset checks pass, including live Central enemy shooting, damage, and audio.
Campaign, lives, flags, game, and audio suites also pass.
Hosting remains stopped; no visible browser or listening check was performed.

## Type-7 motion parameters and live translation

CODE 23 +0x05b8 selects angular rate from the enemy parameter table or participant settings.
A nonzero global at A5-0x3cf8 halves the selected signed long through integer division.
The global's gameplay meaning is not yet assigned.
600 complete original getter executions verify table selection, flag handling, signed division, and extreme values.

The type-7 path in CODE 30 +0x1370 starts at +0x17e6.
It obtains direction from CODE 18 +0x0d78, whose table pointer is A5-0x38c2.
CODE 6 +0x0530 initializes that table with scale 0x4000, one quarter in fixed point.
Each direction component multiplies by the signed throttle word, signed radial-velocity word, and elapsed ticks.
Each product and final coordinate addition wraps as a long.

CODE 30 +0x2578 computes angular change from steering, angular rate, and elapsed ticks.
Positive elapsed time updates heading before translation. Nonpositive elapsed time translates before updating heading.
800 original caller-block executions include the actual rate getter, heading updater, direction lookup, and complete type-7 translation routine.
Marker direction values separate arithmetic verification from the still-modeled SANE table initialization.
The tests cover signed commands, signed elapsed values, rate halving, overflow, and preservation of other geometry fields.
All 70,267 comparison and guard cases pass.

The playable encounter now stores each target's original radial velocity and fixed-point angular rate.
Catalog population obtains them from the resolved enemy parameter table.
Live type-7 movement uses move_type7 and the existing candidate quarter-scale direction table.
This replaces the preview's fixed 0.65-unit travel step and 0.025-radian turn step.
The live adapter uses the unhalved rate; the original global mode is not yet integrated.

Pursuit direction, alternate obstacle-choice headings, the 135-unit stopping distance, and movement collision acceptance remain provisional.
The adapter also converts poses through the existing floating-point item representation between ticks.
Thus recovered motion arithmetic does not establish complete original AI or exact runtime pose preservation.
An integration test verifies that resource radial velocities four and eight produce one and two units per tick on a cardinal heading.
It also verifies two-degree turning with zero translation.
The firing-range-entry test now uses the recovered default travel speed.

All 53 Rust tests, strict Clippy, and the WASM build pass.
Encounter, campaign, lives, flags, game, and audio suites pass with native/WASM replay agreement.
Hosting remains stopped.

## Nearby-participant priority and live yielding

CODE 20 +0x2290..+0x22e4 scans participant slots one through the current slot minus one.
It admits a candidate only when the bitwise AND of participant bytes +0x88 and +5 is nonzero.
Two nonzero bytes with no shared bits do not qualify.
The recovered planar-distance helper compares the candidate and current geometry positions.
Distance strictly below sixty units sets the nearby-predecessor flag. Exactly sixty units does not.
Later participant slots never trigger this flag for the current participant.

800 original scans include the complete participant-to-geometry lookup and distance helpers, without supplied service results.
They cover all twelve participant indices, noncanonical flags, shuffled geometry IDs, and the sixty-unit boundary.
578 cases set the flag. Participant and geometry records remain unchanged.
All 71,067 comparison and guard cases pass.

The final command block at +0x2724 clears throttle when the flag is set.
The live type-7 encounter now applies this gate before translation.
It maps the player to slot one and up to eleven targets to subsequent slots.
The adapter maps positive target health to active status and marks target slots as enemies.
Positions update after each live enemy moves, so subsequent scans use the updated position.
Original participant allocation, activation flags, and scheduling remain adapter assumptions.

A live integration test verifies that the earlier tank moves while the later tank waits.
The later tank resumes after separation grows, and a retired earlier tank does not force yielding.
This gate does not replace the provisional pursuit direction, stopping distance, or alternative obstacle headings.

All 54 Rust tests, strict Clippy, and the WASM build pass.
Native/WASM encounter replay, lives, and campaign suites pass. Hosting remains stopped.

## Recovered movement policies integrated into the encounter

Live type-7 steering now uses the recovered ordinary/combat policies, movement dispatcher, turn-only command, and shared aiming cache.
The alternating-side heading choices and fixed 135-unit stopping rule are removed.
Heading search follows the recovered alternating fifteen-degree sequence and retains its twenty-tick heading deadline.
Combat tracking uses the five-tick pose/angle cache. Turn-only selection uses the recovered strict distance boundaries.
An isolated encounter now settles just below 120 units after its final movement step.

The adapter still selects the player as the target.
It chooses ordinary or combat mode from the original attack-range parameter and resets the heading deadline when that mode changes.
This is an explicit preview intent adapter, not the complete original behavior-transition scheduler.
The cache initializes at timestamp -5 because the preview clock starts at zero instead of Macintosh uptime.
Round restart restores captured navigation state. Enemy respawn resets navigation while retaining the attack-range parameter.

Supported arenas use CollisionQueryWorld::ai_clearance through the preview's runtime-record adapter.
Each enemy builds its query from current entity positions before command selection.
This includes positions updated by earlier enemies in the same tick.
The adapter supplies original model dimensions and the candidate collision direction table.
Unavailable model slots retain a provisional footprint probe, which does not reproduce the full swept rectangle.
Final movement acceptance also retains the existing footprint check and turn-in-place fallback when translation is blocked.

The candidate OS angle conversion remains unchanged and unverified against System 7.
The native guard for an unwritten close-range heading holds position and uses the cached turn-only steering command.
Floating-point item poses, participant allocation, and activation flags remain adapter limitations.
These limits prevent a claim of complete original AI despite the recovered command routines now running live.

The yielding test now moves the earlier tank clear of both priority range and the clearance corridor.
This isolates yielding from obstacle-search delays introduced by the recovered policy.
A new test verifies combat stopping distance, cache/deadline updates, and navigation restoration on restart.
All 55 Rust tests, strict Clippy, and the WASM build pass.
Encounter, campaign, lives, flags, game, and audio suites pass, including native/WASM replay agreement.
Hosting remains stopped. No visible browser-play check was performed.

## Original intent dispatch gate

CODE 20 +23ac..24d4 checks the intent timer before selecting a behavior handler.
A timestamp of -1 permits the check immediately. Otherwise, timestamp plus 15 must be strictly less than the current tick.
Within attack range, eligible kinds enter attack mode unless movement mode already equals 1.
Kinds 21 through 25, 35, and 45 bypass that attack branch.
Game mode zero supplies target 1. Other modes require a nonzero target different from the current participant.
Outside that branch, the signed behavior value selects the original handler through its jump table.

The recovered ai_intent_action function returns the selected handler and optional attack target.
The harness executes 3,232 original dispatch blocks and compares the selected handler, arguments, and unchanged state.
Handler execution is intercepted for these tests; they do not establish each handler's behavior.
The live encounter still uses its provisional range-based intent adapter.
Replacing that adapter requires the tracking handler and its additional state transitions.
All 55 Rust tests and strict Clippy pass.

## Type-7 tracking tail

CODE 20 +1284..14ae preserves movement mode 1 when tracking resumes outside attack range.
Only movement mode 3 changes to 0. That transition also copies the target into the aiming target.
For nonzero game modes, inactive participants instead receive behavior 3 and target zero.
That early exit preserves the tracked pose and heading deadline.
Otherwise, the handler copies the cached target pose and sets the heading deadline to -1.
It does not update the intent-check timestamp.

The Rust ai_type7_tracking_tail function models these writes and the cached-pose request.
The harness compares 4,096 original tail executions, including all movement bytes and independent activity/game-mode combinations.
The cached-pose service is supplied. The prelude at +1240..1284 is excluded explicitly.
The harness also verifies preservation of other record bytes.

The prelude tests subrecord word +40 against 5 and can call target selection before the tail.
CODE 20 +293a increments this word after an accepted firing request; +2a6c clears it during reset.
The complete targeting semantics still require recovery of +754 and its callers.
The new tail remains separate from the live preview adapter until these dependencies are resolved.

All 78,395 differential cases pass. All 55 Rust tests and strict Clippy pass.

## Participant target selector

CODE 20 +754..8f6 scans participants 1 through 12 in ascending order.
It excludes the source participant and the supplied excluded participant. Health must be positive.
Game mode zero requires a nonzero bitwise intersection of active and owner flags.
Other game modes require an active candidate on a different team from the source.

The selector uses recovered planar distance. Nonzero selection mode doubles this distance with wrapping arithmetic.
Candidates beyond 300 units are rejected before the angle service runs.
The score adds twice the absolute signed-word angle, converted to fixed point, to distance.
Both the absolute value and doubled angle preserve original word overflow.
Candidates also require kind 8, distance at most 200, or absolute angle at most 45.
Only a strictly smaller signed score replaces an existing selection. Ties retain the earlier participant.
The returned participant is zero when no candidate qualifies.

Rust ai_select_target accepts the angle service separately. Invalid source indices return None as a native guard.
The harness executes the complete original routine for 1,000 synthetic participant tables.
It compares selected participants and all 1,374 angle-service calls, including both pose arguments and call order.
652 cases select a participant. Record and geometry bytes remain unchanged.
The original distance routine executes directly; final angle results are supplied.
This does not verify the System 7 angle service or complete gameplay targeting.

The tracking prelude passes the local participant as source and the previous target as excluded.
Any selected replacement is passed to the attack transition for that local participant.
Thus this caller must not be generalized into an unconditional enemy-retarget operation.
Integration still requires the caller context and local participant state.
All 79,395 differential cases, 55 Rust tests, strict Clippy, and the WASM build pass.

## Complete type-7 tracking handler

The Rust ai_type7_tracking function now composes the complete CODE 20 +1240 kind-7 path.
A signed counter of at least 5 requests target selection with mode zero.
The source is the local participant. The excluded target is the current handler participant's target, truncated to a signed byte.
A nonzero selection applies the original attack transition to the local participant.
Behaviors 4 and 9 retain their attack state, as in the original transition.
The tracking tail then runs for the handler participant, which can be a different participant.
Selection occurs before the inactive-participant exit. The cached-pose service observes the updated intent state.
The counter remains unchanged. Invalid participant indices return None without mutation as a native guard.

The harness executes 1,200 complete original handlers with selection and cached-pose services supplied.
The attack transition executes from original bytes. All twelve participant states are compared.
The harness also verifies service arguments, state visible to the pose service, and preservation of unrelated record bytes.
These cases include 492 selection calls, 1,010 pose calls, and 659 cases where local and handler participants coincide.
Boundary counters, signed target truncation, blocked attack behaviors, and inactive participants are included.

The original local-command routine calls this handler at CODE 20 +2884.
The general participant dispatcher also calls it at +24b8 and +24c8.
These callers explain why local and handler identities must remain separate in the port.
The live preview still needs original intent initialization, timestamps, and caller state before this handler can replace its adapter.
All 80,595 differential cases, 55 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Type-7 intent initialization and object guarding

CODE 20 +b6c initializes behavior 3 before the general dispatcher checks its intent gate.
For kind 7 and initializer mode zero, it calls opponent selection +a22 and object selection +3b2.
It then obtains distance to participant 1.
A nonzero object target takes priority only when that distance strictly exceeds twice the attack range.
That branch sets behavior 1 and stores the object index as target.
Otherwise, a nonzero opponent sets behavior 0, target, aiming target, tracked pose, and heading deadline -1.
The cached-pose service sees movement mode zero on this branch.
If neither target qualifies, the initializer sets behavior 2 and preserves the other target fields.
All three branches finish with intent timestamp -1 and movement mode 3.
The doubled attack range preserves signed wrapping arithmetic.

Rust ai_initialize_type7 implements these transitions. Its searches, distance, and cached-pose results are supplied explicitly.
800 complete original initializer executions verify state writes, service order, and state visible to the cache service.
The test includes strict range boundaries, overflow, absent targets, and signed opponent truncation.
Nonzero initializer modes and other enemy kinds remain outside this function.

CODE 20 +153a handles behavior 1. Rust ai_track_object now reproduces the complete handler.
Below 40 units, participants other than 1 enter movement mode 2 and aim at participant 1.
This branch preserves the previous tracked pose and heading deadline.
Otherwise, the handler enters movement mode zero, copies the object pose, and resets the heading deadline.
600 complete original executions verify these writes with the original distance routine and no service fixtures.

The object selector at +3b2 scans kind-12 objects and conditionally kind-5 objects.
It filters by an assignment count and relative distance to the enemy and participant 1.
Each admitted candidate invokes Random and can replace the current selection.
This is direct evidence of randomness in object selection, not evidence about movement coasting.
The selector and assignment-count helper still need executable differential coverage before live integration.

All 81,995 differential cases, 55 Rust tests, strict Clippy, and the WASM build pass.
Live intent selection remains provisional. Hosting remains stopped.

## Original object assignment and selection

Rust ai_object_assignment_count reproduces CODE 20 +346.
It counts all twelve participants whose active and owner bytes have a nonzero bitwise intersection.
The participant must have behavior 1 and a target equal to the queried geometry index.
Health, distance, and participant identity do not affect this count.

Rust ai_select_object reproduces CODE 20 +3b2. Geometry indices remain one-based and preserve scan order.
Kind 12 qualifies. Kind 5 also qualifies when signed globals A5-136e and A5-1372 reach 25 and 6 respectively.
The assignment limit is signed truncating division of A5-1372 by six, plus one.
Objects at or above that limit are rejected.
The object must be strictly closer to the source participant than to participant 1, using the recovered planar distance.

Each admitted object increments a signed-word candidate count and calls Random once.
The original takes the wrapping signed-word absolute value of the sample.
It replaces the selected index when the signed remainder modulo candidate count is zero.
The -32768 sample therefore remains negative, matching the original ABS and DIVS instructions.
No admitted candidate means result zero and no Random calls.
The native function rejects slices exceeding the signed geometry-index range before invoking Random.

The harness executes 800 complete original selectors with only Random trap results supplied.
Original assignment counting and planar distance execute directly inside each selector.
It also executes 13,587 separate original assignment scans and compares their counts with Rust.
The selectors choose an object in 526 cases and consume 2,267 Random samples.
Selection results, sample consumption, and preservation of participant/geometry records all match.
The generator of original Random samples remains outside this verification.

The two globals retain neutral field names in the API until their full lifecycle is traced.
Live integration still needs an accurate runtime geometry list and participant assignment state.
All 82,795 differential cases, 55 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Recovered intent integrated into live encounters

The live type-7 adapter now initializes behavior 3 through the recovered mode-zero initializer.
It selects eligible objects through ai_select_object and counts assignments from current participant intents.
The placement list provides stable object indices, matching the existing spawn adapter.
Collected flags retain their placement slot and become kind 14.
Later enemies see assignments made by earlier enemies in the same tick.

The adapter supplies initial effective difficulty and the count of non-retired enemies to object selection.
It uses the existing deterministic xorshift stream for Random words. This is not the original Random generator.
It assumes single-player mode and participant 1 as the opponent, matching the original mode-zero opponent selector.
Local auto-control counters remain zero for these enemy records; local-player auto-control is not implemented.

The original intent gate selects attack, tracking, or object tracking.
A selected object can cause approach movement or a nearby turn-only guard state.
Entering attack range changes behavior and target through ai_begin_attack.
Leaving attack range preserves combat movement mode through the recovered tracking tail.
The previous unconditional range-based mode switch is removed.
Heading deadlines now follow the selected behavior handler and recovered movement policy.
Respawn resets intent while preserving attack range and effective difficulty. Restart restores captured navigation state.

Runtime geometry still differs from the original full list. Selection currently scans the stable authored placement list.
It does not include procedural objects or the complete original runtime object lifecycle.
The initial effective difficulty does not yet reproduce every later global update.
Aim caches use the existing player-pose adapter, candidate angle service, and normalized preview clock.
Target invalidation, collision-triggered intent resets, other game modes, and local auto-control remain unfinished.
These limits prevent a claim of complete original AI.

A new live test verifies object approach, nearby guarding, attack entry, and combat-mode persistence after range exit.
All 56 Rust tests, strict Clippy, and the WASM build pass.
Encounter, campaign, lives, flags, game, and audio suites pass, including native/WASM replay agreement.
Hosting remains stopped; no visible browser-play check was performed.

## Command-generation distance gate

CODE 31 +1f3a..1fa0 decides whether to call the AI command dispatcher.
Geometry kind 10 suppresses commands. Bit zero of the supplied inhibit byte also suppresses commands.
Otherwise, more than five remaining enemies enables a distance check against A5-3d42.
Only distance strictly above that signed limit suppresses commands.
The distance wrapper CODE 6 +17c6 resolves geometry indices and calls the recovered planar distance routine.
CODE 25 +21e0 and +2cd4 initialize the limit to 500 fixed-point units.
CODE 33 +1f0..21a contains controls that adjust this limit by 100 units.

Rust ai_commands_suppressed matches 1,024 original gate executions.
The harness executes the original geometry-distance wrapper without supplied service results.
It verifies whether that wrapper runs, including early exits, all inhibit bytes, and signed count/limit boundaries.
The branch destinations establish command suppression versus dispatcher admission.
The skipped-command block clears steering, translation, and the local firing-request word.
It does not establish suppression of every later lifecycle or geometry update.

The encounter now applies this gate before intent, movement, and firing requests.
It supplies zero for the external inhibit input and uses the original default distance limit.
Remaining enemies are represented by non-retired preview target records.
Existing projectiles continue to update. Target lifecycle remains handled by the existing adapter.
A live test verifies unchanged navigation at six distant enemies and resumed commands after one retires.

The flag-pickup review found no global guard reset in CODE 28 +59c..620.
Its conditional CODE 20 +2a30 call targets only the local participant.
No automatic guard reset was added on flag collection. Other invalidation paths remain under investigation.

All 83,819 differential cases, 57 Rust tests, strict Clippy, and the WASM build pass.
Encounter, campaign, lives, and flags suites pass, including native/WASM replay agreement.
Hosting remains stopped.

## Stored-pose pursuit correction

The live ordinary-movement adapter incorrectly supplied the current player position for behavior 0.
CODE 20 +1742..1754 computes distance from the stored target pose at AI subrecord +c.
The heading service at +187e..1888 uses that same stored pose.
The adapter now supplies tracking.tracked_pose for ordinary movement in both participant pursuit and object approach.
This preserves the five-tick cache lag during participant pursuit.

The differential suite now executes 1,200 complete mode-zero movement routines across behaviors 0, 1, and 3.
It checks the target and source pose arguments passed to the supplied angle service.
It also compares state writes, commands, clearance calls, and preservation of unrelated fields.
Angle conversion and clearance answers remain supplied services in these cases.
The expanded coverage confirms that the same recovered ordinary policy applies to these three behaviors.

A live regression test moves the player between cache refreshes.
The enemy retains its previous desired heading until tick 6, when the cached target position changes.
The movement-path review found no object-kind invalidation in these inspected ordinary branches.
This does not establish that all other original invalidation paths are absent.

All 84,619 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Encounter and flag suites pass, including native/WASM replay agreement and 112 flag-mission setups.
Hosting remains stopped.

## Mode-specific initial opponent acquisition

Rust ai_acquire_opponent reproduces the complete CODE 20 +a22 routine.
Game mode zero returns participant 1 without reading participant records or calling angle services.
Modes 4 and 12 call the recovered scored selector with excluded participant zero and selection mode zero.
All other nonzero mode bytes scan participants 1 through 12 for the nearest eligible opponent.
Eligibility requires a nonzero active byte, positive health, and a team different from the source team.
The owner byte is not tested on this path. There is no range limit or angular filter.
Distance uses the recovered planar metric and signed comparisons.
Only a strictly smaller distance replaces a selection, so ties retain the earlier participant.
No eligible opponent returns zero.
The native guard rejects invalid source indices on paths that access source records.

The harness executes 1,024 complete original calls across all 256 game-mode bytes.
Geometry lookup, nearest-opponent scans, distance calculations, and delegated scored selection execute from original bytes.
Only the final angle service in modes 4 and 12 receives supplied results.
The harness compares selected participants, angle-service arguments/order, and unchanged participant/geometry records.
This verifies mode dispatch and selection; it does not implement networking or complete multiplayer gameplay.
The live single-player adapter continues using the verified mode-zero result of participant 1.

All 85,643 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Enemy-kind firing predicates

Rust enemy_fire_check extends CODE 20 +1e9a recovery to enemy-owned records across all 47 parameter-table kinds.
This function requires a target and accepts the final angle-service result explicitly.
Kind 13 exits without changing its timer. Kind 20 permits distance up to 600 units; other kinds permit 300.
These range exits occur before timer and geometry writes.

Kinds 21, 25, 35, and 45 set the timer to current tick plus ten before the common eligibility check.
The kind mask is stored in the six bytes immediately before CODE 20 +2228.
Kind 22 resets its timer when the wrapping absolute difference between source and target geometry fields +28 is below 25 units.
Kind 26 resets its timer and sets geometry byte +5b when that byte was zero.
The common path initializes timer -1, applies interval/short-range timing, checks the target field, and tests the firing arc.
An eligible kind-26 request sets behavior 4, movement mode zero, and geometry byte +5b to zero.
Eligibility is a firing request, not proof that a weapon was created.

The harness executes 1,410 complete original predicates across all 47 kinds.
It supplies only final direct-angle results and compares eligibility, timer, behavior, movement mode, and geometry byte +5b.
Cases include both 300- and 600-unit boundaries, signed arithmetic extremes, and kind-specific state values.
The cache-angle branch, absent-target path, and local-player parameter path are not covered by these new cases.
The original Random generator is not involved in this predicate.

This function is not yet connected to the stationary enemy types.
Their weapon creation, aiming, and movement rules still need recovery before activation.
All 87,053 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Enemy weapon-request dispatch

Rust enemy_weapon_requests reproduces CODE 31 +1fd0..212a.
A zero firing command or weapon-table word -1 emits no requests.
Other weapon words are truncated to signed bytes before dispatch.
In game mode zero, weapon 5 emits requests 5, 3, and 4 in that order.
Weapon 12 emits weapon 0 when the command word equals 1; other nonzero commands retain weapon 12.
Other weapon bytes emit one request without remapping.

Nonzero game modes require the network-enabled byte at A5-4f4f.
They emit one network-routed request and bypass the local weapon-5 and weapon-12 expansion rules.
Each request converts geometry field +28 to an integer height through signed division by 65536.
This truncates toward zero, including negative fractional values.
The local constructor also receives source kind, tick, geometry pointer, and participant identity.
The network constructor receives participant identity and geometry pointer through its separate argument layout.

The harness executes 1,200 original dispatch blocks and captures calls to both constructors.
It compares request order, signed weapon bytes, integer heights, and routing.
It asserts the remaining constructor arguments and executes the original signed division helper.
The constructors themselves are intercepted, so projectile allocation and effects are not verified by these tests.

The Rust helper is not yet connected to the live projectile adapter.
Weapon-specific constructors remain the next dependency for activating additional enemy weapons.
All 88,253 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Weapon constructor heading preparation

Rust weapon_initial_heading reproduces CODE 30 +2ee..3ee, before direction lookup and projectile construction.
Weapon 4 draws one Random word and subtracts seven plus the wrapping absolute sample modulo eight degrees.
A negative result receives one addition of 360 degrees.
Weapon 3 uses the same spread amount but adds it; a result at least 360 receives one subtraction of 360.
These branches deliberately use a single correction, preserving behavior for out-of-range headings.
Both variants then continue as weapon 0.

Weapon 8 requests target selection with mode zero, excluded participant zero, and the firing participant as source.
If a target is found, the constructor requests its angle relative to the projectile pose.
It adds the signed-word angle to the heading and calls the original angle-wrap helper.
Without a target, it preserves the heading. Either outcome continues as weapon 0.
Other weapon bytes leave heading and weapon identity unchanged at this stage.

The harness executes 1,000 original preparation blocks across all 18 weapon-table slots.
It supplies Random samples, target-selection outcomes, and final angle results.
Original fixed-point multiplication and heading wrapping execute directly.
It compares heading, resulting weapon byte, Random-call count, and target-selection-call count.
It also verifies target-selection arguments and whether the angle service runs.
Cases include missing targets, signed sample extremes, and headings outside the normal range.

This verifies constructor preparation only. Projectile allocation, lifetime, placement, and effects remain separate dependencies.
The helper is not yet connected to live shooting.
All 89,253 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Weapon-specific record construction

Rust initialize_weapon_record reproduces CODE 30 +408..7e8 after heading preparation and direction lookup.
The function mutates a 128-byte geometry record and preserves fields untouched by the original block.
It accepts the lifetime setting returned by CODE 23 +4 and the firing tick.
Lifetime scaling preserves signed-word truncation and overflow before adding the tick.

| Effective weapon | Geometry kind | Lifetime offset | Forward offset factor |
| --- | --- | --- | --- |
| 1 | 9 | (setting / 4) * 3 | 6 |
| 17 | 46 | (setting / 4) * 3 | 6 |
| 2 | 28 | setting * 50 | 6 |
| 6 | 29 | setting * 4 | 3 |
| 7 | 32 | setting * 24 | 0 |
| 10 | 34 | setting * 4 | 6 |
| 15, 16 | 43, 44 | setting * 4 | 3 |
| 11 | 33 | setting * 8 | 6 |
| Other values | 9 | setting | 6 |

The offset factor multiplies the supplied fixed-point direction. An owner byte of zero triples the factor.
Weapon 17 sets geometry field +24 to -1 fixed-point unit.
Weapons 6, 15, and 16 set field +24 to 30 units and copy the source kind to byte +15.
Weapon 6 adds five plus the wrapping absolute Random word modulo five to its height.
For sample -32768, the original signed remainder produces a rise of two units.
Weapons 15 and 16 add five units without Random.
Weapon 7 clears word +30 and copies participant word +5c into geometry word +32.
Weapon 10 sets field +24 to 15 units, adds 20 to height, and clamps the signed result to at least one unit.
Weapon 11 copies heading to field +2c and clears long +1e.

The harness executes 1,080 original construction blocks across all 18 weapon indices.
It runs the original lifetime getter, integer multiplication, and fixed-point arithmetic. Only Random samples are supplied.
All 128 output bytes and Random-call counts match Rust, including signed overflow and untouched-field preservation.
Inputs 3, 4, and 8 exercise the block's fallback path; normal construction first remaps them through heading preparation.

Allocation, initial record setup, ownership finalization, model loading, sound, and spatial-grid insertion remain separate dependencies.
The function is not yet connected to live projectile creation.
All 90,333 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Geometry allocation and common weapon finalization

Rust original_geometry_slot reproduces the complete CODE 25 +e0e counter allocator.
At exactly 302 records, it returns zero and preserves the counter.
Otherwise, it increments the signed word with wrapping arithmetic and returns the new value.
The original check is equality, not a greater-than-or-equal capacity check.
612 original calls verify all ordinary counter values and additional signed boundaries.

The constructor tests a negative allocation result at CODE 30 +26e, not zero.
Thus the inspected allocator and constructor disagree at capacity. No original memory write through index zero is reproduced in Rust.
The native allocate_geometry_slot API accepts only counters from zero through 301.
Full or invalid counters return None without mutation. Differential cases also verify this explicit native guard.
The runtime has not yet adopted this pool; current projectile storage remains the preview adapter.

Removal uses compaction rather than a free-slot scan. CODE 25 +f4a..1046 copies the last record into the removed index.
It then updates participant/grid references, marks the former last record inactive, and decrements the counter.
These observations identify the required lifecycle work; they are not yet a complete compaction differential test.

Rust finalize_weapon_record reproduces CODE 30 +7e8..836, excluding the model-load service.
Weapon 5 stores weapon index zero; other signed weapon bytes are sign-extended to words.
The block sets the timestamp, owner, active byte, command words, and cleared geometry byte +5b.
A nonzero owner increments the participant's active-shot word with wrapping arithmetic.
768 original executions verify all 128 record bytes and the resulting owner counter.
The harness captures the model-load call and verifies its kind argument.

The later grid insertion already has a separate recovered helper, insert_projectile_grid_entry.
Admission, allocation, initialization, model loading, sound, grid insertion, and removal still need end-to-end composition.
All 91,713 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Weapon admission, ammunition, and allocation failure

Rust admit_weapon reproduces CODE 30 +140..292 before successful-allocation counters and geometry writes.
A nonzero owner on team byte 3 exits before selecting a cost or allocating.
Owner zero and enemy-owned records use the cost table at parameter offset +7a2.
Other owners use the table at +7c6.
A nonzero owner's signed active-shot count must be strictly below the global shot limit.

For the local non-enemy owner, ammo below cost emits request 0x30006 at priority 1 and exits.
This signed comparison follows SUB/SLT semantics, including overflow.
The free-ammo flag bypasses charging but does not bypass this eligibility check.
With charging enabled, crossing from above five rounds to five or fewer emits requests 0x1fc1c and 0x3000a.
The remaining ammo uses signed-word wrapping subtraction.
Other owners are not charged on this path.

The allocator runs only after these checks.
A negative result returns the cost to the local owner, even when an earlier branch skipped charging.
This raw behavior is preserved separately from the native allocation guard.
Nonnegative results, including zero, pass the original constructor's admission check.
The guarded allocation API still prevents zero-index native construction.

The harness executes 1,600 original admission blocks.
Allocator results are supplied, while sound calls are captured with exact request values and priorities.
It compares ammo, allocation-call count, accepted slot, and sound order.
Coverage includes team/owner distinctions, shot limits, free-ammo flags, signed extremes, and failed allocations.
CODE 25 +2902..2916 sets the shot limit to 20 for a nonzero A5-12c4 flag, otherwise five.
The configuration flag still needs lifecycle tracing before a live default is selected.

The helper remains separate from live shooting until constructor composition and pool lifecycle are complete.
All 93,313 differential cases, 58 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.

## Geometry compaction tail

Rust compact_geometry_tail reproduces CODE 25 +f4a..104a after removal-side bookkeeping and grid unlinking.
It copies all 128 bytes of the last record into the removed slot.
For participant geometry kinds with a nonzero owner, it updates that participant's geometry index.
The kind mask includes 6, 7, 8, 10, 13, 20 through 27, 31, 35, and 45.
It locates the moved record's grid cell through its position.
Every reference to the former last index in that cell becomes the removed index.
The former last record becomes kind 14 with owner zero, and the geometry count decreases by one.
Removing the last record follows the same sequence.

The harness executes 768 original compaction tails across all kind bytes.
The original clamped cell lookup executes without supplied service results.
Tests compare all geometry bytes, all participant geometry indices, and grid entries.
They include duplicate grid references, empty cells, full cells, and removal of the last slot.
The native API validates indices, moved participant ownership, and cell capacity before mutation.
A Rust regression test verifies atomic rejection of invalid inputs.

This is the compaction tail, not the complete removal routine.
Earlier grid unlinking, active-shot bookkeeping, and participant deactivation still require composition and verification.
The live preview has not adopted the compact geometry pool.
All 94,081 differential cases, 59 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Geometry removal bookkeeping and compaction

Rust remove_geometry composes CODE 25 +e62..1054, including CODE 14 +2e20.
It unlinks the first matching grid entry by replacing it with the last active entry.
Projectile kinds 9, 28, 29, 32, 33, 34, 43, 44, and 46 decrement their nonzero owner's active-shot count.
The signed count wraps, including from -32768 to 32767.
Participant geometry kinds clear their owner's active byte through the original +e2e helper.
That helper calls CODE 14 +2e20 with zero and the participant number before compaction.
CODE 14 writes this zero into the twelve-byte status array at A5 -4c40.
It has no further calls or side effects.
The native helper clears this status byte before compaction.
The compaction helper then repairs the moved record's references and decreases the geometry count.

The earlier raw-record delete_object_record API already implemented the status write.
It now adapts participant fields to remove_geometry instead of maintaining a second removal implementation.
The projectile-only wrapper retains its kind restriction.
Both APIs reject invalid referenced owners and oversized cells before mutation.
The raw-record adapter preserves unrelated participant bytes.

The harness executes 1,024 complete original removal calls, including cell lookup, participant clearing, and the CODE 14 status write.
These calls use no supplied service results.
Cases cover all 256 kind bytes, duplicate and absent grid references, full cells, last-slot removal, and signed shot-count boundaries.
Comparisons cover every geometry byte, participant indices, shot counts, active bytes, status bytes, grid counts, and active grid entries.
The harness also verifies that other participant bytes remain unchanged.
The earlier 341 raw-record deletion cases exercise the same implementation through its compatibility adapters.
A regression test verifies that both APIs preserve all supplied state on rejected inputs.

All 95,105 differential cases, 60 Rust tests, strict Clippy, and the WASM build pass.
The live preview still uses its existing object adapters rather than this compact pool.
Hosting remains stopped.


## Live player ammo admission and warning

The preview player now uses admit_weapon after the existing primary-fire eligibility check.
This replaces the separate ammo subtraction with the recovered CODE 30 admission rule.
A crossing from above five ammo to five or fewer emits the recovered requests before the shot sound.
The existing sound adapter resolves request 0x3000a to original sample 10010.
The browser preloads that sample.
The negative low-word request 0x1fc1c has no sample in this adapter.
Original channel control and priority handling remain unfinished.

The preview supplies a successful admission token instead of a compact-pool allocation.
It still has no configured original concurrent-shot limit.
Thus this change integrates ammo deduction and warning behavior, not full constructor allocation.
Enemy admission and the compact geometry pool remain separate integration work.

A Rust regression covers exact threshold crossing, multi-round costs, zero cost, already-low ammo, and insufficient ammo.
The WASM audio test verifies one warning during 40 successful shots, followed by empty-ammo attempts.
It verifies warning-before-shot ordering, browser preload membership, and no duplicate events on redraw or reset.
All 105 original PCM sample payloads remain unchanged.
All 61 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped; this change has no new browser listening check.


## Fresh-session concurrent-shot limit

CODE 24 +140c..1448 initializes fresh session settings.
Its +142a instruction writes word 1 at settings offset 8, giving bytes zero at offset 8 and one at offset 9.
The single-player path selects game mode zero at +1416 and bypasses the settings dialog at +15c8.
CODE 25 +2602 and +2608 copy those bytes to the free-ammo and shot-limit flags.
Thus fresh single-player settings enable ammo charging and select the nonzero shot-limit branch.
CODE 25 +2902..2916 sets the limit to twenty for a nonzero flag, or five for zero.

The harness executes all 256 flag values through the original selector.
It also executes three original fresh-settings blocks and their flag transfers, including the single-player branch.
These blocks use no supplied services.
The differential report includes the CODE 24 hash.

Live player admission now counts stored shots and uses the recovered twenty-shot limit.
Admission occurs before the preview's projectile update, so a shot awaiting removal still occupies a slot.
A regression fills all twenty slots, verifies rejection without ammo or warning effects, and verifies firing after expiry removes those slots.
This verifies preview scheduling only; the original frame scheduler still requires integration.
Enemy per-owner accounting, loaded-session settings, and compact-pool capacity remain unfinished.

All 95,361 differential cases, 62 Rust tests, strict Clippy, and the WASM build pass.
WASM audio and encounter parity checks pass.
Hosting remains stopped.


## Live enemy shot ownership and admission

Enemy projectiles now retain their stable preview target slot as an owner.
Each enemy counts only its own stored shots against the recovered fresh-session limit of twenty.
The check follows acceptance of the firing command and its timer update, matching the recovered caller/constructor order.
Rejected construction produces no projectile, launch count, or shot sound.
Expiry and impact removal release slots for the removed projectile's owner.
Reset clears projectiles and thus all active-shot occupancy.

A two-enemy regression fills the first owner's twenty slots while the second remains free.
It verifies independent admission, rejected-launch timing and sound behavior, expiry, impact removal, and resumed firing.
The impact case disables the existing spawn grace period to verify actual damage as well as removal.
All 63 Rust tests pass, along with strict Clippy and the WASM build.
The 112-arena replay check, encounter native/WASM parity, and audio checks pass.

Ownership still uses preview target slots rather than original participant and compact geometry records.
The total geometry-pool capacity, loaded-session settings, and original update ordering remain unfinished.
This change does not implement special weapon construction or enemy ammo policies.
Hosting remains stopped.


## Resource weapon selection in live enemy firing

EnemyFireState now retains the Beastie weapon word, which the preview previously discarded.
Accepted type-7 firing commands pass through recovered enemy_weapon_requests with command 1, game mode zero, and grounded height zero.
Weapon -1 emits no constructor request.
Weapon 5 emits requests 5, 3, and 4 in that order.
Weapon 12 with command 1 emits request zero.
Each request independently checks its owner's active-shot limit before heading preparation.

The live path uses recovered weapon_initial_heading for spread requests 3 and 4.
The center and spread projectiles retain the firing owner.
Accepted spread requests consume the existing deterministic preview random stream.
This stream is not the original Macintosh Random implementation.
Blocked requests do not consume randomness, increment launch counters, or emit shot sounds.
The existing basic projectile path still handles special weapon kinds without their original effects.
Target selection for weapon 8, specialized motion/damage, height, and weapon-specific sound routing remain unfinished.

A regression verifies resource-field transfer, unarmed behavior, ordered spread headings, independent burst admission, random consumption, and reset restoration.
With nineteen occupied slots, a three-request burst emits only its center projectile.
All 64 Rust tests, strict Clippy, the WASM build, 112-arena replay checks, encounter parity, and audio checks pass.
The request expansion and heading arithmetic retain their earlier original-code differential coverage.
Hosting remains stopped.


## Original constructor weapon sounds

CODE 15 +bcc..c28 initializes eighteen sound-table words at A5 -1138.
The values are 1, 24, 37, 255, 255, 38, 255, 255, 255, 28, 27, 21, 255, 45, 255, 20, 35, and 47.
CODE 30 +836..864 indexes this table with the original requested weapon byte, before heading conversion changed the effective weapon.
Value 255 suppresses the request.
Other values produce priority one and request 0x00010000 combined with the table word.
The existing sample resolver maps these positive low words to resource IDs beginning at 10000.

The harness executes the original table initializer and all eighteen constructor sound blocks.
It captures the sound-service arguments and verifies priority one.
All 95,379 differential cases match Rust.
Invalid native weapon indices return no request rather than reading outside the table.

Player and enemy launch events now use this recovered lookup.
The basic enemy constructor requests sample 10001, replacing the earlier resource-name-based choice of 10002.
Scattershot requests sample 10038 for its center shot; requests 3 and 4 are silent.
A live regression verifies one Scattershot sound for three projectiles and no basic-shot sounds for its spread requests.
The browser preloads every sample referenced by this table.
Original channel interruption, priority scheduling, and other combat sound call sites remain unfinished.
Special weapon sounds do not imply completed special projectile behavior.

All 64 Rust tests, strict Clippy, the WASM build, audio checks, encounter parity, and 112-arena replay checks pass.
Hosting remains stopped; no new listening check was performed.


## Type-29 bouncing projectile motion

CODE 30 +1dc6 selects geometry kind 29 before the general movement path.
The direction lookup at +1ddc supplies two fixed-point components.
Rust move_bouncing_projectile translates the following +1de0..1e60 motion body.
Each horizontal component changes by the wrapped product of direction and elapsed ticks, divided by eight toward zero.
Height changes by the wrapped product of vertical velocity and elapsed ticks, divided by two toward zero.
Vertical velocity then decreases by 65536 times elapsed ticks, with signed 32-bit wrapping.
If the new height is nonpositive, the routine sets height to zero and halves the wrapped absolute updated velocity.
This includes the original signed-minimum edge behavior.
Other record bytes remain unchanged.

The outer dispatcher skips elapsed zero before this body.
The common tail at +25e8 calls the spatial movement routine after motion.
The native helper covers neither that tail nor hit effects, expiry, or rendering.
The live preview still uses basic projectiles for this weapon.

The harness executes 1,200 original motion bodies with supplied directions and original CODE 36 integer arithmetic.
Inputs include positive and negative elapsed values, signed extrema, ground contact, airborne motion, and overflowing products.
It compares all 128 record bytes after each execution.
All 96,579 differential cases, 64 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Complete type-29 movement route

Rust step_bouncing_projectile composes direction lookup, bouncing motion, and spatial-grid movement.
It corresponds to the type-29 route through CODE 30 +1d54, including its common +25e8 tail.
The original CODE 18 +cd0 lookup reads the integer heading from the fixed-point value.
Its output ordering matches the existing movement_direction helper.
Elapsed zero returns without reading the direction table or changing the record or grid.

The harness executes 720 complete original movement calls through the direction lookup and CODE 30 +f60 grid update.
All 360 integer headings occur with fractional components, varied positions, and positive, negative, or zero elapsed values.
The supplied direction table contains synthetic values to expose indexing and output-order mistakes.
This test does not establish the original table-generation values.
Comparisons cover every record byte and every active entry in all 256 grid cells.
Grid inputs include missing and duplicate IDs, with destination capacity available.

The native wrapper rejects invalid headings and full destination cells before committing motion.
A regression verifies unchanged record and grid state on rejection, plus the original zero-elapsed bypass.
The original grid move does not protect a full destination; rejection is a native memory-safety guard.
All 97,299 differential cases, 65 Rust tests, strict Clippy, and the WASM build pass.
Live projectile height/rendering, specialized hit effects, and compact-pool integration remain unfinished.
Hosting remains stopped.


## Live weapon-6 bouncing projectiles

Enemy weapon 6 now uses initialize_weapon_record to establish its type-29 motion fields.
This supplies the recovered three-unit launch offset, randomized initial height, vertical velocity, and four-times lifetime.
The record adapter supplies grounded source height and a nonzero owner, with the existing preview random stream.
The shot retains its separate stable enemy owner for admission accounting.
The live update uses move_bouncing_projectile and synchronizes planar position and height from its record.
The 75-tick lifetime setting gives this projectile a 300-tick deadline.

First-person and chase projectile markers now use each shot's height.
Basic shots retain their existing four-unit visual height.
Overhead markers remain planar.
The enemy collision adapter samples the actual displacement and interpolates height across the tick.
Scenery checks use that sampled height against existing preview bounds.
Player checks apply the recovered sixty-unit candidate height threshold to bouncing shots, followed by the existing planar preview hit box.
This does not reproduce the original type-29 collision dispatcher, frame scheduling, blast effects, or damage policy.
The compact geometry pool is not yet integrated.

A live regression verifies launch height, first-step displacement, ascent, ground contact, rebound, and expiry without further launches.
A renderer regression verifies that an elevated shot appears above a ground-level shot in first person.
All 67 Rust tests, strict Clippy, the WASM build, 112-arena replay checks, encounter parity, and audio checks pass.
The motion arithmetic retains its earlier original-code differential coverage.
Hosting remains stopped; browser interaction has no new visual check.


## Projectile expiry and ownerless retirement

Rust projectile_lifecycle translates one record's dispatch through CODE 30 +2bbe.
Projectile kinds 9, 28, 29, 32, 33, 34, 43, 44, and 46 share the signed deadline comparison.
A future deadline requests movement.
An expired record normally requests deletion, including weapon 6.
Weapons 1 and 17 instead enter the retirement path at +2c52.
This copies the former kind to byte +15, sets kind 10, decrements the nonzero owner's shot count, and clears ownership.
It also sets the deadline to current time plus sixty, with signed wrapping.

An ownerless kind-10 record waits while its wrapped deadline difference is at least 57.
Below that threshold, it becomes kind 9 and requests deletion.
Weapons 1 and 17 first request the area-effect routine at +2aa6.
Under one-tick scheduling this occurs four ticks after retirement.
Other kind-10 records with nonzero ownership remain untouched by this scan.
The native helper returns movement, deletion, and area-effect actions rather than executing those services.
It performs retirement writes and validates a referenced owner before mutation.

The harness executes 3,098 complete single-record scans through the original Pascal set helpers.
Movement, deletion, and area-effect service calls are captured in order.
Cases cover all kind bytes and explicit combinations of projectile kinds, weapon IDs, owners, and deadline boundaries.
Comparisons include every record byte and all twelve active-shot counts.
A native regression verifies unchanged state when retirement references an invalid owner.
All 100,397 differential cases, 68 Rust tests, strict Clippy, and the WASM build pass.

This confirms ordinary expiry removal for live weapon 6; it does not establish that weapon's contact behavior.
Retirement visuals and area-effect damage for weapons 1 and 17 remain unintegrated.
Hosting remains stopped.


## Original projectile area-effect requests

Rust projectile_area_requests reproduces CODE 30 +2aa6 through its downstream damage calls.
The routine scans twelve participants in ascending order.
It requires a nonzero bitwise intersection of participant active byte +88 and enemy byte +5.
It excludes targets whose geometry kind is 10.
It does not check participant health before requesting damage.

The range metric starts with the recovered geometry planar distance.
The routine subtracts source height from target height with signed wrapping, divides by 65536 toward zero, takes absolute value, and doubles it.
It adds that integer term directly to the fixed-point planar distance.
This apparent scale mismatch is original behavior, not a proposed correction.
Distances at or above 240 shifted left sixteen produce no request.

The base damage comes from the enemy damage bank at parameter offset +75a.
The index is the target geometry's signed low weapon byte at +1d, not the source projectile's weapon.
Below 60 shifted left sixteen, the routine doubles the damage word with wrapping.
Below 120 shifted left sixteen, it wraps the damage-times-three result to a signed word, then divides by two toward zero.
Other admitted targets receive the base damage.
Requests pass mode zero, current global time, the source owner byte, and the selected participant number.

The harness executes 1,200 complete original calls using original geometry distance and integer division routines.
Only the downstream CODE 31 +a84 damage handler is captured.
Tests compare every request and its order, including noncanonical flag bytes, range boundaries, height extremes, and signed damage overflow.
They also verify unchanged geometry and participant records while that handler is intercepted.
The native helper guards invalid referenced indices and returns requests without applying damage itself.

All 101,597 differential cases, 68 Rust tests, strict Clippy, and the WASM build pass.
Live area-effect application, its visual state, and full damage-handler integration remain unfinished.
Hosting remains stopped.


## Original damage response state

Rust `damage_response_fields` reproduces CODE 31 +de2 through +e7c after health, counters, and score handling.
The geometry deadline becomes current time plus 60 for enemies, 260 for the local non-enemy participant, or 66 for other non-enemy participants.
The addition wraps as a signed 32-bit value.
On a kill, the old kind moves from +14 to +15 and kind +14 becomes 10.
Participant byte +54 becomes one, including nonfatal hits.

In a nonzero game mode, a kill first calls the multiplayer death service with attacker and victim bytes.
The Rust helper returns these arguments; it does not execute the service.
A future caller must preserve the original notification-before-transition ordering if the service inspects geometry.

The harness executes 1,024 original blocks with randomized records, noncanonical flag bytes, and signed time boundaries.
It compares all geometry and participant bytes and captures notification arguments and call ordering.
All 102,621 differential vectors, 68 Rust tests, strict Clippy, and the WASM build pass.
This helper is not yet connected to the live damage path. Full handler integration remains unfinished.
Hosting remains stopped.


## Original death-burst gate and radial weapon requests

Rust `death_burst_gate` reproduces the complete CODE 31 +a22 routine.
Above signed global A5-136e value 27, it returns the participant's nonzero enemy byte unchanged.
Otherwise, participant kind 8 and a global value above 19 produce byte one. Other inputs produce zero.
The caller at +e7c requires game mode zero and a nonzero bitwise intersection of this result and the killed byte.
This preserves noncanonical flag behavior. The global's gameplay meaning remains unassigned here.

Rust `radial_burst_requests` reproduces CODE 31 +8c6 through its downstream constructor or network calls.
The routine emits nothing when signed A5-136e is at most 20.
It selects two directions when the geometry owner equals the local participant, or four directions otherwise.
Above 27, it obtains one Random word and applies wrapping absolute value and signed remainder by 360.
Otherwise, the initial angle is zero.
Before each request, it applies wrapping absolute value, adds 180 or 90 degrees, and takes signed remainder by 360.
Thus the first request uses the incremented angle, not the initial angle. The minimum signed Random word preserves its original overflow behavior.

Requests preserve source X/Z and replace the heading with the calculated fixed-point angle.
Height is source fixed-point height divided by 65536 toward zero, then narrowed to a signed word.
In game mode zero, each request calls CODE 30 +11c with source kind 7, weapon 0, current time, and the supplied owner.
The supplied owner is distinct from the geometry owner that determines the direction count.

Other modes use the CODE 27 +288 network service. Local ownership always permits the request.
For owner zero, any nonzero authority byte at A5-4f4f permits it.
For other owners, bit zero must be set in both the authority byte and the participant's enemy byte.
Random consumption still occurs before this network gate.
The native helper returns requests; it does not allocate projectiles or send network messages.

The harness executes 1,024 complete gate calls and 1,600 complete radial routines.
It supplies Random results and captures downstream calls, including arguments, order, and unchanged source geometry.
Original integer multiplication and division execute directly.
All 105,245 differential vectors and 68 Rust tests pass. Strict Clippy and the WASM build also pass.
Live burst integration and full damage-handler composition remain unfinished. Hosting remains stopped.


## Original death fragment requests and constructor argument correction

Rust `death_fragment_requests` reproduces the complete CODE 31 +776 request producer.
The routine obtains two Random words before it checks network authority.
The first word determines the count: wrapping absolute value, signed remainder by three, then plus three.
This normally produces three to five fragments. The minimum signed word, -32768, produces one fragment.
The second word determines the initial angle through wrapping absolute value and signed remainder by 360.

The angular step is integer 360 divided by the count.
Each request uses initial angle plus its one-based index times the step, then the original fixed-point angle wrapper.
X/Z remain unchanged. Height uses signed division by 65536 toward zero and narrowing to a signed word.
The local constructor receives weapon 6, the source record's previous kind at +15, current time, and the supplied owner.
Weapon 6 creates the recovered bouncing projectile. Its constructor and collision effects remain outside this request helper.
Network requests carry weapon 6 but no previous-kind or time argument.
Their authority gate matches the radial routine, including owner-zero and noncanonical flag behavior.

Constructor tracing corrected the preceding radial routine's argument labels.
CODE 30 +13c reads the weapon byte from A6+c; constructor branches read the source-kind byte from A6+a.
CODE 31 +8c6 therefore requests weapon 0 with source kind 7, not weapon 7 with mode zero.
The earlier documentation now uses these correct labels.
Both native request helpers explicitly expose weapon, optional source kind, and optional time.
The harness decodes the two byte arguments separately and ignores unused Pascal padding bytes.

The new harness executes 1,600 complete original death fragment routines.
It supplies the two Random values and captures constructor/network requests in order.
Original multiplication, division, and angle wrapping execute directly.
Checks cover signed extremes, all previous-kind bytes, authority flags, and unchanged source geometry.
Live death fragment integration remains unfinished. Hosting remains stopped.

All 106,845 differential vectors, 68 Rust tests, strict Clippy, and the WASM build pass.


## Damage sound selection connected to player hits

Rust `damage_sounds` reproduces the sound branches at CODE 31 +eee..+f3a and +f84..+fc4.
A nonfatal local hit requests 0x40003 at priority three.
A fatal local hit first requests 0x1fc1c at priority one.
With more than one remaining life, it then requests 0x10005 at priority one.
Otherwise, it requests 0x1000d at priority one and calls the sound flush service.
Nonlocal hits request 0x40004 or, on death, 0x40005 at priority three.
The helper returns the request order and flush flag. It does not execute the audio scheduler.

The harness executes 512 original sound branches with signed life boundaries and noncanonical killed bytes.
It captures the sound and flush services.
The local fatal branch stops before the AI reset loop; the nonlocal branch disables the separate AI notification path.
Thus these checks establish sound routing, not complete damage-handler behavior.

Enemy projectile hits now use this helper for player audio.
Fatal hits no longer play the nonfatal hit sample before the death sample.
Campaign deaths choose their sound from the life count before respawn consumes a life.
The diagnostic encounter has no respawn, so it uses the final-life branch.
The browser preloads original sample 10013 for that branch.
The sample adapter still ignores control request 0x1fc1c, priority scheduling, and the flush flag.
Those behaviors require the original sound-channel scheduler. Enemy target sound routing remains provisional.

A live simulation test forces enemy hits and distinguishes nonfatal audio from final-death audio.
All 107,357 differential vectors, 69 Rust tests, strict Clippy, and the WASM build pass.
Encounter parity, original sample preservation, audio events, and campaign life checks also pass.
Hosting remains stopped.


## Original AI reset and local-death notifications

Rust `reset_ai_participant` reproduces the complete CODE 20 +2a30 service.
It writes participant behavior +9 and movement mode +10 to three.
It sets longwords +40, +44, and +4e to -1; clears longword +4a and words +48 and +52; sets byte +30 to one.
For participant one only, it fills the sixteen global cache bytes at A5-3b68 through A5-3b59 with 0xff.
All other participant bytes remain unchanged, including tracked pose, target fields, heading, reverse flag, and heading deadline.
The harness compares complete records and cache bytes across 768 original calls.

Rust `death_ai_reset_targets` reproduces the CODE 31 +f3a..+f82 notification scan after local death.
It visits participants one through twelve in order and skips inactive bytes.
A nonzero enemy byte admits the reset.
Otherwise, the active byte must have a nonzero bitwise intersection with global A5-12d2.
Another 512 original scans verify call order and noncanonical flag behavior with the reset service captured.

The live encounter now sends these notifications before player respawn or final loss.
Preview target slots map to participants two through twelve. Active targets have positive health and are not retired.
The adapter applies the represented behavior, movement mode, and firing-timer writes from the recovered reset.
It preserves the existing tracked pose and target fields. The remaining raw AI fields have no live representation yet.
This is partial service integration, not a complete participant-record replacement.

A live test verifies the reset after an enemy kills the player and confirms that tracking data survives.
The authored-arena life test previously assumed two deaths for a stationary player.
With the reset, enemies can return to flag guarding after the first respawn. A 16,000-tick diagnostic confirmed that branch.
The test now checks one authored-arena respawn. Its existing open-field fixture checks both respawns and final death.
All health, ammo, score, flag, clock, and life-preservation assertions apply to both fixture respawns.

All 108,637 differential vectors, 70 Rust tests, strict Clippy, and the WASM build pass.
Encounter replay parity and campaign life checks pass. Hosting remains stopped.


## Enemy hit sounds use the recovered damage response

Both live player-projectile collision paths now call `damage_sounds` after the target health update.
A surviving or protected target requests 0x40004. A dead target requests 0x40005.
The sample adapter maps these requests to original samples 10004 and 10005.
The previous implementation emitted both samples on a fatal hit. That duplicate response is removed.
Original request priority remains available from the helper, but browser channel scheduling still ignores it.

The native destruction test now checks each of three hits: two nonfatal samples followed by one death sample.
The type-31 immunity test also requires nonfatal feedback and rejects a death sample while the target remains protected.
The authored-arena WASM audio test checks its one-hit kill at the exact destruction frame.
It requires one death sample and no nonfatal sample in that frame.
The previous assertion required the duplicate hit sound; the replacement follows the recovered branch.
All 105 exported original samples remain byte-preserved, and redraw/reset checks still reject duplicate events.

All 70 Rust tests, strict Clippy, the WASM build, audio checks, encounter parity, and campaign life checks pass.
The added type-31 assertion also passes in a targeted rerun.
Recovered routines are unchanged; the differential baseline remains 108,637 passing vectors.
Full damage-handler composition and original sound-channel scheduling remain unfinished. Hosting remains stopped.


## Local death state and original camera transition

Rust `damage_killed` reproduces CODE 31 +b9a..+bae.
Game mode zero derives byte zero or one from health at or below zero.
Other modes preserve the caller's supplied byte verbatim.
The harness compares 512 original selectors, including signed health boundaries and noncanonical mode/flag bytes.

Rust `local_death_state` reproduces CODE 31 +bae..+c34.
Only a local victim with a nonzero killed byte changes state.
The routine clears geometry words +c, +e, and +22 and enables global byte A5-131b.
It copies the geometry pose into globals A5-1328 onward, then reverses the saved heading by 180 degrees through the original angle wrapper.
It records current time and writes the original constants: duration divisor 200, angular factor 360, radial factor 200, and height factor 750.
Other bytes in the thirty-byte global region remain unchanged.
Another 1,024 original blocks compare complete geometry and global regions, including signed wrapping angles and early exits.

CODE 6 +50c0..+5196 consumes this state for the active death camera.
Rust `death_camera` reproduces that complete calculation with a supplied direction lookup.
Elapsed time is the wrapping difference between current time and the saved start.
It multiplies elapsed time by the angular factor, divides by duration, then converts the integer degrees to fixed point.
It adds the saved heading and wraps the angle before direction lookup.
The radial distance and height rise use equivalent signed integer multiplication and division.
Position adds the returned direction components times radial distance to the saved X/Z.
The camera heading reverses the computed angle by 180 degrees.
Height narrows the rise to a signed word, adds the signed base height with wrapping, then extends the result.
The native helper rejects duration zero before direction lookup. The original assumes a nonzero divisor.

The harness executes 1,024 original camera calculations with original integer arithmetic and angle wrapping.
Direction lookup results are supplied, so these cases do not establish the original direction-table values.
Tests compare lookup angles, output pose, output height, and unchanged input state.
The camera helper does not decide when the transition ends or when respawn occurs.
The live port still respawns immediately; this transition is not connected to rendering yet.

The existing video respawn sheet was inspected again. It shows scenery below the normal ground band around a life loss.
Those stills are consistent with a changing viewpoint, but do not establish this calculation's timing or camera orientation.
The executable remains the evidence for the recovered constants and arithmetic.

All 111,197 differential vectors, 71 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Participant lifecycle timing and transition entry

Rust `participant_transition_ready` reproduces the complete CODE 31 +1cfe routine.
It tests the geometry deadline against current time with signed comparison and treats equality as elapsed.
Deadline 2,000,000,000 is an explicit sentinel and never counts as elapsed, even when current time is greater.
For geometry kinds other than 10, that deadline test determines the returned byte.

Kind 10 uses the same deadline test in game mode zero or for the local participant.
For other participants in multiplayer, the deadline applies when bit zero intersects the authority, enemy, and active bytes.
Otherwise, the routine returns participant changed-byte +54 XOR one. This preserves noncanonical byte results.
The routine does not alter participant or geometry state.
The harness compares 2,048 complete original calls across time boundaries, kind values, flag bytes, and both network paths.

The caller at CODE 31 +1ed8..+1eee invokes lifecycle handler +1a80 when the returned byte is nonzero.
That handler first requires an active participant.
Rust `begin_participant_transition` reproduces the subsequent +1aa0..+1afa state writes.
It clears participant changed-byte +54, sets the deadline sentinel, and restores the geometry kind.
Enemies use participant kind +8; other participants become kind 6.
Health and life handling follows these writes. Dead local participants lose a life, then either respawn or enter final-loss handling.
The earlier recovered damage response sets a local non-enemy deadline to current time plus 260.
Together, these routines establish the local death wait without assuming that the camera divisor 200 is the respawn duration.
The handler clears the death-camera flag during spawn at CODE 31 +546.

Another 512 original blocks verify transition-entry writes across all participant kinds and noncanonical enemy flags.
A Rust integration test connects damage response, deadline admission, and transition entry.
It checks local, enemy, and other-player delays; the exact boundary; and suppression after the sentinel reset.
The live port still uses immediate player respawn. Delayed lifecycle scheduling and camera rendering remain to be connected.
Hosting remains stopped.

All 113,757 differential vectors, 72 Rust tests, strict Clippy, and the WASM build pass.


## Death transition connected to the playable loop

The live game now composes local death state, damage response, lifecycle admission, and transition entry.
On a fatal player hit, it saves the fixed-point pose and starts the original 260-tick deadline.
Existing enemy reset notifications and death audio occur once at the fatal hit.
During the wait, player movement, turning, surface contact, flag collection, and firing stop.
The clock, enemy simulation, existing projectiles, effects, and model animation continue.
Zero health prevents repeated damage and repeated transition initialization.

At the exact deadline, the lifecycle gate admits transition entry and clears the pending state.
Campaign play then consumes a life through the existing recovered respawn adapter.
A remaining life restores health/ammo and chooses a spawn; the final life enters loss.
The diagnostic encounter has no remaining-life loop and enters loss at the same deadline.
Restart clears the pending transition. Desktop and WASM snapshots expose its start and deadline.
The adapter still has provisional within-tick ordering and incomplete original participant fields.

The recovered death camera now supplies pose and height to the renderer.
It uses the candidate unit direction table, whose SANE generation remains modeled.
CODE 18 +c7c confirms the same index/complement output ordering used by the direction adapter.
The base height is 32, matching the spawn write to global A5-20c2.
The death camera overrides interior, exterior, and overhead views; normal view selection survives respawn.
Crosshair rendering stops during the transition.
Projection, palettes, and death fragments remain unfinished. Final loss still uses the preview end state rather than original score entry.

Offline WASM frames at elapsed ticks 20 and 120 were inspected.
The early frame shows the raised viewpoint; the later frame has almost no visible arena geometry.
That exposes a projection/calibration limitation and is not evidence of a faithful visual match.
Local screenshots are `.local/death-camera-20.png` and `.local/death-camera-120.png`.

A native test covers 259 blocked ticks, the exact transition tick, world updates, camera changes, remaining-life respawn, final freeze, and restart.
The WASM death test checks blocked inputs, stable damage counts, the deadline, view overrides, one respawn, and redraw audio.
The life suite now verifies the fatal hit at transition start and life consumption at transition end.
It retains health, ammo, score, flag, clock, and final-death checks.
All 73 Rust tests, strict Clippy, and the WASM build pass.
Death, audio, encounter parity, lives, 112-arena replay/render, and 476 campaign boundary checks pass.
Recovered routines are unchanged; the differential baseline remains 113,757 passing vectors.
Hosting remains stopped.


## Death camera height units corrected from original ground projection

The live death camera previously interpreted renderer-global height as scene units.
CODE 6 +3d86..+3d8c multiplies that global by eight before the ground projection call.
CODE 11 +3ce6 shifts fixed-point horizontal position right by ten, producing a scale of 64 per scene unit.
Its general projection path multiplies both this horizontal value and the supplied height by the same reciprocal coefficient.
Thus a global camera height of eight corresponds to one planar scene unit.
The ordinary global height 32 corresponds to the existing four-unit eye height.
The port now divides recovered death-camera height by eight at the renderer boundary.
The recovered death-state and camera arithmetic remain unchanged.

Rust `project_ground_point` reproduces a single-point call to CODE 11 +3ce6.
It includes the eight-unit near limit, lateral/depth culling, the strict 300-unit sum limit, table indexing, and signed word arithmetic.
For supplied height 256, the original uses a precomputed table offset instead of the general multiply path.
Both paths preserve wrapping screen-coordinate additions.
The native helper rejects an unavailable table entry.
The harness executes 1,024 complete original calls with synthetic reciprocal and precomputed-height table pairs.
It compares accepted points and output coordinates, including near/culling boundaries and the optimized height branch.
These cases establish the arithmetic and units, not the original table contents or complete renderer.

A native test compares equal planar and vertical offsets through the recovered projection.
The death-camera WASM test now also requires visible ground points at elapsed tick 120.
The regenerated offline midpoint frame shows arena geometry and ground where the previous frame was nearly blank.
`.local/death-camera-120.png` records the corrected render.
Full projection matching, palette recovery, and death-fragment rendering remain unfinished.

All 114,781 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
The WASM death transition and 112-arena replay/render checks pass. Hosting remains stopped.


## Original ground projection table initialization

Rust `initialize_ground_projection_table` reproduces CODE 6 +442..+4c2 after allocation.
The table contains 1,200 pairs of signed words. The block initializes indices one through 1,199 and preserves entry zero.
For indices below 32, the reciprocal word is 32768 divided by the index.
For indices at or above 32, it is 1048576 divided by the index.
The result narrows to a signed word, including the value -32768 at index 32.
The second word is 4096 divided by the index. The height-256 projection path uses this precomputed vertical offset.
No floating-point or modeled SANE operation is needed for this initializer.

The harness executes the original initializer four times with different preexisting table bytes.
Original CODE 36 division and the CODE 1 +b8 bytewise word writer run directly.
All 4,800 bytes are compared on each run, including the untouched entry zero.
Another 512 complete original single-point projections use the table generated by the original initializer.
These cases include near-boundary depth entries, culling limits, and ordinary/death-transition heights.
They supplement the earlier synthetic-table checks with the original table values.
The remaining screen-center, viewport, point enumeration, and rendering-dispatch work still needs recovery before replacing the live ground projection.

All 115,297 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
Live rendering is unchanged in this step. Hosting remains stopped.


## Display rectangle and viewport placement

Rust `viewport_metrics` reproduces CODE 33 +746..+7a6 and +7c2..+7e6.
It accepts screen bounds and the display extent words at A5-3ff0 and A5-3ff4.
The view width is the smaller of the screen width and display width.
The view height is the smaller of the screen height minus 80 and display height.
The projection center is half the local view size, with signed division toward zero.
The placement arguments center the view in the screen area after the 80-pixel upper band.
Word subtraction and addition retain the original wrapping behavior.
The later QuickDraw calls also add the screen origin. The helper returns only the relative placement.

The harness executes 1,024 original metric blocks with the original minimum helper.
It compares size, projection center, and placement arguments before QuickDraw OffsetRect.
These cases include offset screen origins, odd dimensions, small screens, and signed-word boundaries.
No QuickDraw service fixture is needed for these blocks.

The apparent limit words are the low halves of display extent longwords, not independent fixed camera constants.
CODE 17 +43c..+4ca writes those longwords at A5-3ff2 and A5-3ff6.
Rust `display_rectangle_setup` reproduces +43c..+4b0, before the InsetRect trap.
A nonzero byte at A5-7bf0 selects the full display rectangle.
When that byte is zero, the inset arguments are `(width - 512) / 2` and `(height - 342) / 2`.
Both divisions truncate toward zero. Odd dimensions can thus retain one extra pixel after the inset.
The original code recalculates the extents after InsetRect, then adds 80 to the rectangle top.
The recalculated height precedes that top adjustment; do not subtract the upper band twice.

Another 512 original blocks verify the initial extents and optional inset arguments.
They use the original CODE 36 division routine and stop before QuickDraw InsetRect.
CODE 17 +42e..+43a also supplies a 512-by-342 fallback rectangle to SetRect.
That fallback and the later rectangle mutations are listing evidence, not executed service checks.
The mode byte also appears in preference loading and renderer branches. Its full effect remains unrecovered.

A separate local execution of CODE 4 startup returned successfully and left both display extent longwords zero.
This ruled out static initialization as their source and directed the trace to CODE 17.
The temporary startup dump is research evidence only; the port does not load it.

All 116,833 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
Live framing remains provisional until the rectangle operations and rendering dispatch are connected.
Hosting remains stopped.


## Full-display ground projection and row traversal

Rust `project_ground_point_for_display` adds the original CODE 11 +3de6 branch to the earlier +3ce6 projection.
The full-display branch accepts lateral magnitudes up to twice the depth.
The compact branch accepts lateral magnitudes up to the depth.
Both retain the eight-unit near limit and the combined lateral-plus-depth limit of 300 units.
Both use identical lookup coefficients, height handling, and projected offsets.
Thus this mode changes the lateral culling range without multiplying the projection scale.
CODE 6 +3daa..+3e42 selects these routines with the same A5-7bf0 display-mode byte traced in viewport setup.

Rust `project_ground_row` implements their row iteration with wrapping coordinate increments.
Zero and negative counts produce no new points. Rejected points still advance the row position.
The native function returns accepted points in order. It replaces the original caller-owned output buffer with a vector.
The existing single-point helper retains compact-mode behavior.

The harness executes 1,024 complete original row calls across both display modes.
Cases include zero and negative counts, rows of up to 64 points, near and lateral boundaries, and both height branches.
The reciprocal table comes from the original initializer.
Each call starts with a random output prefix. The harness verifies that the original preserves it and appends accepted points.
Every appended coordinate and the final output count match the Rust results.
No Mac service fixture is used in these calls.

All 117,857 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
The live ground-point generator remains provisional.
Its replacement still needs the CODE 6 +3b10 world-to-row setup and row selection.
The listing shows every eighth row marked at CODE 6 +4cc..+4ea.
CODE 33 +348..+366 derives ground spacing from the world extent and the word at A5-27be.
These listing observations identify the next recovery targets; they are not yet native implementations.
Hosting remains stopped.


## Original ground lattice in the live renderer

CODE 6 +4c2..+500 initializes 121 row markers and selects 64 divisions.
Every eighth row marker is one. All other markers are zero.
Both branches of the preference check select the same division count.
Rust `ground_grid_defaults` reproduces those outputs.

CODE 33 +260 sets the world extent to 1024.
CODE 33 +348..+36a calculates fixed-point spacing as extent times 65536, divided by the division count.
Rust `ground_grid_spacing` preserves signed arithmetic and guards a zero divisor.
The original defaults give 16 scene units between adjacent grid indices.

CODE 6 +3d08..+3d86 calculates the first sparse column and the sparse sample count.
Rust `ground_sparse_row` reproduces its signed-word arithmetic, including wrap and division toward zero.
The recovered row dispatch uses adjacent columns on marked rows and every eighth column on other rows.
The bounds helper at +3a86..+3b00 clamps indices to zero through the division count, inclusive.
The bounds-helper observation is listing evidence; its complete calculation remains unrecovered.

The live first-person renderer now uses the recovered lattice defaults, spacing, and sparse-column selection.
The full arena contains 1,089 candidate ground points instead of the former uniform grid of 4,225.
Death views use the same lattice. Camera transforms, projection, point color, and visible-row selection remain provisional.
The original point projection is not yet connected to live geometry rendering.

The harness executes four original lattice initializers with different preference bytes.
It also executes 512 original spacing blocks and 1,024 original sparse-column alignment blocks.
Original integer helpers run directly. No service fixtures are needed for these cases.
All 119,397 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
WASM death-transition checks and desktop/WASM replay checks across 112 arenas also pass.
The regenerated `.local/death-camera-120.png` shows the sparse lattice and visible arena geometry.
Hosting remains stopped.


## Visible ground bounds and camera basis

Rust `ground_visible_bounds` reproduces the complete CODE 6 +3918 routine.
Its inputs are a starting position, two displacement vectors, grid spacing, and the division count.
The routine compares four positions: the start, each displaced position, and the position with both displacements.
It converts the minimum and maximum coordinates to grid indices with the original shift and signed division sequence.
It expands each side by two indices, then limits the lower bounds to zero and upper bounds to the division count.
The native helper rejects a zero divisor and a quotient that would overflow original DIVS.
It does not silently normalize inverted bounds.

The harness executes 1,024 complete original calls with the original minimum and maximum helpers.
The cases cover different spacing, division counts, positions outside the arena, and positive or negative displacements.
An initial comparison exposed an omitted starting corner in the native calculation. The corrected four-corner calculation passes.

Rust `ground_camera_basis` reproduces CODE 6 +3b48..+3bc2 with software fixed multiplication.
It takes the direction values stored at A6-48 and A6-4c: table[heading], then its quarter-turn complement.
This is the reverse of the native movement helper output order.
It returns the transformed world origin and column step. The row step exchanges the column components and negates the second.
Multiplication truncates signed fractional results toward zero and retains the original 32-bit result wrapping.

Another 1,024 cases execute the original basis block and the row advance at +3e74..+3e84.
They supply direction values and execute original CODE 11 software multiplication directly.
Positions include signed-long boundaries. Direction-table generation remains a separate modeled dependency.
The harness compares both origin coordinates, both column increments, and both observed row increments.

All 121,445 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
These helpers are not yet connected to live rendering.
The remaining connection includes construction of the visible-region vectors and composition of the original row renderer.
The live sparse lattice from the preceding step remains active. Hosting remains stopped.


## Composed original ground-point pipeline

Rust `ground_points` composes CODE 6 +3b10..+3e96 through point generation, before pixel drawing.
It uses the recovered 1024-unit world, 64 divisions, row markers, and reciprocal projection table.
It constructs the visible-region vectors with the original fixed-point constant 0x00d38000.
The full-display branch doubles selected direction terms before multiplication.
The routine then selects bounds, constructs the camera basis, aligns sparse columns, and projects each selected row.
It multiplies the renderer height by eight before projection, as the original caller does.

The complete-call comparison resolved two interface ambiguities in the earlier helper descriptions.
The direction pair is table[heading] followed by its quarter-turn complement, rather than native movement-helper order.
The basis stores depth before horizontal. The projector receives horizontal before depth.
World-x indices select rows; world-z indices select columns.
The native composition now preserves those orders explicitly.

The harness executes 256 original caller paths through point generation.
Original bounds selection, arithmetic helpers, sparse alignment, and both row projectors execute directly.
Only direction lookup is supplied. Pixel drawing begins after the stopping point and is excluded.
Cases cover seven direction pairs, both display modes, five heights, varied screen centers, and positions across the arena.
Every output count, coordinate, and point order matches the Rust composition.
The first comparison exposed the coordinate-order error; the corrected composition passes all generated vectors.

All 121,701 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
The live renderer still uses its provisional projection with the recovered sparse lattice.
Connecting this pipeline requires consistent framing and projection for ground points and solid geometry.
Original direction-table generation and pixel drawing remain separate recovery targets. Hosting remains stopped.


## Live ground pipeline integration

The first-person renderer now calls `ground_points` for ordinary play and the death camera.
Each frame owns a reciprocal table initialized by the recovered original initializer.
`Game::first_person_camera` supplies fixed camera position, direction values, and renderer height.
Ground points and solid geometry use the same direction pair from the modeled unit table.
This replaces the former floating-point heading calculation for first-person geometry.
It does not change simulation movement or the normal exterior and overhead cameras.

The presentation adapter selects full-display ground culling and requests offsets around center zero.
It scales those offsets by 500/256, then adds the preview center and horizon.
The factor matches the preview solid renderer's 500-pixel focal length to the recovered ground projector's 256-pixel scale.
Integer projection retains its original quantization. Solid geometry still uses the preview projection and rasterizer.
The adapter is a presentation choice, not a recovered original screen mode.
Original viewport placement, palette, pixel drawing, and direction-table generation remain unfinished.

All 74 Rust tests, strict Clippy, the WASM build, and WASM death-transition checks pass.
Desktop/WASM replay and render checks across 112 original arenas pass.
The unchanged recovered helpers retain the preceding 121,701 differential-vector verification.
Offline frames `.local/ground-driving-forward.png` and `.local/ground-driving-turned.png` show the ground and arena during ordinary play.
The regenerated `.local/death-camera-120.png` shows the original distance-limited ground during the death orbit.
These frames were inspected. Browser hosting remains stopped.


## Original color bindings and base palette

Rust `color_bindings` reproduces CODE 6 +838..+94a after resource selection.
The caller selects resource 999 of type cob1, cob4, or cob8 according to the display mode.
The block skips the first word, then copies four paired entries and 47 paired object-kind entries.
It copies the high bytes of four effect entries and eight special entries.
The low bytes of those final twelve entries are unused in this block.
The helper rejects fewer than 128 bytes. The original block assumes its resource is present and long enough.
Later display-depth overrides at +94a..+97c remain separate.

The first special entry goes to A5-3b32, which the ground renderer reads at +3ea4 and +3eb6.
All three original cob resources select color index zero for ground points.
This identifies an index, not the final RGB value.
The harness executes 515 original binding-loader blocks, including all three disc resources and 512 generated byte tables.
It compares every copied byte across all four groups. No resource-service fixture executes inside the tested block.

Rust `base_palette` reproduces CODE 11 +1184..+11c4.
It copies the first 13 RGB triples from a selected clut resource by entry position.
It ignores each entry's value word and preserves all 16 bits of each channel.
The native helper guards resources shorter than 112 bytes.
The harness compares 516 original copies, including all four disc cluts and 512 generated tables.

The listing shows additional palette processing after this copy.
CODE 11 +11c4..+11f8 replaces entries two and three through level-color routines.
The subsequent block forces the last active entry to black and performs display-specific conversion.
CODE 25 +1b08 selects clut 999 without a parameter block, otherwise the ID at parameter offset +81e.
The wave loader writes its color-resource ID there at CODE 25 +1c9e.
These selection and substitution paths remain listing evidence, not newly implemented services.
In particular, clut 999's first RGB triple is 65280/65280/65280; the other disc cluts have different first colors.
Do not replace the live ground color with a universal white constant based only on the binding index.

All 122,732 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
The live palette remains provisional until selection, substitutions, and display conversion are connected.
The point-drawing path also uses separate parity queues in its eight-bit renderer; that path remains unrecovered.
Hosting remains stopped.


## Level-specific palette colors

Rust `level_palette_color` reproduces complete CODE 11 routines +d9c and +e1a.
They select entries from clut 2000 and clut 2001, respectively.
The index is the signed remainder of the wrapped word `level - 1` divided by 30, limited to a minimum of zero.
Normal positive levels repeat the 30-entry sequence.
Clut 2001 is bypassed at display depth four, which returns yellow without requesting a resource.
A missing clut 2001 also returns yellow: RGB 65280/65280/0.
A missing clut 2000 returns magenta: RGB 43264/0/43264.
The original RGB constructor shifts channel words left eight bits.
The initial native fallback had incorrect channel order; the original-call comparison exposed and corrected it.
Present but truncated resources are rejected by the native helper.

The harness executes 1,024 complete original calls.
It supplies GetResource handles, records each requested type and ID, and checks the four-bit resource bypass.
Original index arithmetic, maximum helper, RGB constructors, and resource-byte copies execute directly.
Cases cover both disc resources, generated RGB tables, missing handles, signed level boundaries, and five display depths.
These calls use explicit resource-service fixtures; they do not execute the Macintosh Resource Manager.

Several disc values corroborate the supplied video's color changes.
The table lists channel high bytes for readability; the recovered helper preserves all 16 bits.

| Level | clut 2001 RGB | clut 2000 RGB | Reference observation |
| --- | --- | --- | --- |
| 1 | 255, 255, 0 | 247, 0, 247 | Yellow scenery and magenta horizon in the opening arena |
| 7 | 219, 58, 121 | 67, 67, 133 | Pink scenery around 7:20 |
| 10–12 | 136, 0, 215 | 0, 88, 0 | Purple scenery and green horizon during later levels |
| 13–14 | Blue variants | 137, 255, 137 | Brighter green horizon in the late footage |

These comparisons support the role of the two substitutions, but do not establish the complete draw-color assignment or horizon shading.
The live palette still needs base-resource selection, substitutions, active-color conversion, and renderer integration.
All 123,756 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Composed resource palette

Rust `compose_resource_palette` reproduces CODE 11 +1184..+1224 with a present base clut.
It copies all 13 base RGB triples, substitutes entries two and three, then forces the last active entry to black.
The black assignment follows the substitutions. Small active palettes can thus overwrite a substituted color.
The native helper rejects invalid active counts and malformed resource bytes.
It does not implement the missing-base-resource branch or subsequent display conversion.

The harness executes 512 composed original blocks.
They call both original level-color routines and the original black RGB constructor.
GetResource handles are supplied, and the requested resource order is checked.
Cases vary level, display depth, active counts of 2/4/8/13, base palettes, generated tables, and missing level resources.
All 39 resulting channel words match the native composition, including entries beyond the active count.

The Central campaign inventory contains its own clut 1000, named New Matrix Version Colors.
The executable supplies the shared cob tables, level-color tables, and default clut 999.
Live loading must preserve campaign palette overrides while resolving shared resources from the executable.
Hardcoding only clut 999 would discard the campaign's authored colors.

All 124,268 differential vectors, 74 Rust tests, strict Clippy, and the WASM build pass.
Live palette loading and display conversion remain pending. Hosting remains stopped.


## Live shared palette loading and ground color

`Viewer::load_shared_colors` reads clut and cob8 resources from the owned executable before campaign loading.
It requires a valid cob8 999 table and retains only color resources.
Campaign loading places its resources after the shared tables, so matching campaign IDs take precedence.
Shared colors persist when another campaign loads. Late replacement is rejected without changing the active game.
The browser loads the extracted executable through a local route before opening the selected campaign.
The native drive, replay, and campaign-level commands accept `--colors FILE` for the same path.

Each wave now composes its authored base palette with shared level colors using depth eight and 13 active colors.
This selects the recovered eight-bit path; it does not emulate every original display configuration.
Game snapshots expose the resulting palette and ground color index for verification.
The first-person renderer resolves ground dots through that index and uses channel high bytes for RGBA output.
Missing palette or binding data retains the preview ground color.
Object colors, horizon shading, and original display conversion remain unfinished.

The new palette check covers 16 native/WASM scenarios across all three campaigns and four levels.
It reads authored palette and level-color bytes independently, then compares all 13 RGB triples.
It checks campaign precedence, reload, reset, invalid shared input, and rendered ground pixels.
The test exposed inconsistent native/WASM setup metadata; both setup outputs now include the palette fields.
The death-camera check now loads shared resources and tests visible ground using the selected RGB value.

All 74 Rust tests, strict Clippy, WASM build, and palette checks pass.
The 476 campaign/boundary checks, 112-arena replay checks, and death-transition checks pass.
Recovered arithmetic remains unchanged from the 124,268-vector differential verification.
The offline `.local/ground-palette.png` frame was inspected. Hosting remains stopped.


## Face color selector recovery

CODE 19 +3ea0..+3f3a resolves the color argument before it calls CODE 12 +708.
Selectors 255 and -1 become 13. Signed selectors below 1000 use their signed low byte to index A5-2742.
The loaded byte becomes a signed word. Selectors of 1000 or more pass through unchanged.
The Rust helper rejects indices outside the supplied table. The original can read adjacent globals for those inputs.

The harness executes 1,024 complete original wrapper calls with supplied 47-byte kind tables.
It captures the downstream constructor call and checks that all other 18 argument bytes remain unchanged.
Cases include sentinels, encoded values, signed aliases, and all possible byte values in the supplied tables.
The full suite now passes 125,292 differential vectors. All 75 Rust tests, strict Clippy, and the WASM build pass.

CODE 1 +177a reads the face selector after the vertex list.
For values of 1000 or more, +178c calls CODE 16 +204 before submission at +1850.
That routine searches 64-byte records and can return a record index plus 1000, or -1 when its feature flag is disabled.
Its resource-loading paths remain unrecovered. Thus encoded selectors must not become direct palette indices.
CODE 12 +51e stores the submitted color word at offset 10 in the computed 12-byte face record.

The new selector helper is not yet connected to rendering. Original object colors and downstream material handling remain unfinished.
Hosting remains stopped.


## Face lighting and material-path evidence

CODE 12 +1854..+18dc computes the base face shade from three signed normal components.
The normal uses a scale of 1024. Two horizontal light components come from earlier caller calculations.
The vertical light multiplier is 1842. Two signed sums contribute their positive high words; the routine then adds 24 and caps at 64.
The Rust translation preserves long overflow, signed high words, and word narrowing.

CODE 12 +1924..+1986 adjusts this shade. The bright branch halves the unsigned word and adds 32.
Otherwise, the fade branch scales by a factor between 2400 and 4800, then divides by 4800.
The factor starts with a wrapping word subtraction of view-record depth from 4800.
The bright branch takes precedence when both flags are set.

The harness executes 1,024 original base-light blocks and 1,024 original adjustment blocks.
It supplies normal components, horizontal light components, depth, and flags. Original min/max helpers execute directly.
It covers valid-scale normals, wide signed inputs, overflow, and both flag combinations.
All 127,340 differential vectors, 75 Rust tests, strict Clippy, and the WASM build pass.
These helpers are not yet connected to the live renderer.

The next integration dependency is CODE 11 +227a, called by polygon submission at +2482.
That routine converts color and shade into a packed pixel pattern with display-depth branches.
Its eight-bit branch calls +21be four times, using consecutive shade offsets before packing the result.
The current preview's RGB multiplier cannot reproduce this stage directly.
Horizontal light preparation is at CODE 12 +145e..+14be; it uses constants 858 and 2304 after the direction routine at +140.
These preparation and pattern stages still require recovery and verification.

An inventory of authored shap resources found 119 Central, 120 Eastern, and 119 Southern models.
Each campaign has 94 faces with selectors of 1000 or more, including 1997, 1998, and eight distinct 6000-series selectors.
Central and Southern each have 74 sentinel-255 faces; Eastern has 79. Other selectors fall between 0 and 40.
These counts include shared models repeated across campaigns.
CODE 16 +204 routes selectors 1000 through 1999 into the movie-loading branch.
Its other branch starts at +72c and calls the picture-resource trap at +740.
Resource availability, decoding, and missing-resource behavior still need end-to-end verification.
Hosting remains stopped.


## Shade-to-pixel and eight-bit pattern recovery

`shade_pixel_index` transcribes complete CODE 11 +21be..+2278.
It accepts a shade word, color byte, display depth, four-bit color flag, and supplied A5-7526 remapping table.
The helper preserves signed byte indexing and word overflow. It rejects out-of-table reads instead of accessing adjacent globals.
It includes the special four-bit color-three branch, zero-shade output, grayscale branch, and color-ramp branches.

`face_pixel_pattern_8` transcribes complete CODE 11 +227a..+2450 for depth eight with A5-7bf1 cleared.
Nonpositive shade or color 12 returns an all-ones pattern. Other shades are capped at 64.
Color 11 becomes color zero with half the shade.
The routine uses four lookups with consecutive shade offsets, divided by four.
It packs them in order first, third, fourth, second, preserving signed additions and wrapping shifts.
This output is a packed pixel pattern, not an RGB value.

The harness executes 2,048 complete shade lookups across display depths and 2,048 complete eight-bit pattern builders.
The original builder calls its original lookup routine directly. No OS traps or service hooks are used in these new cases.
Supplied tables alternate between identity mappings and wide signed values to expose arithmetic errors.
All 131,436 differential vectors pass. The Rust safety test also checks rejection of missing and negative color indices.

Live rendering integration remains pending. Pixel-pattern placement and RGB shade-ramp construction are the next dependencies.
CODE 11 +1298 initializes the color remapping table; subsequent branches change it according to display configuration.
CODE 11 +1360..+13ec builds RGB ramps using interpolation at +ca0 and a fixed-division trap at +13a4.
The interpolation routine combines unsigned RGB words; its exact rounding remains to be verified.
The eight-bit builder's alternate A5-7bf1 path and complete one-bit/four-bit pattern builders remain outside this helper.
Hosting remains stopped.


## RGB interpolation and eight-bit shade ramps

`interpolate_rgb` transcribes complete CODE 11 +ca0..+d9a, including the original signed long minimum helper.
The fraction is capped above at 65535, then narrowed to an unsigned word. Negative fractions therefore retain their low word.
Each channel combines unsigned products with complementary weights that sum to 65535. The routine returns the high word without rounding up.
Thus a nonzero source channel loses one unit even when the target weight is zero.

`rgb_shade_ramps_8` transcribes CODE 11 +1360..+13ec with 13 active colors and 16 shades per color.
The first twelve base colors each produce sixteen RGB entries. The thirteenth base color is reserved.
Each ramp advances from the base toward the supplied fade color in exact sixteenth steps, ending at fifteen-sixteenths.
CODE 11 +ef0..+f06 constructs black as the caller's fade color.
The eight-bit shaded setup at +f82..+f90 selects 13 colors and 192 output entries.

The harness executes 1,024 complete original interpolation calls with boundary and random RGB inputs and fractions.
It also executes 64 complete ramp blocks and compares all 192 RGB triples from each block.
The original interpolation and signed minimum routines execute directly.
Only the FixRatio trap is supplied: its tested j/16 results are exact in 16.16 representation, so no rounding assumption is needed.
All 132,524 differential vectors pass. The existing generic modeled-trap count does not include these new FixRatio fixtures.

These helpers are not yet connected to live rendering.
CODE 11 +148a..+1498 copies the 192-color table between two buffers; +17ca sends one buffer to palette installation.
Polygon submission proceeds through CODE 17 +24c, +8fe, and +3940.
The last routine is the next target for pixel-pattern placement and framebuffer interpretation.
Other display modes, special palette entries, dynamic palette effects, and material playback remain unfinished.
Hosting remains stopped.


## Eight-bit pixel-pattern placement

`face_pattern_pixel_8` selects a palette index from the original packed pattern at framebuffer coordinates.
CODE 17 +d56..+d64 selects the low word on even rows and the high word on odd rows.
The eight-bit solid-span path at +1054 duplicates that word into a long and writes through masks at aligned addresses.
Even framebuffer columns use the word's high byte; odd columns use its low byte.
Pattern phase follows framebuffer coordinates, so it does not restart at polygon boundaries.
For a pattern packed as first, third, fourth, second, the two rows are fourth/second and first/third.

The harness executes 1,024 original row-selection blocks followed by complete CODE 17 +e62 span-writer calls.
Each case supplies a changed solid span, a prior span, and an aligned framebuffer with random initial bytes.
Cases vary row parity, pattern bytes, destination starts from zero through 31, and lengths from one through 64.
The comparison covers all 128 destination bytes, including untouched bytes before and after each span.
No OS trap or service fixture is needed for these calls.
Textured spans, unchanged-span optimizations, other pixel depths, and unaligned framebuffer bases remain outside this verification.
The first harness run used an unreachable stop address; the corrected boundary is +d64, before its branch.

All 133,548 differential vectors, 76 Rust tests, strict Clippy, and the WASM build pass.
The sampler remains separate from live rendering until face normals and caller color selection are connected.
CODE 12 +17b2..+17b8 supplies the caller's default color before each face.
The sentinel color 13 retains that default at +18dc; other faces can replace it at +18e4.
The time-dependent flash override at +18ea..+1924 remains a separate dependency.
Hosting remains stopped.


## Complete face-plane constructor

`face_plane` transcribes complete CODE 12 +26c..+530 with its original integer-length routine at +20.
Inputs are three packed signed-word vertices and the submitted color word.
The two edge differences wrap as words. The cross product uses signed products and the original winding order.
While any absolute component exceeds 5000, all three components are divided by four toward zero.
The length helper selects a power-of-two seed, then performs three rounded integer iterations.
The constructor scales each normal component to 1024 and divides toward zero.
If the computed length is zero, all three normal components become -1.

The returned 12-byte record contains X normal, Z normal, plane offset, Y normal, and color.
The plane offset is the negative dot product with the second vertex, including for degenerate faces.
The harness executes 2,048 complete original constructors with original division and length helpers.
Cases include small triangles, wide signed coordinates, wrapped edge differences, coincident vertices, and arbitrary color words.
Every byte of each output record is compared. No service fixtures are required.
All 135,596 differential vectors, 76 Rust tests, strict Clippy, and the WASM build pass.

The constructor is not yet connected to the live renderer.
Live integration must use packed model vertices, not unquantized resource coordinates.
Resource vertex packing already has a separate implementation in `PackedShape::from_resource_vertices` with modeled FixRound behavior.
The remaining lighting setup at CODE 12 +145e..+14be combines direction-table words with constants 858 and 2304.
Caller-selected colors, flash overrides, generated-mesh face colors, and resource-backed materials also need integration.
Hosting remains stopped.


## Lighting direction and per-face color selection

`face_light_components` transcribes CODE 12 +145e..+14be.
It combines the two signed direction words with constants 858 and 2304, shifts each sum left twice, then extracts the signed high word.
The harness compares 1,024 original blocks with supplied direction words. Direction-table generation remains a separate dependency.

`rendered_face_color` transcribes CODE 12 +18dc..+1924 with the default color already initialized by the caller.
Stored color 13 returns the signed caller color without reading the clock or changing flash state.
Other colors read TickCount and compare it with the shared signed deadline.
When the current tick exceeds the deadline, a second TickCount result sets a new deadline thirteen ticks later, with wrapping arithmetic.
The routine toggles the low bit of its shared flag.
If the flag is nonzero and the face index is below the supplied limit, the signed flash color replaces the stored color.
Equality at the deadline does not toggle. Equality at the face limit does not flash.

The harness executes 1,024 original color-selection blocks with supplied TickCount values.
It compares the resulting color, deadline, toggle byte, and number of clock reads.
Cases include signed boundaries, wrapping, noncanonical toggle bytes, inherited colors, and unequal clock reads.
All 137,644 differential vectors, 76 Rust tests, strict Clippy, and the WASM build pass.
The generic modeled-trap total does not include these TickCount fixtures.

Live integration remains pending. Caller state must supply model rotation, default color, flash limits, and display flags.
CODE 12 +1e30..+1e68 forwards these fields from 32-byte prepared object records into the face renderer.
The existing scene identifies original resources, animated resources, and procedural meshes separately.
Procedural preview faces currently use placeholder color zero, so they must not be treated as recovered material selectors.
Hosting remains stopped.


## Live authored-face shading integration

First-person authored solid faces now use the recovered color and shading pipeline.
Shape parsing computes normals from the exact packed vertex prefix, with the existing modeled FixRound conversion.
CODE 1 +182e..+1850 and CODE 12 +772..+77e pass the first three face indices in reverse order to the plane constructor.
The parser follows that order and retains the resulting X/Y/Z normal. Degenerate and unavailable normals retain preview rendering.
The normal is omitted from serialized asset metadata. Procedural preview faces have no recovered normal/material pair yet.

Each campaign setup now retains the shared kind-color bindings on Game. Reset preserves them.
The renderer composes the recovered RGB ramps and resolves authored face selectors through the kind-primary table.
Stored color 13 uses the object's kind-primary color in the ordinary-object adapter.
Other display configuration remappings remain outside this adapter, which selects the eight-bit, 13-color path.
Lighting uses object world rotation and the existing modeled quarter-scale direction table.
Shade, pattern packing, row parity, and pixel selection use the verified helpers.
Pattern index 255 maps to black; ramp indices use their RGB high bytes for the RGBA framebuffer.

CODE 6 +29f8..+2a4c supplies the ordinary object's model, primary color, world rotation, relative rotation, and zero flash limit.
CODE 12 +1c2c..+1cb2 prepares those fields; +1e30..+1e68 forwards them to face rendering.
The world rotation at input offset 14 reaches the lighting lookup. Relative rotation at offset 18 serves geometry transformation.
These caller links were traced in disassembly; the complete caller pipeline has not been differentially verified.
The adapter does not yet connect dynamic color overrides, bright/fade flags, picture/movie materials, or procedural face selectors.
Projection, clipping, visibility, and depth interpolation remain the preview rasterizer.

The polygon rasterizer now accepts a pixel-color function while retaining its existing depth behavior.
A new test compares a patterned rectangle against two adjacent polygons with an odd-column boundary and a nearer occluder.
It checks identical output, dither phase, and untouched background pixels.
The palette integration test now requires shaded red pixels on the authored tower in the fixed Central entry view.
All 77 Rust tests, strict Clippy, WASM build, and 16 native/WASM palette scenarios pass.
The 112-arena replay suite, 476 campaign/boundary checks, 18 native/WASM setups, and death-transition checks also pass.
Recovered arithmetic is unchanged from the 137,644-vector verification.
The offline `.local/ground-palette.png` frame was inspected alongside the supplied video's frame-01 reference.
The scenes and levels differ, so this comparison establishes visible progress, not a pixel-exact match.
Hosting remains stopped; browser interaction remains unverified.


## Live procedural tetrahedron colors and winding

CODE 19 +3212..+328a submits four triangular faces, each with the primary color at A5-2741.
This is kind one's primary color. It is a resolved palette value at submission time.
The native mesh retains selector 1 so it resolves through the same shared kind-color bindings at render time.
The verified zero-based face order is [0,3,1], [1,3,2], [2,3,0], [0,1,2].

The harness executes this original block for every possible primary-color byte and captures all constructor arguments.
The first comparison exposed swapped side-face vertices in the initial translation. The corrected topology matches all 256 cases.
The base face matches the old preview's base winding; the side faces required correction.
The complete generated vector file now passes 137,900 comparisons after the Rust correction.
The constructor itself remains covered by the prior complete face-plane tests.

The scene now builds selector 2 from PackedShape::tetrahedron instead of a separate floating-point mesh.
Its visible vertices therefore use the same packed coordinates as the recovered geometry representation.
A new shaded-preview builder computes normals only for procedural meshes with established face order and color semantics.
Other procedural preview builders remain unchanged.

The Central entry snapshot now contains yellow tetrahedrons instead of diagnostic blue tetrahedrons.
The palette integration check requires yellow pixels in a fixed world-region sample as well as red authored tower shading.
All 77 Rust tests, strict Clippy, WASM build, and 16 native/WASM palette scenarios pass.
All 112 arenas render; replay parity, 476 campaign/boundary checks, 18 native/WASM setups, and death-transition checks pass.
The offline `.local/ground-palette.png` frame was inspected after the winding correction.
Object-specific bright/fade flags, other procedural colors, special materials, and original projection remain unfinished.
Hosting remains stopped.


## Live procedural box-body colors

CODE 19 +257e..+258c supplies kind-zero primary color to selector one's box-face builder.
CODE 19 +2024..+2032 supplies kind-four primary color to selector five's box-face builder.
Both callers use vertex offset zero and call complete CODE 18 +11fa, which submits six quadrilateral faces.

The harness executes both original caller blocks and the complete shared builder for every possible color byte.
A constructor hook captures all face arguments. The comparison covers count, winding, vertex indices, and signed color propagation.
All 512 cases agree with the existing box topology and new selector-to-kind mapping.
The face constructor is separately covered by the existing plane-record verification.

Both procedural scene paths now use the shaded-preview builder for their six-face bodies.
This connects packed-vertex normals and their original kind colors to the recovered first-person shading pipeline.
Selector five's line decorations still use diagnostic line rendering. Its alternate filled decorations remain outside this integration.
Column and flag materials, object-specific lighting flags, and picture/movie faces remain unfinished.

All 138,412 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
The 16 native/WASM palette scenarios, 112-arena replay checks, 476 campaign/boundary checks, and 18 setup comparisons pass.
Hosting remains stopped.


## Live procedural column colors

CODE 19 +3472..+35d2 submits two eight-vertex caps and eight quadrilateral walls for selector three.
Every face receives the signed primary-color byte for kind two at A5 -2740.
The live column preview now uses this selector and packed-vertex normals in the recovered first-person shading path.

The harness captures all ten constructor argument lists for each of the 256 possible color bytes.
These faces agree with the earlier column geometry captures and independent fresh-emulator captures at both call boundaries.
An attempted face reordering failed the existing geometry comparison. The implementation retains the verified ring order.
The harness also checks that CODE 19 remains unchanged before the new face fixtures execute.

All 138,668 differential vectors match Rust after this correction.
Object-specific bright/fade flags, flag materials, and picture/movie faces remain unfinished.

All 77 Rust tests, strict Clippy, the WASM build, and 16 native/WASM palette scenarios pass.
All 112 arenas render, and their desktop/WASM replay comparisons pass.
The offline Central frame was inspected. Hosting remains stopped.


## Live filled-flag colors

CODE 19 +1072..+10f0 submits the two filled flag faces with opposite vertex order.
Both read the signed primary-color byte for kind twelve at A5 -2736.
The harness captures both constructor argument lists for all 256 possible color bytes.
Their indices match the existing flag geometry, including the reversed face.

The live flag mesh now supplies kind twelve to the recovered first-person shading path.
Packed vertices provide both face normals. The two faces retain their original winding.
The pole still uses diagnostic line rendering, and decorative lettering is not implemented.
Original object-specific bright/fade flags remain unfinished.

All 138,924 generated differential vectors match Rust after correction of a verifier macro syntax error.
All 77 Rust tests, strict Clippy, the WASM build, and 16 palette scenarios pass.
All 112 arenas render, and their desktop/WASM replay comparisons pass.
The offline Central regression frame was inspected; it does not isolate a close flag view.
Hosting remains stopped.


## Original edge records and opposite-direction merging

Complete CODE 12 +5f2..+69a inserts or updates a model edge.
The model stores its edge count at +25a and ten-byte records starting at +266.
Each record contains two vertex words, two visibility-reference words, a flag byte, and one unwritten padding byte.

The input vertex words occupy stack-frame offsets +e and +10.
The search compares these words directly with the stored pair and selects the first matching record.
On a match, only the second visibility reference changes, using the input word at +c.
On insertion, the constructor stores the vertex arguments in reverse order.
It also reverses the reference arguments from +a/+c and copies the flag byte from +8.
Thus an opposite-direction submission merges into an existing edge. A same-direction submission can create another record.
Equal-endpoint edges and repeated matching records follow the same first-match rule.

These references are not palette colors. CODE 12 +842..+84c supplies the current face index plus 50 to edge wrapper +69c.
The line projection block copies both references into its projected record at +a/+c.
CODE 12 +b6c..+b82 uses them to index visibility bytes.
This identifies a missing part of original hidden-line handling; it does not establish the final line-color mapping.

The new Rust ModelEdge representation preserves both references and the flag.
Its insertion helper matches the original storage and merge behavior.
The differential harness executes 1,536 complete original insertions with no service fixtures.
Cases cover new records, opposite-direction matches, duplicate ordering, equal endpoints, signed words, and arbitrary flag bytes.
Comparison excludes the new record's uninitialized padding byte.
The helper is not yet connected to preview rendering. Flagpole and decorative-line colors remain provisional.

All 140,460 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
This change adds recovered data handling without changing live rendering. Hosting remains stopped.


## Original projected-edge selection

CODE 12 +b60..+ba2 selects edges for the first line-list path.
It XORs the projected clipping byte with one, then tests the two referenced visibility bytes.
The first visibility byte uses a nonzero test; the second uses a bitwise AND with the XOR result.
These operations matter for non-boolean inputs.

CODE 12 +bde..+cb0 handles the other initial scan path.
A nonzero clipping byte assigns classification one without queuing endpoints.
An eligible flagged edge queues its endpoints and receives classification one.
An unflagged edge with both referenced faces visible also queues its endpoints.
With exactly one visible reference, signed screen-coordinate order determines classification zero or two.
With neither reference visible, classification is one and no endpoints are queued.
The comparison uses projected-record words +2 and +6. Equal words follow the less-than-or-equal branch.

The Rust helpers preserve both paths, including arbitrary byte flags and signed coordinate comparisons.
The harness executes 2,048 original blocks and compares queue counts and classification bytes.
For queued edges, it also checks the eight copied endpoint bytes against the source record.
Inputs cover all combinations of bytes 0, 1, 2, and 255, additional random bytes, and equal coordinates.
The first scan leaves the projected classification byte unchanged.

Later passes consume classifications zero and two and apply special ordering to edges two and eight.
Those passes and the final drawing services are outside this comparison.
The new helpers are not yet connected to the live renderer and do not resolve the reported rendering artifacts by themselves.

All 142,508 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Live rendering is unchanged by this research step. Hosting remains stopped.


## Complete projected-edge drawing dispatcher

The Rust edge_draw_batches helper now translates complete CODE 12 +b30..+fd8.
It accepts a contiguous range of projected records and retains the original one-based indices.
The first mode queues selected endpoints in scan order and calls A5 +6d2.
The other mode classifies records and produces up to three batches.
Empty batches do not call a drawing service.

The initial batch preserves scan order. Its service is +722 or +6d2, selected by the style byte.
The second batch contains classification-two records. Its service is +71a or +70a.
With special ordering enabled, edges two and eight move to the end, in that order.
The third batch contains classification-zero records. Its service is +72a or +712.
It uses insertion sorting with absolute cross-products of signed, word-wrapped coordinate differences.
Equal comparison values insert the new edge before the existing edge.
With special ordering enabled, edges one and nine bypass this sort and append in that order.
All drawing calls receive the supplied color byte unchanged.

The verification harness executes the complete original dispatcher and captures only its final drawing-service calls.
It compares service addresses, colors, endpoint order, batch boundaries, and all output classification bytes.
The first 1,024 cases cover empty ranges, arbitrary flags, signed coordinates, equal slopes, and ranges excluding special indices.
These cases reach all six drawing services but produce sorted batches of at most three edges.
Another 128 cases fill the original 50-edge capacity to exercise longer sorting and special-edge ordering.
No OS services or intermediate arithmetic are replaced in this dispatcher test.
The final drawing services remain capture fixtures; this does not verify their rasterization.

The helper is not yet connected to live rendering. Original projection and line rasterization still require integration.

All 1,152 complete dispatcher comparisons and 143,660 total differential vectors pass.
All 77 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.


## Original line palette patterns

CODE 11 +149c..+1592 builds thirteen line patterns and two supporting word tables.
The input is the signed remapping table at A5 -7526; its final entry identifies black aliases.
At depth four, non-black entries shift left two bits. Black aliases use 15.
The high word shifts this result left four more bits.
For other supplied depths in this selected block, non-black entries shift left four bits and black aliases use 255.
Both output words are equal on that branch.
All word shifts wrap at sixteen bits.

The two zero-extended words are ORed, then multiplied by 0x01010101 with thirty-two-bit wrapping.
This produces the pattern at A5 -755e plus four times the color index.
Four-bit color mode replaces pattern five with 0x48488484 after building the tables.
The Rust line_palette helper preserves this special case and both supporting tables.

The harness executes 512 original setup blocks and the original multiplication helper.
Cases include identity remapping, arbitrary signed words, black aliases, and zero/nonzero four-bit mode bytes.
It compares all thirteen entries in each output table without OS fixtures.
Depths sixteen and thirty-two test this block's arithmetic branch only; they do not establish its outer dispatch for those display modes.

CODE 11 +20a4 reads the supplied color byte as a signed table index in its immediate drawing branch.
It forwards the selected pattern and each endpoint pair to CODE 17 +1d8.
The deferred branch instead records lines through CODE 11 +6da and +4ee.
The downstream rasterizers and deferred queue handling remain outside this verification.
The live renderer still uses provisional line colors until these paths and their callers are integrated.

All 144,172 differential vectors, 77 Rust tests, and strict Clippy pass.
The WASM build passes. Hosting remains stopped.


## Deferred line clipping and raster records

Complete CODE 11 +4ee..+658 clips two screen endpoints against the viewport and constructs a sixteen-byte record.
The input endpoint order is [y0, x0, y1, x1]. The routine first orders endpoints by x.
It clips against left, right, top, and bottom bounds using signed integer multiplication and division.
Rejected lines leave the destination record unchanged and return a zero byte.
Accepted lines return one and write the complete record.

The record contains the clipped starting x, major-axis length, framebuffer address, signed row step, minor-axis length, and major-axis length again.
Its final two bytes identify the major axis and starting pixel mask.
The framebuffer address includes the supplied origin and row stride.
Eight-bit mode uses a byte offset and zero mask.
Four-bit mode halves the x offset and records odd/even position as 255 or zero.
The remaining branch uses an arithmetic shift by three and a rotated 0x7f bit mask.
Coordinate differences, origin addition, and signed row-step negation wrap at word width.
Address arithmetic wraps at long width.

The Rust deferred_line_record helper implements this clipping and record construction.
The harness executes 2,048 complete original routines without service fixtures.
Inputs include viewport sizes from one pixel upward, endpoints inside and outside the viewport, coincident endpoints, and positive/negative row strides.
Coordinates range from -2,048 through 2,048. Origins range from -100 through 100.
Depths one, four, and eight exercise each starting-mask branch.
Comparison checks the accepted record byte-for-byte and confirms unchanged destination memory after rejection.
This bounded input coverage does not establish behavior for every extreme signed-word clipping case.
The helper is not yet connected to live drawing; queue allocation and pixel writing remain separate work.

All 146,220 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Original eight-bit deferred line pixel writer

Complete CODE 10 +440..+4b2 consumes the sixteen-byte records produced by CODE 11 +4ee.
CODE 11 +37a6 reaches this writer through A5 +642 while processing deferred line batches.
Its mask word comes from A5 -764a or -7612, selected by the supplied frame phase.
These tables differ from the immediate line-pattern table at A5 -755e.

The writer initializes its error accumulator to the arithmetic half of the major-axis length.
Each iteration writes `(old_byte & and_mask) | or_mask`, then updates the error and address.
The AND mask is the second-lowest byte of the supplied long; the OR mask is its lowest byte.
The upper two bytes do not affect this writer's pixels.
Horizontal-major records advance one byte every iteration and sometimes add the signed row step.
Vertical-major records add the row step every iteration and sometimes advance one byte.
Both endpoints are included, so a zero-length record writes one pixel.
The record list remains unchanged.

The new Rust raster_deferred_lines_8 helper reproduces this writer within a bounded framebuffer slice.
It reports an out-of-range address instead of accessing memory outside that slice.
The integration harness executes both the original clipper and the complete original pixel writer without service fixtures.
It compares Rust-generated records, all bytes in a 64-by-48 framebuffer, and unchanged original records.
The 512 batches include random initial pixels, arbitrary mask words, empty batches, rejected lines, and overlapping lines.
Explicit samples cover both diagonals, horizontal lines, vertical lines, and single points.
Endpoints also extend beyond the viewport so the integration exercises clipping before drawing.

This establishes the deferred eight-bit writer for the tested batches.
It does not establish the immediate CODE 17 writer or the other display depths.
Frame-phase mask selection, queue management, and live renderer integration remain unfinished.

All 146,732 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Deferred frame-phase masks

Complete CODE 11 +1f88 constructs four-bit deferred masks; +1fdc constructs eight-bit masks.
Both take a signed mapped-color word and a phase byte. Every nonzero phase selects the same branch.
Each constructor writes all four output bytes.

At depth eight, phase zero uses AND 0x38 and OR mapped_color's low byte.
The other phase uses AND 0x07 and OR the low byte of mapped_color shifted left three bits.
Each AND/OR pair appears twice in the output long.
For ordinary mapped colors zero through seven, these operations preserve the other phase's three-bit color field.
Bits outside those fields do not survive the AND operation.

The four-bit phase-zero bytes are [0xcf, color shifted four bits, 0xfc, color].
Its other phase uses [0x3f, color shifted six bits, 0xf3, color shifted two bits].
All shifts use word arithmetic and store only the low byte.
The Rust deferred_line_mask helper preserves these exact writes.

The selected palette loop at CODE 11 +161a..+1742 calls both original constructors for thirteen remapping entries.
It also writes two word tables: mapped color times 80 and five at depth four, or times nine for both at depth eight.
These products wrap at sixteen bits.
The new deferred_line_palette helper exposes the two phase tables and supporting word tables.
This deferred branch is separate from the immediate line palette and its black-alias handling.

The harness executes 1,024 complete mask builders and 512 complete thirteen-color setup loops.
It compares every output byte using arbitrary signed remapping words and zero/nonzero phase bytes.
No arithmetic or service calls are replaced in these tests.
The original eight-bit pixel writer was independently verified with arbitrary masks in the preceding integration tests.
Frame-phase palette RGB composition, queue management, and live renderer integration remain unfinished.

All 148,268 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Deferred RGB tables and composed frame phases

CODE 11 +1596..+161a constructs two square RGB tables from the active base colors.
The selected deferred setup paths supply four or eight base colors.
Each row of the table at A5 -750a repeats one base color.
The table at A5 -708a is its transpose: each row contains the entire base palette.

Complete CODE 11 +1816 selects the transpose for an even frame counter and the row-constant table for an odd counter.
It publishes the palette pointer, copies the configured entry count, and sets the pending-update byte.
For eight-bit deferred pixels, even frames reveal the low three-bit field; odd frames reveal the high three-bit field.
The Rust deferred_rgb_palettes helper returns these tables in even/odd order.

The harness executes 256 original table builds and complete phase selectors.
It compares every RGB word, the selected table, the published entry count, and the pending-update byte.
Inputs include arbitrary RGB words and positive/negative frame counters.

Another 128 composed tests execute two consecutive phases against a shared framebuffer.
Each phase uses original mask construction, viewport clipping, line-record generation, eight-bit pixel writing, and RGB-table selection.
Rust runs the corresponding recovered helpers from the same endpoints and base colors.
Comparison covers all indexed framebuffer bytes and all selected RGB words after each phase.
Initial bytes are 63, representing color-field pair [7,7]; the supplied base color seven is arbitrary in this test.
Lines can overlap or extend beyond the viewport. The second phase must retain the first phase's stored field correctly.
These tests use no service fixtures or emulator substitutes inside the tested paths.

The tests do not include queue allocation, stale-frame erasure, vertical-blank scheduling, or display palette upload.
The existing live renderer still uses its immediate shading path and provisional line rendering.

The first combined run encountered an emulator fetch error at the new stop boundary.
That boundary lay inside the preceding palette test's cached translation; a fresh-emulator execution succeeded.
The harness now clears the translation cache before these new bounded executions.
All 148,652 differential vectors pass after that harness correction.
All 77 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.


## Complete eight-bit deferred paint and erase

CODE 11 +37a6 paints the eight-bit deferred frame groups; +388c erases them.
Each routine copies the 338-byte queue header and visits all thirteen color groups in order.
Every group contains a line-record list and two direct-pixel address lists.
The source header remains unchanged.

Painting selects the phase mask for each group's mapped color.
It sends line records to CODE 10 +440 and applies the same AND/OR bytes to both direct-pixel lists through CODE 11 +4c2.
The direct-pixel mask helper at CODE 11 +257a ignores its lane argument in this eight-bit path.
Overlapping addresses are processed in their original group and list order.

Erasing selects the phase mask for mapped color twelve once, then uses it for every group.
Lines pass through CODE 10 +4b4, whose pixel loop matches +440.
Both direct-pixel lists use color twelve through the original helper and writer.
For valid eight-bit mapped colors, this replaces only the selected three-bit field and preserves the other field.
Erasure does not restore arbitrary original pixels; it writes the mapped background value along the queued geometry.

The new DeferredColorBatch representation retains line records and both direct-pixel lists.
The draw_deferred_frame_8 helper implements both paint and erase within a bounded framebuffer.
The harness executes 256 complete original paint/erase sequences with original helper calls and pixel writers.
It also generates each line record with the original viewport clipper.
No service fixtures replace the tested paths.

Cases use all thirteen groups, variable list lengths, zero/nonzero phases, random valid remapping, clipped lines, and repeated pixel addresses.
Each comparison checks every framebuffer byte after painting and after erasing, plus unchanged original queue headers.
The initial framebuffer can contain either phase's arbitrary three-bit values.
Queue allocation, queue rotation, vertical-blank scheduling, and live renderer integration remain unfinished.

A fixture variable initially replaced report metadata and prevented JSON export.
After separating the queue-header variable, all 148,908 differential vectors pass.
All 77 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.


## Deferred line queue integration

Complete CODE 11 +20a4 takes an immediate/deferred display branch.
With A5 -7be2 zero, it selects the color group's deferred line list and marks the frame dirty.
It reserves capacity for the requested input count, then runs each segment through the original clipper.
Rejected segments decrement the temporary count so subsequent accepted records reuse their slots.
The final count includes only accepted segments. Their input order is retained.
An empty request still marks the frame dirty.

The new DeferredFrameQueue stores all thirteen color groups and the dirty state.
Its queue_lines method composes the recovered clipping and record builder.
Native vectors replace Macintosh allocation; the method does not emulate original capacity-growth behavior.
LineRasterTarget carries viewport, origin, stride, and framebuffer address for this native path.

The harness executes 256 complete queue/paint/erase sequences with eight queue requests each.
Every original queue call includes its capacity check and original clipper.
Lists begin with 128 preallocated slots, so allocation growth does not run.
The test confirms that capacity remains unchanged and checks every group's count after each append.
It compares the final compacted records and dirty state with Rust.
It then executes complete original painting and erasure from those queues and compares every framebuffer byte.
Inputs cover repeated color groups, empty requests, clipped/rejected lines, and accepted lines after rejections.
No inner service is replaced in the executed preallocated path.

This connects native queue construction to the recovered paint/erase pipeline.
It does not yet connect that pipeline to the live game renderer.
Original queue growth, queue rotation, and display scheduling remain unfinished.

All 149,164 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Deferred queue reset, merge, and header rotation

Complete CODE 11 +840 resets a queue only when its dirty byte is nonzero.
It clears the counts of all thirteen line lists and both sets of pixel lists, then clears the dirty byte.
A clean queue is untouched, even if supplied test data contains nonzero counts.
Pointers and capacities remain allocated.

Complete CODE 11 +896 copies the source header to local storage and checks its dirty byte.
A clean source produces no destination changes.
A dirty source marks the destination dirty and appends all three lists for every color group.
Existing destination entries precede source entries. The source queue remains unchanged.
The native clear_if_dirty and append_dirty methods preserve these logical rules using vector storage.

CODE 11 +3ae8..+3b30 rotates three 338-byte headers.
For slots ordered by A5 -6be0, -6a8e, and -67ea, [A,B,C] becomes [B,C,A].
The native rotate_deferred_queues helper moves complete queues in the same order.
This block alone does not include the subsequent merge of the auxiliary queue or its reset.

The harness executes 256 complete merges followed by complete resets with preallocated destination capacity.
It compares every logical record and pixel-address list, checks retained capacities, and confirms the original source header remains unchanged.
The original capacity checks run; no allocation growth is required by these inputs.
The Macintosh BlockMove trap is an explicit memory-copy fixture. All surrounding original merge instructions execute.
Another 256 cases compare every byte of all three original headers after rotation without fixtures.
Their native comparison uses opaque header-byte markers carried through complete logical queue objects.

The tests cover dirty and clean source/destination combinations, empty lists, and existing destination entries.
Original capacity growth, complete frame scheduling, and live renderer integration remain unfinished.

All 149,676 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
The new merge cases use the BlockMove fixture described above. Hosting remains stopped.


## Complete deferred eight-bit frame cycle

The new DeferredFrameCycle composes the recovered queue, drawing, erasure, palette-phase, and rotation behavior.
It translates the deferred eight-bit path through complete CODE 11 +3a78.
The original waits until the prior palette upload clears A5 -7658.
The native method returns None while palette_pending is true; the platform must acknowledge upload before calling it again.
This is a readiness boundary, not an emulation of the original busy-wait timing.

After readiness, the original erases the oldest queue only if it is dirty, then resets it.
The erasure phase is `(frame - 1) & 1`, equal to `(frame + 1) & 1` even across wrapping.
It paints the current queue with that next phase, increments the frame counter with long-width wrapping, and publishes the new palette selection.
It then rotates the three queue headers, appends the auxiliary queue into the newly displayed queue, and resets the auxiliary queue.
Painting is called even when the current queue's dirty byte is zero.
The native method preserves this ordering and sets palette_pending after advancing the counter.

The harness executes 128 complete original frame cycles with palette readiness supplied at entry.
Original erase/draw dispatch, pixel writers, palette publication, rotation, auxiliary merge, and reset all execute.
The Macintosh BlockMove trap remains the explicit fixture for list copies.
Queues have preallocated capacity, and the optional callback flag is disabled.

Each comparison covers every framebuffer byte, the frame counter, all queue contents and dirty states, and the pending palette state.
The harness also checks the published palette pointer and both original flags cleared by the frame routine.
Inputs include positive/negative counters, signed-long wraparound, dirty/clean queues, empty/nonempty lists, and auxiliary geometry.
All line records come from the original clipper before the full cycle runs.
These tests do not cover vertical-blank timing, platform palette upload, allocation growth, or the immediate display branch.
The live game renderer has not yet been switched to this pipeline.

All 149,804 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Packed vertex transformation and screen projection

Complete CODE 11 +3ba8 transforms the packed vertex list and projects each output record.
Each input record supplies three signed coordinate words; its remaining four bytes are skipped.
The routine multiplies coordinates by three supplied matrix rows, with wrapping long additions.
Each row sum shifts arithmetically right fourteen bits, then adds its translation word with word-width wrapping.
The output stores three transformed coordinates and two screen-coordinate words per vertex.
The original count word is copied to the output list.

Complete CODE 11 +3b7e provides the same projection for a single existing record.
Both routines test the sign of the wrapped word subtraction `depth - 10`.
That differs from a normal signed comparison for depths -32768 through -32759.
When this branch skips projection, both previous screen-coordinate words remain unchanged.
Otherwise each signed coordinate shifts left eight bits and divides by the signed depth.
If the DIVS quotient overflows a word, the instruction leaves its destination unchanged and the routine stores that destination's low word.
The Rust project_vertex_record and transform_project_vertices helpers preserve these operations.

The harness executes 2,048 complete single-record projectors and 512 complete transform/project pipelines without service fixtures.
Inputs cover depth boundaries, extreme negative depths, quotient overflow, arbitrary signed matrix rows, identity matrices, and wrapping translations.
List counts include zero, one, two, eight, and sixty vertices.
The comparison checks every output word, retained projection words, the copied original count, and unchanged source records.

The live renderer still uses provisional solid geometry projection.
Original camera/model matrix construction and viewport placement must be connected before these helpers can replace that path faithfully.
This work does not itself fix the reported visibility artifacts.

All 152,364 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Hosting remains stopped.


## Model rotation and translation from render records

CODE 12 +0x1af6 through +0x1b56 constructs the matrix passed to the packed vertex projector.
The Rust model_view_transform helper now reproduces that block and its +0x0140 direction lookup.
The lookup normalizes the signed integer part of the relative heading into 0 through 359 degrees.
It reads sine and cosine from the supplied direction table, then retains their low words.
The rotation rows are [cosine, 0, -sine], [0, 16384, 0], and [sine, 0, cosine].
Negation preserves word overflow.

Translation negates the horizontal render-record coordinate, subtracts camera height from object height, and retains depth.
These coordinates use packed eighth-unit words. Subtraction preserves word overflow.
The helper receives camera height after conversion; it does not reproduce the earlier FixRound trap.

The harness executes 1,024 original setup blocks, including the complete direction lookup, without service fixtures.
It supplies arbitrary signed direction-table values and covers extreme headings, fractional headings, and wrapped translations.
Every matrix and translation word matches Rust. The original render record remains unchanged.
This verifies table access and arithmetic, not the original SANE table initialization.

All 153,388 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.
Live integration still requires the upstream world-to-render-record conversion and viewport placement.
The playable renderer has no visual change from this helper alone. Hosting remains stopped.


## Camera-relative planar coordinates for solid models

Rust render_record_planar reproduces CODE 6 +0x1af2 through +0x1b6a.
Earlier instructions at +0x1a84 subtract the camera's fixed coordinates from each object's fixed position.
The recovered block shifts each difference left five bits, adds 0x8000, and extracts the signed high word.
It then rotates these rounded coordinates with signed word products and wrapping long sums.
The high words become depth in D7 and horizontal displacement in D5.
CODE 6 +0x2a08 and +0x2a0c store these values into render-record offsets 6 and 8.

The basis words come from frame offsets +0x10 and +0x14.
The caller at +0x541e through +0x542c supplies their containing longwords.
CODE 6 +0x5284 through +0x5290 obtains these values from CODE 18 +0x0dcc.
That lookup uses the original indexed cosine/sine tables, rather than evaluating trigonometry per vertex.
The new helper accepts the basis explicitly; it does not claim to verify the original SANE initialization.

The harness executes 2,048 original blocks without service fixtures.
Cases cover arbitrary signed positions, rounding boundaries, cardinal bases, arbitrary signed bases, and overflow.
Both output words match Rust, and the original block restores its stack pointer.
All 155,436 differential vectors, 77 Rust tests, strict Clippy, and the WASM build pass.

Live integration remains pending. It needs consistent coordinate signs, packed model units, height conversion, and viewport placement.
This change adds recovered arithmetic but does not change the playable image. Hosting remains stopped.


## Live integration of solid object origin conversion

The first-person renderer now calls render_record_planar for each solid object's camera-relative origin.
It converts scene positions to fixed coordinates, subtracts the camera position, and supplies the indexed camera basis.
The basis uses candidate_collision_table, which matches the modeled SANE initialization of the original cosine/sine tables.
CODE 18 +0x0dcc and its caller establish the order: cosine at frame +0x10, sine at +0x14.
The adapter negates horizontal displacement and divides both coordinates by eight for scene units.

Game::first_person_heading supplies the normal or death-camera heading for this lookup.
Local mesh rotation, face clipping, and viewport projection remain on the existing path.
Shots and impacts also retain that path. These limits prevent a claim of complete projection fidelity.
The model-heading convention still needs reconciliation between world scenery and dynamic participants before packed rotation can replace local mesh rotation.

Validation passed: 77 Rust tests, strict Clippy, WASM build, 16 palette scenarios, and the 260-tick death-camera test.
All 112 arena view checks and desktop/WASM movement replays passed.
Campaign validation covered 476 levels/boundaries and 18 native/WASM setups.
The offline .local/ground-palette.png frame was inspected; its shaded towers, tetrahedrons, and ground remain visible.
No browser host was started. The earlier 155,436 original comparisons still establish the recovered helpers; this turn changed their live adapter.


## Live packed mesh rotation and native headings

Shape now retains its packed vertex list alongside the inspection coordinates.
Resource loading preserves the existing original vertex conversion, including its 60-vertex limit and modeled FixRound operation.
Procedural models retain the same packed vertices used to construct their faces.
Meshes without a complete packed list keep the diagnostic transform.

The first-person renderer now composes render_record_planar, model_view_transform, and transform_project_vertices.
It uses the transformed XYZ words for depth and clipping, with conversion from eighth-units to scene units.
The final projected words remain unused because viewport placement and near-plane clipping still use the preview path.
The camera height uses the renderer-global word units, including during death transitions.
Object height remains zero in this adapter; original flight and other height-dependent object states remain unfinished.

The original render record receives object heading at CODE 6 +0x1a9a.
CODE 6 +0x29d0 subtracts camera heading before CODE 12 constructs the rotation matrix.
Scenery retains the authored native heading from world_placement.
Dynamic targets currently store a 90-minus-native scene heading, which the renderer converts back before forming the relative angle.
Face lighting now uses that same native world heading.
This resolves the rendering convention without changing simulation or collision adapters.
Those adapters still need a separate audit of scenery orientation.

An asymmetric packed segment regression checks the actual rendered result at two native scenery headings.
At camera heading 90, native object heading zero sends the segment right; matching object and camera headings send it into depth.
This test catches the former quarter-turn convention error that symmetric scenery can conceal.

Validation passed: 78 Rust tests, strict Clippy, WASM, 16 palette scenarios, 112 arenas, and the 260-tick death transition.
Campaign checks covered 476 levels/boundaries and 18 native/WASM setups.
The regenerated offline palette frame was inspected. Hosting remains stopped.
Direction values and resource vertex rounding retain their documented modeled-OS limitations.


## Scenery orientation shared by rendering and collision

The audit found a second use of the participant heading conversion in collision_preview::Query::from_game.
It converted authored scenery heading with 90-minus-heading, although world_placement had already supplied native degrees.
This disagreed with original object records and the recovered model rotation.

Item::native_heading now centralizes the distinction between authored scenery and participant scene headings.
Rendering, collision records, respawn obstacle poses, and AI obstacle tracking use this conversion.
Driving footprints and fallback projectile bounds also distinguish scenery from participants.
Those latter collision algorithms remain provisional; only their orientation mapping changes here.

The error affected Flow Fields because their contact record supplies the heading used by the recovered directional displacement.
The four-heading surface test now checks native headings directly: zero pushes toward negative Z, and ninety pushes toward positive X.
Native headings 180 and 270 push toward positive Z and negative X respectively.

An asymmetric 80-by-8 obstacle test exercises the recovered participant query at native headings zero and ninety.
It also checks the actual driving footprint and fallback point-hit query on opposite sides of the obstacle.
The long side blocks and the narrow side remains clear. Rotating ninety degrees reverses those query results.
This addresses one concrete source of invisible collision area; it does not establish that every reported early collision is fixed.

Validation passed: 79 Rust tests, strict Clippy, WASM, all 112 arena checks, and native/WASM movement and encounter replays.
All 476 campaign checks, 18 native/WASM campaign setups, 16 palette scenarios, and the death-transition test passed.
Hosting remains stopped.


## Consistent object orientation in all playable views

Exterior and overhead rendering still used scenery's native angle as a participant scene angle.
Both paths now use Item::scene_heading, which derives the floating scene rotation from the shared native-heading conversion.
First-person fallback meshes and provisional collision footprints use the same helper.
This prevents a camera switch from rotating the apparent footprint of asymmetric scenery.

The existing asymmetric packed-segment regression now renders all three playable views.
It checks the segment's screen location at native headings zero and ninety in each view.
The test uses empty player geometry so the player model cannot satisfy an obstacle assertion.
The exterior and map camera parameters remain provisional; these checks establish object orientation, not complete original camera fidelity.

Validation passed: 79 Rust tests, strict Clippy, WASM, all 112 arena view checks, and desktop/WASM movement replays.
The death-camera checks and 16 palette scenarios also passed. Hosting remains stopped.


## Original indexed face clipping and screen placement

Rust clip_project_face reproduces the complete CODE 12 +0x12ca routine.
The routine walks polygon indices, starting with the last vertex as the previous endpoint.
It clips against packed depth 16, or two scene units, using signed comparisons.
A crossing emits an interpolated vertex before the current vertex, if the current vertex is inside.
Unclipped vertices retain the projected coordinates already stored in their records.

CODE 11 +0x3b58 performs signed-word interpolation.
Coordinate and depth differences wrap before multiplication and division.
If DIVS overflows, the stored result uses the product's low word, then adds the previous coordinate.
A zero depth difference returns the previous coordinate.
New intersection records use depth 16 and the original +0x3b7e projector.

Screen placement adds the horizontal center and offset, and subtracts projected Y from the vertical center before adding its offset.
All screen operations preserve word overflow.
The Rust interface uses zero-based indices and returns XY pairs; the original receives one-based indices and stores YX pairs.
Empty or invalid index lists return None as host guards.

The harness executes 2,048 complete original clippers with the original interpolation and projection helpers.
No service fixtures are used for these new cases.
Inputs cover signed coordinate extremes, depths around the clipping plane, arbitrary stored projections, repeated indices, and wrapped screen offsets.
It compares output count and every screen coordinate, checks unchanged source records, and checks untouched output capacity.
All 157,484 differential vectors, 79 Rust tests, strict Clippy, and WASM pass.

Live clipping still uses the provisional renderer. Integration must preserve depth-buffer inputs while adopting original intersections and projected coordinates.
Hosting remains stopped.


## Live original clipping and projection for solid faces

First-person packed solid faces now use the recovered indexed clipper and projected vertex words.
The shared clip_face_records helper retains XYZ and projected XY for every emitted vertex.
clip_project_face uses those same records for original screen placement, preserving the existing differential test coverage.
The live depth buffer receives eight divided by packed depth, which equals reciprocal depth in scene units.
Faces with fewer than three surviving points are discarded, matching CODE 12 +0x1832 through +0x183e.

The host still adapts the original 256-pixel projection scale to its provisional 500-pixel scale and horizon.
Two-point model edges, unsupported meshes, shots, and impacts retain separate preview projection paths.
The new depth regression checks both intersection records at depth 16, their projected coordinates, and retained interior vertices.

Integration exposed an upstream requirement: CODE 6 +0x27fe through +0x283c rejects object origins far outside the horizontal view.
Without that gate, an offscreen face at zero origin depth reached a DIVS overflow and filled the screen.
Rust solid_origin_visible now reproduces both original display-mode branches and runs before live solid transformation.
It compares wrapped absolute horizontal minus 288 with depth plus 288.
The full-size branch zero-extends the depth sum before doubling; the smaller-view branch compares signed words.
The host selects the full-size branch. Original display-mode selection and the remaining distance/type visibility gates remain pending.

The harness executes 1,024 original gate blocks, stopping at each branch exit without replacing any arithmetic.
All 158,508 original differential vectors pass, including the earlier complete face-clipping comparisons.
Validation also passed: 80 Rust tests, strict Clippy, WASM, 112 arena checks, native/WASM movement replays, death camera, and 16 palette scenarios.
The corrected offline palette frame was inspected. Hosting remains stopped.


## Original near clipping for model edges

Rust clip_edge_records reproduces CODE 12 +0x106c through +0x1128.
Both endpoints below packed depth 16 reject the edge.
If only one endpoint lies below that plane, the routine replaces it with an interpolated point at depth 16.
Interpolation always runs from the first endpoint toward the second, regardless of which endpoint is replaced.
This order matters because the original uses wrapped signed-word arithmetic and truncating division.
The routine projects the new endpoint and preserves the untouched endpoint's complete record.

The harness executes 2,048 original clipping blocks with the original interpolation and projection helpers.
It compares all ten endpoint words on accepted edges, both rejection flags on rejected edges, and stack balance.
Cases include the clipping boundary, signed extremes, either endpoint behind the plane, both behind, and arbitrary retained projection words.
No arithmetic or OS service fixture is used in these new cases.

First-person packed two-point model geometry now calls this helper.
The depth buffer receives the clipped endpoint depths, while the screen adapter uses their original projected coordinates.
Original edge visibility, classification, and color dispatch are separate from this integration and remain unfinished in the live renderer.
Unsupported geometry and shot/impact markers retain preview projection.

All 160,556 original differential vectors, 80 Rust tests, strict Clippy, and WASM pass.
The 112 arena view checks, native/WASM movement replays, death-camera checks, and 16 palette scenarios also pass.
The updated offline palette frame was inspected. Hosting remains stopped.


## Original solid depth and distance admission

Rust solid_distance_visible reproduces CODE 6 +0x283c through +0x2880.
A nonzero suppression byte rejects the object before arithmetic.
Packed depth 2400 or greater also rejects it, corresponding to 300 scene units of forward depth.
Both branches preserve the supplied distance cache.

A cache word of -1 requests calculation.
The routine squares signed horizontal and depth words, adds with long overflow, and takes the signed high word.
Any other cache value is reused.
Admission then requires that signed cache word to be below 87.
These comparisons preserve the original signed behavior even for abnormal coordinates or cache values.

The harness executes 2,048 original blocks without arithmetic fixtures.
It checks admission, preserved or calculated cache, stack balance, nonzero suppression bytes, and exact depth/cache boundaries.
All 162,604 original differential vectors pass.

First-person solid rendering now applies this helper after the horizontal origin gate.
The adapter supplies suppression zero and cache -1, recalculating distance for each object.
Original type-specific suppression producers and cache scheduling remain pending.
The change affects drawing only; distant enemies remain in the simulation.

Validation passed: 80 Rust tests, strict Clippy, WASM, 112 arena checks, native/WASM movement replays, death camera, and 16 palette scenarios.
The offline frame was inspected after distant solids were culled. Hosting remains stopped.


## Original face-plane visibility test

Rust evaluate_face_plane reproduces complete CODE 12 +0x0aa4.
It reads signed normal words at packed-plane offsets 0, 8, and 2, and the signed constant at offset 4.
Each normal multiplies the corresponding signed coordinate word.
The three products and constant sum with long overflow.
Rust face_plane_visible reproduces complete CODE 12 +0x0afe: a nonnegative wrapped result is visible, including zero.

The harness executes 2,048 complete evaluations and their original visibility wrappers without service fixtures.
Inputs include arbitrary plane bytes and signed coordinates.
Three quarters of the cases choose constants that produce wrapped results of -1, zero, or +1.
The test compares the full signed result, visibility byte, and unchanged plane record.

All 164,652 original differential vectors, 80 Rust tests, strict Clippy, and WASM pass.
Live face culling is not yet connected: the caller's coordinate transform and complete packed plane must be preserved first.
This helper alone makes no visual change. Hosting remains stopped.


## Live original face culling and viewer coordinates

Rust face_viewpoint reproduces CODE 12 +0x1c30 through +0x1c50 and +0x1cec through +0x1d56.
It converts object-minus-camera fixed world offsets to signed words by shifting left five bits and taking the high word.
Unlike the earlier object projection path, this conversion adds no rounding bias.
Each word is negated, then rotated with the object's world heading and the original direction lookup.
The two products produce the viewer's local X and Z words after wrapping long arithmetic and high-word extraction.

The original draw call supplies these words at frame offsets +0x1a and +0x16.
The height coordinate comes from camera height minus object height at +0x17d4 through +0x17dc.
The new helper combines those three coordinates for the original plane evaluator.
The harness executes 1,024 original conversions, including the complete direction lookup and original height subtraction.
All coordinates, stack balance, and unchanged source records match Rust without service fixtures.
Arbitrary supplied table entries test lookup and arithmetic; original SANE initialization remains modeled.

Authored and supported procedural faces now retain their complete 12-byte plane record.
The first-person renderer evaluates that plane at the recovered viewer coordinate before clipping the face.
A negative result rejects it; zero remains visible.
Unsupported faces without plane records keep diagnostic behavior.
The current adapter uses object height zero; flight and other original object-height states remain pending.

A rendered regression shows a front face, reverses its winding, and verifies that the back face no longer draws.
All 165,676 original differential vectors, 81 Rust tests, strict Clippy, and WASM pass.
The 112 arena checks, native/WASM movement replays, death camera, and 16 palette scenarios also pass.
The offline frame was inspected. Hosting remains stopped.


## Original horizon bands and live palette integration

Rust horizon_bands reproduces complete CODE 6 +0x4a1a as rectangle drawing requests.
Ordinary mode uses five-pixel bands starting at shade index zero.
The four-bit flagged branch uses six-pixel bands and starts at index four.
The routine clamps level divided by eight plus eleven into zero through fifteen, with signed division toward zero.
Levels below one emit no shaded bands.
Color three uses shade 48 minus three times the index; other colors use 32 minus twice the index.
The initial bottom is projection-center Y plus three, followed by the supplied vertical offset.
A valid solid-color override emits one rectangle sixteen bands high at shade zero.

The harness executes 1,024 complete original routines.
It models QuickDraw OffsetRect with wrapping word additions and captures the final rectangle paint requests.
All rectangle coordinates, colors, and shades match Rust across display modes, overrides, level boundaries, and signed coordinate ranges.
This verifies draw requests; it does not verify original QuickDraw rasterization.

CODE 6 +0x934 loads the horizon color into A5-0x3b37 from color-binding special entry six.
The owned executable's cob8 resource selects palette index three, which the campaign already replaces with the level horizon color.
CODE 11 +0x2494 routes ordinary horizon rectangles through the recovered +0x227a shade-pattern routine.
The live renderer now uses those patterns and the existing RGB ramps, replacing the fixed magenta gradient when original colors are loaded.

The adapter selects eight-bit mode, default level 32, and no solid-color override.
It scales original band coordinates with the current provisional 500/256 viewport adaptation.
Alternate horizon-state scheduling and original viewport sizing remain pending.
The black sky above the bands remains clear, and horizon drawing does not write the depth buffer.

A rendered regression changes palette index three from magenta to green and checks the resulting horizon pixels and black upper sky.
All 166,700 differential vectors, 82 Rust tests, strict Clippy, and WASM pass.
The 112 arena checks, native/WASM movement replays, death camera, and 16 palette scenarios also pass.
The updated offline frame was inspected. Hosting remains stopped.


## Original aiming marks

Rust crosshair_lines reproduces complete CODE 6 +0x415a.
The routine draws three 15-pixel segments, each 40 through 55 pixels from the projection center.
Two segments lie on the horizon; the third lies above the center.
A nonzero hidden flag suppresses all three segments. Coordinates use wrapping signed words.
The caller selects the color through A5-0x3b33, from special color binding entry one.

The harness executes 1,024 complete original routines and captures the final line service.
Ordered endpoints and selected colors match across hidden states and signed coordinate boundaries.
This verifies drawing requests, not the original line rasterizer.
The separate twelve-point overlay at CODE 6 +0x41fc remains outside this implementation.

First-person rendering now uses these segments and the bound palette color.
The current 500/256 viewport scale remains provisional. Death-camera suppression remains active.
A rendered regression checks all three marks, their color, and the open center.
All 167,724 differential vectors, 83 Rust tests, strict Clippy, and WASM pass.
The 112 arena checks, native/WASM replay, death transition, and 16 palette scenarios also pass.
The offline frame was inspected. Hosting remains stopped.


## Original world, radar, and instrument placement

Rust viewport_layout extends viewport_metrics through CODE 33 +0x87e.
It returns the world rectangle, local radar center and rectangle, and instrument rectangle.
Rectangles retain QuickDraw top/left/bottom/right order and wrapping word arithmetic.
The world rectangle receives the calculated offset, then the supplied screen origin.
The radar center is projection-center X plus 194, with Y equal to 62.
Its rectangle extends 60 pixels in each direction.
The instrument strip is 512 pixels wide and 80 pixels high, centered immediately above the world rectangle.

The harness executes 1,024 original placement sequences with varied bounds, limits, and signed boundaries.
It executes the original minimum and rectangle constructors, and models three QuickDraw OffsetRect calls.
It skips the separate marker initialization call at +0x83a.
All rectangle words, centers, extents, offsets, and stack balance match Rust.
All 168,748 differential vectors, 83 Rust tests, strict Clippy, and the WASM build pass.

This helper is not yet connected to the live layout.
The live viewport scale remains provisional until display setup and host placement are integrated.
CODE 17 +0x43c..+0x4d4 supplies the extent limits through longwords at A5-0x3ff2 and A5-0x3ff6.
The viewport reads their low words. Compact mode applies InsetRect before recalculating these limits.
Its top adjustment follows extent calculation; integration must preserve that order.
Hosting remains stopped.


## Complete display extent and compact-mode rectangle setup

Rust display_rectangle reproduces CODE 17 +0x43c..+0x4d4.
It extends display_rectangle_setup through the compact-mode inset and final top adjustment.
Compact mode centers a nominal 512-by-342 rectangle using signed division toward zero.
Odd dimensions can retain an additional pixel. Coordinate changes wrap as signed words.
The routine recalculates the extent longwords after the inset, then adds 80 to the rectangle top.
The saved height therefore includes that 80-pixel band.
Full mode preserves the input extents and applies only the top adjustment.

The harness executes 1,024 complete original blocks with original CODE 36 division.
QuickDraw InsetRect is modeled with wrapping coordinate additions and subtractions.
It compares both extent longwords, all four final rectangle words, and stack balance.
This verifies the game arithmetic around a modeled OS operation; it does not execute QuickDraw.

A regression checks odd compact dimensions and full-mode composition with viewport_layout.
A 641-by-481 screen produces compact limits of 513 by 343, before the top adjustment.
A 640-by-480 full display produces a 640-by-400 world rectangle below an 80-pixel instrument band.
All 169,772 differential vectors, 84 Rust tests, strict Clippy, and WASM pass.

Further source tracing identifies the viewport screen bounds separately from the adjusted display rectangle.
CODE 11 +0x1ae0..+0x1af6 copies the selected graphics-device rectangle into A5-0x6632.
Its monochrome branch at +0x1a7e..+0x1a8e copies the alternate screen bounds.
The host must supply those screen bounds and the saved extent limits separately.
Live framing, instrument rendering, and display-mode selection remain pending. Hosting remains stopped.


## Drawing viewport origin and video framing discrepancy

Rust drawing_viewport reproduces complete CODE 11 +0x1f22.
It subtracts the screen origin from the supplied rectangle and stores local drawing bounds.
It stores the rectangle width and height as wrapping words.
For color depths, the drawing origin becomes the local rectangle's top-left corner.
At depth one, the existing drawing origin remains unchanged.

The harness executes 1,024 complete original routines with a modeled QuickDraw OffsetRect.
It compares local bounds, extents, drawing origin, preserved source rectangle, and stack balance.
All 170,796 differential vectors, 84 Rust tests, strict Clippy, and WASM pass.

The reference frame supplies a separate visual check, but also an unresolved placement discrepancy.
In .local/reference-video/frame-01.png, bright horizontal crosshair runs occupy X 265..280 and 360..375 at Y 321.
The upper mark occupies Y 266..281 at X 320. Endpoints are inclusive.
These match crosshair_lines centered at [320,321], without scaling.
The image is 640 by 480 pixels.

Composing the recovered compact layout with a zero-origin 640-by-480 screen predicts center [320,280].
Full mode predicts the same center, with different world extents.
That 41-pixel vertical difference remains unexplained; the recording's screen setup or capture transformation is not established.
The original CODE 11 +0x1f22 color path adds the local viewport origin to later raster coordinates.
No additional offset was inserted into the port to force agreement with this frame.
Live camera framing remains provisional while this discrepancy is investigated. Hosting remains stopped.


## Immediate axis lines and complete aiming-mark call chain

Rust axis_line_rectangle reproduces the axis-aligned branches of CODE 17 +0x39c6.
It rejects a suppressed surface or a nonpositive pen dimension.
Horizontal and vertical lines become rectangles with inclusive endpoint coverage.
The lower and right rectangle edges add the pen dimensions using wrapping words.
Reversed endpoints use signed minima and maxima. A coincident point takes the horizontal branch.
Diagonal rendering remains outside this helper.

The harness executes 1,024 complete original axis-line calls and captures CODE 17 +0x4004 rectangle-painter requests.
Cases cover both orientations, reversed endpoints, coincident points, pen dimensions, suppression, and signed coordinate boundaries.
The terminal painter capture returns normally so the original caller completes its stack cleanup.

Another 256 comparisons execute the complete immediate crosshair chain:
CODE 6 +0x415a, CODE 11 +0x201e, CODE 17 +0x1d8, +0x898, and +0x39c6.
Only the terminal rectangle painter is captured. The wrappers, color lookup, pen setup, and coordinate conversion execute original instructions.
The resulting rectangles match composed Rust crosshair_lines and axis_line_rectangle calls.
These paths introduce no additional screen-origin offset before the rectangle painter.
They do not verify final pixel placement or color rasterization.
All 172,076 differential vectors, 84 Rust tests, strict Clippy, and WASM pass.

The immediate path uses the surface at A5-0x401c, unlike the deferred path's drawing-origin globals.
CODE 17 +0x7c2 passes a supplied rectangle into surface setup at +0x201e.
That setup retains the rectangle at surface offset +0x0c and its extents at +0x14/+0x16.
Calls at +0x594, +0x5b6, and +0x5ce supply the adjusted display rectangle at A5-0x3ffe.
That rectangle differs from the CODE 33 world rectangle in compact mode.
Further surface setup and final pixel-address tracing are needed before attributing the video's placement difference to this branch.
The live renderer remains unchanged. Hosting remains stopped.


## Immediate surface presentation and destination row address

Rust surface_blit_geometry reproduces the following CODE 17 arithmetic blocks:
- +0x36c2..+0x36da obtains the surface origin in global coordinates.
- +0x3774..+0x3784 subtracts the destination pixmap origin.
- +0x37a0..+0x37ca converts the clipped rectangle back into surface coordinates.
- +0x1c20..+0x1c5a computes the first destination framebuffer row address.

The helper accepts an explicit port translation, pixmap origin, clipped global rectangle, base address, and signed row stride.
Coordinate arithmetic wraps as words; row-address addition wraps as a longword.
The row address excludes the later horizontal pixel offset.

The harness executes 1,024 original block sequences.
It models LocalToGlobal, GlobalToLocal, and OffsetRect with explicit translations.
It compares local clipping bounds, destination offsets, destination row addresses, and unchanged stack position.
Clipping-region intersection, drawing-surface allocation, and final pixel copying are outside this comparison.
All 173,100 differential vectors, 85 Rust tests, strict Clippy, and WASM pass.

This explains a concrete difference between the two display paths under identical supplied screen bounds.
For a zero-origin 640-by-480 screen in compact mode, the immediate surface starts at [64,149].
Its local extent after the upper-strip adjustment is 512 by 262.
The projection center is [256,171], so the immediate surface predicts a screen center of [320,320].
The deferred viewport instead starts at [64,109] and predicts [320,280].
A regression checks both positions and the immediate destination row address.

Thus the immediate path accounts for 40 pixels of the earlier 41-pixel discrepancy.
The recording shows [320,321]. Its active display branch and actual port origin remain unverified.
A one-pixel capture or port translation is a possibility, not a recovered rule.
Do not add a one-pixel correction without more evidence.
Live framing integration remains pending. Hosting remains stopped.


## Live compact immediate-display framing

First-person rendering now selects the recovered compact layout for a zero-origin 640-by-480 reference screen.
The host scales that screen uniformly by 1.25 to the existing 800-by-600 framebuffer.
It composes display_rectangle and viewport_layout to obtain the surface bounds and projection center.
The original projection scale is 256 reference pixels, replacing the provisional 500 host pixels.
The resulting host focal length is 320 pixels and the projection center is [400,400].

Packed solids, ground points, horizon bands, and aiming marks use the same scale and center.
Unsupported meshes, shots, and impacts use that framing with their existing preview projection arithmetic.
Ground-point and solid-origin culling now select their compact-mode branches.
Pixel and depth writes respect the recovered surface bounds.
The host rounds scaled rectangle edges upward and treats bottom/right edges as exclusive.
This edge rule is a host scaling choice, not an original Macintosh rasterizer claim.
The clip is released after rendering so later overlays and other views remain independent.

Regression checks cover the compact margins, released clipping, aiming marks, horizon palette, and face visibility at the new positions.
The palette verifier samples the same authored tower and tetrahedron in their newly projected locations.
All 86 Rust tests, strict Clippy, WASM, 112 arena checks, native/WASM movement replays, death transition, and 16 palette scenarios pass.
The updated offline frame was inspected. Original arithmetic helpers are unchanged; their last 173,100-vector run remains applicable.

The adapter explicitly selects compact immediate presentation; it does not establish the recording's active mode.
Full-display selection, original instruments and radar, port-origin configuration, and final original rasterization remain pending.
No one-pixel video correction is applied. Hosting remains stopped.


## Original radar projection and edge markers

Rust radar_point reproduces complete CODE 6 +0x342.
It accepts packed planar offsets, a cached distance, the radar center, and three scale parameters.
A cached distance of minus one requests the high word of the wrapped sum of squared offsets.
The initial projection scale is 614.
Beyond the supplied threshold, the routine reduces that scale using the distance excess and falloff factor.
The reduction uses a logical right shift; signed products and low-word truncation remain explicit.
Each output coordinate subtracts the high word of its scaled offset from the corresponding radar center.

Rust radar_edge_points reproduces complete CODE 6 +0xc4.
It emits three pixels at each edge: left, right, top, then bottom, 60 pixels from the radar center.
Both helpers preserve wrapping coordinate behavior.
The harness executes 2,048 complete original projections and 1,024 complete marker initializations without service fixtures.
Cases cover computed and supplied distances, scale thresholds, arbitrary scale parameters, and signed boundaries.
All 176,172 differential vectors, 86 Rust tests, strict Clippy, and WASM pass.

The live radar is not connected yet. Original scale initialization, per-object selection, glyphs, and colors remain separate integration work.
CODE 33 +0x2b4..+0x308 initializes the scale parameters from arena dimensions and fixed constants.
CODE 6 +0x1f4 loads PICT resource 1000 and draws it into a monochrome bitmap.
Its +0x29c..+0x320 loop extracts 41 glyphs through GetPixel calls.
Each glyph samples four columns and five rows, retaining offsets for set pixels.
The owned resource is present at .local/game-resources/50494354/1000.bin and is 173 bytes long.
CODE 6 +0x17f6 projects each selected object, then appends the selected glyph offsets to its color queue.
A timed random-jitter branch in that glyph routine remains unported; it must not be confused with movement randomness.
Hosting remains stopped.


## Original radar glyph atlas decoding and extraction

The new radar module decodes the owned PICT 1000 monochrome PackBitsRect layout.
The picture contains five rows with 22 decoded bytes per row.
Its source rectangle selects 164 pixels, starting six pixels into each bitmap row.
The decoder validates picture length, version, clipping rectangle, bitmap bounds, source/destination rectangles, transfer mode, and final opcode.
It rejects other PICT encodings; this is not a general PICT decoder.
Packed row output is bounded, and truncated rows or rows with incorrect decoded lengths are rejected.

The glyph extractor reproduces CODE 6 +0x29c..+0x320 sampling order.
It scans four columns before five rows for each of 41 glyphs.
Set pixels retain signed offsets X minus one and Y minus two.
The owned atlas contains digits, letters, and five additional marker shapes.
Its largest glyph contains thirteen points, within the original record capacity.

The harness executes 257 original extraction loops, including the decoded owned atlas and 256 synthetic atlases.
GetPixel is modeled as a lookup in the supplied bitmap; QuickDraw picture drawing is not executed.
Every glyph's count, ordered offsets, and stack balance match Rust.
Synthetic glyphs contain at most fifteen points to stay within the original 32-byte record capacity.
The native extractor uses vectors; it does not reproduce out-of-bounds record writes for unsupported dense input.
The verifier also compares the Rust PICT decoder with the independently decoded atlas and rejects every truncated prefix of the owned resource.

All 176,429 differential vectors, 86 Rust tests, strict Clippy, and WASM pass.
The glyph loader is available in the core library. Resource-catalog loading and live radar drawing remain pending.
Hosting remains stopped.


## Radar atlas resource loading in desktop and WASM

The shared executable-resource loader now retains PICT 1000 alongside clut and cob8 resources.
The catalog decodes that resource with the bounded radar atlas decoder.
A present but malformed or unsupported atlas returns an explicit loading error.
An absent atlas remains optional for campaign-only inspection.
Campaign resources retain precedence through the existing ordered resource merge.

Wave setup copies the decoded glyphs into Game. Reset preserves them with the existing loaded resource state.
Desktop and WASM diagnostics expose radar_glyph_count for integration checks.
The existing --colors option also loads these glyphs; its command-line interface remains compatible.
The palette integration test now checks all 41 glyphs across sixteen campaign/level scenarios, resets, and native/WASM parity.
All 86 Rust tests, strict Clippy, WASM, 112 arena checks, movement replays, and 476 campaign checks pass.
Eighteen native/WASM campaign setups also match. The renderer is unchanged by this resource-loading step.

Object selection still requires further recovery before radar drawing is connected.
CODE 6 +0x1dbe..+0x1e1c selects glyph 41 for the selected participant, glyph 39 in one local-session branch, or a participant-derived glyph.
The surrounding visibility gates and network/session state remain necessary context; this is not a universal enemy-symbol rule.
CODE 6 +0x41fc selects the center marker's color from kind-color entries six or seven, according to participant selection.
The edge-marker color remains special binding entry one. None of these observations authorize a guessed color scheme for all contacts.
Hosting remains stopped.


## Enemy radar marker selection

Rust radar::enemy_glyph reproduces CODE 6 +0x1e86..+0x1f44 for its dispatched enemy types.
The original jump table at +0x1be2 routes types 7, 8, 13, 20, 22, 26, 27, 31, 35, and 45 through this branch.
The harness reads those table words directly and verifies the supported set.
The branch emits no radar marker when the view selector is at least two or the radar color is thirteen.
Types 22, 13, and 20 select glyphs 18, 12, and 17, respectively.
Other supported types select glyph 39, except type 31 must first pass the original final-animation-frame gate.
This independently confirms that type 31's final frame controls radar visibility as well as vulnerability.

The harness executes 2,048 original selection blocks and complete CODE 6 +0x197e animation gates.
Only the terminal glyph-drawing request is captured.
It compares glyph IDs, unchanged color and position arguments, the accompanying height copy, preserved source records, and stack balance.
Tests vary view selectors, colors, signed animation bytes, kinds, and coordinate arguments.
All 178,477 differential vectors, 86 Rust tests, strict Clippy, and WASM pass.

CODE 6 +0x89c..+0x8a0 fills A5-0x3b30 from each kind binding's second byte.
Thus this branch uses kinds[kind][1] for radar color, while solid rendering uses the first byte.
Earlier participant visibility gates and other object dispatch branches remain separate.
The helper returns no marker for unsupported types; this is a host boundary, not their recovered original behavior.
Live radar integration still needs scale initialization and those caller conditions. Hosting remains stopped.


## Radar scale initialization and live enemy contacts

Rust radar::scale_for_arena reproduces CODE 33 +0x284..+0x30c.
It retains word wrapping in the expanded arena extent and longword wrapping in squared-distance calculations.
The harness executes 1,024 original initialization blocks with original CODE 36 multiplication and division.
All seven output globals and stack balance match Rust without service fixtures.
The host returns None for a zero divisor; original zero-divisor behavior is outside this comparison.
For the current 1024-unit arena, outputs include threshold 156, scale base 32767, and falloff 1688.

First-person rendering now draws the recovered radar edge pixels, center glyph 41, and contacts for the ten supported enemy types.
It uses the owned glyph atlas and original palette bindings.
Enemy radar colors come from the second kind-color byte. The center marker uses kind six's first color byte.
Contact positions use recovered planar camera coordinates and nonlinear radar projection.
The compact display surface positions the radar. Original pixels become rounded host pixel positions at the existing uniform scale.
The renderer reuses its initialized direction table.

The live adapter includes living, non-retired targets and uses their existing type-31 vulnerability state for the final-frame gate.
Earlier original participant visibility gates, radar jitter, other object branches, pickups, scenery, and exterior radar integration remain pending.
The center marker is suppressed during the current death camera adapter.
This is a partial playable radar, not a complete original object-selection pipeline.

A rendered regression checks contact placement, lateral movement, removal on death, and type-31 phase suppression/restoration.
The offline campaign frame was inspected and contains original edge markers, a player marker, and enemy contacts.
All 179,501 differential vectors, 87 Rust tests, strict Clippy, WASM, 112 arena checks, movement replays, death transition, and 16 palette scenarios pass.
The first differential replay encountered a Rust type-inference compilation error; explicit integer typing fixed it, and replay of the saved original vectors passed.
Hosting remains stopped.

## Radar object branches — CODE 6 +2322 and +241a

The dispatch table routes kinds 19, 36, and 37 to +2322.
These select glyphs 30 (T), 31 (U), and 14 (D), respectively.
Kinds 16, 17, 18, 23, 24, 39, and 40 enter +241a.
Kind 23 selects glyph 11 (A), and kind 24 selects glyph 23 (M).
The remaining kinds in this branch select glyph 39.

Both branches require a signed view value below 2 and a radar color other than 13.
The terminal call preserves the supplied distance, horizontal position, depth, and radar color.
The first branch also copies object height. The second clears the local byte at A6-190e.
Both copy the kind's solid color into A6-35.

The oracle executes 17,920 original branches across all ten kinds, all 256 colors, and seven signed view values.
It checks dispatch entries, glyph requests, local writes, source preservation, and stack balance.
Only the terminal glyph drawing call is captured.
All 197,421 differential vectors match Rust. The 87 workspace tests and strict Clippy pass.

The first-person adapter now draws these contacts from current scene items.
A rendering regression checks position, color-13 suppression, and exclusion of the separate flag branch.
Earlier shared visibility gates, flag visibility, and exterior radar remain incomplete.

## Single-player flag radar and flashing — CODE 6

The flag branch at +24be requires view below 2 and the radar-enabled byte.
The caller derives that byte from the kind's radar color: color 13 disables it.
In session mode zero, the branch emits glyph 40 only when A5-3991 is nonzero.
The glyph request retains the kind-12 radar color and supplied projection arguments.
Other session modes use participant visibility and ownership rules. Those remain pending.

Initialization at +47c6 sets the flash byte to 1 and its deadline to current time plus 25.
The update at +5082 compares current time with that deadline as signed longs.
When due, it XORs the flash byte with 1 and sets the deadline to current time plus 25.
It performs one toggle even after a long delay. Arithmetic wraps at 32 bits.

The oracle executes 2,048 single-player flag branches and 2,048 flash updates.
It captures terminal glyph requests and checks stack balance, output values, and original timing arithmetic.
All 201,517 differential vectors match Rust.

The live adapter updates flashing in the fixed-step loop and initializes it on game reset.
The original invokes the update during display processing. Pause, redraw, and display initialization scheduling remain provisional.
A rendering regression covers visibility through ticks 24, 25, and 50, collected kind 14, and reset.
All 87 Rust tests, strict Clippy, WASM compilation, palette scenarios, and death-transition checks pass.

## Complete radar glyph emission — CODE 6 +17f6

The oracle now executes the complete glyph routine, including its call to the original radar projection.
It compares 2,048 cases against Rust projection and glyph expansion.
Cases cover every glyph slot, thirteen colors, arbitrary signed offsets, and existing queue entries.
The checks include stack balance, glyph preservation, queue counts, ordered points, and writes outside the selected queue.

When A5-12ef is zero, the routine consumes two Random words.
Each axis receives the signed remainder of the wrapping absolute value divided by 21, minus 10.
The word -32768 retains its sign during negation and therefore produces displacement -18.
Normal random words produce offsets from -10 through 10.
The routine applies both offsets before comparing current time with the disturbance deadline.
At or beyond that deadline, it sets the stable byte to 1.
Thus the contact that reaches the deadline still receives disturbance; subsequent contacts do not.

The oracle supplies Random trap results. Projection, arithmetic, deadline handling, and queue writes execute original instructions.
This adds 2,048 cases to the differential suite, bringing the total to 203,565.
The legacy modeled-trap subtotal printed by the harness does not include every later fixture.

CODE 6 +9b6 drains color queues in ascending order from 0 through 12.
The live radar now follows that order and uses recovered glyph expansion.
A rendering regression proves that higher color indices win overlapping contacts regardless of object order.
The live adapter uses the stable branch. Gameplay activation and recovery scheduling for radar disturbance remain pending.
Original fixed queue capacity is replaced by dynamic vectors; the oracle stays within the original capacity.

## Radar disturbance activation — CODE 31 +c4c..+caa

The damage handler enters this block after accepted damage and health subtraction.
It requires the victim to be the local participant and the participant's signed health word to be at most 2.
Non-local victims and health above 2 preserve the previous disturbance state and deadline.

The block issues sound requests (1, 0x1fc1c) and (1, 0x20009), then clears A5-12ef.
The first request has a negative low word; it is not a PCM resource ID.
The second request maps to sound resource 10009 through the recovered sound dispatcher.
Sound queue semantics remain separate from this trigger.

The block calls CODE 1 +890, which reads the selected difficulty at A5-4258.
This differs from the effective difficulty at A5-4256, read by CODE 1 +882.
The initializer at CODE 1 +7fa stores the selected value.
CODE 18 loads that value from preferences or initializes it.

The deadline is current time plus the sign-extended low word of difficulty times 60.
For supported difficulty values 0 through 7, the duration is 0 through 420 ticks.
The zero-duration case still disturbs the next emitted glyph before the glyph routine restores stability.
Repeated accepted low-health hits replace the deadline.

Rust radar::damage_disturbance returns the deadline, cleared stable byte, and both sound requests.
The oracle executes 2,048 original blocks and the original difficulty getter.
It captures only sound service calls.
Cases cover local and remote victims, signed health boundaries, difficulty overflow, and time overflow.
It also checks participant-record preservation, unchanged state on rejected triggers, and stack balance.
All 205,613 differential vectors, 87 workspace tests, strict Clippy, and WASM compilation pass.

The trigger is not yet connected to the live damage adapter.
Integration must preserve per-contact expiry, reset behavior, and sound queue semantics.
Stability writes also occur at CODE 33 +670, CODE 31 +13e4 and +1cec, and CODE 25 +2406.
The latter two follow player health/ammo restoration. Their enclosing control flow still needs integration review.

## Live low-health radar disturbance

Accepted enemy-shot damage now invokes the recovered disturbance trigger after health subtraction.
The viewer passes selected campaign difficulty into the game; standalone arenas use zero.
The adapter emits warning sample 10009 through the existing audio event path.
The original negative sound-control request remains unsupported by that PCM event path.

DisplayState holds the stable byte, deadline, and a provisional deterministic random stream.
It supplies two random words only when the radar is disturbed.
Glyph emission then uses the verified original arithmetic and per-contact expiry rule.
The display state persists across redraws through a RefCell owned by the game.
Movement and spawn random streams remain independent.
This separation supports deterministic simulation, but does not reproduce the original shared Mac OS random stream.

First-person rendering emits world contacts before the center player glyph, following CODE 6 +41fc.
Current scene and target traversal still differs from the full original object scheduler.
The center glyph can receive disturbance; the twelve edge marks cannot.
The radar restores stability on successful respawn, campaign entry refill, and game reset.
Exterior radar and other original restoration paths remain pending.

Tests cover actual enemy-hit activation, warning output, selected-difficulty deadline, and visible pixel changes.
They also cover per-contact expiry, repeated redraws, unchanged movement/spawn state, reset, and respawn.
All 88 Rust tests, strict Clippy, native/WASM builds, sixteen palette scenarios, and WASM death-transition checks pass.
The previously verified 205,613 original-code comparisons cover the unchanged arithmetic helpers.

## Moving acid and mud phase recovery — CODE 20

The original kind mask before +14be contains exactly kinds 23 and 24.
Their tracking branch at +12f8 uses the AI subrecord deadline at +46.
A deadline of -1 becomes current time.
If current time has reached the deadline, the branch sets behavior 5 and movement mode 3.
It then stores current time plus 50 as the deadline.
Before the deadline, it preserves behavior and movement.
Rust ai_surface_phase reproduces these writes with wrapping long arithmetic.

The behavior dispatcher routes behavior 5 to +fb8.
Rust ai_cooldown reproduces that complete handler.
Before its deadline, the handler changes nothing.
When due, it clears behavior and movement and stores current time plus 20.
For kind 23, it also checks the local death byte with XOR 1.
If the result is nonzero, distance at most 400 fixed-point units selects sound request 0x10014.
Greater distance selects request 0x10013. Both use channel argument 1.
Kind 24 has no sound request in this handler.

The movement dispatcher treats mode 3 as no movement command and mode 0 as ordinary movement, subject to predecessor yielding.
These deadlines therefore contribute to intermittent surface-enemy motion.
They do not prove an exact wall-clock cycle: intent polling, suppression, and target tracking also control handler entry.
The manual's moving acid and intelligent Cybermud descriptions are consistent with these branches.

The oracle executes 2,048 complete +fb8 routines, capturing only terminal sound requests.
It compares changed fields and preservation of every other byte across all twelve participant records.
An additional 1,024 cases execute the original +12f8 phase block without fixtures.
The original kind mask, signed timing boundaries, sentinel deadline, wrapping additions, and stack balance are checked.
All 208,685 differential vectors, 88 Rust tests, strict Clippy, and WASM compilation pass.

These helpers are not yet active for live kinds 23 and 24.
The live enemy loop still admits only kind 7.
Remaining work includes their full tracking path, movement integration, placement ownership, and damaging surface contacts.

## Complete surface-enemy initialization and tracking

Rust ai_initialize_surface reproduces the mode-zero initializer at CODE 20 +b6c for kinds 23 and 24.
These kinds skip the object-selection call used by kind 7.
A nonzero acquired opponent becomes the tracking target and aim target.
The initializer copies the cached target pose and clears the heading deadline to -1.
With no opponent, it selects behavior 2.
Both paths set the intent timer to -1 and movement mode to 3.

Rust ai_surface_tracking composes the complete kind-23/24 path through CODE 20 +1240.
The prefix matches the previously recovered kind-7 handler.
If the counter is at least five, it requests a replacement target.
The resulting attack writes can affect the local participant rather than the participant being updated.
Mode-3 movement becomes zero and copies the current target into the aim target.
An inactive participant in a nonzero game mode switches to behavior 3, clears its target, and skips pose acquisition.
Otherwise, the handler copies the cached pose, clears the heading deadline, and applies the recovered surface phase.

The initializer oracle executes 800 complete original routines.
It supplies opponent, distance, and cached-pose services, and verifies that object selection is never called.
The tracking oracle executes 2,048 complete original routines.
It supplies target selection and cached pose, while original instructions perform all participant writes and phase logic.
The checks compare service calls, their observed intent state, all twelve participant records, phase deadlines, and stack balance.

CODE 30 +1370 dispatches kinds 23 and 24 to +17e6, the same movement branch as kind 7.
This establishes a path for live integration using the recovered movement arithmetic.
It does not establish identical parameters, collision responses, firing rules, or surface damage.
The live encounter still admits only kind 7; surface-enemy activation remains pending.

All 211,533 differential vectors, 88 Rust tests, strict Clippy, and WASM compilation pass after this recovery.

## Live moving acid and mud

The encounter adapter now admits kinds 23 and 24 alongside kind 7.
Surface enemies skip the object search and initialize through ai_initialize_surface.
Tracking composes the verified common tail and surface phase; the single-player adapter keeps enemy retarget counters at zero.
Behavior 5 invokes ai_cooldown and emits the recovered acid sound request.
Per-kind resource parameters supply radial velocity, angular velocity, range, and weapon settings.

Movement uses the shared original +17e6 arithmetic and existing steering/clearance adapter.
The generic recovered enemy firing predicate replaces the type-7-only wrapper for these supported kinds.
Weapon admission still respects the original -1 disabled setting.
The existing projectile adapter retains its documented approximations.

EnemyNavigation now stores the shared phase deadline.
Reset and respawn restore its initial -1 value.
The player-death AI reset copies that field from reset_ai_participant, matching CODE 20 +2a72.
Moving targets use the recovered A/M radar glyphs through the target emission path.

A Rust integration test runs both kinds through pause, pursuit, and another pause.
It checks forward progress, phase suppression of translation, acid sound, disabled weapons, and phase reset.
The authored WASM test finds kind 24 in Central level 12 and kind 23 in level 13 at difficulty 5.
It observes movement and phase 5 over 240 ticks, then checks full level reset.
Run it with npm run test:surface-enemies.

All 89 Rust tests, strict Clippy, native/WASM builds, 112 arena checks, and 476 campaign checks pass.
Encounter replay parity, death transition, and sixteen palette scenarios also pass.
The unchanged arithmetic helpers retain their 211,533-vector differential verification.
Moving surface contact damage/slowdown, exact collision response, and complete original scheduling remain unfinished.

## Acid contact requests and live moving-mud slowdown

Rust acid_contact reproduces complete CODE 7 +1096 through +1116.
If current time is less than or equal to A5-3d1c, it issues no requests and preserves the deadline.
Otherwise, it requests sound 0x1001d on channel 1 and sets the deadline to current time plus 150.
The deadline advances before the damage service runs.
Thus a rejected damage event can still consume the contact cooldown.

The handler reads the damage word from resource state +14a.
It reads the attacker from the contacted geometry record's owner byte at +1a.
In game mode zero, it calls CODE 31 +a84 with damage, zero death flag, current time, attacker, and victim.
Other modes call CODE 27 +2e6 without the death flag.
The helper returns these request values; it does not mutate health or implement networking.

The oracle executes 2,048 complete original handlers.
Only terminal sound and local/network damage services are captured.
It checks strict deadline boundaries, clock wrap, signed damage/owner forwarding, geometry preservation, request order, and stack balance.
All 213,581 differential vectors match Rust.

The live contact adapter now treats kind 24 as mud and allows its footprint to be crossed when recovered geometry is available.
The original contact switch sends kinds 18 and 24 to the same +1360 handler.
The integration test places a target-backed kind-24 actor under the tank.
It checks slowdown without blocking, then moves the actor away and checks normal acceleration.
The default adjustment remains 5; original resource/event overrides are not yet connected.

All 89 Rust tests, strict Clippy, native/WASM builds, 112 arena checks, encounter parity, and death checks pass.
Authored surface-enemy tests still pass for Central levels 12 and 13.
Live acid damage, contact-owner routing, surface audio cooldowns, and complete contact scheduling remain pending.

## Live acid damage and resource linkage

The parameter loader stores collision-damage words at runtime offset +11c, indexed by kind.
The acid handler's fixed +14a read therefore selects kind 23: 0x11c + 23 * 2 = 0x14a.
Both static acid and moving acid use this value.
Catalog population now copies it from the resolved active wave parameters.
It retains original defaults when the wave omits that row.
Later event handlers that double or clear the parameter remain unconnected.

The live adapter admits kinds 16 and 23 as passable acid surfaces when recovered geometry is available.
It maps scenery contacts to owner zero and target contacts to the existing participant slots 2 through 12.
This mapping uses the query's current record order; original ownership scheduling remains provisional.
The contact deadline starts at zero in the normalized arena clock, matching CODE 25 +2cdc's current-time initialization.

After each fixed-step movement update, acid_contact can emit its sound and damage request.
The deadline advances even if the existing damage grace rejects the request.
Accepted damage uses recovered signed health subtraction, counters, low-health radar disturbance, and damage sounds.
Lethal acid invokes the same local death notification/reset and camera transition as enemy shots.
Research mode does not apply acid damage. Network damage remains unimplemented.

The contact regression now covers both scenery-backed kind 16 and target-backed kind 23.
With damage 3 and health 10, it checks the first hit, strict deadline equality, repeated hits, damage counters, sound 10029, and death.
It also checks passability and arena-reset deadline initialization.
The shared death adapter was extracted without changing enemy-shot behavior.

All 89 Rust tests, strict Clippy, native/WASM builds, authored surface-enemy checks, encounter parity, death transition, and palette checks pass.
The 213,581-vector differential suite covers the unchanged recovered arithmetic and request helpers.
Exact update ordering, unsupported contact geometry, complete sound queue controls, and parameter-changing events remain unfinished.

## Complete ice/mud handlers and live contact audio

Rust handling_contact reproduces CODE 7 +132a and +1360.
Ice writes adjustment -6 and requests sound 0x10024 when current time reaches its sound deadline.
It then advances the deadline by 6000 ticks.
Mud writes the supplied signed adjustment and requests sound 0x10023 when its separate deadline is reached.
It advances that deadline by 60 ticks.
Both comparisons include equality, unlike the acid handler.
Kinds 18 and 24 share the mud handler.

The oracle executes 2,048 complete original routines.
Only terminal sound playback is captured.
It checks signed adjustment values, deadline equality, clock overflow, sound arguments, preservation of the other clock, and stack balance.
All 215,629 differential vectors match Rust.

The live adapter now uses these handlers for both adjustment and audio.
It preserves independent ice and mud clocks across ticks.
SpeedResult::reset_timestamps resets both clocks to current time, reproducing CODE 30 +1238/+123e.
That condition requires adjustment zero and mode zero.
Thus ordinary movement can re-enable a sound before the surface's long cooldown expires.
Game reset restores both normalized clocks to zero.

Integration checks cover the initial ice sound, suppression during continuous contact, and both clocks after exit.
They also cover mud's 60-tick repetition and immediate sound after leaving and re-entering.
All 89 Rust tests, strict Clippy, native/WASM builds, 112 arenas, encounter parity, death, surface-enemy, and audio checks pass.
The audio check preserves all 105 decoded original samples.
Browser playback was not manually checked; hosting remains stopped.
Exact contact scheduling and mud-strength event overrides remain unfinished.

## Ammo dump contact and refill blocks

CODE 7 +19de..1a06 calls CODE 31 +1170 for a local participant touching kind 5.
The handler checks kind 5 again before it changes state.
The object's word at +1c selects its subtype.

- Subtype 0 adds 20 ammunition and one health point, each capped at the configured capacity.
- Subtype 1 restores ammunition and health to their configured capacities.
- Subtype 2 also restores both capacities, then enters an additional effect path.
- Other subtypes use the gradual refill arithmetic, then test the participant's vertical state for an additional energy effect.

The arithmetic uses signed 16-bit addition before the signed minimum operation.
The health capacity is the shield setting capped at nine. The ammunition capacity is ten times the signed ammunition setting.
Rust `dump_refill` implements only these refill blocks. Subtype selection and additional effects remain outside this helper.

Subtype 0 requests sound 0x1000b. Subtype 1 requests 0x1001e.
The common prefix also requests 0x1fc1c and increments runtime word +6a8.
These observations come from instruction tracing, separate from the arithmetic differential tests.

The local-participant tail selects a new subtype through CODE 25 +0224.
It generates new coordinates and tests candidate placement through CODE 31 +10f6.
The loop permits 51 candidates before it accepts the final candidate without another collision test.
CODE 27 +05f6 receives the resulting position.
The complete placement routine and its random inputs still need differential verification.
Live pickup integration remains pending. A stationary refill pad would omit this original relocation behavior.

The harness executes 1,024 original refill blocks with original capacity and signed-minimum helpers.
All other participant bytes and the stack remain unchanged.
All 216,653 differential vectors match Rust. These tests do not cover the complete pickup handler.
All 89 Rust tests, strict Clippy, and the WASM build pass.

## Dump subtype selection and relocation verification

Rust `dump_subtype` matches the complete CODE 25 +0224 routine with a supplied Mac Random word.
It takes the signed remainder of the wrapping absolute word, modulo ten, then adds one.
Three signed weight bytes define cumulative thresholds for subtypes 0, 1, and 2. All remaining rolls select subtype 3.
CODE 25 +1654 initializes the weights to 5, 2, and 1.
The resource loader at +1876 and an event at +31ec can replace these values.
The default weights are therefore not universal campaign probabilities.

Rust `relocate_dump` matches CODE 31 +1416..14b8 with supplied random words and collision results.
Each candidate gets two coordinates from the signed remainder modulo the wrapped arena extent minus 200.
The routine adds 100 and converts each coordinate to 16.16 fixed point. It preserves the third pose component.
The first 50 candidates use the collision service. Candidate 51 bypasses that service and becomes the final position.
The terminal position service receives the dump identity and selected pose.
The helper also preserves DIVS quotient-overflow behavior: SWAP exposes the unchanged dividend's high word.
A zero divisor returns None in Rust. The original has no valid result for that input.

The harness executes 2,048 complete subtype routines and 256 relocation loops.
It compares every collision query, consumed random word, final position, and stack balance.
The collision predicate and OS Random generator are supplied services, not recovered by these tests.
All 218,957 saved differential vectors match Rust after the division-overflow correction.
The saved replay result is in `.local/disassembly/verify-rust-latest.log`.
All 89 Rust tests, strict Clippy, and the WASM build pass.
Live pickups, special effects, and resource-driven weight integration remain pending.

## Complete pickup effect dispatch

Rust `dump_effect` implements CODE 31 +11d6..13da, before relocation and after the common pickup prefix.
All subtype branches use the original capacity arithmetic from `dump_refill`.
Subtype 0 requests sound 0x1000b. Subtype 1 requests sound 0x1001e.
Subtype 2 fully refills the craft and requests weapon 7, then sound 0x1001a.
The local route supplies source kind 32, the current timestamp, dump position, local owner, and the collector's integer height.
The network route supplies weapon 7, dump position, local owner, and height.
The height conversion uses signed division by 65536, truncated toward zero.
The helper returns the route and height. Caller-owned position, timestamp, and owner remain integration inputs.

All other subtype values use gradual refill arithmetic.
If the collector's vertical value is nonnegative, the routine adds 400 to the local participant's energy word and caps it at 2000.
It then requests sound 0x10010. Negative vertical values skip both the energy change and this sound.
The energy addition wraps as a signed word before the cap.
The energy recipient remains the local participant, even when the supplied collector identity differs.

The manual's printed page 19 describes the orange pickup as a directional invulnerability shield with a visible battering ram.
This description is consistent with the executable's persistent weapon-7/type-32 object path.
It does not imply a global invulnerability timer or an additional health increment.
The live port still needs shield-object behavior and flight-energy state before these effects can function completely.

The harness executes 2,048 original effect-dispatch blocks with original getters, minimum, and division helpers.
Only terminal sound and weapon-service calls are captured.
Checks cover packet constants, local/network routing, local energy ownership, untouched records, and stack balance.
All 221,005 differential vectors match Rust. All 89 Rust tests, strict Clippy, and the WASM build pass.
Pickup creation, resource weights, collision integration, and special-effect objects remain pending.

## Resource-driven pickup weights in live arena state

`dump_weights` implements the default and type-5 override branches of the original bval loader.
The loader interprets the low bytes of four type-5 words as signed weights, not enemy health or movement parameters.
The words occupy the hits-to-kill, collision-damage, attack-range, and short-range positions in an ordinary enemy row.
Later type-5 rows replace earlier rows. Type matching uses the low signed byte, as the original loader does.

The complete loader oracle checks these four globals in all 122 existing cases.
These cases include 22 owned resource tables and 100 synthetic tables with duplicate and aliased type IDs.
All 221,005 vectors still match Rust. The vector count does not increase because these are additional assertions on existing executions.

The arena loader stores the selected weights in `Game::dump_weights`.
Native and WASM setup/frame output expose them for inspection. Reset preserves them.
The campaign verifier reads the original bval bytes independently and checks each selected arena's weights.
All 476 campaign cases pass, including reset, and 18 native/WASM setups match.
All 89 Rust tests, strict Clippy, and the WASM build pass.

The six central tables use four weight sets: 4/2/1/3, 3/3/1/3, 5/2/1/2, and 4/3/1/2.
The ten eastern tables also contain 7/2/1/0 and 6/3/1/0.
The six southern tables use only 7/2/1/0 and 6/3/1/0.
The picker reads only the first three weights. A residual roll selects subtype 3.
Special-event overrides and the signed Random edge case remain separate from these ordinary weight distributions.

The creation trace at CODE 25 +0cba..0cc6 requests six dumps unless mode equals 2.
CODE 25 +04d8 limits the shared object count to 50 before calling +029c for each new dump.
Initial creation uses its own placement loop and random heading. It does not reuse the later relocation loop unchanged.
That creation path remains the next integration target. Loading weights alone does not create live pickups.

## Complete initial dump creation

`create_dump_record` implements CODE 25 +029c..0404 for ordinary nonnegative object counts.
It rejects creation when the shared object count is already 50 or greater.
For an available slot, it sets the owner to zero, subtype to zero, and requested kind before placement.

Each position uses two supplied Random words and the original signed coordinate arithmetic.
The routine checks the candidate against every existing object until it finds a nearby object.
CODE 25 +0154 compares the signed high word of the wrapping sum of absolute coordinate differences against 40.
This is a Manhattan-distance test, not polygon overlap or Euclidean distance.
A nearby object causes another position attempt. Initial placement has no original retry limit.
The host helper returns None if its Random source ends or the extent gives a zero divisor.
It does not report a failed placement as a created object.

After placement, the routine chooses a random heading modulo 360 and converts it to 16.16 fixed point.
It writes the original timestamps, two-billion lifetime value, motion fields, callback, and initialization flags.
Kind 5 then calls the recovered subtype selector. Other kinds retain subtype zero.
All unspecified record bytes remain unchanged.
The caller increments the shared object count only after successful creation.

The harness executes 512 complete original creation calls.
It supplies only OS Random words and runs the original proximity, subtype, and long-multiplication routines.
Cases include capacity rejection, repeated rejected positions, several extents, type-5 and other kinds, and randomized record contents.
Checks compare every output record byte, all consumed Random words, other records, count updates, and stack balance.
All 221,517 differential vectors match Rust. All 89 Rust tests, strict Clippy, and the WASM build pass.

The helper is not yet connected to live scene objects.
Integration must preserve original object identity and placement records used by collision and radar queries.
The existing Item path can supply the type-5 shape, but subtype colors and animation also need connection.
Shield objects and flight state remain necessary for complete special-pickup behavior.

## Live pickup creation and scene integration

Campaign and mission startup now create up to six pickup records with `create_dump_record`.
A separate provisional xorshift stream supplies Random words. It does not alter the existing movement or enemy-spawn stream.
The adapter supplies scenery positions plus the preview player's initial position as the occupied object list.
Original startup identity, order, shared Random state, and player placement remain provisional.
A finite host random budget makes pathological placement fail explicitly instead of freezing startup.

`Game::pickups` retains the original record, subtype, and scene item separately from enemies and flags.
Pickup creation does not change enemy counts, completion bonuses, or flag-placement slots.
Reset restores the captured initial pickups. Native and WASM state output expose each pickup's subtype and pose.
All three render paths include these items. Their static mesh uses procedural selector 6.

The first offline inspection found that selector 6 was unresolved.
The new base mesh traces CODE 19 +37e8..382c: four vertices at horizontal coordinates plus or minus nine, at height zero.
The double-sided faces trace +3984..39bc and retain color selector 13.
This mesh is instruction-traced, not yet covered by a complete builder differential test.
Original subtype colors, animated deformation, radar markers, and collision-model loading remain pending.
Pickups are visible scene objects but are not collectible yet.

All 89 Rust tests, strict Clippy, and the WASM build pass.
All 476 campaign cases pass with pickup subtype and proximity assertions, and 18 native/WASM setups match.
`npm run test:pickups` checks six procedural pickups, view-switch state preservation, and reset in the opening Central arena.
Offline first-person and overhead frames were inspected in `.local/pickups-view-0.png` and `.local/pickups-view-2.png`.
Hosting remains stopped.

## Live pickup contact, refill, and relocation

Pickup records now participate in the recovered spatial-grid contact query.
They follow enemy records in the adapter, so existing enemy indices remain unchanged.
The model loader accepts type 5 with procedural selector 6 and uses its traced static geometry.
The actor footprint remains passable. The projectile filter continues to exclude type 5.

The fixed-step contact adapter calls `dump_effect` for the selected pickup.
It updates ammunition, health, the pickup counter, and flight-energy state, and queues the original sound requests.
The supported player currently has vertical value zero. Flight movement and energy consumption remain unfinished.
Orange pickups retain a `pending_shields` request with original position, height, and timestamp.
That request is not a protective shield object. Shield rendering, movement, and damage interception remain unfinished.

Collection chooses the next subtype before relocation, as the original tail does.
The relocation helper tests candidates against current object positions with the recovered 108-unit Manhattan predicate.
It updates both the scene item and raw pickup record while preserving heading.
Object order, dead-record representation, contact scheduling, and the independent xorshift stream remain provisional adapters.
Restart restores initial pickup records and the post-creation random state, and clears pickup-derived state.

A Rust integration test exercises all four subtypes through the actual contact query.
It checks refill values, sounds, energy, pending shield requests, relocation, prevention of immediate repeated collection, and reset.
The WASM pickup test fires ammunition and drives to a white dump with normal inputs, without teleporting.
It verifies a refill to 40 rounds and relocation of the collected dump.

All 90 Rust tests, strict Clippy, and the WASM build pass.
All 476 campaign checks and 18 native/WASM setups pass.
The game, encounter parity, and 260-tick death-transition checks also pass.
Original arithmetic routines are unchanged from the 221,517-vector verification.
Subtype colors, pulsing animation, radar markers, shield protection, and flight controls remain unfinished.
Hosting remains stopped.

## Pickup subtype colors and radar markers

CODE 6 +23ac..23fc selects the pickup subtype's color from A5-3b00.
It writes that color for solid rendering and uses it for radar glyph 38.
Views below two permit the radar marker. Color 13 suppresses it.
The source record remains unchanged. The branch also clears the surface presentation flag.
Rust guards subtype indices outside zero through three before this original unchecked table read.

The existing color-binding loader already preserves the four entries in `ColorBindings::effects`.
The owned cob8 resource supplies palette indices 4, 0, 5, and 9.
The renderer now uses these entries for live pickup bodies and radar markers.
Markers follow relocation and enter the existing color queues and low-health disturbance path.
Exterior and overhead pickup outlines also use the selected palette color.
Other exterior/overhead projection and color behavior remains diagnostic.

The harness executes 2,048 original presentation branches with terminal glyph drawing captured.
It checks subtype colors, hidden-color and view gates, glyph packets, local writes, source preservation, and stack balance.
All 223,565 differential vectors match Rust.
The live radar test checks subtype changes, color-13 suppression, and relocation.

A new WASM pixel check exposed an earlier omission: the separate overhead object list did not include pickups.
The list now includes them. The test verifies actual colored pixels for each subtype present in the opening arena.
The corrected offline overhead frame shows green, white, and blue dump outlines.
This supersedes the earlier claim that all three paths already drew the new objects.
All 90 Rust tests, strict Clippy, the WASM build, and the driven pickup/refill/reset check pass.
Pulsing geometry, shield protection, and flight controls remain unfinished. Hosting remains stopped.


## Pickup ground animation — CODE 19 +2276

The complete routine updates four packed vertices in two model buffers.
The caller at +5fe6 supplies the signed remainder of the current clock divided by 60.
The vertex coordinate is signed -72 times the wrapped phase plus 90, divided by 220.
For normal phases zero through 59, the packed half-width increases from 29 to 48.
These coordinates represent world half-widths from 3.625 to 6, followed by a reset.
All pickups share this animation phase.

The oracle executes 1,024 complete routines with the original CODE 12 vertex setter and CODE 36 multiplication.
Cases cover all normal phases, arbitrary signed phases, and eight positive model slots.
Both buffers, surrounding bytes, and stack balance are checked.
All 224,589 differential vectors match Rust.
The nonpositive-slot early return and enclosing clock dispatcher are instruction-traced, not new oracle cases.

The live adapter updates packed and display vertices once per simulation tick.
It preserves cached collision models, item bounds, and contact records.
The original routine does not update those cached dimensions; this does not prove every surrounding collision path ignores animated vertices.
A simulation test checks expansion, repetition after 60 ticks, and unchanged contact records and bounds.
Exact original dispatcher scheduling remains unfinished.

## Flight altitude — CODE 30 +154c..1668

The recovered `flight_height` helper covers active flight, descent, and the temporary hold branch.
Its inputs include the existing hover height, base vertical position, speed byte, vehicle setting, clock, and elapsed time.

Active flight computes a target of 200 minus absolute speed times the signed quotient 160 / setting.
It then adds `(now & 127) - 64` to the target.
When the target is at least the current height, height increases by `(21 * elapsed) / 7`.
Height then decreases by `(19 * elapsed) / 13`, with a lower bound of zero.
All multiplication wraps as signed long arithmetic before division.
The resulting height, multiplied by 65536, adds to the supplied base vertical position.
A zero setting traps in the original active branch; Rust returns None instead.

Without active flight, a positive height descends using the same subtraction.
A nonpositive height leaves the base position unchanged.
A nonzero hold byte bypasses height evolution and decrements toward zero using a signed word maximum.
The hold branch still adds the existing height to the base position.

The preceding code at +1532 derives base vertical position from the renderer-height word minus 32.
Earlier branches also handle vertical transitions and can clear hover height.
These paths must be integrated with camera state, collision queries, projectiles, and resets.
The helper is not yet connected to live flight.

The main loop at CODE 33 +da..172 also identifies flight-energy consumption.
When its pause gate is clear, it subtracts max(1, elapsed / 4), using signed word arithmetic, then clamps energy to zero.
The resulting positive-energy test sets the flight-enabled byte.
That energy block remains instruction-traced; the altitude oracle does not verify it.

The oracle executes 2,048 original altitude blocks with original long arithmetic and signed maximum routines.
It checks active, falling, and hold branches, including arbitrary signed heights, clocks, speeds, and settings.
It also checks every participant-record byte and stack balance.
All 226,637 differential vectors match Rust.

## Flight and hyperspace energy clocks — CODE 33 +da..172

Rust `energy_clock` reproduces the local resource update and flight-enabled flag.
The elapsed clock is the wrapped signed difference between the current and previous clocks.
When paused, both resource words stay unchanged. The positive-flight-energy test still runs.

When unpaused, hyperspace energy gains elapsed time, truncates to a signed word, and caps at 2000.
Flight cost is elapsed time divided by four, truncated to a signed word, then bounded below by one.
The subtraction from flight energy wraps as a signed word before clamping at zero.
Thus repeated updates with zero elapsed time still consume one unit each.
Negative input energy can wrap through subtraction; Rust preserves this original edge behavior.
The helper does not replace the caller's previous-clock value.

This block runs before the participant-update loop at +188..1d0.
A live adapter must preserve that ordering relative to pickup effects and altitude updates.
The current fixed-step adapter does not yet call the helper.

### Remaining vertical integration paths

The current collision adapter writes player height as zero in `Query::from_game`.
`Game::first_person_camera` still supplies renderer height 32 outside death transitions.
The camera conversion uses eighth-unit coordinates: renderer height 32 represents world height four.
Basic player shots also start at world height four.
Pickup effects currently receive vertical value zero.
These paths must share a recovered vertical state before flight becomes playable.
The original code also has separate submergence transitions before the recovered hover block.

The oracle executes 2,048 original energy blocks with the original signed minimum, maximum, and long division routines.
Cases cover pause states, all twelve participant slots, clock overflow, signed-word overflow, and energy depletion.
Checks include all participant bytes, the unchanged previous clock, the enabled flag, and stack balance.
All 228,685 differential vectors match Rust. The 91 Rust tests, strict Clippy, and WASM build pass.

## Shared vertical transition and hover state — CODE 30 +145a..1668

`VerticalState` and `update_vertical` now cover the preceding vertical-transition branches as well as hover.
A positive transition updates camera scale and renderer height, with an upper scale clamp of four fixed units.
Crossing that clamp restores renderer height 32 and clears the transition.
A negative transition disables hover and clears accumulated hover height before updating camera state.
Its lower scale clamp is minus twelve fixed units, with renderer height minus 96.
The comparison is strict at both limits; equality does not clear the transition.

The renderer-height increment uses the wrapped transition shifted left three bits, divided by 65536.
The height word wraps before the base vertical position is calculated.
Base position uses the wrapped renderer-height word minus 32, multiplied by 65536.
The recovered hover update then supplies the final participant vertical position.
Transition velocity does not multiply by the elapsed-clock argument in these branches.
The surrounding caller remains responsible for its scheduling.

The oracle executes 2,048 original combined blocks with original arithmetic helpers.
It checks both camera globals, hover state, the flight-enabled byte, transition clamps, hold behavior, and all participant bytes.
All 230,733 differential vectors match Rust.
A sequence test combines the recovered energy and vertical updates through takeoff, depletion, and landing.
Depletion starts descent rather than setting height immediately to zero.
All 92 Rust tests, strict Clippy, and the WASM build pass.

Live flight integration is still pending.
CODE 6 +521e..5238 adds positive hover height to the render-camera height when its suppression byte is clear.
This camera addition is instruction-traced, not covered by the combined vertical-block oracle.
The existing recovered object-pair query uses a 60-unit vertical difference gate, with type-5/type-36 exceptions and a special type-12 branch.
The current footprint-based movement blocker does not yet apply those vertical rules.
Integrating flight requires resolving that movement path, not merely lifting the camera.

## Live movement uses recovered contact geometry

The tank movement adapter now calls the recovered participant-contact query when its model slots and geometry are available.
It supplies the proposed position and heading, with a recomputed grid cell.
The query uses original rectangle dimensions, radii, model selection, traversal, and vertical overlap gates.
The new adapter method accepts proposed height; the current live caller still supplies zero.
This prepares movement for shared flight state without enabling incomplete flight behavior.

The adapter removes passable pickups, flags, and known ground surfaces from its movement-only grid.
That filtering retains the port's current policy. It is not a claim that original collision response is fully recovered.
Surface and pickup queries retain their separate complete grids.
Arena-edge checks, rejection response, cached-cell scheduling, and record order remain preview choices.
If the recovered query cannot run, movement retains its previous footprint fallback.

Adapter checks cover native obstacle orientation, clearance at the narrow side, the inclusive 60-unit vertical threshold, and clearance above it.
They also verify passable mud without a loaded mud slot and unchanged source records after proposed-height checks.
The WASM driving check still collects a white refill and relocates the dump.
All 112 arena checks, 476 campaign checks, 18 campaign native/WASM setups, encounter replay, and death-transition checks pass.
All 92 Rust tests, strict Clippy, and the WASM build pass.

The arena test initially compared a delivered collision sound with the following empty frame queue.
It now drains the transient audio queue before taking the zero-time camera-state baseline.
Camera checks still compare all simulation fields and require an empty redraw audio queue.
No recovered arithmetic changed, so the existing 230,733-vector oracle was not rerun for this adapter change.

## Live flight integration

Blue pickup energy now drives the shared vertical state in the fixed-step simulation.
Each tick updates energy before contact effects and updates altitude using the preceding speed byte.
This is explicit adapter scheduling, not a recovered original main-loop timing claim.
Pickup effects receive the current raw vertical position.

Movement and surface queries receive the same raw fixed-point height.
The unsupported-model movement fallback retains planar footprints and applies the ordinary recovered 60-unit height gate.
The flag fallback uses the original strict 140-unit threshold.
The cockpit camera adds positive hover height to the renderer height.
The diagnostic chase camera and tank mesh rise by the scene altitude.
The overhead map remains a planar view.

Ordinary player shots retain their existing four-unit display offset above the craft.
Their collision-query height uses the underlying raw altitude rather than that display offset.
This replaces the prior hardcoded raw query height of four with zero for ground shots, matching the source height passed to original weapon creation.
Unsupported projectile geometry uses the actual displayed shot height for its fallback bounds.
Enemy shot hit checks compare raw vertical separation against the original 60-unit gate.
Enemy targeting, projectile visual offsets, and bouncing-shot scene scaling still need broader recovery.

Local respawn resets vertical state while preserving remaining flight energy, consistent with CODE 31 +530..572.
The next update can lift the craft again. Full round reset clears energy and vertical state.
Death records retain player altitude, while the existing death-camera path remains separate.

`test:flight` drives to an actual blue pickup without teleporting or injecting energy.
It checks takeoff, energy consumption, camera-state preservation, elevated shots, depletion, descent, landing, and complete reset.
Offline cockpit and chase frames were inspected. Hosting remains stopped.
Rust checks cover energy-preserving respawn, camera and shot altitude, and collision-record height propagation.
All 93 Rust tests, Clippy, and the WASM build pass.
The 112-arena, 476-campaign, native/WASM replay, pickup, encounter, and death checks also pass.
The existing 230,733-vector oracle remains the evidence for the unchanged arithmetic helpers.

Original cached-cell scheduling, full landing collision response, submergence triggers, and special flight weapons remain unfinished.
The browser status line now reports flight energy and scene altitude.

## Solid-contact rebound records — CODE 7 +1be6..1d1e

Rust `contact_rebound` reproduces the record changes after the movement rollback call.
Boundary contact ID -999 skips the height adjustment.
For another contact, a positive moving height sets the hold byte at +5a to 30.
If moving height exceeds target height, the routine adds three raw fixed-point increments and clamps a negative overflow result to zero.
Otherwise, it subtracts three raw increments without a lower clamp.
These increments are not three world units.

The routine then negates words +e and +c and the signed speed byte at +22.
Negation wraps at the original word and byte widths.
All other record bytes remain unchanged.
The oracle executes 2,048 original blocks across twelve participant slots and sixteen moving-object slots.
It checks boundary handling, signed-height extremes, arithmetic overflow, and every object record.
All 232,781 differential vectors match Rust. The 93 Rust tests, Clippy, and WASM build pass.

The preceding branch at +adc..1b14 skips ordinary rebound when the moving record already has a nonzero hold byte.
Boundary contact bypasses that hold gate.
CODE 7 +1bc6..1be6 calls movement with negated elapsed time before applying rebound.
The port's current rejection of a proposed pose is not yet verified as equivalent to that rollback service.
Therefore the recovered record helper is not yet connected to live movement.

The original special-contact set at +1f5e includes kinds 5, 12, 13, 16, 17, 18, 19, 21, 23, 24, 30, 35, 36, 37, 39, 41, and 45.
Kind 11 also takes that branch in game mode 12.
These are handler-dispatch cases, not a blanket passable-object list.
The kind-19 handler returns a value that can change the subsequent rebound decision.
Complete special handling, rollback scheduling, and post-response services remain recovery targets.

## Single-player solid response gate — CODE 7 +1adc..1be6

`solid_response_request` covers the gate and service requests before the recovered rebound record changes.
A nonzero special-handler result skips the ordinary response.
A non-boundary contact also skips the response when the moving record's hold byte is nonzero.
Boundary contact -999 bypasses that hold check.

For an admitted response, the original first calls CODE 28 +268 with contact ID and participant ID.
It then requests sound 0x20007 for a local object collision, or 0x20012 for a local boundary collision.
Remote participants do not request either local sound.
The final request calls CODE 30 +1d54 with the moving object ID, participant ID, and negated elapsed time.
Negation wraps as a signed long, including the minimum signed-long value.

The helper is scoped to game mode zero.
The multiplayer rollback branch and the preceding special-handler dispatcher remain separate recovery targets.
The oracle captures notification, sound, and movement calls; it does not execute those service bodies.
In particular, a verified rollback request does not prove that rejecting a proposed pose matches the original movement service.

The new fixture initially failed its stack assertion after installing hooks over previously translated code.
The same fixture passed in an isolated emulator.
It now flushes Unicorn's translation cache after installing terminal-service and block-exit hooks, as the earlier service fixtures do.

All 2,048 original response-gate cases pass in the full harness after that cache flush.
Checks include twelve participant slots, special-handler suppression, hold values, boundary bypass, signed elapsed extremes, packet order, record preservation, and stack balance.
All 234,829 differential vectors match Rust. The 93 Rust tests, Clippy, and WASM build pass.
Live collision response still awaits the movement rollback service and complete special-contact handling.

## Complete local type-6 movement dispatcher and rollback

`type6_step` composes the recovered turning, planar speed/movement, and shared vertical routines.
The verified scope is local single-player type 6, with coast and surface adjustment zero.
The input turn-rate value represents the original participant getter result.

The complete original CODE 30 +1d54 dispatcher returns immediately for zero elapsed time.
For positive elapsed time, it turns before calling translation and vertical movement.
For negative elapsed time, it calls translation and vertical movement before turning.
The turn delta uses the turn command, participant turn rate, and elapsed time.
The original translation routine applies negative-time speed changes after calculating its planar displacement vector.
Its altitude calculation uses that updated speed.

The oracle executes 512 complete original dispatcher calls through their return instructions.
The original heading update, speed update, vertical update, arithmetic, and CODE 23 turn-rate getter all execute.
Only the final CODE 30 +f60 spatial-index update is captured.
Checks cover pose, speed, turn ramp, shared vertical state, stack balance, and the old/new poses passed to spatial maintenance.
The supplied direction table uses generated sine samples; this does not verify original Macintosh trigonometric table generation.
Coast randomness, nonzero surface adjustment, multiplayer contexts, and other object kinds are outside these new dispatcher cases.

All 235,341 differential vectors match Rust.
A sequence test illustrates why rollback cannot always restore a snapshot.
At the speed cap, forward acceleration leaves speed at five; negative-time rollback then reduces it to four while restoring position.
The test composes the same recovered dispatcher helper for both updates.
All 94 Rust tests, strict Clippy, and the WASM build pass.

The helper is not yet wired into live collision response.
The live adapter must preserve original dispatch order, special-contact decisions, and spatial maintenance around rollback and rebound.

## Live ordinary rollback and rebound

The live movement adapter now uses the verified complete type-6 step when coast and surface adjustment are zero and collision geometry is available.
The proposed pose and altitude go through the recovered contact query.
For ordinary solid contacts, the adapter applies the verified response gate, negative-time movement, then record rebound.
It transfers the resulting pose, speed, turn ramp, vertical position, and hold byte back into live state.
The local sound request follows the recovered response gate, including the distinct boundary request.

A nonzero airborne hold suppresses ordinary rollback and its sound while movement continues.
Boundary contact still bypasses that hold gate.
The supported path now uses original center-coordinate boundary selection rather than the fallback tank-footprint edge check.
Altitude updates use the movement routine's updated speed in this path.
This supersedes the earlier adapter's preceding-speed altitude timing for ordinary movement.

Special-contact kinds retain the existing adapter because their individual handlers can change response decisions.
Unsupported geometry, nonzero coast, and nonzero surface adjustment also retain the previous movement path.
Passable-surface filtering, input sampling, contact order, and reconstructed spatial cells remain adapter choices.
The current turn-rate input remains 65536, matching the preceding live adapter; participant turn-rate lifecycle remains unfinished.
Input command words are recomputed each tick; their persistent original record lifecycle is not claimed here.
The ordinary path does not reproduce every notification or post-response service in the complete collision routine.

A live integration test checks rollback from capped speed five, followed by rebound to speed minus four.
It checks pose restoration in that case, raw height correction, the 30-update hold, and suppression of repeated rollback and sound.
All 95 Rust tests, strict Clippy, and the WASM build pass.
WASM pickup driving, flight/landing/reset, 112 arenas, 476 campaign checks, 18 campaign native/WASM setups, encounters, and death transitions pass.
The unchanged recovered arithmetic remains covered by the 235,341-vector oracle; it was not rerun for this adapter integration.

## Live submergence and emergence contacts — CODE 7 +125c/+12b8

The original local contact dispatcher maps kind 36 to +125c and kind 37 to +12b8.
The owned jump-table entries at +1776 confirm both targets.
Kind 36 starts transition +65536 only when raw height is negative and that transition is not already active.
It requests sound 0x1000c without changing flight energy.
Kind 37 starts transition -65536 unless that transition is already active.
It requests sound 0x10029 and clears the contacted participant's flight energy.
An already-active downward transition leaves energy unchanged and does not repeat its sound.

The oracle executes 2,048 complete original handlers across twelve participant slots and sixteen object slots.
Only sound playback is captured. Object and participant records are checked byte-for-byte.
Signed heights, transition extremes, energy values, stack balance, and dispatch targets are covered.
All 237,389 differential vectors match Rust.

Live surface contact now applies these effects before the shared vertical update.
Both transit kinds are passable in the movement-only query.
The existing type-36 vertical-overlap exception lets a submerged tank reach an emergence device.
A Rust test checks descent, energy clearing, transition limits, single sound requests, and return to the surface.
The WASM test drives through the owned Central world 10110 placements: down at [464,560], then up at [560,464].
It checks the submerged renderer height of -96, return to 32, and complete reset without teleporting.

All 96 Rust tests, Clippy, WASM build, flight checks, 112 arenas, and 476 campaign checks pass.
The handler integration still uses adapter contact scheduling and supported recovered geometry.
Submerged rendering, special weapons, enemy interactions, and complete original spatial-update timing remain unfinished.
Hosting remains stopped.


## Horizon height during live vertical transitions

CODE 6 +4af6 reads A5-20c2 for the horizon band limit.
The existing recovered helper divides that signed height by eight, adds eleven, and clamps the result to 0 through 15.
The live renderer now supplies `vertical.renderer_height` instead of constant 32.
Bands disappear during descent and return during emergence. Hover height does not change the band limit.

Central world 10110 has no matching wave in the loaded catalog, so standalone mode lacks a campaign palette.
Its diagnostic gradient previously bypassed the recovered horizon helper, even with shared executable colors loaded.
The fallback now uses the same recovered band geometry with explicitly diagnostic colors.
No campaign palette is invented for an unlinked arena.

The Rust rendering test checks surface, intermediate, submerged, and hover states with a supplied palette.
It also checks submerged and surface rendering without a palette.
The WASM transit test checks visible horizon pixels before descent, their absence below ground, and their return after emergence.
Offline frames were inspected. All 96 Rust tests, strict Clippy, WASM build, transit driving, and flight checks pass.
The recovered arithmetic did not change; the 237,389-vector oracle was not rerun for this renderer integration.
Other submerged object visibility rules and the complete original render-state schedule remain unfinished.

## Prepared-object drawing order — CODE 12 +1dd4..1efe

The negative byte traced from submerged objects is a drawing class, not a visibility flag.
CODE 6 initializes local A6-190e to one at +1bb0.
For type 6, a negative resolved height changes that class to minus one at +1c7a.
CODE 6 +2a42 copies the class into prepared record offset +21.
CODE 12 +1cac copies it into the next rendering record at offset +1b.
The preparation pass also accumulates signed minimum and maximum classes, initially zero and one.

When A5-7be2 is nonzero, CODE 12 visits classes in ascending signed order.
Within each class, it visits prepared records in descending index order and calls the filled drawing routine at +1428.
When A5-7be2 is zero, it visits records in ascending index order and calls the wireframe routine at +fda.
Neither dispatch branch discards a record because its class is negative.
The original byte increment exits on overflow after class 127.

`render_dispatch_order` reproduces these dispatch orders in Rust.
Its input is the prepared record sequence, not the unsorted world obstacle sequence.
The live renderer does not use this helper yet. Earlier preparation, ordering, and complete class assignment require recovery first.
Applying this helper directly to world items would not reproduce the original pipeline.

Run `.local/re-tools/bin/python scripts/verify-render-dispatch.py` to repeat the focused differential test.
It executes the owned CODE 12 block and captures only terminal drawing calls.
The test covers all signed class values, empty lists, equal classes, mixed classes, and lists of up to 100 records.
It checks exact dispatch order, the submitted geometry pointer, record preservation, and stack balance.
All 1,556 vectors match Rust. They are separate from the existing 237,389-vector main oracle.
This evidence does not establish the earlier visibility gates or original pixel output.

## Render preparation: insertion and overlap marking

CODE 12 +1a5e..1af6 inserts each incoming object into the prepared record list.
It compares signed words: incoming offset +6 against existing prepared offset +0.
Existing equal keys remain before the new record, so insertion is stable.
The block shifts complete 32-byte records and their corresponding geometry pointers together.
`render_preparation_order` now reproduces the resulting input-index sequence.

This preparation order and the later dispatch order are distinct.
Filled drawing reverses prepared indices within each signed class, including ties in the insertion key.
Wireframe drawing keeps prepared index order.
Neither rule alone establishes the complete original visibility pipeline.

CODE 12 +1d56..1dc2 marks overlapping record intervals.
The original gate is bytewise `detail & (filled ^ 1)`, not two independent Boolean tests.
For ordinary Boolean inputs, marking runs only with detail enabled and filled mode disabled.
For each other record, its minimum must be at most the new maximum, and its maximum at least the new minimum.
Both comparisons are signed and inclusive. A match sets both records' flag at offset +9 to one.
Other flag bytes remain unchanged, including non-Boolean values.
`render_overlap_flags` reproduces this block, with explicit rejection of invalid host array lengths or indices.

The focused verifier now covers 262 original insertion sequences and 1,024 original overlap blocks.
Insertion checks complete moved records and geometry-pointer associations after every insertion.
Overlap checks all record bytes except the permitted flag updates, plus stack balance.
Both cover signed extremes; overlap also covers non-Boolean mode bytes and inverted intervals.
Together with the prior dispatch cases, all 2,842 focused differential vectors match Rust.
All 96 workspace tests and strict Clippy pass.

These helpers are not yet connected to live drawing.
The intervening transform call, prepared-record construction, and per-type class assignment remain integration prerequisites.
The test supplies per-record initialization after each original insertion; it does not claim complete preparation execution.

## Prepared rendering record construction — CODE 12 +1c20..1d56

The complete record-construction block now has a Rust implementation: `prepare_render_record`.
It accepts the original 64-byte input record and updates the original 32-byte prepared record.
The model inputs are the signed counts at +25c and +25e, plus the byte at +265.
The supplied interval comes from the preceding projected-vertex scan.

The block copies the model identifier, color, heading, height, rendering parameters, and drawing class.
It converts fixed deltas with a wrapping shift, high-word extraction, and word negation.
It initializes the overlap flag from whether the model's +25e count is nonzero.
It updates signed class bounds without assuming their previous values are zero and one.
For a positive face count, it calculates face-culling coordinates through the original direction lookup.
Otherwise, the two output coordinate words remain untouched. The unused byte at +17 also remains untouched.

The focused verifier executes the complete original block, including CODE 12 +140.
It varies signed counts, all record fields, class bounds, direction-table values, and output slots.
It checks all output bytes, input preservation, guard bytes, and stack balance.
All 1,024 construction cases match Rust in isolation.

The vertex transform itself is already recovered as `transform_project_vertices` and used by the cockpit renderer.
The transform setup is also recovered as `model_view_transform`.
The earlier integration notes overstated that gap: the remaining task is to compose the stages and reproduce their surrounding record lifecycle.
In particular, projected-interval scanning, source class assignment, and complete preparation-to-drawing execution still need integration evidence.
This construction helper does not change live drawing yet.

The combined focused suite passes all 3,866 vectors after flushing cached emulator blocks at the new stop boundary.
All 96 Rust tests, strict Clippy, and the WASM build pass.
The main 237,389-vector oracle was not rerun; its recovered routines did not change.

## Integrated object transformation and preparation — CODE 12 +1af6..1d56

`prepare_render_geometry` now composes transform setup, vertex projection, interval scanning, and prepared-record construction.
The verifier executes that entire original block, including CODE 11 +3ba8 and CODE 12 +140.
Neither service is replaced by a stub. The camera-height word and input rendering record are supplied fixtures.

The interval scan uses the same bytewise detail gate as overlap marking.
When enabled, it starts with minimum 9,999 and maximum -9,999, then reads each projected horizontal coordinate.
These sentinel bounds remain relevant even when all coordinates lie beyond one sentinel.
When disabled, the prior interval locals remain unchanged.

The geometry source can be a base model or an override model.
Transformation writes the selected geometry's vertex count, but scanning uses the base model's count.
When the scan count exceeds the transformed count, the original reads retained coordinates from the output buffer's tail.
Rust preserves those entries and scans them too. It rejects counts beyond the supplied buffer capacity.
Near-depth projection also preserves prior projected coordinates, as established by the earlier transform recovery.

The integrated cases vary base and override geometry, buffer capacity, scan count, signed coordinates, metadata, and non-Boolean mode bytes.
They check every output vertex word, every prepared-record byte, interval locals, class bounds, input preservation, buffer guards, and stack balance.
All 512 integrated cases match the original, including scans beyond newly transformed geometry into retained buffer entries.

This connects previously isolated arithmetic through an original executable path.
It does not yet reproduce the complete CODE 12 entry point or its final rasterization.
Live integration still requires source-object class assignment, model metadata, and the surrounding insertion/buffer lifecycle.
The current `Shape` representation does not retain every original runtime model field required by this path.

The combined focused suite passes all 4,378 vectors.
All 96 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.

## Runtime model metadata setters and field correction

Further tracing corrects the provisional name `edge_count` used by `RenderModelFields`.
Runtime +25a holds the total edge count. Runtime +25c holds the face count.
Runtime +25e is an edge-range split used by the wireframe routine, not the total edge count.
CODE 12 +123e and +125e use it to divide the drawing ranges.
The Rust field is now `edge_split`. Deserialization accepts the old name for saved fixtures.
This changes the field name, not the previously verified nonzero test.

Complete CODE 12 +08ee writes the split at +25e, two face references at +260/+262, and a mode byte at +264.
`configure_model_edges` reproduces those writes after the caller resolves the selected model.
Complete CODE 12 +0926 independently sets +265 to one; `enable_model_render_mode` reproduces that write.
The two mode bytes are distinct. Prepared-record construction copies +265, not +264.
`RenderModelFields::from_runtime` reads the exact fields consumed by prepared-record construction.

The original initializer at CODE 12 +0882 allocates and clears the runtime model.
CODE 19 +37e4 invokes the separate +0926 setter while building ammo dumps.
Other observed +08ee callers include CODE 19 +050c, +0b68, and +1e34.
These fixed builder calls do not establish metadata for every resource-loaded model.
The resource's trailing `Plane Sides` word remains separate; this trace does not prove a mapping to these runtime fields.

The focused verifier adds 1,024 complete setter cases across one hundred selected model slots.
Cases execute either setter or both, with signed words and arbitrary mode bytes.
The checks compare the entire 0x804-byte model, selected-slot lookup, Pascal stack cleanup, and Rust field extraction.
All 5,402 focused vectors match Rust. All 96 Rust tests and strict Clippy pass.
Live metadata propagation remains pending complete loader integration.

## Resource Plane Sides and live overhead fill

CODE 1 +192a..197c resolves the trailing `Plane Sides` word and stores it through CODE 19 +3e1e.
A value of -1 selects the resource line count. Signed values above eight clamp to eight; there is no lower clamp.
The compatibility branch substitutes -1 without consuming the trailing word when A5-414c is two and A5-414b's upper nibble is zero.
`resource_plane_sides` reproduces both the value and whether that word is consumed.

CODE 6 +3308..3404 uses this per-model count in its top-view drawing path.
It submits lines after the selected prefix, then fills a polygon from the first selected projected vertices.
Thus `Plane Sides` is top-view fill metadata, not the runtime edge split at +25e.
The current shape decoder already retains the trailing word.

The overhead renderer now uses that authored fill count through `Shape::top_view_fill_count`.
It draws remaining top-view lines and fills the selected vertex prefix.
The adapter uses the ordinary resource-tail path; original compatibility-global selection remains outside this integration.
Invalid host geometry counts produce no fill instead of indexing beyond the supplied vertices.
Procedural meshes without top-view data retain the existing projected-edge fallback.
Camera framing, map rotation, colors, and exact polygon rasterization remain provisional.

The focused oracle adds 1,024 original loader-tail executions, including the original slot setter.
It checks signed extremes, compatibility overrides, resource-pointer advancement, selected-slot writes, and stack balance.
All 6,426 focused comparisons pass.
A Rust rendering regression checks filled and outline-only top views plus invalid geometry bounds.
All 97 Rust tests, strict Clippy, WASM build, 112-arena checks, and flight/view/reset checks pass.
The offline WASM overhead frame was inspected and shows filled tank and obstacle silhouettes.
Hosting remains stopped.

## Live overhead primary colors

CODE 6 +2cfe..2d10 reads the primary kind-color table at A5-2742 for top-view drawing.
That table is the first byte of each recovered kind binding, not the second byte used for radar.
The ordinary single-player type-6 branch retains this color unless its flashing override applies.
The pickup branch at +308a selects the effect color by pickup subtype.

Overhead scenery, ordinary enemies, flags, and the player now resolve their primary binding through the current level palette.
Pickups retain their subtype effect colors. Missing palette or binding data retains diagnostic fallback colors.
The adapter does not yet implement all original flashing, participant, and multiplayer color overrides.
It also retains the provisional grid, background, projection, and shot colors.

The rendering regression supplies different primary and radar colors, then changes the palette while keeping the binding fixed.
It checks primary selection, palette changes, and the no-palette fallback.
All 97 Rust tests, strict Clippy, WASM build, 112-arena native/WASM checks, and flight/view/reset checks pass.
The offline overhead frame was inspected: the opening level now has yellow scenery, red enemies, and a green player.
No recovered arithmetic changed; the existing original-code oracle suites were not rerun for this adapter change.
Hosting remains stopped.

## Live enemy firing uses player altitude

The live encounter adapter still supplied zero for the target geometry's +28 field.
That constant predated the player flight and transit integration.
The recovered CODE 20 +1e9a firing predicate rejects negative target height after updating its firing timer.
Supplying zero bypassed this original exclusion while the player was submerged.

The adapter now supplies `vertical.vertical`, the player's signed 16.16 vertical position.
This connects the existing original predicate to live type-7 and surface-enemy firing checks.
It does not disable enemy tracking or clear already-fired projectiles.
The original check permits positive target height; it does not grant flight immunity.

A live Rust test holds a submerged player near an aligned enemy with a zero firing interval.
It checks no firing for thirty ticks, timer initialization, and resumed firing after emergence without resetting the enemy.
A separate airborne case checks that positive height still permits firing.
All 98 Rust tests, strict Clippy, and the WASM build pass.
The arithmetic was unchanged; its existing 1,410-case original firing oracle covers negative, zero, and positive target heights across 47 kinds.

Encounter native/WASM replay parity, transit driving, and flight/view/reset checks also pass.
Hosting remains stopped.

## Behavior 4/9 attack tracking — CODE 20 +0ef4

The recovered intent dispatcher selects +0ef4 for behavior states four and nine.
This complete handler is now implemented as `ai_attack_tracking` and connected to the live encounter action dispatch.
It sets movement mode zero, copies the cached target pose, and resets the heading deadline to -1.
It requests behavior three when the original approximate planar distance exceeds a difficulty-dependent threshold.
The threshold is `500 - 3 * effective difficulty`, calculated with signed-word wrapping before conversion to fixed coordinates.
Equality does not trigger reinitialization. The target identifier remains unchanged on this distance branch.

CODE 1 +0882 supplies effective difficulty from A5-4256, distinct from selected difficulty at A5-4258.
The live adapter supplies its existing effective-difficulty field.
For a nonzero game mode and an inactive participant, the handler instead sets behavior three and clears the target.
That branch does not request a cached pose or change movement and heading state.

The oracle executes 1,024 complete original handlers.
Only cached-pose retrieval is supplied; the original distance routine, difficulty getter, and multiplication helper execute directly.
Cases check every participant byte, geometry preservation, cached-pose requests, and stack balance.
The full main oracle now passes 238,413 vectors. The separate rendering suite is unchanged at 6,426 vectors.

A live test exercises states four and nine at different effective difficulties.
It checks retained pursuit at the larger threshold, reinitialization at the smaller threshold, and normal initialization on the next update.
All 99 Rust tests, strict Clippy, and the WASM build pass.
This adds a missing handler but does not yet enable additional stationary enemy types.
Their initialization and movement dispatch paths still require integration evidence.


## Type-8 pursuit and movement comparison

The original CODE 30 +1370 dispatch sends kinds seven and eight to the same planar movement branch at +17e6.
The oracle now executes 800 additional kind-eight movement caller blocks from +2578 to +25e8.
These include the angular-rate getter, turn update, translation dispatch, and original direction lookup with a synthetic marker table.
Cases cover positive, negative, and zero elapsed time, wrapped arithmetic, and preservation of unrelated geometry bytes.
This bounded test excludes the outer dispatcher and subsequent spatial-index update.

Another 800 cases execute the complete mode-zero initializer at CODE 20 +0b6c for kind eight.
Opponent selection, distance, and cached pose are supplied services.
Unlike kind seven, kind eight does not request the world-object search.
Its state changes match `ai_initialize_type7` with object zero, including failure to acquire an opponent.
The test checks service order, the timer reset, and preservation of every unrelated participant byte.

Another 1,200 cases execute the complete tracking handler at CODE 20 +1240 with kind-eight participants.
Target selection and cached pose are supplied services; the handler itself executes from the owned executable.
Its transitions match the shared kind-seven Rust tracking handler, including local-participant writes and nonzero game modes.
All 241,213 main differential vectors pass. This adds 2,800 cases; the separate rendering suite is unchanged.

Kind eight remains disabled in live encounter updates.
CODE 20 +0256 requests prediction mode one for this enemy, unlike ordinary kind-seven mode zero.
The live cache adapter currently supplies the player pose without extrapolation.
Recover CODE 20 +0040 predictive arithmetic before enabling kind-eight pursuit as an original-behavior implementation.


## Predictive target pose and live type-8 pursuit

`ai_predict_pose` transcribes the complete CODE 20 +0040 target calculation.
Mode zero copies the target pose. Nonzero modes calculate an intercept from target speed byte +22 and target heading.
The calculation uses signed high-word coordinate differences, wrapping integer products, and signed-word truncation of travel time and distance.
The output retains the target heading. The read of turn byte +23 does not affect this result.

CODE 18 +0d24 supplies the direction pair from the unit table.
CODE 11 +3c5a supplies software fixed multiplication for the speed-square term.
CODE 12 +0020 supplies three rounded integer-root iterations; `ai_integer_root` retains its word wrapping and DIVU overflow behavior.
CODE 36 supplies long multiplication and signed division.
A standard floating-point intercept formula would not preserve these operations.

`scripts/verify-ai-prediction.py` executes 2,048 complete original calls with no replacement service results or OS traps.
It uses a synthetic unit table, random participant slots, full-range coordinates, signed speed bytes, and modes zero, one, two, and 255.
The checks compare all output coordinates and preserve participant records, geometry, output boundaries, and stack balance.
Original speeds -24 and 24 are excluded because the predictive denominator is zero.
Rust returns `None` for invalid lookup indices and division faults; separate tests cover these host guards and the mode-zero bypass.
These cases are separate from the 241,213 main vectors and 6,426 rendering vectors.

The live encounter now admits type eight into movement updates and supplies predictive cache results from the player’s stored speed.
Kind-eight initialization skips object selection. Its shared pursuit and movement handlers use the preceding original-code comparisons.
Ordinary type seven continues to use mode-zero current-pose aiming.
If prediction reaches a host guard, the live adapter uses the current target pose.
The direction table still uses modeled SANE values; OS angle conversion, collision response, and outer scheduling retain their documented limits.

A live regression checks prediction ahead of a moving player, initialization without object-selection randomness, and movement after turning.
The authored WASM test finds kind eight in Central level four, checks movement, and checks reset.
All 101 Rust tests, strict Clippy, the WASM build, and existing native/WASM encounter replay checks pass.
Hosting remains stopped.


## Firing selects cached aim in movement mode one

CODE 20 +212a..+219a selects the angle used by the firing arc check.
If target slot is one, source slot is not one, and movement byte is one, it calls the cached angle getter at +031c.
Other combinations request the direct geometry angle through CODE 6 +1680.
This condition is independent of enemy kind. Type eight therefore uses its predictive cache on the qualifying path.
The previous encounter adapter always recalculated the direct player angle, bypassing this original branch.

`ai_fire_uses_cached_angle` now selects the live firing angle with the original condition.
The shot still launches from the firing tank’s heading; this change selects firing eligibility, not a new projectile trajectory.
The existing cache refresh supplies the cached angle. Full original command/movement scheduling remains unfinished.

The prediction verifier now includes 720 original firing-angle routing blocks.
Cases cover all twelve source and target slots with movement bytes zero, one, two, three, and 255.
Terminal cached/direct angle results are supplied with distinct values.
The test checks service selection, participant and geometry pointers, the returned angle, record preservation, and stack balance.
All 2,768 cases in this separate suite pass: 2,048 complete prediction calls plus 720 routing blocks.
The main and rendering vector counts remain unchanged.

A live regression covers both kinds seven and eight.
An aligned cached angle permits firing in mode one; a misaligned cached angle suppresses it despite direct alignment.
Mode three still fires from direct alignment when the unused cache angle differs.
All 102 Rust tests, strict Clippy, and the WASM build pass.
The authored type-eight movement/reset check and death transition check also pass.
Native/WASM encounter replay parity also passes. Hosting remains stopped.


## Composed weapon-eight target selection and heading

`weapon_target_angle` composes CODE 30 +0382..+03ca with the complete CODE 20 +0754 selector.
The constructor passes the firing participant as source, mode zero, and excluded participant zero.
Selection scores candidates relative to the participant pose. The final angle uses the selected target pose and the projectile pose.
These source poses need not be identical. The helper preserves both calls instead of reusing the selection angle.
`weapon_initial_heading` then applies the correction and continues as weapon zero.
Without a selected target, heading remains unchanged and construction still continues as weapon zero.
This constructor does not establish continuously homing flight.

`scripts/verify-weapon-targeting.py` executes 1,024 composed original constructor blocks.
The original selector, distance calculation, and heading wrapper run directly. Only angle conversion results are supplied.
Fixtures vary all participant slots, ownership and activity bytes, teams, health, kind-eight angular exceptions, range boundaries, and game modes.
Distinct projectile poses and final-angle results expose accidental reuse of owner-pose angles.
The test compares ordered angle-service arguments, resulting weapon and heading, unchanged source records, projectile writes, and stack balance.
All 1,024 comparisons pass. This is a separate suite; existing main, prediction, and rendering counts are unchanged.

In single-player mode, selection requires a nonzero bitwise intersection of candidate activity and ownership bytes.
The current player adapter has ownership zero, while enemy adapters have ownership one.
Therefore an enemy weapon-eight implementation must not automatically substitute player slot one for the selected target.
The helper is not connected to live weapon controls yet. Player special-weapon controls and complete projectile hit routing remain unfinished.
All 102 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.


## Optional-weapon cycling and wave-hint unlocks

`cycle_optional_weapon` implements the complete CODE 6 +121e routine.
It advances through slots zero to seventeen, wrapping seventeen to zero.
Only availability bit zero admits a slot. If none qualifies, it returns the original selection after one complete cycle.
Rust guards current slots outside this range instead of reading adjacent memory.

`unlock_optional_weapon` implements the complete CODE 6 +54ec routine.
The argument is a wave hint ID, not a level number.
Hints map to weapon slots as follows:

| Hint | Weapon slot |
| --- | --- |
| 10 | 1 |
| 11 | 9 |
| 12 | 10 |
| 13 | 2 |
| 14 | 11 |
| 15 | 5 |
| 17 | 17 |
| 18 | 13 |

A zero availability byte causes automatic selection of the unlocked weapon. Every unlock writes availability one.
A repeated unlock preserves selection. Unlike cycling, this test treats every nonzero availability byte as already unlocked.
Hint sixteen tests slots in order 10, 2, 9, 5, 1, 17, 13, 11, selecting each missing slot in turn.
It then enables all eight slots. Thus the last previously missing slot in that order becomes selected.
Other hint IDs leave state unchanged.

The new `scripts/verify-weapon-selection.py` executes 2,048 complete cycles and 2,048 complete hint-unlock calls.
No arithmetic or OS service is replaced. Cases cover empty availability, even nonzero flags, wraparound, repeated unlocks, and signed hint limits.
Every parameter-record byte, adjacent selection-global bytes, and stack balance is checked.
All 4,096 comparisons pass. These form a separate suite; other vector counts are unchanged.

CODE 33 +0612..+0638 clears the eighteen availability bytes and selected slot during initialization.
CODE 25 +1eea loads the wave resource and calls the unlock routine for successive hint words through the requested level, capped by resource count.
The hint word is at offset six in the first resource row, after the count word.
These callers were inspected in disassembly; their complete resource-loading execution is not part of the new comparison suite.

The supplied video review identifies a Scattershot hint at level eleven around 13:40.
The owned Central wave resource 10000 has hint fifteen at level eleven; resources 10001 and 10002 have hints twenty-one and fifteen respectively.
Resource 10002 starts with hint twenty-two, unlike the opening flight-weapon hint in the recording.
These are consistency checks, not proof of executable revision or difficulty settings.

The manual identifies Command for selection and Option for optional fire on the standard Mac layout.
CODE 31 +18c2..+1902 admits selection only when its control word equals one and more than ten ticks have elapsed.
It updates the selection timestamp even when the slot does not change, and requests sound 0x10022 only when selection changes.
These input/timing branches still require execution tests before live integration.
Browser selection controls and special-weapon firing remain unfinished.
All 102 Rust tests, strict Clippy, and the WASM build pass. Hosting remains stopped.


## Optional-weapon input timing and request order

`optional_weapon_selection` composes CODE 31 +18c2..+1902 with the original CODE 6 cycling routine.
Selection requires control word one and a signed, wrapping tick difference greater than ten.
An accepted selection always writes the current tick, even if cycling returns the same slot.
Sound request 0x10022 with argument one occurs only when the slot changes.
This preserves the repeat delay when the player holds selection with no alternative weapon available.

`optional_weapon_fire` transcribes CODE 31 +187a..+18c2.
It requires control word one and more than fifteen ticks since A5-1376, the shared primary-attempt timestamp.
Selected weapon five expands into requests [5, 3, 4], in that order, after one timing check.
Other selected bytes produce one signed-byte request. Noncanonical control words do not fire.
This branch does not itself update the timestamp; the called firing routine owns that state change.
Projectile admission, ammo charging, and effects remain separate from these input requests.

The weapon-selection suite now contains 8,192 original comparisons.
It retains the 4,096 complete cycling/unlock calls and adds 2,048 selection-input blocks plus 2,048 optional-fire blocks.
The selection blocks execute the original cycle and capture only terminal sound requests.
The firing blocks capture ordered projectile requests without executing projectile construction.
Cases cover exact tick thresholds, signed overflow, empty weapon sets, nonboolean controls, and signed selected bytes.
All globals outside the allowed selection/timestamp writes remain unchanged; parameter records and stack balance are also checked.

All 102 Rust tests, strict Clippy, and the WASM build pass.
These helpers are not yet wired into live browser controls. Hosting remains stopped.


## Campaign optional-weapon loadouts integrated

`Game::configure_optional_weapons` now creates the campaign loadout from original wave hints.
It starts with eighteen zero availability bytes and selection zero, matching the inspected CODE 33 initialization.
It applies the verified hint-unlock routine to every row through the selected level, capped by the resource length.
It does not wrap the hint prefix when world selection repeats beyond the final row.
This follows the inspected CODE 25 +1eea caller rather than applying only the currently selected wave’s hint.

The game stores current and initial `OptionalWeapons` state.
Reset restores initial selection and availability; loading an earlier level starts with a fresh set.
Campaign setup calls this method before target population. Standalone research arenas retain empty availability.
Native and WASM snapshots expose the same `optional_weapons` object with `selected` and `available` fields.
No new selection or firing control is connected in this change.
Future campaign carryover must preserve a manual selection unless a newly unlocked weapon changes it.

The campaign verifier now checks loadouts against cumulative resource hints for all 476 level and repeat-boundary cases.
It checks the first three Central resource-10000 selections explicitly: slots thirteen, seventeen, and one.
All eighteen native/WASM setup comparisons pass, including the new loadout state.
A Rust regression changes selection, resets, loads beyond the wave count, and reloads an earlier level.
All 103 Rust tests, strict Clippy, the WASM build, and encounter replay parity pass.
The original-code suites are unchanged because the recovered unlock arithmetic did not change.
Hosting remains stopped.


## Live optional-weapon selection controls

The fixed-step game now passes `Input::select_weapon` through the recovered selection timing routine.
Selection state changes only on simulation ticks. The accepted timestamp starts at zero and resets with the level.
Changed selections request original sound 0x10022, decoded by the existing sound adapter as sample 10034.
A held control repeats after eleven ticks; unchanged selection still advances its repeat timestamp without requesting sound.
Death transitions replace selection input with the same empty input used for movement and firing.

The browser maps Q and the touch Select weapon button to this input. The desktop window also maps Q.
Q is a port binding; the original standard Mac binding is Command.
The additive `game_frame_controls` WASM entry accepts selection input. Existing `game_frame` callers supply no selection input and retain their ABI.
The browser clears Q with the other held controls on pause and focus loss.
Its status text names the selected weapon and explicitly states that optional firing is unavailable.
Names follow hint strings in owned Central STR# 1007 and the recovered hint-to-slot mapping.
The label updates only when its text changes, avoiding repeated live-region announcements during rendering.

A Rust regression checks tick boundaries, held repeats, change-only sound, empty alternative sets, reset, and death suppression.
`scripts/verify-weapon-controls.mjs` checks the live WASM path at ticks eleven and twenty-two.
It also checks button release, zero-time redraws, sound delivery, reset, and silent cycling with one available weapon.
All 104 Rust tests, strict Clippy, WASM build, JavaScript syntax, encounter replay parity, and 476 campaign checks pass.
All eighteen native/WASM campaign setups still agree.
Visible browser keyboard/touch interaction has not been tested in a hosted page; hosting remains stopped.
Special-weapon firing and the original weapon-icon renderer remain unfinished.


## Complete single-player local firing dispatch

`local_weapon_attempt` composes CODE 30 +08b2, CODE 28 +0004, CODE 27 +0288/+01d8, and CODE 26 +0906.
A nonzero A5-12d2 disables the entire attempt without updating its timestamp.
Otherwise, the attempt writes the current tick before testing ammo, health, and the firing-inhibit byte.
The original eligibility arithmetic retains signed-word subtraction and its byte-mask behavior.
A successful request reaches the constructor with the selected weapon, source pose, current tick, participant, kind seven, and integer height.
Height uses signed division by 65536, truncating toward zero.
Valid selected slots zero through seventeen remain unchanged at this dispatch stage.
Flight-specific behavior belongs to later construction/update branches, not this gate.

A rejected eligibility check requests sound 0x30006 with argument one when ammo is less than or equal to cost.
Equality matters when another condition rejects the attempt.
This routine does not deduct ammo or allocate a projectile; those remain constructor-admission responsibilities.
Rust guards resource slots outside zero through seventeen instead of reading outside the cost table.

`scripts/verify-local-weapon-fire.py` executes 2,048 complete original dispatch paths.
Eligibility, packet creation, local participant stamping, weapon filtering, and signed height division execute directly.
Only terminal projectile-constructor and sound calls are captured.
Cases cover all eighteen weapon slots, all twelve participant slots, signed cost/ammo limits, disabled attempts, health, inhibit bytes, and height boundaries.
The checks compare constructor arguments, sound requests, last-attempt ticks, untouched records and globals, and stack balance.
All 2,048 comparisons pass as a separate suite.

Primary fire now uses this composed attempt result before its existing admission and projectile path.
It uses the original empty-ammo request instead of the adapter’s stricter ammo-less-than-cost check.
This does not yet connect optional firing controls or implement all selected weapon effects.
All 104 Rust tests, strict Clippy, WASM build, encounter replay parity, weapon-selection controls, and flight/firing checks pass.
Hosting remains stopped.


## Playable Scattershot

Player Scattershot now uses the recovered optional-fire gate and ordered requests [5, 3, 4].
Each request passes through the verified local-attempt and constructor-admission rules independently.
The catalog copies all eighteen original player ammo costs into the game instead of retaining only slot zero.
The owned Central wval 10000 charges one round for each of slots five, three, and four.
Thus a complete burst costs three rounds; insufficient ammo or active-shot capacity can admit only part of a burst.

The center request preserves heading. Side requests use the recovered seven-to-fourteen-degree spread calculation with one Random sample each.
Random samples occur only after admission, as in the original constructor.
The existing deterministic adapter supplies samples; the original Random generator remains unrecovered.
CODE 30 heading preparation converts slots three and four to weapon zero.
The common record finalizer also stores weapon zero for slot five.
All three therefore use the existing basic projectile motion, lifetime, hit path, and primary damage table entry.
The center uses the original Scattershot sound; side requests have no separate firing sound.
Existing collision sampling, projectile allocation, and height-display limitations remain.

Primary fire runs before optional fire, which runs before selection, matching the inspected CODE 31 input order.
Primary fire can postpone optional fire through their shared attempt timestamp.
The optional-fire timing gate runs once before the burst; later requests still execute after the center updates the timestamp.

Browser and desktop E now fire Scattershot. The browser also has a touch fire button when Scattershot is selected.
Other selected weapon effects remain unavailable and are identified as unfinished in the browser status.
The additive `game_frame_weapons` ABI accepts optional firing; prior frame APIs retain their behavior.

Native tests cover spread signs, tick-sixteen eligibility, per-request ammo costs, partial bursts, shot limits, Random consumption, and primary-fire precedence.
A damage regression confirms that Scattershot damages vulnerable targets and preserves vulnerability checks.
The WASM test loads original Central level eleven at difficulty zero and checks bursts at ticks sixteen and thirty-two.
It compares ammo, shots, tank state, counters, sound, and selection against a native replay.
Reset and unsupported-weapon guards also pass.
All 106 Rust tests, strict Clippy, the WASM build, JavaScript syntax, encounter replay parity, and selection controls pass.
The original-code suites are unchanged; this change composes previously verified routines.
Hosted keyboard and touch interaction remains unverified. Hosting remains stopped.


## Seeker reacquisition and steering

CODE 30 +240e..+248c handles stored weapon 9 during projectile movement.
This differs from weapon 8, which corrects its heading during construction and becomes weapon zero.
The owned weapon-selection hints identify slot 9 as Seekers.

The movement branch calls CODE 20 +08f8 with the projectile object, owner, and mode zero.
The selector measures from the projectile, not the owner.
It scans participants 1 through 12 and excludes the owner, inactive records, and nonpositive health.
It does not inspect team or participant owner flags.

The distance limit is 300 units. Beyond 200 units, non-kind-8 targets require absolute angle error at most 45 degrees.
Scoring adds distance to twice the absolute angle error in fixed point. Equal scores retain the earlier participant.
Word absolute-value and score arithmetic preserve original signed overflow behavior.

After selection, the branch requests the angle again, divides the signed integer by three, and adds the fixed-point result to heading.
It wraps the result with CODE 7 +0004. No target leaves heading unchanged.
This correction precedes direction lookup and planar movement; it is not multiplied by elapsed ticks in this branch.

Rust `seeker_select_target` and `seeker_heading` reproduce these operations.
`scripts/verify-seeker.py` compares 1,024 composed executions against the owned machine code.
The original selector, distance, multiplication, and wrapping execute without replacement.
Only angle-conversion results are supplied. Ordered angle arguments, preserved records, projectile writes, and stack balance are checked.
The report is `.local/disassembly/seeker-vectors.json`.

The live projectile adapter now calls these helpers before movement for weapon 9, when elapsed ticks are nonzero.
It rebuilds direction from the corrected heading and retains fixed-point position.
The adapter uses player slot 1 and the first eleven stable target slots, matching existing enemy AI integration.
It refreshes health between projectiles so earlier kills cannot remain eligible.
The angle callback uses the existing recovered quadrant handling with modeled slope conversion.
This participant mapping, original scheduling, collision sampling, and OS math remain provisional.

Optional fire now admits slot 9, charges its resource cost, and emits its original firing sound.
Central level 13 at difficulty 0 provides the native/WASM parity fixture.
The Rust test adds a target after launch and then kills it, checking acquisition and loss during flight.
The new projectile `weapon` field preserves identity through snapshots and steering updates.


## Smart Missile guidance and ascent

The weapon-10 constructor at CODE 30 +0616..+0694 creates geometry kind 34.
It sets vertical velocity to 15 fixed-point units and adds 20 units to height, with a minimum height of one.
Its lifetime is four times the supplied lifetime word. Constructor recovery already covers these fields.

The movement branch at CODE 30 +211c..+22a6 differs from Seeker movement.
If the owner has valid kind-6 geometry, elapsed time increases by its signed integer quotient divided by three.
Owner zero or geometry index minus one skips this adjustment.

While vertical velocity is positive, the missile does not select a target.
At height at most 100 fixed-point units, it adds twice velocity times elapsed, divided by five.
Above 100, it changes velocity to minus two and leaves height unchanged for that update.
The next update can select a target with the same projectile-based selector used by Seekers.

After ascent, a selected target produces the same one-third integer-angle heading correction.
If the target is higher, the missile subtracts its signed vertical step from height.
Otherwise, including equal target height or no target, it adds that step.
This phase clamps negative height to zero. The ascent branch does not apply that clamp.
Arithmetic preserves wrapping long multiplication and signed division.

`smart_missile_elapsed` and `smart_missile_guidance` reproduce these branches.
`scripts/verify-missile-guidance.py` executes CODE 30 +211c through +22a6 against 2,048 cases.
The complete original target selector, distance, multiplication, division, and heading wrapping run unchanged.
Only the angle service receives supplied results; argument order is checked.
The harness checks projectile writes, preserved geometry and participant records, and stack balance.
Cases cover the ascent threshold, zero velocity, target heights, missing owner geometry, and elapsed-time adjustment.
The report is `.local/disassembly/missile-guidance-vectors.json`.

The following planar branch uses twice the projectile direction times adjusted elapsed, divided by five.
The guidance-only comparison stops before that branch; the composed flight comparison below includes it.
Smart Missiles are not yet playable. Their complete movement, height-aware collision, damage, and presentation still need integration.


## Composed Smart Missile flight

`move_smart_missile` reproduces CODE 30 +22a6..+22ec after guidance.
It uses the projectile direction table, whose intended scale is six times the unit table.
Each axis adds twice its direction component times adjusted elapsed, divided by five.
The multiplication wraps before signed division. Coordinate addition also wraps.
Direction lookup uses the corrected heading, including its original integer-degree truncation.

Run `.local/re-tools/bin/python scripts/verify-missile-guidance.py --flight` for the composed comparison.
It executes CODE 30 +211c..+22ec, including the original CODE 18 direction lookup.
All 2,048 cases match Rust. The table contains synthetic irregular values, including signed extremes.
This checks indexing, output order, scaling, and overflow; it does not verify original OS-generated table values.
The harness also checks that the direction table remains unchanged.
The report is `.local/disassembly/missile-flight-vectors.json`.

The Rust unit test checks coordinate overflow, fractional division order, preserved vertical fields, and invalid-heading rejection.
The helper requires the caller to apply the owner-dependent elapsed adjustment exactly once.
The original outer movement function skips work for zero elapsed; these helpers represent its inner branch only.
Spatial-grid updates, collision, expiry, rendering, and playable integration remain outside this composed comparison.


## Smart Missile collision filtering

CODE 7 +0ad6 uses the same swept-query dimensions for cannon kind 9 and missile kind 34.
The query midpoint uses wrapping coordinate sums and a logical right shift.
Width is 1.5 fixed-point units. Length is six times elapsed plus three units.
This collision length does not use the missile's slower planar movement formula.
The original caller supplies its outer elapsed value; the missile movement branch adjusts a separate local copy.

After selection, CODE 7 +0b76..+0d72 rejects vertical separation greater than 60 fixed-point units.
It also rejects target kinds 29 and 42, and same-owner kind 32.
The original membership table at +0d8a includes both kinds 9 and 34.
Neither source kind enters the separate Spinner, shield, or mine side-effect branches.

`direct_projectile_contact` now reproduces this shared post-query filter.
`CollisionQueryWorld::basic_projectile_hit` accepts both supported source kinds.
`Query::hit_projectile` preserves the requested geometry kind and raw fixed-point height.
The existing `hit` method still supplies kind 9 for existing cannon and Seeker callers.
Unsupported source kinds return None instead of using these incomplete rules.

`scripts/verify-projectile-contact.py` compares 2,048 executions of the original filter against Rust.
The selected hit is supplied; the preceding spatial query is not part of this comparison.
All 48 target kinds, owner equality, signed height boundaries, and wrapping subtraction are covered.
Source records and stack remain unchanged. No service stubs are used.
The report is `.local/disassembly/projectile-contact-vectors.json`.

The adapter regression checks both kinds against an obstacle at exactly 60 units and one fixed-point increment above that limit.
Both produce a selected hit at the boundary and a discarded hit above it.
This prepares collision integration; Smart Missile firing remains disabled until flight state and damage are connected.


## Playable Smart Missiles

The player loop now admits optional weapon 10 through the existing attempt and admission routines.
Launch calls `initialize_weapon_record` with the quantized height from the recovered local firing request.
It supplies owner 1, source kind 6, and the existing base lifetime of 75.
This produces the original launch offset, 300-tick deadline, initial vertical velocity, and elevated starting height.
Each shot keeps a `MissileGuidance` record through flight and snapshots.

Each nonzero movement update applies the owner-kind elapsed adjustment once, then guidance and planar movement.
The collision query receives unadjusted elapsed, geometry kind 34, and the exact raw missile height.
The fallback sweep interpolates missile endpoints rather than applying cannon velocity.
Missiles at zero height or outside arena bounds are removed; selected collisions still use the existing impact path.
The original ground-test membership table at CODE 30 +2aa0 includes kind 34.

The catalog now retains all weapon damage values as well as ammunition costs.
Missile impacts use slot-10 damage, preserving the existing vulnerability gate.
Central specifies ammunition cost 6, damage 3, and firing sound 10027.
The remaining primary and Scattershot damage behavior is unchanged.

The Rust test checks launch cost, sound, lifetime, ascent, transition, descent, movement, and resource-defined impact damage.
`scripts/verify-missile-game.mjs` compares Central level 7 at difficulty 0 between native and WASM after 32 ticks.
It also checks two firing gates, ammunition, launch state, sound, reset, and unsupported-weapon behavior.
All 111 Rust tests, strict Clippy, and missile, Seeker, Scattershot, and encounter parity checks pass.

The eleven-enemy participant mapping, zero enemy heights, modeled OS angle conversion, direction tables, and scheduling remain provisional.
Rendered height uses the existing scene conversion. Missile body geometry and effects still use preview projectile presentation.
The live query now uses `hit_projectile` for all supported player shots; the obsolete cannon-only convenience method was removed.
This supersedes the earlier pending-integration notes without extending the original-code oracle's stated scope.


## Original missile model in playable views

The owned Central, Eastern, and South shape-selection tables assign shape 10001 to geometry kind 34 in all three columns.
The catalog now retains that model for runtime missiles.
First-person rendering uses its original packed vertices, face planes, and polygon indices through the existing clipping and depth-buffer paths.
Object translation and face viewpoint both include the missile's original fixed-point height.
Chase and overhead views also draw the model with the projectile's position and heading.
The old shot marker remains only when the missile model is unavailable.

`render-missile-model` creates an isolated Central fixture with one missile after 24 ticks.
It renders all three cameras, checks that each differs from the missing-model fallback, and checks unchanged projectile state.
Run `cargo run --quiet --locked -p spectre-tools -- render-missile-model .local/extracted/central.bin .local/missile-render`.
The three PPM frames are local artifacts. The first-person frame was converted to PNG and visually inspected.
The isolated fixture uses diagnostic colors; it does not establish original missile color, orientation conventions, or camera fidelity.

All 111 Rust tests, strict Clippy, the WASM build, and missile, Seeker, and encounter parity checks pass.
No browser server was started. Browser interaction remains unverified for this rendering change.


## Composed retirement-to-blast sequence

`projectile_lifecycle_requests` now composes projectile retirement and area-request generation against the same mutable geometry records.
It returns the lifecycle action plus ordered damage requests. The caller must apply requests before deleting the record.
Movement remains a separate caller action.

`scripts/verify-blast-sequence.py` executes 512 multi-tick sequences through original CODE 30 +2bbe and the real +2aa6 area routine.
The original Pascal set helpers, planar distance, and integer division run unchanged.
Only movement, deletion, and terminal CODE 31 damage calls are captured.
The fixture scans one source record while twelve participant geometries remain available to the area routine.

A source deadline of 100 still permits movement at tick 99.
At 100, retirement changes the geometry kind, clears ownership, decrements the former owner's shot count, and sets deadline 160.
The one-tick sequence emits no area requests at 101, 102, or 103.
At 104, it invokes the original area routine and then deletion.
Alternate sequences skip to 102 and 104, checking that behavior depends on time rather than invocation count.

All emitted blast requests have attacker zero because the earlier retirement cleared the owner byte.
The area routine indexes damage using each target's stored low weapon byte.
These composed observations confirm earlier isolated-helper results. Do not restore the launch owner or substitute a flat grenade damage value.
Score and kill consequences still require the complete downstream damage-handler integration.

The comparison checks every source mutation, all twelve shot counters, request order, terminal deletion order, and unchanged target records.
The report is `.local/disassembly/blast-sequence-vectors.json`.
All 512 sequences, the 111 Rust tests, and strict Clippy pass.
Grenade and Cyberbomb live flight, retirement visuals, and damage application remain unfinished.


## Full ownerless enemy damage path

`scripts/verify-ownerless-damage.py` executes complete CODE 31 +0a84 calls for the blast's attacker-zero case.
The fixture uses single-player mode, local participant 1, nonlocal victim 2, a valid geometry index, and a nonzero enemy byte.
The concurrent movie flag is zero. Local-player damage, missing geometry, multiplayer, and movie notifications are outside this comparison.

The real type-31 readiness predicate executes through CODE 6 +197e.
Rust composes `participant_health_after_guard`, `damage_killed`, `damage_response_fields`, `death_burst_gate`, and `damage_sounds`.
Terminal sound, radial-burst, and fragment calls are captured with arguments and order.
Their internal allocation and presentation are not executed in this harness.

All 1,024 original calls match the composed Rust helpers, including noncanonical enemy flags and signed arithmetic extremes.
The original parameter block remains byte-for-byte unchanged, confirming no score or combat-counter updates on this path.
The handler clears the damage-grace global and records the supplied time as the latest enemy hit.
Type-31 protection suppresses subtraction until the final animation slot; existing negative health still clamps to zero.
On a kill, the usual response transition, eligible radial-burst request, fragment request, and death sound remain active.
Even protected nonfatal hits still run response-field and hit-sound handling.

The report is `.local/disassembly/ownerless-damage-vectors.json`.
Strict Clippy passes after adding the verifier composition.
Live blast integration must preserve ownerless attribution and these side effects instead of calling the current direct-shot scoring path.


## Grenade impact attribution differs from timer expiry

CODE 30 +294a..+29a6 dispatches weapons 1 and 17 to the area routine after a selected collision with a living enemy.
In single-player mode, it calls the area routine immediately.
In other modes, authority bit zero must be set. Nonzero authority with bit zero clear does not suffice.
`projectile_impact_uses_area` reproduces this decision after the caller establishes the collision and health conditions.

The area routine receives the existing projectile record. This path does not clear its owner first.
Thus direct enemy-impact blast requests retain launch ownership, unlike the delayed ownerless expiry blasts.
The earlier no-score finding applies to the ownerless damage-handler path, not all grenade or Cyberbomb explosions.
The exact resulting score still depends on the downstream damage-handler gates and target death.

Run `.local/re-tools/bin/python scripts/verify-blast-sequence.py --impact` for the composed impact comparison.
It executes original CODE 30 +294a..+2a26 with the real Pascal set helpers and area selector.
All 512 cases match Rust, including both special weapons, twelve launch owners, multiple game modes, and noncanonical authority bytes.
Terminal damage requests are captured. Their arguments and order, preserved source ownership, unchanged records, and stack balance are checked.
The report is `.local/disassembly/blast-impact-vectors.json`.

This comparison starts after collision selection and the positive-health check.
It does not establish scenery-impact retirement, contact rewind, deletion ordering after +2a26, or full multiplayer damage application.
Strict Clippy passes. Live grenade firing remains disabled until the distinct impact and timed paths are connected.


## Playable grenade integration

Optional slot 1 now uses the original admission and initializer paths with the current 75-tick base lifetime.
The initializer produces a 54-tick flight deadline, native launch offset, and quantized launch height.
The shot retains its original geometry record for lifecycle updates.
At expiry, `projectile_lifecycle` retires it and clears ownership. Four ticks later, the loop applies area requests and removes it.
Retiring shots no longer consume the derived active-shot capacity. The preview still has no original pool or stored owner-shot counters.

A selected living-enemy impact uses `projectile_impact_uses_area` and applies area damage immediately, before later shots update.
The source keeps its launch owner on this path. Dead targets are removed from the active collision query after the blast.
The fallback collision path uses the end-of-step position for the area source; its sampling remains provisional.
Scenery and boundary contacts still use the existing preview removal behavior. Original contact rewind and retirement presentation remain unfinished.

The area adapter maps player slot 1 and eleven stable target slots into original records.
Dead targets use kind 10; retired targets are inactive. Enemy heights remain zero.
Target weapon indices select the resource damage bank, which starts identically to the player bank in the recovered loader.
The loop applies protected-health subtraction, hit sounds, damage counters, kill counts, and eligible score awards.
Ownerless requests produce no player-hit or score credit. Immediate player-owned blast kills can produce both.
Original response deadlines, radial death bursts, fragment creation, and complete victim lifecycle side effects remain unintegrated.
The existing enemy respawn and mission adapters still control those parts of gameplay.

The live regression checks launch cost, sound, lifetime, four-tick retirement delay, target-indexed damage, impact attribution, and protected targets.
`scripts/verify-grenade-game.mjs` checks Central level 3 firing at ticks 16 and 32, cost, sound, reset, and native/WASM parity.
All 112 Rust tests, strict Clippy, the WASM build, and grenade, missile, Seeker, Scattershot, and encounter parity checks pass.
Hosting remains stopped. No hosted browser interaction was performed.


## Conditional projectile contact retirement

CODE 25 +2d04..+2d26 derives A5-3cf7 from the word at A5-7b94.
Machine types 1, 2, 3, and 8 produce zero. Other signed values produce one.
These identify the Macintosh 512Ke, Plus, SE, and Portable.

CODE 3 +0818..+0826 requests SysEnvirons version 1 with its output record at A5-7b96.
The jump-table call reaches CODE 1 +2994; its available-trap path invokes trap A090 at +29b4.
The machineType word is at record offset 2, which resolves to A5-7b94.
The adjacent systemVersion and processor reads at CODE 3 +082e and +084c agree with this layout.
Apple documents this layout in the [Gestalt Manager reference](https://leopard-adc.pepas.com/documentation/mac/pdf/Operating_System_Utilities/Gestalt_Manager.pdf).
The gate depends on hardware identity. A performance optimization is a plausible explanation, but the executable does not establish intent.
`projectile_contact_retirement_enabled` reproduces the lookup.

CODE 30 +2676 deletes the projectile immediately when that flag is zero.
Otherwise, it invokes movement with negative min(elapsed low signed word, 2).
The word negation wraps before sign extension. This matters for the minimum signed word.
Movement receives the existing owner and object ID before any retirement writes.

After movement, the routine sets deadline to current global time plus 60.
It copies the old kind to byte +15, sets kind 10, decrements the nonzero owner's shot count, and clears ownership.
`retire_projectile_contact` reproduces the response with movement supplied as a callback.
The callback must implement original movement semantics; this helper does not itself establish reverse displacement or grid updates.

`scripts/verify-contact-retirement.py` compares 1,024 composed flag-initialization and contact-response executions against Rust.
The original signed minimum helper executes unchanged.
Movement is captured and supplies a coordinate mutation to check that retirement preserves its result.
Deletion is captured separately. The harness checks callback arguments and ordering, all record bytes, all owner counts, and stack balance.
The report is `.local/disassembly/contact-retirement-vectors.json`.

All comparisons, 112 Rust tests, and strict Clippy pass.
Live scenery impacts still delete the grenade. Native hardware policy, full movement composition, and caller contact ordering remain pending.
Do not enable retirement unconditionally from the presence of a grenade weapon alone.


## Projectile boundary and ground priority

`projectile_environment_contact` reproduces CODE 30 +2788..+2814 and the complete boundary routine at +261e.
The caller first moves the projectile, then queries collisions using the saved previous pose.
The environment gate reads the resulting position and selected hit ID.

Coordinates outside the inclusive interval [0, arena extent] trigger boundary contact.
Kinds 34 and 46 trigger ground contact when signed height at +28 is zero or negative.
Kinds 43 and 44 instead use the signed field at +24.
Other projectile kinds have no ground-height test in this block.
A positive selected hit takes priority over both boundary and ground contact.
Only a nonpositive hit ID permits this block to call contact retirement at +280c.

`scripts/verify-projectile-environment.py` checks 2,048 original-code executions against Rust.
The original boundary routine runs unchanged; no service calls are replaced.
The harness supplies the post-movement record, arena extent, and collision result.
Cases cover all nine admitted projectile kinds, exact boundaries, signed extremes, height fields, and hit priority.
All record bytes and the stack remain unchanged.
The report is `.local/disassembly/projectile-environment-vectors.json`.

This verifies the environment gate, not the complete collision caller.
Selected-object dispatch, reverse movement composition, and live retirement integration remain pending.


## Projectile selected-object routing

`projectile_object_contact` reproduces CODE 30 +2814..+28ee.
A nonpositive selected hit produces no object response.
The first set excludes kinds 5, 10, 12, 14, 16–19, 23, 24, 30, 36, 37, and 39–42.
This exclusion precedes participant classification.

Kinds 6–8, 13, 20–22, 26, 27, 31, 35, and 45 select the participant branch.
Projectile kind 28 also selects that branch for target kind 25.
Its additional set contains kinds 23 and 24, but the earlier exclusion already removes them.
When participant health is nonpositive, the caller skips damage and selects projectile deletion.
Positive health proceeds to the damage dispatch at +28ee.
Other target kinds select the object-response call at +2a1c, then contact retirement at +2a52.

The object-response entry, CODE 31 +10b0, immediately branches from +10b6 to the epilogue at +10ec.
The apparent hit-counter and damage code at +10b8..+10e8 is unreachable from this entry.
Do not implement scenery damage from that skipped body.

`scripts/verify-projectile-object-route.py` compares 11,520 original routing executions against Rust.
Cases cover all 256 target-kind bytes, nine projectile kinds, nonpositive hit IDs, and participant health boundaries.
The harness stops before damage dispatch and checks geometry, participant records, and stack preservation.
It also executes all 6,126 selected object-response calls through their complete original entry and return.
Those calls leave the geometry and participant records unchanged. No services are replaced.
The report is `.local/disassembly/projectile-object-route-vectors.json`.

The helper selects the next branch; it does not execute damage, deletion, or retirement.
Live integration still requires the full movement and response sequence.


### Live ignored-contact integration

Player shots now use the recovered object exclusions after the original collision query selects a hit.
An ignored hit preserves the completed movement and the projectile. It does not create an impact or apply damage.
This branch also precedes boundary and ground handling, as in the original caller.
The provisional collision fallback applies the same object exclusions to scenery and targets.
Its collision sampling and selection order remain provisional.

The regression `projectile_contacts_preserve_ignored_surfaces` exercises both collision paths.
Moving acid, moving mud, and Flow Fields preserve shots, target health, and the absence of impact effects.
Control cases verify that walls still stop shots and ordinary tanks still take damage.
All 113 Rust tests, strict Clippy, the WASM build, and five native/WASM gameplay suites pass.
The suites cover Grenades, Smart Missiles, Seekers, Scattershot, and encounters.

Only the ignored-contact branch is integrated here.
Complete object-response retirement, damage dispatch, and reverse movement composition remain pending.


## Composed grenade contact movement and live retirement

`scripts/verify-contact-retirement.py --flight` executes the complete contact response and its original movement call.
The 1,024 cases use kind 9 with weapons 0, 1, 3, 4, 5, and 17.
The original direction lookup and software multiplication execute with a supplied, deliberately irregular direction table.
The harness captures terminal grid relocation and deletion. It checks old/new poses, call order, all record bytes, owner counts, and stack balance.
Zero elapsed skips movement and grid relocation, but retirement still changes the record.
Signed elapsed extremes exercise the low-word minimum and wrapping negation.
The report is `.local/disassembly/contact-retirement-flight-vectors.json`.
This check does not establish the original SANE-generated table or execute the grid relocation service.

Live player grenades now use this response for scenery and boundary contacts.
The native adapter chooses the enabled effect path, without the original old-Mac hardware restriction.
Movement starts from the completed step endpoint, then reverses by the recovered elapsed amount.
Retirement clears ownership and releases the active-shot slot.
The existing lifecycle executes the area blast four ticks later.
Direct participant impacts retain their separate immediate-blast path and launch attribution.

Both collision paths pass the wall-contact regression: position rewind, no immediate impact, delayed damage, zero player-hit credit, and zero score.
The fallback still uses provisional collision samples; the recovered-query adapter retains its preview record pool.
The native renderer still lacks the original retirement animation. Other projectile types still need contact-retirement integration.
All 113 Rust tests, strict Clippy, the WASM build, and five weapon/encounter parity suites pass.


## Retired-projectile world visibility correction

The earlier assumption that retirement needs a visible world animation was too broad.
CODE 6 dispatches kind 10 through its table at +1be2 to +26d0.
That branch prepares color and age-dependent fields, and can request radar glyph 39 through +17f6.
However, +288c..+2894 explicitly excludes kind 10 before the world-model queue append at +29e6.
The age-dependent fields alone do not prove that the original draws a world animation.

`scripts/verify-retired-render.py` executes 512 complete paths from the kind dispatch to the per-object loop tail.
The original min/max helpers run unchanged. The harness supplies projected coordinates and captures the terminal radar request.
Cases vary retirement age, previous kind, effect flag, and view argument.
Every path reaches the kind-10 exclusion and leaves the queue count and queue storage unchanged.
The object record and stack remain unchanged. The report is `.local/disassembly/retired-render-trace.json`.
This is an original-code trace, not a full rendered-image comparison.

The first-person renderer now omits retired grenade markers while the lifecycle waits for the blast.
A pixel regression compares active, retired, and absent shots after wall contact.
The retired frame matches the absent-shot frame; the active marker changes pixels.
The existing delayed-blast regression still passes in both collision paths.
Exterior and overhead markers, radar glyph integration, and blast visuals require separate recovery.


## Retired-projectile radar integration

`radar::retired_glyph` reproduces the request gate at CODE 6 +26d0..+271e.
The request uses glyph 39 when the signed view argument is below 2 and the kind-10 radar color is not 13.
The request color comes from the kind-10 body-color table, not its radar-color table.
The admission byte at local -f comes from the radar-color comparison at +1aa4..+1ab8.
Previous kind and retirement age do not change this request.

The retired-render harness now varies both color fields and signed view values.
It compares 512 original request results against Rust in `.local/disassembly/retired-radar-vectors.json`.
The original world-model exclusion remains checked on every path.
The harness supplies the admission byte from the observed color comparison; it captures the drawing service.

The shared radar renderer now projects retired shots and emits glyph 39 through the existing glyph, disturbance, and color queues.
A pixel regression verifies body-color output, radar-color suppression, and marker removal when the shot disappears.
The native adapter still uses its preview entity order rather than the original compact object-pool order.
Original exterior and overhead world rendering remain separate recovery targets.


## Live cannon and Scattershot contact retirement

The verified kind-9 movement composition now also handles player weapons 0, 3, 4, and 5 after scenery or boundary contact.
These rounds reverse movement, clear ownership, release their active-shot slot, and enter kind 10.
Four ticks later, the recovered lifecycle deletes them without area damage.
First-person suppression and retired radar markers apply through the shared state.
Participant impacts retain their separate immediate damage and deletion path.

`Shot::lifecycle_record` replaces the grenade-specific field name.
Ordinary rounds construct the required lifecycle fields when contact occurs.
This is a typed gameplay adapter, not recovery of the original allocation or compact geometry pool.
Seekers, Smart Missiles, bouncing rounds, and enemy shots still need their own movement/retirement composition.

The collision regression covers weapons 0, 3, 4, and 5 in both collision paths.
It checks ignored surfaces, wall rewind, cleared ownership, three waiting ticks, fourth-tick deletion, and absence of an area blast.
An older preview test now observes retirement rather than requiring a synthetic impact flash.
All 113 Rust tests, strict Clippy, WASM build, 112-arena replay checks, grenade/Scattershot parity, and encounter checks pass.


## Seeker contact retirement composition

`scripts/verify-seeker-contact.py` executes CODE 30 +2676 and the complete movement routine at +1d54 for weapon 9.
The original target selector, distance approximation, angle wrapping, direction lookup, and multiplication execute unchanged.
The harness supplies angle results and a deliberately irregular direction table. It captures terminal grid relocation.
All 1,024 cases match Rust record bytes, participant shot counts, ordered angle arguments, relocation poses, and stack balance.
The report is `.local/disassembly/seeker-contact-vectors.json`.

A nonzero reverse elapsed value runs target selection and heading correction before reverse translation.
A zero reverse elapsed value skips guidance and grid relocation. Retirement still clears ownership afterward.
Thus reversing the previous displacement alone does not reproduce Seeker impact movement.

Live player Seekers now compose these recovered operations after scenery and boundary contacts.
The adapter refreshes the projectile heading and direction before retirement, using the existing modeled angle and direction services.
The collision regression now includes weapon 9 in both collision paths.
All 113 Rust tests, strict Clippy, WASM build, and Seeker/grenade/encounter parity checks pass.
The full original grid service, participant pool, and operating-system angle conversion remain outside this verification.


## Smart Missile contact retirement composition

`scripts/verify-missile-contact.py` executes the complete original retirement and kind-34 movement routines.
Its 1,024 cases cover ascent, descent, target heights, owner-dependent elapsed adjustment, missing owner geometry, and signed integer limits.
Original selection, distance, wrapping, direction lookup, multiplication, and division execute unchanged.
The harness supplies angle results and a direction table, and captures terminal grid relocation.
Rust matches all projectile bytes, participant shot counts, ordered angle requests, relocation poses, and stack balance.
The report is `.local/disassembly/missile-contact-vectors.json`.

Live player Smart Missiles now run this composition for scenery, boundary, and ground contacts.
Reverse movement can change altitude, vertical velocity, and heading before ownership clears.
The existing lifecycle deletes the retired missile four ticks later without area damage.
Direct participant hits retain their separate damage path.

The regression checks wall and ground contacts in both collision paths.
It verifies reverse displacement and altitude, kind 10 with previous kind 34, cleared ownership, and fourth-tick deletion.
The first-person model list excludes retired missiles. Exterior and overhead model behavior remains provisional and unchanged.
Modeled angle/table generation and preview participant/grid adapters remain implementation limits.


## Enemy cannon and Scattershot retirement

Enemy weapons 0, 3, 4, and 5 now use the shared contact-retirement helper after wall or boundary contact.
The lifecycle adapter derives the original participant ID from the stable enemy slot instead of assigning player ownership.
Only the represented participant IDs 2–12 use this path. Additional preview slots retain the prior response.
The original movement/retirement comparison already covers all twelve valid owner IDs.

Retired enemy shots no longer count against their owner's active-shot limit.
Their records remain for four ticks, then the recovered lifecycle deletes them without area damage.
The enemy collision fallback now applies the recovered scenery exclusion set before treating surfaces as walls.
Its interpolated geometry selection and player hit box remain provisional.
Direct player hits retain their existing damage and deletion path.

The new regression covers all four weapon slots, Flow Field pass-through, wall rewind, and cleared ownership.
It fills one enemy's twenty-shot limit and verifies rejection before contact, then admission while the retired record still exists.
It also checks fourth-tick removal, no area effect, and unchanged player health.
Enemy bouncing and guided rounds still need retirement integration.


## Bouncing-round contact retirement

`scripts/verify-contact-retirement.py --bouncing` executes the original hardware gate, contact response, and complete kind-29 movement.
The original direction lookup, multiplication, division, minimum, and bounce branch execute unchanged.
A supplied irregular direction table tests component order and arithmetic. Grid relocation and deletion remain captured terminal calls.
All 1,024 cases match Rust across signed elapsed, height, velocity, deadline, and owner-count boundaries.
The report is `.local/disassembly/contact-retirement-bouncing-vectors.json`.

Reverse movement reapplies vertical integration and bounce rules; it is not restoration of a saved height.
Zero reverse elapsed skips movement, as in the original dispatcher.
The existing flight and captured-movement suites also pass after the harness extension.

Enemy weapon-6 wall and boundary hits now use this composition before retirement.
The original kind and vertical fields remain available in the lifecycle record.
The active-shot slot is released immediately; deletion occurs four ticks later without an area blast.
The regression checks reverse height and velocity, horizontal rewind, ownership clearing, and shot-capacity timing.
Enemy collision sampling and vertical rendering scale remain preview adapters.
All 114 Rust tests, strict Clippy, and the WASM build pass.
