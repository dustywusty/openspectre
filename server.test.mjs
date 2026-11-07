import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { once } from 'node:events';

test('serves exact disk bytes, read-only ranges, and a local CD manifest', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'spectre-test-'));
  const disk = join(dir, 'fixture.iso');
  const bytes = Buffer.from(Array.from({ length: 4096 }, (_, i) => i % 251));
  await writeFile(disk, bytes);
  const child = spawn(process.execPath, ['server.mjs'], {
    cwd: new URL('.', import.meta.url),
    env: { ...process.env, PORT: '0', SPECTRE_ISO: disk },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  try {
    const base = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Server did not start')), 10000);
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`Server exited: ${code}`)); });
      child.stdout.on('data', chunk => {
        const match = String(chunk).match(/Spectre VR: (http:\/\/localhost:\d+)/);
        if (match) { clearTimeout(timer); resolve(match[1]); }
      });
    });
    const url = base + '/media/SpectreVRCD_1_1.iso';
    for (const [range, start, end] of [
      ['bytes=0-1023', 0, 1024], ['bytes=1024-2047', 1024, 2048],
      ['bytes=-32', 4064, 4096], ['bytes=4000-', 4000, 4096],
      ['bytes=4000-9999', 4000, 4096],
    ]) {
      const response = await fetch(url, { headers: { Range: range } });
      assert.equal(response.status, 206);
      assert.equal(response.headers.get('content-range'), `bytes ${start}-${end - 1}/4096`);
      assert.deepEqual(Buffer.from(await response.arrayBuffer()), bytes.subarray(start, end));
    }
    for (const range of ['bytes=4096-', 'bytes=10-2', 'bytes=-0', 'bytes=-', 'bytes=0-1,5-6']) {
      assert.equal((await fetch(url, { headers: { Range: range } })).status, 416);
    }
    const head = await fetch(url, { method: 'HEAD' });
    assert.equal(head.headers.get('content-length'), '4096');
    assert.equal((await head.arrayBuffer()).byteLength, 0);
    assert.equal((await fetch(url, { method: 'POST' })).status, 405);
    const full = await fetch(url);
    assert.deepEqual(Buffer.from(await full.arrayBuffer()), bytes);
    const manifest = await fetch(base + '/CD-ROM/' + Buffer.from(url).toString('base64'), { method: 'PUT' });
    const data = await manifest.json();
    assert.equal(data.srcUrl, url);
    assert.equal(data.fileSize, bytes.length);
    assert.equal(data.fetchMode, 'cors');
    const other = await fetch(base + '/CD-ROM/' + Buffer.from('file:///etc/passwd').toString('base64'), { method: 'PUT' });
    assert.equal(other.status, 400);
    const page = await fetch(base);
    assert.equal(page.headers.get('cross-origin-opener-policy'), 'same-origin');
    assert.equal(page.headers.get('cross-origin-embedder-policy'), 'require-corp');
    assert.match(await page.text(), /Start Macintosh/);
  } finally {
    const exited = once(child, 'exit');
    child.kill();
    if (child.exitCode === null) await exited;
    await rm(dir, { recursive: true, force: true });
  }
});
