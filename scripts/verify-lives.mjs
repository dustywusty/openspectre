import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const bytes=readFileSync('.local/extracted/central.bin'),ptr=w.input_buffer(bytes.length);
new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);
assert.equal(w.load_archive(),1);
const index=output().findIndex(e=>e.kind==='wrld'&&e.id===10000);
assert.equal(w.game_start_mission(index),1);
w.game_frame(0,0,0,0,0);
const initial=output();
assert.deepEqual(initial.mission,{score:0,lives:3,deaths:0});
let previous=initial,respawns=0;
for(let tick=0;tick<4000 && previous.encounter.status==='playing' && respawns<1;tick++){
  w.game_frame(1/60,0,0,0,0);
  const state=output();
  assert.equal(state.combat_counters.player_shots,state.fired);
  assert.equal(state.combat_counters.other_shots,state.encounter.enemy_fired);
  if(state.death_transition) {
    assert.equal(state.death_transition.deadline-state.death_transition.started,260);
    assert.equal(state.encounter.health,0);
    if(!previous.death_transition) {
      assert.equal(state.combat_counters.player_damage_events,previous.combat_counters.player_damage_events+1,'fatal hit counted at transition start');
    }
    if(previous.death_transition) {
      assert.equal(state.mission.lives,previous.mission.lives);
      assert.equal(state.combat_counters.player_damage_events,previous.combat_counters.player_damage_events);
      assert.deepEqual([state.tank.x,state.tank.z],[previous.tank.x,previous.tank.z]);
    }
  }
  if(state.mission.deaths>previous.mission.deaths){
    assert.equal(state.mission.lives,previous.mission.lives-1);
    assert.equal(state.combat_counters.player_damage_events,previous.combat_counters.player_damage_events,'transition does not add another hit');
    assert.ok(previous.death_transition,'respawn follows a death transition');
    assert.equal(state.tank.ticks,previous.death_transition.deadline,'transition completes at original deadline');
    assert.equal(state.death_transition,null);
    assert.equal(state.mission.score,previous.mission.score);
    assert.deepEqual(state.flags,previous.flags);
    assert.equal(state.tank.ticks,previous.tank.ticks+1,'respawn preserves clock');
    assert.ok(state.destroyed>=previous.destroyed,'enemy progress never resets');
    if(state.mission.lives>0){
      respawns++;
      assert.equal(state.encounter.status,'playing');
      assert.equal(state.encounter.health,6);
      assert.equal(state.ammo,40);
      assert.equal(state.tank.speed,0);
      assert.notDeepEqual([state.tank.x,state.tank.z],[previous.tank.x,previous.tank.z]);
      assert.ok(state.tank.x>=100&&state.tank.x<=924&&state.tank.z>=100&&state.tank.z<=924);
    } else {
      assert.equal(state.encounter.status,'lost');
    }
  }
  previous=state;
}
assert.equal(respawns,1,'authored arena produces a playable respawn');
w.game_reset();w.game_frame(0,0,0,0,0);
assert.deepEqual(output(),initial,'restart resets the entire mission');
// Repeated-death coverage uses an explicit open-field fixture. After the
// recovered death reset, authored-arena enemies can choose to guard flags.
// Change only an in-memory copy; original archive bytes on disk stay intact.
const fixture=Buffer.from(bytes);
const base=128+Math.ceil(fixture.readUInt16BE(120)/128)*128+Math.ceil(fixture.readUInt32BE(83)/128)*128;
const raw=readFileSync('.local/central-shape-resources/77726c64/10000.bin');
const offset=fixture.indexOf(raw,base);
assert.ok(offset>=base,'locate exact authored world payload');
for(let i=0;i<raw.readUInt16BE(0);i++){
  const row=offset+2+i*8;
  fixture.writeInt16BE(14,row);
  fixture.writeInt16BE(0,row+2);fixture.writeInt16BE(0,row+4);
}
const enemies=readFileSync('.local/central-shape-resources/656e656d/10000.bin');
const enemyOffset=fixture.indexOf(enemies,base);
assert.ok(enemyOffset>=base,'locate enemy resource for open-field fixture');
for(let i=0;i<enemies.readUInt16BE(0);i++){
  const row=enemyOffset+2+i*8;
  const count=enemies.readInt16BE(2+i*8)===7?1:0;
  fixture.writeInt16BE(count,row+2);fixture.writeInt16BE(count,row+4);
  fixture.writeInt16BE(1,row+6);
}
const fixturePtr=w.input_buffer(fixture.length);
new Uint8Array(w.memory.buffer,fixturePtr,fixture.length).set(fixture);
assert.equal(w.load_archive(),1);
const fixtureIndex=output().findIndex(e=>e.kind==='wrld'&&e.id===10000);
assert.equal(w.game_start_mission(fixtureIndex),1);
// The enemy-clear checks below need the original finite rebirth limits.
w.game_enemy_respawn(0);
assert.equal(output().targets.length,1,'open-field fixture has one shooter');
w.game_frame(0,0,0,0,0);previous=output();
let fixtureRespawns=0;
for(let tick=0;tick<12000&&previous.encounter.status==='playing';tick++){
  w.game_frame(1/60,0,0,0,0);
  const state=output();
  if(state.death_transition) {
    assert.equal(state.death_transition.deadline-state.death_transition.started,260);
    assert.equal(state.encounter.health,0);
    if(!previous.death_transition) {
      assert.equal(state.combat_counters.player_damage_events,previous.combat_counters.player_damage_events+1,'fatal hit counted at transition start');
    }
    if(previous.death_transition) {
      assert.equal(state.mission.lives,previous.mission.lives);
      assert.equal(state.combat_counters.player_damage_events,previous.combat_counters.player_damage_events);
      assert.deepEqual([state.tank.x,state.tank.z],[previous.tank.x,previous.tank.z]);
    }
  }
  if(state.mission.deaths>previous.mission.deaths){
    assert.equal(state.mission.lives,previous.mission.lives-1);
    assert.equal(state.combat_counters.player_damage_events,previous.combat_counters.player_damage_events,'transition does not add another hit');
    assert.ok(previous.death_transition,'respawn follows a death transition');
    assert.equal(state.tank.ticks,previous.death_transition.deadline,'transition completes at original deadline');
    assert.equal(state.death_transition,null);
    assert.equal(state.mission.score,previous.mission.score);
    assert.deepEqual(state.flags,previous.flags);
    assert.equal(state.tank.ticks,previous.tank.ticks+1,'respawn preserves clock');
    assert.ok(state.destroyed>=previous.destroyed,'enemy progress never resets');
    if(state.mission.lives>0){
      fixtureRespawns++;
      assert.equal(state.encounter.status,'playing');
      assert.equal(state.encounter.health,6);
      assert.equal(state.ammo,40);
      assert.equal(state.tank.speed,0);
      assert.notDeepEqual([state.tank.x,state.tank.z],[previous.tank.x,previous.tank.z]);
      assert.ok(state.tank.x>=100&&state.tank.x<=924&&state.tank.z>=100&&state.tank.z<=924);
    } else {
      assert.equal(state.encounter.status,'lost');
    }
  }
  previous=state;
}
assert.equal(fixtureRespawns,2,'two playable open-field respawns');
assert.equal(previous.mission.lives,0);
assert.equal(previous.encounter.status,'lost');
const finalTicks=previous.tank.ticks;
w.game_frame(.1,1,1,0,1);
assert.equal(output().tank.ticks,finalTicks,'final death stops simulation');
w.game_reset();
let enemyClearSounds=0;
for(let tick=0;tick<180;tick++){
  w.game_frame(1/60,0,0,0,1);
  enemyClearSounds+=output().audio_events.filter(id=>id===10044).length;
}
assert.equal(enemyClearSounds,1,'original enemy-clear cue occurs once');
const cleared=output();
assert.equal(cleared.targets[0].retired,true,'single enemy permanently retires');
assert.equal(cleared.enemy_clear_award,50,'one initial enemy life awards 50 points at level one');
assert.equal(cleared.mission.score,50+((cleared.targets[0].kill_score<<16)>>16));
assert.equal(cleared.encounter.status,'playing','enemy-clear award does not replace flag objective');
for(let tick=0;tick<60;tick++){
  w.game_frame(1/60,0,0,0,1);
  assert.ok(!output().audio_events.includes(10044),'enemy-clear cue cannot repeat');
}
assert.equal(output().mission.score,cleared.mission.score,'enemy-clear award happens once');
w.game_reset();w.game_frame(0,0,0,0,0);
assert.equal(output().enemy_clear_award,0,'reset clears the award');
assert.equal(output().mission.score,0);
assert.deepEqual(output().combat_counters,{player_shots:0,player_hits:0,other_shots:0,player_damage_events:0});
// Completion fixture: three flags overlap the initial tank position.
const completionSpawn=output().tank;
for(let i=0;i<3;i++){
  fixture.writeInt16BE(Math.round(completionSpawn.x),offset+2+i*8+2);
  fixture.writeInt16BE(Math.round(completionSpawn.z),offset+2+i*8+4);
}
const completionPtr=w.input_buffer(fixture.length);
new Uint8Array(w.memory.buffer,completionPtr,fixture.length).set(fixture);
assert.equal(w.load_archive(),1);
assert.equal(w.game_start_mission(output().findIndex(e=>e.kind==='wrld'&&e.id===10000)),1);
for(let tick=0;tick<10;tick++)w.game_frame(1/60,0,0,0,0);
const completed=output();
assert.equal(completed.encounter.status,'won');
assert.equal(completed.flags.collected,3);
assert.equal(completed.par_seconds,45);
assert.deepEqual(completed.completion.bonuses,[800,0,0]);
assert.deepEqual(completed.completion.unrated,[true,true]);
assert.equal(completed.mission.score,860,'three flags plus quickness bonus');
w.game_frame(.1,1,1,0,1);
assert.equal(output().mission.score,860,'completion cannot award twice');
assert.equal(output().tank.ticks,completed.tank.ticks,'completion freezes gameplay');
w.game_reset();w.game_frame(0,0,0,0,0);
assert.equal(output().completion,null);
assert.equal(output().mission.score,0);
console.log('Passed: authored-arena WASM combat refills on a randomized respawn, preserves progress, and restarts; an explicit open-field fixture verifies both respawn refills, three deaths, final freeze, the one-time last-enemy bonus, and completion ratings/bonuses.');

// The same explicit completion fixture exercises real campaign advancement.
assert.equal(w.game_start_campaign_level(1,0),1);
assert.equal(w.game_next_campaign_level(),0,'cannot skip an unfinished level');
for(let tick=0;tick<10;tick++)w.game_frame(1/60,0,0,0,0);
const campaignWon=output();
assert.equal(campaignWon.encounter.status,'won');
assert.equal(w.game_next_campaign_level(),1);
assert.equal(output().campaign.level,2);
w.game_frame(0,0,0,0,0);
const entered=output();
assert.deepEqual(entered.mission,campaignWon.mission,'score, lives and diagnostic deaths carry forward');
assert.equal(entered.encounter.health,6);assert.equal(entered.ammo,40);
assert.equal(entered.completion,null);assert.equal(entered.flags.collected,0);
assert.equal(entered.tank.ticks,0);
assert.equal(w.game_next_campaign_level(),0,'cannot advance twice from one completion');
w.game_frame(0,0,0,0,0);assert.deepEqual(output(),entered);
for(let tick=0;tick<30;tick++)w.game_frame(1/60,0,0,0,1);
w.game_reset();w.game_frame(0,0,0,0,0);
assert.deepEqual(output(),entered,'restart returns to the carried entry state');
console.log('Passed: completed campaign advances once, carries progress, refills, and restores the level-entry baseline on restart.');
// A missing next-world resource must leave the completed current level intact.
const brokenNext=Buffer.from(fixture);
const waveBytes=readFileSync('.local/central-shape-resources/77617665/10000.bin');
const waveOffset=brokenNext.indexOf(waveBytes,base);
assert.ok(waveOffset>=base);
brokenNext.writeInt16BE(32767,waveOffset+2+16);
const brokenPtr=w.input_buffer(brokenNext.length);
new Uint8Array(w.memory.buffer,brokenPtr,brokenNext.length).set(brokenNext);
assert.equal(w.load_archive(),1);assert.equal(w.game_start_campaign_level(1,0),1);
for(let tick=0;tick<10;tick++)w.game_frame(1/60,0,0,0,0);
const beforeFailedAdvance=output();assert.equal(beforeFailedAdvance.encounter.status,'won');
assert.equal(w.game_next_campaign_level(),0);
w.game_frame(0,0,0,0,0);assert.deepEqual(output(),beforeFailedAdvance);
console.log('Passed: failed next-level loading preserves the completed arena and its awarded score.');

// Finish a synthetic level 15 to cross an actual upgrade boundary.
const upgradeFixture=Buffer.from(fixture);
const upgradeWaveOffset=upgradeFixture.indexOf(waveBytes,base);
assert.ok(upgradeWaveOffset>=base);
for(const field of [0,2,6])upgradeFixture.writeInt16BE(10000,upgradeWaveOffset+2+14*16+field);
for(let i=0;i<raw.readUInt16BE(0);i++){
  upgradeFixture.writeInt16BE(Math.round(completionSpawn.x),offset+2+i*8+2);
  upgradeFixture.writeInt16BE(Math.round(completionSpawn.z),offset+2+i*8+4);
}
const upgradePtr=w.input_buffer(upgradeFixture.length);
new Uint8Array(w.memory.buffer,upgradePtr,upgradeFixture.length).set(upgradeFixture);
assert.equal(w.load_archive(),1);assert.equal(w.game_start_campaign_level(15,0),1);
for(let tick=0;tick<60;tick++)w.game_frame(1/60,0,0,0,0);
assert.equal(output().encounter.status,'won');
assert.equal(w.game_next_campaign_level(),1);w.game_frame(0,0,0,0,0);
const upgraded=output();
assert.equal(upgraded.campaign.level,16);
assert.deepEqual(upgraded.vehicle_settings,[5,7,4,0,0,0,0]);
assert.equal(upgraded.encounter.health,7);assert.equal(upgraded.ammo,40);
w.game_reset();w.game_frame(0,0,0,0,0);assert.deepEqual(output(),upgraded);
assert.equal(w.game_next_campaign_level(),0);w.game_frame(0,0,0,0,0);assert.deepEqual(output(),upgraded);
console.log('Passed: entering level 16 grants exactly one shield upgrade, refills it, and retains it on restart.');
