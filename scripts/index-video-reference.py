#!/usr/bin/env python3
"""Build timestamped reference sheets and an offline review page from the local video."""
import hashlib
import html
import json
import math
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
folder = root / '.local/reference-video'
video = folder / 'full-gameplay.mp4'
info = json.loads((folder / 'full-gameplay.info.json').read_text())
probe = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-select_streams', 'v:0',
    '-show_entries', 'stream=r_frame_rate', '-of', 'json', str(video)]))
assert probe['streams'][0]['r_frame_rate'] == '30/1', 'Sampling assumes the verified 30 fps source'
jobs = [(f'overview-{i+1:02}', i*360, 30, min(12, math.ceil((info['duration']-i*360)/30))) for i in range(4)]
jobs += [('opening',0,2,15),('first-transition',45,2,15),('mid-combat',440,2,12),
         ('level11-intro',820,2,12),('respawns',875,2,15),('ending',1250,4,11),
         ('refill-detail',458,8/30,12),('empty-ammo-detail',1262,1,12)]
manifest = {'source':info['webpage_url'], 'title':info['title'], 'duration_seconds':info['duration'],
    'sha256':hashlib.sha256(video.read_bytes()).hexdigest(), 'description':info.get('description',''),
    'review_method':'Timestamped still samples across the full run, with denser event sequences; not continuous playback.', 'sheets':[]}
for name,start,step,count in jobs:
    frames = round(step*30)
    subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-ss',str(start),'-i',str(video),
        '-vf',f"select='not(mod(n,{frames}))',scale=400:300,tile=3x{math.ceil(count/3)}:nb_frames={count}",
        '-frames:v','1','-q:v','2',str(folder/f'{name}.jpg')],check=True)
    manifest['sheets'].append({'name':name,'file':f'{name}.jpg',
        'timestamps_seconds':[round(start+i*frames/30,6) for i in range(count)]})
(folder/'review-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
def stamp(t):
    return f'{int(t)//60:02}:{t%60:05.2f}'
sections=[]
for sheet in manifest['sheets']:
    buttons=' '.join(f'<button data-time="{t}">{stamp(t)}</button>' for t in sheet['timestamps_seconds'])
    sections.append(f'<section><h2>{html.escape(sheet["name"])}</h2><p>Frames run left to right, then down.</p><div>{buttons}</div><img loading="lazy" src="{sheet["file"]}" alt="{html.escape(sheet["name"])} reference frames"></section>')
(folder/'full-index.html').write_text('''<!doctype html><meta charset="utf-8"><title>Spectre full video reference</title>
<style>body{background:#090e12;color:#e6eeea;font:16px system-ui;max-width:1200px;margin:24px auto;padding:16px}video{width:640px;max-width:100%}img{width:100%}button{margin:4px;padding:8px;background:#24352f;color:white;border:1px solid #65796f;cursor:pointer}section{margin-top:40px}a{color:#b4edb2}</style>
<h1>Spectre VR — full reference recording</h1><p>21:34. Sampled across the entire run with denser event sequences. QuickTime sequences were omitted by the uploader.</p>
<p><a href="index.html">Earlier 4:00–5:00 review</a> · <a href="review-manifest.json">Timestamp manifest</a></p>
<video id="video" controls preload="metadata" src="full-gameplay.mp4"></video><p>Click a timestamp to seek and play that sequence.</p>
'''+'\n'.join(sections)+'''<script>document.addEventListener('click',e=>{const b=e.target.closest('[data-time]');if(!b)return;const v=document.querySelector('video');v.currentTime=Number(b.dataset.time);v.scrollIntoView({block:'center'});v.play().catch(()=>{});});</script>''')
print(f'Indexed {sum(len(s["timestamps_seconds"]) for s in manifest["sheets"])} timestamped frames in {len(jobs)} sheets; no server required.')
