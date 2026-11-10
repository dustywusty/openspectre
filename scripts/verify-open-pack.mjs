import assert from 'node:assert/strict';
import {readFileSync, mkdtempSync, cpSync, symlinkSync, existsSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawn, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {WebSocket} from 'ws';

import {recorder} from './playthrough-recorder.mjs';
const recording=recorder(process.argv.includes('--record'));

const root=new URL('../',import.meta.url);
const manifest=JSON.parse(readFileSync(new URL('assets/open/generated/manifest.json',root)));
for(const [file,hash] of Object.entries(manifest.files)){
  const data=readFileSync(new URL(`assets/open/generated/${file}`,root));
  assert.equal(createHash('sha256').update(data).digest('hex'),hash,`manifest: ${file}`);
  if(file.endsWith('.wav')){
    assert.equal(data.toString('ascii',0,4),'RIFF');assert.equal(data.readUInt16LE(22),1);
    assert.equal(data.readUInt32LE(24),22050);assert.ok(data.length>4000);
  }
}
// Rebuild from source in a separate directory, then compare every artifact.
const rebuilt=mkdtempSync(join(tmpdir(),'openspectre-assets-'));
try{
  const r=spawnSync('python3',[new URL('scripts/build-open-assets.py',root).pathname,'--output',rebuilt],{encoding:'utf8'});
  assert.equal(r.status,0,r.stderr);
  assert.deepEqual(JSON.parse(readFileSync(join(rebuilt,'manifest.json'))),manifest,'reproducible pack');
}finally{rmSync(rebuilt,{recursive:true,force:true});}

const module=await WebAssembly.compile(readFileSync(new URL('native-web/spectre_core.wasm',root)));
const make=async campaign=>{
  const w=(await WebAssembly.instantiate(module,{})).exports;
  const text=()=>new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len()));
  const output=()=>JSON.parse(text());
  const feed=(bytes,fn)=>{const p=w.input_buffer(bytes.length);new Uint8Array(w.memory.buffer,p,bytes.length).set(bytes);assert.equal(w[fn](),1,text());};
  feed(readFileSync(new URL('assets/open/generated/spectre.bin',root)),'load_shared_colors');
  feed(readFileSync(new URL(`assets/open/generated/${campaign}.bin`,root)),'load_archive');
  return {w,output,entries:output()};
};
let models=0,arenas=0;
for(const campaign of manifest.campaigns){
  const {w,output,entries}=await make(campaign.id);
  for(let i=0;i<entries.length;i++){
    const entry=entries[i];
    if(entry.kind==='shap'){
      const ptr=w.render(i,.65,.3,1,1);assert.ok(ptr);
      const pixels=new Uint8Array(w.memory.buffer,ptr,800*600*4);
      assert.ok(pixels.some((v,j)=>j%4!==3&&v>100),`${entry.name} renders`);models++;
    }else{
      assert.equal(w.game_start_mission(i),1);const scene=output();
      assert.equal(scene.unresolved,0);assert.ok(scene.targets.length>0);assert.ok(scene.items.some(i=>i.kind===12));
      for(const camera of [0,1,2]){
        w.game_camera(camera);const ptr=w.game_frame(0,0,0,0,0);
        const pixels=new Uint8Array(w.memory.buffer,ptr,800*600*4);
        assert.ok(pixels.some((v,j)=>j%4!==3&&v>100),`${entry.name} camera ${camera}`);
      }
      w.game_infinite_health(1);
      for(let t=0;t<90;t++)w.game_frame(1/60,1,t<30?1:0,0,1);
      assert.ok(output().fired>0);assert.equal(output().death_transition,null);
      arenas++;
    }
  }
  for(const difficulty of [0,3,5,7])for(let level=1;level<=5;level++){
    assert.equal(w.game_start_campaign_level(level,difficulty),1);
    const s=output();assert.equal(s.campaign.world,10000+(level-1)%4);
    assert.equal(s.optional_weapons.available[5],1,'first level unlocks scattershot');
    if(level>=4)for(const weapon of [1,5,9,10])assert.equal(s.optional_weapons.available[weapon],1);
    assert.ok(s.palette);assert.equal(s.radar_glyph_count,41);
    if(difficulty===0&&level<=4){
      const weapon=[5,1,9,10][level-1];
      for(let tick=0;tick<17;tick++)w.game_frame_weapons(1/60,0,0,0,0,0,1);
      assert.ok(output().shots.some(shot=>shot.weapon===weapon),`replacement weapon ${weapon} fires`);
      assert.ok(output().ammo<40);
    }
  }
}
console.log(`PASS: ${models} replacement models, ${arenas} arenas, all views, campaign tiers, weapons, palettes, radar, and reproducible sound assets.`);

// Actual drive-and-shoot combat and normal deaths on the bundled opening map.
const {w,output,entries}=await make('central');
const arena=entries.findIndex(e=>e.kind==='wrld'&&e.id===10000);
assert.equal(w.game_start_mission(arena),1);w.game_camera(2);w.game_frame(0,0,0,0,0);
const initial=output();
for(const [camera,name] of [[0,'cockpit'],[1,'chase'],[2,'overhead']]){
  w.game_camera(camera);recording.screenshot(w,w.game_frame(0,0,0,0,0),name);
}
try{
  await recording.start('01-combat','Cannon combat | normal health');
  for(let t=0;t<1200;t++){await recording.frame(w,w.game_frame(1/60,0,0,0,1));if(output().destroyed>0)break;}
  assert.ok(output().destroyed>0,'cannons can kill replacement enemies');
  await recording.stop();
  await recording.start('02-death','Enemy damage and death | normal health');
  w.game_reset();w.game_frame(0,0,0,0,0);
  for(let t=0;t<3000&&!output().death_transition;t++)await recording.frame(w,w.game_frame(1/60,0,0,0,0));
  assert.ok(output().death_transition,'replacement enemies damage and kill the player');
  // Include the visible death spin after the transition starts.
  for(let t=0;t<260;t++)await recording.frame(w,w.game_frame(1/60,0,0,0,0));
  await recording.stop();
  await recording.start('03-beacon-mission','Beacon mission | INFINITE HEALTH navigation test');
  assert.equal(w.game_start_campaign_level(1,0),1);
  w.game_camera(1);
  w.game_infinite_health(1);w.game_frame(0,0,0,0,0);
  // Drive around cover and along the perimeter; no simulation state is injected.
  for(const [x,z] of [[160,512],[160,160],[96,160],[96,864],[160,864],[864,864]]){
    for(let t=0;t<1800;t++){
      const tank=output().tank;
      if(Math.hypot(x-tank.x,z-tank.z)<10||output().encounter.status==='won')break;
      const aim=Math.atan2(x-tank.x,z-tank.z);
      const delta=Math.atan2(Math.sin(aim-tank.heading),Math.cos(aim-tank.heading));
      await recording.frame(w,w.game_frame(1/60,Math.abs(delta)<.18?1:0,Math.abs(delta)>.025?Math.sign(delta):0,0,1));
    }
  }
  assert.equal(output().flags.collected,output().flags.total,'driving collects every replacement beacon');
  assert.equal(output().encounter.status,'won');assert.ok(output().completion);
  assert.ok(output().mission.score>initial.mission.score);
}finally{await recording.stop();}
assert.equal(w.game_next_campaign_level(),1,'campaign can advance after victory');
assert.equal(output().campaign.level,2);
assert.equal(w.dm_start(),1);w.dm_input(0,0,0,0,1);
for(let t=0;t<6000;t++)w.dm_step();w.dm_snapshot();
assert.equal(output().winner,0,'replacement arena supports a complete deathmatch');
console.log('PASS: live enemy combat, death, beacon collection, scoring, and a five-kill multiplayer match.');

// Enemy respawn option: hunt every enemy with real drive/aim/fire inputs, then
// check that eliminated enemies return when enabled and stay dead when disabled.
{
  const hunt=(game,budget,done)=>{
    for(let t=0;t<budget;t++){
      const s=game.output();if(done(s))return;
      const live=s.targets.filter(e=>e.health>0&&!e.retired);
      if(!live.length){game.w.game_frame(1/60,0,0,0,0);continue;}
      const near=live.map(e=>[e.item.x-s.tank.x,e.item.z-s.tank.z]).sort((a,b)=>Math.hypot(...a)-Math.hypot(...b))[0];
      const aim=Math.atan2(near[0],near[1]);
      const delta=Math.atan2(Math.sin(aim-s.tank.heading),Math.cos(aim-s.tank.heading));
      game.w.game_frame(1/60,Math.abs(delta)<.2&&Math.hypot(...near)>150?1:0,Math.abs(delta)>.03?Math.sign(delta):0,0,Number(Math.abs(delta)<.12));
    }
  };
  const start=async respawn=>{
    const game=await make('central');
    assert.equal(game.w.game_start_mission(game.entries.findIndex(e=>e.kind==='wrld'&&e.id===10000)),1);
    game.w.game_frame(0,0,0,0,0);
    assert.equal(game.output().enemy_respawn,true,'enemy respawn defaults on');
    game.w.game_enemy_respawn(Number(respawn));game.w.game_infinite_health(1);game.w.game_frame(0,0,0,0,0);
    assert.equal(game.output().enemy_respawn,respawn);
    return game;
  };
  const on=await start(true);const enemies=on.output().targets.length;
  hunt(on,60*300,s=>s.targets.every(e=>e.deaths>0)&&s.destroyed>enemies);
  let s=on.output();
  assert.ok(s.targets.every(e=>e.deaths>0),'every enemy was eliminated');
  assert.ok(s.targets.some(e=>e.deaths>=2),'a respawned enemy was eliminated again');
  assert.ok(s.targets.every(e=>e.health>0&&!e.retired),'eliminated enemies respawned');
  const off=await start(false);
  hunt(off,60*300,s=>s.targets.every(e=>e.retired));
  for(let t=0;t<600;t++)off.w.game_frame(1/60,0,0,0,1);
  s=off.output();
  assert.ok(s.targets.every(e=>e.retired&&e.health===0),'with respawn off, eliminated enemies stay dead');
  assert.equal(s.destroyed,enemies);
  console.log(`PASS: enemy respawn on (${on.output().destroyed} kills of ${enemies} enemies, all back) and off (all retired).`);
}

// Run the real HTTP/WSS server from a directory that has no .local or disc files.
const clean=mkdtempSync(join(tmpdir(),'openspectre-no-disc-'));
let server,peer;
try{
  for(const file of ['native-server.mjs','multiplayer-server.mjs','asset-pack.mjs','package.json','native-web','assets'])cpSync(new URL(file,root),join(clean,file),{recursive:true});
  symlinkSync(new URL('node_modules',root).pathname,join(clean,'node_modules'),'dir');
  assert.equal(existsSync(join(clean,'.local')),false);
  server=spawn(process.execPath,['native-server.mjs'],{cwd:clean,env:{...process.env,NATIVE_PORT:'0',SPECTRE_ASSETS:'open'},stdio:['ignore','pipe','pipe']});
  const url=await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>reject(Error('Server start timed out')),10000);
    server.stdout.on('data',b=>{const match=b.toString().match(/http:\/\/localhost:\d+/);if(match){clearTimeout(timer);resolve(match[0]);}});
    server.on('error',error=>{clearTimeout(timer);reject(error);});
    server.on('exit',code=>{clearTimeout(timer);reject(Error(`Server exited ${code}`));});
  });
  const get=async path=>{const r=await fetch(url+path);assert.equal(r.status,200,path);return r;};
  const config=await (await get('/pack.json')).json();assert.equal(config.id,'open');
  for(const file of ['/','/app.js','/audio.js','/multiplayer.js','/hud.js','/hud.css','/style.css','/spectre_core.wasm','/data/spectre.bin',...config.campaigns.map(c=>`/data/${c.id}.bin`),...config.soundIds.map(id=>`/audio/${id}.wav`)])await (await get(file)).arrayBuffer();
  assert.equal((await fetch(url+'/.local/extracted/spectre.bin')).status,404);
  peer=new WebSocket(url.replace('http:','ws:')+'/multiplayer/socket');
  await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>reject(Error('Room creation timed out')),10000);
    peer.on('open',()=>peer.send(JSON.stringify({type:'create'})));
    peer.on('message',bytes=>{const msg=JSON.parse(bytes);if(msg.type==='welcome'){assert.equal(msg.pack,'open');clearTimeout(timer);resolve();}else if(msg.type==='error'){clearTimeout(timer);reject(Error(msg.message));}});
    peer.on('error',reject);
  });
  console.log('PASS: default server, every browser asset, audio, and multiplayer room work with no .local directory.');
}finally{peer?.terminate();if(server&&server.exitCode===null&&server.signalCode===null){const exited=new Promise(resolve=>server.once('exit',resolve));server.kill();await exited;}rmSync(clean,{recursive:true,force:true});}
