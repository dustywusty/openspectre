import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const shared=readFileSync('.local/extracted/spectre.bin'),sharedPtr=w.input_buffer(shared.length);
new Uint8Array(w.memory.buffer,sharedPtr,shared.length).set(shared);assert.equal(w.load_shared_colors(),1);
function snapshot(name){
 const ptr=w.game_frame(0,0,0,0,0);
 const pixels=new Uint8Array(w.memory.buffer,ptr,800*600*4);
 // Check a broad strip: isolated ground dots or scenery must not mimic a horizon band.
 let colored=0;
 for(let y=315;y<395;y++)for(let x=0;x<800;x++){
  const i=(y*800+x)*4;
  if(pixels[i]>0&&pixels[i+1]===0&&pixels[i+2]>0)colored++;
 }
 if(name==='submerged')assert.equal(colored,0,'submerged view has no magenta horizon');
 else assert.ok(colored>10000,`${name} view has visible horizon bands`);
 if(process.argv.includes('--snapshot'))execFileSync('ffmpeg',['-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-i','pipe:0','-frames:v','1',`.local/transit-${name}.png`],{input:Buffer.from(new Uint8Array(w.memory.buffer,ptr,800*600*4)),stdio:['pipe','ignore','pipe']});
}
const bytes=readFileSync('.local/extracted/central.bin'),ptr=w.input_buffer(bytes.length);
new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);assert.equal(w.load_archive(),1);
const entries=output(),index=entries.findIndex(e=>e.kind==='wrld'&&e.id===10110);
assert.ok(index>=0);assert.equal(w.game_start(index),1);w.game_frame(0,0,0,0,0);
const initial=output();snapshot('surface');
const world=readFileSync('.local/central-shape-resources/77726c64/10110.bin');
const transits=[];
for(let i=0;i<world.readUInt16BE(0);i++){
 const offset=2+8*i,kind=world.readInt16BE(offset);
 if(kind===36||kind===37)transits.push({kind,x:world.readInt16BE(offset+4),z:world.readInt16BE(offset+2)});
}
function drive(destination,done){
 for(let tick=0;tick<1200&&!done(output());tick++){
  const state=output(),dx=destination.x-state.tank.x,dz=destination.z-state.tank.z;
  const desired=Math.atan2(dx,dz),error=Math.atan2(Math.sin(desired-state.tank.heading),Math.cos(desired-state.tank.heading));
  w.game_frame(1/60,Math.abs(error)<0.35?1:0,Math.abs(error)>0.04?Math.sign(error):0,0,0);
 }
 assert.ok(done(output()),`reach transit ${JSON.stringify(destination)}: ${JSON.stringify(output().tank)}`);
}
drive(transits.find(p=>p.kind===37&&p.x===464),state=>state.vertical.transition<0);
for(let tick=0;tick<25;tick++)w.game_frame(1/60,0,0,1,0);
assert.equal(output().vertical.renderer_height,-96);assert.equal(output().vertical.transition,0);snapshot('submerged');
drive(transits.find(p=>p.kind===36&&p.x===560),state=>state.vertical.transition>0);
for(let tick=0;tick<25;tick++)w.game_frame(1/60,0,0,1,0);
assert.equal(output().vertical.vertical,0);assert.equal(output().vertical.renderer_height,32);snapshot('returned');
w.game_reset();w.game_frame(0,0,0,0,0);assert.deepEqual(output(),initial);
console.log('Passed: WASM drives through original Central 10110 down/up transit placements, submerges, resurfaces, and resets without teleporting.');
