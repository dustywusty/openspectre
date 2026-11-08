# OpenSpectre desktop client

The desktop client runs the same Rust game core as the web playground.
It includes all 12 Signal Frontier arenas, 48 sound effects, and its interface font.
It does not require a server, a browser, Node.js, or an original game disc.

## Play

On macOS, open **OpenSpectre.app**. You can move the app to Applications.
On Windows, extract the archive and open **spectre-client.exe**.
On Linux, extract the archive and run **spectre-client**.
Linux requires a graphical desktop, OpenGL, and ALSA.

Choose **Single player**, select an arena, and choose **Deploy**.
Drive through every beacon to complete the arena. You start each arena with three lives.
The results screen offers another arena or a new run.
Each arena records your best score, fastest completion, and number of wins.

| Action | Key |
| --- | --- |
| Drive and turn | WASD or arrow keys |
| Fire cannon | Space |
| Brake | Shift |
| Change camera | Tab |
| Zoom chase or map view | + / - |
| Pause or resume | Escape |
| Fullscreen | F or F11 |
| Mute | M |
| Navigate menus | Arrow keys or Tab |
| Activate a menu item | Enter |
| Deploy the selected arena | Space |

Menus also accept mouse clicks. The arena pauses when the window loses focus.
Return to the window and choose **Resume** to continue.
Settings include effects volume, mute, fullscreen, starting camera, picture filtering, and control hints.
Settings save automatically. Scores save when you complete an arena.
Closing the app ends the current run. There is no mid-arena save.

## Build from source

The client requires Rust 1.88 or newer and the platform's compiler tools.
On Debian or Ubuntu, install the audio development package:

```sh
sudo apt-get install libasound2-dev
```

Run the client from the repository:

```sh
cargo run --locked --release -p spectre-client
```

The repository contains the generated assets. No asset build is necessary for this command.
Release mode keeps the software renderer fast.

## Package

Create a package for the current operating system with Python 3:

```sh
python3 scripts/package-desktop.py
```

The script writes packages to `artifacts/desktop/`.
On macOS, it creates an app and a ZIP archive with an icon and a local ad-hoc signature.
The app is not notarized. Public macOS distribution requires a Developer ID signature and notarization.
On Windows, the script creates a ZIP archive. On Linux, it creates a TAR.GZ archive.
Version tags build six packages for macOS, Windows, and Linux on x86-64 and ARM64.
See the [desktop release process](desktop-releases.md).
The local macOS ARM64 package passed the automated graphical smoke test.

## Saved data

| Platform | Profile location |
| --- | --- |
| macOS | `~/Library/Application Support/OpenSpectre/profile.json` |
| Windows | `%APPDATA%\OpenSpectre\profile.json` |
| Linux | `$XDG_CONFIG_HOME/openspectre/profile.json`, or `~/.config/openspectre/profile.json` |

Set `SPECTRE_DATA_DIR` to use a different profile directory.
An unreadable profile produces a visible warning and starts with default settings.
The client reports save errors in the window.

## Verification

Run the core and client tests:

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Run the desktop smoke test in a graphical session:

```sh
cargo run --locked --release -p spectre-client -- --smoke-test artifacts/desktop-smoke
```

The smoke test uses its own profile and closes after completion.
It uses scripted controls and ignores live keyboard, mouse, and focus changes. Closing its window still stops the run.
It captures the title, arena picker, gameplay, pause, settings, results, and fullscreen screens.
It verifies pause timing, replay, persisted settings, and native sound-event dispatch.
For the win check, it moves beacons onto the tank and uses the core's contact and scoring logic.
For the loss screen, it supplies a lost-match fixture. Core tests cover damage and life consumption.
Screenshots and `report.json` appear in the output directory.
Sound-event dispatch does not verify the speaker or output-device volume.

## Scope and credits

This milestone provides single-player beacon runs. Multiplayer rooms and controller support remain future work.
The web playground retains its existing multiplayer and research tools.
The client uses Macroquad for its window, input, texture presentation, and sound playback.
The simulation and software renderer remain in `spectre-core`.

Signal Frontier assets use the license in `assets/open/LICENSE`.
The Space Grotesk interface font comes from the Google Fonts repository and uses the SIL Open Font License.
Its license is included in `crates/spectre-client/assets/SpaceGrotesk-OFL.txt`.
Packages include asset, font, and dependency license notices.
