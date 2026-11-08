// Two real browser clients, one real server, ordered WebSocket delay injection.
import assert from 'node:assert/strict';
import dns from 'node:dns';
import {spawn,spawnSync} from 'node:child_process';
import {mkdirSync,writeFileSync,appendFileSync} from 'node:fs';
import {chromium} from 'playwright';
const tunneled=process.argv.includes('--tunnel');
const dir=tunneled?'artifacts/multiplayer-tunnel':'artifacts/multiplayer';mkdirSync(dir,{recursive:true});
const server=spawn(process.execPath,['native-server.mjs'],{env:{...process.env,NATIVE_PORT:'0',SPECTRE_ASSETS:'open'},stdio:['ignore','pipe','inherit']});
let browser,context,tunnel,edgeIP;
const systemLookup=dns.lookup;
const socketURLs=[];
const timers=new Set(),errors=[],samples=[];
let phase='Local baseline',delay=0,jitter=false,epoch=performance.now();
try{
  const localOrigin=await new Promise((resolve,reject)=>{
    const timeout=setTimeout(()=>reject(Error('Server startup timed out')),10000);
    server.stdout.on('data',b=>{const url=b.toString().match(/http:\/\/localhost:\d+/);if(url){clearTimeout(timeout);resolve(url[0]);}});
    server.on('error',reject);server.on('exit',code=>reject(Error(`Server exited ${code}`)));
  });
  let origin=localOrigin;
  if(tunneled){
    writeFileSync(`${dir}/tunnel.log`,'');
    tunnel=spawn('cloudflared',['tunnel','--protocol','http2','--url',localOrigin,'--no-autoupdate'],{stdio:['ignore','ignore','pipe']});
    origin=await new Promise((resolve,reject)=>{
      const timeout=setTimeout(()=>reject(Error('Public tunnel startup timed out')),30000);
      tunnel.stderr.on('data',b=>{appendFileSync(`${dir}/tunnel.log`,b);const text=b.toString();const url=text.match(/https:\/\/[a-z0-9-]+\.trycloudflare\.com/);if(url){clearTimeout(timeout);resolve(url[0]);}});
      tunnel.once('error',e=>{clearTimeout(timeout);reject(e);});
      tunnel.once('exit',code=>{clearTimeout(timeout);reject(Error(`Tunnel exited ${code}`));});
    });
    console.log(`Tunnel announced: ${origin}`);
    const host=new URL(origin).hostname;
    try{await dns.promises.lookup(host);}catch{
      // Some local resolvers cache NXDOMAIN for newly issued tunnel names.
      // Resolve only this public hostname; never redirect traffic to localhost.
      const resolver=new dns.promises.Resolver();resolver.setServers(['1.1.1.1']);
      for(let attempt=0;attempt<30;attempt++){
        try{[edgeIP]=await resolver.resolve4(host);break;}catch{await new Promise(resolve=>setTimeout(resolve,1000));}
      }
      assert.ok(edgeIP,'public DNS resolves the new tunnel hostname');
      dns.lookup=(name,options,callback)=>{
        if(name!==host)return systemLookup(name,options,callback);
        if(typeof options==='function'){callback=options;options={};}
        queueMicrotask(()=>options?.all?callback(null,[{address:edgeIP,family:4}]):callback(null,edgeIP,4));
      };
      console.log(`Public DNS fallback: ${host} -> ${edgeIP}`);
    }
    // The URL can be announced before the public route is ready.
    let ready=false,lastError='';
    for(let attempt=0;attempt<30;attempt++){
      try{const response=await fetch(origin+'/pack.json',{signal:AbortSignal.timeout(5000)});if(response.ok&&(await response.json()).id==='open'){ready=true;break;}lastError=`HTTP ${response.status}`;}catch(e){lastError=e.message;}
      await new Promise(resolve=>setTimeout(resolve,1000));
    }
    assert.ok(ready,`Public tunnel unavailable: ${lastError}; see ${dir}/tunnel.log`);
    console.log(`Public game origin: ${origin}`);
  }
  browser=await chromium.launch({...(process.env.PLAYWRIGHT_CHANNEL?{channel:process.env.PLAYWRIGHT_CHANNEL}:{}),args:edgeIP?[`--host-resolver-rules=MAP ${new URL(origin).hostname} ${edgeIP}`]:[]});
  context=await browser.newContext({viewport:{width:1600,height:760},recordVideo:{dir,size:{width:1600,height:760},fps:30}});
  // Instrument the served module only; production source and simulation are unchanged.
  await context.route('**/multiplayer.js',async route=>{
    try{
    const response=await fetch(route.request().url());assert.ok(response.ok,'multiplayer module loads through requested origin');let body=await response.text();
    body=body.replace('constructor() {','constructor() { window.__mp=this;');
    body=body.replace("if(msg.type==='welcome'){","if(msg.type==='pong'&&typeof msg.at==='number')(window.__rtts??=[]).push(performance.now()-msg.at); if(msg.type==='welcome'){");
    body+=`\nconst originalDraw=Multiplayer.prototype.draw;\nMultiplayer.prototype.draw=function(t){\n const d=window.__diag??={frames:0,held:0,maxGap:0,maxCorrection:0};\n if(this.ready){d.frames++;d.maxGap=Math.max(d.maxGap,t-(d.last??t));\n if(this.previous && t-this.received>(this.latest.tick-this.previous.tick)*1000/60)d.held++;\n d.maxCorrection=Math.max(d.maxCorrection,Math.hypot(this.correction?.x??0,this.correction?.z??0));}\n d.last=t;return originalDraw.call(this,t);\n};`;
    await route.fulfill({status:response.status,contentType:'text/javascript',body});
    }catch(error){errors.push(error.message);await route.abort();}
  });
  await context.routeWebSocket('**/multiplayer/socket',ws=>{
    socketURLs.push(ws.url());
    assert.equal(new URL(ws.url()).host,new URL(origin).host,'WebSocket uses the requested origin');
    const upstream=ws.connectToServer();let upDue=0,downDue=0,count=0;
    const later=(message,to,down)=>{
      const now=performance.now();let latency=delay;
      if(jitter&&down){latency+=(count++%5)*18;if((now-epoch)%4000<300)latency+=300;}
      const due=Math.max(down?downDue:upDue,now+latency);
      if(down)downDue=due;else upDue=due;
      const timer=setTimeout(()=>{timers.delete(timer);to.send(message);},Math.max(0,due-now));timers.add(timer);
    };
    ws.onMessage(message=>later(message,upstream,false));
    upstream.onMessage(message=>later(message,ws,true));
  });
  await context.route('**/multiplayer-recording',route=>route.fulfill({contentType:'text/html',body:`<!doctype html><style>body{margin:0;background:#071018;color:white;font:18px monospace}header{height:60px;padding:8px;box-sizing:border-box}iframe{border:0;width:800px;height:700px;float:left}</style><header id="label">Loading two browser clients</header><iframe name="a" src="/?play=multiplayer"></iframe><iframe name="b" src="/?play=multiplayer"></iframe>`}));
  const page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));
  await page.goto(origin+'/multiplayer-recording');
  await page.waitForFunction(()=>[...document.querySelectorAll('iframe')].every(f=>f.contentWindow.__mp?.w));
  const frames=[page.frame({name:'a'}),page.frame({name:'b'})];
  for(const frame of frames){
    await frame.addStyleTag({content:'body{margin:0!important;padding:0!important} #multiplayer-panel{position:fixed!important;inset:0!important;margin:0!important;padding:8px!important;border:0!important;background:#071018;overflow:hidden} #multiplayer-panel>.game-viewport{max-width:768px!important;width:768px!important} #mp-view{width:768px!important;height:576px!important;max-height:none!important}'});
  }
  await frames[0].locator('#mp-create').click();
  await frames[0].waitForFunction(()=>window.__mp.room);
  const room=await frames[0].evaluate(()=>window.__mp.room);
  await frames[1].locator('#mp-room').fill(room);await frames[1].locator('#mp-join').click();
  await Promise.all(frames.map(f=>f.waitForFunction(()=>window.__mp.ready)));
  for(const frame of frames)await frame.addStyleTag({content:'#multiplayer-panel > .toolbar, #multiplayer-panel > .drive-controls, #multiplayer-panel > p:not(#mp-status){display:none!important} #mp-status{margin:4px!important}'});
  // DOM keyboard events go through the same canvas handlers as physical keys.
  for(const frame of frames)await frame.evaluate(()=>{
    window.__motionOnly=true;window.__drive=setInterval(()=>{
      const m=window.__mp;if(!m?.ready){if(m?.latest?.winner!==null&&!document.querySelector('#mp-rematch').disabled)document.querySelector('#mp-rematch').click();return;}
      const me=m.predicted;if(!me)return;
      const target={x:m.slot===0?320:704,z:window.__turn?640:384};
      const aim=Math.atan2(target.x-me.x,target.z-me.z);
      const angle=Math.atan2(Math.sin(aim-me.heading),Math.cos(aim-me.heading));
      const wanted=new Set(window.__motionOnly?[]:['Space']);
      if(Math.abs(angle)<.65&&Math.hypot(target.x-me.x,target.z-me.z)>15)wanted.add('KeyW');
      if(Math.abs(angle)>.05)wanted.add(angle>0?'KeyD':'KeyA');
      for(const code of ['Space','KeyW','KeyA','KeyD']){
        if(wanted.has(code)&&!m.keys.has(code))m.canvas.dispatchEvent(new KeyboardEvent('keydown',{code,bubbles:true}));
        if(!wanted.has(code)&&m.keys.has(code))window.dispatchEvent(new KeyboardEvent('keyup',{code,bubbles:true}));
      }
    },50);
  });
  for(const profile of [{name:tunneled?'Public tunnel - no injected delay':'Local baseline',delay:0,jitter:false},{name:'150 ms added round-trip delay',delay:75,jitter:false},{name:'150 ms RTT + jitter + 300 ms receive stalls',delay:75,jitter:true}]){
    phase=(tunneled&&profile.delay?'Public tunnel + ':'')+profile.name;delay=profile.delay;jitter=profile.jitter;epoch=performance.now();
    console.log(`Recording: ${phase}`);
    for(const frame of frames)await frame.evaluate(()=>{window.__diag=undefined;window.__rtts=[];window.__mp.camera=2;document.querySelector('#mp-camera').value='2';});
    for(let step=0;step<60;step++){
      await Promise.all(frames.map(f=>f.evaluate(step=>{window.__motionOnly=step<40;window.__turn=Math.floor(step/10)%2===1;},step)));
      const data=await Promise.all(frames.map(f=>f.evaluate(()=>{
        const m=window.__mp;return {diag:window.__diag,rtts:window.__rtts??[],tick:m.latest.tick,pending:m.pending.length,correction:Math.hypot(m.correction?.x??0,m.correction?.z??0),players:m.latest.players.map(p=>({x:p.x,z:p.z,health:p.health,kills:p.kills})),status:document.querySelector('#mp-status').textContent};
      })));
      samples.push({phase,elapsed:performance.now()-epoch,clients:data});
      await page.locator('#label').evaluate((el,{phase,step,data})=>{el.textContent=`${phase} | ${(step/4).toFixed(1)}s | Player 1 (left) / Player 2 (right)\nPending inputs ${data.map(d=>d.pending).join(' / ')} | correction ${data.map(d=>d.correction.toFixed(1)).join(' / ')} world units`;el.style.whiteSpace='pre';},{phase,step,data});
      if(step===40)for(const frame of frames)await frame.evaluate(()=>{window.__mp.camera=1;document.querySelector('#mp-camera').value='1';});
      if(step===30)await page.screenshot({path:`${dir}/${profile.delay===0?'baseline':profile.jitter?'jitter':'latency'}.png`});
      await new Promise(resolve=>setTimeout(resolve,250));
    }
  }
  const video=page.video();
  for(const frame of frames)await frame.evaluate(()=>clearInterval(window.__drive));
  for(const timer of timers)clearTimeout(timer);timers.clear();
  await context.close();context=null;
  const source=await video.path();
  const encoded=spawnSync('ffmpeg',['-loglevel','error','-y','-i',source,'-c:v','libx264','-preset','fast','-crf','23','-pix_fmt','yuv420p','-movflags','+faststart',`${dir}/two-clients.mp4`]);
  assert.equal(encoded.status,0,encoded.stderr?.toString());
  const summary=[...new Set(samples.map(s=>s.phase))].map(phase=>{
    const rows=samples.filter(s=>s.phase===phase);return {phase,clients:[0,1].map(i=>{
      const last=rows.at(-1).clients[i];const d=last.diag;const rtts=[...last.rtts].sort((a,b)=>a-b);return {rttSamples:rtts.length,rttMedianMs:rtts.length?rtts[Math.floor(rtts.length/2)]:null,rttMaxMs:rtts.length?rtts.at(-1):null,frames:d.frames,remoteHoldPercent:100*d.held/d.frames,maxFrameGapMs:d.maxGap,maxCorrectionUnits:d.maxCorrection,maxPendingInputs:Math.max(...rows.map(r=>r.clients[i].pending)),kills:rows.at(-1).clients[i].players.map(p=>p.kills)};
    })};
  });
  writeFileSync(`${dir}/telemetry.json`,JSON.stringify({origin,tunneled,edgeIP,socketURLs,errors,summary,samples},null,2));
  assert.equal(socketURLs.length,2,'two WebSockets connected');
  console.log(JSON.stringify(summary,null,2));assert.deepEqual(errors,[],'browser errors');
  for(const phase of new Set(samples.map(s=>s.phase)))for(const i of [0,1]){
    const rows=samples.filter(s=>s.phase===phase);
    assert.ok(new Set(rows.map(s=>s.clients[i].tick)).size>10,'both clients receive advancing simulation');
    assert.ok(new Set(rows.map(s=>s.clients[i].players[i].x.toFixed(1)+','+s.clients[i].players[i].z.toFixed(1))).size>10,'both tanks move');
  }
  console.log(`PASS: two real clients recorded at ${dir}/two-clients.mp4`);
}finally{
  for(const timer of timers)clearTimeout(timer);
  await context?.close();await browser?.close();tunnel?.kill();server.kill();dns.lookup=systemLookup;
}
