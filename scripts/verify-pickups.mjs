import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
for(const [path,load] of [['.local/extracted/spectre.bin','load_shared_colors'],['.local/extracted/central.bin','load_archive']]){
 const bytes=readFileSync(path),ptr=w.input_buffer(bytes.length);
 new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);assert.equal(w[load](),1);
}
assert.equal(w.game_start_campaign_level(1,0),1);
w.game_frame(0,0,0,0,0);const initial=output();
assert.equal(initial.pickups.length,6);
assert.ok(initial.pickups.every(p=>p.item.shape_id===6&&p.item.source==='recovered procedural vertices'));
for(const mode of [0,1,2]){
 w.game_camera(mode);w.game_zoom(-1);const frame=w.game_frame(0,0,0,0,0);
 assert.deepEqual(output(),initial,'view changes preserve pickup and simulation state');
 if(mode===2){
  const pixels=new Uint8Array(w.memory.buffer,frame,800*600*4);
  for(const subtype of new Set(initial.pickups.map(p=>p.subtype))){
   const rgb=initial.palette[[4,0,5,9][subtype]].map(v=>v>>8);let count=0;
   for(let i=0;i<pixels.length;i+=4)if(rgb.every((v,c)=>pixels[i+c]===v))count++;
   assert.ok(count>=8,`overhead view visibly renders subtype ${subtype} in its owned palette color`);
  }
 }

 if(process.argv.includes('--snapshot'))execFileSync('ffmpeg',['-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-i','pipe:0','-frames:v','1',`.local/pickups-view-${mode}.png`],{input:Buffer.from(new Uint8Array(w.memory.buffer,frame,800*600*4)),stdio:['pipe','ignore','pipe']});
}
w.game_frame(1/60,1,0,0,1);w.game_reset();w.game_frame(0,0,0,0,0);
assert.deepEqual(output(),initial,'reset restores pickup records and simulation state');
w.game_encounter(0);
for(let tick=0;tick<30;tick++) w.game_frame(1/60,0,0,1,1);
const spent=output();assert.ok(spent.ammo<40,'fire consumes ammunition before pickup');
const destination=spent.pickups[2].item;
let collected;
for(let tick=0;tick<600;tick++){
 const state=output(),dx=destination.x-state.tank.x,dz=destination.z-state.tank.z;
 const desired=Math.atan2(dx,dz),error=Math.atan2(Math.sin(desired-state.tank.heading),Math.cos(desired-state.tank.heading));
 w.game_frame(1/60,Math.abs(error)<0.35?1:0,Math.abs(error)>0.04?Math.sign(error):0,0,0);
 if(output().pickups_collected){collected=output();break;}
}
assert.ok(collected,'drive to an authored pickup without teleporting');
assert.equal(collected.ammo,40,'white pickup restores ammunition');
assert.notDeepEqual([collected.pickups[2].item.x,collected.pickups[2].item.z],[destination.x,destination.z],'collected dump relocates');
console.log('Passed: six procedural pickups load, views/reset preserve state, and a driven WASM tank collects a full refill and relocates the dump. Pickup animation repeats every sixty ticks. Shield protection and special flight weapons remain pending.');
