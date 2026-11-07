import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const bytes=readFileSync('.local/extracted/central.bin');
const ptr=w.input_buffer(bytes.length);
new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);
assert.equal(w.load_archive(),1);
const frame=(seconds,held)=>{w.game_frame_controls(seconds,0,0,0,0,Number(held));return output();};
assert.equal(w.game_start_campaign_level(3,0),1);
const initial=frame(0,false).optional_weapons;
assert.equal(initial.selected,1);
for(let i=0;i<10;i++)assert.equal(frame(1/60,true).optional_weapons.selected,1);
let state=frame(1/60,true);
assert.equal(state.optional_weapons.selected,13);
assert.equal(state.audio_events.filter(id=>id===10034).length,1);
const tick=state.tank.ticks;
for(let i=0;i<20;i++){
  state=frame(0,true);
  assert.equal(state.optional_weapons.selected,13);
  assert.equal(state.tank.ticks,tick);
  assert.ok(!state.audio_events.includes(10034),'redraw does not repeat selection sound');
}
for(let i=0;i<10;i++)assert.equal(frame(1/60,true).optional_weapons.selected,13);
state=frame(1/60,true);assert.equal(state.optional_weapons.selected,17);
assert.ok(state.audio_events.includes(10034));
for(let i=0;i<20;i++)assert.equal(frame(1/60,false).optional_weapons.selected,17,'released button does not cycle');
w.game_reset();assert.deepEqual(frame(0,false).optional_weapons,initial);
for(let i=0;i<10;i++)assert.equal(frame(1/60,true).optional_weapons.selected,1,'reset clears repeat timer');
assert.equal(frame(1/60,true).optional_weapons.selected,13);
assert.equal(w.game_start_campaign_level(1,0),1);
for(let i=0;i<33;i++){
  state=frame(1/60,true);assert.equal(state.optional_weapons.selected,13);
  assert.ok(!state.audio_events.includes(10034),'one available weapon produces no change sound');
}
console.log('Passed: WASM optional selection at ticks 11/22, repeated hold, release, zero-time redraw, audio, reset, and single-weapon silence.');
