import assert from 'node:assert/strict';
import {readRecoveredCases} from './read-recovered-cases.mjs';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
// Reference packed dimensions/radii come from execution of original vertex
// insertion and slot finalization, with explicit FixRound/FixDiv models.
const cases = readRecoveredCases('resource_vertices');
let checked=0;
for (const campaign of ['central','eastern','south']) {
  const run=spawnSync('cargo',['run','--quiet','--locked','-p','spectre-tools','--','model-slots',`.local/extracted/${campaign}.bin`],{encoding:'utf8'});
  assert.equal(run.status,0,run.stderr);
  const types=JSON.parse(run.stdout);
  const reference = id => cases.find(c=>c.routine==='resource_vertices' && c.source===`${campaign}/${id}`);
  const checkShape = (actual,id,half=false) => {
    const expected=reference(id); assert.ok(expected,`reference ${id}`);
    assert.deepEqual(actual.vertices,expected.packed);
    assert.deepEqual(actual.dimensions,expected.dimensions.map(n=>half?Math.trunc(n/2):n));
    assert.equal(actual.radius,half?Math.trunc(expected.radius/2):expected.radius);
    checked++;
  };
  for (const [kind,id] of [[3,10097],[6,10011],[7,10010],[15,10013],[16,10024],[17,10094],[18,10095],[19,10030],[22,10028],[27,10012]]) {
    assert.equal(types[kind].supported,true); assert.equal(types[kind].next,2);
    checkShape(types[kind].geometry[0],id,kind===16);
  }
  for (const [kind,resource] of [[30,3001],[31,3002]]) {
    const payload=readFileSync(`.local/${campaign}-shape-resources/61736870/${resource}.bin`);
    const ids=Array.from({length:payload.readInt16BE(0)},(_,i)=>payload.readInt16BE(2+i*2));
    assert.equal(types[kind].supported,true);
    assert.equal(types[kind].geometry.length,ids.length);assert.equal(types[kind].next,ids.length+1);
    types[kind].geometry.forEach((shape,i)=> {assert.equal(shape.slot,i+1);checkShape(shape,ids[i]);});
  }
  // Alias 23 uses dependency 43, then selected type 16 without the outer
  // type-16 halving path. These two loads must not be conflated.
  assert.equal(types[23].current[23],types[23].current[16]);
  checkShape(types[23].geometry[0],10029);
  checkShape(types[23].geometry[1],10024);
  assert.equal(types[20].current[34],1);checkShape(types[20].geometry[0],10001);
  checkShape(types[20].geometry[1],10027);
  for(const kind of [0,1,2,4]) assert.equal(types[kind].supported,true);
  assert.deepEqual(types[4].geometry[0].dimensions,[10*65536,10*65536]);
  assert.equal(types[4].geometry[0].radius,4);
  assert.equal(types[8].supported,false);
  assert.equal(types[8].next,1);assert.deepEqual(types[8].geometry,[]);
}
console.log(`Passed: ${checked} resource slot geometries match original-instruction fixtures; animation offsets, alias dependencies, size overrides, and unsupported-builder rollback checked across three campaigns.`);
