import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const {instance:{exports:w}} = await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'), {});
const output = () => JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
const verified = new Set();
const evidence = [];
for (const campaign of ['central','eastern','south']) {
  const bytes = readFileSync(`.local/extracted/${campaign}.bin`);
  const input=w.input_buffer(bytes.length);
  new Uint8Array(w.memory.buffer,input,bytes.length).set(bytes);
  assert.equal(w.load_archive(),1);
  const wave = readFileSync(`.local/${campaign}-shape-resources/77617665/10002.bin`);
  for (let level=1;level<=wave.readUInt16BE(0)&&verified.size<1;level++) {
    assert.equal(w.game_start_campaign_level(level,5),1);
    w.game_frame(0,0,0,0,0);
    const initial=output();
    const candidates=initial.targets.map((t,i)=>({kind:t.item.kind,index:i}))
      .filter(t=>t.kind===8&&!verified.has(t.kind));
    if (!candidates.length) continue;
    const moved=new Set();
    for(let tick=0;tick<240;tick++) {
      w.game_frame(1/60,0,0,0,0);
      const state=output();
      for(const {kind,index} of candidates) {
        const target=state.targets[index];
        if(target.health<=0||target.retired)continue;
        if(target.item.x!==initial.targets[index].item.x||target.item.z!==initial.targets[index].item.z)moved.add(kind);
      }
    }
    for(const {kind} of candidates)if(moved.has(kind)&&!verified.has(kind)){
      verified.add(kind);evidence.push({campaign,level,kind});
    }
    w.game_reset();w.game_frame(0,0,0,0,0);
    assert.deepEqual(output(),initial,'predictive-enemy level reset restores initial state');
  }
}
assert.deepEqual([...verified].sort(),[8],'original kind 8 must move in an authored level');
console.log('Passed: authored WASM type-8 enemies move and reset.',evidence);
