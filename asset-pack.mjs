import { readFileSync } from 'node:fs';

export function assetPack(id = process.env.SPECTRE_ASSETS || 'open') {
  if (!['open', 'classic'].includes(id)) throw new Error('SPECTRE_ASSETS must be open or classic');
  const open = id === 'open';
  const manifest = open ? JSON.parse(readFileSync(new URL('assets/open/generated/manifest.json', import.meta.url))) : null;
  const dataRoot = new URL(open ? 'assets/open/generated/' : '.local/extracted/', import.meta.url);
  const audioRoot = new URL(open ? 'assets/open/generated/audio/' : '.local/audio/', import.meta.url);
  return {
    id, dataRoot, audioRoot,
    data: name => new URL(`${name}.bin`, dataRoot),
    audio: id => new URL(`${id}.wav`, audioRoot),
    public: {
      id, name: manifest?.name ?? 'Spectre VR/CD · Classic disc',
      description: open ? 'Bundled original models, arenas, radar, and sound. No disc required.' : 'Assets extracted locally from your own Spectre VR/CD disc.',
      campaigns: manifest?.campaigns ?? [
        {id:'central',name:'Central Systems Matrix'}, {id:'eastern',name:'Eastern Edge Matrix'}, {id:'south',name:'South Central Matrix'},
      ],
      modelCount: manifest ? manifest.models_per_campaign : 358,
      arenaCount: manifest ? manifest.campaigns.reduce((n,c)=>n+c.arenas.length,0) : 112,
      soundIds: open ? Array.from({length:48},(_,i)=>10001+i) : [10001,10002,10003,10004,10005,10006,10007,10009,10010,10013,10017,10020,10021,10023,10024,10027,10028,10035,10037,10038,10042,10044,10045,10047],
    },
  };
}
