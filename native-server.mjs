import http from 'node:http';
import { attachDeathmatch } from './multiplayer-server.mjs';
import { assetPack } from './asset-pack.mjs';
const pack = assetPack();
import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { pipeline } from 'node:stream/promises';

// Deliberately no emulator proxy, ROM, System image, or upstream HTTP requests.
const routes = new Map([
  ['/', ['native-web/index.html', 'text/html; charset=utf-8']],
  ['/style.css', ['native-web/style.css', 'text/css']],
  ['/progress.json', ['native-web/progress.json', 'application/json']],
  ['/audio.js', ['native-web/audio.js', 'text/javascript']],
  ...pack.public.soundIds.map(id => [`/audio/${id}.wav`, [pack.audio(id), 'audio/wav']]),
  ['/hud.js', ['native-web/hud.js', 'text/javascript']],
  ['/hud.css', ['native-web/hud.css', 'text/css']],
  ['/multiplayer.js', ['native-web/multiplayer.js', 'text/javascript']],
  ['/app.js', ['native-web/app.js', 'text/javascript']],
  ['/spectre_core.wasm', ['native-web/spectre_core.wasm', 'application/wasm']],
  ['/data/spectre.bin', [pack.data('spectre'), 'application/octet-stream']],
  ...pack.public.campaigns.map(({id}) => [`/data/${id}.bin`, [pack.data(id), 'application/octet-stream']]),
]);
const port = Number(process.env.NATIVE_PORT || 5174);
const server = http.createServer(async (req, res) => {
  try {
    if (!['GET', 'HEAD'].includes(req.method)) { res.writeHead(405).end(); return; }
    const path = new URL(req.url, 'http://localhost').pathname;
    if (path === '/pack.json') { res.writeHead(200, {'Content-Type':'application/json','Cache-Control':'no-store'}).end(req.method==='HEAD'?undefined:JSON.stringify(pack.public)); return; }
    const route = routes.get(path);
    if (!route) { res.writeHead(404).end('Not found'); return; }
    const file = new URL(route[0], import.meta.url);
    const info = await stat(file);
    res.writeHead(200, { 'Content-Type': route[1], 'Content-Length': info.size, 'Cache-Control': 'no-store' });
    if (req.method === 'HEAD') res.end(); else await pipeline(createReadStream(file), res);
  } catch (e) {
    if (!res.headersSent) res.writeHead(e.code === 'ENOENT' ? 404 : 500);
    res.end(pack.id === 'classic' ? 'Classic assets missing. Extract your disc and run npm run build:audio, or start the bundled version with npm start.' : 'Bundled asset missing. Run npm run build.');
  }
});
attachDeathmatch(server, { pack });
server.listen(port, process.env.NATIVE_HOST || '127.0.0.1', () => console.log(`${pack.public.name}: http://localhost:${server.address().port}`));
