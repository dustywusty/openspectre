import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const bytes=readFileSync('.local/extracted/central.bin');
const input=w.input_buffer(bytes.length);
new Uint8Array(w.memory.buffer,input,bytes.length).set(bytes);
assert.equal(w.load_archive(),1);
const index=output().findIndex(e=>e.kind==='wrld'&&e.id===10000);
assert.equal(w.game_start(index),1); w.game_encounter(1); w.game_frame(0,0,0,0,0);
const initial=output();
for(let i=0;i<240;i++)w.game_frame(1/60,1,.4,0,1);
const native=spawnSync('cargo',['run','--quiet','--locked','-p','spectre-tools','--','replay','.local/extracted/central.bin','encounter'],{encoding:'utf8'});
assert.equal(native.status,0,native.stderr);
function close(a,b) {
 if(typeof a==='number')assert.ok(Math.abs(a-b)<.001,`${a} != ${b}`);
 else if(a&&typeof a==='object'){assert.deepEqual(Object.keys(a),Object.keys(b));for(const k of Object.keys(a))close(a[k],b[k]);}
 else assert.equal(a,b);
}
close(output(),JSON.parse(native.stdout));
w.game_reset();w.game_frame(0,0,0,0,0);assert.deepEqual(output(),initial);
const audio=[];
for(let i=0;i<1200;i++){w.game_frame(1/60,0,0,0,0);audio.push(...output().audio_events);}
const state=output();
assert.ok(state.targets.some((t,i)=>t.item.x!==initial.targets[i].item.x||t.item.z!==initial.targets[i].item.z),'actual Central enemies move');
assert.ok(state.encounter.enemy_fired>0,'actual Central enemies fire');
assert.ok(state.encounter.health<6,'actual Central enemies can hit the player');
assert.ok(audio.includes(10001)&&audio.includes(10003),'enemy shots and player hits produce audio');
w.game_encounter(0);w.game_frame(0,0,0,0,0);
assert.equal(output().encounter.enabled,false);assert.equal(output().tank.ticks,0);
console.log('Passed: encounter native/WASM replay parity; reset; Central pursuit, shooting, player damage and audio; research mode.');
console.log({enemyShots:state.encounter.enemy_fired,hull:state.encounter.health,status:state.encounter.status});
