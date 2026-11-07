import http from 'node:http';
import { createReadStream } from 'node:fs';
import { stat, readFile } from 'node:fs/promises';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { resolve } from 'node:path';
import { homedir } from 'node:os';
import { setDefaultAutoSelectFamilyAttemptTimeout } from 'node:net';

// Give each address enough time on networks where the default 250 ms is short.
setDefaultAutoSelectFamilyAttemptTimeout(5000);

const port = Number(process.env.PORT || 5173);
const disk = resolve(process.env.SPECTRE_ISO || resolve(homedir(), 'SpectreVRCD_1_1.iso'));
const diskStat = await stat(disk);
if (!diskStat.isFile()) throw new Error(`Not a disk image: ${disk}`);
let origin = `http://localhost:${port}`;
const mediaPath = '/media/SpectreVRCD_1_1.iso';
const upstream = 'https://infinitemac.org';

// The game image is read-only. Only this one configured file is exposed.
const server = http.createServer(async (req, res) => {
  res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
  res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
  res.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
  try {
    const url = new URL(req.url, origin);
    if (url.pathname === '/varz') { res.writeHead(204).end(); return; }
    if (url.pathname === '/') {
      res.setHeader('Content-Type', 'text/html; charset=utf-8');
      res.setHeader('Cache-Control', 'no-store');
      res.end(await readFile(new URL('./index.html', import.meta.url)));
      return;
    }
    if (url.pathname === mediaPath) {
      if (!['GET', 'HEAD'].includes(req.method)) {
        res.writeHead(405).end(); return;
      }
      const size = diskStat.size;
      let start = 0, end = size - 1;
      if (req.headers.range) {
        const match = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range);
        if (!match || (!match[1] && !match[2])) {
          res.writeHead(416, { 'Content-Range': `bytes */${size}` }).end(); return;
        }
        if (!match[1]) start = Math.max(0, size - Number(match[2]));
        else {
          start = Number(match[1]);
          if (match[2]) end = Math.min(Number(match[2]), size - 1);
        }
        if (start > end || start >= size || !Number.isSafeInteger(start)) {
          res.writeHead(416, { 'Content-Range': `bytes */${size}` }).end(); return;
        }
        res.statusCode = 206;
        res.setHeader('Content-Range', `bytes ${start}-${end}/${size}`);
      }
      res.setHeader('Accept-Ranges', 'bytes');
      res.setHeader('Content-Type', 'application/octet-stream');
      res.setHeader('Content-Length', end - start + 1);
      res.setHeader('Cache-Control', 'no-store');
      if (req.method === 'HEAD') res.end();
      else await pipeline(createReadStream(disk, { start, end }), res);
      return;
    }
    // Supply the emulator's CD manifest locally, without sending the ISO upstream.
    if (url.pathname.startsWith('/CD-ROM/') && req.method === 'PUT') {
      const requested = Buffer.from(decodeURIComponent(url.pathname.slice(8)), 'base64').toString();
      if (requested !== origin + mediaPath) {
        res.writeHead(400).end('Only the configured Spectre CD is supported.'); return;
      }
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ name: 'Spectre VR/CD', srcUrl: origin + mediaPath,
        fileSize: diskStat.size, fetchMode: 'cors', platform: 'Macintosh' }));
      return;
    }
    // Serve the hosted emulator through the same origin so WASM shared memory
    // and local CD sector reads work together. No game data is sent upstream.
    if (!['GET', 'HEAD'].includes(req.method)) {
      res.writeHead(405).end(); return;
    }
    const target = new URL(url.pathname + url.search, upstream);
    if (target.origin !== upstream) { res.writeHead(400).end(); return; }
    const response = await fetch(target, { method: req.method,
      signal: AbortSignal.timeout(60000) });
    res.statusCode = response.status;
    for (const name of ['content-type', 'cache-control', 'etag', 'last-modified', 'service-worker-allowed']) {
      if (response.headers.has(name)) res.setHeader(name, response.headers.get(name));
    }
    if (req.method === 'HEAD' || !response.body) res.end();
    else await pipeline(Readable.fromWeb(response.body), res);
  } catch (error) {
    console.error(error.message);
    if (!res.headersSent) res.writeHead(502, { 'Content-Type': 'text/plain' });
    res.end('Unable to load the emulator. Check the server terminal and internet connection.');
  }
});
server.listen(port, '127.0.0.1', () => {
  origin = `http://localhost:${server.address().port}`;
  console.log(`Spectre VR: ${origin}\nRead-only CD: ${disk}\nEmulator assets: ${upstream}`);
});
