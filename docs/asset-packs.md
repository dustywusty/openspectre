# Asset packs

OpenSpectre has two server modes. Both use the same Rust simulation and WASM module.

| Mode | Command | Content |
| --- | --- | --- |
| Bundled | `npm start` | Signal Frontier replacement assets, included in Git |
| Classic | `npm run start:classic` | Assets extracted from your own Spectre VR/CD image |
| Original executable | `npm run start:emulator` | Separate Macintosh emulator reference with your disc image |

Stop the server before changing modes. Refresh each browser after the restart. Existing multiplayer rooms do not survive a restart.
The `/pack.json` endpoint identifies the active pack. Both players in a room use that pack.
Classic mode does not fall back to bundled assets when a file is missing.

## Signal Frontier

The replacement pack contains four zones with four arenas each:

- **Neon Foundry:** Crosscurrent, Switchyard, Prism Court, Citadel.
- **Glass Expanse:** Mirror Lake, Long Division, Frozen Compass, Refraction.
- **Ember Circuit:** Hot Junction, Furnace Row, Overpass, Last Light.
- **Midnight Circuit:** Violet Grid, Neon Arcade, Ghost Network, After Hours.

Each zone has 53 model entries. These include tank variants, cover, supplies, hazards, transit markers, projectiles, and Aegis animation stages.
The pack includes new palettes, radar symbols, enemy parameters, weapon costs, score thresholds, and campaign sequences.
Its 48 sound effects use synthesized tones and deterministic noise.

These assets are new designs. They do not copy disc meshes, arena placements, graphics, or sound recordings.
Resource numbers identify engine interfaces. Matching a resource number does not mean its contents came from the original game.
The classic recovery code remains in the engine, with its documented approximations.

The bundled pack uses full-frame solid first-person and chase rendering, plus overhead rendering.
Classic mode retains its existing projection research path. Renderer selection follows the archive's `opsp` resource marker.

## Edit the pack

Edit `assets/open/pack.json` to change model dimensions, model families, arena placements, palettes, enemy balance, or weapon costs.

Arena object rows use `[kind, x, z, heading_degrees]`. Coordinates are in the 1024-unit world.
The six `flag_sites` precede arena obstacles. The shared mission rules select active beacons from these slots.
Keep routes between beacons open. Tank spawns and enemy placements still use the existing engine's placement rules.

The geometry builders, radar drawings, sound synthesis, and resource encoder live in `scripts/build-open-assets.py`.
They use Python's standard library. The generator does not read `.local`, a disc image, extracted resources, or network data.

Regenerate the pack:

```sh
npm run build:assets
```

The generated files stay in `assets/open/generated/` and are included in Git.
`manifest.json` records hashes for every generated archive and sound. Rebuilding the same source produces identical files.
The assets use the resource format already supported by the engine. This keeps desktop, browser, and server data consistent.

The asset files in this directory have their own [license](../assets/open/LICENSE).
That license does not apply to original Spectre VR/CD content or imply rights to distribute a disc image.

## Build and test without a disc

```sh
npm ci
rustup target add wasm32-unknown-unknown
npm run build
npm run test:open
npm run test:open-network
```

The pack test checks every model, arena, camera mode, palette, radar atlas, and sound file.
It also checks enemy combat, death, beacon collection, score, campaign advancement, and a complete deathmatch.
A temporary installation without `.local` must serve every browser asset and create a multiplayer room.
The network test checks two-player control, input acknowledgements, room limits, rematches, disconnects, and replacement players.

## Classic mode

Classic mode requires all four extracted archives and the exported sound files:

```sh
python3 scripts/extract-disc.py /path/to/SpectreVRCD_1_1.iso
npm run build:audio
npm run start:classic
```

The disc image and extracted files stay out of Git. The extraction script checks that the source image remains unchanged.
Classic assets are served only when classic mode is selected.
A public classic server sends those selected assets to connected browsers, as the earlier prototype did.

The original executable remains separate from the native port:

```sh
SPECTRE_ISO=/path/to/SpectreVRCD_1_1.iso npm run start:emulator
```

Open `http://localhost:5173` for that reference. The native game uses port 5174 by default.
