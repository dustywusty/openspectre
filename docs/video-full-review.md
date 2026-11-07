# Full gameplay reference review

Source: [Spectre VR, supplied by the user](https://www.youtube.com/watch?v=b0X74Oe80tg).
Reviewed on 2026-11-07. The recording lasts 21:34 and reaches level 14.
The visible title identifies Spectre VR CD and Central Systems Matrix. The exact executable revision remains unverified.

The uploader states that they omitted QuickTime sequences because those crashed their emulator.
This omission limits conclusions about movie-triggered events, including type-31 activation.
The opening also shows a custom vehicle selection. Comparisons must account for its settings.

## Review method and local artifacts

This review inspected still samples across the complete recording, with denser samples around selected events.
It was not continuous audiovisual playback. Exact input timing, sounds, and brief unsampled events remain unverified.

- Full local recording: `.local/reference-video/full-gameplay.mp4`.
- Offline video player, timestamp links, and contact sheets: `.local/reference-video/full-index.html`.
- Timestamp manifest, source description, and video hash: `.local/reference-video/review-manifest.json`.
- Reproducible extraction: `python3 scripts/index-video-reference.py`.

The four overview sheets sample every 30 seconds, beginning at 0:00, 6:00, 12:00, and 18:00.
Additional sheets cover vehicle selection, level transitions, combat, replenishment, respawn, and the final loss.
Frames run left to right, then down. The manifest records each frame's exact requested timestamp.
Video and extracted images remain local and ignored by Git. The review requires no web server.

## Direct observations

| Time | Visible evidence | Consequence for the port |
| --- | --- | --- |
| 0:00–0:06 | Central campaign selection, VR CD title, and vehicle menu. Custom is selected with Speed 6, Shields 6, Ammo 3. | The footage does not use our balanced preset. Match settings before comparing handling or capacity. |
| 0:04–0:06 | Balanced shows 5/6/4; Speedy shows 8/4/3; Strong shows 3/8/4. | These provide independent visual checks for the recovered preset table. |
| 0:08–0:16 | Three lives, 30 ammo, a 60-second timer, level introduction, and a weapon icon. | Startup includes lives, vehicle capacity, timer, hint, and weapon state. |
| 0:16–0:28 | Solid yellow scenery, colored objects, orange ground features, dotted ground, and a magenta horizon. | The first arena is not monochrome. Material and level palette recovery are necessary. |
| 0:57–1:01 | A nearby green object and flag appear before the results screen. Score changes from 135 to 935 across the transition. | Completion awards additional score. These samples alone do not prove the objective or bonus formula. |
| 1:03–1:13 | Results precede a new introduction. Level 2 begins with cleared damage, 30 ammo, a different weapon icon, and a 70-second timer. | Round completion is a sequence of states, not an immediate arena reset. |
| 4:00–4:30 | Lives rise from three to four while the score crosses 5,000. | An extra-life threshold is a hypothesis for code tracing, not yet a verified formula. |
| 5:30 | Level 6 introduction warns about a named enemy class. | Hints and changing enemy types are part of campaign progression. |
| 6:30 | Results include separate quickness, accuracy, and avoidance ratings. | Ratings require their own recovered formulas and presentation. |
| 7:20–7:42 | Red-and-gray enemy craft, green objects, flag poles, pink scenery, and triangular fragments are visible. The weapon icon differs from early levels. | Recover per-type colors, weapons, moving enemies, and destruction fragments. |
| 7:39.07–7:39.33 | Ammo changes from 16 to 30 and the damage bar clears. Lives remain four and the level remains seven. | Replenishment can occur within a level without consuming a life. The trigger remains unconfirmed. |
| 8:00 | The H meter is low and red; it is green and full in many other samples. The separate F meter also changes. | These are independent HUD states. Do not label them health or fuel without tracing their meaning. |
| 9:00–11:30 | Different pink geometry, white ground polygons, several nearby craft, and red fragments appear. | Later encounters need more than static tanks and blue obstacles. |
| 12:30–14:30 | Green horizon bands and purple scenery replace earlier palettes. Weapon icons change again. | Palette and weapon selection vary across the campaign. |
| 13:40–13:50 | Level 11 begins with a 330-second timer and a hint about a spreading weapon. | Timers are level data; later levels differ substantially from the opening minute. |
| 14:53–14:55 | Lives drop from two to one. Damage clears and ammo refills to 30; level 11 and score 16,855 persist. | Respawn preserves progress and resets vehicle condition. It does not restart the whole encounter. |
| 17:30–20:30 | The horizon changes to a brighter green band. Later combat continues through levels 13 and 14. | The renderer needs campaign palettes, not one fixed magenta background. |
| 21:02–21:13 | Ammo remains zero while scenery and timer continue to change. | Empty ammunition must not end the round. This contradicts the former encounter loss rule. |
| 21:14–21:22 | Lives reach zero, the world view gives way to a small wireframe object, and a name-entry dialog appears. | Final death has a separate transition and score-entry flow. |
| 21:30 | High scores show level 14 and score 26,350. | Persisted high scores belong to the completed game flow. |

## Presentation targets

The HUD uses thin green vector-style lettering against black, with outlined damage and ammunition bars.
The damage bar fills red as damage rises; it is not a conventional remaining-health bar.
The upper-right weapon box changes its line drawing between levels and sometimes within a level.
A cluster of colored points above the right horizon is consistent with radar, but its projection remains unverified.
The aiming marks stay near the horizon, with the central vertical mark above it.

The footage shows large triangular fragments crossing the foreground after nearby combat.
Enemies have recognizable red and gray components. Scenery colors change between level groups.
The low camera and dotted ground provide motion cues without a wireframe grid.
The playable renderer still uses diagnostic face colors and a provisional camera.
Its new depth buffer fixes visibility errors but does not reproduce the original renderer.

## Behavioral limits and priorities

The footage does not establish exact acceleration, random coasting, collision padding, firing cadence, or AI probabilities.
Camera motion cannot by itself distinguish player input from enemy steering or an effect.
A visual disappearance does not by itself identify a kill, pickup, or level-completion trigger.

The next implementation priorities supported by this review are:

1. Recover replenishment and flag interaction from the executable; correlate their objects with the footage.
2. Recover death and respawn while preserving timer, score, and surviving world state.
3. Connect original wave sequencing, timers, hints, palettes, and weapon selection.
4. Replace provisional AI with the original participant behavior dispatcher and movement policies.
5. Reconstruct the HUD, radar, fragments, results, and high-score flow.

The zero-ammo loss condition has been removed from encounter mode.
A regression test confirms that movement and simulation continue with empty ammunition.
Replenishment and lives are still pending, so restarting remains the preview's way to refill ammunition.

## Manual follow-up

The [original manual review](manual-reference.md) resolves the H/F meter names and confirms three playable camera modes, directional Flow Fields, ice, and Cybermud. It also distinguishes white full-refill dumps from green gradual-refill dumps. These descriptions narrow the research targets; they do not establish the exact object collected at 7:39 or executable handling constants.

## Results-screen cross-check

The first-transition contact sheet was inspected again after recovering CODE 21's rating code.
The results read Not Bad / Not Bad / Radical for Quickness / Accuracy / Avoidance.
Disc STR# 1010 maps these to ratings 3 / 3 / 4.
The recovered awards are 200 / 200 / 400, totaling 800.
This exactly matches the visible 135 to 935 score change.
The sampled footage does not establish the precise launch/hit counters used to calculate those ratings.


## Pixel check against recovered aiming marks

The local frame-01.png is 640 by 480 pixels.
Bright horizontal segments run from X 265 through 280 and 360 through 375, at Y 321.
The vertical segment runs from Y 266 through 281, at X 320.
These inclusive endpoints match the executable's aiming marks around center [320,321], with no scale change.
The check used an RGB threshold above 160 in all channels around the three marks.

The recovered screen-layout formulas predict center [320,280] for a zero-origin 640-by-480 screen.
The recording's actual display configuration and any capture transformation remain unverified.
Do not infer a new camera offset from this frame alone.


Further tracing distinguishes immediate surface placement from deferred viewport placement.
With zero-origin 640-by-480 screen bounds, compact immediate presentation predicts center [320,320].
The deferred path predicts [320,280]. The video still measures [320,321].
The immediate path therefore explains 40 pixels of the discrepancy under these assumptions.
The active branch, port translation, and remaining one-pixel difference are not established by the frame.
