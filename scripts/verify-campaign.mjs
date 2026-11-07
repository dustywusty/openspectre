import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
let checked=0;
for(const campaign of ['central','eastern','south']){
  const path=`.local/extracted/${campaign}.bin`;
  const bytes=readFileSync(path),ptr=w.input_buffer(bytes.length);
  new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);
  assert.equal(w.load_archive(),1);
  for(const [id,difficulty] of [[10000,0],[10001,3],[10002,5]]){
    const wave=readFileSync(`.local/${campaign}-shape-resources/77617665/${id}.bin`);
    const count=wave.readUInt16BE(0);
    for(let level=1;level<=count+1;level++){
      assert.equal(w.game_start_campaign_level(level,difficulty),1,`${campaign}/${id} level ${level}`);
      const scene=output(),index=(level-1)%count,offset=2+16*index;
      assert.deepEqual(scene.campaign,{level,difficulty,wave_resource:id,wave_index:index,
        world:wave.readInt16BE(offset),health_modifier:Math.floor((level-1)/count)});
      assert.ok(scene.scenario.includes(`enem ${wave.readInt16BE(offset+2)} · bval ${wave.readInt16BE(offset+6)}`));
      const available=Array(18).fill(0);let selected=0;
      const grants={10:[1],11:[9],12:[10],13:[2],14:[11],15:[5],16:[10,2,9,5,1,17,13,11],17:[17],18:[13]};
      for(let row=0;row<Math.min(level,count);row++){
        for(const slot of grants[wave.readInt16BE(2+16*row+4)]??[]){
          if(!available[slot])selected=slot;
          available[slot]=1;
        }
      }
      assert.deepEqual(scene.optional_weapons,{selected,available},'loadout accumulates original hints through the level, capped at the resource end');
      if(campaign==='central'&&id===10000&&level<=3){
        assert.equal(scene.optional_weapons.selected,[13,17,1][level-1],'opening original unlock sequence');
      }

      const bval=readFileSync(`.local/${campaign}-shape-resources/6276616c/${wave.readInt16BE(offset+6)}.bin`);
      let weights=[5,2,1,2];
      for(let row=0;row<bval.readUInt16BE(0);row++){
        const start=2+28*row;
        if(bval.readInt8(start+1)===5) weights=[3,5,7,9].map(n=>bval.readInt8(start+n));
      }
      assert.deepEqual(scene.dump_weights,weights,'pickup weights come from the selected original bval row');
      assert.ok(scene.pickups.length<=6,'creation requests at most six dumps');
      for(const pickup of scene.pickups){
        assert.equal(pickup.item.kind,5);
        assert.ok(pickup.subtype>=0&&pickup.subtype<=3);
        assert.ok(Number.isFinite(pickup.item.x)&&Number.isFinite(pickup.item.z));
      }
      for(let first=0;first<scene.pickups.length;first++) for(let second=0;second<first;second++){
        const a=scene.pickups[first].item,b=scene.pickups[second].item;
        assert.ok(Math.abs(a.x-b.x)+Math.abs(a.z-b.z)>=40,'initial dumps respect original proximity rejection');
      }
      assert.equal(scene.flags.enabled,true);
      assert.ok(scene.flags.total>0);
      assert.ok(scene.targets.length>0&&scene.targets.length<=11);
      assert.ok(scene.par_seconds>0);
      w.game_frame(0,0,0,0,0);const initial=output();
      w.game_frame(1/60,1,0,0,1);w.game_reset();w.game_frame(0,0,0,0,0);
      assert.deepEqual(output(),initial,'reset preserves the selected level and restores its starting state');
      if(level===1||level===count+1){
        const native=spawnSync('cargo',['run','--quiet','--locked','-p','spectre-tools','--','campaign-level',path,String(level),String(difficulty)],{encoding:'utf8'});
        assert.equal(native.status,0,native.stderr);
        assert.deepEqual(JSON.parse(native.stdout),scene,'native/WASM level setup agrees');
      }
      checked++;
    }
  }
  w.game_frame(0,0,0,0,0);const prior=output();
  for(const [level,difficulty] of [[0,0],[-1,0],[32768,0],[1,-1],[1,8]]){
    assert.equal(w.game_start_campaign_level(level,difficulty),0);
    w.game_frame(0,0,0,0,0);assert.deepEqual(output(),prior,'failed level load preserves the active game');
  }
}
console.log(`Passed: ${checked} campaign levels/boundaries load and reset; 18 native/WASM setups match; invalid inputs preserve active games.`);
