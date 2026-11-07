import { GameAudio } from '/audio.js';
import { GameHud } from '/hud.js';
const $=s=>document.querySelector(s);
export class Multiplayer {
  constructor() {
    this.panel=$('#multiplayer-panel');this.canvas=$('#mp-view');this.context=this.canvas.getContext('2d',{alpha:false});
    this.hud=new GameHud(this.canvas);
    this.keys=new Set();this.touches=new Map();this.slot=null;this.active=false;this.camera=1;
    this.sound=new GameAudio($('#mp-sound-status'));
    this.roomFromURL=new URL(location.href).searchParams.get('room');
    this.invitePending=Boolean(this.roomFromURL);
    $('#mp-room').value=this.roomFromURL??'';
    $('#mp-create').onclick=()=>this.connect();
    $('#mp-join').onclick=()=>this.connect($('#mp-room').value.trim());
    $('#mp-leave').onclick=()=>this.leave();
    $('#mp-rematch').onclick=()=>{this.send({type:'rematch'});$('#mp-rematch').disabled=true;};
    $('#mp-copy').onclick=async()=>{
      try {await navigator.clipboard.writeText($('#mp-link').value);$('#mp-copy').textContent='Copied';}
      catch {$('#mp-link').select();this.status('Select and copy the room link below.');}
    };
    $('#mp-camera').onchange=()=>{this.camera=Number($('#mp-camera').value);};
    $('#mp-fullscreen').onclick=()=>this.fullscreen();
    $('#mp-mute').onclick=()=>{this.sound.muted=!this.sound.muted;this.sound.applyVolume();$('#mp-mute').textContent=this.sound.muted?'Unmute':'Mute';$('#mp-mute').setAttribute('aria-pressed',String(this.sound.muted));};
    this.canvas.onpointerdown=()=>{this.canvas.focus();void this.sound.unlock();};
    this.canvas.onkeydown=e=>{
      if(['KeyW','KeyA','KeyS','KeyD','ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space','ShiftLeft','ShiftRight'].includes(e.code)) {e.preventDefault();this.keys.add(e.code);this.sendInput();}
      if(e.code==='Tab'&&!e.shiftKey){e.preventDefault();if(!e.repeat){this.camera=(this.camera+1)%3;$('#mp-camera').value=String(this.camera);}}
      if(e.code==='KeyF'&&!e.repeat){e.preventDefault();void this.fullscreen();}
      if(e.code==='Escape'){this.release();this.canvas.blur();}
    };
    window.addEventListener('keyup',e=>{if(this.keys.delete(e.code))this.sendInput();});
    this.canvas.onblur=()=>this.release();
    window.addEventListener('blur',()=>this.release());
    document.addEventListener('visibilitychange',()=>{if(document.hidden)this.release();});
    document.querySelectorAll('[data-mp-control]').forEach(button=>{
      button.onpointerdown=e=>{e.preventDefault();button.setPointerCapture(e.pointerId);this.touches.set(e.pointerId,button.dataset.mpControl);void this.sound.unlock();this.sendInput();};
      button.onpointerup=button.onpointercancel=button.onlostpointercapture=e=>{this.touches.delete(e.pointerId);this.sendInput();};
    });

    setInterval(()=>{if(this.ws?.readyState===WebSocket.OPEN)this.send({type:'ping',at:performance.now()});},2000);
    requestAnimationFrame(time=>this.draw(time));
  }
  status(text){$('#mp-status').textContent=text;this.hud.setStatus(text);}
  async enable(value){
    this.active=value;this.panel.hidden=!value;
    if(!value){this.leave();return;}
    if(!this.w) {this.status('Loading multiplayer renderer…');try{await this.load();this.status('Create a room, or join your friend’s room.');}catch(e){this.status(`Could not load multiplayer: ${e.message}`);return;}}
    if(!this.active)return;
    $('#mp-create').disabled=Boolean(this.ws);$('#mp-join').disabled=Boolean(this.ws);
    if(this.invitePending&&!this.ws){
      this.invitePending=false;
      await this.connect(this.roomFromURL,{unlockSound:false});
    }
  }
  load(){
    return this.loading??= (async()=>{
      const get=async path=>{const r=await fetch(path);if(!r.ok)throw new Error(`${path}: ${r.status}`);return r.arrayBuffer();};
      const [bytes,shared,arena]=await Promise.all([get('/spectre_core.wasm'),get('/data/spectre.bin'),get('/data/central.bin')]);
      this.w=(await WebAssembly.instantiate(bytes,{})).instance.exports;
      for(const [bytes,fn] of [[shared,'load_shared_colors'],[arena,'load_archive']]){this.feed(new Uint8Array(bytes));if(!this.w[fn]())throw new Error('Original assets could not load');}
      if(!this.w.dm_prediction_start())throw new Error('Reload to update the movement engine.');
      if(!this.w.dm_view_start())throw new Error('Multiplayer renderer is missing. Reload after the build finishes.');
    })().catch(e=>{this.loading=null;this.w=null;throw e;});
  }
  feed(bytes){const ptr=this.w.input_buffer(bytes.length);new Uint8Array(this.w.memory.buffer,ptr,bytes.length).set(bytes);}
  send(message){if(this.ws?.readyState===WebSocket.OPEN)this.ws.send(JSON.stringify(message));}
  sendInput(){ /* Controls are sampled once per simulation tick. */ }
  controls(){
    const down=(...codes)=>codes.some(c=>this.keys.has(c)||[...this.touches.values()].includes(c));
    return {throttle:Number(down('KeyW','ArrowUp'))-Number(down('KeyS','ArrowDown')),steer:Number(down('KeyD','ArrowRight'))-Number(down('KeyA','ArrowLeft')),brake:down('ShiftLeft','ShiftRight'),fire:down('Space')};
  }
  release(){this.keys.clear();this.touches.clear();this.send({type:'release',round:this.round});}
  async connect(room,{unlockSound=true}={}){
    if(!this.w){this.status('The renderer is still loading. Try Join room once it is ready.');return;}
    if(room!==undefined&&!/^[a-f0-9]{12}$/.test(room)){this.status('Paste the 12-character room code, or open the full invitation link.');return;}
    this.leave();
    const attempt=this.attempt;
    $('#mp-create').disabled=true;$('#mp-join').disabled=true;
    if(unlockSound)void this.sound.unlock();
    if(!this.active||attempt!==this.attempt)return;
    this.status('Connecting…');
    const ws=new WebSocket(`${location.protocol==='https:'?'wss':'ws'}://${location.host}/multiplayer/socket`);this.ws=ws;
    const fail=message=>{
      if(this.ws!==ws)return;
      clearTimeout(this.connectionTimer);this.ws=null;this.slot=null;ws.close();
      this.status(message);$('#mp-create').disabled=false;$('#mp-join').disabled=false;$('#mp-leave').disabled=true;
    };
    this.connectionTimer=setTimeout(()=>fail('Connection timed out. Try Join room again; this network may be blocking the game connection.'),15000);
    ws.onopen=()=>{if(this.ws===ws)this.send(room===undefined?{type:'create'}:{type:'join',room});};
    ws.onmessage=e=>{
      if(this.ws!==ws)return;
      let msg;try{msg=JSON.parse(e.data);}catch{return;}
      if(msg.type==='welcome'){
        clearTimeout(this.connectionTimer);
        this.slot=msg.slot;this.room=msg.room;
        const url=new URL(location.href);url.searchParams.set('room',msg.room);url.hash='drive';
        $('#mp-link').value=url.href;$('#mp-invite').hidden=false;$('#mp-leave').disabled=false;
        $('#mp-room').value=msg.room;$('#mp-copy').textContent='Copy invite link';
        this.canvas.focus({preventScroll:true});
      }else if(msg.type==='state'){
        this.previous=this.latest;this.latest=msg.state;this.received=performance.now();
        if(this.previous&&this.latest.tick<=this.previous.tick)this.previous=null;
        this.sound.play(msg.state.sounds);
        this.sound.play(msg.state.player_sounds?.[1-(this.slot??0)]??[]);
        const old=this.predicted;
        const reset=this.round!==msg.round;
        this.round=msg.round;
        if(reset){this.pending=[];this.sequence=0;this.accumulator=0;}
        this.pending=(this.pending??[]).filter(input=>input.seq>(msg.ack?.[this.slot]??0));
        this.feed(new TextEncoder().encode(JSON.stringify(msg.state.players[this.slot??0])));
        if(this.w.dm_predict_restore()){
          this.predicted=this.output();
          for(const input of this.pending)this.predict(input,false);
          if(!reset&&old&&msg.state.players[this.slot].health>0&&Math.hypot(old.x-this.predicted.x,old.z-this.predicted.z)<40){
            this.correction={x:(this.correction?.x??0)+old.x-this.predicted.x,z:(this.correction?.z??0)+old.z-this.predicted.z,
              heading:(this.correction?.heading??0)+Math.atan2(Math.sin(old.heading-this.predicted.heading),Math.cos(old.heading-this.predicted.heading))};
          }else this.correction=null;
        }
        this.ready=msg.connected.every(Boolean)&&msg.state.winner===null;
        const me=msg.state.players[this.slot??0];
        const other=msg.state.players[1-(this.slot??0)];
        $('#mp-score').textContent=`You ${me.kills} — ${other.kills} Opponent · Hull ${me.health}/3 · First to 5`;
        const ready=msg.connected.every(Boolean);
        const winner=msg.state.winner;
        this.status(!ready?'Waiting for another player · share the invite link':winner!==null?(winner===this.slot?'You win!':'Opponent wins!'):me.respawn>0?`Respawning in ${(me.respawn/60).toFixed(1)}s`:me.shield>0?'Spawn shield · firing ends protection':'Live · click the arena to drive');
        this.hud.updateMultiplayer(msg.state,{slot:this.slot??0,connected:msg.connected});
        $('#mp-rematch').hidden=winner===null;$('#mp-rematch').disabled=!ready||msg.rematch[this.slot??0];
        $('#mp-rematch-note').textContent=msg.rematch.some(Boolean)?'Rematch requested · both players must agree':'';
      }else if(msg.type==='error'){fail(msg.message);}
      else if(msg.type==='pong'&&typeof msg.at==='number')$('#mp-latency').textContent=`${Math.round(performance.now()-msg.at)} ms round trip`;
    };
    ws.onclose=()=>{if(this.ws!==ws)return;clearTimeout(this.connectionTimer);this.ws=null;this.slot=null;this.release();this.status('Disconnected · create or join a room to reconnect.');$('#mp-create').disabled=false;$('#mp-join').disabled=false;$('#mp-leave').disabled=true;$('#mp-invite').hidden=true;};
    ws.onerror=()=>fail('Connection failed. Try Join room again, or another network if this Wi-Fi blocks game connections.');
  }
  leave(){
    clearTimeout(this.connectionTimer);
    this.hud.clear();
    this.attempt=(this.attempt??0)+1;
    this.release();const ws=this.ws;this.ws=null;ws?.close();this.slot=null;this.latest=null;this.previous=null;this.predicted=null;this.correction=null;this.pending=[];this.round=null;this.ready=false;this.sound.stop();
    $('#mp-create').disabled=!this.w;$('#mp-join').disabled=!this.w;$('#mp-leave').disabled=true;$('#mp-invite').hidden=true;$('#mp-rematch').hidden=true;$('#mp-rematch-note').textContent='';$('#mp-score').textContent='Two players · First to 5 · Cannons only';$('#mp-latency').textContent='';this.status('Create a room, or join your friend’s room.');
  }
  output(){return JSON.parse(new TextDecoder().decode(new Uint8Array(this.w.memory.buffer,this.w.output_ptr(),this.w.output_len())));}
  predict(input,audio){
    this.w.dm_predict_step(input.throttle,input.steer,Number(input.brake),Number(input.fire));
    this.predicted=this.output();
    if(audio)this.sound.play(this.predicted.sounds);
  }
  async fullscreen(){
    try{if(document.fullscreenElement)await document.exitFullscreen();else await this.panel.requestFullscreen();this.canvas.focus();}
    catch{this.status('Fullscreen is unavailable here. Open this URL in a browser window.');}
  }
  draw(time){
    requestAnimationFrame(t=>this.draw(t));
    const elapsed=Math.min(100,Math.max(0,time-(this.lastDraw??time)));this.lastDraw=time;
    if(!this.active||!this.w||!this.latest||document.hidden)return;
    if(this.ready&&this.predicted&&this.ws?.readyState===WebSocket.OPEN){
      this.accumulator=(this.accumulator??0)+elapsed;
      while(this.accumulator>=1000/60){
        this.accumulator-=1000/60;
        if(this.pending.length>=120||this.ws.bufferedAmount>65536)continue;
        const input={type:'input',round:this.round,seq:++this.sequence,...this.controls()};
        this.pending.push(input);this.send(input);this.predict(input,true);
      }
    }else this.accumulator=0;
    const state=structuredClone(this.latest);
    if(this.previous){
      const span=Math.max(1,(this.latest.tick-this.previous.tick)*1000/60);
      const t=Math.min(1,(time-this.received)/span);
      for(let i=0;i<2;i++){
        if(i===this.slot)continue;
        const a=this.previous.players[i],b=state.players[i];
        if(a.health===0||b.health===0||Math.hypot(b.x-a.x,b.z-a.z)>100)continue;
        b.x=a.x+(b.x-a.x)*t;b.z=a.z+(b.z-a.z)*t;
        const delta=Math.atan2(Math.sin(b.heading-a.heading),Math.cos(b.heading-a.heading));b.heading=a.heading+delta*t;
      }
      for(const b of state.shots){const a=this.previous.shots.find(a=>a.id===b.id&&a.owner===b.owner);if(a){b.x=a.x+(b.x-a.x)*t;b.z=a.z+(b.z-a.z)*t;}}
    }
    if(this.predicted&&this.slot!==null){
      const {x,z,heading,speed}=this.predicted;
      const decay=Math.exp(-elapsed/65);
      if(this.correction)for(const k of ['x','z','heading'])this.correction[k]*=decay;
      Object.assign(state.players[this.slot],{x:x+(this.correction?.x??0),z:z+(this.correction?.z??0),heading:heading+(this.correction?.heading??0),speed});
      state.shots=state.shots.filter(s=>s.owner!==this.slot).concat(this.predicted.shots.map(s=>({...s,owner:this.slot})));
    }
    try{
      this.feed(new TextEncoder().encode(JSON.stringify(state)));this.w.game_camera(this.camera);
      const ptr=this.w.dm_render(this.slot??0);if(!ptr)throw new Error('Invalid server snapshot');
      this.context.putImageData(new ImageData(new Uint8ClampedArray(this.w.memory.buffer,ptr,800*600*4),800,600),0,0);
    }catch(error){this.status(`Rendering failed: ${error.message}`);this.latest=null;}
  }
}
