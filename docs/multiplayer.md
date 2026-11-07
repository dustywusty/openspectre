# Two-player deathmatch

The browser has Single player and Multiplayer buttons above the game. Switching to Multiplayer pauses the single-player game. Switching back preserves it and leaves the multiplayer room.

Bundled multiplayer uses Crosscurrent, a new arena in Signal Frontier. Classic mode uses Central Systems Matrix arena 10000 from your disc extraction. The server runs the Rust simulation through WASM at 60 ticks per second. It sends snapshots about 20 times per second. Each browser renders its own view with the same Rust library.

## Play

1. Run `npm ci` and `npm run build`.
2. Run `npm start` for the bundled pack, or `npm run start:classic` after extracting your disc.
3. Open `http://localhost:5174/#drive` and select Multiplayer.
4. Select Create room. Give the invitation link to the other player.
5. The other player opens the link and joins automatically. Click the arena to drive and enable sound.

For people on other computers, open the public URL before creating the room. A localhost invitation only works on the host computer.

This command creates a temporary public URL:

```sh
cloudflared tunnel --protocol http2 --url http://localhost:5174 --no-autoupdate
```

Keep the computer awake and both server processes running. The URL changes when the tunnel restarts. The public server serves the local game assets to connected browsers.

WASD or arrow keys drive. Space fires. Shift brakes. Tab changes the view. F enters fullscreen. Click the arena to take control. Losing focus releases controls; it does not pause a live match.

## Match rules

- Two players, one arena, cannons only.
- Three hits destroy a tank. Cannon ammunition is unlimited.
- Respawn takes two seconds. Spawn protection lasts 1.5 seconds; firing ends it.
- Five kills win. A simultaneous tied fifth kill continues until one player leads.
- Both players must request a rematch to reset it.
- A disconnect stops the simulation. A replacement player starts a fresh match.
- Empty rooms are removed. The server accepts up to four rooms.

These are new demo rules. They do not reproduce the original networking protocol or original multiplayer rules. Player hit detection uses the tank footprint. Scenery collision uses the existing recovery adapter. The demo starts in chase view. The bundled pack uses a solid renderer. The classic cockpit renderer remains a research view with projection issues.

The server accepts controls only. Clients cannot set health, positions, damage, or scores. Stale controls expire after 500 milliseconds. Local movement and cannon shots use the same Rust simulation as the server, with sequenced inputs and server acknowledgements.
The client restores server checkpoints and repeats inputs that the server has not acknowledged. Small visual corrections fade smoothly.
Opponent movement uses interpolation between snapshots. Damage and scores still depend on the server connection.

## Checks

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
npm run build:wasm
npm run test:open
npm run test:open-network
```

Rust tests cover movement, hits, cover, deaths, respawns, spawn protection expiry, and victory. The WASM check plays a full match on the original arena and checks both rendered viewpoints. The network check covers room joining, shared state, input validation and expiry, room capacity, rematches, disconnection, replacement players, and cleanup.

Classic recovery checks remain available through `npm run test:multiplayer` after disc extraction.
