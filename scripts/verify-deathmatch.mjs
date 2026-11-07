import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const w=(await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{})).instance.exports;
const text=()=>new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len()));
const feed=bytes=>{const p=w.input_buffer(bytes.length);new Uint8Array(w.memory.buffer,p,bytes.length).set(bytes);};
for(const [name,fn] of [['spectre','load_shared_colors'],['central','load_archive']]){feed(readFileSync(`.local/extracted/${name}.bin`));assert.equal(w[fn](),1);}
assert.equal(w.dm_start(),1);
w.dm_input(0,0,0,0,1);
for(let i=0;i<6000;i++)w.dm_step();
w.dm_snapshot();const won=JSON.parse(text());
assert.equal(won.winner,0);assert.equal(won.players[0].kills,5);assert.equal(won.players[1].deaths,5);
assert.ok(won.tick<6000,'match stops at victory');
assert.equal(w.dm_start(),1);w.dm_snapshot();const fresh=JSON.parse(text());
assert.equal(fresh.tick,0);assert.equal(fresh.winner,null);assert.deepEqual(fresh.players.map(p=>p.kills),[0,0]);
assert.equal(w.dm_view_start(),1);
const frames=[];
for(let slot=0;slot<2;slot++){
  feed(new TextEncoder().encode(JSON.stringify(fresh)));
  const p=w.dm_render(slot);assert.ok(p);
  const frame=Buffer.from(new Uint8Array(w.memory.buffer,p,800*600*4));frames.push(frame);
  assert.ok(frame.some((v,i)=>i%4!==3&&v>150),'original arena renders');
}
assert.notDeepEqual(frames[0],frames[1],'each player has their own viewpoint');
console.log('PASS: original arena cannon hits, five kills, respawns, winner freeze, reset, and both Rust-rendered viewpoints.');
// Local motion must respond before an acknowledgement and replay deterministically.
assert.equal(w.dm_start(),1);
assert.equal(w.dm_prediction_start(),1);
w.dm_snapshot();const initial=JSON.parse(text());
const restore=player=>{feed(new TextEncoder().encode(JSON.stringify(player)));assert.equal(w.dm_predict_restore(),1);};
restore(initial.players[0]);
w.dm_predict_step(1,1,0,0);
const immediate=JSON.parse(text());
assert.notEqual(immediate.heading,initial.players[0].heading,'steering responds before the server steps');
restore(initial.players[0]);
const commands=Array.from({length:90},(_,i)=>[i<60?1:-1,i<30?1:-1,0,i%13===0?1:0]);
const checkpoints=[];
for(const c of commands){
  w.dm_input(0,...c);w.dm_step();w.dm_snapshot();checkpoints.push(JSON.parse(text()).players[0]);
  w.dm_predict_step(...c);const predicted=JSON.parse(text());
  for(const k of ['x','z','heading','speed'])assert.equal(Math.fround(predicted[k]),Math.fround(checkpoints.at(-1)[k]),`same movement ${k}`);
}
// Simulate snapshots arriving 12 ticks late (200 ms), replaying unacknowledged input.
for(let tick=12;tick<90;tick+=7){
  restore(checkpoints[tick-12]);
  for(let i=tick-11;i<=tick;i++)w.dm_predict_step(...commands[i]);
  const predicted=JSON.parse(text());
  for(const k of ['x','z','heading','speed'])assert.equal(Math.fround(predicted[k]),Math.fround(checkpoints[tick][k]),`delayed reconciliation ${k}`);
}
console.log('PASS: immediate local steering, identical Rust motion, and 200 ms delayed checkpoint replay.');
