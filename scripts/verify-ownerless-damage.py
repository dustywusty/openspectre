#!/usr/bin/env python3
"""Execute full single-player damage handler for a nonlocal enemy and attacker zero."""
import hashlib,json,random,struct,subprocess
from pathlib import Path
from unicorn import Uc,UC_ARCH_M68K,UC_MODE_BIG_ENDIAN,UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000,UC_M68K_REG_A5,UC_M68K_REG_A7,UC_M68K_REG_PC
root=Path(__file__).resolve().parent.parent
resources=root/'.local/game-resources/434f4445'
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000);u.mem_map(0x10000,0xf0000)
segments={31:0x10000,6:0x30000}
for segment,address in segments.items():u.mem_write(address,(resources/f'{segment}.bin').read_bytes())
a5,stack,stop=0xa0000,0x90000,0x80000
u.mem_write(a5+0x4e2,bytes.fromhex('4ef9')+struct.pack('>I',0x3197e))
for offset,address in [(-0x1346,0xb0000),(-0x1342,0xc0000),(-0x1350,0xd0000)]:u.mem_write(a5+offset,struct.pack('>I',address))
u.mem_write(a5-0x133d,b'\x01');u.mem_write(a5-0x12ca,b'\0');u.mem_write(a5-0x12d2,b'\0')
calls=[]
def service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x972:
        channel,request=struct.unpack('>hI',cpu.mem_read(sp+4,6));calls.append(['sound',channel,request]);size=6
    else:
        owner,obj=struct.unpack('>bxh',cpu.mem_read(sp+4,4));calls.append(['burst' if address==0x108c6 else 'fragments',owner,obj]);size=4
    cpu.reg_write(UC_M68K_REG_A7,sp+4+size);cpu.reg_write(UC_M68K_REG_PC,ret)
for address in [a5+0x972,0x108c6,0x10776]:u.hook_add(UC_HOOK_CODE,service,begin=address,end=address)
rng=random.Random(0x31a84);cases=[]
for iteration in range(1024):
    record=bytearray(rng.randbytes(128));participant=bytearray(rng.randbytes(256))
    participant[5]=rng.choice([1,2,3,128,255]);participant[8]=rng.choice([7,8,31,45]);record[0x14]=participant[8]
    struct.pack_into('>h',participant,0x56,1)
    health=rng.choice([-32768,-1,0,1,3,10,32767]);damage=rng.choice([-32768,-1,0,1,3,32767]);struct.pack_into('>h',participant,0x5a,health)
    phase=rng.choice([[5,1,5],[4,1,5],[-1,-3,3],[127,127,1]])
    for offset,value in zip([-0x25cf,-0x25ff,-0x262f],phase):u.mem_write(a5+offset,bytes([value&255]))
    now=rng.choice([-2147483648,0,100,2147483647]);level=rng.choice([0,19,20,27,28,32767])
    u.mem_write(a5-0x136e,struct.pack('>h',level));u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.mem_write(a5-0x3d18,struct.pack('>i',123));u.mem_write(a5-0x3d00,struct.pack('>i',456))
    params=rng.randbytes(0x900);u.mem_write(0xd0000,params)
    u.mem_write(0xb0000,bytes(record));u.mem_write(0xc0100,bytes(participant))
    u.mem_write(stack,struct.pack('>IhhiBxBx',stop,damage,0,now,0,2));calls.clear()
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack);u.emu_start(0x10a84,stop,count=100000)
    assert u.reg_read(UC_M68K_REG_PC)==stop and u.reg_read(UC_M68K_REG_A7)==stack+16
    assert bytes(u.mem_read(0xd0000,len(params)))==params,'ownerless hit must not update score or combat counters'
    assert struct.unpack('>i',u.mem_read(a5-0x3d18,4))[0]==0
    assert struct.unpack('>i',u.mem_read(a5-0x3d00,4))[0]==now
    cases.append(dict(routine='ownerless_enemy_damage',a=damage,b=now,level=level,phase=phase,record=list(record),participant=list(participant),after_record=list(u.mem_read(0xb0000,128)),after_participant=list(u.mem_read(0xc0100,256)),calls=calls.copy(),expected=0))
output=root/'.local/disassembly/ownerless-damage-vectors.json'
output.write_text(json.dumps(dict(method='Complete original CODE 31 damage handler, attacker zero, nonlocal enemy, single-player, concurrent movie flag zero; terminal sound/burst/fragment calls captured',hashes={str(k):hashlib.sha256((resources/f'{k}.bin').read_bytes()).hexdigest() for k in segments},movement_marker_table=[0]*360,cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
print('1024 full ownerless enemy damage calls match composed Rust helpers; health guard, response fields, effects order, score/counter preservation and globals checked')
