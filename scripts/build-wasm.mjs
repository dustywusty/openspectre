import { spawnSync } from 'node:child_process';
import { copyFileSync } from 'node:fs';
const root = new URL('../', import.meta.url);
const result = spawnSync('cargo', ['build', '--locked', '-p', 'spectre-core', '--release', '--target', 'wasm32-unknown-unknown'], { cwd: root, stdio: 'inherit' });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status || 1);
copyFileSync(new URL('target/wasm32-unknown-unknown/release/spectre_core.wasm', root), new URL('native-web/spectre_core.wasm', root));
console.log('Native Rust WASM copied to native-web/spectre_core.wasm');
