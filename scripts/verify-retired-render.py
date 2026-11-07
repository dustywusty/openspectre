#!/usr/bin/env python3
"""Trace original kind-10 render preparation through its world-model exclusion."""
import hashlib, json, random, struct, subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A4, UC_M68K_REG_A5, UC_M68K_REG_A6, UC_M68K_REG_A7, UC_M68K_REG_D4, UC_M68K_REG_D5, UC_M68K_REG_D7, UC_M68K_REG_PC
root=Path(__file__).resolve().parent.parent
code=(root/'.local/game-resources/434f4445/6.bin').read_bytes()
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000);u.mem_map(0x10000,0x70000);u.mem_write(0x10000,code)
a5,frame,stack,record_address=0x60000,0x50000,0x40000,0x70000
calls=[];visited=[]
def trace(cpu,address,size,data):
    if address in [0x126d0,0x1288c,0x129e6]: visited.append(address)
    if address==0x117f6:
        sp=cpu.reg_read(UC_M68K_REG_A7)
        calls.append(list(struct.unpack('>hBxhhh',cpu.mem_read(sp+4,10))))
        ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
        cpu.reg_write(UC_M68K_REG_A7,sp+14);cpu.reg_write(UC_M68K_REG_PC,ret)
u.hook_add(UC_HOOK_CODE,trace)
rng=random.Random(0x6288c);results=[];cases=[]
for i in range(512):
    record=bytearray(rng.randbytes(128));record[0x14]=10;record[0x15]=rng.randrange(48);record[0x1a]=1
    now=rng.randrange(-1000,1000);deadline=now+rng.randrange(-10,70)
    struct.pack_into('>i',record,0x16,deadline)
    u.mem_write(record_address,bytes(record));u.mem_write(a5-0x134c,struct.pack('>i',now))
    body=i%13;radar=(i//13)%14;view=[-32768,-1,0,1,2,3,32767][i%7]
    u.mem_write(a5-0x2742,bytes([body]*48));u.mem_write(a5-0x3b30,bytes([radar]*48));u.mem_write(a5-0x25ee,bytes([2]*48))
    u.mem_write(a5-0x7be2,bytes([i%2]));u.mem_write(a5-0x7bf0,bytes([0]));u.mem_write(a5-0x12ca,bytes([0]))
    locals_before=bytes(0x1b00);u.mem_write(frame-0x1b00,locals_before)
    u.mem_write(frame+8,struct.pack('>h',view));u.mem_write(frame-0xf,bytes([int(radar!=13)]))
    u.mem_write(frame-0x40,struct.pack('>h',7))
    for reg,value in [(UC_M68K_REG_A4,record_address),(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,frame),(UC_M68K_REG_A7,stack),(UC_M68K_REG_D4,1),(UC_M68K_REG_D5,0),(UC_M68K_REG_D7,200)]:u.reg_write(reg,value)
    calls.clear();visited.clear()
    u.emu_start(0x11bc4,0x12a52,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x12a52 and u.reg_read(UC_M68K_REG_A7)==stack
    assert visited==[0x126d0,0x1288c],visited
    assert bytes(u.mem_read(record_address,128))==record
    assert struct.unpack('>h',u.mem_read(frame-0x40,2))[0]==7
    assert bytes(u.mem_read(frame-0x1900,0x1800))==bytes(0x1800)
    assert len(calls)<=1
    cases.append(dict(routine='retired_radar_glyph',a=view,b=body,color=radar,result=calls[0][:2] if calls else None,expected=0))
    results.append(dict(now=now,deadline=deadline,previous_kind=record[0x15],effect_flag=i%2,view=view,radar_calls=calls.copy(),world_model_queued=False))
p=root/'.local/disassembly/retired-render-trace.json'
p.write_text(json.dumps(dict(sha256=hashlib.sha256(code).hexdigest(),method='Original kind-10 dispatch, preparation, and exclusion; radar drawing captured; original min/max execute; projection inputs supplied',cases=results))+'\n')
print('512 original retired-render traces: kind-10 branch reached, radar calls captured, no world model queued; record and stack preserved')
output=root/'.local/disassembly/retired-radar-vectors.json'
output.write_text(json.dumps(dict(method='Original retired-object render branch, radar terminal captured; admission byte supplied from original color comparison',movement_marker_table=[0]*360,cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
