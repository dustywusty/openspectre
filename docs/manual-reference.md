# Original manual: cameras and ground effects

Reviewed 2026-11-07. Source: [Spectre VR User’s Guide scan](https://d2.xp.myabandonware.com/f/ljit/Spectre-VR_Manual_Mac_EN.pdf), linked under the Mac edition on [My Abandonware](https://www.myabandonware.com/game/spectre-vr-20h).
The guide covers both Mac and PC. Its descriptions are primary design evidence; exact CD 1.1 constants still need executable verification.

Local scan: `.local/reference-video/Spectre-VR_Manual_Mac_EN.pdf`.
OCR: `.local/reference-video/manual-ocr.txt`. OCR is only a search aid. PDF pages 21 and 26 were visually checked.
Page numbers below are printed pages; PDF positions are three pages later.

## Cameras — page 23 (PDF 26)

Three playable views: interior, exterior, and overhead map. Tab or keypad * cycles them in that order. Plus/minus zoom exterior and map views. Exterior omits the aiming crosshair; map omits the separate radar. This confirms that the overhead view is an active gameplay mode.

The port now has first person, diagnostic chase, and a playable overhead map. Browser and desktop share the Rust renderer. Tab/keypad * cycles views, and plus/minus zooms the latter two. The overhead map uses original top-view line geometry where present, otherwise projected mesh edges. Camera position, projection, zoom limits, colors, and map rotation remain provisional. The map currently follows the tank with a fixed north-up orientation. It omits both crosshair and separate radar.

## Ground effects — page 18 (PDF 21)

- Flow Fields: usually light-blue arrows that push the craft in their direction. These are a plausible match for the user's speed-pad recollection, not a confirmed identification of a particular video object.
- Ice: white crystalline squares; sliding and loss of turning ability.
- Cybermud: gold hexagons that slow the craft. Later variants move and attack.
- Acid: damage on contact; later variants can move.
- Force Fields: impassable surface lines, but sufficient flight height permits crossing.
- Anti Grav Packs: grant flight energy, tracked by the F meter.

Do not substitute generic acceleration boosts for Flow Fields. Their force, contact bounds, duration, and interaction with player input need disassembly evidence.

This page also distinguishes gradual green ammo/repair dumps from white dumps that restore both completely. The latter may explain the same-level refill observed around 7:39 in the supplied video; the exact collected object is still unconfirmed.

## Other useful evidence

Page 23 identifies H as Hyperspace energy. This resolves the prior video review's uncertain H/F labels.

The disc's VRchitect help, in `.local/extracted/vrchitect.bin` (MacRoman), describes its grid editor as matching Spectre's bird's-eye view. Its “Placing Flags” section explains that Spectre converts the first entries of an arena's obstacle list into flags. The count varies by level. Thus raw obstacle placement alone is insufficient to reconstruct a playable campaign arena.

The original disc README says Shotgun's concurrent videos disable enemy weapons while allowing enemies to track the player. This is another reason to distinguish the supplied video's omitted movie sequences from the original event schedule.

## Next recovery targets

1. Trace original camera selection, orientation, and zoom state to replace provisional projection parameters.
2. Recover remaining ground-effect scheduling, moving mud, and collision-response ordering.
3. Recover remaining ammo-dump creation behavior in the world loader.
4. Match refill types and H/F behavior against the executable and footage.

The subsequent camera implementation changes rendering and view controls only. All 112 arenas passed WASM view/zoom and simulation-preservation checks. Desktop/WASM replay parity and encounter checks pass. A Central map frame was rendered offline and inspected; browser interaction remains unverified while hosting is stopped.

## Ground-effect recovery update

The port now uses recovered contact handling for ice (kind 17), static Cybermud (kind 18), and Flow Fields (kind 39).
Flow Fields add directional displacement. Their behavior is not a generic forward-speed multiplier.
The direction table still uses modeled SANE operations. Surface scheduling and solid-response ordering remain provisional.
See README.md for the current implementation limits.

## Additional overhead-view reference

The [Swords and Software gallery](https://www.swordsandsoftware.com/svr.php) includes a [top-down gameplay screenshot](https://www.swordsandsoftware.com/images/svr03.png).
Reviewed on 2026-11-07. Local copy: `.local/reference-video/swords-top-down.png`.

The visible DOSBox title identifies this as DOS footage, not the owned Mac CD build.
The image shows an overhead arena, visible tank bodies, ground dots, walls, and the status display.
It has no separate radar or aiming crosshair. This agrees with the manual's description of the playable map view.
A single frame cannot establish camera tracking, map rotation, zoom behavior, or movement timing.
Do not use its pixel coordinates or colors as exact Mac rendering constants.

The user's ground-speed recollection remains consistent with Flow Fields, but the specific object is still unidentified.
Use the manual and recovered kind-39 contact handler for directional force behavior.
