import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
const {instance:{exports:w}}=await WebAssembly.instantiate(readFileSync('native-web/spectre_core.wasm'),{});
const text=()=>new TextDecoder().decode(new Uint8Array(w.memory.buffer,w.output_ptr(),w.output_len()));
const upload=path=>{const bytes=readFileSync(path);const ptr=w.input_buffer(bytes.length);new Uint8Array(w.memory.buffer,ptr,bytes.length).set(bytes);};
upload('.local/extracted/central.bin');assert.equal(w.load_shared_colors(),0,'campaign cannot stand in for executable bindings');
upload('.local/extracted/spectre.bin');assert.equal(w.load_shared_colors(),1,text());
const rgb=(bytes,index)=>[0,1,2].map(channel=>bytes.readUInt16BE(10+index*8+channel*2));
const levelTables=[2000,2001].map(id=>readFileSync(`.local/game-resources/636c7574/${id}.bin`));
let checked=0;
for(const campaign of ['central','eastern','south','central']) {
 upload(`.local/extracted/${campaign}.bin`);assert.equal(w.load_archive(),1,text());
 for(const level of [1,7,13,31]) {
  assert.equal(w.game_start_campaign_level(level,0),1,text());
  const frame=w.game_frame(0,0,0,0,0);const state=JSON.parse(text());
  const wave=readFileSync(`.local/${campaign}-shape-resources/77617665/${state.campaign.wave_resource}.bin`);
  const id=wave.readInt16BE(2+state.campaign.wave_index*16+14);
  let base;
  try {base=readFileSync(`.local/${campaign}-shape-resources/636c7574/${id}.bin`);} catch {base=readFileSync(`.local/game-resources/636c7574/${id}.bin`);}
  const expected=Array.from({length:13},(_,i)=>rgb(base,i));
  expected[2]=rgb(levelTables[1],(level-1)%30);expected[3]=rgb(levelTables[0],(level-1)%30);expected[12]=[0,0,0];
  assert.deepEqual(state.palette,expected,`${campaign} level ${level} uses authored base and shared level colors`);
  assert.equal(state.ground_color_index,0);
  assert.equal(state.radar_glyph_count,41,`${campaign} level ${level} loads executable radar atlas`);
  if(campaign==='central'&&level===1) {
   const pixels=new Uint8Array(w.memory.buffer,frame,800*600*4);const ground=expected[0].map(v=>v>>8);
   assert.ok(pixels.some((_,i)=>i%4===0&&ground.every((v,c)=>pixels[i+c]===v)),'recovered ground color appears in rendered frame');
   // The authored tower at this fixed entry viewpoint has shaded red faces.
   // Compact original framing maps these authored faces into the world surface.
   // Diagnostic blue scenery cannot pass.
   let redShadePixels=0;
   for(let y=310;y<400;y++)for(let x=208;x<263;x++) {
    const i=(y*800+x)*4;
    if(pixels[i]>32&&pixels[i]<255&&pixels[i+1]===0&&pixels[i+2]===0)redShadePixels++;
   }
   assert.ok(redShadePixels>100,'authored tower uses red shade ramps instead of diagnostic blue');
   let yellowPixels=0;
   for(let y=375;y<406;y++)for(let x=323;x<352;x++) {
    const i=(y*800+x)*4;
    if(pixels[i]>32&&pixels[i+1]>32&&pixels[i+2]===0)yellowPixels++;
   }
   assert.ok(yellowPixels>100,'procedural tetrahedron resolves kind-one level color');
   if(process.argv.includes('--snapshot'))execFileSync('ffmpeg',['-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-i','pipe:0','-frames:v','1','.local/ground-palette.png'],{input:Buffer.from(pixels),stdio:['pipe','ignore','pipe']});
  }
  w.game_reset();w.game_frame(0,0,0,0,0);assert.deepEqual(JSON.parse(text()).palette,expected,'reset preserves palette');assert.equal(JSON.parse(text()).radar_glyph_count,41,'reset preserves radar glyphs');
  const native=JSON.parse(execFileSync('target/debug/spectre-tools',['campaign-level',`.local/extracted/${campaign}.bin`,String(level),'0','--colors','.local/extracted/spectre.bin'],{encoding:'utf8'}));
  assert.deepEqual(native.palette,expected,'desktop and WASM palette match');
  assert.equal(native.radar_glyph_count,state.radar_glyph_count,'desktop and WASM radar atlas availability match');
  checked++;
 }
}
upload('.local/extracted/spectre.bin');assert.equal(w.load_shared_colors(),0,'late resource replacement cannot silently change an active game');
w.game_frame(0,0,0,0,0);assert.ok(JSON.parse(text()).palette);
console.log(`Passed: ${checked} native/WASM palette scenarios, campaign precedence, reload/reset, invalid shared input, and live ground color.`);
