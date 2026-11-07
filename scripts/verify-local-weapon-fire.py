#!/usr/bin/env python3
"""Execute the original local firing path through single-player request dispatch."""
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A5, UC_M68K_REG_A7, UC_M68K_REG_PC
root=Path(__file__).resolve().parent.parent
resources=root/'.local/game-resources/434f4445'
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000,0xf0000)
segments={26:0x10000,27:0x20000,28:0x30000,30:0x40000,36:0x50000}
for segment,address in segments.items():u.mem_write(address,(resources/f'{segment}.bin').read_bytes())
a5,stack,stop=0xa0000,0x90000,0x80000
for offset,address in [(0x10a2,0x30004),(0x1012,0x20288),(0xfca,0x10906),(0x128a,0x50038)]:
    u.mem_write(a5+offset,bytes.fromhex('4ef9')+struct.pack('>I',address))
for offset,address in [(0x1342,0xc0000),(0x1346,0xb0000),(0x1350,0xd0000)]:u.mem_write(a5-offset,struct.pack('>I',address))
u.mem_write(a5-0x12ca,b'\0')
shots=[];sounds=[]
def terminal(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x111a:
        height,kind,weapon,tick,ptr,owner=struct.unpack('>hBxBxIIBx',cpu.mem_read(sp+4,16))
        assert kind==7 and owner==participant and tick==now&0xffffffff
        assert list(struct.unpack('>iii',cpu.mem_read(ptr,12)))==pose
        shots.append([weapon,height]);end=sp+20
    else:
        sounds.append(list(struct.unpack('>hI',cpu.mem_read(sp+4,6))));end=sp+10
    cpu.reg_write(UC_M68K_REG_A7,end);cpu.reg_write(UC_M68K_REG_PC,ret)
for address in [a5+0x111a,a5+0x972]:u.hook_add(UC_HOOK_CODE,terminal,begin=address,end=address)
rng=random.Random(0x3008b2);cases=[]
for iteration in range(2048):
    participant=iteration%12+1;weapon=iteration%18
    now=rng.randint(-2**31,2**31-1);last=rng.randint(-2**31,2**31-1)
    disabled=rng.choice([0,0,0,1,2,255]);cost=rng.choice([-32768,-1,0,1,2,10,32767])
    ammo=rng.choice([cost, max(-32768,cost-1), min(32767,cost+1), -32768,32767])
    health=rng.choice([-32768,-1,0,1,6,32767]);inhibit=rng.choice([0,0,1,2,255])
    height=rng.choice([-2**31,-65537,-65536,-1,0,1,65535,65536,2**31-1])
    pose=[rng.randint(-2**31,2**31-1) for _ in range(3)]
    records=bytearray(rng.randbytes(12*256));off=(participant-1)*256
    struct.pack_into('>h',records,off+0x56,participant)
    struct.pack_into('>hh',records,off+0x5a,health,0);struct.pack_into('>h',records,off+0x5e,ammo)
    geometry=bytearray(rng.randbytes(12*128));off=(participant-1)*128
    struct.pack_into('>iii',geometry,off,*pose);struct.pack_into('>i',geometry,off+0x28,height)
    params=bytearray(rng.randbytes(0x820));struct.pack_into('>h',params,0x7c6+weapon*2,cost)
    for address,data in [(0xc0000,records),(0xb0000,geometry),(0xd0000,params)]:u.mem_write(address,bytes(data))
    for offset,value in [(0x12d2,disabled),(0x133d,participant),(0x2d70,inhibit)]:u.mem_write(a5-offset,bytes([value]))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x1376,struct.pack('>i',last))
    globals_before=bytearray(u.mem_read(a5-0x7000,0x7000));shots.clear();sounds.clear()
    u.mem_write(stack,struct.pack('>IBx',stop,weapon));u.reg_write(UC_M68K_REG_A7,stack);u.reg_write(UC_M68K_REG_A5,a5)
    u.emu_start(0x408b2,stop,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==stop and u.reg_read(UC_M68K_REG_A7)==stack+6
    for address,data in [(0xc0000,records),(0xb0000,geometry),(0xd0000,params)]:assert bytes(u.mem_read(address,len(data)))==data
    after=bytes(u.mem_read(a5-0x7000,0x7000));start=0x7000-0x1376
    globals_before[start:start+4]=after[start:start+4];assert bytes(globals_before)==after
    assert len(shots)<=1 and sounds in [[],[[1,0x30006]]]
    timestamp=struct.unpack('>i',u.mem_read(a5-0x1376,4))[0]
    cases.append(dict(routine='local_weapon_attempt',a=0,b=0,input=dict(now=now,last_attempt=last,disabled=disabled,weapon=weapon,
        ammo=ammo,cost=cost,health=health,inhibit=inhibit,height=height),result=[timestamp,shots[0] if shots else None,bool(sounds)],expected=0))
output=root/'.local/disassembly/local-weapon-fire-vectors.json'
output.write_text(json.dumps(dict(method='Complete original local fire, eligibility, packet creation and single-player dispatch; only constructor and sound requests captured',
    hashes={f'CODE{s}':hashlib.sha256((resources/f'{s}.bin').read_bytes()).hexdigest() for s in segments},movement_marker_table=[0]*360,cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
print('2048 complete original local-fire dispatches; constructor arguments, sounds, timestamp, records, globals and stack checked')
