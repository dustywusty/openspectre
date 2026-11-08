// Record the real WASM framebuffer. Simulation remains fixed at 60 Hz.
import {mkdirSync, writeFileSync, rmSync, statSync} from 'node:fs';
import {spawn, spawnSync} from 'node:child_process';
import {once} from 'node:events';
import {join} from 'node:path';

export function recorder(enabled, directory='artifacts/playthrough') {
  let encoder, completion, tick=0, caption='', recordingName='';
  if(enabled) {
    const check=spawnSync('ffmpeg',['-version']);
    if(check.status!==0)throw Error('Recording requires ffmpeg on PATH.');
    mkdirSync(directory,{recursive:true});
    for(const file of ['01-combat.mp4','02-death.mp4','03-beacon-mission.mp4','01-combat.gif','02-death.gif','03-beacon-mission.gif','cockpit.png','chase.png','overhead.png'])rmSync(join(directory,file),{force:true});
    writeFileSync(join(directory,'README.txt'),'Actual WASM framebuffer, 800x600, 30 fps at real simulation speed.\nSilent recordings; browser HTML HUD and audio are not captured.\n01: Cannon combat, normal health.\n02: Enemy damage and death, normal health.\n03: Beacon mission, INFINITE HEALTH enabled for navigation testing.\nScreenshots show the three camera modes at the opening spawn.\n');
  }
  const pixels=(w,p)=>Buffer.from(new Uint8Array(w.memory.buffer,p,800*600*4));
  // Small built-in bitmap alphabet: no platform font or FFmpeg drawtext dependency.
  const alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZ';
  const glyphs=['0E11111F111111','1E11111E11111E','0F10101010100F','1E11111111111E','1F10101E10101F','1F10101E101010','0F10101711110F','1111111F111111','0E04040404040E','0702020212120C','11121418141211','1010101010101F','111B1515111111','11191513111111','0E11111111110E','1E11111E101010','0E11111115120D','1E11111E141211','0F10100E01011E','1F040404040404','1111111111110E','11111111110A04','11111115151B11','11110A040A1111','11110A04040404','1F01020408101F'];
  const labeled=(w,p)=>{
    const data=pixels(w,p);
    for(let y=570;y<600;y++)for(let x=0;x<800;x++){const i=(y*800+x)*4;data[i]=data[i+1]=data[i+2]=0;data[i+3]=255;}
    for(let n=0;n<caption.length;n++){
      const glyph=glyphs[alphabet.indexOf(caption[n])];if(!glyph)continue;
      for(let y=0;y<7;y++)for(let x=0;x<5;x++)if(parseInt(glyph.slice(y*2,y*2+2),16)&(1<<(4-x)))
        for(let dy=0;dy<2;dy++)for(let dx=0;dx<2;dx++){const i=((578+y*2+dy)*800+12+n*12+x*2+dx)*4;data[i]=data[i+1]=data[i+2]=255;}
    }
    return data;
  };
  return {
    screenshot(w,p,name) {
      if(!enabled)return;
      const result=spawnSync('ffmpeg',['-loglevel','error','-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-i','pipe:0','-frames:v','1',join(directory,`${name}.png`)],{input:pixels(w,p)});
      if(result.status!==0)throw Error(result.stderr?.toString()||'Screenshot encoder failed');
    },
    async start(name,label) {
      if(!enabled)return;
      tick=0;caption=label.toUpperCase();recordingName=name;
      encoder=spawn('ffmpeg',['-loglevel','error','-y','-f','rawvideo','-pixel_format','rgba','-video_size','800x600','-framerate','30','-i','pipe:0','-an','-c:v','libx264','-preset','fast','-crf','23','-pix_fmt','yuv420p','-movflags','+faststart',join(directory,`${name}.mp4`)],{stdio:['pipe','ignore','inherit']});
      completion=new Promise((resolve,reject)=>{
        encoder.once('error',reject);
        encoder.once('close',code=>code===0?resolve():reject(Error(`Video encoder exited ${code}`)));
      });
      // Attach a handler immediately, including when failure precedes the first frame.
      completion.catch(()=>{});
      encoder.stdin.on('error',()=>{});
    },
    async frame(w,p) {
      if(!enabled||tick++%2)return;
      // Copy before the next WASM call can mutate or grow memory. Respect pipe backpressure.
      if(!encoder.stdin.write(labeled(w,p)))await Promise.race([once(encoder.stdin,'drain'),completion.then(()=>{throw Error('Encoder stopped before recording finished');})]);
    },
    async stop() {
      if(!enabled||!encoder)return;
      encoder.stdin.end();
      try{await completion;}finally{encoder=undefined;}
      // GIF previews render inline in PR comments without an authenticated artifact download.
      const preview=join(directory,`${recordingName}.gif`);
      for(const [width,fps] of [[640,10],[480,6]]) {
        const filter=`fps=${fps},scale=${width}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=3`;
        const result=spawnSync('ffmpeg',['-loglevel','error','-y','-i',join(directory,`${recordingName}.mp4`),'-filter_complex',filter,'-loop','0',preview]);
        if(result.status!==0)throw Error('Inline preview encoder failed');
        if(statSync(preview).size<8*1024*1024)return;
      }
      throw Error('Inline preview exceeds the 8 MB publication limit');
    }
  };
}
