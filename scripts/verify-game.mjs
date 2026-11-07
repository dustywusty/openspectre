import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
const {instance} = await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'), {});
const w = instance.exports;
const output = () => JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len())));
let worlds = 0;
const collisionModes = {};
for (const campaign of ['central','eastern','south']) {
  const bytes = readFileSync(`.local/extracted/${campaign}.bin`);
  const input = w.input_buffer(bytes.length);
  new Uint8Array(w.memory.buffer,input,bytes.length).set(bytes);
  assert.equal(w.load_archive(),1);
  const entries = output();
  for (let i=0;i<entries.length;i++) {
    if (entries[i].kind !== 'wrld') continue;
    assert.equal(w.game_start(i),1,`spawn ${campaign} ${entries[i].id}`);
    const ptr = w.game_frame(1/60,1,0,0,1);
    const frame = new Uint8Array(w.memory.buffer,ptr,800*600*4);
    assert.ok(frame.some((v,k)=>k%4!==3 && v>200));
    assert.equal(output().tank.ticks,1); assert.equal(output().fired,0);
    // Audio events are a per-frame delivery queue, drained even on zero-time frames.
    // Take the camera-state baseline after that drain, without advancing simulation.
    w.game_frame(0,0,0,0,0);
    assert.deepEqual(output().audio_events,[]);
    const stateBeforeCamera = output();
    const firstPerson = Buffer.from(frame);
    w.game_camera(1);
    const chasePtr = w.game_frame(0,0,0,0,0);
    assert.deepEqual(output(),stateBeforeCamera,'camera switch preserves simulation');
    assert.notDeepEqual(Buffer.from(w.memory.buffer,chasePtr,800*600*4),firstPerson,'camera modes differ');
    const chase = Buffer.from(new Uint8Array(w.memory.buffer,chasePtr,800*600*4));
    w.game_camera(2);
    const mapPtr = w.game_frame(0,0,0,0,0);
    const map = Buffer.from(new Uint8Array(w.memory.buffer,mapPtr,800*600*4));
    assert.deepEqual(output(),stateBeforeCamera,'overhead preserves simulation');
    assert.notDeepEqual(map,firstPerson,'map differs from cockpit');
    assert.notDeepEqual(map,chase,'map differs from chase');
    for (const camera of [1,2]) {
      w.game_camera(camera);
      const normal = Buffer.from(new Uint8Array(w.memory.buffer,w.game_frame(0,0,0,0,0),800*600*4));
      for (const level of [-2,3]) {
        w.game_zoom(level);
        const ptr = w.game_frame(0,0,0,0,0);
        assert.notDeepEqual(Buffer.from(w.memory.buffer,ptr,800*600*4),normal,'zoom changes projection');
        assert.deepEqual(output(),stateBeforeCamera,'zoom preserves simulation');
      }
      w.game_zoom(0);
    }
    w.game_camera(0);
    const mode=output().collision_mode;
    collisionModes[mode]=(collisionModes[mode]||0)+1;
    for(let tick=0;tick<40;tick++) {
      w.game_frame(1/60,0,0,0,1);
      if(mode==='recovered query / preview records') assert.equal(output().collision_mode,mode,`query stays active ${campaign} ${entries[i].id}`);
      assert.equal(output().destroyed,output().targets.filter(t=>t.health===0).length,'no duplicate target destruction');
    }
    worlds++;
  }
  const index=entries.findIndex(e=>e.kind==='wrld' && e.id===10000);
  w.game_start(index);
  for(let i=0;i<240;i++) w.game_frame(1/60,1,.4,0,1);
  const browser=output();
  const native=spawnSync('cargo',['run','--quiet','--locked','-p','spectre-tools','--','replay',`.local/extracted/${campaign}.bin`],{encoding:'utf8'});
  assert.equal(native.status,0,native.stderr);
  const desktop=JSON.parse(native.stdout);
  // Trigonometric implementations can differ slightly across CPU/WASM targets.
  function close(a,b) {
    if(typeof a==='number') assert.ok(Math.abs(a-b)<0.001,`${a} != ${b}`);
    else if(a && typeof a==='object') { assert.deepEqual(Object.keys(a),Object.keys(b)); for(const k of Object.keys(a)) close(a[k],b[k]); }
    else assert.equal(a,b);
  }
  close(browser,desktop);
  w.game_reset(); w.game_frame(0,0,0,0,0);
  assert.equal(output().tank.ticks,0); assert.equal(output().fired,0);
  if(campaign==='central') {
    assert.equal(output().collision_mode,'recovered query / preview records');
    const initial=output().targets;
    assert.equal(initial.length,4);
    assert.deepEqual(initial.map(t=>t.health),[1,1,3,3]);
    for(let i=0;i<180;i++) w.game_frame(1/60,0,0,0,1);
    assert.equal(output().collision_mode,'recovered query / preview records','firing retains recovered query');
    assert.ok(output().destroyed>=1,'holding fire destroys the target ahead');
    assert.ok(output().targets.some(t=>t.health===0));
    w.game_reset(); w.game_frame(0,0,0,0,0);
    assert.equal(output().destroyed,0);
    assert.deepEqual(output().targets,initial,'reset restores target positions and health');
    assert.ok(initial.filter(t=>t.item.kind===31).every(t=>!t.vulnerable));
    assert.equal(w.game_replay_type31_event(),1);
    for(let i=0;i<60;i++) w.game_frame(1/60,0,0,0,0);
    assert.ok(output().targets.filter(t=>t.item.kind===31).every(t=>t.vulnerable && t.animation_frame===9));
    assert.equal(output().collision_mode,'recovered query / preview records','final animation slot resolves');
    w.game_reset(); w.game_frame(0,0,0,0,0);
    assert.deepEqual(output().targets,initial,'reset restores animation and immunity too');
    assert.equal(output().ammo,40);
    for(let i=0;i<600;i++) w.game_frame(1/60,0,0,0,1);
    assert.equal(output().fired,40,'finite ammo limits actual WASM shots');
    assert.equal(output().ammo,0);
    w.game_reset();
    for(let i=0;i<10;i++) w.game_frame(1/60,0,0,0,1);
    assert.equal(output().fired,0,'ten ticks are not enough to fire');
    w.game_frame(1/60,0,0,0,1);
    assert.equal(output().fired,1);
    assert.equal(output().ammo,39,'reset refills ammunition');
  }
}
console.log(`Passed: ${worlds} original arenas spawn/render; desktop/WASM 240-tick replays match within 0.001 units; firing destroys targets; reset restores them.`);

console.log('Collision paths:', collisionModes);
assert.ok(collisionModes['recovered query / preview records']>0);
