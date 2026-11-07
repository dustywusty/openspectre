// The footage shows green counters and outlined bars across the upper left.
// This is a native-port HUD: values are live simulation state, not recovered UI code.
const weaponNames = {1:'Grenades',2:'Mines',5:'Scattershot',9:'Seekers',10:'Smart missiles',11:'Spinners',13:'EM pulse',17:'Cyberbombs'};
const number = value => Number.isFinite(value) ? value : 0;
const count = value => String(Math.max(0, Math.trunc(number(value))));
const clock = ticks => { const seconds = Math.max(0, Math.floor(number(ticks) / 60)); return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2,'0')}`; };
export function singleHudState(state, {playing = true} = {}) {
  const mission = state.mission ?? {}, encounter = state.encounter ?? {}, flags = state.flags ?? {};
  const maxHealth = Math.max(1, Math.min(number(state.vehicle_settings?.[1]) || 6, 9));
  let status = playing ? '' : 'Paused · Drive tank to continue';
  let tone = 'normal';
  if (state.death_transition) {status = flags.enabled && mission.lives > 1 ? 'Tank destroyed · respawning' : 'Tank destroyed'; tone = 'danger';}
  if (state.completion || encounter.status === 'won') {status = 'Arena cleared'; tone = 'success';}
  if (['lost','no_spawn'].includes(encounter.status)) {status = encounter.status === 'no_spawn' ? 'No safe respawn position' : 'Tank destroyed · restart round'; tone = 'danger';}
  return {
    health: encounter.enabled ? number(encounter.health) : null, maxHealth,
    ammo: count(state.ammo), scoreLabel: 'Score', score: count(mission.score), secondaryLabel: flags.enabled ? 'Lives' : 'Kills',
    secondary: count(flags.enabled ? mission.lives : state.destroyed),
    objectiveLabel: flags.enabled ? 'Flags' : 'Deaths', objective: flags.enabled ? `${count(flags.collected)}/${count(flags.total)}` : count(mission.deaths),
    mode: state.campaign ? `Level ${count(state.campaign.level)}` : 'Single player',
    time: clock(state.tank?.ticks), weapon: weaponNames[state.optional_weapons?.selected] ?? 'Cannon',
    weaponHint: weaponNames[state.optional_weapons?.selected] ? ([1,5,9,10].includes(state.optional_weapons.selected) ? 'SPACE cannon · E optional' : 'Optional weapon unavailable') : 'SPACE fire',
    status, tone, hit: Boolean(encounter.hit_flash),
  };
}
export function multiplayerHudState(state, {slot = 0, connected = [true,true]} = {}) {
  const me = state.players?.[slot] ?? {}, other = state.players?.[1-slot] ?? {};
  let status = '', tone = 'normal';
  if (!connected.every(Boolean)) status = 'Waiting for opponent · share invite link';
  else if (state.winner !== null && state.winner !== undefined) {status = state.winner === slot ? 'You win · first to 5' : 'Opponent wins · first to 5'; tone = state.winner === slot ? 'success' : 'danger';}
  else if (me.respawn > 0) {status = `Respawning in ${(me.respawn/60).toFixed(1)}s`; tone = 'danger';}
  else if (me.shield > 0) {status = `Spawn shield ${(me.shield/60).toFixed(1)}s · firing ends protection`; tone = 'shield';}
  return {
    health: number(me.health), maxHealth: 3, ammo: '∞', scoreLabel: 'You', score: count(me.kills),
    secondaryLabel:'Opponent', secondary: count(other.kills), objectiveLabel:'Deaths', objective: count(me.deaths),
    mode:'Deathmatch · first to 5', time:clock(state.tick), weapon:'Cannon', weaponHint:'SPACE fire · unlimited ammo',
    status, tone, hit:false,
  };
}
export class GameHud {
  constructor(canvas) {
    this.canvas = canvas;
    this.viewport = document.createElement('div');
    this.viewport.className = 'game-viewport';
    canvas.before(this.viewport); this.viewport.append(canvas);
    this.element = document.createElement('div');
    this.element.className = 'game-overlay';
    this.element.setAttribute('aria-label','Game instruments');
    this.element.innerHTML = `<div class="game-instruments">
      <div class="game-instrument-column">
        <div class="game-readout"><span>Hull</span><strong data-hud="hull">—</strong></div>
        <div class="game-hull-track" role="meter" aria-label="Hull remaining" aria-valuemin="0"><i></i></div>
        <div class="game-readout"><span>Ammo</span><strong data-hud="ammo">—</strong></div>
        <div class="game-readout"><span data-hud="secondaryLabel">Lives</span><strong data-hud="secondary">—</strong></div>
      </div>
      <div class="game-instrument-column">
        <div class="game-readout"><span data-hud="scoreLabel">Score</span><strong data-hud="score">0</strong></div>
        <div class="game-readout"><span data-hud="objectiveLabel">Flags</span><strong data-hud="objective">—</strong></div>
        <div class="game-readout"><span>Elapsed</span><strong data-hud="time">0:00</strong></div>
      </div>
      <div class="game-weapon"><span data-hud="mode">Single player</span><strong data-hud="weapon">Cannon</strong><small data-hud="weaponHint">SPACE fire</small></div>
      </div><div class="game-notice" aria-hidden="true" hidden></div><span class="game-hud-announcement" role="status" aria-live="polite"></span>`;
    this.viewport.append(this.element);
    this.exitButton = document.createElement('button');
    this.exitButton.type = 'button';
    this.exitButton.className = 'game-fullscreen-exit';
    this.exitButton.textContent = 'Exit fullscreen';
    this.exitButton.onpointerdown = e => e.preventDefault();
    this.exitButton.onclick = async () => {
      const exit = document.exitFullscreen ?? document.webkitExitFullscreen;
      if (exit) await exit.call(document);
      this.canvas.focus({preventScroll:true});
    };
    this.viewport.append(this.exitButton);
    if (canvas.id === 'drive-view') {
      const play = document.createElement('button');
      play.type = 'button'; play.className = 'game-fullscreen-play';
      play.textContent = 'Drive / pause';
      play.onpointerdown = e => e.preventDefault();
      play.onclick = () => { document.querySelector('#play').click(); canvas.focus({preventScroll:true}); };
      this.viewport.append(play);
    }
    this.fields = Object.fromEntries([...this.element.querySelectorAll('[data-hud]')].map(el=>[el.dataset.hud, el]));
    this.meter = this.element.querySelector('[role=meter]'); this.fill = this.meter.firstElementChild;
    this.notice = this.element.querySelector('.game-notice');
    this.announcement = this.element.querySelector('.game-hud-announcement');
    this.observer = new ResizeObserver(()=>this.resize()); this.observer.observe(canvas);
    this.clear();
  }
  resize() {
    // Fullscreen fills the display; the normal embedded view retains its aspect ratio.
    const width = this.canvas.clientWidth, height = this.canvas.clientHeight;
    const scale = Math.min(width / this.canvas.width, height / this.canvas.height);
    const fill = getComputedStyle(this.canvas).objectFit === 'fill';
    this.element.style.width = `${fill ? width : this.canvas.width * scale}px`;
    this.element.style.height = `${fill ? height : this.canvas.height * scale}px`;
    this.element.style.fontSize = `${Math.max(11,Math.min(16,14*scale))}px`;
  }
  updateSingle(state, options) { this.update(singleHudState(state, options)); }
  updateMultiplayer(state, options) { this.update(multiplayerHudState(state, options)); }
  update(model) {
    this.element.hidden = false;
    for (const [key, el] of Object.entries(this.fields)) {
      const text = key === 'hull' ? (model.health === null ? '—' : `${count(model.health)}/${model.maxHealth}`) : String(model[key] ?? '');
      if (el.textContent !== text) el.textContent = text;
    }
    const fraction = model.health === null ? 0 : Math.max(0,Math.min(1,model.health/model.maxHealth));
    this.fill.style.width = `${fraction*100}%`;
    this.meter.setAttribute('aria-valuemax',String(model.maxHealth));
    this.meter.setAttribute('aria-valuenow',String(model.health ?? 0));
    this.meter.setAttribute('aria-valuetext',model.health === null ? 'Hull is not used in research mode' : `${model.health} of ${model.maxHealth}`);
    this.element.dataset.lowHull = String(model.health !== null && fraction <= 1/3);
    this.element.dataset.hit = String(model.hit);
    this.setStatus(model.status,model.tone);
  }
  setStatus(text, tone = 'normal') {
    this.notice.hidden = !text; this.notice.dataset.tone = tone;
    // Countdown updates are visible but not re-announced every network tick.
    const announcement = text.replace(/\d+\.\d+s/g,'');
    if (this.notice.dataset.announcement !== announcement) {
      this.announcement.textContent = announcement; this.notice.dataset.announcement = announcement;
    }
    if (this.notice.textContent !== text) this.notice.textContent = text;
  }
  clear() { this.element.hidden = true; }
  destroy() { this.observer.disconnect(); this.viewport.before(this.canvas); this.viewport.remove(); }
}
