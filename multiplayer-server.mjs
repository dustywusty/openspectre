import { assetPack } from './asset-pack.mjs';
import { WebSocketServer, WebSocket } from 'ws';
import { randomBytes } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';

// The server owns the simulation. Clients can submit controls, never positions,
// hits, health, scores, player IDs, or arbitrary simulation step counts.
export function attachDeathmatch(server, { pack = assetPack() } = {}) {
  const wss = new WebSocketServer({ noServer: true, maxPayload: 1024, perMessageDeflate: false });
  const rooms = new Map();
  let assets;
  const send = (ws, data) => {
    if (ws?.readyState !== WebSocket.OPEN) return;
    if (ws.bufferedAmount > 256 * 1024) { ws.close(1013, 'Connection too slow'); return; }
    ws.send(JSON.stringify(data));
  };
  const load = () => assets ??= Promise.all([
    readFile(new URL('native-web/spectre_core.wasm', import.meta.url)).then(b=>WebAssembly.compile(b)),
    readFile(pack.data('spectre')),
    readFile(pack.data('central')),
  ]).catch(error=>{ assets=undefined; throw error; });
  async function createRoom() {
    // Reserve before the async load so simultaneous creates cannot bypass the cap.
    if (rooms.size >= 4) throw new Error('All four rooms are occupied. Try again shortly.');
    const id=randomBytes(6).toString('hex');
    const room={id,clients:[null,null],lastInput:[0,0],votes:[false,false],w:null,last:performance.now(),accumulator:0,steps:0,round:0,inputs:[[],[]],ack:[0,0],received:[0,0]};
    rooms.set(id,room);
    try {
      const [module,shared,arena]=await load();
      room.w=(await WebAssembly.instantiate(module,{})).exports;
      const feed=(bytes,fn)=>{const p=room.w.input_buffer(bytes.length);new Uint8Array(room.w.memory.buffer,p,bytes.length).set(bytes);if(!room.w[fn]())throw new Error('Could not load the deathmatch assets');};
      feed(shared,'load_shared_colors');feed(arena,'load_archive');
      if(!room.w.dm_start())throw new Error('Could not initialize the arena');
      return room;
    } catch(error) {rooms.delete(id);throw error;}
  }
  function broadcast(room) {
    room.w.dm_snapshot();
    const state=JSON.parse(new TextDecoder().decode(new Uint8Array(room.w.memory.buffer,room.w.output_ptr(),room.w.output_len())));
    const message={type:'state',state,connected:room.clients.map(c=>c?.readyState===WebSocket.OPEN),rematch:room.votes,round:room.round,ack:room.ack};
    for(const ws of room.clients)send(ws,message);
  }
  function reset(room) {
    if(!room.w.dm_start())throw new Error('Match reset failed');
    room.round++;room.inputs=[[],[]];room.ack=[0,0];room.received=[0,0];room.votes=[false,false];room.lastInput=[0,0];room.accumulator=0;room.last=performance.now();
  }
  server.on('upgrade',(req,socket,head)=>{
    const url=new URL(req.url,'http://localhost');
    if(url.pathname!=='/multiplayer/socket') {socket.destroy();return;}
    if(req.headers.origin) {
      try {if(new URL(req.headers.origin).host!==req.headers.host){socket.destroy();return;}}
      catch {socket.destroy();return;}
    }
    wss.handleUpgrade(req,socket,head,ws=>wss.emit('connection',ws,req));
  });
  wss.on('connection',ws=>{
    let room,slot,busy=false,windowStart=performance.now(),messages=0;
    ws.alive=true;
    const joinDeadline=setTimeout(()=>{if(!room)ws.close(1008,'Join a room first');},15000);
    ws.on('pong',()=>{ws.alive=true;});
    ws.on('error',()=>{});
    ws.on('message',async bytes=>{
      const now=performance.now();
      if(now-windowStart>1000){windowStart=now;messages=0;}
      if(++messages>120){ws.close(1008,'Too many messages');return;}
      let msg;try{msg=JSON.parse(bytes);}catch{ws.close(1008,'Invalid message');return;}
      if(!msg || typeof msg!=='object')return;
      if(msg.type==='ping'){send(ws,{type:'pong',at:msg.at});return;}
      if(msg.type==='release'&&room&&msg.round===room.round) {
        room.inputs[slot]=[];room.ack[slot]=room.received[slot];
        room.w.dm_input(slot,0,0,0,0);return;
      }
      if(msg.type==='input'&&room) {
        if(![-1,0,1].includes(msg.throttle)||![-1,0,1].includes(msg.steer)||typeof msg.fire!=='boolean'||typeof msg.brake!=='boolean')return;
        if(msg.seq!==undefined) {
          if(msg.round!==room.round||!Number.isSafeInteger(msg.seq)||msg.seq<=room.received[slot]||msg.seq>room.received[slot]+120||room.inputs[slot].length>=120)return;
          room.received[slot]=msg.seq;room.inputs[slot].push(msg);room.lastInput[slot]=now;return;
        }
        room.lastInput[slot]=now;
        room.w.dm_input(slot,msg.throttle,msg.steer,Number(msg.brake),Number(msg.fire));return;
      }
      if(msg.type==='rematch'&&room) {
        room.votes[slot]=true;
        if(room.votes.every(Boolean)&&room.clients.every(Boolean))reset(room);
        broadcast(room);return;
      }
      if(room||busy||!['create','join'].includes(msg.type))return;
      busy=true;
      try {
        let candidate;
        if(msg.type==='create')candidate=await createRoom();
        else {
          if(typeof msg.room!=='string'||!/^[a-f0-9]{12}$/.test(msg.room))throw new Error('Invalid room link.');
          candidate=rooms.get(msg.room);
          if(!candidate?.w)throw new Error('Room not found. Ask your friend for a new link.');
        }
        if(ws.readyState!==WebSocket.OPEN) {if(msg.type==='create')rooms.delete(candidate.id);return;}
        const free=candidate.clients.findIndex(c=>!c);
        if(free<0)throw new Error('This room already has two players.');
        room=candidate;slot=free;room.clients[slot]=ws;clearTimeout(joinDeadline);
        if(room.clients.every(Boolean))reset(room);
        send(ws,{type:'welcome',room:room.id,slot,pack:pack.id});broadcast(room);
      } catch(error){send(ws,{type:'error',message:error.message});}
      finally{busy=false;}
    });
    ws.on('close',()=>{
      clearTimeout(joinDeadline);
      if(!room)return;
      room.clients[slot]=null;room.votes=[false,false];
      room.w.dm_input(0,0,0,0,0);room.w.dm_input(1,0,0,0,0);
      if(room.clients.every(c=>!c))rooms.delete(room.id);else broadcast(room);
    });
  });
  const timer=setInterval(()=>{
    const now=performance.now();
    for(const room of rooms.values()) {
      if(!room.w)continue;
      const elapsed=Math.min(100,now-room.last);room.last=now;
      if(!room.clients.every(c=>c?.readyState===WebSocket.OPEN)){room.accumulator=0;continue;}
      for(let i=0;i<2;i++)if(now-room.lastInput[i]>500)room.w.dm_input(i,0,0,0,0);
      room.accumulator+=elapsed;
      while(room.accumulator>=1000/60){
        for(let i=0;i<2;i++){
          const input=room.inputs[i].shift();
          if(input){room.w.dm_input(i,input.throttle,input.steer,Number(input.brake),Number(input.fire));room.ack[i]=input.seq;}
        }
        room.w.dm_step();room.accumulator-=1000/60;room.steps++;}
      if(room.steps>=3){room.steps=0;broadcast(room);}
    }
  },8);
  const heartbeat=setInterval(()=>{for(const ws of wss.clients){if(!ws.alive){ws.terminate();continue;}ws.alive=false;ws.ping();}},10000);
  server.on('close',()=>{clearInterval(timer);clearInterval(heartbeat);for(const ws of wss.clients)ws.terminate();wss.close();rooms.clear();});
  return {rooms,wss};
}
