import assert from 'node:assert/strict';
import {readRecoveredCases} from './read-recovered-cases.mjs';
import {readFileSync} from 'node:fs';
const oracle=JSON.parse(readFileSync('.local/disassembly/shape-selection.json'));
const poses=new Map(readRecoveredCases('world_placement').map(c=>[c.source,c.pose]));
const animations=JSON.parse(readFileSync('.local/disassembly/animation-selection.json'));
const {instance}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const w=instance.exports;
const output=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
let maps=0,placements=0,resolved=0,unresolved=0;
for(const campaign of ['central','eastern','south']) {
  const data=readFileSync(`.local/extracted/${campaign}.bin`);
  const ptr=w.input_buffer(data.length);
  new Uint8Array(w.memory.buffer,ptr,data.length).set(data);
  assert.equal(w.load_archive(),1);
  const entries=output();
  const report=JSON.parse(readFileSync(`.local/${campaign}-shape-resources/inventory.json`));
  for(let i=0;i<entries.length;i++) {
    if(entries[i].kind!=='wrld') continue;
    const world=report.decoded.find(r=>r.kind==='wrld'&&r.id===entries[i].id).value.obstacles;
    const expected=world.map((o,index)=>({...o,index})).filter(o=>oracle[campaign][o.kind]?.[2]!==0);
    assert.equal(w.game_start(i),1);
    const scene=output();
    for(const item of [...scene.items,...scene.targets.map(t=>t.item)]) {
      const frames=animations[campaign][item.shape_id];
      if(!frames?.length) continue;
      const first=report.decoded.find(r=>r.kind==='shap'&&r.id===frames[0]);
      if(!first) continue;
      assert.equal(item.source,'original animation, frozen first frame');
      for(let k=0;k<3;k++) {
        const values=first.value.vertices.map(v=>v[['x','y','z'][k]]);
        assert.ok(Math.abs(item.min[k]-Math.min(...values))<0.001);
        assert.ok(Math.abs(item.max[k]-Math.max(...values))<0.001);
      }
    }
    assert.equal(scene.items.length,expected.length);
    scene.items.forEach((item,j)=>{
      const o=expected[j];
      assert.equal(item.shape_id,oracle[campaign][o.kind]?.[2]??-1);
      const pose=poses.get(`${campaign}/${entries[i].id}/${o.index}`);
      assert.ok(pose,'original placement instruction fixture exists');
      assert.equal(item.x,pose[0]/65536);assert.equal(item.z,pose[1]/65536);
      // Verify stored degree conversion. Render-axis orientation is a separate question.
      assert.ok(Math.abs(item.heading-pose[2]/65536*Math.PI/180)<0.0001);
      for(let k=0;k<3;k++) assert.ok(Number.isFinite(item.min[k])&&item.min[k]<=item.max[k]);
    });
    if(campaign==='central'&&entries[i].id===10000) {
      assert.equal(scene.resolved,20);assert.equal(scene.unresolved,0);
      assert.equal(world.length-scene.items.length,6);
    }
    placements+=scene.items.length;resolved+=scene.resolved;unresolved+=scene.unresolved;maps++;
  }
}
console.log(`Passed: ${maps} maps, ${placements} placements match original instruction selections and coordinate conversion. ${resolved} resolved, ${unresolved} explicitly unresolved.`);
