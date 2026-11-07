#!/usr/bin/env python3
"""Execute original retirement scans through the real area-effect selector."""
import hashlib,json,random,struct,subprocess,sys
from pathlib import Path
from unicorn import Uc,UC_ARCH_M68K,UC_MODE_BIG_ENDIAN,UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000,UC_M68K_REG_A5,UC_M68K_REG_A7,UC_M68K_REG_A4,UC_M68K_REG_A3,UC_M68K_REG_A6,UC_M68K_REG_D5,UC_M68K_REG_PC
impact='--impact' in sys.argv
root=Path(__file__).resolve().parent.parent
resources=root/'.local/game-resources/434f4445'
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000);u.mem_map(0x10000,0xf0000)
segments={30:0x10000,36:0x20000,6:0x30000}
for segment,address in segments.items():u.mem_write(address,(resources/f'{segment}.bin').read_bytes())
a5,stack,stop=0xa0000,0x90000,0x80000
for offset,address in [(0x129a,0x200ec),(0x12a2,0x2012e),(0x128a,0x20038),(0x4d2,0x317c6)]:
    u.mem_write(a5+offset,bytes.fromhex('4ef9')+struct.pack('>I',address))
for offset,address in [(-0x1346,0xb0000),(-0x1342,0xc0000),(-0x1350,0xd0000)]:u.mem_write(a5+offset,struct.pack('>I',address))
u.mem_write(a5-0x12c8,struct.pack('>h',0));u.mem_write(a5-0x1348,struct.pack('>h',1))
requests=[];calls=[]
def service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x119a:
        damage,mode,now,attacker,victim=struct.unpack('>hhibxbx',cpu.mem_read(sp+4,12))
        requests.append(dict(damage=damage,mode=mode,now=now,attacker=attacker,victim=victim));calls.append('Damage');size=12
    elif address==0x12700:calls.append('Move');size=6
    else:calls.append('Delete');size=2
    cpu.reg_write(UC_M68K_REG_A7,sp+4+size);cpu.reg_write(UC_M68K_REG_PC,ret)
for address in [a5+0x119a,0x12700,a5+0xe62]:u.hook_add(UC_HOOK_CODE,service,begin=address,end=address)
rng=random.Random(0x2bbe2aa6);cases=[]
for iteration in range(512):
    records=[bytearray(128) for _ in range(13)];participants=[bytearray(256) for _ in range(12)]
    source=records[0];source[0x14]=[9,46][iteration%2];source[0x1a]=rng.randrange(1,13)
    struct.pack_into('>h',source,0x1c,[1,17][iteration%2]);struct.pack_into('>i',source,0x16,100)
    struct.pack_into('>i',source,0x28,rng.choice([0,20<<16,100<<16]))
    counts=[rng.randrange(1,21) for _ in range(12)]
    for i,(target,participant) in enumerate(zip(records[1:],participants)):
        target[0x14]=rng.choice([7,8,10,31]);target[0x1d]=rng.randrange(18)
        struct.pack_into('>ii',target,0,rng.choice([0,59,60,119,120,239,240])<<16,0)
        struct.pack_into('>i',target,0x28,rng.choice([-65536,0,65536,100<<16]))
        participant[0x88]=rng.choice([0,1,2,255]);participant[5]=rng.choice([0,1,2,255])
        struct.pack_into('>hh',participant,0x56,i+2,counts[i])
    damage=[rng.choice([-32768,-1,0,1,3,20000,32767]) for _ in range(18)]
    initial=[list(r) for r in records];initial_participants=[list(p) for p in participants]
    u.mem_write(0xb0000,b''.join(bytes(r) for r in records));u.mem_write(0xc0000,b''.join(bytes(p) for p in participants))
    u.mem_write(0xd075a,struct.pack('>18h',*damage));steps=[]
    if impact:
        now=100;requests.clear();calls.clear()
        u.mem_write(a5-0x134c,struct.pack('>i',now));game_mode=[0,1,4,12,255][iteration%5];authority=[0,1,2,3,128,255][iteration%6]
        u.mem_write(a5-0x12ca,bytes([game_mode]));u.mem_write(a5-0x4f4f,bytes([authority]))
        u.mem_write(0xe0005,b'\x01')
        for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A7,stack),(UC_M68K_REG_A4,0xb0000),
                          (UC_M68K_REG_A3,0xe0000),(UC_M68K_REG_A6,0x98000),(UC_M68K_REG_D5,1)]:u.reg_write(reg,value)
        u.emu_start(0x1294a,0x12a26,count=100000)
        assert u.reg_read(UC_M68K_REG_PC)==0x12a26 and u.reg_read(UC_M68K_REG_A7)==stack
        assert calls==['Damage']*len(requests)
        assert all(request['attacker']==source[0x1a] for request in requests)
        assert bytes(u.mem_read(0xb0000,13*128))==b''.join(bytes(r) for r in records)
        assert bytes(u.mem_read(0xc0000,12*256))==b''.join(bytes(p) for p in participants)
        cases.append(dict(routine='projectile_impact_area_requests',a=now,b=1,expected=0,records=initial,game_mode=game_mode,authority=authority,
                          participants=initial_participants,damage=damage,requests=requests.copy()))
        continue
    times=[99,100,101,102,103,104] if iteration%2==0 else [99,100,102,104]
    for now in times:
        before=bytes(u.mem_read(0xb0000,128));requests.clear();calls.clear()
        u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(stack,struct.pack('>Ii',stop,now))
        u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack);u.emu_start(0x12bbe,stop,count=100000)
        assert u.reg_read(UC_M68K_REG_PC)==stop and u.reg_read(UC_M68K_REG_A7)==stack+8
        after=bytes(u.mem_read(0xb0000,128))
        action='Move' if calls==['Move'] else 'AreaEffectThenDelete' if calls and calls[-1]=='Delete' else 'Retire' if before[0x14]!=10 and after[0x14]==10 else 'None'
        if action=='AreaEffectThenDelete':assert calls==['Damage']*len(requests)+['Delete']
        after_counts=[struct.unpack('>h',u.mem_read(0xc0000+i*256+0x58,2))[0] for i in range(12)]
        steps.append(dict(now=now,action=action,requests=requests.copy(),counts=after_counts,source=list(after)))
        assert bytes(u.mem_read(0xb0080,12*128))==b''.join(bytes(r) for r in records[1:])
        expected_participants=bytearray(b''.join(bytes(p) for p in participants))
        for i,count in enumerate(after_counts):struct.pack_into('>h',expected_participants,i*256+0x58,count)
        assert bytes(u.mem_read(0xc0000,12*256))==expected_participants
    assert steps[-1]['action']=='AreaEffectThenDelete'
    assert all(request['attacker']==0 for request in steps[-1]['requests'])
    cases.append(dict(routine='projectile_lifecycle_sequence',a=0,b=0,expected=0,records=initial,participants=initial_participants,counts=counts,damage=damage,steps=steps))
output=root/('.local/disassembly/blast-impact-vectors.json' if impact else '.local/disassembly/blast-sequence-vectors.json')
output.write_text(json.dumps(dict(method=('Original enemy-impact special-weapon dispatch composed with original area selector; terminal damage captured' if impact else 'Original complete single-record expiry scan composed with original area selector; movement, deletion, and terminal damage captured'),hashes={str(k):hashlib.sha256((resources/f'{k}.bin').read_bytes()).hexdigest() for k in segments},movement_marker_table=[0]*360,cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
print('512 original impact-to-blast cases match; launch ownership preserved; original set helpers and area routine executed' if impact else '512 multi-tick retirement-to-blast sequences match; original set helpers, distance, division, request ordering, cleared attribution, counts and preserved records checked')
