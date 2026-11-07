import { access } from 'node:fs/promises';
import { assetPack } from '../asset-pack.mjs';
const pack=assetPack('classic');
try {
  await Promise.all(['spectre','central','eastern','south'].map(name=>access(pack.data(name))));
  await Promise.all(pack.public.soundIds.map(id=>access(pack.audio(id))));
} catch {
  console.error('Classic assets are missing. Extract your disc with python3 scripts/extract-disc.py /path/to/disc.iso, then run npm run build:audio.');
  console.error('For the bundled game without a disc, use npm start.');
  process.exit(1);
}
process.env.SPECTRE_ASSETS='classic';
await import('../native-server.mjs');
