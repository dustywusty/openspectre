# OpenSpectre

**Drive. Dodge. Collect the beacons. Pew pew.**

A Rust tank game that runs on your desktop and in your browser through WebAssembly.
The bundled **Signal Frontier** pack includes 12 arenas, three zones, new models, and synthesized sounds. No original disc is needed.

![Chase view: cyan player tank facing enemy tanks and arena cover](docs/images/chase.png)

| Cockpit | Overhead |
| --- | --- |
| ![First-person view across Crosscurrent](docs/images/cockpit.png) | ![Overhead map of tanks, beacons, and cover](docs/images/overhead.png) |

*Actual WASM renderer captures from the bundled opening arena. Browser HUD overlays are not shown.*

## Play

Install Rust 1.88 or newer, Node.js 22 or newer, and Python 3.

```sh
rustup target add wasm32-unknown-unknown
npm ci
npm run build
npm start
```

Open **http://localhost:5174/#drive** and click **Drive tank**.

| Action | Control |
| --- | --- |
| Drive and steer | WASD or arrow keys |
| Fire cannon | Space |
| Brake | Shift |
| Select / fire optional weapon | Q / E |
| Change view | Tab |
| Fullscreen | F |
| Reset / pause | R / Escape |

Click the arena to enable sound and take control. Single-player pauses when it loses focus.
The **Infinite health** checkbox is off by default.

For a native desktop window using the same Rust simulation:

```sh
cargo run --locked -p spectre-tools --features desktop -- drive assets/open/generated/central.bin --colors assets/open/generated/spectre.bin
```

Desktop audio is not implemented. The browser is the primary play surface.

## Bring a friend

Select **Multiplayer**, create a room, and share the invitation link. The other player joins automatically.
First to five kills wins. To play across computers, host a reachable server or use a tunnel before creating the room.
See [multiplayer setup](docs/multiplayer.md).

## Use your classic disc

OpenSpectre also loads resources extracted from your own Spectre VR disc.
After the build above, on macOS:

```sh
brew install hfsutils
python3 scripts/extract-disc.py /absolute/path/to/SpectreVRCD_1_1.iso
npm run build:audio
npm run start:classic
```

Stop any running server before switching packs, then refresh the browser.
Original assets stay in the ignored `.local/` directory and are not included in this repository.
See [asset packs](docs/asset-packs.md) for pack sources and loading details.

This is a playable reconstruction under development. Recovered routines inform the simulation, but some behavior remains approximate.
Signal Frontier has new arenas and artwork; classic mode loads original resources. Neither mode runs the Macintosh executable.
The [recovery notes](docs/disassembly.md) document the evidence and remaining gaps.

## Build evidence

```sh
cargo test --workspace
npm test
npm run test:open
npm run test:open-network
```

With FFmpeg installed (`brew install ffmpeg` on macOS):

```sh
npm run test:open:record
```

Recordings and screenshots appear in `artifacts/playthrough/`. The test records cannon combat, enemy damage and death, and beacon collection through mission completion.
The navigation test uses infinite health, labeled in the video. Captures contain actual WASM frames at 30 fps; browser HUD and audio are not captured.

[CI](https://github.com/dustywusty/openspectre/actions/workflows/standalone.yml) uploads this evidence and links it in the build summary. Same-repository pull requests also get an updated recording comment. Fork pull requests use the build summary.
Downloads require GitHub sign-in and remain available for 30 days. Check the build result: failed tests may leave partial recordings.

## Explore

- [Asset pack source and classic loading](docs/asset-packs.md)
- [Multiplayer architecture and rules](docs/multiplayer.md)
- [Native port](docs/native-port.md)
- [Disassembly and recovered behavior](docs/disassembly.md)
- [Development history, resource tools, and emulator reference](docs/development-history.md)
