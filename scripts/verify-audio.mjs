import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {GAME_SOUND_IDS} from '../native-web/audio.js';
const manifest=JSON.parse(readFileSync('.local/audio/manifest.json'));
assert.ok(GAME_SOUND_IDS.every(id=>manifest.some(entry=>entry.id===id)),'all browser cues have exported original samples');
for(const entry of manifest) {
  const original=readFileSync(`.local/game-resources/736e6420/${entry.id}.bin`);
  const format=original.readUInt16BE(0);
  const commands=format===1?4+original.readUInt16BE(2)*6:4;
  assert.equal(original.readUInt16BE(commands),1);
  const header=original.readUInt32BE(commands+6);
  const length=original.readUInt32BE(header+4);
  const wav=readFileSync(`.local/audio/${entry.id}.wav`);
  assert.equal(wav.toString('ascii',0,4),'RIFF');
  assert.equal(wav.readUInt32LE(4)+8,wav.length);
  assert.equal(wav.readUInt32LE(40),length);
  assert.deepEqual(wav.subarray(44,44+length),original.subarray(header+22,header+22+length),'PCM bytes preserved');
  assert.equal(wav.readUInt32LE(24),Math.round(original.readUInt32BE(header+8)/65536));
}
const {instance}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const w=instance.exports;
const data=readFileSync('.local/extracted/central.bin');const ptr=w.input_buffer(data.length);new Uint8Array(w.memory.buffer,ptr,data.length).set(data);
assert.equal(w.load_archive(),1);
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const index=output().findIndex(e=>e.kind==='wrld'&&e.id===10000);assert.equal(w.game_start(index),1);
let shots=0,hits=0,explosions=0,empty=0,lowAmmo=0,previousDestroyed=0;
for(let i=0;i<600;i++) {
  w.game_frame(1/60,0,0,0,1);
  const state=output();
  const events=state.audio_events;
  if(state.destroyed>previousDestroyed) {
    assert.equal(state.destroyed-previousDestroyed,1,'this fixture kills one target per frame');
    assert.equal(events.filter(id=>id===10005).length,1,'one original death response');
    assert.equal(events.filter(id=>id===10004).length,0,'fatal hit does not also emit nonfatal response');
  }
  previousDestroyed=state.destroyed;
  lowAmmo+=events.filter(id=>id===10010).length;
  if(events.includes(10010)) {
    assert.ok(GAME_SOUND_IDS.includes(10010),'browser preloads low-ammo sample');
    assert.ok(events.indexOf(10010)<events.indexOf(10001),'warning precedes the accepted shot sound');
  }
  shots+=events.filter(id=>id===10001).length;hits+=events.filter(id=>id===10004).length;
  explosions+=events.filter(id=>id===10005).length;empty+=events.filter(id=>id===10006).length;
  w.game_frame(0,0,0,0,0);assert.deepEqual(output().audio_events,[],'redraw cannot repeat sounds');
}
assert.equal(lowAmmo,1);assert.equal(shots,40);assert.equal(hits,0,'one-hit target has no nonfatal response');assert.equal(explosions,1);assert.ok(empty>0);
w.game_reset();w.game_frame(0,0,0,0,0);assert.deepEqual(output().audio_events,[]);
console.log(`Passed: ${manifest.length} original samples preserved; ${shots} shot sounds, ${hits} hits, ${explosions} explosions, ${empty} empty-ammo events, ${lowAmmo} low-ammo warning; no redraw/reset duplicates.`);
