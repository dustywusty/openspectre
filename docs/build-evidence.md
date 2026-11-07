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

[CI](https://github.com/dustywusty/openspectre/actions/workflows/standalone.yml) uploads this evidence and links it in the build summary. Same-repository pull requests also get an updated recording comment. Fork pull requests use the build summary.
Downloads require GitHub sign-in and remain available for 30 days. Check the build result: failed tests may leave partial recordings.
