import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const path='.local/extracted/central.bin',bytes=readFileSync(path),ptr=w.input_buffer(bytes.length);
new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);assert.equal(w.load_archive(),1);
const frame=(seconds,fire)=>{w.game_frame_weapons(seconds,0,0,0,0,0,Number(fire));return output();};
assert.equal(w.game_start_campaign_level(11,0),1);
const initial=frame(0,false);assert.equal(initial.optional_weapons.selected,5);
for(let i=0;i<15;i++)assert.equal(frame(1/60,true).fired,0);
let state=frame(1/60,true);
assert.equal(state.fired,3);assert.equal(state.ammo,initial.ammo-3);
assert.ok(state.audio_events.includes(10038),'Scattershot uses the original sound');
for(let i=0;i<15;i++)assert.equal(frame(1/60,true).fired,3);
state=frame(1/60,true);assert.equal(state.fired,6);assert.equal(state.ammo,initial.ammo-6);
const native=spawnSync('cargo',['run','--quiet','--locked','-p','spectre-tools','--','replay-scattershot',path],{encoding:'utf8'});
assert.equal(native.status,0,native.stderr);
const expected=JSON.parse(native.stdout);
function close(a,b){
 if(typeof a==='number')assert.ok(Math.abs(a-b)<.001,`${a} != ${b}`);
 else if(a&&typeof a==='object'){assert.deepEqual(Object.keys(a),Object.keys(b));for(const key of Object.keys(a))close(a[key],b[key]);}
 else assert.equal(a,b);
}
for(const key of Object.keys(expected))close(state[key],expected[key]);
w.game_reset();state=frame(0,false);assert.equal(state.fired,0);assert.equal(state.ammo,initial.ammo);
assert.deepEqual(state.optional_weapons,initial.optional_weapons);
assert.equal(w.game_start_campaign_level(1,0),1);const unsupported=frame(0,false);
for(let i=0;i<32;i++)state=frame(1/60,true);
assert.equal(state.fired,0,'unimplemented optional weapons do not masquerade as cannon shots');
assert.equal(state.ammo,unsupported.ammo);
console.log('Passed: original-level Scattershot at ticks 16/32, resource ammo charges, sound, native/WASM replay parity, reset, and unsupported-weapon guard.');
