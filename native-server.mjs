import http from 'node:http';
import { attachDeathmatch } from './multiplayer-server.mjs';
import { GAME_SOUND_IDS } from './native-web/audio.js';
import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { pipeline } from 'node:stream/promises';

// Deliberately no emulator proxy, ROM, System image, or upstream HTTP requests.
const routes = new Map([
  ['/', ['native-web/index.html', 'text/html; charset=utf-8']],
  ['/style.css', ['native-web/style.css', 'text/css']],
  ['/progress.json', ['native-web/progress.json', 'application/json']],
  ['/audio.js', ['native-web/audio.js', 'text/javascript']],
  ...GAME_SOUND_IDS.map(id => [`/audio/${id}.wav`, [`.local/audio/${id}.wav`, 'audio/wav']]),
  ['/hud.js', ['native-web/hud.js', 'text/javascript']],
  ['/hud.css', ['native-web/hud.css', 'text/css']],
  ['/multiplayer.js', ['native-web/multiplayer.js', 'text/javascript']],
  ['/app.js', ['native-web/app.js', 'text/javascript']],
  ['/spectre_core.wasm', ['native-web/spectre_core.wasm', 'application/wasm']],
  ['/data/spectre.bin', ['.local/extracted/spectre.bin', 'application/octet-stream']],
  ...['central', 'eastern', 'south'].map(name => [`/data/${name}.bin`, [`.local/extracted/${name}.bin`, 'application/octet-stream']]),
]);
const port = Number(process.env.NATIVE_PORT || 5174);
const server = http.createServer(async (req, res) => {
  try {
    if (!['GET', 'HEAD'].includes(req.method)) { res.writeHead(405).end(); return; }
    const path = new URL(req.url, 'http://localhost').pathname;
    const route = routes.get(path);
    if (!route) { res.writeHead(404).end('Not found'); return; }
    const file = new URL(route[0], import.meta.url);
    const info = await stat(file);
    res.writeHead(200, { 'Content-Type': route[1], 'Content-Length': info.size, 'Cache-Control': 'no-store' });
    if (req.method === 'HEAD') res.end(); else await pipeline(createReadStream(file), res);
  } catch (e) {
    if (!res.headersSent) res.writeHead(e.code === 'ENOENT' ? 404 : 500);
    res.end('Missing local asset. Run the extraction and WASM build commands in README.md.');
  }
});
attachDeathmatch(server);
server.listen(port, '127.0.0.1', () => console.log(`Spectre Rust workbench: http://localhost:${port}`));
