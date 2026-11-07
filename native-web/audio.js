// Original local-disc samples. Damage-response, flag, extra-life, enemy-clear,
// and weapon-constructor IDs are recovered; channel scheduling remains provisional.
export const GAME_SOUND_IDS = [10001,10002,10003,10004,10005,10006,10007,10010,10013,10017,10020,10021,10023,10024,10027,10028,10035,10037,10038,10042,10044,10045,10047];
export class GameAudio {
  constructor(status) {
    this.status=status; this.context=null; this.buffers=new Map(); this.voices=new Set();
    this.volume=0.35; this.muted=false; this.loading=null;
  }
  async unlock() {
    try {
      if(!this.context) {
        this.context=new AudioContext();
        this.gain=this.context.createGain(); this.gain.connect(this.context.destination);this.applyVolume();
      }
      await this.context.resume();
      if(!this.loading) this.loading=this.load();
      await this.loading;
      this.status.textContent=this.muted?'Sound muted':'Original sound effects ready';
    } catch(error) {
      this.loading=null;this.status.textContent='Sound unavailable · click Drive tank to retry';
      console.warn('Sound setup failed',error);
    }
  }
  async load() {
    this.status.textContent='Loading original sound effects…';
    await Promise.all(GAME_SOUND_IDS.map(async id=>{
      const response=await fetch(`/audio/${id}.wav`);
      if(!response.ok) throw new Error(`Sound ${id}: HTTP ${response.status}`);
      this.buffers.set(id,await this.context.decodeAudioData(await response.arrayBuffer()));
    }));
  }
  applyVolume() {if(this.gain)this.gain.gain.setTargetAtTime(this.muted?0:this.volume,this.context.currentTime,0.015);}
  stop() {for(const source of this.voices){try{source.stop();}catch{}}this.voices.clear();}
  play(events) {
    if(this.muted||!this.context||this.context.state!=='running')return;
    for(const id of events) {
      const buffer=this.buffers.get(id); if(!buffer)continue;
      if(this.voices.size>=12){const oldest=this.voices.values().next().value;oldest.stop();this.voices.delete(oldest);}
      const source=this.context.createBufferSource();source.buffer=buffer;source.connect(this.gain);
      source.onended=()=>{this.voices.delete(source);source.disconnect();};this.voices.add(source);source.start();
    }
  }
}
