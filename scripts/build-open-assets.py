#!/usr/bin/env python3
"""Build Signal Frontier from authored parameters, primitives, and synthesized sound.

This compiler never reads .local, an original disc, or an extracted resource.
The output uses the engine's resource format, not original asset contents.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import wave

ROOT = Path(__file__).resolve().parents[1]
U = lambda n: struct.pack('>H', n & 65535)
I = lambda n: struct.pack('>i', int(n))
words = lambda values: b''.join(U(n) for n in values)


def archive(resources):
    """Serialize a named resource fork; IDs are engine interface identifiers."""
    payload, names = bytearray(), bytearray()
    groups = {}
    for kind, ident, name, data in resources:
        offset = len(payload)
        payload += I(len(data)) + data
        name_offset = len(names)
        encoded = name.encode('ascii')[:63]
        names += bytes([len(encoded)]) + encoded
        groups.setdefault(kind, []).append((ident, name_offset, offset))
    types = bytearray(U(len(groups)-1))
    refs = bytearray()
    for kind, rows in groups.items():
        types += kind.encode('ascii') + U(len(rows)-1) + U(2+8*len(groups)+len(refs))
        for ident, name, offset in rows:
            refs += U(ident) + U(name) + I(offset) + I(0)
    header = b''.join(I(n) for n in [256, 256+len(payload), len(payload), 28+len(types)+len(refs)+len(names)])
    resource_map = header + bytes(8) + U(28) + U(28+len(types)+len(refs)) + types + refs + names
    return header + bytes(240) + payload + resource_map


class Mesh:
    def __init__(self):
        self.vertices, self.faces = [], []

    def part(self, vertices, faces, color=0):
        start = len(self.vertices)
        self.vertices.extend(vertices)
        self.faces.extend(([start+i for i in face], color) for face in faces)

    def box(self, x, y, z, center=(0, 0, 0), color=0):
        cx, cy, cz = center
        self.part([(cx+a, cy+b, cz+c) for a,b,c in [(-x,0,-z),(x,0,-z),(x,0,z),(-x,0,z),(-x,y,-z),(x,y,-z),(x,y,z),(-x,y,z)]],
                  [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]],color)

    def prism(self, radius, height, sides=6, color=0, tip=False):
        base=[(round(math.cos(i*math.tau/sides)*radius,3),0,round(math.sin(i*math.tau/sides)*radius,3)) for i in range(sides)]
        if tip:
            self.part(base+[(0,height,0)], [list(reversed(range(sides)))]+[[i,(i+1)%sides,sides] for i in range(sides)], color)
        else:
            self.part(base+[(x,height,z) for x,_,z in base], [list(reversed(range(sides))),list(range(sides,sides*2))]+[[i,(i+1)%sides,(i+1)%sides+sides,i+sides] for i in range(sides)],color)

    def encode(self, top):
        def vertex_block(points):
            return U(len(points)-1)+b''.join(I(round(c*65536)) for v in points for c in v)
        data=vertex_block(self.vertices)+U(len(self.faces)-1)
        for face,color in self.faces:
            data += U(len(face)-1)+words(i+1 for i in face)+U(color)
        points=[(x,0,z) for x,z in top]
        data += vertex_block(points)+U(len(points)-1)
        data += b''.join(words([i+1,(i+1)%len(points)+1]) for i in range(len(points)))
        return data+U(len(points))


def model(spec, phase=0):
    x,y,z=spec['size']; family=spec['family']; m=Mesh()
    top=[(-x,-z),(x,-z),(x,z),(-x,z)]
    if family=='tank':
        # Narrow nose, separated track pods, low hexagonal turret and long barrel.
        m.box(x*.72,y*.45,z*.8)
        for side in [-1,1]: m.box(x*.24,y*.38,z,(side*x*.86,0,0),1)
        m.box(x*.48,y*.44,z*.42,(0,y*.45,-z*.15),0)
        m.box(1.7,2.4,z*.8,(0,y*.62,z*.7),1)
        top=[(-x,-z),(x,-z),(x,z*.8),(2,z*1.5),(-2,z*1.5),(-x,z*.8)]
    elif family=='crystal':
        m.prism(x,y,5 if spec['kind']==31 else 6,tip=True)
        m.box(x*.5,3,z*.5,(0,2+phase*5,0),1)
    elif family=='rocket':
        m.box(x,y,z*.6)
        m.part([(-x,0,z*.6),(x,0,z*.6),(0,y,z*.6),(0,y/2,z*1.4)],[[0,1,3],[1,2,3],[2,0,3]],1)
        top=[(-x,-z),(x,-z),(0,z*1.4)]
    elif family=='beacon':
        m.prism(x,4,6)
        m.box(1.3,y,1.3)
        m.part([(0,y-12,0),(12,y-9,0),(0,y,0)],[[0,1,2],[2,1,0]],1)
        top=[(-4,-4),(4,-4),(4,4),(-4,4)]
    elif family=='ring':
        for i in range(8):
            a=i*math.tau/8
            m.box(3,y,3,(math.cos(a)*x,0,math.sin(a)*z),i%2)
        top=[(math.cos(i*math.tau/8)*x,math.sin(i*math.tau/8)*z) for i in range(8)]
    elif family=='pad':
        m.box(x,y,z)
        # An arrow within the surface makes flow direction readable.
        m.part([(-x*.45,y+.2,-z*.3),(x*.45,y+.2,-z*.3),(0,y+.2,z*.65)],[[0,2,1]],1)
    elif family=='tower':
        m.prism(x,y*.8,6)
        m.prism(x*.65,y,4,1,True)
        top=[(math.cos(i*math.tau/6)*x,math.sin(i*math.tau/6)*z) for i in range(6)]
    else:
        m.box(x,y*.7,z)
        m.box(x*.8,y*.3,z*.8,(0,y*.7,0),1)
    return m.encode(top)


def palette(colors):
    return I(0)+U(0)+U(len(colors)-1)+b''.join(U(i)+words(int(c[j:j+2],16)*257 for j in [0,2,4]) for i,c in enumerate(colors))


def radar():
    # Forty-one newly drawn four-by-five symbols, with orientation arrows first.
    patterns=['0100/1110/0100/0100/0000','0000/0110/1111/0110/0000','0100/0100/0100/1110/0100','0000/1100/1111/1100/0000',
              '0110/1001/1001/1001/0110','0000/0100/1110/0100/0000','1110/1010/1110/0000/0000','0100/1110/1110/0100/0000']
    glyphs=[patterns[i%len(patterns)].split('/') for i in range(41)]
    glyphs[40]='0100/0111/0101/0100/0100'.split('/')
    frame=words([0,0,5,164]); bounds=words([0,0,5,176])
    header=bytearray(52);header[2:10]=frame;header[10:13]=bytes([17,1,1]);header[13:15]=U(10)
    header[15:23]=frame;header[23]=0x98;header[24:26]=U(22);header[26:34]=bounds;header[34:42]=frame;header[42:50]=frame
    rows=bytearray()
    for y in range(5):
        row=bytearray(22)
        for i,glyph in enumerate(glyphs):
            for x,ch in enumerate(glyph[y]):
                if ch=='1':
                    bit=i*4+x;row[bit//8]|=128>>(bit%8)
        rows+=bytes([23,21])+row
    result=header+rows+b'\xff';result[:2]=U(len(result));return result


def color_bindings():
    kinds=[7,6,5,4,9,5,4,3,7,10,9,10,2,6,12,5,8,4,6,9,7,6,3,8,9,9,3,7,10,7,6,6,9,10,7,3,10,7,4,4,9,2,10,8,3,3,9]
    data=U(0)+bytes([4,4,3,3,5,5,7,7])
    data+=bytes(c for color in kinds for c in [color,color])
    data+=bytes(c for color in [5,8,4,2,9,11,4,10,2,6,7,8] for c in [color,0])
    assert len(data)==128
    return data


def sound(ident, output):
    rng=random.Random(ident); rate=22050
    event=ident-10000
    explosions={3,4,5,13,24,35,37,45}; chimes={6,7,20,21,23,38,42,44,47}
    duration=.65 if event in explosions else .38 if event in chimes else .17
    phase=0.; frames=[]; filtered=0.
    for i in range(int(rate*duration)):
        t=i/rate; p=t/duration
        if event in explosions:
            filtered=filtered*.72+rng.uniform(-1,1)*.28
            signal=.7*filtered+.25*math.sin(math.tau*(70-35*p)*t)
        elif event in chimes:
            note=[1,1.25,1.5][min(2,int(p*3))]
            signal=.3*math.sin(math.tau*(440+event*5)*note*t)+.1*math.sin(math.tau*1320*t)
        else:
            freq=(900+event*17)*(1-p)**2+65
            phase+=math.tau*freq/rate
            signal=.35*math.sin(phase)+.18*rng.uniform(-1,1)*(1-p)
        envelope=min(1,t/.004)*(1-p)**2
        frames.append(struct.pack('<h',int(max(-1,min(1,signal*envelope))*28000)))
    with wave.open(str(output),'wb') as wav:
        wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(rate);wav.writeframes(b''.join(frames))


def build(spec, out):
    out.mkdir(parents=True,exist_ok=True);(out/'audio').mkdir(exist_ok=True)
    shared=[('cob8',999,'Signal Frontier colors',color_bindings()),('PICT',1000,'Signal Frontier radar',radar())]
    (out/'spectre.bin').write_bytes(archive(shared))
    for zone in spec['zones']:
        resources=[('opsp',1,'Signal Frontier',b'OpenSpectre replacement pack v1')]
        def add(kind,ident,name,data):resources.append((kind,ident,name,data))
        choices=[]
        for m in spec['models']:
            kind=m['kind'];ident=10200+kind
            if kind==6: ident=10010
            if kind==7: ident=10011
            add('shap',ident,m['name'],model(m))
            choices.append(0 if kind==14 else ident)
        # Original ABI asks for these dependency IDs; all geometry here is new.
        for ident in [10099,10100]: add('shap',ident,'Switchgate segment',model(spec['models'][19]))
        for phase in range(4):
            m=dict(spec['models'][31]);m['size']=[18-phase*2,40-phase*6,18-phase*2]
            add('shap',10310+phase,f'Aegis stage {phase+1}',model(m,phase))
        add('ashp',3002,'Aegis opening',words([4,10310,10311,10312,10313]));choices[31]=3002
        add('shps',10000,'Signal Frontier models',U(len(choices))+words(n for ident in choices for n in [ident]*3))
        add('clut',1000,zone['name'],palette(zone['palette']))
        for ident,index in [(2000,3),(2001,2)]:add('clut',ident,'Level accents',palette([zone['palette'][index]]*30))
        p=spec['enemy_parameters'];params=[]
        for kind in [7,8,13,20,22,23,24,26,27,31,35,45]:
            health=6 if kind==31 else p['health'];speed=0 if kind==31 else p['radial_velocity']
            vals=[kind,health,p['collision_damage'],p['attack_range'],p['short_firing_range'],p['firing_arc'],p['firing_interval'],speed,p['angular_velocity']]
            params.append(words(vals)+I(p['score'])+I(p['bonus'])+U(0))
        # Supply allocation weights occupy type 5's parameter fields.
        params.append(words([5,4,3,2,2,0,0,0,0])+I(0)+I(0)+U(0))
        add('bval',10000,'Signal Frontier balance',U(len(params))+b''.join(params))
        add('wval',10000,'Signal Frontier weapons',U(len(spec['weapons']))+b''.join(words(row) for row in spec['weapons']))
        waves=[]
        for index,arena in enumerate(zone['arenas']):
            ident=10000+index
            # Six reserved flag sites stay clear of the authored cover layout.
            objects=[[14,x,z,0] for x,z in spec['flag_sites']]+arena['objects']
            add('wrld',ident,arena['name'],U(len(objects))+b''.join(words([kind,z,x,heading]) for kind,x,z,heading in objects))
            enemies=[[7,2,2,0]] if index==0 else [[7,2,2,0],[8,1,1,0]] if index<3 else [[7,2,2,0],[31,1,1,0]]
            add('enem',ident,arena['name']+' patrol',U(len(enemies))+b''.join(words(row) for row in enemies))
            waves.append([ident,ident,[15,10,11,12][index],10000,0,0,0,1000])
        for ident in [10000,10001,10002]:
            add('wave',ident,'Signal Frontier campaign',U(len(waves))+b''.join(words(row) for row in waves))
            add('sstf',ident,'Signal Frontier score',bytes(14)+b''.join(I(2500*i) for i in range(1,11))+U(5000))
        (out/(zone['key']+'.bin')).write_bytes(archive(resources))
    for ident in spec['sound_ids']:sound(ident,out/'audio'/f'{ident}.wav')
    files={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.rglob('*')) if p.is_file() and p.name!='manifest.json'}
    manifest={'name':spec['name'],'version':spec['version'],'origin':'Authored primitives, arena coordinates, balance values, radar drawings, and synthesized PCM. No disc inputs.',
              'campaigns':[{'id':z['key'],'name':z['name'],'arenas':[a['name'] for a in z['arenas']]} for z in spec['zones']],
              'models_per_campaign':len(spec['models'])+6,'sound_count':len(spec['sound_ids']),'files':files}
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f"Built {manifest['name']}: {sum(len(z['arenas']) for z in spec['zones'])} arenas, {manifest['models_per_campaign']} models per campaign, {len(spec['sound_ids'])} sounds")


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,default=ROOT/'assets/open/generated')
    args=parser.parse_args()
    build(json.loads((ROOT/'assets/open/pack.json').read_text()),args.output)
