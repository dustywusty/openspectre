import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
const {instance:{exports:w}} = await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'), {});
const output = () => JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
let count=0, rated=0, unlinked=0, incomplete=0;
const modes={};
for (const campaign of ['central','eastern','south']) {
  const path = `.local/extracted/${campaign}.bin`;
  const bytes=readFileSync(path);
  const ptr=w.input_buffer(bytes.length);
  new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);
  assert.equal(w.load_archive(),1);
  const entries=output();
  for (let i=0;i<entries.length;i++) {
    if(entries[i].kind!=='wrld') continue;
    assert.equal(w.game_start_mission(i),1);
    const scene=output();
    const bvalId=scene.scenario.match(/bval (\d+)/)?.[1];
    if(bvalId) {
      const values=readFileSync(`.local/${campaign}-shape-resources/6276616c/${bvalId}.bin`);
      const scores=new Map(Array.from({length:values.readUInt16BE(0)},(_,row)=>{
        const offset=2+28*row;
        return [values.readInt8(offset+1),values.readInt32BE(offset+18)];
      }));
      for(const target of scene.targets) assert.equal(target.kill_score,(target.item.kind===5?10:scores.get(target.item.kind))??10,
        'target score matches original bval bytes or recovered default');
    }

    const flags=scene.items.filter(i=>i.kind===12);
    assert.equal(flags.length,campaign==='eastern' && entries[i].id===10017 ? 2 : 3);
    assert.ok(flags.every(i=>i.shape_id===12 && i.source==='recovered procedural vertices'), 'original flag model resolved');
    w.game_frame(0,0,0,0,0);
    const initial=output();
    assert.deepEqual(initial.flags,{enabled:true,total:flags.length,collected:0,pickup_points:0});
    assert.equal(initial.encounter.enabled,true);
    if(scene.scenario==='No wave references this map'){
      assert.equal(initial.par_seconds,null);unlinked++;
    }else if(scene.scenario.includes('completion ratings unavailable: incomplete enemy plan')){
      assert.equal(initial.par_seconds,null);incomplete++;
    }else{
      assert.ok(Number.isInteger(initial.par_seconds)&&initial.par_seconds>0,`mission ${campaign}/${entries[i].id} has target time ${initial.par_seconds}; ${scene.scenario}`);rated++;
    }
    if((campaign==='central'||campaign==='south')&&entries[i].id===10100){
      const defaults=scene.targets.filter(t=>t.item.kind===45);
      assert.equal(defaults.length,5,'omitted bval row retains all five planned type-45 enemies');
      assert.ok(defaults.every(t=>t.health===1&&t.kill_score===10));
      assert.equal(initial.par_seconds,150,'default bonus weight contributes to target time');
    }
    assert.equal(initial.completion,null);
    modes[initial.collision_mode]=(modes[initial.collision_mode]??0)+1;
    for(let tick=0;tick<10;tick++) w.game_frame(1/60,1,0,0,0);
    assert.equal(output().encounter.status,'playing');
    w.game_reset();w.game_frame(0,0,0,0,0);
    assert.deepEqual(output(),initial,'reset restores mission state');
    count++;
  }
  const i=entries.findIndex(e=>e.kind==='wrld' && e.id===10000);
  assert.equal(w.game_start_mission(i),1);
  for(let tick=0;tick<240;tick++) w.game_frame(1/60,1,.4,0,1);
  const wasm=output();
  const replay=spawnSync('cargo',['run','--quiet','--locked','-p','spectre-tools','--','replay',path,'mission'],{encoding:'utf8'});
  assert.equal(replay.status,0,replay.stderr);
  const native=JSON.parse(replay.stdout);
  assert.deepEqual(wasm.flags,native.flags);
  assert.deepEqual(wasm.mission,native.mission);
  assert.deepEqual(wasm.encounter,native.encounter);
  assert.equal(wasm.tank.ticks,native.tank.ticks);
  assert.ok(Math.abs(wasm.tank.x-native.tank.x)<.001);
  assert.ok(Math.abs(wasm.tank.z-native.tank.z)<.001);
  if (campaign === 'central') {
    assert.equal(w.game_start_mission(i),1);
    w.game_camera(2); w.game_zoom(-2);
    const frame=w.game_frame(0,0,0,0,0);
    const pixels=new Uint8Array(w.memory.buffer,frame,800*600*4);
    assert.ok(pixels.some((v,k)=>k%4===0 && v===255 && pixels[k+1]===240 && pixels[k+2]===40),
      'flag geometry produces visible yellow pixels');
    w.game_camera(0);
    // Drive through the actual authored flag at (624,240), without changing
    // simulation coordinates or disabling enemies.
    for(let tick=0;tick<1200 && output().flags.collected===0;tick++) {
      const t=output().tank;
      const aim=Math.atan2(624-t.x,240-t.z);
      const delta=Math.atan2(Math.sin(aim-t.heading),Math.cos(aim-t.heading));
      w.game_frame(1/60,Math.abs(delta)<.2?1:0,Math.abs(delta)>.025?Math.sign(delta):0,0,1);
    }
    assert.equal(output().flags.collected,1,'driving collects an authored flag');
    assert.equal(output().flags.pickup_points,20);
    assert.ok(output().audio_events.includes(10017),'authored flag pickup emits original flagPull cue');
    assert.equal(output().mission.score,20 + output().enemy_clear_award + output().targets.reduce((sum,t)=>sum+t.deaths*((t.kill_score<<16)>>16),0));
    assert.equal(output().mission.lives,3);
    assert.equal(output().encounter.status,'playing');
    assert.equal(output().collision_mode,'recovered query / preview records');
    w.game_reset();w.game_frame(0,0,0,0,0);
    assert.equal(output().flags.collected,0,'restart restores collected authored flag');
    for(let tick=0;tick<180;tick++) w.game_frame(1/60,0,0,0,1);
    const combat=output();
    assert.ok(combat.destroyed>0,'actual WASM shots destroy an enemy');
    const killed=combat.targets.filter(t=>t.deaths>0);
    assert.ok(killed.every(t=>t.kill_score>0));
    assert.ok(killed.some(t=>t.health>0&&!t.retired),'killed mission enemy respawns while lives remain');
    assert.equal(combat.destroyed,combat.targets.reduce((sum,t)=>sum+t.deaths,0));
    assert.equal(combat.mission.score,combat.enemy_clear_award+killed.reduce((sum,t)=>sum+t.deaths*((t.kill_score<<16)>>16),0),
      'combat awards recovered resource points for each killed enemy');
    assert.equal(combat.flags.collected,0,'combat score is independent of flags');
    w.game_reset();w.game_frame(0,0,0,0,0);
    assert.equal(output().mission.score,0,'restart resets combat score');

  }

}
console.log(`Passed: ${count} authored flag missions start and reset; three desktop/WASM mission replays agree.`);
console.log('Mission collision paths:',modes);

console.log(`Completion target times: ${rated} linked arenas; ${unlinked} unlinked layouts and ${incomplete} incomplete enemy plans explicitly omit ratings.`);
