# Build evidence

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

[CI](https://github.com/dustywusty/openspectre/actions/workflows/standalone.yml) uploads this evidence and links it in the build summary.
Same-repository pull requests show playable videos and screenshots directly in an updated comment. No archive download is needed to review them.
The publication job uploads media as GitHub attachments. It does not check out or execute pull-request code.
Fork pull requests use the build summary.
Original file archives require GitHub sign-in and remain available for 30 days. Check the build result: failed tests may leave partial recordings.

## Two-browser multiplayer recording

```sh
npx playwright install chromium ffmpeg
npm run test:multiplayer:record
```

This command starts an isolated server and two browser clients. It records both views on the same timeline, including the HUD.
The clients drive and fire through canvas keyboard handlers. The simulation and production client source remain unchanged.

The recording compares local networking, 150 ms of added round-trip delay, and that delay with jitter and 300 ms receive stalls.
The test preserves WebSocket message order. It changes from overhead to chase view during each phase.
These are controlled local conditions, not a measurement of an Internet connection.

The output directory is `artifacts/multiplayer/`. It contains `two-clients.mp4`, screenshots, and `telemetry.json`.
Telemetry includes queued inputs, correction distance, browser frame gaps, and frames that have no new opponent snapshot to interpolate toward.
The recording is silent. A passing check confirms that both clients run without browser errors; it does not establish acceptable latency.

To route both clients through a real public Cloudflare relay, install `cloudflared`, then run:

```sh
npm run test:multiplayer:record -- --tunnel
```

The test starts a temporary tunnel to its isolated server and closes both when it finishes.
Both game pages and both WebSocket connections use the public HTTPS origin.
The first phase adds no artificial delay. Later phases add the same delay and jitter profiles on top of the real connection.
The telemetry includes the public origin, WebSocket URLs, and measured application ping round-trip times for each phase.
Outputs go to `artifacts/multiplayer-tunnel/`, separate from the local test.
Both browsers still run on the same computer. This exercises a public relay, not two geographically separated players.
If the system resolver cannot resolve a new tunnel hostname, the test retries with Cloudflare public DNS.
That fallback applies only to the test process and browser. It points to the public edge address and does not change system DNS settings.
