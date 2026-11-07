// Explicit local corpus check. Requires the user's extracted CD and a WASM build.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
const root = new URL('../', import.meta.url);
const read = path => readFileSync(new URL(path, root));
const { instance } = await WebAssembly.instantiate(read('native-web/spectre_core.wasm'), {});
const w = instance.exports;
assert.equal(WebAssembly.Module.imports(await WebAssembly.compile(read('native-web/spectre_core.wasm'))).length, 0);
let frames = 0;
for (const [name, models, maps] of [['central', 119, 30], ['eastern', 120, 52], ['south', 119, 30]]) {
  const bytes = read(`.local/extracted/${name}.bin`);
  const ptr = w.input_buffer(bytes.length);
  new Uint8Array(w.memory.buffer, ptr, bytes.length).set(bytes);
  assert.equal(w.load_archive(), 1, `load ${name}`);
  const entries = JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.output_ptr(), w.output_len())));
  assert.equal(entries.filter(e => e.kind === 'shap').length, models);
  assert.equal(entries.filter(e => e.kind === 'wrld').length, maps);
  const tank = entries.findIndex(e => e.kind === 'shap' && e.id === 10010);
  assert.equal(entries[tank].vertices, 14);
  assert.equal(entries[tank].faces, 13);
  for (let i = 0; i < entries.length; i++) {
    const frame = new Uint8Array(w.memory.buffer, w.render(i, .65, .3, 1, 1), 800 * 600 * 4);
    assert.equal(frame[3], 255);
    assert.ok(frame.some((v, k) => k % 4 !== 3 && v > 100), `${name} ${entries[i].kind} ${entries[i].id} drew no visible geometry`);
    frames++;
  }
  if (name === 'central') {
    const result = spawnSync('cargo', ['run', '--locked', '-p', 'spectre-tools', '--', 'render', '.local/extracted/central.bin', '.local/verify-native.ppm'], { cwd: root, stdio: 'inherit' });
    assert.equal(result.status, 0);
    const ppm = read('.local/verify-native.ppm');
    const rgb = ppm.subarray(ppm.indexOf('255\n') + 4);
    const rgba = new Uint8Array(w.memory.buffer, w.render(tank, .65, .3, 1, 1), 800 * 600 * 4);
    assert.equal(rgb.length, 800 * 600 * 3);
    let mismatches = 0;
    for (let p = 0; p < 800 * 600; p++) {
      if (rgb[p*3] !== rgba[p*4] || rgb[p*3+1] !== rgba[p*4+1] || rgb[p*3+2] !== rgba[p*4+2]) mismatches++;
    }
    assert.equal(mismatches, 0, 'Native and WASM reference frame differ');
    console.log('Native/WASM Tank 1 frame: pixel-identical.');
  }
}
const ptr = w.input_buffer(12);
new Uint8Array(w.memory.buffer, ptr, 12).fill(255);
assert.equal(w.load_archive(), 0, 'Malformed archive must fail without trapping');
assert.equal(w.input_buffer(33 * 1024 * 1024), 0);
console.log(`Passed: ${frames} original model/map renders; malformed input; zero WASM imports.`);
