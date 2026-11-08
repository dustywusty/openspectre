import assert from 'node:assert/strict';
import {WebSocket} from 'ws';
const origin=new URL(process.argv[2]??'http://127.0.0.1:18081');
const peers=[];
const timeout=setTimeout(()=>{console.error('Deployment smoke test timed out');process.exit(1);},20000);
try{
  for(const path of ['/','/app.js','/hud.js','/multiplayer.js','/style.css','/data/central.bin','/audio/10001.wav']){
    const response=await fetch(new URL(path,origin));assert.equal(response.status,200,path);assert.ok((await response.arrayBuffer()).byteLength>0);
  }
  const pack=await (await fetch(new URL('/pack.json',origin))).json();assert.equal(pack.id,'open');
  const bytes=new Uint8Array(await (await fetch(new URL('/spectre_core.wasm',origin))).arrayBuffer());
  assert.deepEqual([...bytes.slice(0,4)],[0,97,115,109]);await WebAssembly.compile(bytes);
  const connect=async command=>{
    const url=new URL('/multiplayer/socket',origin);url.protocol=origin.protocol==='https:'?'wss:':'ws:';
    const ws=new WebSocket(url);peers.push(ws);let state;
    ws.on('message',b=>{const msg=JSON.parse(b);if(msg.type==='state')state=msg.state;});
    const welcome=await new Promise((resolve,reject)=>{
      ws.on('error',reject);ws.on('open',()=>ws.send(JSON.stringify(command)));
      ws.on('message',b=>{const msg=JSON.parse(b);if(msg.type==='welcome')resolve(msg);if(msg.type==='error')reject(Error(msg.message));});
    });
    return {ws,welcome,get state(){return state;}};
  };
  const a=await connect({type:'create'}),b=await connect({type:'join',room:a.welcome.room});
  assert.equal(a.welcome.pack,'open');assert.equal(b.welcome.slot,1);
  await new Promise(resolve=>setTimeout(resolve,250));const tick=a.state.tick;
  await new Promise(resolve=>setTimeout(resolve,250));assert.ok(a.state.tick>tick);assert.ok(b.state.tick>tick);
  console.log(`PASS: HTTP assets, WASM compile, and two live multiplayer clients at ${origin.origin}`);
}finally{clearTimeout(timeout);for(const peer of peers)peer.terminate();}
