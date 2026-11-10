import { GameAudio } from '/audio.js';
import { GameHud } from '/hud.js';
const $ = selector => document.querySelector(selector);
const canvas = $('#view');
const ctx = canvas.getContext('2d', { alpha: false });
const status = $('#status');
let activePack;
let wasm, entries = [], selected = 0, yaw = 0.65, pitch = 0.3, zoom = 1;
let filled = true, spinning = !matchMedia('(prefers-reduced-motion: reduce)').matches, dragging = false, lastX = 0, lastY = 0;
let generation = 0;
const driveCanvas = $('#drive-view');
const driveContext = driveCanvas.getContext('2d', { alpha: false });
const driveHud = new GameHud(driveCanvas);
const sound = new GameAudio($('#sound-status'));
$('#sound-volume').oninput = e => { sound.volume=Number(e.target.value)/100; sound.applyVolume(); };
$('#sound-mute').onclick = e => { sound.muted=!sound.muted; sound.applyVolume(); $('#sound-status').textContent=sound.muted?'Sound muted':'Sound effects ready'; if(sound.muted)sound.stop(); e.currentTarget.textContent=sound.muted?'Unmute':'Mute'; e.currentTarget.setAttribute('aria-pressed',String(sound.muted)); if(!sound.muted)void sound.unlock(); };
const gamePanel = $('#game-panel');
const fullscreenButton = $('#fullscreen');
const fullscreenElement = () => document.fullscreenElement ?? document.webkitFullscreenElement;
async function toggleFullscreen() {
  try {
    if(fullscreenElement() === gamePanel) {
      await (document.exitFullscreen?.() ?? document.webkitExitFullscreen?.());
    } else {
      const enter = gamePanel.requestFullscreen ?? gamePanel.webkitRequestFullscreen;
      if(!enter) throw new Error('Fullscreen is unavailable');
      await enter.call(gamePanel);
    }
    driveCanvas.focus({preventScroll:true});
  } catch(error) {
    $('#drive-state').textContent = 'Fullscreen unavailable in this browser. Try opening the game in a separate browser window.';
    console.warn('Fullscreen request failed',error);
  }
}
fullscreenButton.onpointerdown = e => e.preventDefault();
fullscreenButton.onclick = toggleFullscreen;
for(const event of ['fullscreenchange','webkitfullscreenchange']) document.addEventListener(event,()=>{
  const active = fullscreenElement() === gamePanel;
  fullscreenButton.textContent = active ? 'Exit fullscreen' : 'Fullscreen';
  fullscreenButton.setAttribute('aria-pressed',String(active));
  driveCanvas.focus({preventScroll:true});
});
const keys = new Set();
const touchKeys = new Map();
let driving = false, gameReady = false;
function pauseDrive(stopAudio=true) {
  driveHud.setStatus('Paused');
  driving = false; keys.clear(); touchKeys.clear(); if(stopAudio)sound.stop();
  $('#play').textContent = 'Play';
  $('#drive-state').textContent = gameReady ? 'Paused' : 'Arena unavailable';
}
let shownResults = '';
function updateResults(state) {
  const result=state.completion;
  const key=JSON.stringify([result,state.par_seconds,state.mission.score,state.tank.ticks]);
  if(key===shownResults)return;
  shownResults=key;
  $('#mission-results').hidden=!result;
  $('#next-level').hidden=!(result&&state.campaign);
  if(!result)return;
  const names=['','Get Help','Marginal','Not Bad','Radical','Perfect'];
  for(let i=0;i<3;i++){
    const unrated=i>0&&result.unrated[i-1];
    $(`#rating-${i}`).textContent=unrated?'Unrated':names[result.ratings[i]];
    $(`#rating-bar-${i}`).value=result.bars[i];
    $(`#rating-bonus-${i}`).textContent=`${result.bonuses[i]} bonus points`;
  }
  $('#results-time').textContent=`${Math.trunc(state.tank.ticks/60)} seconds · Target ${state.par_seconds} seconds`;
  $('#results-total').textContent=`Completion bonus ${result.bonuses.reduce((a,b)=>a+b,0)} · Total score ${state.mission.score}`;
}
function gameDraw(dt = 0) {
  if (!gameReady) return;
  const down = code => keys.has(code) || [...touchKeys.values()].includes(code);
  const axis = (a,b,c,d) => Number(down(a)||down(b))-Number(down(c)||down(d));
  wasm.game_infinite_health(Number($('#infinite-health').checked));
  wasm.game_enemy_respawn(Number($('#enemy-respawn').checked));
  wasm.game_camera(Number($('#camera').value));
  const ptr = wasm.game_frame_weapons(dt, axis('KeyW','ArrowUp','KeyS','ArrowDown'), axis('KeyD','ArrowRight','KeyA','ArrowLeft'), Number(down('ShiftLeft')||down('ShiftRight')), Number(down('Space')), Number(down('KeyQ')), Number(down('KeyE')));
  driveContext.putImageData(new ImageData(new Uint8ClampedArray(wasm.memory.buffer,ptr,800*600*4),800,600),0,0);
  const state = JSON.parse(text());
  driveHud.updateSingle(state,{playing:driving});
  if(driving && dt>0) sound.play(state.audio_events ?? []);
  updateResults(state);
  const weaponNames={1:'Grenades',2:'Proximity mines',5:'Scattershot',9:'Seekers',10:'Smart missiles',11:'Spinners',13:'EM pulsebursts',17:'Cyberbombs'};
  const optional=state.optional_weapons;
  const hasOptional=optional?.available.some(value=>value&1);
  const playableOptional=[1,5,9,10].includes(optional?.selected);
  $('#fire-optional').disabled=!playableOptional;
  const weaponLabel=hasOptional ? `${weaponNames[optional.selected] ?? 'Cannon'}${playableOptional ? '' : ' · unavailable'}` : '';
  $('#optional-weapon').hidden=!hasOptional;
  if($('#optional-weapon').textContent!==weaponLabel)$('#optional-weapon').textContent=weaponLabel;
  const tank = state.tank;
  const encounter = state.encounter;
  driveCanvas.style.boxShadow = encounter.hit_flash ? 'inset 0 0 0 5px #f66, 0 0 20px #c33' : '';
  const endings = {won:'Arena cleared',lost:'Tank destroyed',no_enemies:'No enemies in this arena',no_spawn:'No safe respawn position'};
  if(encounter.enabled && encounter.status !== 'playing') {
    if(driving) pauseDrive(false);
    $('#drive-state').textContent = `${endings[encounter.status] ?? encounter.status}${encounter.status==='won'&&state.flags.enabled&&!state.completion?' · Ratings unavailable for this arena':''} · R or Restart round to retry`;
  }

  $('#type31-event').disabled = encounter.enabled || !state.targets.some(t => t.item.kind === 31);
  const protectedCount = state.targets.filter(t => t.health > 0 && !t.vulnerable).length;
  $('#telemetry').textContent = `${state.death_transition ? (state.flags.enabled && state.mission.lives>1 ? 'Respawning · ' : 'Tank destroyed · ') : ''}${encounter.enabled ? `Hull ${encounter.health}/${Math.min(state.vehicle_settings[1],9)} · ` : ''}${state.flags.enabled ? `Lives ${state.mission.lives} · Flags ${state.flags.collected}/${state.flags.total} · Score ${state.mission.score} · Deaths ${state.mission.deaths} · ` : ''}Speed ${tank.speed.toFixed(0)} · Flight ${state.flight_energy} · Altitude ${((state.vertical?.vertical ?? 0)/524288).toFixed(1)} · Surface ${tank.surface} · X ${tank.x.toFixed(0)} / Z ${tank.z.toFixed(0)} · Kills ${state.destroyed} · Retired ${state.targets.filter(t=>t.retired).length}/${state.targets.length}${state.enemy_clear_award ? ` · Enemy-clear bonus ${state.enemy_clear_award}` : ''} · Protected ${protectedCount} · Ammo ${state.ammo} · Shots ${state.fired} · ${state.collision_mode} · Tick ${tank.ticks}${tank.blocked ? ' · CONTACT' : ''}`;
}
function startArena() {
  pauseDrive();
  $('#mission-results').hidden=true; shownResults='';
  gameReady = false; $('#type31-event').disabled = true;
  const campaignMode=Number($('#mode').value)===3;
  $('#campaign-controls').hidden=!campaignMode;
  $('#arena').disabled=campaignMode||!$('#arena').options.length;
  const level=Number($('#campaign-level').value);
  if(campaignMode&&(!Number.isInteger(level)||level<1||level>32767)){
    $('#drive-state').textContent='Enter a level from 1 to 32767';
    $('#play').disabled=true;$('#restart').disabled=true;return;
  }
  const loaded=$('#arena').options.length&&(campaignMode
    ?wasm.game_start_campaign_level(level,Number($('#campaign-difficulty').value))
    :(Number($('#mode').value)===2?wasm.game_start_mission:wasm.game_start)(Number($('#arena').value)));
  if (!loaded) {
    $('#drive-state').textContent = $('#arena').options.length ? text() : 'No world layouts in this archive';
    $('#play').disabled = true; $('#restart').disabled = true; return;
  }
  const scene = JSON.parse(text());
  $('#scene-info').textContent = campaignMode ? `${scene.scenario} · ${scene.targets.length} targets · Fresh round` : `${scene.resolved} model placements · ${scene.targets.length} targets · ${activePack?.id==='open'?'Signal Frontier':scene.scenario}`;
  wasm.game_encounter(Number($('#mode').value));
  gameReady = true; $('#play').disabled = false; $('#restart').disabled = false;
  $('#drive-state').textContent = 'Ready';
  gameDraw();
}
$('#arena').onchange = startArena;
$('#mode').onchange = startArena;
$('#campaign-level').onchange = startArena;
$('#campaign-difficulty').onchange = startArena;
$('#play').onclick = () => {
  if (!gameReady) return;
  if (driving) { pauseDrive(); return; }
  void sound.unlock();
  driving = true; last = performance.now();
  $('#play').textContent = 'Pause';
  $('#drive-state').textContent = 'Playing';
  driveCanvas.focus({ preventScroll: true });
};
$('#play').onpointerdown = e => e.preventDefault();
$('#restart').onpointerdown = e => e.preventDefault();
$('#type31-event').onpointerdown = e => e.preventDefault();
$('#type31-event').onclick = () => {
  if (gameReady && wasm.game_replay_type31_event()) {
    gameDraw();
    $('#drive-state').textContent = 'Unlock event queued · animation advances while driving';
  }
};
function restartRound() { if(gameReady) { pauseDrive(); wasm.game_reset(); gameDraw(); $('#drive-state').textContent='Ready'; } }
$('#restart').onclick = restartRound;
$('#next-level').onclick = () => {
  if(!gameReady)return;
  pauseDrive();
  if(!wasm.game_next_campaign_level()){
    $('#drive-state').textContent=text();return;
  }
  const scene=JSON.parse(text());
  $('#campaign-level').value=String(scene.campaign.level);
  $('#scene-info').textContent=`${scene.scenario} · ${scene.targets.length} targets · Continuing campaign`;
  shownResults='';gameDraw();
  void sound.unlock();driving=true;last=performance.now();
  $('#play').textContent='Pause';$('#drive-state').textContent=`Level ${scene.campaign.level} · Driving`;
  driveCanvas.focus({preventScroll:true});
};

let cameraZoom = 0;
function setCamera(mode) {
  $('#camera').value = String(mode);
  $('#zoom-in').disabled = $('#zoom-out').disabled = mode === 0;
  gameDraw();
}
function zoomCamera(delta) {
  if (!gameReady || Number($('#camera').value) === 0) return;
  cameraZoom = Math.max(-2, Math.min(3, cameraZoom + delta));
  wasm.game_zoom(cameraZoom);
  gameDraw();
}
$('#zoom-in').onclick = () => zoomCamera(1);
$('#zoom-out').onclick = () => zoomCamera(-1);
$('#zoom-in').disabled = $('#zoom-out').disabled = true;
const driveKeys = new Set(['KeyW','KeyA','KeyS','KeyD','ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space','ShiftLeft','ShiftRight','KeyQ','KeyE']);
driveCanvas.onkeydown = e => {
  if (driveKeys.has(e.code)) { e.preventDefault(); if (driving) keys.add(e.code); }
  if ((e.code === 'Tab' && !e.shiftKey) || e.code === 'NumpadMultiply') {
    e.preventDefault();
    if (!e.repeat) setCamera((Number($('#camera').value) + 1) % 3);
  }
  if (['Equal', 'NumpadAdd', 'Minus', 'NumpadSubtract'].includes(e.code)) {
    e.preventDefault();
    zoomCamera(['Equal', 'NumpadAdd'].includes(e.code) ? 1 : -1);
  }
  if (e.code === 'KeyF' && !e.repeat) { e.preventDefault(); void toggleFullscreen(); }
  if (e.code === 'Escape') pauseDrive();
  if (e.code === 'KeyR' && gameReady) { restartRound(); }
};
window.addEventListener('keyup', e => keys.delete(e.code));
driveCanvas.addEventListener('blur', () => { if (driving) pauseDrive(); });
window.addEventListener('blur', pauseDrive);
document.querySelectorAll('[data-control]').forEach(button => {
  button.onpointerdown = e => {
    e.preventDefault();
    if (!driving || !gameReady) return;
    button.setPointerCapture(e.pointerId);
    touchKeys.set(e.pointerId, button.dataset.control);
  };
  button.onpointerup = button.onpointercancel = button.onlostpointercapture = e => touchKeys.delete(e.pointerId);
});

const text = () => new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, wasm.output_ptr(), wasm.output_len()));

function draw() {
  if (!wasm || !entries.length) return;
  const ptr = wasm.render(selected, yaw, pitch, zoom, Number(filled));
  ctx.putImageData(new ImageData(new Uint8ClampedArray(wasm.memory.buffer, ptr, 800 * 600 * 4), 800, 600), 0, 0);
}
function describe() {
  const e = entries[selected];
  $('#name').textContent = e.name || `World ${e.id}`;
  $('#details').textContent = e.kind === 'shap'
    ? `shap ${e.id} · ${e.vertices} vertices · ${e.faces} polygon faces · overhead geometry at lower right`
    : `wrld ${e.id} · ${e.objects} placements · top-down map · markers show raw positions and facing; object meanings are not yet resolved`;
  document.querySelectorAll('.model-card').forEach(card => card.setAttribute('aria-pressed', String(Number(card.dataset.index) === selected)));
  draw();
}
function gallery() {
  const models = entries.map((entry, index) => ({ ...entry, index })).filter(e => e.kind === 'shap' && e.vertices > 5);
  const chosen = [];
  for (const pattern of [/tank\s*1/i, /tank\s*2/i, /tank\s*3/i, /orb/i, /cube/i]) {
    const model = models.find(e => pattern.test(e.name) && !chosen.includes(e));
    if (model) chosen.push(model);
  }
  for (const model of models) { if (chosen.length >= 5) break; if (!chosen.includes(model)) chosen.push(model); }
  const scratch = document.createElement('canvas'); scratch.width = 800; scratch.height = 600;
  const context = scratch.getContext('2d');
  $('#gallery').replaceChildren(...chosen.map(e => {
    const ptr = wasm.render(e.index, .65, .3, 1, 1);
    context.putImageData(new ImageData(new Uint8ClampedArray(wasm.memory.buffer, ptr, 800 * 600 * 4), 800, 600), 0, 0);
    const button = document.createElement('button'); button.className = 'model-card'; button.dataset.index = e.index;
    const img = document.createElement('img'); img.src = scratch.toDataURL(); img.alt = ''; img.width = 800; img.height = 600;
    const label = document.createElement('span'); label.textContent = e.name || `Model ${e.id}`;
    button.append(img, label);
    button.onclick = () => { selected = e.index; $('#search').value = ''; list(); describe(); };
    return button;
  }));
}
function list() {
  const term = $('#search').value.toLowerCase();
  const options = entries.flatMap((e, i) => {
    if (!`${e.name} ${e.kind} ${e.id}`.toLowerCase().includes(term)) return [];
    const option = new Option(`${e.kind === 'shap' ? '◇' : '▦'} ${e.name || 'World'} · ${e.id}`, String(i));
    option.selected = i === selected;
    return [option];
  });
  $('#assets').replaceChildren(...options);
}
async function loadBytes(bytes, label) {
  if (bytes.byteLength > 32 * 1024 * 1024) throw new Error('Open an extracted matrix file smaller than 32 MiB.');
  const ptr = wasm.input_buffer(bytes.byteLength);
  if (!ptr) throw new Error('Unable to allocate the resource buffer.');
  new Uint8Array(wasm.memory.buffer, ptr, bytes.byteLength).set(new Uint8Array(bytes));
  if (!wasm.load_archive()) throw new Error(text());
  entries = JSON.parse(text());
  $('#arena').replaceChildren(...entries.flatMap((e,i) => e.kind === 'wrld' ? [new Option(e.name || `World ${e.id}`, String(i))] : []));
  const firstWorld = entries.findIndex(e => e.kind === 'wrld' && e.id === 10000);
  if (firstWorld >= 0) $('#arena').value = String(firstWorld);
  startArena();
  selected = Math.max(0, entries.findIndex(e => e.kind === 'shap' && e.id === 10010));
  $('#search').value = '';
  gallery(); list(); describe();
  status.textContent = `${label} · ${entries.filter(e => e.kind === 'shap').length} models + ${entries.filter(e => e.kind === 'wrld').length} maps decoded in Rust`;
}
async function loadCampaign() {
  pauseDrive();
  const ownGeneration = ++generation;
  status.textContent = 'Loading arena pack…';
  try {
    const response = await fetch(`/data/${$('#campaign').value}.bin`);
    if (!response.ok) throw new Error('Arena pack missing. See the setup instructions for the selected asset mode.');
    const bytes = await response.arrayBuffer();
    if (generation === ownGeneration) await loadBytes(bytes, $('#campaign').selectedOptions[0].text);
  } catch (e) { if (generation === ownGeneration) status.textContent = e.message; }
}
$('#campaign').onchange = loadCampaign;
$('#search').oninput = list;
$('#assets').onchange = () => { selected = Number($('#assets').value); describe(); };
$('#open').onclick = () => $('#file').click();
$('#file').onchange = async () => {
  const file = $('#file').files[0]; if (!file) return;
  const token = ++generation;
  try { const data = await file.arrayBuffer(); if (token === generation) await loadBytes(data, file.name); }
  catch (e) { status.textContent = e.message; }
  $('#file').value = '';
};
$('#spin').onclick = () => { spinning = !spinning; $('#spin').textContent = spinning ? 'Stop rotation' : 'Auto-rotate'; };
$('#fill').onclick = () => { filled = !filled; $('#fill').textContent = filled ? 'Wireframe' : 'Filled preview'; draw(); };
$('#reset').onclick = () => { yaw = .65; pitch = .3; zoom = 1; draw(); };
canvas.onpointerdown = e => { canvas.focus(); canvas.setPointerCapture(e.pointerId); dragging = true; lastX = e.clientX; lastY = e.clientY; };
canvas.onpointermove = e => {
  if (!dragging) return;
  yaw += (e.clientX - lastX) * .008;
  pitch = Math.max(-1.5, Math.min(1.5, pitch + (e.clientY - lastY) * .008));
  lastX = e.clientX; lastY = e.clientY; draw();
};
canvas.onpointerup = canvas.onpointercancel = canvas.onlostpointercapture = () => { dragging = false; };
canvas.addEventListener('wheel', e => { e.preventDefault(); zoom = Math.max(.3, Math.min(2, zoom * Math.exp(-e.deltaY * .001))); draw(); }, { passive: false });
canvas.onkeydown = e => {
  if (!entries.length) return;
  if (e.key === 'ArrowLeft') yaw -= .1;
  else if (e.key === 'ArrowRight') yaw += .1;
  else if (e.key === 'ArrowUp') pitch = Math.min(1.5, pitch + .1);
  else if (e.key === 'ArrowDown') pitch = Math.max(-1.5, pitch - .1);
  else if (e.key === '[' || e.key === ']') { selected = (selected + entries.length + (e.key === '[' ? -1 : 1)) % entries.length; list(); describe(); }
  else return;
  e.preventDefault(); draw();
};
let multiplayer, multiplayerLoading, multiplayerActive=false;
let last = 0;
async function selectPlayMode(multi) {
  pauseDrive();
  multiplayerActive=multi;
  gamePanel.hidden=multi;
  $('#multiplayer-panel').hidden=!multi;
  $('#single-player-toggle').setAttribute('aria-pressed',String(!multi));
  $('#multiplayer-toggle').setAttribute('aria-pressed',String(multi));
  if(!multi){if(multiplayer)await multiplayer.enable(false);return;}
  try {
    multiplayerLoading??=import('/multiplayer.js').then(({Multiplayer})=>new Multiplayer());
    multiplayer=await multiplayerLoading;
    await multiplayer.enable(multiplayerActive);
  } catch(error) {
    multiplayerLoading=null;
    $('#mp-status').textContent=`Multiplayer could not load: ${error.message}. Single player is still available.`;
  }
}
$('#single-player-toggle').onclick=()=>void selectPlayMode(false);
$('#multiplayer-toggle').onclick=()=>void selectPlayMode(true);
if(new URL(location.href).searchParams.has('room')||new URL(location.href).searchParams.get('play')==='multiplayer')void selectPlayMode(true);
function tick(time) {
  if (driving && !document.hidden) gameDraw(Math.max(0, (time - last) / 1000));
  if ($('#workbench').open && !driving && !multiplayerActive && spinning && !document.hidden) { yaw += Math.min((time - last) / 1000, .1) * .4; draw(); }
  last = time; requestAnimationFrame(tick);
}
document.addEventListener('visibilitychange', () => { if (document.hidden) pauseDrive(); else { requestAnimationFrame(draw); gameDraw(); } });
window.addEventListener('resize', () => requestAnimationFrame(draw));
try {
  const packResponse = await fetch('/pack.json');
  if(!packResponse.ok)throw new Error('Asset pack information unavailable. Restart the server.');
  activePack=await packResponse.json();
  $('#pack-status').textContent=activePack.name;
  $('#campaign').replaceChildren(...activePack.campaigns.map(c=>new Option(c.name,c.id)));
  if(activePack.id==='open'){
    $('#camera').value='0';
    $('#camera').options[1].textContent='Chase';
  }
  const response = await fetch('/spectre_core.wasm');
  if (!response.ok) throw new Error('WASM module missing. Run npm run build:wasm.');
  const { instance } = await WebAssembly.instantiateStreaming(response, {});
  wasm = instance.exports;
  const sharedResponse = await fetch('/data/spectre.bin');
  if (!sharedResponse.ok) throw new Error(activePack.id==='classic'?'Classic executable resources missing. Extract your disc or use npm start for the bundled pack.':'Bundled resources missing. Run npm run build.');
  const sharedBytes = await sharedResponse.arrayBuffer();
  const sharedPtr = wasm.input_buffer(sharedBytes.byteLength);
  if (!sharedPtr) throw new Error('Unable to allocate the shared resource buffer.');
  new Uint8Array(wasm.memory.buffer, sharedPtr, sharedBytes.byteLength).set(new Uint8Array(sharedBytes));
  if (!wasm.load_shared_colors()) throw new Error(text());
  document.querySelectorAll('[disabled]').forEach(e => e.disabled = false);
  $('#spin').textContent = spinning ? 'Stop rotation' : 'Auto-rotate';
  await loadCampaign();
  requestAnimationFrame(draw);
  requestAnimationFrame(tick);
} catch (e) { $('#drive-state').textContent = e.message; $('#name').textContent = 'Unable to load assets'; status.textContent = e.message; }

$('#camera').onchange = () => setCamera(Number($('#camera').value));

// Debug aid is opt-in for each page load; never persisted with browser form state.
$('#infinite-health').checked=false;
$('#infinite-health').onchange=()=>gameDraw();
// Gameplay preference, on by default and remembered across visits.
$('#enemy-respawn').checked=localStorage.getItem('spectre.enemyRespawn')!=='0';
$('#enemy-respawn').onchange=()=>{localStorage.setItem('spectre.enemyRespawn',$('#enemy-respawn').checked?'1':'0');gameDraw();};
