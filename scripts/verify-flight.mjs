import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
for(const [path,load] of [['.local/extracted/spectre.bin','load_shared_colors'],['.local/extracted/central.bin','load_archive']]){
 const bytes=readFileSync(path),ptr=w.input_buffer(bytes.length);
 new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);assert.equal(w[load](),1);
}
assert.equal(w.game_start_campaign_level(1,0),1);w.game_encounter(0);w.game_frame(0,0,0,0,0);
const initial=output(),destination=initial.pickups.find(p=>p.subtype===3)?.item;
assert.ok(destination,'opening campaign contains an original blue pickup');
for(let tick=0;tick<1000&&!output().flight_energy;tick++){
 const state=output(),dx=destination.x-state.tank.x,dz=destination.z-state.tank.z;
 const desired=Math.atan2(dx,dz),error=Math.atan2(Math.sin(desired-state.tank.heading),Math.cos(desired-state.tank.heading));
 w.game_frame(1/60,Math.abs(error)<0.35?1:0,Math.abs(error)>0.04?Math.sign(error):0,0,0);
}
assert.ok(output().flight_energy>0,'driven tank collects blue flight energy');
for(let tick=0;tick<100;tick++)w.game_frame(1/60,0,0,1,0);
w.game_frame(0,0,0,0,0); // Drain transient audio before comparing view-only frames.
const airborne=output();
assert.ok(airborne.vertical.vertical>60*65536,'tank rises above the ordinary collision-height gate');
assert.ok(airborne.flight_energy<400&&airborne.flight_energy>0,'flight energy is consumed');
for(const mode of [0,1,2]){
 w.game_camera(mode);const frame=w.game_frame(0,0,0,0,0);
 assert.deepEqual(output(),airborne,'camera change preserves airborne simulation');
 if(process.argv.includes('--snapshot'))execFileSync('ffmpeg',['-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-i','pipe:0','-frames:v','1',`.local/flight-view-${mode}.png`],{input:Buffer.from(new Uint8Array(w.memory.buffer,frame,800*600*4)),stdio:['pipe','ignore','pipe']});
}
w.game_frame(1/60,0,0,1,1);
assert.ok(output().shots.some(s=>s.height>4),'shots launch above ground while flying');
let depletion;
for(let tick=0;tick<800;tick++){
 w.game_frame(1/60,0,0,1,0);const state=output();
 if(!state.flight_energy&&!depletion)depletion=state;
}
assert.ok(depletion&&!depletion.vertical.active&&depletion.vertical.hover>0,'depletion starts gradual descent');
assert.equal(output().vertical.vertical,0,'tank lands after energy is exhausted');
w.game_reset();w.game_encounter(0);w.game_frame(0,0,0,0,0);
assert.deepEqual(output(),initial,'reset restores all flight and pickup state');
console.log('Passed: drive to blue pickup, take off, consume energy, switch views, fire aloft, descend, land, and reset in WASM.');
