import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const shared=readFileSync('.local/extracted/spectre.bin');
const sharedInput=w.input_buffer(shared.length);
new Uint8Array(w.memory.buffer,sharedInput,shared.length).set(shared);
assert.equal(w.load_shared_colors(),1);
const bytes=readFileSync('.local/extracted/central.bin');
const input=w.input_buffer(bytes.length);
new Uint8Array(w.memory.buffer,input,bytes.length).set(bytes);
assert.equal(w.load_archive(),1);
assert.equal(w.game_start_mission(output().findIndex(e=>e.kind==='wrld'&&e.id===10000)),1);
let state;
w.game_frame(0,0,0,0,0);
const initialHull=output().encounter.health;
w.game_infinite_health(1);
for(let i=0;i<1800;i++) {
  w.game_frame(1/60,0,0,0,0);state=output();
  assert.equal(state.encounter.health,initialHull,'infinite health prevents hull loss');
  assert.equal(state.death_transition,null,'infinite health prevents death');
}
assert.ok(state.combat_counters.player_damage_events>initialHull,'enemy hits still register');
w.game_infinite_health(0);

for(let i=0;i<3000;i++) {
  w.game_frame(1/60,0,0,0,0);state=output();
  if(state.death_transition)break;
}
assert.ok(state.death_transition,'authored arena reaches local death');
const initial=state;
assert.equal(initial.death_transition.deadline-initial.tank.ticks,260);
const pixels=ptr=>Buffer.from(new Uint8Array(w.memory.buffer,ptr,800*600*4));
w.game_camera(0);
const first=pixels(w.game_frame(0,0,0,0,0));
w.game_camera(2);
const firstMap=pixels(w.game_frame(0,0,0,0,0));
assert.notDeepEqual(firstMap,first,'overhead death stays in 2D');
const center=frame=>Buffer.concat(Array.from({length:64},(_,y)=>frame.subarray(((268+y)*800+368)*4,((268+y)*800+432)*4)));
const gridMask=frame=>Buffer.from(Array.from({length:800*600},(_,i)=>Number(frame[i*4]===24&&frame[i*4+1]===43&&frame[i*4+2]===51)));
w.game_camera(0);
for(let elapsed=1;elapsed<260;elapsed++) {
  const ptr=w.game_frame(1/60,1,1,1,1);state=output();
  assert.deepEqual(state.death_transition,initial.death_transition);
  assert.deepEqual([state.tank.x,state.tank.z,state.tank.heading],[initial.tank.x,initial.tank.z,initial.tank.heading]);
  assert.equal(state.fired,initial.fired);assert.equal(state.ammo,initial.ammo);
  assert.equal(state.encounter.health,0);assert.equal(state.encounter.status,'playing');
  assert.equal(state.mission.lives,initial.mission.lives);
  assert.equal(state.combat_counters.player_damage_events,initial.combat_counters.player_damage_events);
  if(elapsed===20 || elapsed===120) {
    const current=pixels(ptr);assert.notDeepEqual(current,first,'death camera changes rendered frame');
    if(elapsed===120) {
      let groundPoints=0;
      const groundColor=state.palette[state.ground_color_index].map(channel=>channel>>8);
      for(let i=0;i<current.length;i+=4)if(groundColor.every((channel,c)=>current[i+c]===channel))groundPoints++;
      assert.ok(groundPoints>0,'mid-transition ground remains visible with original height units');
    }
    for(const mode of [0,1]) {
      w.game_camera(mode);
      assert.deepEqual(pixels(w.game_frame(0,0,0,0,0)),current,'3D views retain death camera');
      assert.deepEqual(output().audio_events,[],'redraw cannot repeat death audio');
    }
    w.game_camera(2);
    const map=pixels(w.game_frame(0,0,0,0,0));
    assert.notDeepEqual(map,current,'overhead never switches to first person');
    assert.notDeepEqual(center(map),center(firstMap),'tank symbol spins in 2D');
    assert.notDeepEqual(gridMask(map),gridMask(firstMap),'arena grid rotates and pulls back during death');
    assert.deepEqual(output().audio_events,[],'map redraw cannot repeat death audio');
    w.game_camera(0);
    if(process.argv.includes('--snapshot')) {
      const {execFileSync}=await import('node:child_process');
      execFileSync('ffmpeg',['-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-i','pipe:0','-frames:v','1',`.local/death-camera-${elapsed}.png`],{input:current,stdio:['pipe','ignore','pipe']});
    }
  }
}
w.game_frame(1/60,1,1,1,1);state=output();
assert.equal(state.tank.ticks,initial.death_transition.deadline);
assert.equal(state.death_transition,null);assert.equal(state.mission.lives,initial.mission.lives-1);
assert.equal(state.mission.deaths,initial.mission.deaths+1);assert.ok(state.encounter.health>0);
w.game_reset();w.game_frame(0,0,0,0,0);assert.equal(output().death_transition,null);
console.log('Passed: 260-tick WASM death transition, blocked controls, stable hit counter, 3D death camera and spinning 2D tank, one respawn, reset, and redraw audio.');
