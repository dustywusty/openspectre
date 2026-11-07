#!/usr/bin/env python3
"""Execute isolated original 68K routines and compare their outputs to Rust.
No Mac OS, registration, or game startup required. Only a test tool uses emulation.
Requires unicorn==2.1.4. Own-disc bytes and generated vectors remain in .local.
"""
import hashlib
import json
import random
import struct
import subprocess
import sys
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_M68K_REG_A0, UC_M68K_REG_A7, UC_M68K_REG_A5, UC_M68K_REG_PC, UC_M68K_REG_A4, UC_M68K_REG_A1, UC_M68K_REG_A3, UC_M68K_REG_A6, UC_M68K_REG_D0, UC_M68K_REG_D4, UC_M68K_REG_D5, UC_M68K_REG_D6, UC_M68K_REG_D7, UC_CPU_M68K_M68000

root = Path(__file__).resolve().parent.parent
source = root / '.local/game-resources/434f4445'
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0x90000)
code7 = (source/'7.bin').read_bytes()
code18 = (source/'18.bin').read_bytes()
u.mem_write(0x10000, code7)
u.mem_write(0x20000, code18)
stop, stack, a5 = 0x40000, 0x50000, 0x60000
# Distinct marker tables test the two writes and the exact table index.
for index in range(360):
    u.mem_write(a5-0x3312+index*4, struct.pack('>I', index))
    u.mem_write(a5-0x38b2+index*4, struct.pack('>I', 1000+index))

def run(address, arguments, instruction_limit=10000):
    u.reg_write(UC_M68K_REG_A7, stack)
    u.reg_write(UC_M68K_REG_A5, a5)
    u.mem_write(stack, struct.pack('>I', stop)+arguments+bytes(16))
    u.emu_start(address, stop, count=instruction_limit)
    assert u.reg_read(UC_M68K_REG_PC) == stop, 'Routine did not return'
    return struct.unpack('>i', u.mem_read(stack+4+len(arguments), 4))[0]

# Link the original arithmetic helpers through A5 jump stubs.
for segment, address in [(6,0x70000),(30,0x80000),(36,0x90000)]:
    u.mem_write(address,(source/f'{segment}.bin').read_bytes())
for displacement,target in [(0x452,0x7002c),(0x45a,0x7003c),(0x1282,0x90004),(0x128a,0x90038)]:
    u.mem_write(a5+displacement, bytes.fromhex('4ef9')+struct.pack('>I',target))
u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
rng = random.Random(1991)
full = 360 << 16
values = [-2**31,2**31-1,-full,-1,0,1,full-1,full,full+1,180<<16,90<<16]
values += [rng.randint(-2**31,2**31-1) for _ in range(250)]
cases = []
for a in values:
    result = run(0x10004, struct.pack('>i',a))
    cases.append(dict(routine='wrap',a=a,b=0,expected=result))
    b = rng.randrange(full)
    # Include wide signed inputs to preserve wrapping instruction behavior.
    result = run(0x10036, struct.pack('>ii',b,a))
    cases.append(dict(routine='distance',a=a,b=b,expected=result))
    run(0x20dcc,struct.pack('>IIi',0x41004,0x41000,a))
    first,second=struct.unpack('>II',u.mem_read(0x41000,8))
    assert second == 1000+first
    cases.append(dict(routine='index',a=a,b=0,expected=first))
for _ in range(250):
    a,b=rng.randrange(full),rng.randrange(full)
    result=run(0x10036,struct.pack('>ii',b,a))
    cases.append(dict(routine='distance',a=a,b=b,expected=result))
# Compare the full heading-update routine, including its type-6 ramp branch.
for _ in range(300):
    heading=rng.randrange(full)
    ramp=rng.randint(0,8)
    kind=rng.choice([0,6,7])
    command=rng.choice([-1,0,1])
    elapsed=rng.choice([-63,-8,-4,-1,0,1,2,4,8,63])
    delta=rng.randint(-20<<16,20<<16)
    record=bytearray(128)
    struct.pack_into('>i',record,8,heading)
    struct.pack_into('>h',record,12,command)
    struct.pack_into('>b',record,0x14,kind)
    struct.pack_into('>b',record,0x1a,1)
    struct.pack_into('>b',record,0x23,ramp)
    u.mem_write(0x42000,bytes(record))
    run(0x810b8,struct.pack('>iih',elapsed,delta,1))
    result=struct.unpack('>i',u.mem_read(0x42008,4))[0]
    after=struct.unpack('>b',u.mem_read(0x42023,1))[0]
    cases.append(dict(routine='turn',a=heading,b=ramp,kind=kind,command=command,elapsed=elapsed,delta=delta,expected=result,ramp=after))
# Test the normal damage block, not the preceding immunity/mode guards.
u.mem_write(0x30000,(source/'31.bin').read_bytes())
for health,damage in [(1,1),(3,1),(3,4),(0,1),(32767,-1),(-32768,1)]+[(rng.randint(-32768,32767),rng.randint(-32768,32767)) for _ in range(250)]:
    u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_A6,0x50000)
    u.mem_write(0x4205a,struct.pack('>h',health))
    u.mem_write(0x50008,struct.pack('>h',damage))
    u.emu_start(0x30b2e,0x30b40,count=100)
    assert u.reg_read(UC_M68K_REG_PC)==0x30b40
    result=struct.unpack('>h',u.mem_read(0x4205a,2))[0]
    cases.append(dict(routine='damage',a=health,b=damage,expected=result))
# Execute the participant type guard and real CODE 6 helper, without a stubbed predicate.
u.mem_write(a5+0x4e2, bytes.fromhex('4ef9')+struct.pack('>I',0x7197e))
guard_rng = random.Random(31)
guard_phases = [[0,0,0], [0,0,1], [127,127,1], [-128,-128,1],
                [127,-128,0], [-128,127,2]]
guard_phases += [[guard_rng.randint(-128,127) for _ in range(3)] for _ in range(200)]
for phase in guard_phases:
    for kind in [6,31]:
        for health, damage in [(3,1), (1,4), (-1,1), (32767,-1)]:
            u.reg_write(UC_M68K_REG_A7,0x51000)
            u.reg_write(UC_M68K_REG_A5,a5)
            u.reg_write(UC_M68K_REG_A4,0x42000)
            u.reg_write(UC_M68K_REG_A6,0x50000)
            u.mem_write(0x42008,struct.pack('>b',kind))
            u.mem_write(0x4205a,struct.pack('>h',health))
            u.mem_write(0x50008,struct.pack('>h',damage))
            for displacement,value in zip([0x25cf,0x25ff,0x262f],phase):
                u.mem_write(a5-displacement,struct.pack('>b',value))
            u.emu_start(0x30b0c,0x30b40,count=200)
            assert u.reg_read(UC_M68K_REG_PC)==0x30b40
            result=struct.unpack('>h',u.mem_read(0x4205a,2))[0]
            cases.append(dict(routine='guarded_participant_damage',a=health,b=damage,
                              kind=kind,phase=phase,expected=result))
# Execute the complete speed update with an explicit Random trap result.
# Stop before the OS trap, supply its word result, resume original instructions.
random_state={'value':0,'used':False}
def supply_random(cpu,address,size,user):
    random_state['used']=True
    sp=cpu.reg_read(UC_M68K_REG_A7)
    cpu.mem_write(sp,struct.pack('>h',random_state['value']))
    cpu.reg_write(UC_M68K_REG_PC,address+2)
hook=u.hook_add(UC_HOOK_CODE,supply_random,begin=0x81320,end=0x81320)
for _ in range(1200):
    speed=rng.randint(-128,127)
    command=rng.choice([-32768,-2,-1,0,0,0,1,2,32767])
    elapsed=rng.choice([-63,-8,-1,0,1,2,4,8,63,2**31-1,-2**31])
    limit=rng.choice([0,1,5,12,63,127])
    coast=rng.choice([-128,-1,0,1,2,3,4,5,6,127])
    adjustment=rng.choice([-128,-6,-3,-1,0,0,1,3,127])
    mode=rng.choice([-1,0,1])
    random_state.update(value=rng.randint(-32768,32767),used=False)
    record=bytearray(128)
    struct.pack_into('>h',record,0x0e,command)
    struct.pack_into('>b',record,0x22,speed)
    u.mem_write(0x42000,bytes(record))
    u.mem_write(a5-0x3f8e+0x20,struct.pack('>b',limit))
    u.mem_write(a5-0x3f8e+0x23,struct.pack('>b',coast))
    u.mem_write(a5-0x3d32,struct.pack('>b',adjustment))
    u.mem_write(a5-0x12ca,struct.pack('>b',mode))
    u.mem_write(a5-0x134c,struct.pack('>i',123456))
    for offset in [0x3d20,0x12f8]: u.mem_write(a5-offset,bytes(4))
    run(0x811d6,struct.pack('>ihbb',elapsed,1,1,0))
    result=struct.unpack('>b',u.mem_read(0x42022,1))[0]
    assert u.mem_read(a5-0x3d32,1)==b'\0'
    times=[struct.unpack('>i',u.mem_read(a5-offset,4))[0] for offset in [0x3d20,0x12f8]]
    assert times[0]==times[1] and times[0] in [0,123456]
    cases.append(dict(routine='speed',a=speed,b=command,elapsed=elapsed,limit=limit,
        coast=coast,adjustment=adjustment,mode=mode,random=random_state['value'],
        used_random=random_state['used'],reset_timestamps=times[0]==123456,expected=result))
# Run the entire translation routine in four mode/locality contexts with
# ordinary ground state and no active extra displacement. Direction values are
# marker data, not a claim that the original trigonometric table is recovered.
u.mem_write(a5+0xbba,bytes.fromhex('4ef9')+struct.pack('>I',0x20d24))
u.mem_write(a5-0x38be,struct.pack('>I',0x43000))
table=[rng.randint(-65536,65536) for _ in range(360)]
u.mem_write(0x43000,struct.pack('>360i',*table))
for degree in range(360):
    for elapsed in [-4,0,4]:
        heading=(degree<<16)+rng.randrange(65536)
        position=[rng.randint(-2**31,2**31-1) for _ in range(2)]
        speed=rng.randint(-12,12)
        command=rng.choice([-1,0,1])
        coast=rng.choice([0,1,3,6])
        adjustment=rng.choice([-3,0,1])
        random_state.update(value=rng.randint(-32768,32767),used=False)
        record=bytearray(128)
        struct.pack_into('>iii',record,0,*position,heading)
        struct.pack_into('>h',record,0x0e,command)
        record[0x14]=6; record[0x1a]=1
        struct.pack_into('>b',record,0x22,speed)
        for mode,local in [(-1,2),(0,1),(0,2),(1,1)]:
            u.mem_write(0x42000,bytes(record))
            u.mem_write(a5-0x133d,bytes([local]))
            u.mem_write(a5-0x12ca,struct.pack('>b',mode))
            u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
            u.mem_write(0x45000,bytes(256))
            u.mem_write(a5-0x20c2,struct.pack('>h',32))
            u.mem_write(a5-0x20c0,bytes(4))
            u.mem_write(a5-0x131a,bytes(4))
            random_state['used']=False
            u.mem_write(a5-0x3f8e+0x20,b'\x0c')
            u.mem_write(a5-0x3f8e+0x23,struct.pack('>b',coast))
            u.mem_write(a5-0x3d32,struct.pack('>b',adjustment))
            run(0x81370,struct.pack('>hhhihh',0,0,0,elapsed,0,1))
            after=list(struct.unpack('>ii',u.mem_read(0x42000,8)))
            final_speed=struct.unpack('>b',u.mem_read(0x42022,1))[0]
            cases.append(dict(routine='translation',a=heading,b=speed,position=position,
                command=command,elapsed=elapsed,limit=12,coast=coast,adjustment=adjustment,mode=mode,local=local,
                random=random_state['value'],used_random=random_state['used'],
                after=after,expected=final_speed))
u.hook_del(hook)
u.mem_map(0xa0000,0x10000)
u.mem_write(0xa0000,(source/'22.bin').read_bytes())
for selector in [-32768,-1,0,1,2,3,4,5,6,7,32767]:
    for coast in range(8):
        current=[rng.randrange(256) for _ in range(7)]
        saved=[rng.randrange(256) for _ in range(7)]
        saved[3]=coast
        u.mem_write(0x42000,bytes(current))
        for i,offset in enumerate([0x3d9f,0x3da0,0x3da1,0x3da2,None,0x3da3,0x3da4]):
            if offset is not None: u.mem_write(a5-offset,bytes([saved[i]]))
        run(0xa0132,struct.pack('>hI',selector,0x42000))
        after=list(u.mem_read(0x42000,7))
        cases.append(dict(routine='vehicle_settings',a=selector,b=0,current=current,saved=saved,
                          after=after,expected=after[3]))
# All combinations of the eight directional slots, plus noncanonical words.
key_cases=[]
for mask in range(256):
    keys=[0]*13
    for bit,index in enumerate(range(5,13)): keys[index]=(mask>>bit)&1
    key_cases.append(keys)
key_cases += [[rng.choice([-1,0,1,2,32767]) for _ in range(13)] for _ in range(100)]
for keys in key_cases:
    for index,value in enumerate(keys):
        u.mem_write(a5-0x6694+index*6,struct.pack('>h',value))
    u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A4,0x42000)
    u.emu_start(0x31764,0x3181c,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x3181c
    turn,throttle=struct.unpack('>hh',u.mem_read(0x4200c,4))
    cases.append(dict(routine='keyboard',a=0,b=0,keys=keys,turn=turn,expected=throttle))
u.mem_map(0xb0000,0x10000)
u.mem_write(0xb0000,(source/'25.bin').read_bytes())
for machine in list(range(-2,21))+[-32768,32767]:
    for error in [0,-5500,1]:
        u.reg_write(UC_M68K_REG_A5,a5)
        u.reg_write(UC_M68K_REG_A6,0x50000)
        u.reg_write(UC_M68K_REG_A7,0x4f000)
        u.mem_write(0x50000-0x154,struct.pack('>h',machine))
        u.mem_write(0x4f000,struct.pack('>h',error))
        u.emu_start(0xb2668,0xb269a,count=100)
        assert u.reg_read(UC_M68K_REG_PC)==0xb269a
        flag=u.mem_read(a5-0x3cf8,1)[0]
        cases.append(dict(routine='machine_turn_rate',a=machine,b=error,expected=flag))
# Type-31 model transition: execute the complete original routine.
u.mem_map(0xc0000,0x10000)
u.mem_write(0xc0000,(source/'19.bin').read_bytes())
phase_rng=random.Random(1931)
phase_cases=[(10,10,10,-1,100,6),(10,10,10,0,100,6),
             (18,10,10,100,106,6),(18,10,10,100,107,6)]
phase_cases += [(phase_rng.randint(-128,127),phase_rng.randint(-128,127),
                 phase_rng.randint(-128,127),phase_rng.choice([-1,0,100,2147483647,-2147483648]),
                 phase_rng.choice([0,100,106,107,2147483647,-2147483648]),
                 phase_rng.choice([-32768,-1,0,6,32767])) for _ in range(500)]
for current,first,count,timer,now,interval in phase_cases:
    for offset,value in zip([0x25cf,0x25ff,0x262f],[current,first,count]):
        u.mem_write(a5-offset,struct.pack('>b',value))
    u.mem_write(a5-0x135e,struct.pack('>i',timer))
    run(0xc4b6c,struct.pack('>hi',interval,now))
    after=struct.unpack('>b',u.mem_read(a5-0x25cf,1))[0]
    timer_after=struct.unpack('>i',u.mem_read(a5-0x135e,4))[0]
    cases.append(dict(routine='type31_model',a=current,b=timer,first=first,count=count,
                      now=now,interval=interval,expected=after,timer_after=timer_after))
# Original enemy initialization proves participant type -> object type and health-table routing.
for kind in range(47):
    for object_index in [1,3,9]:
        u.reg_write(UC_M68K_REG_A4,0x45000)
        u.reg_write(UC_M68K_REG_A5,a5)
        u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
        u.mem_write(a5-0x1350,struct.pack('>I',0x46000))
        u.mem_write(0x45008,bytes([kind]))
        u.mem_write(0x45056,struct.pack('>h',object_index))
        u.mem_write(0x460be+2*kind,struct.pack('>h',100+kind))
        u.mem_write(0x46060+2*kind,struct.pack('>h',200+kind))
        u.emu_start(0xb2352,0xb2390,count=100)
        assert u.reg_read(UC_M68K_REG_PC)==0xb2390
        assert u.mem_read(0x42000+(object_index-1)*128+0x14,1)[0]==kind
        assert struct.unpack('>hh',u.mem_read(0x4505a,4))==(100+kind,200+kind)
print('141 original enemy type/health linkage checks passed.')
# Complete weapon-table loader with only Resource Manager services supplied.
weapon_state={'present':True}
def weapon_resource_service(cpu,address,size,user):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0xb1974:
        assert bytes(cpu.mem_read(sp,6))==struct.pack('>h4s',10000,b'wval')
        cpu.mem_write(sp+6,struct.pack('>I',0x46f00 if weapon_state['present'] else 0))
        cpu.reg_write(UC_M68K_REG_A7,sp+6)
    elif address==0xb1a30:
        cpu.reg_write(UC_M68K_REG_A7,sp+4)
    cpu.reg_write(UC_M68K_REG_PC,address+2)
weapon_hooks=[u.hook_add(UC_HOOK_CODE,weapon_resource_service,begin=p,end=p)
              for p in [0xb1974,0xb1984,0xb1a2c,0xb1a30]]
weapon_rng=random.Random(10000)
weapon_cases=[None,[],[[0,1,1],[0,7,3],[17,-1,32767],[18,99,99],[256,4,5],[-256,9,8]]]
for campaign in ['central','eastern','south']:
    payload=(root/f'.local/{campaign}-shape-resources/7776616c/10000.bin').read_bytes()
    count=struct.unpack_from('>H',payload)[0]
    assert len(payload)==2+6*count
    weapon_cases.append([list(struct.unpack_from('>hhh',payload,2+6*i)) for i in range(count)])
weapon_cases += [[[weapon_rng.choice(list(range(-2,20))+[-32768,32767,256,-256]),
                   weapon_rng.randint(-32768,32767),weapon_rng.randint(-32768,32767)]
                  for _ in range(weapon_rng.randrange(1,40))] for _ in range(100)]
for entries in weapon_cases:
    weapon_state['present']=entries is not None
    rows=entries or []
    u.mem_write(a5-0x1350,struct.pack('>I',0x46000))
    u.mem_write(0x46f00,struct.pack('>I',0x47000))
    u.mem_write(0x47000,struct.pack('>H',len(rows))+b''.join(struct.pack('>hhh',*r) for r in rows))
    u.mem_write(0x4675a,bytes([0x5a])*144)
    run(0xb1902,b'')
    banks=[list(struct.unpack('>18h',u.mem_read(0x46000+offset,36))) for offset in [0x75a,0x77e,0x7a2,0x7c6]]
    cases.append(dict(routine='weapon_tables',a=0,b=0,entries=rows,banks=banks,expected=0))
for hook in weapon_hooks: u.hook_del(hook)
# Original complete local firing predicate; preserve its raw byte result.
u.mem_map(0xd0000,0x10000)
u.mem_write(0xd0000,(source/'28.bin').read_bytes())
fire_rng=random.Random(28)
fire_cases=[(a,c,h,f) for a,c in [(0,0),(0,1),(1,1),(40,1),(-32768,32767),(32767,-32768)]
            for h in [-1,0,1] for f in [0,1,2,255]]
fire_cases += [(fire_rng.randint(-32768,32767),fire_rng.randint(-32768,32767),
                fire_rng.randint(-32768,32767),fire_rng.randrange(256)) for _ in range(500)]
for ammo,cost,health,inhibit in fire_cases:
    u.mem_write(a5-0x1350,struct.pack('>I',0x46000))
    u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(a5-0x133d,b'\x01')
    u.mem_write(a5-0x2d70,bytes([inhibit]))
    u.mem_write(0x45056,struct.pack('>h',1))
    u.mem_write(0x4505a,struct.pack('>h',health))
    u.mem_write(0x4505e,struct.pack('>h',ammo))
    u.mem_write(0x467c6,struct.pack('>h',cost))
    run(0xd0004,bytes([0,0,1,0]))
    result=u.mem_read(stack+8,1)[0]
    cases.append(dict(routine='fire_eligible',a=ammo,b=cost,health=health,inhibit=inhibit,expected=result))
# Original basic-shot launch block, with original scaling and lifetime getter.
u.mem_map(0xe0000,0x20000)
u.mem_write(0xe0000,(source/'11.bin').read_bytes())
u.mem_write(0xf0000,(source/'23.bin').read_bytes())
for displacement,target in [(0x75a,0xe3c5a),(0xde2,0xf0004),(0xbb2,0x20cd0)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
shot_rng=random.Random(9)
for _ in range(300):
    position=[shot_rng.randint(-2**31,2**31-1) for _ in range(2)]
    direction=[shot_rng.randint(-65536,65536) for _ in range(2)]
    now=shot_rng.randint(-2**31,2**31-1)
    lifetime=shot_rng.choice([75,-32768,32767,0,-1])
    u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A6,0x50000)
    u.reg_write(UC_M68K_REG_A7,0x51000)
    u.reg_write(UC_M68K_REG_D4,now & 0xffffffff)
    u.reg_write(UC_M68K_REG_D7,1)
    u.mem_write(0x42000,struct.pack('>ii',*position))
    u.mem_write(0x50000-0x16,struct.pack('>ii',*direction))
    u.mem_write(a5-0x3dae,struct.pack('>h',lifetime))
    u.emu_start(0x80794,0x807e8,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x807e8
    assert u.mem_read(0x42014,1)[0]==9
    after=list(struct.unpack('>ii',u.mem_read(0x42000,8)))
    deadline=struct.unpack('>i',u.mem_read(0x42016,4))[0]
    cases.append(dict(routine='shot_launch',a=now,b=lifetime,position=position,direction=direction,after=after,expected=deadline))
# Execute the type-9 movement dispatch through the point before spatial-grid update.
shot_table=[shot_rng.randint(-6*65536,6*65536) for _ in range(360)]
u.mem_write(a5-0x38ba,struct.pack('>I',0x43000))
u.mem_write(0x43000,struct.pack('>360i',*shot_table))
for degree in range(360):
    heading=(degree<<16)+shot_rng.randrange(65536)
    elapsed=shot_rng.choice([-4,-1,1,4,63])
    position=[shot_rng.randint(-2**31,2**31-1) for _ in range(2)]
    record=bytearray(128);struct.pack_into('>iii',record,0,*position,heading);record[0x14]=9
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(0x42000,bytes(record))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>Iihh',stop,elapsed,0,1))
    u.emu_start(0x81d54,0x825e8,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x825e8
    after=list(struct.unpack('>ii',u.mem_read(0x42000,8)))
    complement=90-degree if degree<=90 else 450-degree
    direction=[shot_table[complement],shot_table[degree]]
    cases.append(dict(routine='shot_motion',a=elapsed,b=0,position=position,direction=direction,after=after,expected=0))
# Primary fire timing: stop at the original call site or the skipped-call branch.
def stop_at_branch(uc, address, size, data):
    uc.emu_stop()
hooks=[u.hook_add(UC_HOOK_CODE,stop_at_branch,begin=p,end=p) for p in [0x31874,0x3187a]]
timing_rng=random.Random(1874)
timing_pairs=[(n,0) for n in [-1,0,9,10,11,12]]+[(2**31-1,-1),(-2**31,2**31-1),(0,2**31-1)]
timing_pairs += [(timing_rng.randint(-2**31,2**31-1),timing_rng.randint(-2**31,2**31-1)) for _ in range(250)]
for now,last in timing_pairs:
    for pressed in [False,True]:
        u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x1376,struct.pack('>i',last))
        u.reg_write(UC_M68K_REG_D0,255 if pressed else 0);u.reg_write(UC_M68K_REG_A5,a5)
        u.emu_start(0x31862,stop,count=100)
        pc=u.reg_read(UC_M68K_REG_PC);assert pc in [0x31874,0x3187a]
        cases.append(dict(routine='fire_due',a=now,b=last,pressed=pressed,expected=int(pc==0x31874)))
for hook in hooks:u.hook_del(hook)
hooks=[u.hook_add(UC_HOOK_CODE,stop_at_branch,begin=p,end=p) for p in [0x808ce,0x80948]]
for disabled in range(256):
    now,last=timing_pairs[disabled]
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x1376,struct.pack('>i',last))
    u.mem_write(a5-0x12d2,bytes([disabled]));u.reg_write(UC_M68K_REG_A5,a5)
    u.emu_start(0x808be,stop,count=100)
    assert u.reg_read(UC_M68K_REG_PC) in [0x808ce,0x80948]
    result=struct.unpack('>i',u.mem_read(a5-0x1376,4))[0]
    cases.append(dict(routine='attempt_timestamp',a=now,b=last,disabled=disabled,expected=result))
for hook in hooks:u.hook_del(hook)
# Full object expiry dispatcher, weapon 0/type 9, until movement or removal.
for displacement,target in [(0x129a,0x900ec),(0x12a2,0x9012e)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
hooks=[u.hook_add(UC_HOOK_CODE,stop_at_branch,begin=p,end=p) for p in [0x82c8c,0x82c94]]
expiry_rng=random.Random(0x2bbe)
expiry_pairs=[(now,deadline) for now in [-2**31,-1,0,74,75,76,2**31-1]
              for deadline in [-2**31,-1,0,74,75,76,2**31-1]]
expiry_pairs += [(expiry_rng.randint(-2**31,2**31-1),expiry_rng.randint(-2**31,2**31-1)) for _ in range(300)]
for now,deadline in expiry_pairs:
    record=bytearray(128);record[0x14]=9;struct.pack_into('>i',record,0x16,deadline)
    u.mem_write(0x42000,bytes(record));u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.mem_write(a5-0x12c8,struct.pack('>h',0));u.mem_write(a5-0x1348,struct.pack('>h',1))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>Ii',stop,1))
    u.emu_start(0x82bbe,stop,count=10000)
    pc=u.reg_read(UC_M68K_REG_PC);assert pc in [0x82c8c,0x82c94],hex(pc)
    cases.append(dict(routine='shot_expired',a=now,b=deadline,expected=int(pc==0x82c8c)))
for hook in hooks:u.hook_del(hook)
# Original elapsed-time calculation, including original min/max helpers.
elapsed_pairs=[(v,0) for v in [-2**31,-65537,-65536,-32769,-32768,-1,0,1,62,63,64,32767,32768,65535,65536,65537,2**31-1]]
elapsed_pairs += [(expiry_rng.randint(-2**31,2**31-1),expiry_rng.randint(-2**31,2**31-1)) for _ in range(300)]
for now,last in elapsed_pairs:
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_A6,0x50000);u.reg_write(UC_M68K_REG_A7,0x51000)
    u.mem_write(0x50008,struct.pack('>i',now));u.mem_write(0x42010,struct.pack('>i',last))
    u.emu_start(0x8271a,0x8273a,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x8273a
    result=u.reg_read(UC_M68K_REG_D6)
    cases.append(dict(routine='projectile_elapsed',a=now,b=last,expected=result))
# Complete oriented rectangle overlap, with distinct direction marker tables.
u.mem_write(a5+0xbca,bytes.fromhex('4ef9')+struct.pack('>I',0x20dcc))
for index in range(360):
    u.mem_write(a5-0x38b2+index*4,struct.pack('>I',(index*193)%65536))
    u.mem_write(a5-0x3312+index*4,struct.pack('>I',(index*317+1000)%65536))
rect_rng=random.Random(0x250)
for index in range(1000):
    first=[rect_rng.randrange(1024<<16),rect_rng.randrange(1024<<16),rect_rng.randrange(360<<16)]
    second=[first[0]+rect_rng.randrange(-100<<16,100<<16),first[1]+rect_rng.randrange(-100<<16,100<<16),rect_rng.randrange(360<<16)]
    dimensions=[rect_rng.randrange(1,100<<16) for _ in range(4)]
    if index<100: second=first.copy()
    if index>=800:
        first[:2]=[rect_rng.randint(-2**31,2**31-1) for _ in range(2)]
        second[:2]=[rect_rng.randint(-2**31,2**31-1) for _ in range(2)]
        dimensions=[rect_rng.randint(-2**31,2**31-1) for _ in range(4)]
    u.mem_write(0x42000,struct.pack('>iii',*first));u.mem_write(0x42100,struct.pack('>iii',*second))
    run(0x10250,struct.pack('>iiiiII',*dimensions,0x42100,0x42000))
    result=u.mem_read(stack+28,1)[0]
    cases.append(dict(routine='rect_overlap',a=0,b=0,first=first,second=second,dimensions=dimensions,expected=result))
# Planar dimension accumulation from packed runtime vertices. Radius is independent;
# bypass its square-root routine (which calls the Mac FixDiv trap), not the dimension code.
u.mem_map(0x100000,0x10000);u.mem_write(0x100000,(source/'12.bin').read_bytes())
u.mem_write(a5+0x47a,bytes.fromhex('4ef9')+struct.pack('>I',0x7005c))
def bypass_radius(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    uc.mem_write(sp+8,bytes(4));uc.reg_write(UC_M68K_REG_A7,sp+8);uc.reg_write(UC_M68K_REG_PC,ret)
hook=u.hook_add(UC_HOOK_CODE,bypass_radius,begin=0x1000c4,end=0x1000c4)
dimension_rng=random.Random(0x944)
vertex_lists=[[],[[0,0,0]],[[1,32767,-1]],[[-32768,0,32767]],[[80,0,-160],[-16,0,8]]]
vertex_lists += [[[dimension_rng.randint(-32768,32767) for _ in range(3)] for _ in range(dimension_rng.randrange(1,40))] for _ in range(200)]
for vertices in vertex_lists:
    packed=struct.pack('>h',len(vertices))+b''.join(struct.pack('>hhhhh',*v,0,0) for v in vertices)
    u.mem_write(0x47000,packed)
    u.mem_write(a5-0x6490,struct.pack('>h',1));u.mem_write(a5-0x648e,struct.pack('>I',0x47000))
    u.mem_write(0x41000,bytes([0x55])*12)
    run(0x100944,struct.pack('>III',0x41000,0x41004,0x41008),instruction_limit=100000)
    dimensions=list(struct.unpack('>ii',u.mem_read(0x41000,8)))
    assert u.mem_read(a5-0x6490,2)==bytes(2)
    cases.append(dict(routine='collision_dimensions',a=0,b=0,vertices=vertices,dimensions=dimensions,expected=0))
u.hook_del(hook)
# Vertex insertion with a documented FixRound model; not original OS trap execution.
def model_fixround(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    value=struct.unpack('>i',uc.mem_read(sp,4))[0]
    rounded=((value+32768)//65536)&0xffff
    uc.mem_write(sp+4,struct.pack('>H',rounded));uc.reg_write(UC_M68K_REG_A7,sp+4)
    uc.reg_write(UC_M68K_REG_PC,address+2)
hooks=[u.hook_add(UC_HOOK_CODE,model_fixround,begin=p,end=p) for p in [0x1005a6,0x1005c0,0x1005da]]
vertex_cases=[([v,v,-v],65536) for v in [-65536,-4097,-4096,-4095,-1,0,1,4095,4096,4097,65536]]
vertex_cases += [([dimension_rng.randint(-100*65536,100*65536) for _ in range(3)],dimension_rng.choice([32768,65536,98304,-65536])) for _ in range(200)]
# Actual resource coordinates, with the scale-1 path from CODE 19 +0x3f4a.
unique_vertices=set()
for campaign in ['central','eastern','south']:
    for path in sorted((root/f'.local/{campaign}-shape-resources/73686170').glob('*.bin')):
        payload=path.read_bytes();count=struct.unpack_from('>H',payload)[0]+1
        for index in range(count): unique_vertices.add(struct.unpack_from('>iii',payload,2+12*index))
vertex_cases += [(list(v),65536) for v in sorted(unique_vertices)]
for vertex,scale in vertex_cases:
    u.mem_write(a5-0x6490,struct.pack('>h',1));u.mem_write(a5-0x6494,struct.pack('>i',scale))
    u.mem_write(a5-0x648e,struct.pack('>I',0x47000));u.mem_write(0x47000,bytes(12))
    u.mem_write(a5-0x7b99,b'\0')
    run(0x100532,struct.pack('>iii',*reversed(vertex)))
    assert u.mem_read(0x47000,2)==b'\0\1'
    packed=list(struct.unpack('>hhh',u.mem_read(0x47002,6)))
    cases.append(dict(routine='runtime_vertex',a=scale,b=0,vertex=vertex,packed=packed,expected=0,conditional='modeled FixRound'))
for hook in hooks:u.hook_del(hook)
print(f'{len(vertex_cases)} vertex cases use modeled FixRound; {len(unique_vertices)} unique resource vertices')
# Original projectile broad-phase candidate filter, before rectangle overlap.
hooks=[u.hook_add(UC_HOOK_CODE,stop_at_branch,begin=p,end=p) for p in [0x10914,0x10978]]
filter_rng=random.Random(0x8b4)
filters=[]
for kind in range(-128,128):
    for ids in [[2,1],[2,2]]:
        for position in [[10<<16,20<<16],[11<<16,20<<16],[10<<16,21<<16]]:
            filters.append((ids,position,[0,0],[10,20],kind))
for _ in range(500):
    filters.append(([filter_rng.randint(-32768,32767) for _ in range(2)],
                    [filter_rng.randint(-2**31,2**31-1) for _ in range(2)],
                    [filter_rng.randint(-32768,32767) for _ in range(2)],
                    [filter_rng.randint(-32768,32767) for _ in range(2)],filter_rng.randint(-128,127)))
filters += [([2,1],[-32768<<16,0],[0,0],[0,0],7),([2,1],[0,-32768<<16],[0,0],[0,0],31)]
for ids,position,center,extents,kind in filters:
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x42000);u.reg_write(UC_M68K_REG_A6,0x50000)
    u.reg_write(UC_M68K_REG_D5,ids[0]&0xffff);u.reg_write(UC_M68K_REG_D6,extents[0]&0xffff);u.reg_write(UC_M68K_REG_D4,extents[1]&0xffff)
    u.mem_write(0x50008,struct.pack('>h',ids[1]));u.mem_write(0x50000-0x1a,struct.pack('>h',center[0]));u.mem_write(0x50000-0x1c,struct.pack('>h',center[1]))
    u.mem_write(0x42000,struct.pack('>ii',*position));u.mem_write(0x42014,struct.pack('>b',kind))
    u.emu_start(0x108b4,stop,count=1000)
    pc=u.reg_read(UC_M68K_REG_PC);assert pc in [0x10914,0x10978]
    cases.append(dict(routine='projectile_candidate',a=kind,b=0,ids=ids,position=position,center=center,extents=extents,expected=int(pc==0x10914)))
for hook in hooks:u.hook_del(hook)
# Complete multiple-hit preference helper, with signed direction marker tables.
for index in range(500):
    current=[filter_rng.randint(-2**31,2**31-1) for _ in range(2)]
    candidate=current.copy() if index<50 else [filter_rng.randint(-2**31,2**31-1) for _ in range(2)]
    degree=index%360;heading=(degree<<16)+filter_rng.randrange(65536)
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(0x42000,struct.pack('>ii',*current));u.mem_write(0x42080,struct.pack('>ii',*candidate))
    run(0x10618,struct.pack('>ihh',heading,1,2))
    result=u.mem_read(stack+12,1)[0]
    signed=lambda n: n if n<32768 else n-65536
    direction=[signed((degree*193)%65536),signed((degree*317+1000)%65536)]
    cases.append(dict(routine='prefer_hit',a=0,b=0,current=current,candidate=candidate,direction=direction,expected=result))
# Entire grid-bound prefix, including original direction and min/max helpers.
grid_rng=random.Random(0x6ba)
grid_cases=[([v<<16,v<<16,90<<16],[98304,9<<16],16) for v in [-1,0,36,46,63,64,512,1023,1024,32767]]
grid_cases += [([512<<16,512<<16,d<<16],[98304,length],16) for d in [0,45,90,180,270,359] for length in [32<<16,33<<16,34<<16,35<<16,381<<16]]
grid_cases += [([grid_rng.randint(-2**31,2**31-1) for _ in range(2)]+[grid_rng.randrange(360<<16)],
                [grid_rng.randint(-2**31,2**31-1) for _ in range(2)],grid_rng.choice([0,1,16,32,32767,-32768])) for _ in range(500)]
for pose,dimensions,cells in grid_cases:
    u.mem_write(0x42000,struct.pack('>iii',*pose));u.mem_write(a5-0x1380,struct.pack('>h',cells))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>IhiiI',stop,1,*dimensions,0x42000))
    u.emu_start(0x106ba,0x10828,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x10828
    frame=u.reg_read(UC_M68K_REG_A6)
    bounds=list(struct.unpack('>hhhh',u.mem_read(frame-0x28,8)))
    signed=lambda n: (n&65535) if (n&65535)<32768 else (n&65535)-65536
    extents=[signed(u.reg_read(UC_M68K_REG_D6)),signed(u.reg_read(UC_M68K_REG_D4))]
    cases.append(dict(routine='projectile_grid',a=cells,b=0,pose=pose,dimensions=dimensions,extents=extents,bounds=bounds,expected=0))
# Grid traversal only: execute original loops and index loads, bypass hit testing.
visited=[]
def capture_grid_entry(uc,address,size,data):
    value=uc.reg_read(UC_M68K_REG_D5)&65535
    visited.append(value if value<32768 else value-65536)
    uc.reg_write(UC_M68K_REG_PC,0x10978)
hook=u.hook_add(UC_HOOK_CODE,capture_grid_entry,begin=0x1088e,end=0x1088e)
traversal_rng=random.Random(0x828)
traversal_bounds=[[0,0,15,15],[0,0,0,0],[15,15,15,15],[1000,0,15,15],[0,1000,15,15]]
traversal_bounds += [[traversal_rng.randrange(16) for _ in range(4)] for _ in range(70)]
for index,bounds in enumerate(traversal_bounds):
    grid=[[traversal_rng.randrange(1,20) for _ in range(traversal_rng.randrange(6))] for _ in range(256)]
    if index==1:grid[0]=[7]*20
    packed=b''.join(struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))) for cell in grid)
    u.mem_write(0x48000,packed);u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x50000)
    u.mem_write(0x50000-0x28,struct.pack('>hhhh',*bounds));visited.clear()
    u.emu_start(0x10828,0x109ae,count=100000)
    assert u.reg_read(UC_M68K_REG_PC)==0x109ae
    cases.append(dict(routine='grid_traversal',a=0,b=0,bounds=bounds,grid=grid,visited=visited.copy(),expected=0))
u.hook_del(hook)
# Original clamped object-cell helper and complete type-9 grid movement routine.
u.mem_write(a5+0x77a,bytes.fromhex('4ef9')+struct.pack('>I',0xe3eee))
cell_rng=random.Random(0xf60)
positions=[[v,v] for v in [-2**31,-1,0,(64<<16)-1,64<<16,(1024<<16)-1,1024<<16,2**31-1]]
positions += [[cell_rng.randint(-2**31,2**31-1) for _ in range(2)] for _ in range(300)]
for position in positions:
    u.mem_write(0x44000,struct.pack('>ii',*position))
    run(0xe3eee,struct.pack('>III',0x41004,0x41000,0x44000))
    cell=[struct.unpack('>h',u.mem_read(addr,2))[0] for addr in [0x41000,0x41004]]
    cases.append(dict(routine='object_cell',a=position[0],b=position[1],cell=cell,expected=0))
for index in range(200):
    old=[cell_rng.randrange(1024<<16) for _ in range(2)]
    new=old.copy() if index<25 else [cell_rng.randrange(1024<<16) for _ in range(2)]
    old_cell=(old[0]>>22)*16+(old[1]>>22)
    new_cell=(new[0]>>22)*16+(new[1]>>22)
    grid=[[] for _ in range(256)]
    grid[old_cell]=[cell_rng.randrange(1,10) for _ in range(cell_rng.randrange(20))]
    if old_cell!=new_cell: grid[new_cell]=[cell_rng.randrange(1,10) for _ in range(cell_rng.randrange(20))]
    packed=b''.join(struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))) for cell in grid)
    u.mem_write(0x48000,packed);u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));record=bytearray(128);record[0x14]=9;u.mem_write(0x42000,bytes(record))
    u.mem_write(0x44000,struct.pack('>iii',*old,0));u.mem_write(0x44100,struct.pack('>iii',*new,0))
    run(0x80f60,struct.pack('>IIh',0x44100,0x44000,1))
    after=[]
    for cell in range(256):
        words=struct.unpack('>21h',u.mem_read(0x48000+42*cell,42))
        assert 0<=words[0]<=20
        after.append(list(words[1:1+words[0]]))
    cases.append(dict(routine='grid_move',a=0,b=0,old=old,new=new,grid=grid,after=after,expected=0))
# Projectile insertion and grid-unlink blocks. Object-array deletion is separate.
cell_cases=[list(range(1,n+1)) for n in range(21)]
cell_cases += [[cell_rng.randrange(1,10) for _ in range(cell_rng.randrange(21))] for _ in range(100)]
for cell in cell_cases:
    for object_id in [1,20,21,-1]:
        for routine,start_address,end_address in [('grid_insert',0x8088c,0x808a4),('grid_unlink',0xb0ea4,0xb0eda)]:
            u.mem_write(0x48000,struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))))
            u.reg_write(UC_M68K_REG_A3,0x48000);u.reg_write(UC_M68K_REG_A6,0x50000)
            u.reg_write(UC_M68K_REG_D6,object_id&65535)
            u.mem_write(0x50000-0xe,struct.pack('>h',object_id))
            u.emu_start(start_address,end_address,count=10000)
            assert u.reg_read(UC_M68K_REG_PC)==end_address
            words=struct.unpack('>21h',u.mem_read(0x48000,42));assert 0<=words[0]<=20
            after=list(words[1:1+words[0]])
            cases.append(dict(routine=routine,a=object_id,b=0,cell=cell,after=after,expected=int(len(cell)!=len(after))))
# Complete original deletion routine for projectile kinds, including participant state.
u.mem_map(0x110000,0x10000);u.mem_write(0x110000,(source/'14.bin').read_bytes())
u.mem_write(a5+0x90a,bytes.fromhex('4ef9')+struct.pack('>I',0x112e20))
delete_rng=random.Random(0xe62)
projectile_kinds=[9,28,29,32,33,34,43,44,46]
for iteration in range(341):
    count=delete_rng.randrange(1,8);object_id=delete_rng.randrange(1,count+1)
    records=[];grid=[[] for _ in range(256)]
    for index in range(count):
        record=bytearray(delete_rng.randbytes(128))
        position=[delete_rng.randrange(1024<<16) for _ in range(2)]
        struct.pack_into('>ii',record,0,*position)
        record[0x14]=iteration%47 if index==count-1 else 9
        record[0x1a]=delete_rng.randrange(13)
        if index==object_id-1:
            record[0x14]=projectile_kinds[iteration%len(projectile_kinds)] if iteration<200 else (iteration-200)//3
            if iteration>=200:record[0x1a]=1+iteration%12
        records.append(list(record))
        grid[(position[0]>>22)*16+(position[1]>>22)].extend([index+1]*(2 if iteration%5==0 else 1))
    participants=[list(delete_rng.randbytes(256)) for _ in range(12)]
    status=list(delete_rng.randbytes(12));u.mem_write(a5-0x4c40,bytes(status))
    u.mem_write(0x42000,b''.join(bytes(r) for r in records));u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(0x45000,b''.join(bytes(p) for p in participants));u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    u.mem_write(a5-0x1348,struct.pack('>h',count));u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    u.mem_write(0x48000,b''.join(struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))) for cell in grid))
    run(0xb0e62,struct.pack('>h',object_id))
    after_grid=[]
    for cell in range(256):
        words=struct.unpack('>21h',u.mem_read(0x48000+42*cell,42));after_grid.append(list(words[1:1+words[0]]))
    cases.append(dict(routine='delete_projectile' if iteration<200 else 'delete_object',status=status,after_status=list(u.mem_read(a5-0x4c40,12)),a=object_id,b=0,records=records,grid=grid,participants=participants,
                      after_records=[list(u.mem_read(0x42000+128*i,128)) for i in range(count)],
                      after_participants=[list(u.mem_read(0x45000+256*i,256)) for i in range(12)],after_grid=after_grid,
                      after_count=struct.unpack('>h',u.mem_read(a5-0x1348,2))[0],expected=0))
# Every located CODE 19 planar collision-dimension override block.
override_blocks=[(0,0x220e,0x223a,4),(1,0x2c4a,0x2ca2,18),(2,0x3cbe,0x3d0a,9),
                 (2,0x42d4,0x4320,28),(1,0x5b42,0x5b9a,16),(3,0x14c4,0x1508,12)]
override_rng=random.Random(0x2210)
for operation,start_address,end_address,kind in override_blocks:
    for index in range(100):
        slot=override_rng.randrange(2,100);source_slot=1
        before=[override_rng.randint(-2**31,2**31-1) for _ in range(2)]
        source_dimensions=[override_rng.randint(-2**31,2**31-1) for _ in range(2)]
        u.mem_write(a5-0x25ee+kind,bytes([slot]));u.mem_write(a5-0x25ee+2,bytes([source_slot]))
        for axis,base in enumerate([a5-0x2572,a5-0x23e2]):
            u.mem_write(base+4*(slot-1),struct.pack('>i',before[axis]))
            u.mem_write(base,struct.pack('>i',source_dimensions[axis]))
        u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
        u.emu_start(0xc0000+start_address,0xc0000+end_address,count=10000)
        assert u.reg_read(UC_M68K_REG_PC)==0xc0000+end_address
        after=[struct.unpack('>i',u.mem_read(base+4*(slot-1),4))[0] for base in [a5-0x2572,a5-0x23e2]]
        cases.append(dict(routine='dimension_override',a=operation,b=0,before=before,source=source_dimensions,after=after,block=hex(start_address),expected=0))
# Original procedural-selector dispatch, stopping at the first builder call.
u.mem_map(0x120000,0x10000);u.mem_write(0x120000,(source/'1.bin').read_bytes())
entry_calls={0x138e:0x2454,0x1396:0x312c,0x139e:0x3328,0x13a4:0x3664,0x13aa:0x1e0e,0x13b0:0xfd4,
             0x13b6:0x3a7c,0x13bc:0x434c,0x13c2:0x3fc2,0x13c8:0x37ca,0x13ce:0x28d8,0x13d4:0x2646,
             0x13da:0x48bc,0x13e2:0x4d38,0x13fe:0x53a0,0x1404:0x2cd4,0x140c:0x5a34,0x1412:0xb42,
             0x1418:0x1532,0x141e:0x5e24,0x1422:-1}
hooks=[u.hook_add(UC_HOOK_CODE,stop_at_branch,begin=0x120000+p,end=0x120000+p) for p in entry_calls]
for selector in [-32768,-1,0]+list(range(1,1000))+[10000,32767]:
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>Ihhh',stop,1,selector,0))
    u.emu_start(0x12133a,stop,count=1000)
    pc=u.reg_read(UC_M68K_REG_PC)-0x120000;assert pc in entry_calls
    cases.append(dict(routine='procedural_entry',a=selector,b=0,expected=entry_calls[pc]))
for hook in hooks:u.hook_del(hook)
# Final hit veto, executing the original tail after candidate selection.
# Owner-state bytes are inputs; their gameplay meaning remains untraced.
hook=u.hook_add(UC_HOOK_CODE,stop_at_branch,begin=0x10ab2,end=0x10ab2)
veto_rng=random.Random(0x9ae)
veto_cases=[([1,2],[hit,excluded],[0,0],[0,0]) for hit in range(48) for excluded in range(48)]
for side in range(2):
    for state in range(256):
        owners=[1,12];states=[0,0];states[side]=state
        veto_cases.append(([1,2],[7,6],owners,states))
for _ in range(1000):
    veto_cases.append(([veto_rng.choice([-32768,-1,0,1]),veto_rng.choice([-32768,-1,0,2])],
                       [veto_rng.randint(-128,127) for _ in range(2)],
                       [veto_rng.choice([-128,-1,0,1]),veto_rng.choice([-128,-1,0,12])],
                       [veto_rng.choice([0,3,255]) for _ in range(2)]))
for ids,kinds,owners,states in veto_cases:
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack)
    u.reg_write(UC_M68K_REG_D7,ids[0]&0xffff)
    u.mem_write(stack+8,struct.pack('>h',ids[1]))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    for index in range(2):
        u.mem_write(0x42014+128*index,struct.pack('>b',kinds[index]))
        u.mem_write(0x4201a+128*index,struct.pack('>b',owners[index]))
        if owners[index]>0:u.mem_write(0x45004+256*(owners[index]-1),bytes([states[index]]))
    u.emu_start(0x109ae,stop,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x10ab2
    result=u.reg_read(UC_M68K_REG_D7)&0xffff
    if result>=32768:result-=65536
    cases.append(dict(routine='finalize_hit',a=0,b=0,ids=ids,kinds=kinds,
                      states=[states[i] if owners[i]>0 else None for i in range(2)],expected=result))
u.hook_del(hook)
# Complete collision query: no instruction bypasses or modeled services.
u.mem_write(a5+0x4da,bytes.fromhex('4ef9')+struct.pack('>I',0x718ea))
query_rng=random.Random(0x6ba19)
query_hits=[0,0]
basic_results={}
basic_rng=random.Random(0xad6)
subprocess.run([sys.executable,str(root/'scripts/recover-direction-table.py')],cwd=root,check=True)
collision_table=json.loads((root/'.local/disassembly/direction-table-candidate.json').read_text())['collision']
for iteration in range(1600):
    if iteration==800:
        for index,(cosine,sine) in enumerate(collision_table):
            u.mem_write(a5-0x38b2+index*4,struct.pack('>i',cosine))
            u.mem_write(a5-0x3312+index*4,struct.pack('>i',sine))
    pose=[query_rng.randrange(80,944)<<16,query_rng.randrange(80,944)<<16,query_rng.randrange(360)<<16]
    if iteration<8:pose[:2]=[[0,0],[1023<<16,1023<<16],[36<<16,36<<16],[64<<16,64<<16]][iteration%4]
    dimensions=[98304,query_rng.randrange(1,101)<<16]
    slots=[1+kind%6 for kind in range(48)]
    sizes=[[query_rng.randrange(2,33)<<16,query_rng.randrange(2,33)<<16] for _ in range(8)]
    states=[query_rng.choice([0,0,0,1,2,3]) for _ in range(12)]
    records=[];grid=[[] for _ in range(256)]
    for index in range(20):
        record=bytearray(128)
        position=[max(0,min(1023<<16,pose[i]+(query_rng.randrange(-60,61)<<16))) for i in range(2)]
        if index<2:position=pose[:2].copy()
        struct.pack_into('>iii',record,0,*position,query_rng.randrange(360)<<16)
        kind=query_rng.randrange(48) if index else query_rng.choice([6,7,10,21,23,24,35,45])
        record[0x14]=kind;record[0x1a]=query_rng.choice([0,0,1,12])
        if kind==42:
            struct.pack_into('>h',record,0x44,1);record[0x3e]=query_rng.randrange(3)
        records.append(list(record))
        cell=(position[0]>>22)*16+(position[1]>>22)
        if len(grid[cell])<20:grid[cell].append(index+1)
    # Duplicate IDs and traversal order must survive composition.
    for cell in grid:
        if cell and len(cell)<20 and query_rng.randrange(3)==0:cell.append(cell[0])
        query_rng.shuffle(cell)
    excluded=query_rng.choice([-1,0,1,2])
    u.mem_write(0x42000,b''.join(bytes(r) for r in records))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    for index,state in enumerate(states):u.mem_write(0x45004+index*256,bytes([state]))
    u.mem_write(a5-0x25ee,bytes(slots))
    for index,size in enumerate(sizes):
        u.mem_write(a5-0x2572+index*4,struct.pack('>i',size[0]))
        u.mem_write(a5-0x23e2+index*4,struct.pack('>i',size[1]))
    u.mem_write(a5-0x1380,struct.pack('>h',16))
    u.mem_write(0x48000,b''.join(struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))) for cell in grid))
    u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    u.mem_write(0x44000,struct.pack('>iii',*pose))
    run(0x106ba,struct.pack('>hiiI',excluded,*dimensions,0x44000),instruction_limit=100000)
    result=struct.unpack('>h',u.mem_read(stack+18,2))[0]
    query_hits[iteration//800]+=int(result>0)
    cases.append(dict(routine='collision_query',a=excluded,b=0,candidate_table=iteration>=800,pose=pose,dimensions=dimensions,
                      slots=slots,sizes=sizes,states=states,records=records,grid=grid,expected=result))
    if iteration>=800:
        records=[r.copy() for r in records]
        records[0][0x14]=9
        records[0][0x1a]=basic_rng.choice([0,1,12])
        participant_objects=[2]*12
        for index,object_id in enumerate(participant_objects):
            u.mem_write(0x45056+256*index,struct.pack('>h',object_id))
        for record in records:
            height=basic_rng.choice([0,60<<16,61<<16,-1,-2**31])
            record[0x28:0x2c]=struct.pack('>i',height)
        packed=b''.join(bytes(r) for r in records)
        u.mem_write(0x42000,packed)
        current=list(struct.unpack('>iii',bytes(records[0][:12])))
        next_pose=[max(0,min(1023<<16,current[i]+(basic_rng.randrange(-30,31)<<16))) for i in range(2)]+[basic_rng.randrange(360)<<16]
        elapsed=basic_rng.randrange(64)
        u.mem_write(0x44000,struct.pack('>iii',*next_pose))
        run(0x10ad6,struct.pack('>iIh',elapsed,0x44000,1),instruction_limit=150000)
        result=struct.unpack('>h',u.mem_read(stack+14,2))[0]
        assert u.mem_read(0x42000,len(packed))==packed
        basic_results['hit' if result>0 else str(result)]=basic_results.get('hit' if result>0 else str(result),0)+1
        cases.append(dict(routine='basic_collision',a=1,b=elapsed,candidate_table=True,pose=next_pose,dimensions=[0,0],
                          slots=slots,sizes=sizes,states=states,records=records,grid=grid,
                          participant_objects=participant_objects,expected=result))
print(f'Complete basic projectile caller: {basic_results}')
assert all(basic_results.get(key,0)>0 for key in ['hit','0','-1'])
assert all(0<n<800 for n in query_hits)
print(f'Complete collision query: {query_hits} hits across 800 marker and 800 modeled-SANE table cases')
# Slot reservation and completion bookkeeping with supplied builder counts.
u.mem_write(a5+0xc32,bytes.fromhex('4ef9')+struct.pack('>I',0xc23c6))
slot_rng=random.Random(0x2358)
slot_cases=[(kind,next_slot,current,-1,count) for kind in range(47)
            for next_slot in [1,100,101] for current in [-1,1] for count in [0,1,10]]
slot_cases += [(slot_rng.randrange(47),slot_rng.randint(-32768,32767),slot_rng.randint(-128,127),
                slot_rng.randint(-128,127),slot_rng.randint(-32768,32767)) for _ in range(200)]
for kind,next_slot,current,first,loaded in slot_cases:
    u.mem_write(a5-0x25ee+kind,struct.pack('>b',current))
    u.mem_write(a5-0x261e+kind,struct.pack('>b',first))
    u.mem_write(0x41000,struct.pack('>h',next_slot))
    run(0xc2358,struct.pack('>IBB',0x41000,kind,0))
    allowed=u.mem_read(stack+10,1)[0]
    # Original loader tail always completes, even when reservation was declined.
    u.reg_write(UC_M68K_REG_A4,0x41000);u.reg_write(UC_M68K_REG_D5,loaded&65535)
    u.reg_write(UC_M68K_REG_D6,kind);u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0x121322,0x12132c,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x12132c
    after=[struct.unpack('>h',u.mem_read(0x41000,2))[0]]
    after += [struct.unpack('>b',u.mem_read(a5+offset+kind,1))[0] for offset in [-0x25ee,-0x261e,-0x264e]]
    cases.append(dict(routine='model_reservation',a=kind,b=next_slot,current=current,first=first,loaded=loaded,after=after,expected=allowed))
# Full outer alias branches. Selected-model loading is an explicit service fixture.
# Its reservation/completion semantics were independently compared above.
alias_calls=[];alias_builder_counts=[];loader_events=[]
def selected_model_fixture(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    ret,counter=struct.unpack('>II',uc.mem_read(sp,8));kind=uc.mem_read(sp+8,1)[0]
    alias_calls.append(kind)
    next_slot=struct.unpack('>h',uc.mem_read(counter,2))[0]
    loader_events.append(['selected',kind,next_slot])
    current=struct.unpack('>b',uc.mem_read(a5-0x25ee+kind,1))[0]
    loaded=0
    if current==-1:
        if next_slot>100:uc.mem_write(a5-0x25ee+kind,b'\1')
        else:
            uc.mem_write(a5-0x25ee+kind,bytes([next_slot&255]))
            uc.mem_write(a5-0x261e+kind,bytes([next_slot&255]))
            loaded=alias_builder_counts[kind]
    uc.mem_write(counter,struct.pack('>H',(next_slot+loaded)&65535))
    uc.mem_write(a5-0x264e+kind,bytes([loaded&255]))
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0x11a,bytes.fromhex('4ef9')+struct.pack('>I',0x121206))
hook=u.hook_add(UC_HOOK_CODE,selected_model_fixture,begin=0x121206,end=0x121206)
alias_rng=random.Random(0x5a80)
for kind in [14,23,24,25,26]:
    for iteration in range(100):
        current=[alias_rng.choice([-1,-1,1,17,99]) for _ in range(47)]
        first=[alias_rng.randint(-1,100) for _ in range(47)]
        counts=[alias_rng.randrange(11) for _ in range(47)]
        if iteration<75:current[kind]=-1
        if iteration<10:current=[-1]*47
        next_slot=alias_rng.choice([1,50,99,100,101,127])
        alias_builder_counts=[alias_rng.choice([0,1,10]) for _ in range(47)]
        for offset,values in [(-0x25ee,current),(-0x261e,first),(-0x264e,counts)]:
            u.mem_write(a5+offset,bytes(v&255 for v in values))
        u.mem_write(a5-0x25be,struct.pack('>h',next_slot));alias_calls.clear()
        run(0xc5a80,bytes([kind,0]),instruction_limit=10000)
        after={name:list(struct.unpack('>47b',u.mem_read(a5+offset,47))) for name,offset in [('current',-0x25ee),('first',-0x261e),('counts',-0x264e)]}
        after.update(next=struct.unpack('>h',u.mem_read(a5-0x25be,2))[0],calls=alias_calls.copy())
        cases.append(dict(routine='model_alias',a=kind,b=next_slot,current=current,first=first,counts=counts,conditional='supplied selected-model loader',
                          builder_counts=alias_builder_counts.copy(),after=after,expected=0))
u.hook_del(hook)
# Complete outer type dispatch. Geometry services remain explicit fixtures.
resource_counts=[]
def raw_resource_fixture(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    ret,slot,resource_id=struct.unpack('>Ihh',uc.mem_read(sp,8));kind=uc.mem_read(sp+8,1)[0]
    loader_events.append(['resource',kind,resource_id,slot])
    assert kind==14 and resource_id in [10099,10100]
    uc.mem_write(sp+10,struct.pack('>h',resource_counts[resource_id-10099]))
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0x122,bytes.fromhex('4ef9')+struct.pack('>I',0x12165e))
hooks=[u.hook_add(UC_HOOK_CODE,selected_model_fixture,begin=0x121206,end=0x121206),
       u.hook_add(UC_HOOK_CODE,raw_resource_fixture,begin=0x12165e,end=0x12165e)]
for kind in range(47):
    for iteration in range(24):
        current=[alias_rng.choice([-1,-1,1,17,99]) for _ in range(47)]
        first=[alias_rng.randint(-1,100) for _ in range(47)]
        counts=[alias_rng.randrange(11) for _ in range(47)]
        if iteration<18:current[kind]=-1
        if iteration<4:current=[-1]*47
        next_slot=alias_rng.choice([1,50,99,100,101,127])
        alias_builder_counts=[alias_rng.choice([0,1,10]) for _ in range(47)]
        resource_counts=[alias_rng.choice([0,1,2]) for _ in range(2)]
        sizes=[[alias_rng.randint(-2**31,2**31-1) for _ in range(2)] for _ in range(100)]
        radii=[alias_rng.randint(-32768,32767) for _ in range(100)]
        requests=alias_rng.choice([0,1,32767,-32768]);flag=alias_rng.randint(-32768,32767)
        for offset,values in [(-0x25ee,current),(-0x261e,first),(-0x264e,counts)]:
            u.mem_write(a5+offset,bytes(v&255 for v in values))
        for index,(width,length) in enumerate(sizes):
            u.mem_write(a5-0x2572+4*index,struct.pack('>i',width))
            u.mem_write(a5-0x23e2+4*index,struct.pack('>i',length))
        u.mem_write(a5-0x2252,struct.pack('>100h',*radii))
        u.mem_write(a5-0x3ce8,struct.pack('>h',requests));u.mem_write(a5-0x3cee,struct.pack('>h',flag))
        u.mem_write(a5-0x25be,struct.pack('>h',next_slot));loader_events.clear()
        run(0xc5a80,bytes([kind,0]),instruction_limit=10000)
        after={name:list(struct.unpack('>47b',u.mem_read(a5+offset,47))) for name,offset in [('current',-0x25ee),('first',-0x261e),('counts',-0x264e)]}
        after.update(next=struct.unpack('>h',u.mem_read(a5-0x25be,2))[0],calls=loader_events.copy(),
                     requests=struct.unpack('>h',u.mem_read(a5-0x3ce8,2))[0],flag=struct.unpack('>h',u.mem_read(a5-0x3cee,2))[0],
                     sizes=[list(pair) for pair in zip(struct.unpack('>100i',u.mem_read(a5-0x2572,400)),struct.unpack('>100i',u.mem_read(a5-0x23e2,400)))],
                     radii=list(struct.unpack('>100h',u.mem_read(a5-0x2252,200))))
        cases.append(dict(routine='model_dispatch',a=kind,b=next_slot,current=current,first=first,counts=counts,
                          conditional='supplied selected-model loader',requests=requests,flag=flag,sizes=sizes,radii=radii,
                          builder_counts=alias_builder_counts.copy(),resource_counts=resource_counts.copy(),after=after,expected=0))
for hook in hooks:u.hook_del(hook)
# Complete selected-model loader. Resource Manager and inner builder results
# are supplied; reservation, display predicate, dispatch, and completion execute.
selected_state={}; selected_calls=[]
def selected_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0x121234:
        assert bytes(cpu.mem_read(sp,6))==struct.pack('>h4s',10000,b'shps')
        cpu.mem_write(sp+6,struct.pack('>I',0x46f00 if selected_state['present'] else 0))
        cpu.reg_write(UC_M68K_REG_A7,sp+6)
        cpu.reg_write(UC_M68K_REG_PC,address+2)
    elif address in [0x12128e,0x121292]:
        cpu.reg_write(UC_M68K_REG_PC,address+2)
    else:
        ret,slot,selector,kind=struct.unpack('>IhhB',cpu.mem_read(sp,9))
        selected_calls.append([address-0x120000,selector,slot,kind])
        cpu.mem_write(sp+10,struct.pack('>h',selected_state['loaded']))
        cpu.reg_write(UC_M68K_REG_A7,sp+10);cpu.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0xc2a,bytes.fromhex('4ef9')+struct.pack('>I',0xc2358))
u.mem_write(a5+0xcba,bytes.fromhex('4ef9')+struct.pack('>I',0xc3f3c))
hooks=[u.hook_add(UC_HOOK_CODE,selected_service,begin=p,end=p) for p in
       [0x121234,0x12128e,0x121292,0x12165e,0x12133a,0x121434,0x12150c,0x1215ba]]
selected_rng=random.Random(0x1206)
selectors=[-32768,-1,0,1,999,1000,1999,2000,2999,3000,3999,4000,9999,10000,10999,11000,32767]
for iteration in range(600):
    kind=iteration%47
    rows=[[selected_rng.choice(selectors) for _ in range(3)] for _ in range(47)]
    current=[-1]*47;first=[-1]*47;counts=[selected_rng.randrange(11) for _ in range(47)]
    if iteration%5==0:current[kind]=selected_rng.choice([1,17,99])
    next_slot=selected_rng.choice([-1,0,1,99,100,101,127,32767])
    selected_state=dict(present=iteration%7!=0,alternate=bool(iteration%2),loaded=selected_rng.choice([-32768,-1,0,1,10,32767]))
    display=bool((iteration//2)%2)
    u.mem_write(a5-0x7be5,bytes([int(selected_state['alternate'])]))
    for offset,values in [(-0x25ee,current),(-0x261e,first),(-0x264e,counts)]:u.mem_write(a5+offset,bytes(v&255 for v in values))
    u.mem_write(a5-0x25be,struct.pack('>h',next_slot));u.mem_write(a5-0x7be2,bytes([int(display)]))
    u.mem_write(0x46f00,struct.pack('>I',0x47000))
    u.mem_write(0x47000,struct.pack('>h',47)+b''.join(struct.pack('>hhh',*r) for r in rows))
    selected_calls.clear()
    run(0x121206,struct.pack('>IBx',a5-0x25be,kind))
    after={name:list(struct.unpack('>47b',u.mem_read(a5+offset,47))) for name,offset in [('current',-0x25ee),('first',-0x261e),('counts',-0x264e)]}
    after.update(next=struct.unpack('>h',u.mem_read(a5-0x25be,2))[0],calls=selected_calls.copy())
    cases.append(dict(routine='model_selected',a=kind,b=next_slot,current=current,first=first,counts=counts,rows=rows,display=display,
                      conditional='supplied inner model builders',after=after,expected=0,**selected_state))
for hook in hooks:u.hook_del(hook)

# Full animation loader with Resource Manager lookup and inner builders supplied.
# In particular, unsupported IDs preserve the previous D5 builder result.
animation_state={}; animation_calls=[]
def animation_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0x1215d6:
        assert bytes(cpu.mem_read(sp,6))==struct.pack('>h4s',animation_state['resource'],b'ashp')
        cpu.mem_write(sp+6,struct.pack('>I',0x46f00 if animation_state['present'] else 0))
        cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,address+2)
    else:
        ret,slot,selector,kind=struct.unpack('>IhhB',cpu.mem_read(sp,9))
        count=animation_state['returns'][len(animation_calls)]
        animation_calls.append([address-0x120000,selector,slot,kind])
        cpu.mem_write(sp+10,struct.pack('>h',count))
        cpu.reg_write(UC_M68K_REG_A7,sp+10);cpu.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,animation_service,begin=p,end=p) for p in [0x1215d6,0x12165e,0x12133a]]
animation_rng=random.Random(0x15ba)
animation_cases=[(3000,None),(3000,[]),(3000,[0,0]),(3000,[10000,0,10001,-1]),(3000,[1,999,10000,10999,1000,11000])]
for campaign in ['central','eastern','south']:
    for path in sorted((root/f'.local/{campaign}-shape-resources/61736870').glob('*.bin')):
        payload=path.read_bytes(); count=struct.unpack_from('>h',payload)[0]
        animation_cases.append((int(path.stem),list(struct.unpack_from(f'>{count}h',payload,2))))
animation_cases += [(3000,[animation_rng.choice([0,-1,1,21,999,1000,3999,10000,10800,10999,11000])
                              for _ in range(animation_rng.randrange(1,25))]) for _ in range(250)]
for resource,frames in animation_cases:
    base=animation_rng.choice([-32768,-1,0,1,99,32760]); previous=animation_rng.choice([-32768,-1,0,1,32767])
    kind=animation_rng.randrange(47); rows=frames or []
    returns=[animation_rng.choice([-32768,-1,0,1,3,32767]) for _ in rows]
    animation_state=dict(resource=resource,present=frames is not None,returns=returns)
    u.mem_write(0x46f00,struct.pack('>I',0x47000))
    u.mem_write(0x47000,struct.pack('>h',len(rows))+b''.join(struct.pack('>h',v) for v in rows))
    u.reg_write(UC_M68K_REG_D5,previous&65535);animation_calls.clear()
    run(0x1215ba,struct.pack('>hhBx',base,resource,kind))
    total=struct.unpack('>h',u.mem_read(stack+10,2))[0]
    assert u.reg_read(UC_M68K_REG_D5)==previous&65535
    cases.append(dict(routine='model_animation',a=base,b=previous,kind=kind,frames=rows,present=frames is not None,
                      returns=returns,calls=animation_calls.copy(),expected=total,conditional='supplied inner model builders'))
for hook in hooks:u.hook_del(hook)

# Full radius arithmetic and slot finalization, with only FixDiv modeled.
def model_fixdiv(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    denominator,numerator=struct.unpack('>ii',cpu.mem_read(sp,8))
    if denominator==0: result=-2**31 if numerator<0 else 2**31-1
    else:
        result=abs(numerator*65536)//abs(denominator)
        if (numerator<0)!=(denominator<0):result=-result
        result=max(-2**31,min(2**31-1,result))
    cpu.mem_write(sp+8,struct.pack('>i',result))
    cpu.reg_write(UC_M68K_REG_A7,sp+8);cpu.reg_write(UC_M68K_REG_PC,address+2)
hook=u.hook_add(UC_HOOK_CODE,model_fixdiv,begin=0x100012,end=0x100012)
radius_rng=random.Random(0xc4)
for value in [-2**31,-1,0,1,2,32767,32768,65535,65536,131072,131073,2**31-1]+[radius_rng.randrange(1,2**31) for _ in range(250)]:
    result=run(0x1000c4,struct.pack('>i',value))
    cases.append(dict(routine='runtime_sqrt',a=value,b=0,expected=result,conditional='modeled FixDiv'))
radius_vertices=vertex_lists.copy()
for campaign in ['central','eastern','south']:
    for path in sorted((root/f'.local/{campaign}-shape-resources/73686170').glob('*.bin')):
        payload=path.read_bytes();count=(struct.unpack_from('>H',payload)[0]+1)&65535
        vertices=[]
        for i in range(min(count,60)):
            coords=struct.unpack_from('>iii',payload,2+12*i)
            vertices.append([((((v*8+32768)>>16)+32768)&65535)-32768 for v in coords])
        radius_vertices.append(vertices)
u.mem_write(a5+0x7e2,bytes.fromhex('4ef9')+struct.pack('>I',0x100944))
for vertices in radius_vertices:
    slot=radius_rng.randrange(1,101)
    u.mem_write(0x47000,struct.pack('>h',len(vertices))+b''.join(struct.pack('>hhhhh',*v,0,0) for v in vertices))
    u.mem_write(a5-0x6490,struct.pack('>h',1));u.mem_write(a5-0x648e,struct.pack('>I',0x47000))
    for offset,size in [(-0x2572,400),(-0x23e2,400),(-0x2252,200)]:u.mem_write(a5+offset,bytes([0x55])*size)
    run(0xc3f64,struct.pack('>h',slot),instruction_limit=500000)
    dims=[struct.unpack('>i',u.mem_read(a5+offset+4*(slot-1),4))[0] for offset in [-0x2572,-0x23e2]]
    radius=struct.unpack('>h',u.mem_read(a5-0x2252+2*(slot-1),2))[0]
    for offset,stride in [(-0x2572,4),(-0x23e2,4),(-0x2252,2)]:
        memory=bytes(u.mem_read(a5+offset,100*stride))
        assert memory[:(slot-1)*stride]==bytes([0x55])*((slot-1)*stride)
        assert memory[slot*stride:]==bytes([0x55])*((100-slot)*stride)
    assert u.mem_read(a5-0x6490,2)==bytes(2)
    cases.append(dict(routine='collision_radius',a=slot,b=0,vertices=vertices,dimensions=dims,expected=radius,conditional='modeled FixDiv'))
u.hook_del(hook)

# Complete resource vertex loop, packing, and planar dimensions, composed.
# Runtime allocation is supplied; FixRound and FixDiv are modeled. Original slot finalization executes.
u.mem_write(a5+0x7a2,bytes.fromhex('4ef9')+struct.pack('>I',0x100532))
hooks=[u.hook_add(UC_HOOK_CODE,model_fixround,begin=p,end=p) for p in [0x1005a6,0x1005c0,0x1005da]]
hooks.append(u.hook_add(UC_HOOK_CODE,model_fixdiv,begin=0x100012,end=0x100012))
resources=[]
for campaign in ['central','eastern','south']:
    for path in sorted((root/f'.local/{campaign}-shape-resources/73686170').glob('*.bin')):
        resources.append((f'{campaign}/{path.stem}',path.read_bytes()))
for count in [0,1,59,60,61,65]:
    payload=struct.pack('>H',(count-1)&65535)+b''.join(struct.pack('>iii',i*65536+4096,-4096,-i*65536-4096) for i in range(count))
    resources.append((f'capacity-{count}',payload))
for name,payload in resources:
    u.mem_write(0x47000,bytes(2052));u.mem_write(0x46000,payload)
    u.mem_write(a5-0x6490,struct.pack('>h',1));u.mem_write(a5-0x6494,struct.pack('>i',65536))
    u.mem_write(a5-0x648e,struct.pack('>I',0x47000));u.mem_write(a5-0x7b99,b'\0')
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack-0x100)
    u.reg_write(UC_M68K_REG_A7,stack-0x1000);u.reg_write(UC_M68K_REG_A3,0x46000)
    u.emu_start(0x1216c0,0x121724,count=200000)
    assert u.reg_read(UC_M68K_REG_PC)==0x121724
    count=struct.unpack('>h',u.mem_read(0x47000,2))[0]
    packed=[list(struct.unpack('>hhh',u.mem_read(0x47002+10*i,6))) for i in range(count)]
    run(0xc3f64,struct.pack('>h',1),instruction_limit=500000)
    dimensions=[struct.unpack('>i',u.mem_read(a5+offset,4))[0] for offset in [-0x2572,-0x23e2]]
    radius=struct.unpack('>h',u.mem_read(a5-0x2252,2))[0]
    cases.append(dict(routine='resource_vertices',a=0,b=0,source=name,payload=list(payload),packed=packed,dimensions=dimensions,
                      conditional='modeled FixRound and FixDiv',radius=radius,expected=0))
for hook in hooks:u.hook_del(hook)
print(f'{len(resources)} resource vertex pipelines compared; modeled FixRound/FixDiv, original slot finalizer')
# Selector-3 column: original vertex loop and original polygon argument lists.
# Skip initialization/radar; packed vertex insertion executes with modeled FixRound.
u.mem_write(a5+0x792,bytes.fromhex('4ef9')+struct.pack('>I',0x100140))
u.mem_write(a5+0x7c2,bytes.fromhex('4ef9')+struct.pack('>I',0x100708))
column_faces=[]
def capture_column_face(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    count=struct.unpack('>h',uc.mem_read(sp+22,2))[0]
    column_faces.append([struct.unpack('>h',uc.mem_read(sp+20-2*i,2))[0]-1 for i in range(count)])
    uc.reg_write(UC_M68K_REG_A7,sp+24);uc.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,model_fixround,begin=p,end=p) for p in [0x1005a6,0x1005c0,0x1005da]]
hooks.append(u.hook_add(UC_HOOK_CODE,capture_column_face,begin=0x100708,end=0x100708))
column_tables=[json.loads((root/'.local/disassembly/direction-table-candidate.json').read_text())['base']]
column_tables += [[alias_rng.randint(-65536,65536) for _ in range(360)] for _ in range(20)]
for column_table in column_tables:
    u.mem_write(a5-0x38b6,struct.pack('>I',0x43000));u.mem_write(0x43000,struct.pack('>360i',*column_table))
    u.mem_write(0x47000,bytes(2052));u.mem_write(a5-0x6490,struct.pack('>h',1))
    u.mem_write(a5-0x6494,struct.pack('>i',65536));u.mem_write(a5-0x648e,struct.pack('>I',0x47000))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack-0x100);u.reg_write(UC_M68K_REG_A7,stack-0x1000)
    u.emu_start(0xc3342,0xc33be,count=50000)
    assert u.reg_read(UC_M68K_REG_PC)==0xc33be and u.mem_read(0x47000,2)==b'\0\x10'
    packed=[list(struct.unpack('>hhh',u.mem_read(0x47002+10*i,6))) for i in range(16)]
    column_faces.clear()
    u.emu_start(0xc3472,0xc35d2,count=5000)
    assert u.reg_read(UC_M68K_REG_PC)==0xc35d2 and len(column_faces)==10
    cases.append(dict(routine='procedural_column',a=0,b=0,table=column_table,packed=packed,faces=column_faces.copy(),conditional='modeled FixRound',expected=0))
for hook in hooks:u.hook_del(hook)
# Original shared box helper plus selectors 1 and 5. Face/edge calls are captured.
u.mem_write(a5+0xbf2,bytes.fromhex('4ef9')+struct.pack('>I',0x210ac))
u.mem_write(a5+0xbfa,bytes.fromhex('4ef9')+struct.pack('>I',0x211fa))
u.mem_write(a5+0x7aa,bytes.fromhex('4ef9')+struct.pack('>I',0x10069c))
def capture_obstacle_edge(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    column_faces.append([struct.unpack('>h',uc.mem_read(sp+offset,2))[0]-1 for offset in [8,6]])
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,model_fixround,begin=p,end=p) for p in [0x1005a6,0x1005c0,0x1005da]]
hooks += [u.hook_add(UC_HOOK_CODE,capture_column_face,begin=0x100708,end=0x100708),
          u.hook_add(UC_HOOK_CODE,capture_obstacle_edge,begin=0x10069c,end=0x10069c)]
def initialize_packed_model():
    u.mem_write(0x47000,bytes(2052));u.mem_write(a5-0x6490,struct.pack('>h',1))
    u.mem_write(a5-0x6494,struct.pack('>i',65536));u.mem_write(a5-0x648e,struct.pack('>I',0x47000))
    u.mem_write(a5-0x7b99,b'\0')
def read_packed_model():
    count=struct.unpack('>h',u.mem_read(0x47000,2))[0]
    return [list(struct.unpack('>hhh',u.mem_read(0x47002+10*i,6))) for i in range(count)]
box_arguments=[[v<<16 for v in values] for values in [[0,0,18,18,18,0,9,0],[3,3,4,24,4,0,12,0]]]
box_arguments += [[alias_rng.randint(-100*65536,100*65536) for _ in range(8)] for _ in range(200)]
for arguments in box_arguments:
    initialize_packed_model();run(0x210ac,struct.pack('>8i',*arguments))
    packed=read_packed_model();column_faces.clear();run(0x211fa,struct.pack('>hh',0,0))
    cases.append(dict(routine='procedural_box',a=0,b=0,arguments=arguments,packed=packed,faces=column_faces.copy(),conditional='modeled FixRound',expected=0))
for selector,filled in [(1,0),(5,0),(5,1)]:
    initialize_packed_model();u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack-0x100);u.reg_write(UC_M68K_REG_A7,stack-0x1000)
    start,end=(0xc246e,0xc2490) if selector==1 else (0xc1e38,0xc1f26)
    u.emu_start(start,end,count=20000);assert u.reg_read(UC_M68K_REG_PC)==end
    packed=read_packed_model();column_faces.clear()
    u.mem_write(a5-0x7be2,bytes([filled]))
    start,end=(0xc257e,0xc258c) if selector==1 else (0xc2024,0xc213e)
    u.emu_start(start,end,count=10000);assert u.reg_read(UC_M68K_REG_PC)==end
    cases.append(dict(routine='fixed_obstacle',a=selector,b=filled,packed=packed,faces=column_faces.copy(),conditional='modeled FixRound',expected=0))
# Flag base mesh: run original vertex insertion and face/edge calls.
hooks.append(u.hook_add(UC_HOOK_CODE,model_fixdiv,begin=0x100012,end=0x100012))
# Decorative letter meshes and radar lines do not alter the planar extrema.
for filled in [0,1]:
    initialize_packed_model()
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack-0x100);u.reg_write(UC_M68K_REG_A7,stack-0x1000)
    u.mem_write(a5-0x7be2,bytes([filled]))
    column_faces.clear()
    u.emu_start(0xc1014,0xc10f0,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0xc10f0
    packed=read_packed_model()
    run(0xc3f64,struct.pack('>h',1),instruction_limit=500000)
    dimensions=[struct.unpack('>i',u.mem_read(a5+offset,4))[0] for offset in [-0x2572,-0x23e2]]
    radius=struct.unpack('>h',u.mem_read(a5-0x2252,2))[0]
    cases.append(dict(routine='flag_mesh',a=filled,b=0,packed=packed,faces=column_faces.copy(),
                      dimensions=dimensions,radius=radius,conditional='modeled FixRound and FixDiv',expected=0))
# Full guarded copy of both dimensions AND radius, including missing slots.
for index in range(300):
    destination=override_rng.choice([-1,2,3,50,100])
    origin=override_rng.choice([-1,1,2,49,99])
    u.mem_write(a5-0x25ee+12,struct.pack('>b',destination))
    u.mem_write(a5-0x25ee+2,struct.pack('>b',origin))
    dimensions=[[override_rng.randint(-2**31,2**31-1) for _ in range(2)] for _ in range(100)]
    radii=[override_rng.randint(-32768,32767) for _ in range(100)]
    for axis,base in enumerate([a5-0x2572,a5-0x23e2]):
        u.mem_write(base,struct.pack('>100i',*[v[axis] for v in dimensions]))
    u.mem_write(a5-0x2252,struct.pack('>100h',*radii))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0xc14ac,0xc152a,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0xc152a
    after=[[struct.unpack('>i',u.mem_read(base+4*i,4))[0] for base in [a5-0x2572,a5-0x23e2]] for i in range(100)]
    after_radii=list(struct.unpack('>100h',u.mem_read(a5-0x2252,200)))
    cases.append(dict(routine='flag_collision_copy',a=destination,b=origin,dimensions=dimensions,radii=radii,
                      after=after,after_radii=after_radii,expected=0))
for hook in hooks:u.hook_del(hook)
# Original world placement conversion, including its actual integer multiply helper.
# No resource manager, model loader, or rendering behavior is substituted here.
world_inputs=[]
for campaign in ['central','eastern','south']:
    for path in sorted((root/f'.local/{campaign}-shape-resources/77726c64').glob('*.bin')):
        data=path.read_bytes(); count=struct.unpack_from('>H',data)[0]
        for index in range(count):
            kind,v,h,facing=struct.unpack_from('>hhhh',data,2+index*8)
            world_inputs.append((f'{campaign}/{path.stem}/{index}',h,v,facing,1024))
world_rng=random.Random(250)
for i in range(400):
    world_inputs.append((f'synthetic/{i}',*[world_rng.randint(-32768,32767) for _ in range(3)],world_rng.choice([1,64,512,1024,32767])))
# Earlier harness sections may change jump stubs: link this helper explicitly.
u.mem_write(a5+0x1282,bytes.fromhex('4ef9')+struct.pack('>I',0x90004))
for name,h,v,facing,modulus in world_inputs:
    u.mem_write(0x46000,struct.pack('>Hhhhh',1,0,v,h,facing))
    u.mem_write(0x42000,bytes([0x55])*128)
    u.mem_write(a5-0x137e,struct.pack('>h',modulus))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x46000)
    u.reg_write(UC_M68K_REG_A3,0x42000);u.reg_write(UC_M68K_REG_D7,1)
    u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0xb0b60,0xb0bc4,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0xb0bc4
    pose=list(struct.unpack('>iii',u.mem_read(0x42000,12)))
    assert bytes(u.mem_read(0x4200c,116))==bytes([0x55])*116
    cases.append(dict(routine='world_placement',a=h,b=v,facing=facing,modulus=modulus,pose=pose,source=name,expected=0))
print(f'{len(world_inputs)} original world placement conversions executed')
# Flow Field arithmetic, entered after the original contact/direction selection.
# Executes original CODE 7 instructions and original CODE 36 multiply/divide.
for displacement,target in [(0x1282,0x90004),(0x128a,0x90038)]:
    u.mem_write(a5+displacement, bytes.fromhex('4ef9')+struct.pack('>I',target))
flow_rng=random.Random(71394)
for index in range(400):
    position=[flow_rng.randint(-2**31,2**31-1) for _ in range(2)]
    direction=[flow_rng.randint(-65536,65536) for _ in range(2)]
    now=flow_rng.randint(-2**31,2**31-1)
    previous=now-flow_rng.choice([0,1,2,3,4,63,1000])
    previous=((previous+2**31)%2**32)-2**31
    if index>=300:
        direction=[flow_rng.randint(-2**31,2**31-1) for _ in range(2)]
        previous=flow_rng.randint(-2**31,2**31-1)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x100)
    u.mem_write(0x42000,struct.pack('>ii',*position)+bytes([0x55])*120)
    u.mem_write(stack-4,struct.pack('>i',direction[0]));u.mem_write(stack-8,struct.pack('>i',direction[1]))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x3d28,struct.pack('>i',previous))
    u.emu_start(0x11426,0x11464,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x11464
    after=list(struct.unpack('>ii',u.mem_read(0x42000,8)))
    assert bytes(u.mem_read(0x42008,120))==bytes([0x55])*120
    cases.append(dict(routine='flow_displacement',a=now,b=previous,position=position,direction=direction,after=after,expected=0))
print('400 original Flow Field displacement blocks executed')
# Complete object-pair overlap: original model slot lookup and rectangle routines.
u.mem_write(a5+0x4da,bytes.fromhex('4ef9')+struct.pack('>I',0x718ea))
for index,(cosine,sine) in enumerate(collision_table):
    u.mem_write(a5-0x38b2+index*4,struct.pack('>i',cosine))
    u.mem_write(a5-0x3312+index*4,struct.pack('>i',sine))
pair_rng=random.Random(0x2ce)
pair_hits=0
for iteration in range(600):
    kinds=pair_rng.sample([5,6,7,12,16,17,18,23,24,36,39,40],2)
    records=[bytearray(128),bytearray(128)]
    pos=[pair_rng.randrange(100,900)<<16 for _ in range(2)]
    for i,r in enumerate(records):
        delta=[pair_rng.choice([0,1,20,60,107,108,-1,-20,-60])<<16 for _ in range(2)] if i else [0,0]
        struct.pack_into('>iii',r,0,*[pos[j]+delta[j] for j in range(2)],pair_rng.randrange(360)<<16)
        struct.pack_into('>i',r,0x28,pair_rng.choice([0,0,59,60,61,139,140,141,-60])<<16)
        r[0x14]=kinds[i]
    dimensions=[pair_rng.randrange(2,100)<<16 for _ in range(4)]
    radii=[pair_rng.randrange(1,80) for _ in range(2)]
    if iteration>=500:
        for r in records:
            for offset in [0,4,0x28]:struct.pack_into('>i',r,offset,pair_rng.randint(-2**31,2**31-1))
        radii=[pair_rng.randint(-32768,32767) for _ in range(2)]
    slots=bytearray([1]*48);slots[kinds[1]]=2
    u.mem_write(a5-0x25ee,bytes(slots))
    u.mem_write(a5-0x2252,struct.pack('>hh',*radii))
    # +2ce passes second model dimensions first to +250 (stack argument order).
    u.mem_write(a5-0x2572,struct.pack('>ii',dimensions[0],dimensions[2]))
    u.mem_write(a5-0x23e2,struct.pack('>ii',dimensions[1],dimensions[3]))
    u.mem_write(0x42000,bytes(records[0]));u.mem_write(0x42100,bytes(records[1]))
    run(0x102ce,struct.pack('>II',0x42100,0x42000))
    result=u.mem_read(stack+12,1)[0]
    pair_hits+=result
    cases.append(dict(routine='object_pair_overlap',a=0,b=0,records=[list(r) for r in records],dimensions=dimensions,radii=radii,expected=result))
print(f'600 complete object-pair overlap routines executed: {pair_hits} hits')
# Original fixed-point prefilter, including fractional edge cases and overflow.
contact_rng=random.Random(0xd90)
for iteration in range(500):
    first=[contact_rng.randint(-2**31,2**31-1) for _ in range(2)]
    delta=contact_rng.choice([0,1,0x6bffff,0x6c0000,0x6c0001,-0x6c0000])
    second=[((first[0]+delta+2**31)%2**32)-2**31,first[1]]
    if iteration>=250:second=[contact_rng.randint(-2**31,2**31-1) for _ in range(2)]
    u.mem_write(0x42000,struct.pack('>ii',*first));u.mem_write(0x42100,struct.pack('>ii',*second))
    run(0xe3cba,struct.pack('>II',0x42000,0x42100))
    cases.append(dict(routine='contact_near',a=0,b=0,first=first,second=second,expected=u.mem_read(stack+12,1)[0]))
# Post-overlap selection block. Real table dispatch, no replaced instructions.
for moving,candidate in [(a,b) for a in range(48) for b in range(48)]+[(255,255),(128,6),(6,128)]:
    for populated in [False,True]:
        before=[11,12] if populated else [None,None]
        same_owner=bool(contact_rng.randrange(2))
        u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack)
        u.reg_write(UC_M68K_REG_A4,0x42000);u.reg_write(UC_M68K_REG_A3,0x44000)
        u.reg_write(UC_M68K_REG_D7,1);u.reg_write(UC_M68K_REG_A7,stack-0x100)
        u.mem_write(0x42000,bytes(128));u.mem_write(0x42100,bytes(128))
        u.mem_write(0x42014,bytes([candidate]));u.mem_write(0x42114,bytes([moving]))
        u.mem_write(0x4201a,bytes([1 if same_owner else 2]));u.mem_write(stack+0x24,b'\x01')
        u.mem_write(0x44000,struct.pack('>hh',1,37))
        for offset,ptr in [(8,0x46002),(0x10,0x42100),(0x14,0x46000),(0x18,0x46004),(0x1c,0x46005)]:
            u.mem_write(stack+offset,struct.pack('>I',ptr))
        u.mem_write(0x46000,struct.pack('>hhBB',11,12,int(populated),int(populated)))
        u.emu_start(0x10f3a,0x1104e,count=1000)
        assert u.reg_read(UC_M68K_REG_PC)==0x1104e
        ids=struct.unpack('>hh',u.mem_read(0x46000,4));flags=u.mem_read(0x46004,2)
        after=[ids[n] if flags[n] else None for n in range(2)]
        cases.append(dict(routine='contact_select',a=moving,b=candidate,before=before,after=after,same_owner=same_owner,expected=0))
print('500 fixed-point prefilters and 4614 contact-selection blocks executed')
# Complete participant contact query, original grid bounds and all overlap calls.
for disp,target in [(0x762,0xe3cba),(0x4da,0x718ea),(0x452,0x7002c),(0x45a,0x7003c),(0x1282,0x90004),(0xbca,0x20dcc)]:
    u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
scan_rng=random.Random(0xd901)
contact_counts=[0,0]
for iteration in range(400):
    records=[bytearray(128) for _ in range(16)]
    pose=[scan_rng.randrange(1,1024)<<16,scan_rng.randrange(1,1024)<<16,scan_rng.randrange(360)<<16]
    if iteration<12:pose[:2]=[[0,64<<16],[1024<<16,64<<16],[-1,64<<16],[1,1]][iteration%4]
    cached=[max(0,min(15,v>>22)) for v in pose[:2]]
    if iteration%7==0:cached=[scan_rng.randrange(16),scan_rng.randrange(16)]
    struct.pack_into('>iii',records[0],0,*pose);records[0][0x14]=scan_rng.choice([6,7,8,13,20,21,22,23,24,25,26,27,31,35,45]);records[0][0x1a]=1
    slots=[1+k%6 for k in range(48)];sizes=[[scan_rng.randrange(8,81)<<16,scan_rng.randrange(8,81)<<16] for _ in range(6)];radii=[scan_rng.randrange(20,80) for _ in range(6)]
    states=[scan_rng.choice([0,0,0,3])]+[0]*11
    grid=[[] for _ in range(256)]
    for j,r in enumerate(records):
        if j:
            xy=[pose[axis]+(scan_rng.choice([-80,-30,-1,0,0,1,30,80])<<16) for axis in range(2)]
            struct.pack_into('>iii',r,0,*xy,scan_rng.randrange(360)<<16)
            struct.pack_into('>i',r,0x28,scan_rng.choice([0,0,0,60,61,140])<<16)
            r[0x14]=scan_rng.randrange(48);r[0x1a]=scan_rng.choice([0,0,1,2])
        x,z=struct.unpack_from('>ii',r)
        cell=max(0,min(15,x>>22))*16+max(0,min(15,z>>22));grid[cell].append(j+1)
    for cell in grid:
        if cell and len(cell)<20:cell.append(cell[0]);scan_rng.shuffle(cell)
    u.mem_write(0x42000,b''.join(bytes(r) for r in records))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    u.mem_write(0x45000,bytes(256));u.mem_write(0x45004,bytes([states[0]]));u.mem_write(0x45056,struct.pack('>h',1));u.mem_write(0x45060,struct.pack('>hh',*cached))
    u.mem_write(a5-0x1380,struct.pack('>h',16));u.mem_write(a5-0x137e,struct.pack('>h',1024))
    u.mem_write(a5-0x25ee,bytes(slots));u.mem_write(a5-0x2252,struct.pack('>6h',*radii))
    for j,size in enumerate(sizes):
        u.mem_write(a5-0x2572+j*4,struct.pack('>i',size[0]));u.mem_write(a5-0x23e2+j*4,struct.pack('>i',size[1]))
    u.mem_write(0x48000,b''.join(struct.pack('>21h',len(c),*c,*([0]*(20-len(c)))) for c in grid));u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    u.mem_write(0x46000,bytes(256))
    run(0x10d90,struct.pack('>IIIIIIIbB',0x46002,0x46008,0x46100,0x46000,0x46004,0x46005,0,1,0),instruction_limit=200000)
    ids=struct.unpack('>hh',u.mem_read(0x46000,4));flags=u.mem_read(0x46004,2)
    contacts=[ids[n] if flags[n] else None for n in range(2)]
    for n in range(2):contact_counts[n]+=int(flags[n]!=0)
    cases.append(dict(routine='participant_contacts',a=1,b=1,candidate_table=True,pose=pose,dimensions=[0,0],cached_cell=cached,radii=radii,slots=slots,sizes=sizes,states=states,records=[list(r) for r in records],grid=grid,contacts=contacts,expected=0))
print(f'400 complete participant contact scans executed: {contact_counts}')
# Original remaining-flag counter and refresh loop, no services replaced.
flag_rng=random.Random(0x21a)
for iteration in range(400):
    kinds=[flag_rng.choice([0,12,12,14,39]) for _ in range(127)]
    limit=iteration-128 if iteration<256 else flag_rng.randint(-128,127)
    records=[bytearray(128) for _ in kinds]
    for record,kind in zip(records,kinds):record[0x14]=kind
    u.mem_write(0x42000,b''.join(bytes(r) for r in records));u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(a5-0x12cf,struct.pack('>b',limit))
    run(0xd021a,b'')
    count=struct.unpack('>h',u.mem_read(stack+4,2))[0]
    cases.append(dict(routine='remaining_flags',a=limit,b=0,kinds=kinds,expected=count))
for disp,target in [(0x452,0x7002c)]:u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
for iteration in range(400):
    level=flag_rng.randint(1,100) if iteration<300 else flag_rng.randint(-32768,32767)
    objects=flag_rng.randint(6,100) if iteration<300 else flag_rng.randint(-32768,32767)
    kinds=[flag_rng.choice([0,12,14,14,39]) for _ in range(127)]
    records=[bytearray(128) for _ in kinds]
    for record,kind in zip(records,kinds):record[0x14]=kind
    u.mem_write(0x42000,b''.join(bytes(r) for r in records));u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    u.mem_write(a5-0x136e,struct.pack('>h',level));u.mem_write(a5-0x12c8,struct.pack('>h',objects))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x100)
    u.emu_start(0xb1f6c,0xb1fe8,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0xb1fe8
    after=[u.mem_read(0x42014+i*128,1)[0] for i in range(127)]
    count=struct.unpack('>b',u.mem_read(a5-0x12cf,1))[0]
    cases.append(dict(routine='refresh_flags',a=level,b=objects,kinds=kinds,after=after,expected=count))
print('400 original flag counts and 400 flag refresh loops executed')

# Original full threshold and score routines. Resource Manager operations are
# supplied services; threshold selection, score chunks, and life gates execute.
u.mem_write(0x30000,(source/'31.bin').read_bytes())
u.mem_write(a5+0x452,bytes.fromhex('4ef9')+struct.pack('>I',0x7002c))
score_resource=None
score_sounds=[]
score_hud=[]
def score_services(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==a5+0xeaa:
        ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
        cpu.mem_write(sp+4,struct.pack('>h',10000))
        cpu.reg_write(UC_M68K_REG_A7,sp+4);cpu.reg_write(UC_M68K_REG_PC,ret)
    elif address==0x30026:
        # GetResource: result handle follows six argument bytes.
        cpu.mem_write(sp+6,struct.pack('>I',0x41000 if score_resource is not None else 0))
        cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,address+2)
    elif address in [0x30032,0x30038,0x3008e]:
        cpu.reg_write(UC_M68K_REG_PC,address+2)
    else:
        ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
        if address==a5+0x972:
            score_sounds.append(list(struct.unpack('>hI',cpu.mem_read(sp+4,6))))
            consumed=6
        else:
            score_hud.append(cpu.mem_read(sp+4,1)[0]);consumed=2
        cpu.reg_write(UC_M68K_REG_A7,sp+4+consumed);cpu.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,score_services,begin=p,end=p)
       for p in [a5+0xeaa,0x30026,0x30032,0x30038,0x3008e,a5+0x972,a5+0x107a]]
score_resources=[None]+[p.read_bytes() for p in sorted((root/'.local/central-shape-resources/73737466').glob('*.bin'))]
# Synthetic positive repeat interval, retaining all original thresholds.
score_resources.append(score_resources[1][:-2]+struct.pack('>h',10000))
score_rng=random.Random(0x319e)
for resource_index,score_resource in enumerate(score_resources):
    u.mem_write(0x41000,struct.pack('>I',0x41100))
    if score_resource is not None:u.mem_write(0x41100,score_resource)
    inputs=[-1,0,4999,5000,5001,9999,10000,139999,140000,140001,180000,200000]
    # Above-last signed repeat behavior is expensive, so test that boundary
    # explicitly, then sample routine scoring within the first thresholds.
    for score in inputs:
        result=run(0x30004,struct.pack('>i',score),instruction_limit=4000000)
        cases.append(dict(routine='next_life',a=score,b=0,payload=list(score_resource) if score_resource else None,expected=result))
    for iteration in range(100):
        score=score_rng.randrange(0,80000)
        delta=score_rng.choice([-32768,-20,0,20,4999,5000,10000,32767])
        lives=score_rng.choice([-32768,0,3,32767])
        disabled=iteration%2
        participant=1+iteration%12
        records=bytearray(12*256);struct.pack_into('>i',records,(participant-1)*256,score)
        struct.pack_into('>h',records,(participant-1)*256+0x5c,lives)
        u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
        u.mem_write(a5-0x12ca,bytes([disabled]));u.mem_write(a5-0x133d,bytes([participant]))
        score_sounds.clear();score_hud.clear()
        run(0x3009e,struct.pack('>hbB',delta,participant,0),instruction_limit=4000000)
        actual=struct.unpack('>i',u.mem_read(0x42000+(participant-1)*256,4))[0]
        after_lives=struct.unpack('>h',u.mem_read(0x42000+(participant-1)*256+0x5c,2))[0]
        assert score_hud==[participant]
        assert all(sound==[1,0x1002a] for sound in score_sounds)
        cases.append(dict(routine='award_score',a=score,b=delta,lives=lives,disabled=bool(disabled),
                          payload=list(score_resource) if score_resource else None,
                          after_lives=after_lives,awards=len(score_sounds),expected=actual))
for hook in hooks:u.hook_del(hook)
print('60 threshold and 500 complete score-handler executions compared; resource/audio/HUD services supplied')

# Full score-resource selector with the original CODE 1 difficulty getter.
u.mem_write(a5+0xba,bytes.fromhex('4ef9')+struct.pack('>I',0x120890))
selection_inputs=[-32768,-1,0,1,2,3,4,5,6,7,8,32767]
for difficulty in selection_inputs:
    for mode_12d2 in [0,1,128,255]:
        for mode_12ca in [0,1,128,255]:
            u.mem_write(a5-0x4258,struct.pack('>h',difficulty))
            u.mem_write(a5-0x12d2,bytes([mode_12d2]))
            u.mem_write(a5-0x12ca,bytes([mode_12ca]))
            run(0xb1ab6,b'',instruction_limit=1000)
            resource_id=struct.unpack('>h',u.mem_read(stack+4,2))[0]
            cases.append(dict(routine='score_resource',a=difficulty,b=mode_12d2,mode=mode_12ca,expected=resource_id))
print('192 complete score resource selections executed with the original difficulty getter')

# Full local-player lifecycle dispatch, running original health/ammo helpers.
# Movie availability, visual transitions, placement, and control reset are
# explicit service fixtures; participant score and collected bytes are checked.
for disp,target in [(0xe0a,0xf0684),(0xe12,0xf06fc),(0x452,0x7002c)]:
    u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
death_calls=[]
def death_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    death_calls.append(address)
    consumed={a5+0xaa:2,a5+0xcea:0,a5+0x122a:4,a5+0x9ea:0,
              a5+0x4ea:0,a5+0xe3a:0,0x304b4:4}[address]
    if address==a5+0x9ea:cpu.mem_write(sp+4,b'\0\0')
    cpu.reg_write(UC_M68K_REG_A7,sp+4+consumed);cpu.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,death_service,begin=p,end=p)
       for p in [a5+0xaa,a5+0xcea,a5+0x122a,a5+0x9ea,a5+0x4ea,a5+0xe3a,0x304b4]]
death_rng=random.Random(0x1a80)
for iteration in range(400):
    participant=1+iteration%12;active=iteration%5!=0
    health=death_rng.choice([-32768,-1,0,1,6,32767])
    lives=death_rng.choice([-32768,-1,0,1,2,3,32767])
    ammo=death_rng.randint(-32768,32767)
    shields=death_rng.randint(-128,127);ammo_setting=death_rng.randint(-128,127)
    mode=death_rng.choice([0,0,0,1,128,255])
    records=bytearray(12*256);base=(participant-1)*256
    preserved=death_rng.randbytes(4);records[base:base+4]=preserved
    records[base+0x73:base+0x83]=bytes([1,0]*8)
    records[base+0x88]=int(active)
    struct.pack_into('>hhhh',records,base+0x56,1,0,health,lives)
    struct.pack_into('>h',records,base+0x5e,ammo)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,bytes(128));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x133d,bytes([participant]));u.mem_write(a5-0x12ca,bytes([mode]));u.mem_write(a5-0x12d2,b'\0')
    u.mem_write(a5-0x3f8e+(participant-1)*40+0x21,struct.pack('>bb',shields,ammo_setting))
    death_calls.clear()
    run(0x31a80,struct.pack('>bB',participant,0),instruction_limit=10000)
    after=struct.unpack('>hhh',u.mem_read(0x42000+base+0x5a,6))
    assert u.mem_read(0x42000+base,4)==preserved
    assert u.mem_read(0x42000+base+0x73,16)==bytes([1,0]*8)
    cases.append(dict(routine='local_respawn',a=health,b=lives,ammo=ammo,active=active,
                      shields=shields,ammo_setting=ammo_setting,mode=mode,after=list(after),
                      game_over=a5+0x122a in death_calls,respawn=0x304b4 in death_calls,expected=0))
for hook in hooks:u.hook_del(hook)
print('400 local lifecycle executions compared, with explicit movie/placement/UI service fixtures')

# Original post-damage scoring gate, through the actual score-call boundary.
# Capture the award argument; award_score itself is verified separately above.
combat_rng=random.Random(0xdca)
combat_inputs=[(kind,1,2,0,1,1,0) for kind in range(256)]
combat_inputs += [(7,1,2,0,1,1,mode) for mode in range(256)]
combat_inputs += [(combat_rng.randrange(256),combat_rng.randrange(0,13),combat_rng.randrange(1,13),
                   combat_rng.choice([0,1,2,128,255]),combat_rng.choice([0,1,2,128,255]),
                   combat_rng.choice([0,1,255]),combat_rng.choice([0,1,6,13,14,18,2,255])) for _ in range(300)]
combat_calls=[]
def capture_combat_award(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    combat_calls.append([struct.unpack('>h',cpu.mem_read(sp+4,2))[0],cpu.mem_read(sp+6,1)[0]])
    cpu.mem_write(sp+8,b'\0\0')
    cpu.reg_write(UC_M68K_REG_A7,sp+8);cpu.reg_write(UC_M68K_REG_PC,ret)
hook=u.hook_add(UC_HOOK_CODE,capture_combat_award,begin=0x3009e,end=0x3009e)
for kind,attacker,victim,af,vf,killed,mode in combat_inputs:
    if attacker==victim:af=vf
    score=combat_rng.randint(-2**31,2**31-1)
    participants=bytearray(12*256)
    if attacker:participants[(attacker-1)*256+5]=af
    participants[(victim-1)*256+5]=vf
    struct.pack_into('>h',participants,(victim-1)*256+0x56,1)
    u.mem_write(0x42000,bytes(participants));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    obj=bytearray(128);obj[0x14]=kind
    u.mem_write(0x44000,bytes(obj));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(0x45000,bytes(0x900));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    if kind<47:u.mem_write(0x45000+0x4c8+4*kind,struct.pack('>i',score))
    u.mem_write(a5-0x12ca,bytes([mode]))
    u.reg_write(UC_M68K_REG_D5,attacker);u.reg_write(UC_M68K_REG_D7,victim);u.reg_write(UC_M68K_REG_D6,killed)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x100)
    combat_calls.clear()
    u.emu_start(0x30caa,0x30de2,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x30de2
    count=sum(struct.unpack('>47h',u.mem_read(0x45000+0x69e,94)))
    assert len(combat_calls)<=1
    if combat_calls:assert combat_calls[0][1]==attacker
    cases.append(dict(routine='combat_score',a=kind,b=score,owners=[attacker,victim],flags=[af,vf],
                      killed=killed,mode=mode,award=combat_calls[0][0] if combat_calls else None,
                      counted=count==1,expected=0))
u.hook_del(hook)
print('812 original combat-score gates and award arguments compared')

# Original par time and all three completion ratings/bonus calculations.
# Run through +0xd44, before drawing and award presentation. No UI fixtures.
u.mem_map(0x150000,0x10000);u.mem_write(0x150000,(source/'21.bin').read_bytes())
for disp,target in [(0x1282,0x90004),(0x128a,0x90038)]:
    u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
rating_rng=random.Random(0x21b40)
for iteration in range(400):
    counts=[rating_rng.randrange(0,4) for _ in range(47)]
    multipliers=[rating_rng.randrange(0,3) for _ in range(47)]
    bonuses=[rating_rng.randrange(0,100) for _ in range(47)]
    flag_count=rating_rng.randrange(1,21)
    if iteration>=300:
        counts=[rating_rng.randint(-32768,32767) for _ in range(47)]
        multipliers=[rating_rng.randint(-32768,32767) for _ in range(47)]
        bonuses=[rating_rng.randint(-2**31,2**31-1) for _ in range(47)]
        flag_count=rating_rng.randint(-128,127)
    table_data=bytearray(0x900)
    for i in range(47):
        struct.pack_into('>h',table_data,2+2*i,counts[i])
        struct.pack_into('>h',table_data,0x60+2*i,multipliers[i])
        struct.pack_into('>i',table_data,0x584+4*i,bonuses[i])
    u.mem_write(0x45000,bytes(table_data));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(a5-0x12cf,struct.pack('>b',flag_count))
    run(0x150004,b'')
    par=struct.unpack('>h',u.mem_read(stack+4,2))[0]
    cases.append(dict(routine='arena_par',a=flag_count,b=0,counts=counts,multipliers=multipliers,bonuses=bonuses,expected=par))
    if par==0:continue
    start=rating_rng.randint(-100000,100000);end=start+rating_rng.randrange(0,200000)
    counters=[rating_rng.randint(-5,1000) for _ in range(4)]
    if iteration%5==0:counters[0]=0
    if iteration%7==0:counters[2]=0
    previous=[rating_rng.randint(-32768,32767) for _ in range(3)]
    u.mem_write(a5-0x12e6,struct.pack('>i',start));u.mem_write(a5-0x12e2,struct.pack('>i',end))
    u.mem_write(0x45000+0x7fc,struct.pack('>4i',*counters))
    u.mem_write(a5-0x3d84,struct.pack('>3h',*previous))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0x150b40,0x150d44,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x150d44
    result=dict(ratings=list(struct.unpack('>3h',u.mem_read(a5-0x3d84,6))),
                bars=list(struct.unpack('>3i',u.mem_read(a5-0x3d9c,12))),
                unrated=[bool(v) for v in u.mem_read(a5-0x3d7e,2)],
                bonuses=list(struct.unpack('>3i',u.mem_read(a5-0x3d90,12))))
    cases.append(dict(routine='completion_ratings',a=par,b=0,timestamps=[start,end],counters=counters,
                      previous=previous,result=result,expected=0))
print('400 original par-time calculations and completion-rating prefixes executed')

# Enemy resource rows through count selection and participant cap balancing.
# Resource locking/release and model loading are service boundaries. Random
# supplies signed words; original abs, remainder, indexing, and cap loop execute.
plan_rng=random.Random(0x251d7e)
plan_samples=[];plan_index=0
def plan_service(cpu,address,size,data):
    global plan_index
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0xb1e08:
        cpu.mem_write(sp,struct.pack('>h',plan_samples[plan_index]));plan_index+=1
        cpu.reg_write(UC_M68K_REG_PC,address+2)
    elif address in [0xb1e46,0xb1e4a]:
        if address==0xb1e4a:cpu.reg_write(UC_M68K_REG_A7,sp+4)
        cpu.reg_write(UC_M68K_REG_PC,address+2)
    else:
        ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
        cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0xb2,bytes.fromhex('4ef9')+struct.pack('>I',0x120882))
hooks=[u.hook_add(UC_HOOK_CODE,plan_service,begin=p,end=p) for p in [0xb1e08,0xb1e46,0xb1e4a,a5+0xd2a]]
plan_resources=[]
for campaign in ['central','eastern','south']:
    for path in sorted((root/f'.local/{campaign}-shape-resources/656e656d').glob('*.bin')):
        plan_resources.append((f'{campaign}/{path.stem}',path.read_bytes()))
plan_inputs=plan_resources+[(None,struct.pack('>h',3)+b''.join(struct.pack('>4h',kind,minimum,maximum,lives)
    for kind,minimum,maximum,lives in [(7,2,8,1),(31,1,4,2),(263,1,6,3)])) for _ in range(200)]
plan_count=0;plan_guard_count=0
for source_name,payload in plan_inputs:
    rows=[list(struct.unpack_from('>4h',payload,2+8*i)) for i in range(struct.unpack_from('>h',payload)[0])]
    if not rows:continue
    difficulty=plan_rng.randint(-100,127);reduced=bool(plan_rng.randrange(2))
    plan_samples=[plan_rng.randint(-32767,32767) for _ in rows];plan_index=0
    u.mem_write(0x41000,struct.pack('>I',0x41100));u.mem_write(0x41100,payload)
    u.mem_write(a5-0x4256,struct.pack('>h',difficulty));u.mem_write(a5-0x7be6,bytes([int(reduced)]))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000,bytes(0x900))
    u.reg_write(UC_M68K_REG_A3,0x41000);u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x100)
    u.emu_start(0xb1d7e,0xb1ee2,count=200000)
    finished=u.reg_read(UC_M68K_REG_PC)==0xb1ee2
    counts=list(struct.unpack('>47h',u.mem_read(0x45002,94)))
    lives=list(struct.unpack('>47h',u.mem_read(0x45060,94)))
    total=struct.unpack('>h',u.mem_read(0x45000,2))[0]
    if not finished:
        assert 0xb1e92<=u.reg_read(UC_M68K_REG_PC)<=0xb1ee0
        assert total>=(6 if reduced else 12) and all(v<=1 for v in counts)
        plan_guard_count+=1
    cases.append(dict(routine='enemy_plan',guarded=not finished,a=difficulty,b=int(reduced),rows=rows,random=plan_samples,
                      counts=counts,lives=lives,source=source_name,expected=total))
    plan_count+=1
for hook in hooks:u.hook_del(hook)
print(f'{plan_count} original enemy-plan fixtures, including {plan_guard_count} stalled cap loops guarded by the host; supplied Random/model/resource services')
# Complete original same-kind life transfer; no service fixtures or traps.
borrow_rng=random.Random(0x3119da)
for iteration in range(600):
    recipient=iteration%12
    flags=[[borrow_rng.choice([0,1,2,128,255]),borrow_rng.choice([0,1,2,128,255]),borrow_rng.choice([7,8,31])] for _ in range(12)]
    lives=[borrow_rng.choice([-32768,-1,0,1,2,3,32767]) for _ in range(12)]
    records=bytearray(12*256)
    for i in range(12):
        records[i*256+0x88]=flags[i][0];records[i*256+5]=flags[i][1];records[i*256+8]=flags[i][2]
        struct.pack_into('>h',records,i*256+0x5c,lives[i])
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    run(0x319da,struct.pack('>BB',recipient+1,0))
    returned=bool(u.mem_read(stack+6,1)[0])
    after=[struct.unpack('>h',u.mem_read(0x42000+i*256+0x5c,2))[0] for i in range(12)]
    changed=bytearray(u.mem_read(0x42000,len(records)))
    for i in range(12):changed[i*256+0x5c:i*256+0x5e]=records[i*256+0x5c:i*256+0x5e]
    assert changed==records
    cases.append(dict(routine='borrow_enemy_life',a=recipient,b=0,flags=flags,lives=lives,after=after,expected=int(returned)))
print('600 complete original same-kind enemy life transfers executed without service fixtures')
# Original post-death life consumption and retirement gate, calling the full
# transfer routine above. Stop before geometry removal or health refill.
def retirement_boundary(cpu,address,size,data):cpu.emu_stop()
# Explicit hooks also stop at boundaries inside previously translated blocks.
hooks=[u.hook_add(UC_HOOK_CODE,retirement_boundary,begin=p,end=p) for p in [0x31c0c,0x31cb8]]
for iteration in range(600):
    recipient=iteration%12;mode=borrow_rng.choice([0,0,0,1,128,255])
    flags=[[borrow_rng.choice([0,1,2,255]),borrow_rng.choice([0,1,2,255]),borrow_rng.choice([7,8,31])] for _ in range(12)]
    lives=[borrow_rng.choice([-32768,-1,0,1,2,3,32767]) for _ in range(12)]
    records=bytearray(12*256)
    for i in range(12):
        records[i*256+0x88]=flags[i][0];records[i*256+5]=flags[i][1];records[i*256+8]=flags[i][2]
        struct.pack_into('>h',records,i*256+0x5c,lives[i])
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x12ca,bytes([mode]))
    u.reg_write(UC_M68K_REG_A4,0x42000+recipient*256);u.reg_write(UC_M68K_REG_D7,recipient+1)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0x31bd0,0x31cb8,count=10000)
    pc=u.reg_read(UC_M68K_REG_PC);assert pc in [0x31c0c,0x31cb8]
    after=[struct.unpack('>h',u.mem_read(0x42000+i*256+0x5c,2))[0] for i in range(12)]
    cases.append(dict(routine='consume_enemy_life',a=recipient,b=mode,flags=flags,lives=lives,after=after,expected=int(pc==0x31c0c)))
for hook in hooks:u.hook_del(hook)
print('600 original enemy life consumption and retirement gates executed')

# Complete proposed-coordinate routine, with only the Random trap supplied.
spawn_sample=0
def spawn_random(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    cpu.mem_write(sp,struct.pack('>h',spawn_sample));cpu.reg_write(UC_M68K_REG_PC,address+2)
hooks=[u.hook_add(UC_HOOK_CODE,spawn_random,begin=p,end=p) for p in [0x301f0,0x3022a,0x30250]]
spawn_rng=random.Random(0x310194)
for iteration in range(768):
    kind=iteration%256-128;enemy=[0,1,255][iteration//256]
    extent=spawn_rng.choice([256,512,1024,2048,10000])
    spawn_sample=spawn_rng.choice([-32768,-1,0,1,32767,spawn_rng.randint(-32768,32767)])
    participant=iteration%12+1
    records=bytearray(12*256);base=(participant-1)*256
    records[base+5]=enemy;records[base+8]=kind&255
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x137e,struct.pack('>h',extent))
    result=run(0x30194,struct.pack('>BB',participant,0))
    cases.append(dict(routine='spawn_coordinate',a=extent,b=spawn_sample,kind=kind,enemy=enemy,expected=result))
for hook in hooks:u.hook_del(hook)
print('768 complete original spawn coordinate proposals executed with supplied Random words')
# Full normal-mode placement routine. Original coordinate/heading arithmetic
# and retry loop run; geometry commit and collision queries are service fixtures.
search_samples=[];search_sample_index=0;search_attempt=0;search_contacts=[]
def search_service(cpu,address,size,data):
    global search_sample_index,search_attempt
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address in [0x304e6,0x301f0,0x3022a,0x30250]:
        cpu.mem_write(sp,struct.pack('>h',search_samples[search_sample_index]));search_sample_index+=1
        cpu.reg_write(UC_M68K_REG_PC,address+2);return
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x1162:
        pose_ptr=struct.unpack('>I',cpu.mem_read(sp+4,4))[0]
        cpu.mem_write(0x44000,bytes(cpu.mem_read(pose_ptr,12)))
        search_attempt+=1;consumed=10
    elif address==a5+0x54a:
        collision_ptr=struct.unpack('>I',cpu.mem_read(sp+20,4))[0]
        cpu.mem_write(collision_ptr,bytes([search_contacts[search_attempt-1][0]]));consumed=30
    else:
        cpu.mem_write(sp+6,bytes([search_contacts[search_attempt-1][1],0]));consumed=2
    cpu.reg_write(UC_M68K_REG_A7,sp+4+consumed);cpu.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,search_service,begin=p,end=p) for p in
       [0x304e6,0x301f0,0x3022a,0x30250,a5+0x1162,a5+0x54a,0x30332]]
for iteration in range(160):
    participant=1+iteration%2;enemy=int(participant!=1);kind=[6,7,35,45][iteration%4]
    extent=1024
    failures=501 if iteration<4 else iteration%8
    blocked=[1,1] if iteration%2 else [0,1]
    search_contacts=[blocked for _ in range(failures)]+[[0,0]]
    if iteration>=120:search_contacts=[[iteration%4, (iteration//4)%4]]*502
    search_samples=[spawn_rng.randint(-32768,32767) for _ in range(1003)]
    search_sample_index=0;search_attempt=0
    records=bytearray(12*256);base=(participant-1)*256
    records[base+5]=enemy;records[base+8]=kind
    struct.pack_into('>h',records,base+0x56,1)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,bytes(128));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x133d,b'\x01');u.mem_write(a5-0x12ca,b'\0')
    u.mem_write(a5-0x137e,struct.pack('>h',extent))
    run(0x304b4,struct.pack('>hBB',0,participant,0),instruction_limit=1000000)
    pose=list(struct.unpack('>3i',u.mem_read(0x44000,12)))
    assert search_sample_index==1+2*search_attempt
    cases.append(dict(routine='normal_spawn_search',a=extent,b=enemy,kind=kind,
                      random=search_samples[:search_sample_index],contacts=search_contacts[:search_attempt],
                      pose=pose,expected=search_attempt))
for hook in hooks:u.hook_del(hook)
print('160 complete normal placement searches executed with geometry/contact services supplied')
# Complete participant clearance calls original scans and Manhattan proximity.
u.mem_write(a5+0xe4a,bytes.fromhex('4ef9')+struct.pack('>I',0xb0154))
for iteration in range(600):
    participant=iteration%12;local=(iteration//12)%12;mode=spawn_rng.choice([0,2,4,11,12,1,15,16,-1,-128])
    flags=[[spawn_rng.choice([0,1,2,255]),spawn_rng.choice([0,1,128,255])] for _ in range(12)]
    ids=[spawn_rng.choice([-1]+list(range(1,13))) for _ in range(12)]
    positions=[[spawn_rng.randrange(0,1024<<16),spawn_rng.randrange(0,1024<<16)] for _ in range(12)]
    if iteration>=500:positions=[[spawn_rng.randint(-2**31,2**31-1) for _ in range(2)] for _ in range(12)]
    records=bytearray(12*256);geometry=bytearray(12*128)
    for i in range(12):
        records[i*256+0x88]=flags[i][0];records[i*256+5]=flags[i][1]
        struct.pack_into('>h',records,i*256+0x56,ids[i])
        struct.pack_into('>2i',geometry,i*128,*positions[i])
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,bytes(geometry));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x133d,bytes([local+1]));u.mem_write(a5-0x12ca,struct.pack('>b',mode))
    run(0x30332,struct.pack('>BB',participant+1,0))
    result=u.mem_read(stack+6,1)[0]
    cases.append(dict(routine='spawn_participant_blocked',a=participant,b=local,mode=mode,flags=flags,ids=ids,positions=positions,expected=result))
print('600 complete original participant spawn clearance scans executed without service fixtures')
# Team coordinate branch: original arithmetic helpers, no service fixtures.
# Explicit stop hooks handle boundaries in previously translated blocks.
def variant_boundary(cpu,address,size,data):cpu.emu_stop()
variant_hook=u.hook_add(UC_HOOK_CODE,variant_boundary,begin=0x3070e,end=0x3070e)
for iteration in range(600):
    mode=spawn_rng.choice(list(range(16))+[-128,-1,16,127]);team=spawn_rng.choice([-1,0,1,2,127])
    extent=spawn_rng.choice([512,1000,1024,2048])
    pose=[spawn_rng.randint(-2**31,2**31-1) for _ in range(3)]
    anchor=[spawn_rng.choice([500*65536-1,500*65536,500*65536+1,900*65536]) for _ in range(2)]
    u.mem_write(0x42000,bytes(256));u.mem_write(0x42004,struct.pack('>b',team))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.reg_write(UC_M68K_REG_D7,1)
    u.mem_write(0x44000,struct.pack('>2i',*anchor)+bytes(120)+struct.pack('>2i',*anchor)+bytes(120))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x12ca,struct.pack('>b',mode))
    u.mem_write(a5-0x137e,struct.pack('>h',extent));u.mem_write(stack-0x92,struct.pack('>3i',*pose))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x200)
    u.emu_start(0x30608,0x3070e,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x3070e
    after=list(struct.unpack('>3i',u.mem_read(stack-0x92,12)))
    cases.append(dict(routine='spawn_team_coordinates',a=mode,b=team,extent=extent,pose=pose,anchor=anchor,after=after,expected=0))
u.hook_del(variant_hook)
print('600 original team spawn coordinate transforms executed')
variant_hook=u.hook_add(UC_HOOK_CODE,variant_boundary,begin=0x30608,end=0x30608)
# Type-20 flag-relative branch with supplied Random and direction lookup.
flag_sample=0;flag_direction=[0,0]
def flag_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0x305b2:
        cpu.mem_write(sp,struct.pack('>h',flag_sample));cpu.reg_write(UC_M68K_REG_PC,address+2);return
    ret,first,second=struct.unpack('>3I',cpu.mem_read(sp,12))
    # Pascal stack: second formal output pointer is nearest the return address.
    cpu.mem_write(first,struct.pack('>i',flag_direction[0]))
    cpu.mem_write(second,struct.pack('>i',flag_direction[1]))
    cpu.reg_write(UC_M68K_REG_A7,sp+16);cpu.reg_write(UC_M68K_REG_PC,ret)
hooks=[u.hook_add(UC_HOOK_CODE,flag_service,begin=p,end=p) for p in [0x305b2,a5+0xbaa]]
for iteration in range(400):
    flag_sample=spawn_rng.choice([-32768,-1,0,1,32767,spawn_rng.randint(-32768,32767)])
    flag_direction=[spawn_rng.randint(-65536,65536) for _ in range(2)]
    pose=[spawn_rng.randint(-2**31,2**31-1) for _ in range(3)]
    cursor=spawn_rng.randint(1,12)
    geometry=bytearray(13*128);struct.pack_into('>3i',geometry,(cursor-1)*128,*pose)
    u.mem_write(0x44000,bytes(geometry));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1300,struct.pack('>h',cursor))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x200)
    u.emu_start(0x30594,0x30608,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x30608
    after=list(struct.unpack('>3i',u.mem_read(stack-0x92,12)))
    assert struct.unpack('>h',u.mem_read(a5-0x1300,2))[0]==cursor+1
    cases.append(dict(routine='spawn_beside_flag',a=flag_sample,b=0,flag=pose[:2],direction=flag_direction,after=after,expected=0))
for hook in hooks:u.hook_del(hook)
u.hook_del(variant_hook)
print('400 original flag-relative spawn branches executed with Random/direction services supplied')
# Combined full placement: team transforms, flag cursor, retries and Random order.
def combined_search_service(cpu,address,size,data):
    global search_sample_index
    if address==0x305b2:
        sp=cpu.reg_read(UC_M68K_REG_A7)
        cpu.mem_write(sp,struct.pack('>h',search_samples[search_sample_index]));search_sample_index+=1
        cpu.reg_write(UC_M68K_REG_PC,address+2)
    elif address==a5+0xbaa:flag_service(cpu,address,size,data)
    else:search_service(cpu,address,size,data)
hooks=[u.hook_add(UC_HOOK_CODE,combined_search_service,begin=p,end=p) for p in
       [0x304e6,0x301f0,0x3022a,0x30250,0x305b2,a5+0xbaa,a5+0x1162,a5+0x54a,0x30332]]
for iteration in range(240):
    context=dict(enemy=1,kind=20 if iteration%2 else 7,extent=1024,
                 mode=[0,2,12,15,-1,4][iteration%6],team=[0,1,2][iteration%3],object_id=1)
    entries=[dict(kind=7,pose=[spawn_rng.randrange(1024)<<16,spawn_rng.randrange(1024)<<16,0])]
    entries += [dict(kind=12 if i<3 else 2,pose=[spawn_rng.randrange(1024)<<16,spawn_rng.randrange(1024)<<16,0]) for i in range(5)]
    cursor=2;failures=iteration%6
    search_contacts=[[1,0]]*failures+[[0,0]]
    search_samples=[spawn_rng.randint(-32768,32767) for _ in range(30)]
    flag_direction=[spawn_rng.randint(-65536,65536) for _ in range(2)]
    search_sample_index=0;search_attempt=0
    records=bytearray(12*256);records[4]=context['team'];records[5]=1;records[8]=context['kind']
    struct.pack_into('>h',records,0x56,1)
    geometry=bytearray(len(entries)*128)
    for i,entry in enumerate(entries):
        struct.pack_into('>3i',geometry,i*128,*entry['pose']);geometry[i*128+0x14]=entry['kind']
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,bytes(geometry));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x133d,b'\x02');u.mem_write(a5-0x12ca,struct.pack('>b',context['mode']))
    u.mem_write(a5-0x137e,struct.pack('>h',1024));u.mem_write(a5-0x1300,struct.pack('>h',cursor))
    u.ctl_remove_cache(0x30000,0x34000)
    run(0x304b4,struct.pack('>hBB',0,1,0),instruction_limit=100000)
    pose=list(struct.unpack('>3i',u.mem_read(0x44000,12)))
    after_cursor=struct.unpack('>h',u.mem_read(a5-0x1300,2))[0]
    cases.append(dict(routine='spawn_search',a=cursor,b=0,context=context,geometry=entries,direction=flag_direction,
                      random=search_samples[:search_sample_index],contacts=search_contacts,pose=pose,
                      cursor=after_cursor,expected=search_attempt))
for hook in hooks:u.hook_del(hook)
print('240 complete combined placement searches executed with explicit direction/geometry/contact services')
# Entire last-enemy bonus calculation, through the score-call boundary.
# No score/audio fixtures: stop immediately before invoking the score handler.
def bonus_boundary(cpu,address,size,data):cpu.emu_stop()
bonus_hook=u.hook_add(UC_HOOK_CODE,bonus_boundary,begin=0x31cb2,end=0x31cb2)
bonus_rng=random.Random(0x311c40)
for iteration in range(500):
    level=bonus_rng.choice([-32768,-1,0,1,20,21,26,27,35,36,32767])
    counts=[bonus_rng.randint(0,4) for _ in range(47)]
    lives=[bonus_rng.randint(0,10) for _ in range(47)]
    if iteration>=350:
        counts=[bonus_rng.randint(-32768,32767) for _ in range(47)]
        lives=[bonus_rng.randint(-32768,32767) for _ in range(47)]
    bonus_table=bytearray(0x900)
    for i in range(47):
        struct.pack_into('>h',bonus_table,2+2*i,counts[i]);struct.pack_into('>h',bonus_table,0x60+2*i,lives[i])
    u.mem_write(0x45000,bytes(bonus_table));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(a5-0x136e,struct.pack('>h',level));u.mem_write(a5-0x133d,b'\x01')
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,stack-0x200)
    u.emu_start(0x31c40,0x31cb2,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x31cb2
    sp=u.reg_read(UC_M68K_REG_A7)
    award=struct.unpack('>h',u.mem_read(sp,2))[0]
    assert u.mem_read(sp+2,1)==b'\x01'
    cases.append(dict(routine='last_enemy_bonus',a=level,b=0,counts=counts,lives=lives,expected=award))
u.hook_del(bonus_hook)
print('500 complete last-enemy bonus calculations executed through the score-call boundary')
# Complete sound request wrappers, through the sound backend boundary.
u.mem_map(0x160000,0x10000);u.mem_write(0x160000,(source/'15.bin').read_bytes())
sound_calls=[]
def sound_backend(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    ret,sample,secondary=struct.unpack('>Iii',cpu.mem_read(sp,12))
    sound_calls.append([sample,secondary])
    cpu.reg_write(UC_M68K_REG_A7,sp+4);cpu.reg_write(UC_M68K_REG_PC,ret)
sound_hook=u.hook_add(UC_HOOK_CODE,sound_backend,begin=a5+0x31a,end=a5+0x31a)
sound_rng=random.Random(0x150b42)
requests=[0,1,0x10011,0x10017,0x1002a,0x1002c,9999,10000,32767,32768,65535,0xffffffff]
requests += [sound_rng.getrandbits(32) for _ in range(500)]
for request in requests:
    sound_calls.clear();u.mem_write(a5-0x42f8,struct.pack('>h',-123))
    run(0x160b42,struct.pack('>hI',sound_rng.randint(-32768,32767),request))
    assert len(sound_calls)<=1
    if sound_calls:assert sound_calls[0][1]==0
    selected=sound_calls[0][0] if sound_calls else None
    assert struct.unpack('>h',u.mem_read(a5-0x42f8,2))[0]==(selected if selected is not None else -123)
    cases.append(dict(routine='sound_request_id',a=request,b=0,expected=selected if selected is not None else -1))
u.hook_del(sound_hook)
print(f'{len(requests)} complete original sound request routes executed with backend calls captured')
# Complete damage prefix, including victim admission, grace, type-31 guard,
# damage arithmetic and the counters. Supply only TickCount and phase lookup.
counter_now=0;counter_phase=0
def counter_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0x30ac2:
        cpu.mem_write(sp,struct.pack('>i',counter_now));cpu.reg_write(UC_M68K_REG_PC,address+2)
    elif address==a5+0x4e2:
        ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
        cpu.mem_write(sp+4,bytes([counter_phase,0]));cpu.reg_write(UC_M68K_REG_A7,sp+4);cpu.reg_write(UC_M68K_REG_PC,ret)
    else:cpu.emu_stop()
hooks=[u.hook_add(UC_HOOK_CODE,counter_service,begin=p,end=p) for p in [0x30ac2,a5+0x4e2,0x30b9a,0x30fc4]]
counter_rng=random.Random(0x31b40)
counter_keys=['player_shots','player_hits','other_shots','player_damage_events']
for iteration in range(600):
    local=iteration%12+1;victim=local if iteration%2 else counter_rng.randint(1,12)
    attacker=counter_rng.choice([0,local,counter_rng.randint(1,12)])
    geometry_id=counter_rng.choice([-1,1,2]);mode=counter_rng.choice([0,0,1,255])
    grace=counter_rng.randint(-1000,1000);counter_now=grace+counter_rng.choice([-1,0,74,75,76,1000])
    counter_phase=iteration%2;kind=counter_rng.choice([6,7,31]);health=counter_rng.randint(0,10);damage=counter_rng.choice([0,1,5])
    before=[counter_rng.choice([0,1,2**31-1,-2**31]) for _ in range(4)]
    records=bytearray(12*256);base=(victim-1)*256
    records[base+5]=int(victim!=local);records[base+8]=kind
    struct.pack_into('>h',records,base+0x56,geometry_id);struct.pack_into('>h',records,base+0x5a,health)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x457fc,struct.pack('>4i',*before))
    u.mem_write(a5-0x133d,bytes([local]));u.mem_write(a5-0x12ca,bytes([mode]));u.mem_write(a5-0x3d18,struct.pack('>i',grace))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>IhhiBBBB',stop,damage,0,counter_now,attacker,0,victim,0))
    u.ctl_remove_cache(0x30000,0x34000)
    u.emu_start(0x30a84,0x30b9a,count=10000)
    pc=u.reg_read(UC_M68K_REG_PC);assert pc in [0x30b9a,0x30fc4]
    after=list(struct.unpack('>4i',u.mem_read(0x457fc,16)))
    after_grace=struct.unpack('>i',u.mem_read(a5-0x3d18,4))[0]
    cases.append(dict(routine='combat_damage_counter',a=grace,b=after_grace,
        event=dict(local=local,attacker=attacker,victim=victim,geometry_id=geometry_id,mode=mode,now=counter_now),
        before=dict(zip(counter_keys,before)),after=dict(zip(counter_keys,after)),expected=int(pc==0x30b9a)))
for hook in hooks:u.hook_del(hook)
for iteration in range(200):
    local=iteration%12+1;owner=counter_rng.randint(0,12)
    before=[counter_rng.choice([0,1,2**31-1,-2**31]) for _ in range(4)]
    u.mem_write(a5-0x133d,bytes([local]));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(0x457fc,struct.pack('>4i',*before));u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_D5,owner)
    u.emu_start(0x80292,0x802aa,count=100)
    assert u.reg_read(UC_M68K_REG_PC)==0x802aa
    after=list(struct.unpack('>4i',u.mem_read(0x457fc,16)))
    cases.append(dict(routine='combat_launch_counter',a=local,b=owner,before=dict(zip(counter_keys,before)),after=dict(zip(counter_keys,after)),expected=0))
print('600 original damage prefixes and 200 launch counter blocks executed')
# Execute the original complete default-table initialization before GetResource.
# Poison the destination to ensure absent resource rows cannot inherit stale data.
u.mem_write(0x45000, bytes([0xa5])*0x900)
u.mem_write(a5-0x1350, struct.pack('>I',0x45000))
u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
u.emu_start(0xb14dc,0xb166c,count=10000)
assert u.reg_read(UC_M68K_REG_PC)==0xb166c
parameter_fields = [
    ('hits_to_kill',0xbe,'h',False),('collision_damage',0x11c,'h',False),
    ('attack_range',0x17a,'i',True),('short_firing_range',0x236,'i',True),
    ('firing_arc',0x2f2,'h',False),('firing_interval',0x350,'h',False),
    ('radial_velocity',0x3ae,'h',False),('angular_velocity',0x40c,'i',True),
    ('score',0x4c8,'i',False),('bonus',0x584,'i',False),('weapon',0x640,'h',False)]
for kind in range(47):
    parameters = dict(kind=kind)
    for name,offset,fmt,fixed in parameter_fields:
        size=struct.calcsize('>'+fmt)
        value=struct.unpack('>'+fmt,u.mem_read(0x45000+offset+kind*size,size))[0]
        if fixed:
            assert value & 0xffff == 0
            value >>= 16
        parameters[name]=value
    cases.append(dict(routine='default_enemy_parameters',a=kind,b=0,parameters=parameters,expected=0))
print('47 original default enemy parameter records executed and captured')
# Complete parameter loader with Resource Manager calls supplied. Arithmetic
# helpers and the type-5 branch execute as original instructions.
def parameter_services(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address in [0xb1678,0xb168c]:
        cpu.mem_write(sp+6,struct.pack('>I',0x43000))
        cpu.reg_write(UC_M68K_REG_A7,sp+6)
    elif address==0xb18f6:
        cpu.reg_write(UC_M68K_REG_A7,sp+4)
    cpu.reg_write(UC_M68K_REG_PC,address+2)
parameter_hooks=[u.hook_add(UC_HOOK_CODE,parameter_services,begin=p,end=p)
                 for p in [0xb1678,0xb168c,0xb16a2,0xb18f2,0xb18f6]]
parameter_resources=[p.read_bytes() for campaign in ['central','eastern','south']
    for p in sorted((root/f'.local/{campaign}-shape-resources/6276616c').glob('*.bin'))]
parameter_rng=random.Random(0x2514dc)
for iteration in range(100):
    rows=[]
    for kind in [5,7,263,45,-1,47,128,261,0,46]:
        words=[kind]+[parameter_rng.randint(-32768,32767) for _ in range(8)]
        rows.append(struct.pack('>9h2ih',*words,parameter_rng.randint(-2**31,2**31-1),
            parameter_rng.randint(-2**31,2**31-1),parameter_rng.randint(-32768,32767)))
    parameter_resources.append(struct.pack('>H',len(rows))+b''.join(rows))
for iteration,resource in enumerate(parameter_resources):
    modifier=parameter_rng.choice([0,1,20,-1,32767,-32768])
    flag=parameter_rng.choice([-128,-1,0,1,127]);mode=parameter_rng.choice([42,0,41,43])
    extra=flag<0 or mode!=42
    u.mem_write(0x45000,bytes([0xa5])*0x900)
    u.mem_write(0x43000,struct.pack('>I',0x47000));u.mem_write(0x47000,resource)
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(a5-0x7be4,struct.pack('>b',flag));u.mem_write(a5-0x7be8,bytes([mode]))
    u.mem_write(a5-0x7bef,b'\x00')
    u.mem_write(a5+0x1282,bytes.fromhex('4ef9')+struct.pack('>I',0x90004))
    u.ctl_remove_cache(0xb0000,0xb4000)
    run(0xb14dc,struct.pack('>hh',modifier,10000),instruction_limit=50000)
    parameters=[]
    for kind in range(47):
        record=dict(kind=kind)
        for name,offset,fmt,fixed in parameter_fields:
            size=struct.calcsize('>'+fmt)
            value=struct.unpack('>'+fmt,u.mem_read(0x45000+offset+kind*size,size))[0]
            if fixed:
                assert value & 0xffff == 0
                value >>= 16
            record[name]=value
        parameters.append(record)
    weights=[struct.unpack('>b',u.mem_read(a5-offset,1))[0] for offset in [0x3fc7,0x3fc8,0x3fc9,0x3fca]]
    cases.append(dict(routine='enemy_parameters',a=modifier,b=int(extra),resource=list(resource),parameters=parameters,dump_weights=weights,expected=0))
for hook in parameter_hooks:u.hook_del(hook)
print(f'{len(parameter_resources)} complete original enemy parameter loaders executed with Resource Manager services supplied')
# Original wave index/cycle loop, row reads, and repeat-cycle suppression.
wave_resources=[p.read_bytes() for campaign in ['central','eastern','south']
    for p in sorted((root/f'.local/{campaign}-shape-resources/77617665').glob('*.bin'))]
wave_rng=random.Random(0x251c32)
for count in [1,2,7,31]:
    wave_resources.append(struct.pack('>H',count)+b''.join(
        struct.pack('>8h',*[wave_rng.randint(-32768,32767) for _ in range(8)]) for _ in range(count)))
wave_case_count=0
for resource in wave_resources:
    count=struct.unpack_from('>H',resource)[0]
    assert 0<count<=32767
    u.mem_write(0x43000,struct.pack('>I',0x47000));u.mem_write(0x47000,resource)
    for level in sorted(set(range(1,min(32767,count*2+1)+1)) | {32767}):
        u.mem_write(a5-0x136e,struct.pack('>h',level))
        u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
        u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x43000)
        u.reg_write(UC_M68K_REG_A6,stack-0x200)
        u.emu_start(0xb1c32,0xb1c80,count=300000)
        assert u.reg_read(UC_M68K_REG_PC)==0xb1c80
        index=u.reg_read(UC_M68K_REG_D6)&0xffff
        cycle=u.reg_read(UC_M68K_REG_D4)&0xffff
        u.emu_start(0xb1cc8,0xb1cd4,count=100)
        assert u.reg_read(UC_M68K_REG_PC)==0xb1cd4
        fields=dict(zip(['world','enemies','hint','beasties','start_movie','end_movie','song','colors'],
            struct.unpack_from('>8h',resource,2+(index-1)*16)))
        for field,offset in [('world',0x1308),('enemies',0x130a),('hint',0x1302),('start_movie',0x1304),('end_movie',0x1306)]:
            fields[field]=struct.unpack('>h',u.mem_read(a5-offset,2))[0]
        cases.append(dict(routine='select_wave',a=level,b=0,resource=list(resource),
            selection=dict(index=index-1,health_modifier=cycle,wave=fields),expected=0))
        wave_case_count+=1
# Host guards: original code lacks these checks and would access invalid rows.
for resource,level in [(b'\x00\x00',1),(wave_resources[0],0),(wave_resources[0],-1)]:
    cases.append(dict(routine='select_wave',a=level,b=0,resource=list(resource),selection=None,expected=0))
print(f'{wave_case_count} original wave selections and three explicit host guards recorded')
# Resolve zero wave resource IDs through the original Random traps and branches.
wave_samples=[];wave_sample_index=0
def wave_random_service(cpu,address,size,data):
    global wave_sample_index
    sp=cpu.reg_read(UC_M68K_REG_A7)
    cpu.mem_write(sp,struct.pack('>h',wave_samples[wave_sample_index]))
    wave_sample_index+=1
    cpu.reg_write(UC_M68K_REG_PC,address+2)
wave_hooks=[u.hook_add(UC_HOOK_CODE,wave_random_service,begin=p,end=p) for p in [0xb1ce2,0xb1d24]]
for iteration in range(600):
    world=wave_rng.choice([0,0,10000,10100,32767,-32768,-1])
    enemies=wave_rng.choice([0,0,10000,10111,32767,-32768,-1])
    counts=[wave_rng.choice([-32768,-1,0,1,2,7,31,32767]) for _ in range(2)]
    alternate=wave_rng.choice([0,0,1,128,255])
    wave_samples=[wave_rng.choice([-32768,-32767,-1,0,1,32767,wave_rng.randint(-32768,32767)]) for _ in range(2)]
    wave_sample_index=0
    u.mem_write(a5-0x1308,struct.pack('>h',world));u.mem_write(a5-0x130a,struct.pack('>h',enemies))
    u.mem_write(a5-0x3d36,struct.pack('>h',counts[0]));u.mem_write(a5-0x3d38,struct.pack('>h',counts[1]))
    u.mem_write(a5-0x7be6,bytes([alternate]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack-0x200);u.reg_write(UC_M68K_REG_A7,stack)
    u.ctl_remove_cache(0xb0000,0xb4000)
    u.emu_start(0xb1cd4,0xb1d46,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0xb1d46
    resolved_world=struct.unpack('>h',u.mem_read(a5-0x1308,2))[0]
    resolved_enemies=struct.unpack('>h',u.mem_read(a5-0x130a,2))[0]
    assert struct.unpack('>h',u.mem_read(stack-0x200+8,2))[0]==resolved_world
    cases.append(dict(routine='resolve_wave_resources',a=world,b=enemies,counts=counts,alternate=alternate,random=wave_samples,
        result=dict(world=resolved_world,enemies=resolved_enemies,random_used=wave_sample_index),expected=0))
for hook in wave_hooks:u.hook_del(hook)
for world,enemies,counts,samples in [(0,0,[1,1],[]),(0,0,[1,1],[0]),(10000,0,[1,1],[])]:
    cases.append(dict(routine='resolve_wave_resources',a=world,b=enemies,counts=counts,alternate=0,random=samples,result=None,expected=0))
print('600 original random wave resource resolutions and three missing-sample guards recorded')
# Full resource-count loops: supplied Resource Manager lookups preserve gaps,
# signed ID/count wrapping, and the original -1 sentinel.
prefix_ids=None;prefix_gap=-1;prefix_queries=[]
def prefix_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address in [0xb1a54,0xb1a92]:
        resource_id=struct.unpack('>h',cpu.mem_read(sp,2))[0]
        prefix_queries.append(resource_id)
        present=resource_id in prefix_ids if prefix_ids is not None else resource_id!=prefix_gap
        cpu.mem_write(sp+6,struct.pack('>I',0x43000 if present else 0))
        cpu.reg_write(UC_M68K_REG_A7,sp+6)
    else:cpu.reg_write(UC_M68K_REG_A7,sp+4)
    cpu.reg_write(UC_M68K_REG_PC,address+2)
prefix_hooks=[u.hook_add(UC_HOOK_CODE,prefix_service,begin=p,end=p) for p in [0xb1a54,0xb1a92,0xb1a64,0xb1aa2]]
prefix_sets=[]
for campaign in ['central','eastern','south']:
    for kind in ['77726c64','656e656d']:
        prefix_sets.append(([int(p.stem) for p in (root/f'.local/{campaign}-shape-resources/{kind}').glob('*.bin')],-1))
prefix_sets += [([], -1),([10000,10002,10100],-1),([9999,10001],-1)]
prefix_sets += [(None,gap) for gap in [10000,10001,32767,-32768,-32767,-22768,-1]]
for prefix_ids,prefix_gap in prefix_sets:
    for start,offset in [(0xb1a3a,0x3d36),(0xb1a78,0x3d38)]:
        prefix_queries=[]
        u.ctl_remove_cache(0xb0000,0xb4000)
        run(start,b'',instruction_limit=2000000)
        result=struct.unpack('>h',u.mem_read(a5-offset,2))[0]
        cases.append(dict(routine='resource_prefix_count',a=prefix_gap,b=0,ids=prefix_ids,queries=prefix_queries,expected=result))
for hook in prefix_hooks:u.hook_del(hook)
print(f'{len(prefix_sets)*2} complete original resource enumerations executed, including ID/count wrapping')
# Initial effective difficulty arithmetic; stop before the TickCount scheduler.
for iteration in range(200):
    difficulty=wave_rng.choice([0,3,7,127,-32768,32767,wave_rng.randint(-32768,32767)])
    level=wave_rng.choice([0,1,126,127,128,-32768,32767,wave_rng.randint(-32768,32767)])
    u.mem_write(a5-0x4258,struct.pack('>h',difficulty))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>Ih',stop,level))
    u.emu_start(0x120842,0x12086c,count=100)
    assert u.reg_read(UC_M68K_REG_PC)==0x12086c
    effective=struct.unpack('>h',u.mem_read(a5-0x4256,2))[0]
    cases.append(dict(routine='initial_effective_difficulty',a=difficulty,b=level,expected=effective))
print('200 original initial effective-difficulty calculations executed')
# Original local wave refill, including the actual capacity helpers. Verify
# the complete participant buffer is unchanged except the two refill words.
for disp,target in [(0xe0a,0xf0684),(0xe12,0xf06fc),(0x452,0x7002c)]:
    u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
for iteration in range(200):
    participant=iteration%12+1;shields=wave_rng.randint(-128,127);ammo_setting=wave_rng.randint(-128,127)
    records=bytearray(wave_rng.randbytes(12*256));base=(participant-1)*256
    records[base+5]=0
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x133d,bytes([participant]));u.mem_write(a5-0x3f8e+(participant-1)*40+0x21,struct.pack('>bb',shields,ammo_setting))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0xb23b6,0xb2406,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0xb2406
    after=bytes(u.mem_read(0x42000,len(records)))
    health=struct.unpack_from('>h',after,base+0x5a)[0];ammo=struct.unpack_from('>h',after,base+0x5e)[0]
    struct.pack_into('>h',records,base+0x5a,health);struct.pack_into('>h',records,base+0x5e,ammo)
    assert after==bytes(records),'Wave refill changed another participant field'
    cases.append(dict(routine='wave_refill',a=shields,b=ammo_setting,refill=[health,ammo],expected=0))
print('200 original wave refill blocks executed; all other participant bytes preserved')
# Full campaign upgrade routine with the original setting-validator and min
# helper. No validator results or eligibility decisions are supplied.
u.mem_write(a5+0xdf2,bytes.fromhex('4ef9')+struct.pack('>I',0xf0078))
u.mem_write(a5+0x452,bytes.fromhex('4ef9')+struct.pack('>I',0x7002c))
upgrade_inputs=[]
for level in [0,8,10,11,15,16,17,24,32,40,48,56,64,72,80,32760,32767,-32768]:
    for settings in [[5,6,4,0,0,0,0],[5,10,4,0,0,1,2],[5,10,5,0,0,1,2],[8,10,5,0,0,1,2]]:
        upgrade_inputs.append((level,settings))
for iteration in range(500):
    level=wave_rng.choice([16,24,32760,wave_rng.randint(-32768,32767)])
    upgrade_inputs.append((level,list(wave_rng.randbytes(7))))
for iteration,(level,settings) in enumerate(upgrade_inputs):
    participant=iteration%12+1
    address=a5-0x3f8e+(participant-1)*40
    record=bytearray(wave_rng.randbytes(40));record[0x20:0x27]=bytes(settings)
    u.mem_write(address,bytes(record));u.mem_write(a5-0x133d,bytes([participant]))
    u.mem_write(a5-0x136e,struct.pack('>h',level))
    run(0xb1ff6,b'',instruction_limit=2000)
    after=bytes(u.mem_read(address,40));record[0x20:0x27]=after[0x20:0x27]
    assert bytes(record)==after,'Upgrade modified a non-setting field'
    cases.append(dict(routine='campaign_upgrade',a=level,b=0,settings=settings,after=list(after[0x20:0x27]),expected=0))
print(f'{len(upgrade_inputs)} complete original campaign upgrades executed with original validators')
# Complete original type-7 firing predicate. Only the angle service is supplied;
# range, timing, parameter reads, target-state rejection and arc arithmetic run.
u.mem_map(0x170000,0x10000);u.mem_write(0x170000,(source/'20.bin').read_bytes())
fire_angle=0
def fire_angle_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.mem_write(sp+12,struct.pack('>h',fire_angle))
    cpu.reg_write(UC_M68K_REG_A7,sp+12);cpu.reg_write(UC_M68K_REG_PC,ret)
fire_hook=u.hook_add(UC_HOOK_CODE,fire_angle_service,begin=a5+0x4c2,end=a5+0x4c2)
fire_rng=random.Random(0x201e9a)
for iteration in range(800):
    now=fire_rng.choice([0,19,20,21,119,120,121,2**31-1,-2**31,fire_rng.randint(-200,200)])
    timer=fire_rng.choice([-1,0,((now-20+2**31)%2**32)-2**31,((now-120+2**31)%2**32)-2**31,2**31-1,-2**31])
    distance=fire_rng.choice([0,80<<16,(300<<16)-1,300<<16,(300<<16)+1,-1,fire_rng.randint(-2**31,2**31-1)])
    interval=fire_rng.choice([0,20,120,-1,-32768,32767])
    short_range=fire_rng.choice([80<<16,distance,distance-1,0])
    arc=fire_rng.choice([-32768,-1,0,1,15,32767]);fire_angle=fire_rng.choice([-32768,-180,-5,-4,0,4,5,180,32767])
    target_state=fire_rng.choice([-1,0,1])
    records=bytearray(12*256)
    struct.pack_into('>h',records,0x56,1)
    records[256+5]=1;records[256+8]=7
    struct.pack_into('>h',records,256+0x56,2);struct.pack_into('>i',records,256+0x44,timer)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,bytes(256));u.mem_write(0x44028,struct.pack('>i',target_state))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(0x45000+0x350+7*2,struct.pack('>h',interval));u.mem_write(0x45000+0x2f2+7*2,struct.pack('>h',arc))
    u.mem_write(0x45000+0x236+7*4,struct.pack('>i',short_range));u.mem_write(a5-0x134c,struct.pack('>i',now))
    run(0x171e9a,struct.pack('>iBBBB',distance,1,0,2,0),instruction_limit=2000)
    result=bool(u.mem_read(stack+12,1)[0]);after_timer=struct.unpack('>i',u.mem_read(0x42144,4))[0]
    cases.append(dict(routine='type7_fire_check',a=0,b=0,
        input=dict(now=now,timer=timer,distance=distance,interval=interval,short_range=short_range,
                   firing_arc=arc,angle=fire_angle,target_field_28=target_state),result=[result,after_timer],expected=0))
u.hook_del(fire_hook)
print('800 complete original type-7 firing predicates executed with angle-service results supplied')
# Full original distance helper, including its original signed maximum helper.
for iteration in range(300):
    first=[fire_rng.choice([0,1,-1,-2**31,2**31-1,fire_rng.randint(-2**31,2**31-1)]) for _ in range(2)]
    second=[fire_rng.choice([0,1,-1,-2**31,2**31-1,fire_rng.randint(-2**31,2**31-1)]) for _ in range(2)]
    u.mem_write(0x44000,struct.pack('>3i',*first,0));u.mem_write(0x44080,struct.pack('>3i',*second,0))
    value=run(0x71760,struct.pack('>II',0x44000,0x44080))
    cases.append(dict(routine='ai_planar_distance',a=0,b=0,first=first,second=second,expected=value))
print('300 complete original planar distance approximations executed')
# Full aiming-angle helper with explicit OS service results, not atan2 rounding.
aim_calls=[];aim_return=0
def aim_services(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0x71724:
        denominator,numerator=struct.unpack('>hh',cpu.mem_read(sp,4))
        aim_calls.append([numerator,denominator])
        cpu.mem_write(sp+4,struct.pack('>i',0x12345678))
        cpu.reg_write(UC_M68K_REG_A7,sp+4)
    else:
        assert struct.unpack('>i',cpu.mem_read(sp,4))[0]==0x12345678
        cpu.mem_write(sp+4,struct.pack('>h',aim_return))
        cpu.reg_write(UC_M68K_REG_A7,sp+4)
    cpu.reg_write(UC_M68K_REG_PC,address+2)
aim_hooks=[u.hook_add(UC_HOOK_CODE,aim_services,begin=p,end=p) for p in [0x71724,0x71726]]
aim_rng=random.Random(0x61680)
for iteration in range(800):
    origin=[aim_rng.randint(-1024<<16,1024<<16) for _ in range(2)]
    delta=[aim_rng.choice([-65537,-65536,-1,0,1,65535,65536,65537,aim_rng.randint(-1024<<16,1024<<16)]) for _ in range(2)]
    target=[origin[i]+delta[i] for i in range(2)]
    source_pose=origin+[aim_rng.choice([0,90<<16,180<<16,359<<16,aim_rng.randint(-2**31,2**31-1)])]
    aim_return=aim_rng.choice([-32768,0,1,45,89,90,179,32767]);aim_calls=[]
    u.mem_write(0x44000,struct.pack('>3i',*target,0));u.mem_write(0x44080,struct.pack('>3i',*source_pose))
    u.reg_write(UC_M68K_REG_A7,stack);u.reg_write(UC_M68K_REG_A5,a5)
    u.mem_write(stack,struct.pack('>IIIh',stop,0x44000,0x44080,12345))
    u.ctl_remove_cache(0x71680,0x71760)
    u.emu_start(0x71680,stop,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==stop
    result=struct.unpack('>h',u.mem_read(stack+12,2))[0]
    if result==12345:result=None
    cases.append(dict(routine='ai_aim_angle',a=aim_return,b=0,target=target,source=source_pose,result=result,calls=aim_calls,expected=0))
for hook in aim_hooks:u.hook_del(hook)
print('800 complete original aiming helpers executed with FixRatio/AngleFromSlope service results supplied')
# Signed-word rotation can cycle forever at the -32768 coordinate boundary.
# Confirm bounded original nontermination; Rust explicitly guards these inputs.
for target in [[-2**31,0],[0,-2**31],[-2**31,-2**31]]:
    source_pose=[0,0,0]
    u.mem_write(0x44000,struct.pack('>3i',*target,0));u.mem_write(0x44080,struct.pack('>3i',*source_pose))
    u.reg_write(UC_M68K_REG_A7,stack);u.reg_write(UC_M68K_REG_A5,a5)
    u.mem_write(stack,struct.pack('>IIIh',stop,0x44000,0x44080,12345))
    u.emu_start(0x71680,stop,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)!=stop
    cases.append(dict(routine='ai_aim_angle',guarded=True,a=0,b=0,target=target,source=source_pose,result=None,calls=[],expected=0))
print('Three original nonterminating angle-rotation inputs covered by explicit host guards')
# Original nonlocal movement decision block, with collision-probe results supplied.
movement_calls=[];movement_probe=False
def movement_probe_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    ret,width,distance,heading=struct.unpack('>Iiii',cpu.mem_read(sp,16))
    assert width==40<<16 and distance==80<<16
    movement_calls.append(heading)
    cpu.mem_write(sp+18,bytes([int(movement_probe),0]))
    cpu.reg_write(UC_M68K_REG_A7,sp+18);cpu.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0x522,bytes.fromhex('4ef9')+struct.pack('>I',0x10004))
movement_hook=u.hook_add(UC_HOOK_CODE,movement_probe_service,begin=0x1705b0,end=0x1705b0)
u.ctl_flush_tb()
movement_rng=random.Random(0x201b8e)
movement_inputs=[]
for delta in [-180<<16,-10<<16,-1<<16,0,1<<16,10<<16,180<<16]:
    for epsilon in [-1,0,1]:
        for reverse in [False,True]:
            for probe in [False,True]:
                movement_inputs.append((delta+epsilon,0,45<<16,reverse,probe))
for _ in range(600):
    movement_inputs.append((movement_rng.randint(-2**31,2**31-1),movement_rng.randint(-2**31,2**31-1),
        movement_rng.choice([0,1<<16,10<<16,45<<16,180<<16,-2**31,2**31-1]),
        bool(movement_rng.randrange(2)),bool(movement_rng.randrange(2))))
for desired,current,threshold,reverse,movement_probe in movement_inputs:
    movement_calls=[]
    u.reg_write(UC_M68K_REG_A7,0x4f000);u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_A3,0x41002);u.reg_write(UC_M68K_REG_D7,2)
    u.mem_write(0x41000,bytes(4));u.mem_write(stack+8,struct.pack('>I',0x41000))
    u.mem_write(stack-4,struct.pack('>i',current));u.mem_write(stack-0x28,struct.pack('>i',80<<16))
    u.mem_write(0x42018,struct.pack('>iiB',desired,threshold,int(reverse)))
    u.emu_start(0x171b8e,0x171c88,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x171c88
    difference=struct.unpack('>i',struct.pack('>I',u.reg_read(UC_M68K_REG_D4)))[0]
    steering,throttle=struct.unpack('>hh',u.mem_read(0x41000,4))
    cases.append(dict(routine='ai_movement_commands',a=desired,b=current,threshold=threshold,reverse=reverse,
        probe=movement_probe,result=[difference,steering,throttle],calls=movement_calls,expected=0))
u.hook_del(movement_hook)
print(f'{len(movement_inputs)} original nonlocal AI movement decision blocks executed with supplied clearance probes')
# Full original heading search, including real angle wrapping and exact probe order.
search_calls=[];search_accept=0
def search_probe_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    ret,width,distance,heading=struct.unpack('>Iiii',cpu.mem_read(sp,16))
    assert width==40<<16 and distance==80<<16
    search_calls.append(heading)
    cpu.mem_write(sp+18,bytes([int(len(search_calls)==search_accept),0]))
    cpu.reg_write(UC_M68K_REG_A7,sp+18);cpu.reg_write(UC_M68K_REG_PC,ret)
search_hook=u.hook_add(UC_HOOK_CODE,search_probe_service,begin=0x1705b0,end=0x1705b0)
u.ctl_flush_tb()
search_count=0
for heading in [0,1,-1,359<<16,360<<16,-360<<16,2**31-1,-2**31]:
    for search_accept in range(27):
        search_calls=[]
        u.reg_write(UC_M68K_REG_A7,stack);u.reg_write(UC_M68K_REG_A5,a5)
        u.mem_write(stack,struct.pack('>IiiiBBi',stop,40<<16,80<<16,heading,2,0,123456789))
        u.emu_start(0x1706ea,stop,count=100000)
        assert u.reg_read(UC_M68K_REG_PC)==stop, (heading,search_accept,hex(u.reg_read(UC_M68K_REG_PC)))
        result=struct.unpack('>i',u.mem_read(stack+18,4))[0]
        if result==123456789:result=None
        cases.append(dict(routine='ai_clear_heading',a=heading,b=search_accept,result=result,calls=search_calls,expected=0))
        search_count+=1
u.hook_del(search_hook)
print(f'{search_count} complete original heading searches executed with supplied clearance probes')
# Complete clearance probe: original direction-table lookup and boundary logic.
# Only the inner collision selection is supplied; its arguments are captured.
clearance_calls=[];clearance_hit=0
clearance_width=0;clearance_distance=0
def clearance_collision_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    ret,excluded,width,distance,pose=struct.unpack('>IhiiI',cpu.mem_read(sp,18))
    assert excluded==3 and width==clearance_width and distance==clearance_distance
    clearance_calls.append(list(struct.unpack('>iii',cpu.mem_read(pose,12))))
    cpu.mem_write(sp+18,struct.pack('>h',clearance_hit))
    cpu.reg_write(UC_M68K_REG_A7,sp+18);cpu.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0xbca,bytes.fromhex('4ef9')+struct.pack('>I',0x20dcc))
clearance_hook=u.hook_add(UC_HOOK_CODE,clearance_collision_service,begin=a5+0x53a,end=a5+0x53a)
u.ctl_flush_tb()
clearance_rng=random.Random(0x2005b0)
for iteration in range(1000):
    side=clearance_rng.choice([0,1,100,2000,32767,-1,-32768])
    position=[clearance_rng.choice([0,1,-1,side<<16,(side<<16)+65535,100<<16,2**31-1,-2**31,clearance_rng.randint(-2**31,2**31-1)]) for _ in range(2)]
    heading=clearance_rng.randint(-2**31,2**31-1)
    clearance_distance=clearance_rng.choice([0,1,-1,30<<16,80<<16,2**31-1,-2**31,clearance_rng.randint(-2**31,2**31-1)])
    clearance_width=clearance_rng.randint(-2**31,2**31-1)
    direction=[clearance_rng.choice([0,16384,-16384,32767,-32768,65536,clearance_rng.randint(-2**31,2**31-1)]) for _ in range(2)]
    hit_kind=clearance_rng.choice([None,0,7,31,32,33,255]);clearance_hit=0 if hit_kind is None else 5
    records=bytearray(12*256);struct.pack_into('>h',records,256+0x56,3)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(0x44100,struct.pack('>iii',*position,0))
    u.mem_write(0x44214,bytes([hit_kind or 0]));u.mem_write(a5-0x137e,struct.pack('>h',side))
    index=(heading>>16)%360
    u.mem_write(a5-0x3312+index*4,struct.pack('>i',direction[1]))
    u.mem_write(a5-0x38b2+index*4,struct.pack('>i',direction[0]))
    clearance_calls=[]
    run(0x1705b0,struct.pack('>iiiBB',clearance_width,clearance_distance,heading,2,0),instruction_limit=2000)
    result=u.mem_read(stack+18,1)[0]
    cases.append(dict(routine='ai_clearance_probe',a=heading,b=clearance_distance,position=position,side=side,
        direction=direction,hit_kind=hit_kind,calls=clearance_calls,expected=result))
u.hook_del(clearance_hook)
print('1000 complete original AI clearance probes executed with collision selection supplied')
# Compose the complete probe with the original collision selector and geometry.
# Candidate direction values remain modeled-SANE data; no service result is supplied.
for index,(cosine,sine) in enumerate(collision_table):
    u.mem_write(a5-0x38b2+index*4,struct.pack('>i',cosine))
    u.mem_write(a5-0x3312+index*4,struct.pack('>i',sine))
for displacement,target in [(0x53a,0x106ba),(0x4da,0x718ea),(0xbca,0x20dcc)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
clearance_scenes=[case for case in cases if case['routine']=='collision_query' and case.get('candidate_table')][:400]
clearance_outcomes=[0,0]
for scene in clearance_scenes:
    dimensions=[40<<16,80<<16]
    u.mem_write(0x42000,b''.join(bytes(r) for r in scene['records']))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    for index,state in enumerate(scene['states']):u.mem_write(0x45004+index*256,bytes([state]))
    u.mem_write(0x45156,struct.pack('>h',1))
    u.mem_write(a5-0x25ee,bytes(scene['slots']))
    for index,size in enumerate(scene['sizes']):
        u.mem_write(a5-0x2572+index*4,struct.pack('>i',size[0]));u.mem_write(a5-0x23e2+index*4,struct.pack('>i',size[1]))
    u.mem_write(a5-0x1380,struct.pack('>h',16));u.mem_write(a5-0x137e,struct.pack('>h',1024))
    u.mem_write(0x48000,b''.join(struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))) for cell in scene['grid']))
    u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    run(0x1705b0,struct.pack('>iiiBB',*dimensions,scene['pose'][2],2,0),instruction_limit=100000)
    result=u.mem_read(stack+18,1)[0];clearance_outcomes[result]+=1
    cases.append(dict(scene,routine='ai_clearance_scene',a=1,b=0,dimensions=dimensions,expected=result))
print(f'400 complete original AI clearance scenes executed without supplied service results: {clearance_outcomes}')
# Original reverse-state block, with actual angle wrapping and no service fixtures.
reverse_rng=random.Random(0x20197e)
reverse_inputs=[]
for participant in [1,2,12]:
    for angle in [-180,-130,-90,0,90,130,180]:
        for epsilon in [-1,0,1]:
            for allowed in [False,True]:
                for mode in [0,1,2,255]:
                    reverse_inputs.append(((angle<<16)+epsilon,0,allowed,participant,mode,2**31-1))
for _ in range(500):
    reverse_inputs.append((reverse_rng.randint(-2**31,2**31-1),reverse_rng.randint(-2**31,2**31-1),
        bool(reverse_rng.randrange(2)),reverse_rng.choice([1,2,12,-128,127]),reverse_rng.randrange(256),
        reverse_rng.randint(-2**31,2**31-1)))
u.mem_write(a5+0x522,bytes.fromhex('4ef9')+struct.pack('>I',0x10004));u.ctl_flush_tb()
for desired,reference,allowed,participant,mode,now in reverse_inputs:
    record=bytearray(reverse_rng.randbytes(128));struct.pack_into('>i',record,0x18,desired)
    u.mem_write(0x42000,bytes(record))
    u.reg_write(UC_M68K_REG_A7,0x4f000);u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_D6,reference&0xffffffff);u.reg_write(UC_M68K_REG_D7,participant&0xff)
    u.mem_write(stack-0x1f,bytes([int(allowed)]));u.mem_write(stack+0x10,bytes([mode]))
    u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.emu_start(0x17197e,0x171a72,count=2000)
    assert u.reg_read(UC_M68K_REG_PC)==0x171a72
    after=bytes(u.mem_read(0x42000,128))
    heading=struct.unpack_from('>i',after,0x18)[0];reverse=after[0x20];deadline=struct.unpack_from('>i',after,0x22)[0]
    record[0x18:0x1c]=after[0x18:0x1c];record[0x20]=after[0x20];record[0x22:0x26]=after[0x22:0x26]
    assert bytes(record)==after,'Reverse decision changed unrelated participant fields'
    cases.append(dict(routine='ai_reverse_state',a=desired,b=reference,allowed=allowed,participant=participant,mode=mode,now=now,
        result=[heading,reverse,deadline],expected=0))
print(f'{len(reverse_inputs)} original reverse-state blocks executed without service fixtures')
# Entire ordinary behavior-0/1/3 nonlocal movement policies. Aim and clearance
# services are explicit fixtures; all policy state, search, and commands execute.
policy_calls=[];policy_aim_calls=0;policy_angle=0;policy_mask=0
def policy_services(cpu,address,size,data):
    global policy_aim_calls
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x4c2:
        policy_aim_calls+=1
        target_ptr,source_ptr=struct.unpack('>II',cpu.mem_read(sp+4,8))
        assert list(struct.unpack('>iii',cpu.mem_read(target_ptr,12)))==[*target,0]
        assert list(struct.unpack('>iii',cpu.mem_read(source_ptr,12)))==source_pose
        cpu.mem_write(sp+12,struct.pack('>h',policy_angle));new_sp=sp+12
    else:
        width,distance,heading=struct.unpack('>iii',cpu.mem_read(sp+4,12))
        policy_calls.append([heading,width,distance])
        answer=bool(policy_mask & (1<<(len(policy_calls)-1)))
        cpu.mem_write(sp+18,bytes([int(answer),0]));new_sp=sp+18
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
for displacement,target in [(0x46a,0x7004c),(0x4ca,0x71760)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
policy_hooks=[u.hook_add(UC_HOOK_CODE,policy_services,begin=address,end=address) for address in [a5+0x4c2,0x1705b0]]
u.ctl_flush_tb()
for iteration in range(1200):
    source_pose=[reverse_rng.randrange(1024)<<16,reverse_rng.randrange(1024)<<16,reverse_rng.randrange(360)<<16]
    target=[reverse_rng.randrange(1024)<<16,reverse_rng.randrange(1024)<<16]
    now=reverse_rng.choice([0,19,20,21,2**31-1,-2**31,reverse_rng.randrange(10000)])
    state=dict(heading=reverse_rng.randrange(360)<<16,angle_threshold=reverse_rng.choice([0,1,40,90])<<16,
        reverse=reverse_rng.choice([0,1,255]),deadline=reverse_rng.choice([-1,now,now-1 if now>-2**31 else now,now+1 if now<2**31-1 else now]))
    policy_angle=reverse_rng.choice([-180,-130,-90,-1,0,1,90,130,180,32767,-32768])
    policy_mask=reverse_rng.choice([0,1,1<<24,2**26-1,reverse_rng.getrandbits(26)])
    records=bytearray(12*256);records[256+8]=7;records[256+9]=[0,1,3][iteration//400]
    struct.pack_into('>h',records,256+0x56,1);struct.pack_into('>iii',records,256+0x14,*target,0)
    struct.pack_into('>ii',records,256+0x20,state['heading'],state['angle_threshold'])
    records[256+0x28]=state['reverse'];struct.pack_into('>i',records,256+0x2a,state['deadline'])
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,struct.pack('>iii',*source_pose));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(0x41000,bytes.fromhex('12345678'))
    policy_calls=[];policy_aim_calls=0
    run(0x171672,struct.pack('>IIBBBB',0x41000,0x41002,0,0,2,0),instruction_limit=100000)
    after=bytes(u.mem_read(0x42000,12*256))
    updated=dict(heading=struct.unpack_from('>i',after,256+0x20)[0],angle_threshold=struct.unpack_from('>i',after,256+0x24)[0],
        reverse=after[256+0x28],deadline=struct.unpack_from('>i',after,256+0x2a)[0])
    records[256+0x20:256+0x29]=after[256+0x20:256+0x29]
    records[256+0x2a:256+0x2e]=after[256+0x2a:256+0x2e]
    assert bytes(records)==after,'Ordinary movement policy changed unrelated fields'
    result=list(struct.unpack('>hh',u.mem_read(0x41000,4)))
    cases.append(dict(routine='ai_standard_movement',a=now,b=policy_angle,source=source_pose,target=target,state=state,
        after=updated,result=result,calls=policy_calls,aim_calls=policy_aim_calls,mask=policy_mask,expected=0))
for hook in policy_hooks:u.hook_del(hook)
print('1200 complete original behavior-0/1/3 nonlocal movement policies executed with explicit aim and clearance services')
# Entire attack-tracking transition, including the behavior-4/9 early exits.
intent_rng=random.Random(0x200e6c)
def intent_state(record):
    return dict(behavior=record[9],target=struct.unpack_from('>h',record,10)[0],movement=record[16],
        aim_target=struct.unpack_from('>h',record,18)[0],heading_deadline=struct.unpack_from('>i',record,42)[0],
        field_26=struct.unpack_from('>h',record,46)[0])
for behavior in range(256):
    for target in [-128,1,12,127]:
        participant=intent_rng.randrange(1,13);offset=(participant-1)*256
        records=bytearray(intent_rng.randbytes(12*256));records[offset+9]=behavior
        state=intent_state(records[offset:offset+256])
        u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
        run(0x170e6c,struct.pack('>bBBB',target,0,participant,0))
        after=bytes(u.mem_read(0x42000,12*256));updated=intent_state(after[offset:offset+256])
        for start,end in [(9,12),(16,17),(18,20),(42,48)]:records[offset+start:offset+end]=after[offset+start:offset+end]
        assert bytes(records)==after,'Attack transition changed unrelated participant fields'
        cases.append(dict(routine='ai_begin_attack',a=target,b=0,state=state,after=updated,expected=0))
print('1024 complete original attack-tracking transitions executed without service fixtures')
# Execute dispatch block and capture actual callee and mode arguments.
dispatch_result=0
def dispatch_service(cpu,address,size,data):
    global dispatch_result
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==0x171672:
        steering,throttle,mode,_,participant,_=struct.unpack('>IIBBBB',cpu.mem_read(sp+4,12))
        assert (steering,throttle,participant)==(0x41000,0x41002,2)
        dispatch_result=1 if mode==0 else 2
        new_sp=sp+16
    else:
        steering,throttle,participant,_=struct.unpack('>IIBB',cpu.mem_read(sp+4,10))
        assert (steering,throttle,participant)==(0x41000,0x41002,2)
        dispatch_result=3 if address==0x171d7c else 4
        new_sp=sp+14
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
dispatch_hooks=[u.hook_add(UC_HOOK_CODE,dispatch_service,begin=address,end=address) for address in [0x171672,0x171d7c,0x1715fa]]
u.ctl_flush_tb()
for mode in range(256):
    for nearby in [False,True]:
        for distance in [(30<<16)-1,30<<16,(30<<16)+1,(120<<16)-1,120<<16,(120<<16)+1]:
            dispatch_result=0
            u.reg_write(UC_M68K_REG_A7,0x4f000);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A4,0x42000)
            u.reg_write(UC_M68K_REG_D5,int(nearby));u.reg_write(UC_M68K_REG_D6,distance);u.reg_write(UC_M68K_REG_D7,2)
            u.mem_write(0x42008,bytes([mode]));u.mem_write(stack+12,struct.pack('>II',0x41000,0x41002))
            u.emu_start(0x17253a,0x1725ca,count=1000)
            assert u.reg_read(UC_M68K_REG_PC)==0x1725ca
            cases.append(dict(routine='ai_movement_policy',a=mode,b=distance,nearby=nearby,expected=dispatch_result))
for hook in dispatch_hooks:u.hook_del(hook)
print('3072 original movement dispatches executed with callee arguments captured')
# Complete turn-only policy; the aim service is supplied, throttle must survive.
turn_only_angle=0
def turn_only_aim_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    assert cpu.mem_read(sp+4,1)[0]==2
    cpu.mem_write(sp+6,struct.pack('>h',turn_only_angle))
    cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,ret)
turn_hook=u.hook_add(UC_HOOK_CODE,turn_only_aim_service,begin=0x17031c,end=0x17031c)
u.ctl_flush_tb()
turn_values=[-32768,-180,-2,-1,0,1,2,180,32767]+[intent_rng.randint(-32768,32767) for _ in range(500)]
for turn_only_angle in turn_values:
    throttle=intent_rng.randint(-32768,32767)
    u.mem_write(0x41000,struct.pack('>hh',12345,throttle))
    run(0x171d7c,struct.pack('>IIBB',0x41000,0x41002,2,0))
    steering,after_throttle=struct.unpack('>hh',u.mem_read(0x41000,4))
    assert throttle==after_throttle,'Turn-only policy modified throttle'
    cases.append(dict(routine='ai_turn_only',a=turn_only_angle,b=0,expected=steering))
u.hook_del(turn_hook)
print('509 complete original turn-only policies executed with supplied aim results; throttle preserved')
# Complete nonlocal combat movement with predicted pose, cached angle, and
# clearance responses supplied. Preserve the original uninitialized local as
# an explicit fixture input rather than silently inventing a gameplay rule.
combat_calls=[];combat_aim_calls=0;combat_pose_calls=0;combat_angle=0;combat_mask=0;combat_pose=[0,0,0]
def combat_services(cpu,address,size,data):
    global combat_aim_calls,combat_pose_calls
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==0x1702e4:
        combat_pose_calls+=1
        pointer=struct.unpack('>I',cpu.mem_read(sp+6,4))[0]
        cpu.mem_write(pointer,struct.pack('>iii',*combat_pose));new_sp=sp+6
    elif address==0x17031c:
        combat_aim_calls+=1
        cpu.mem_write(sp+6,struct.pack('>h',combat_angle));new_sp=sp+6
    else:
        width,distance,heading=struct.unpack('>iii',cpu.mem_read(sp+4,12))
        combat_calls.append([heading,width,distance])
        cpu.mem_write(sp+18,bytes([int(bool(combat_mask & (1<<(len(combat_calls)-1)))),0]));new_sp=sp+18
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
combat_hooks=[u.hook_add(UC_HOOK_CODE,combat_services,begin=address,end=address) for address in [0x1702e4,0x17031c,0x1705b0]]
u.ctl_flush_tb()
combat_rng=random.Random(0x2016f0)
for iteration in range(700):
    source_pose=[500<<16,500<<16,combat_rng.randrange(360)<<16]
    separation=combat_rng.choice([0,(30<<16)-1,30<<16,(30<<16)+1,(120<<16)-1,120<<16,(120<<16)+1,300<<16])
    target=[source_pose[0]+separation,source_pose[1]]
    combat_pose=[*target,combat_rng.randrange(360)<<16]
    now=combat_rng.choice([0,19,20,21,2**31-1,-2**31,combat_rng.randrange(10000)])
    state=dict(heading=combat_rng.randrange(360)<<16,angle_threshold=combat_rng.choice([0,1,40,90])<<16,
        reverse=combat_rng.choice([0,1,255]),deadline=combat_rng.choice([-1,now,now+1 if now<2**31-1 else now]))
    combat_angle=combat_rng.choice([-180,-90,-1,0,1,90,180,32767,-32768])
    combat_mask=combat_rng.choice([0,1,1<<24,2**27-1,combat_rng.getrandbits(27)])
    scratch=combat_rng.randint(-2**31,2**31-1)
    records=bytearray(12*256);records[256+8]=7;records[256+9]=combat_rng.choice([0,3])
    struct.pack_into('>h',records,256+0x56,1)
    struct.pack_into('>ii',records,256+0x20,state['heading'],state['angle_threshold'])
    records[256+0x28]=state['reverse'];struct.pack_into('>i',records,256+0x2a,state['deadline'])
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,struct.pack('>iii',*source_pose));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(0x41000,bytes.fromhex('12345678'))
    u.mem_write(stack-4-0x24,struct.pack('>i',scratch))
    combat_calls=[];combat_aim_calls=0;combat_pose_calls=0
    run(0x171672,struct.pack('>IIBBBB',0x41000,0x41002,1,0,2,0),instruction_limit=100000)
    assert combat_pose_calls==1
    after=bytes(u.mem_read(0x42000,12*256))
    assert list(struct.unpack_from('>iii',after,256+0x14))==combat_pose
    updated=dict(heading=struct.unpack_from('>i',after,256+0x20)[0],angle_threshold=struct.unpack_from('>i',after,256+0x24)[0],
        reverse=after[256+0x28],deadline=struct.unpack_from('>i',after,256+0x2a)[0])
    for start,end in [(0x14,0x29),(0x2a,0x2e)]:records[256+start:256+end]=after[256+start:256+end]
    assert bytes(records)==after,'Combat policy modified unrelated fields'
    cases.append(dict(routine='ai_combat_movement',a=now,b=combat_angle,source=source_pose,target=target,state=state,
        after=updated,result=list(struct.unpack('>hh',u.mem_read(0x41000,4))),calls=combat_calls,
        aim_calls=combat_aim_calls,mask=combat_mask,scratch=scratch,expected=0))
for hook in combat_hooks:u.hook_del(hook)
print('700 complete nonlocal combat movement policies executed with explicit prediction, aim, clearance, and scratch-local fixtures')
# Full pose and angle getters share the real cache updater and angle wrapper.
# Prediction output and final angle are explicit services; scheduling is original.
cache_modes=[];cache_poses=[];cache_predicted=[0,0,0];cache_angle=0;cache_source=[0,0,0]
cache_target=0;cache_participant=2
def cache_services(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==0x170040:
        mode,_,target,_,participant,_=struct.unpack('>BBBBBB',cpu.mem_read(sp+4,6))
        assert target==cache_target&255 and participant==cache_participant
        cache_modes.append(mode)
        pointer=struct.unpack('>I',cpu.mem_read(sp+10,4))[0]
        cpu.mem_write(pointer,struct.pack('>iii',*cache_predicted));new_sp=sp+10
    else:
        target,source_pose=struct.unpack('>II',cpu.mem_read(sp+4,8))
        assert list(struct.unpack('>iii',cpu.mem_read(source_pose,12)))==cache_source
        cache_poses.append(list(struct.unpack('>iii',cpu.mem_read(target,12))))
        cpu.mem_write(sp+12,struct.pack('>h',cache_angle));new_sp=sp+12
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
cache_hooks=[u.hook_add(UC_HOOK_CODE,cache_services,begin=address,end=address) for address in [0x170040,a5+0x4c2]]
u.ctl_flush_tb()
cache_rng=random.Random(0x200256)
for iteration in range(800):
    now=cache_rng.choice([0,3,4,5,6,2**31-1,-2**31,cache_rng.randint(-2**31,2**31-1)])
    timestamp=cache_rng.choice([-1,now,((now-4+2**31)%2**32)-2**31,((now-5+2**31)%2**32)-2**31,((now-6+2**31)%2**32)-2**31])
    owner=cache_rng.choice([0,1,2,3,254,255]);kind=cache_rng.choice([7,8,255])
    cache_target=cache_rng.choice([-128,-1,1,12,127]);cache_participant=cache_rng.randrange(1,13)
    cache_predicted=[cache_rng.randint(-2**31,2**31-1) for _ in range(3)]
    cache_source=[cache_rng.randint(-2**31,2**31-1) for _ in range(3)]
    cache_angle=cache_rng.randint(-32768,32767)
    before=dict(pose=[cache_rng.randint(-2**31,2**31-1) for _ in range(3)],angle=cache_rng.randint(-32768,32767),timestamp=timestamp)
    records=bytearray(cache_rng.randbytes(12*256));offset=(cache_participant-1)*256
    records[offset+5]=owner;records[offset+8]=kind;records[offset+0x13]=cache_target&255
    struct.pack_into('>h',records,offset+0x56,1)
    struct.pack_into('>iiihi',records,offset+0x32,*before['pose'],before['angle'],timestamp)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,struct.pack('>iii',*cache_source));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x134c,struct.pack('>i',now))
    cache_modes=[];cache_poses=[]
    run(0x1702e4,struct.pack('>BBI',cache_participant,0,0x41000))
    output_pose=list(struct.unpack('>iii',u.mem_read(0x41000,12)))
    run(0x17031c,struct.pack('>BB',cache_participant,0))
    output_angle=struct.unpack('>h',u.mem_read(stack+6,2))[0]
    after_records=bytes(u.mem_read(0x42000,12*256))
    values=struct.unpack_from('>iiihi',after_records,offset+0x32)
    after=dict(pose=list(values[:3]),angle=values[3],timestamp=values[4])
    assert output_pose==after['pose'] and output_angle==after['angle']
    records[offset+0x32:offset+0x44]=after_records[offset+0x32:offset+0x44]
    assert bytes(records)==after_records,'Aim cache modified unrelated participant fields'
    cases.append(dict(routine='ai_aim_cache',a=now,b=cache_angle,owner=owner,kind=kind,before=before,after=after,
        predicted=cache_predicted,modes=cache_modes,poses=cache_poses,expected=0))
for hook in cache_hooks:u.hook_del(hook)
print('800 paired original pose/angle getter calls verified through the real cache; prediction and angle services supplied')
# Normal type-7 enemy cache uses the actual mode-zero targeting routine.
# Observe its mode without replacing execution; only final aim is supplied.
def observe_target_copy(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    mode=cpu.mem_read(sp+4,1)[0]
    assert mode==0
    cache_modes.append(mode)
cache_hooks=[u.hook_add(UC_HOOK_CODE,observe_target_copy,begin=0x170040,end=0x170040),
    u.hook_add(UC_HOOK_CODE,cache_services,begin=a5+0x4c2,end=a5+0x4c2)]
u.ctl_flush_tb()
for iteration in range(200):
    now=cache_rng.randrange(10000);timestamp=now-cache_rng.choice([0,4,5,6])
    cache_predicted=[cache_rng.randint(-2**31,2**31-1) for _ in range(3)]
    cache_source=[cache_rng.randint(-2**31,2**31-1) for _ in range(3)]
    cache_angle=cache_rng.randint(-32768,32767)
    before=dict(pose=[cache_rng.randint(-2**31,2**31-1) for _ in range(3)],angle=cache_rng.randint(-32768,32767),timestamp=timestamp)
    records=bytearray(cache_rng.randbytes(12*256));offset=256
    records[offset+5]=1;records[offset+8]=7;records[offset+0x13]=1
    struct.pack_into('>h',records,0x56,1);struct.pack_into('>h',records,offset+0x56,2)
    struct.pack_into('>iiihi',records,offset+0x32,*before['pose'],before['angle'],timestamp)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,struct.pack('>iii',*cache_predicted));u.mem_write(0x44080,struct.pack('>iii',*cache_source))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x134c,struct.pack('>i',now))
    cache_modes=[];cache_poses=[]
    run(0x1702e4,struct.pack('>BBI',2,0,0x41000))
    output_pose=list(struct.unpack('>iii',u.mem_read(0x41000,12)))
    run(0x17031c,struct.pack('>BB',2,0))
    output_angle=struct.unpack('>h',u.mem_read(stack+6,2))[0]
    after_records=bytes(u.mem_read(0x42000,12*256));values=struct.unpack_from('>iiihi',after_records,offset+0x32)
    after=dict(pose=list(values[:3]),angle=values[3],timestamp=values[4])
    assert output_pose==after['pose'] and output_angle==after['angle']
    records[offset+0x32:offset+0x44]=after_records[offset+0x32:offset+0x44]
    assert bytes(records)==after_records
    cases.append(dict(routine='ai_aim_cache',a=now,b=cache_angle,owner=1,kind=7,before=before,after=after,
        predicted=cache_predicted,modes=cache_modes,poses=cache_poses,expected=0))
for hook in cache_hooks:u.hook_del(hook)
print('200 paired type-7 pose/angle getters execute original target-copy logic; only final angle supplied')
# Complete angular-rate getter: resource/participant selection and half-rate flag.
motion_rng=random.Random(0x3017e6)
for iteration in range(600):
    enemy=motion_rng.choice([0,1,2,255]);half=motion_rng.choice([0,1,255]);kind=motion_rng.randrange(47)
    enemy_rate=motion_rng.choice([-2**31,2**31-1,-3,-1,0,1,3,motion_rng.randint(-2**31,2**31-1)])
    player_rate=motion_rng.randint(-2**31,2**31-1)
    records=bytearray(12*256);records[256+5]=enemy;records[256+8]=kind
    struct.pack_into('>i',records,256+0x6c,player_rate)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000+0x40c+kind*4,struct.pack('>i',enemy_rate))
    u.mem_write(a5-0x3cf8,bytes([half]))
    value=run(0xf05b8,struct.pack('>BB',2,0))
    cases.append(dict(routine='participant_angular_rate',a=enemy_rate,b=player_rate,enemy=enemy,half=half,expected=value))
print('600 complete original angular-rate getters executed without supplied service results')
# Original movement caller block includes angular-rate getter, integer products,
# heading update, actual direction lookup, and entire kind-7/kind-8 translation routine.
for displacement,target in [(0xe02,0xf05b8),(0xbc2,0x20d78),(0x1282,0x90004),(0x128a,0x90038)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.mem_write(a5-0x38c2,struct.pack('>I',0x43000));u.mem_write(0x43000,struct.pack('>360i',*table))
u.ctl_flush_tb()
for iteration in range(1600):
    kind=7 if iteration<800 else 8
    pose=[motion_rng.randint(-2**31,2**31-1),motion_rng.randint(-2**31,2**31-1),motion_rng.randrange(360<<16)]
    commands=[motion_rng.choice([-32768,-1,0,1,32767]) for _ in range(2)]
    radial=motion_rng.choice([-32768,-5,-1,0,1,5,32767]);angular=motion_rng.choice([-2**31,2**31-1,-65536,0,65536,3<<16])
    elapsed=motion_rng.choice([-63,-4,-1,0,1,4,63]);half=motion_rng.randrange(2)
    rate=int(angular/2) if half else angular
    record=bytearray(motion_rng.randbytes(128));struct.pack_into('>iii',record,0,*pose)
    struct.pack_into('>hh',record,12,*commands);record[0x14]=kind;record[0x1a]=2
    u.mem_write(0x42000,bytes(record));u.mem_write(a5-0x1346,struct.pack('>I',0x42000))
    participants=bytearray(12*256);participants[256+5]=1;participants[256+8]=kind
    u.mem_write(0x44000,bytes(participants));u.mem_write(a5-0x1342,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000+0x3ae+kind*2,struct.pack('>h',radial))
    u.mem_write(0x45000+0x40c+kind*4,struct.pack('>i',angular));u.mem_write(a5-0x3cf8,bytes([half]))
    u.reg_write(UC_M68K_REG_A7,0x4f000);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A4,0x42000)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_D6,1);u.reg_write(UC_M68K_REG_D7,elapsed&0xffffffff)
    u.mem_write(stack+12,bytes(2))
    u.emu_start(0x82578,0x825e8,count=100000)
    assert u.reg_read(UC_M68K_REG_PC)==0x825e8
    after_record=bytes(u.mem_read(0x42000,128));after=list(struct.unpack_from('>iii',after_record,0))
    record[:12]=after_record[:12];assert bytes(record)==after_record,'Type-7 motion changed other geometry fields'
    cases.append(dict(routine='type7_motion',kind=kind,a=rate,b=elapsed,pose=pose,commands=commands,radial=radial,after=after,expected=0))
print('1600 original kind-7/kind-8 movement caller blocks executed with original getter, turn, translation, and marker direction table')
# Original nearby-predecessor scan and full distance/geometry lookup helpers.
for displacement,target in [(0x4d2,0x717c6),(0x4ca,0x71760)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
near_rng=random.Random(0x202290)
near_count=0
for iteration in range(800):
    participant=iteration%12+1
    flags=[[near_rng.choice([0,1,2,3,128,255]) for _ in range(2)] for _ in range(12)]
    positions=[[500<<16,500<<16] for _ in range(12)]
    for i in range(12):
        positions[i]=[(500<<16)+near_rng.choice([0,-1,1,(60<<16)-1,60<<16,(60<<16)+1,-60<<16]),500<<16]
    positions[participant-1]=[500<<16,500<<16]
    ids=list(range(1,13));near_rng.shuffle(ids)
    records=bytearray(12*256);geometry=bytearray(12*128)
    for i in range(12):
        records[i*256+0x88]=flags[i][0];records[i*256+5]=flags[i][1]
        struct.pack_into('>h',records,i*256+0x56,ids[i])
        struct.pack_into('>iii',geometry,(ids[i]-1)*128,*positions[i],0)
    u.mem_write(0x42000,bytes(records));u.mem_write(0x44000,bytes(geometry))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,stack);u.reg_write(UC_M68K_REG_A7,0x4f000)
    u.reg_write(UC_M68K_REG_D7,participant)
    u.emu_start(0x172290,0x1722e4,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x1722e4
    result=u.reg_read(UC_M68K_REG_D5)&255;near_count+=result
    assert bytes(u.mem_read(0x42000,len(records)))==records
    assert bytes(u.mem_read(0x44000,len(geometry)))==geometry
    cases.append(dict(routine='ai_nearby_predecessor',a=participant,b=0,flags=flags,positions=positions,expected=result))
print(f'800 original nearby-predecessor scans executed without service fixtures: {near_count} yielding cases')
# Intent timing/range/mask gate and original behavior jump table.
# Capture selected callees and their arguments; no handler decisions are supplied.
intent_action=[None,None];intent_participant=2;intent_distance=0
def intent_action_service(cpu,address,size,data):
    global intent_action
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    entry=address-0x170000
    if entry==0xe6c:
        target=struct.unpack('>b',cpu.mem_read(sp+4,1))[0]
        assert cpu.mem_read(sp+6,1)[0]==intent_participant
        intent_action=[entry,target];new_sp=sp+8
    elif entry==0xfb8:
        assert struct.unpack('>i',cpu.mem_read(sp+4,4))[0]==intent_distance
        assert cpu.mem_read(sp+8,1)[0]==intent_participant
        intent_action=[entry,None];new_sp=sp+10
    else:
        assert cpu.mem_read(sp+4,1)[0]==intent_participant
        intent_action=[entry,None];new_sp=sp+6
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
intent_hooks=[u.hook_add(UC_HOOK_CODE,intent_action_service,begin=0x170000+entry,end=0x170000+entry)
    for entry in [0xe6c,0x1240,0x153a,0xef4,0xfb8,0x1140,0x1042,0x14be]]
u.ctl_flush_tb()
intent_inputs=[]
for kind in range(47):
    for behavior in range(14):
        for movement in [0,1]:
            for distance in [250<<16,(250<<16)+1]:
                intent_inputs.append(dict(now=100,last_check=-1,distance=distance,attack_range=250<<16,kind=kind,
                    behavior=behavior,movement=movement,game_mode=0,target=1,participant=2))
for _ in range(600):
    now=near_rng.choice([0,15,16,2**31-1,-2**31,near_rng.randint(-2**31,2**31-1)])
    last=near_rng.choice([-1,0,((now-15+2**31)%2**32)-2**31,((now-16+2**31)%2**32)-2**31])
    intent_inputs.append(dict(now=now,last_check=last,distance=near_rng.choice([0,250<<16,(250<<16)+1]),
        attack_range=250<<16,kind=near_rng.randrange(47),behavior=near_rng.randint(-128,127),movement=near_rng.randrange(5),
        game_mode=near_rng.choice([0,1,255]),target=near_rng.choice([0,1,2,12,257,-1,-32768,32767]),participant=near_rng.randrange(1,13)))
for event in intent_inputs:
    intent_action=[None,None];intent_participant=event['participant'];intent_distance=event['distance']
    record=bytearray(128);record[0]=event['kind'];record[1]=event['behavior']&255;record[8]=event['movement']
    struct.pack_into('>h',record,2,event['target']);struct.pack_into('>i',record,4,event['last_check'])
    u.mem_write(0x42000,bytes(record));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(0x45000+0x17a+event['kind']*4,struct.pack('>i',event['attack_range']))
    u.mem_write(a5-0x134c,struct.pack('>i',event['now']));u.mem_write(a5-0x12ca,bytes([event['game_mode']]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x42000);u.reg_write(UC_M68K_REG_A7,0x4f000)
    u.reg_write(UC_M68K_REG_D6,event['distance']&0xffffffff);u.reg_write(UC_M68K_REG_D7,event['participant'])
    u.emu_start(0x1723ac,0x1724d4,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x1724d4
    assert bytes(u.mem_read(0x42000,128))==record
    cases.append(dict(routine='ai_intent_action',a=0,b=0,input=event,result=intent_action,expected=0))
for hook in intent_hooks:u.hook_del(hook)
print(f'{len(intent_inputs)} original intent-dispatch blocks executed with callee arguments captured')
# Kind-7 tracking tail: original state writes, supplied cached-pose service.
tracking_calls=0
tracking_supplied=[0,0,0]
def tracking_pose_service(cpu,address,size,data):
    global tracking_calls
    sp=cpu.reg_read(UC_M68K_REG_A7)
    assert cpu.mem_read(sp+4,1)[0]==2
    destination=struct.unpack('>I',cpu.mem_read(sp+6,4))[0]
    cpu.mem_write(destination,struct.pack('>iii',*tracking_supplied))
    tracking_calls+=1
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,ret)
tracking_hook=u.hook_add(UC_HOOK_CODE,tracking_pose_service,begin=0x1702e4,end=0x1702e4)
u.ctl_flush_tb()
def tracking_state(record):
    return dict(behavior=record[1],target=struct.unpack_from('>h',record,2)[0],movement=record[8],
        aim_target=struct.unpack_from('>h',record,10)[0],heading_deadline=struct.unpack_from('>i',record,0x22)[0],
        field_26=struct.unpack_from('>h',record,0x26)[0])
for iteration in range(4096):
    record=bytearray(rng.randbytes(128));record[0]=7;record[8]=iteration%256
    mode=[0,1,2,255][(iteration//256)%4];active=[0,1,128,255][iteration//1024]
    before=tracking_state(record);pose_before=list(struct.unpack_from('>iii',record,12))
    tracking_supplied=[rng.randint(-2**31,2**31-1) for _ in range(3)];tracking_calls=0
    u.mem_write(0x42108,bytes(record));u.mem_write(0x42188,bytes([active]))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x12ca,bytes([mode]))
    u.reg_write(UC_M68K_REG_A4,0x42108);u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A6,0x4e000);u.reg_write(UC_M68K_REG_A7,0x4d000);u.reg_write(UC_M68K_REG_D6,2)
    u.emu_start(0x171284,0x1714ae,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x1714ae
    after=bytes(u.mem_read(0x42108,128));pose_after=list(struct.unpack_from('>iii',after,12))
    expected_unchanged=bytearray(record)
    for lo,hi in [(1,4),(8,9),(10,24),(0x22,0x26)]:expected_unchanged[lo:hi]=after[lo:hi]
    assert bytes(expected_unchanged)==after
    assert tracking_calls in [0,1]
    cases.append(dict(routine='ai_type7_tracking_tail',a=mode,b=active,before=before,after=tracking_state(after),
        pose_before=pose_before,pose_after=pose_after,supplied=tracking_supplied,called=bool(tracking_calls),expected=0))
u.hook_del(tracking_hook)
print('4096 original kind-7 tracking tails executed; cached pose supplied, retarget prelude excluded')
# Complete target selector; only the final angle service is supplied.
selection_calls=[];selection_participants=[];selection_angles=[]
def selection_angle_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    ptrs=struct.unpack('>II',cpu.mem_read(sp+4,8))
    poses=[list(struct.unpack('>iii',cpu.mem_read(ptr,12))) for ptr in ptrs]
    selection_calls.append(poses)
    index=next(i for i,p in enumerate(selection_participants) if p['pose']==poses[0])
    cpu.mem_write(sp+12,struct.pack('>h',selection_angles[index]))
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+12);cpu.reg_write(UC_M68K_REG_PC,ret)
selection_hook=u.hook_add(UC_HOOK_CODE,selection_angle_service,begin=a5+0x4c2,end=a5+0x4c2)
u.mem_write(a5+0x4ca,bytes.fromhex('4ef9')+struct.pack('>I',0x71760))
u.ctl_flush_tb()
for iteration in range(1000):
    source_id=rng.randrange(1,13);excluded=rng.randrange(0,13)
    mode=rng.choice([0,1,2,255]);game_mode=rng.choice([0,1,2,255])
    selection_participants=[];selection_angles=[];selection_calls=[]
    records=bytearray(rng.randbytes(12*256));geometry=bytearray(12*128)
    for i in range(12):
        pose=[rng.randrange(-320,321)<<16,rng.randrange(-100,101)<<16,i<<16]
        if iteration%3==0:pose[0]=rng.choice([0,149,150,151,199,200,201,299,300,301])<<16;pose[1]=0
        p=dict(team=rng.randrange(3),owner=rng.choice([0,1,2,255]),active=rng.choice([0,1,2,255]),
            health=rng.choice([-1,0,1,100]),kind=rng.choice([7,8,21]),pose=pose)
        selection_participants.append(p);selection_angles.append(rng.choice([-32768,-180,-46,-45,0,45,46,180,32767]))
        off=i*256;records[off+4]=p['team'];records[off+5]=p['owner'];records[off+8]=p['kind'];records[off+0x88]=p['active']
        struct.pack_into('>h',records,off+0x5a,p['health']);struct.pack_into('>h',records,off+0x56,i+1)
        struct.pack_into('>iii',geometry,i*128,*pose)
    u.mem_write(0x42000,bytes(records));u.mem_write(0x44000,bytes(geometry))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x12ca,bytes([game_mode]))
    run(0x170754,bytes([mode,0,excluded,0,source_id,0]))
    selected=u.mem_read(stack+10,1)[0]
    assert bytes(u.mem_read(0x42000,len(records)))==records
    assert bytes(u.mem_read(0x44000,len(geometry)))==geometry
    cases.append(dict(routine='ai_select_target',a=source_id,b=excluded,mode=mode,game_mode=game_mode,
        participants=selection_participants,angles=selection_angles,calls=selection_calls,expected=selected))
for iteration in range(1024):
    source_id=rng.randrange(1,13);excluded=rng.randrange(0,13)
    mode=rng.choice([0,1,2,255]);game_mode=iteration%256
    selection_participants=[];selection_angles=[];selection_calls=[]
    records=bytearray(rng.randbytes(12*256));geometry=bytearray(12*128)
    for i in range(12):
        pose=[rng.randrange(-320,321)<<16,rng.randrange(-100,101)<<16,i<<16]
        if iteration%3==0:pose[0]=rng.choice([0,149,150,151,199,200,201,299,300,301])<<16;pose[1]=0
        p=dict(team=rng.randrange(3),owner=rng.choice([0,1,2,255]),active=rng.choice([0,1,2,255]),
            health=rng.choice([-1,0,1,100]),kind=rng.choice([7,8,21]),pose=pose)
        selection_participants.append(p);selection_angles.append(rng.choice([-32768,-180,-46,-45,0,45,46,180,32767]))
        off=i*256;records[off+4]=p['team'];records[off+5]=p['owner'];records[off+8]=p['kind'];records[off+0x88]=p['active']
        struct.pack_into('>h',records,off+0x5a,p['health']);struct.pack_into('>h',records,off+0x56,i+1)
        struct.pack_into('>iii',geometry,i*128,*pose)
    u.mem_write(0x42000,bytes(records));u.mem_write(0x44000,bytes(geometry))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x12ca,bytes([game_mode]))
    run(0x170a22,bytes([source_id,0]))
    selected=u.mem_read(stack+6,1)[0]
    assert bytes(u.mem_read(0x42000,len(records)))==records
    assert bytes(u.mem_read(0x44000,len(geometry)))==geometry
    cases.append(dict(routine='ai_acquire_opponent',a=source_id,b=excluded,mode=mode,game_mode=game_mode,
        participants=selection_participants,angles=selection_angles,calls=selection_calls,expected=selected))
print('1024 complete original opponent-acquisition calls executed; angle service supplied only for modes 4/12')
u.hook_del(selection_hook)
print('1000 complete original target selectors executed; final angle service supplied')
# Complete type-7 tracking handler, including local-participant attack writes.
full_tracking_selections=[];full_tracking_poses=[];full_tracking_selected=0
full_tracking_supplied=[0,0,0]
def full_tracking_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==0x170754:
        args=bytes(cpu.mem_read(sp+4,6));signed=lambda v:v if v<128 else v-256
        full_tracking_selections.append([signed(args[4]),signed(args[2]),args[0]])
        cpu.mem_write(sp+10,bytes([full_tracking_selected&255]));new_sp=sp+10
    else:
        participant=cpu.mem_read(sp+4,1)[0]
        record=bytes(cpu.mem_read(0x42000+(participant-1)*256+8,128))
        full_tracking_poses.append([participant,tracking_state(record)])
        ptr=struct.unpack('>I',cpu.mem_read(sp+6,4))[0]
        cpu.mem_write(ptr,struct.pack('>iii',*full_tracking_supplied));new_sp=sp+6
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
def full_tracking_states(records):
    result=[]
    for i in range(12):
        offset=i*256;record=records[offset+8:offset+136]
        result.append(dict(intent=tracking_state(record),tracked_pose=list(struct.unpack_from('>iii',record,12)),
            counter=struct.unpack_from('>h',record,0x40)[0],active=records[offset+0x88]))
    return result
full_tracking_hooks=[u.hook_add(UC_HOOK_CODE,full_tracking_service,begin=addr,end=addr) for addr in [0x170754,0x1702e4]]
u.ctl_flush_tb()
for iteration in range(2400):
    kind=7 if iteration<1200 else 8
    participant=rng.randrange(1,13);local=participant if iteration%2 else rng.randrange(1,13)
    game_mode=rng.choice([0,1,255]);full_tracking_selected=rng.choice([0,1,12,-128,127])
    records=bytearray(rng.randbytes(12*256))
    for i in range(12):
        offset=i*256;records[offset+8]=kind
        records[offset+9]=rng.choice([0,3,4,9,255]);records[offset+16]=rng.choice([0,1,3,4,255])
        records[offset+0x88]=rng.choice([0,1,128,255])
        struct.pack_into('>h',records,offset+0x48,rng.choice([-32768,-1,0,4,5,6,32767]))
    before=full_tracking_states(records)
    full_tracking_supplied=[rng.randint(-2**31,2**31-1) for _ in range(3)]
    full_tracking_selections=[];full_tracking_poses=[]
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x12ca,bytes([game_mode]));u.mem_write(a5-0x133d,bytes([local]))
    run(0x171240,bytes([participant,0]))
    after_records=bytes(u.mem_read(0x42000,len(records)));after=full_tracking_states(after_records)
    unchanged=bytearray(records)
    for i in range(12):
        offset=i*256+8
        for lo,hi in [(1,4),(8,9),(10,24),(0x22,0x28)]:
            unchanged[offset+lo:offset+hi]=after_records[offset+lo:offset+hi]
    assert bytes(unchanged)==after_records
    cases.append(dict(routine='ai_type7_tracking',kind=kind,a=participant,b=local,game_mode=game_mode,
        before=before,after=after,supplied=full_tracking_supplied,selected=full_tracking_selected,
        selections=full_tracking_selections,poses=full_tracking_poses,called=bool(full_tracking_poses),expected=0))
for iteration in range(2048):
    participant=rng.randrange(1,13);local=participant if iteration%2 else rng.randrange(1,13)
    game_mode=rng.choice([0,1,255]);full_tracking_selected=rng.choice([0,1,12,-128,127])
    records=bytearray(rng.randbytes(12*256))
    for i in range(12):
        offset=i*256;records[offset+8]=7
        records[offset+9]=rng.choice([0,3,4,9,255]);records[offset+16]=rng.choice([0,1,3,4,255])
        records[offset+0x88]=rng.choice([0,1,128,255])
        struct.pack_into('>h',records,offset+0x48,rng.choice([-32768,-1,0,4,5,6,32767]))
    kind=rng.choice([23,24]);records[(participant-1)*256+8]=kind
    now=rng.choice([-2**31,2**31-1,rng.randint(-2**31,2**31-1)])
    deadline=rng.choice([-1,now,rng.randint(-2**31,2**31-1)])
    struct.pack_into('>i',records,(participant-1)*256+0x4e,deadline)
    u.mem_write(a5-0x134c,struct.pack('>i',now))
    before=full_tracking_states(records)
    full_tracking_supplied=[rng.randint(-2**31,2**31-1) for _ in range(3)]
    full_tracking_selections=[];full_tracking_poses=[]
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x12ca,bytes([game_mode]));u.mem_write(a5-0x133d,bytes([local]))
    run(0x171240,bytes([participant,0]))
    after_records=bytes(u.mem_read(0x42000,len(records)));after=full_tracking_states(after_records)
    unchanged=bytearray(records)
    for i in range(12):
        offset=i*256+8
        for lo,hi in [(1,4),(8,9),(10,24),(0x22,0x28)]:
            unchanged[offset+lo:offset+hi]=after_records[offset+lo:offset+hi]
    phase_offset=(participant-1)*256+0x4e
    unchanged[phase_offset:phase_offset+4]=after_records[phase_offset:phase_offset+4]
    assert bytes(unchanged)==after_records
    assert u.reg_read(UC_M68K_REG_A7)==stack+6
    deadline_after=struct.unpack_from('>i',after_records,phase_offset)[0]
    cases.append(dict(routine='ai_surface_tracking',a=participant,b=local,game_mode=game_mode,kind=kind,now=now,deadline=deadline,deadline_after=deadline_after,
        before=before,after=after,supplied=full_tracking_supplied,selected=full_tracking_selected,
        selections=full_tracking_selections,poses=full_tracking_poses,called=bool(full_tracking_poses),expected=0))
print('2048 complete original kind-23/kind-24 tracking handlers executed; target selection and cached pose supplied, all participant changes, call order, deadlines and stack checked')
for hook in full_tracking_hooks:u.hook_del(hook)
print('2400 complete original kind-7/kind-8 tracking handlers executed; selection and cached-pose services supplied')
# Complete mode-zero kind-7 initializer. Search/distance/cache services supplied.
init_opponent=1;init_object=0;init_distance=0;init_poses=[];init_calls=[];init_pose=[0,0,0]
def init_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    init_calls.append(address-0x170000)
    if address in [0x170a22,0x1703b2]:
        assert cpu.mem_read(sp+4,1)[0]==2
        if address==0x170a22:cpu.mem_write(sp+6,bytes([init_opponent&255]))
        else:cpu.mem_write(sp+6,struct.pack('>h',init_object))
        new_sp=sp+6
    elif address==0x170004:
        assert cpu.mem_read(sp+4,1)[0]==1 and cpu.mem_read(sp+6,1)[0]==2
        cpu.mem_write(sp+8,struct.pack('>i',init_distance));new_sp=sp+8
    else:
        assert cpu.mem_read(sp+4,1)[0]==2
        init_poses.append(tracking_state(bytes(cpu.mem_read(0x42108,128))))
        ptr=struct.unpack('>I',cpu.mem_read(sp+6,4))[0]
        cpu.mem_write(ptr,struct.pack('>iii',*init_pose));new_sp=sp+6
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
init_hooks=[u.hook_add(UC_HOOK_CODE,init_service,begin=addr,end=addr) for addr in [0x170a22,0x1703b2,0x170004,0x1702e4]]
u.ctl_flush_tb()
for iteration in range(800):
    records=bytearray(rng.randbytes(12*256));records[256+8]=7
    init_opponent=rng.choice([0,1,12,-128,127]);init_object=rng.choice([0,1,32767,-32768,-1])
    attack_range=rng.choice([0,1,250<<16,0x40000000,0x7fffffff,-1,-2**31])
    twice=((attack_range*2+2**31)%2**32)-2**31
    init_distance=rng.choice([twice,max(-2**31,twice-1),min(2**31-1,twice+1),rng.randint(-2**31,2**31-1)])
    init_pose=[rng.randint(-2**31,2**31-1) for _ in range(3)];init_calls=[];init_poses=[]
    before=full_tracking_states(records)[1];timer=struct.unpack_from('>i',records,256+12)[0]
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000+0x17a+7*4,struct.pack('>i',attack_range))
    run(0x170b6c,bytes([0,0,2,0]))
    after_records=bytes(u.mem_read(0x42000,len(records)));after=full_tracking_states(after_records)[1]
    assert struct.unpack_from('>i',after_records,256+12)[0]==-1
    assert init_calls==[0xa22,0x3b2,4]+([0x2e4] if init_poses else [])
    unchanged=bytearray(records);offset=256+8
    for lo,hi in [(1,9),(10,24),(0x22,0x26)]:unchanged[offset+lo:offset+hi]=after_records[offset+lo:offset+hi]
    assert bytes(unchanged)==after_records
    cases.append(dict(routine='ai_initialize_type7',a=init_opponent,b=init_object,distance=init_distance,range=attack_range,
        before=before,after=after,timer=timer,supplied=init_pose,poses=init_poses,called=bool(init_poses),expected=0))
for iteration in range(1600):
    kind=rng.choice([23,24]) if iteration<800 else 8;records=bytearray(rng.randbytes(12*256));records[256+8]=kind
    init_opponent=rng.choice([0,1,12,-128,127]);init_object=rng.choice([0,1,32767,-32768,-1])
    attack_range=rng.choice([0,1,250<<16,0x40000000,0x7fffffff,-1,-2**31])
    twice=((attack_range*2+2**31)%2**32)-2**31
    init_distance=rng.choice([twice,max(-2**31,twice-1),min(2**31-1,twice+1),rng.randint(-2**31,2**31-1)])
    init_pose=[rng.randint(-2**31,2**31-1) for _ in range(3)];init_calls=[];init_poses=[]
    before=full_tracking_states(records)[1];timer=struct.unpack_from('>i',records,256+12)[0]
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000+0x17a+kind*4,struct.pack('>i',attack_range))
    run(0x170b6c,bytes([0,0,2,0]))
    after_records=bytes(u.mem_read(0x42000,len(records)));after=full_tracking_states(after_records)[1]
    assert struct.unpack_from('>i',after_records,256+12)[0]==-1
    assert init_calls==[0xa22,4]+([0x2e4] if init_poses else [])
    unchanged=bytearray(records);offset=256+8
    for lo,hi in [(1,9),(10,24),(0x22,0x26)]:unchanged[offset+lo:offset+hi]=after_records[offset+lo:offset+hi]
    assert bytes(unchanged)==after_records
    cases.append(dict(routine='ai_initialize_type7' if kind==8 else 'ai_initialize_surface',a=init_opponent,b=0,kind=kind,distance=init_distance,range=attack_range,
        before=before,after=after,timer=timer,supplied=init_pose,poses=init_poses,called=bool(init_poses),expected=0))
print('1600 complete mode-zero surface-enemy/kind-8 initializers executed; opponent, distance and cached pose supplied; absence of object search and all record writes checked')
for hook in init_hooks:u.hook_del(hook)
print('800 complete mode-zero type-7 initializers executed; searches, distance and cached pose supplied')
# Complete object tracking, including original planar distance.
u.mem_write(a5+0x4ca,bytes.fromhex('4ef9')+struct.pack('>I',0x71760));u.ctl_flush_tb()
for iteration in range(600):
    participant=rng.choice([1,2,12]);records=bytearray(rng.randbytes(12*256));offset=(participant-1)*256
    struct.pack_into('>h',records,offset+0x56,1);struct.pack_into('>h',records,offset+10,2)
    source_pose=[0,0,rng.randint(-2**31,2**31-1)]
    object_pose=[rng.choice([(40<<16)-1,40<<16,(40<<16)+1,rng.randrange(-100,101)<<16]),
        0 if iteration%2 else rng.randrange(-100,101)<<16,rng.randint(-2**31,2**31-1)]
    before=full_tracking_states(records)[participant-1]
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(0x44000,struct.pack('>iii',*source_pose));u.mem_write(0x44080,struct.pack('>iii',*object_pose))
    run(0x17153a,bytes([participant,0]))
    after_records=bytes(u.mem_read(0x42000,len(records)));after=full_tracking_states(after_records)[participant-1]
    unchanged=bytearray(records)
    for lo,hi in [(8,9),(10,24),(0x22,0x26)]:unchanged[offset+8+lo:offset+8+hi]=after_records[offset+8+lo:offset+8+hi]
    assert bytes(unchanged)==after_records
    cases.append(dict(routine='ai_track_object',a=participant,b=0,before=before,after=after,
        source_pose=source_pose,object_pose=object_pose,expected=0))
print('600 complete original object tracking handlers executed without service fixtures')
# Complete object selector and assignment counter; only Random trap supplied.
object_samples=[];object_consumed=0
def object_random_service(cpu,address,size,data):
    global object_consumed
    sp=cpu.reg_read(UC_M68K_REG_A7)
    cpu.mem_write(sp,struct.pack('>h',object_samples[object_consumed]));object_consumed+=1
    cpu.reg_write(UC_M68K_REG_PC,address+2)
object_hook=u.hook_add(UC_HOOK_CODE,object_random_service,begin=0x1704b0,end=0x1704b0)
u.ctl_flush_tb()
for iteration in range(800):
    count=rng.randrange(2,33);records=bytearray(rng.randbytes(12*256));geometry=bytearray(count*128)
    participants=[];objects=[]
    for i in range(12):
        p=dict(active=rng.choice([0,1,2,255]),owner=rng.choice([0,1,2,255]),
            behavior=rng.choice([0,1,1,3,255]),target=rng.randrange(1,count+1))
        participants.append(p);off=i*256
        records[off+0x88]=p['active'];records[off+5]=p['owner'];records[off+9]=p['behavior']
        struct.pack_into('>h',records,off+10,p['target']);struct.pack_into('>h',records,off+0x56,1 if i==0 else 2)
    for i in range(count):
        position=[rng.randrange(-400,401)<<16,rng.randrange(-400,401)<<16]
        kind=rng.choice([5,7,12,12,14]);struct.pack_into('>ii',geometry,i*128,*position);geometry[i*128+0x14]=kind
        objects.append(dict(kind=kind,position=position,assignments=0))
    level=rng.choice([-32768,0,24,25,26,32767]);remaining=rng.choice([-32768,-7,-6,-1,0,5,6,7,12,32767])
    object_samples=[rng.choice([-32768,-1,0,1,32767,rng.randint(-32768,32767)]) for _ in range(count)]
    object_consumed=0
    u.mem_write(0x42000,bytes(records));u.mem_write(0x44000,bytes(geometry))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x136e,struct.pack('>h',level));u.mem_write(a5-0x1372,struct.pack('>h',remaining))
    u.mem_write(a5-0x12c8,struct.pack('>h',count))
    for i,obj in enumerate(objects):
        run(0x170346,struct.pack('>h',i+1));obj['assignments']=struct.unpack('>h',u.mem_read(stack+6,2))[0]
    run(0x1703b2,bytes([2,0]))
    selected=struct.unpack('>h',u.mem_read(stack+6,2))[0]
    assert bytes(u.mem_read(0x42000,len(records)))==records
    assert bytes(u.mem_read(0x44000,len(geometry)))==geometry
    cases.append(dict(routine='ai_select_object',a=level,b=remaining,source=objects[1]['position'],player=objects[0]['position'],
        objects=objects,participants=participants,samples=object_samples,consumed=object_consumed,expected=selected))
u.hook_del(object_hook)
print('800 complete original object selectors and per-object assignment scans executed; only Random samples supplied')
# Original command suppression branch, original geometry-distance wrapper.
suppression_result=False
suppression_distance_calls=0
def suppression_stop(cpu,address,size,data):
    global suppression_result
    suppression_result=address==0x31fa2
    cpu.reg_write(UC_M68K_REG_PC,stop)
def suppression_observe(cpu,address,size,data):
    global suppression_distance_calls
    suppression_distance_calls+=1
suppression_hooks=[u.hook_add(UC_HOOK_CODE,suppression_stop,begin=p,end=p) for p in [0x31fa2,0x31fb6]]
suppression_hooks.append(u.hook_add(UC_HOOK_CODE,suppression_observe,begin=0x717c6,end=0x717c6))
u.mem_write(a5+0x4d2,bytes.fromhex('4ef9')+struct.pack('>I',0x717c6));u.ctl_flush_tb()
for iteration in range(1024):
    kind=10 if iteration%4==0 else 7;inhibit=iteration%256
    remaining=rng.choice([-32768,0,5,6,12,32767]);limit=rng.choice([-1,0,499<<16,500<<16,501<<16,2**31-1])
    position=rng.choice([0,499<<16,500<<16,501<<16,1000<<16])
    records=bytearray(12*256);struct.pack_into('>h',records,0x56,1);struct.pack_into('>h',records,256+0x56,2)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(0x44000,bytes(256))
    u.mem_write(0x44080,struct.pack('>i',position));u.mem_write(0x44094,bytes([kind]))
    u.mem_write(a5-0x133d,bytes([1]));u.mem_write(a5-0x1372,struct.pack('>h',remaining));u.mem_write(a5-0x3d42,struct.pack('>i',limit))
    u.mem_write(0x4e00c,bytes([inhibit]));u.reg_write(UC_M68K_REG_A6,0x4e000)
    u.reg_write(UC_M68K_REG_A4,0x44080);u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_D7,2)
    u.reg_write(UC_M68K_REG_A7,0x4d000);suppression_distance_calls=0
    u.emu_start(0x31f3a,stop,count=2000)
    assert u.reg_read(UC_M68K_REG_PC)==stop
    assert suppression_distance_calls==int(kind!=10 and inhibit&1==0 and remaining>5)
    cases.append(dict(routine='ai_commands_suppressed',a=kind,b=inhibit,remaining=remaining,distance=position,
        limit=limit,expected=int(suppression_result)))
for hook in suppression_hooks:u.hook_del(hook)
print('1024 original command-suppression gates executed with original geometry distance and no service fixtures')

# General enemy-owned firing path: all valid resource kinds, target present.
fire_hook=u.hook_add(UC_HOOK_CODE,fire_angle_service,begin=a5+0x4c2,end=a5+0x4c2)
u.ctl_flush_tb()
for iteration in range(1410):
    kind=iteration%47
    source_state=fire_rng.choice([-2**31,-1,0,24<<16,25<<16,26<<16,2**31-1])
    geometry_flag=fire_rng.choice([0,1,128,255]);behavior=fire_rng.randrange(256);movement=fire_rng.choice([0,2,3,255])
    now=fire_rng.choice([0,19,20,21,119,120,121,2**31-1,-2**31,fire_rng.randint(-200,200)])
    timer=fire_rng.choice([-1,0,((now-20+2**31)%2**32)-2**31,((now-120+2**31)%2**32)-2**31,2**31-1,-2**31])
    distance=fire_rng.choice([0,80<<16,(300<<16)-1,300<<16,(300<<16)+1,(600<<16)-1,600<<16,(600<<16)+1,-1,fire_rng.randint(-2**31,2**31-1)])
    interval=fire_rng.choice([0,20,120,-1,-32768,32767])
    short_range=fire_rng.choice([80<<16,distance,distance-1,0])
    arc=fire_rng.choice([-32768,-1,0,1,15,32767]);fire_angle=fire_rng.choice([-32768,-180,-5,-4,0,4,5,180,32767])
    target_state=fire_rng.choice([-1,0,1])
    records=bytearray(12*256)
    struct.pack_into('>h',records,0x56,1)
    records[256+5]=1;records[256+8]=kind;records[256+9]=behavior;records[256+16]=movement
    struct.pack_into('>h',records,256+0x56,2);struct.pack_into('>i',records,256+0x44,timer)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x44000,bytes(256));u.mem_write(0x44028,struct.pack('>i',target_state))
    u.mem_write(0x440a8,struct.pack('>i',source_state));u.mem_write(0x440db,bytes([geometry_flag]))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(0x45000+0x350+kind*2,struct.pack('>h',interval));u.mem_write(0x45000+0x2f2+kind*2,struct.pack('>h',arc))
    u.mem_write(0x45000+0x236+kind*4,struct.pack('>i',short_range));u.mem_write(a5-0x134c,struct.pack('>i',now))
    run(0x171e9a,struct.pack('>iBBBB',distance,1,0,2,0),instruction_limit=2000)
    result=bool(u.mem_read(stack+12,1)[0]);after_timer=struct.unpack('>i',u.mem_read(0x42144,4))[0]
    cases.append(dict(routine='enemy_fire_check',a=kind,b=source_state,flag=geometry_flag,behavior=behavior,movement=movement,
        input=dict(now=now,timer=timer,distance=distance,interval=interval,short_range=short_range,
                   firing_arc=arc,angle=fire_angle,target_field_28=target_state),result=[result,after_timer,u.mem_read(0x440db,1)[0],u.mem_read(0x42109,1)[0],u.mem_read(0x42110,1)[0]],expected=0))
u.hook_del(fire_hook)
print('1410 complete original enemy firing predicates executed across 47 kinds; final angle service supplied')
# Original firing-command expansion; capture weapon constructors, run division.
weapon_requests=[];weapon_kind=7;weapon_now=0
signed_byte=lambda v:v if v<128 else v-256
def weapon_request_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    height=struct.unpack('>h',cpu.mem_read(sp+4,2))[0]
    if address==a5+0x111a:
        assert cpu.mem_read(sp+6,1)[0]==weapon_kind
        weapon=signed_byte(cpu.mem_read(sp+8,1)[0])
        assert struct.unpack('>i',cpu.mem_read(sp+10,4))[0]==weapon_now
        assert struct.unpack('>I',cpu.mem_read(sp+14,4))[0]==0x44000
        assert cpu.mem_read(sp+18,1)[0]==2
        new_sp=sp+20;network=False
    else:
        assert cpu.mem_read(sp+6,1)[0]==2
        weapon=signed_byte(cpu.mem_read(sp+8,1)[0])
        assert struct.unpack('>I',cpu.mem_read(sp+10,4))[0]==0x44000
        new_sp=sp+14;network=True
    weapon_requests.append([weapon,height,network])
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
weapon_hooks=[u.hook_add(UC_HOOK_CODE,weapon_request_service,begin=p,end=p) for p in [a5+0x111a,a5+0x1012]]
u.mem_write(a5+0x128a,bytes.fromhex('4ef9')+struct.pack('>I',0x90038));u.ctl_flush_tb()
for iteration in range(1200):
    command=rng.choice([-32768,-1,0,1,2,32767]);weapon=rng.choice([-32768,-1,0,3,4,5,12,127,128,255,261,268,32767])
    game_mode=rng.choice([0,0,1,4,12,255]);network=rng.choice([0,1,128,255])
    height=rng.choice([-2**31,-65537,-65536,-65535,-1,0,1,65535,65536,2**31-1,rng.randint(-2**31,2**31-1)])
    weapon_kind=rng.randrange(47);weapon_now=rng.randint(-2**31,2**31-1);weapon_requests=[]
    u.mem_write(0x44000,bytes(128));u.mem_write(0x44014,bytes([weapon_kind]));u.mem_write(0x44028,struct.pack('>i',height))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000+0x640+weapon_kind*2,struct.pack('>h',weapon))
    u.mem_write(a5-0x12ca,bytes([game_mode]));u.mem_write(a5-0x4f4f,bytes([network]));u.mem_write(a5-0x134c,struct.pack('>i',weapon_now))
    u.mem_write(0x4dffe,struct.pack('>h',command));u.reg_write(UC_M68K_REG_A6,0x4e000)
    u.reg_write(UC_M68K_REG_A4,0x44000);u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_D7,2);u.reg_write(UC_M68K_REG_A7,0x4d000)
    u.emu_start(0x31fd0,0x3212a,count=5000)
    assert u.reg_read(UC_M68K_REG_PC)==0x3212a
    cases.append(dict(routine='enemy_weapon_requests',a=command,b=weapon,game_mode=game_mode,network=network,
        height=height,result=weapon_requests,expected=0))
for hook in weapon_hooks:u.hook_del(hook)
print('1200 original firing-command expansions executed; constructor calls captured, original height division executed')
# Original weapon heading preparation. Random, target selection and angle supplied.
heading_sample=0;heading_angle=None;heading_random_calls=0;heading_target_calls=0
heading_angle_calls=0
def heading_service(cpu,address,size,data):
    global heading_random_calls,heading_target_calls,heading_angle_calls
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address in [0x802f6,0x8033c]:
        cpu.mem_write(sp,struct.pack('>h',heading_sample));heading_random_calls+=1
        cpu.reg_write(UC_M68K_REG_PC,address+2);return
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0xd5a:
        assert cpu.mem_read(sp+4,1)[0]==0 and cpu.mem_read(sp+6,1)[0]==0 and cpu.mem_read(sp+8,1)[0]==2
        heading_target_calls+=1;cpu.mem_write(sp+10,bytes([0 if heading_angle is None else 1]));new_sp=sp+10
    else:
        heading_angle_calls+=1
        cpu.mem_write(sp+12,struct.pack('>h',heading_angle));new_sp=sp+12
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
heading_hooks=[u.hook_add(UC_HOOK_CODE,heading_service,begin=p,end=p) for p in [0x802f6,0x8033c,a5+0xd5a,a5+0x4c2]]
u.mem_write(a5+0x1282,bytes.fromhex('4ef9')+struct.pack('>I',0x90004));u.ctl_flush_tb()
for iteration in range(1000):
    weapon=iteration%18
    heading=rng.choice([-2**31,-1,0,1,359<<16,360<<16,2**31-1,rng.randrange(360)<<16])
    heading_sample=rng.choice([-32768,-32767,-8,-1,0,1,7,8,32767,rng.randint(-32768,32767)])
    heading_angle=rng.choice([None,-32768,-180,-1,0,1,180,32767])
    heading_random_calls=0;heading_target_calls=0;heading_angle_calls=0
    u.mem_write(0x44000,bytes(128));u.mem_write(0x44008,struct.pack('>i',heading))
    u.mem_write(a5-0x1346,struct.pack('>I',0x45000));u.mem_write(0x45000,bytes(128))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(0x42056,struct.pack('>h',1))
    u.reg_write(UC_M68K_REG_A4,0x44000);u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x4e000)
    u.reg_write(UC_M68K_REG_A7,0x4d000);u.reg_write(UC_M68K_REG_D5,2);u.reg_write(UC_M68K_REG_D6,weapon)
    u.emu_start(0x802ee,0x803ee,count=50000)
    assert u.reg_read(UC_M68K_REG_PC)==0x803ee
    assert heading_angle_calls==int(weapon==8 and heading_angle is not None)
    final_heading=struct.unpack('>i',u.mem_read(0x44008,4))[0]
    final_weapon=signed_byte(u.reg_read(UC_M68K_REG_D6)&255)
    cases.append(dict(routine='weapon_initial_heading',a=weapon,b=heading,sample=heading_sample,angle=heading_angle,
        random_calls=heading_random_calls,target_calls=heading_target_calls,result=[final_weapon,final_heading],expected=0))
for hook in heading_hooks:u.hook_del(hook)
print('1000 original weapon-heading preparation blocks executed; Random and targeting services supplied')
# Weapon-specific record construction; original lifetime getter and arithmetic.
record_sample=0;record_random_calls=0
def record_random_service(cpu,address,size,data):
    global record_random_calls
    cpu.mem_write(cpu.reg_read(UC_M68K_REG_A7),struct.pack('>h',record_sample));record_random_calls+=1
    cpu.reg_write(UC_M68K_REG_PC,address+2)
record_hook=u.hook_add(UC_HOOK_CODE,record_random_service,begin=0x805aa,end=0x805aa)
for displacement,target in [(0x75a,0xe3c5a),(0xde2,0xf0004),(0x1282,0x90004)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.mem_write(a5-0x7b99,bytes([0]));u.ctl_flush_tb()
for iteration in range(1080):
    weapon=iteration%18;owner=rng.choice([0,1,2,12]);source_kind=rng.randrange(47)
    now=rng.randint(-2**31,2**31-1);lifetime=rng.choice([-32768,-1,0,1,75,32767])
    direction=[rng.randint(-65536,65536) for _ in range(2)];owner_field=rng.randint(-32768,32767)
    before=rng.randbytes(128);record_sample=rng.choice([-32768,-1,0,1,32767,rng.randint(-32768,32767)]);record_random_calls=0
    u.mem_write(0x44000,before);u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x42000+(owner-1)*256+0x5c,struct.pack('>h',owner_field))
    u.mem_write(a5-0x3dae,struct.pack('>h',lifetime));u.mem_write(0x4dfea,struct.pack('>ii',*direction))
    u.mem_write(0x4e00a,bytes([source_kind]));u.reg_write(UC_M68K_REG_A6,0x4e000)
    u.reg_write(UC_M68K_REG_A4,0x44000);u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,0x4d000)
    u.reg_write(UC_M68K_REG_D4,now&0xffffffff);u.reg_write(UC_M68K_REG_D5,owner);u.reg_write(UC_M68K_REG_D6,weapon)
    u.reg_write(UC_M68K_REG_D7,3 if owner==0 else 1)
    u.emu_start(0x80408,0x807e8,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x807e8
    after=bytes(u.mem_read(0x44000,128))
    cases.append(dict(routine='initialize_weapon_record',a=0,b=0,before=list(before),after=list(after),sample=record_sample,calls=record_random_calls,
        input=dict(weapon=weapon,now=now,lifetime=lifetime,owner=owner,source_kind=source_kind,owner_field_5c=owner_field,direction=direction),expected=0))
u.hook_del(record_hook)
print('1080 original weapon-specific record constructors executed; only Random samples supplied')
# Complete original geometry allocation counter, including capacity mismatch.
slot_inputs=list(range(-3,306))+[-32768,32766,32767]+[rng.randint(-32768,32767) for _ in range(300)]
for counter in slot_inputs:
    u.mem_write(a5-0x1348,struct.pack('>h',counter))
    run(0xb0e0e,b'')
    result=struct.unpack('>h',u.mem_read(stack+4,2))[0]
    after=struct.unpack('>h',u.mem_read(a5-0x1348,2))[0]
    cases.append(dict(routine='original_geometry_slot',a=counter,b=after,expected=result))
print(f'{len(slot_inputs)} complete original geometry-slot allocations executed without service fixtures')
# Original common weapon finalization; model-loading service captured.
finalize_kind=0;finalize_calls=0
def finalize_model_service(cpu,address,size,data):
    global finalize_calls
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    assert cpu.mem_read(sp+4,1)[0]==finalize_kind
    finalize_calls+=1;cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,ret)
finalize_hook=u.hook_add(UC_HOOK_CODE,finalize_model_service,begin=a5+0xd2a,end=a5+0xd2a)
u.ctl_flush_tb()
for iteration in range(768):
    weapon=signed_byte(iteration%256);owner=rng.choice([0,1,2,12]);now=rng.randint(-2**31,2**31-1)
    count=rng.choice([-32768,-1,0,1,32767]);before=rng.randbytes(128);finalize_kind=before[0x14];finalize_calls=0
    records=rng.randbytes(12*256);u.mem_write(0x42000,records);u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    counter_addr=0x42000+(owner-1)*256+0x58;u.mem_write(counter_addr,struct.pack('>h',count))
    u.mem_write(0x44000,before);u.reg_write(UC_M68K_REG_A4,0x44000);u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A7,0x4d000);u.reg_write(UC_M68K_REG_D4,now&0xffffffff)
    u.reg_write(UC_M68K_REG_D5,owner);u.reg_write(UC_M68K_REG_D6,weapon&255)
    u.emu_start(0x807e8,0x80836,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x80836 and finalize_calls==1
    after=bytes(u.mem_read(0x44000,128));after_count=struct.unpack('>h',u.mem_read(counter_addr,2))[0]
    cases.append(dict(routine='finalize_weapon_record',a=weapon,b=now,owner=owner,count=count,after_count=after_count,
        before=list(before),after=list(after),expected=0))
u.hook_del(finalize_hook)
print('768 original common weapon finalization blocks executed; model-load service captured')
# Constructor admission and ammo effects; allocator and sound services captured.
admission_slot=1;admission_allocations=0;admission_sounds=[];admission_success=False
def admission_service(cpu,address,size,data):
    global admission_allocations,admission_success
    if address in [0x80292,0x808a4]:
        admission_success=address==0x80292;cpu.reg_write(UC_M68K_REG_PC,stop);return
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0xe5a:
        admission_allocations+=1;cpu.mem_write(sp+4,struct.pack('>h',admission_slot));new_sp=sp+4
    else:
        priority,request=struct.unpack('>hi',cpu.mem_read(sp+4,6));admission_sounds.append([priority,request]);new_sp=sp+10
    cpu.reg_write(UC_M68K_REG_A7,new_sp);cpu.reg_write(UC_M68K_REG_PC,ret)
admission_hooks=[u.hook_add(UC_HOOK_CODE,admission_service,begin=p,end=p) for p in [0x80292,0x808a4,a5+0xe5a,a5+0x972]]
u.ctl_flush_tb()
for iteration in range(1600):
    owner=rng.choice([0,1,2,12]);local=rng.choice([1,2,12]);weapon=rng.randrange(18)
    event=dict(owner=owner,local=local,team=rng.choice([0,1,2,3,255]),enemy=rng.choice([0,0,1,255]),
        active_shots=rng.choice([-32768,-1,0,1,10,32767]),shot_limit=rng.choice([-32768,0,1,10,32767]),
        ammo=rng.choice([-32768,-1,0,1,5,6,40,32767]),enemy_cost=rng.choice([-32768,-1,0,1,5,32767]),
        player_cost=rng.choice([-32768,-1,0,1,5,32767]),free_ammo=rng.choice([0,1,255]))
    admission_slot=rng.choice([-32768,-1,0,1,302]);admission_allocations=0;admission_sounds=[];admission_success=False
    addr=0x42000+(owner-1)*256
    u.mem_write(addr,bytes(256));u.mem_write(addr+4,bytes([event['team'],event['enemy']]))
    u.mem_write(addr+0x58,struct.pack('>h',event['active_shots']));u.mem_write(addr+0x5e,struct.pack('>h',event['ammo']))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1350,struct.pack('>I',0x45000))
    u.mem_write(0x45000+0x7a2+weapon*2,struct.pack('>h',event['enemy_cost']))
    u.mem_write(0x45000+0x7c6+weapon*2,struct.pack('>h',event['player_cost']))
    u.mem_write(a5-0x133c,struct.pack('>h',event['shot_limit']));u.mem_write(a5-0x133d,bytes([local]))
    u.mem_write(a5-0x12c5,bytes([event['free_ammo']]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x4e000);u.reg_write(UC_M68K_REG_A7,0x4d000)
    u.reg_write(UC_M68K_REG_D5,owner);u.reg_write(UC_M68K_REG_D6,weapon)
    u.emu_start(0x80140,stop,count=2000)
    assert u.reg_read(UC_M68K_REG_PC)==stop and admission_allocations in [0,1]
    ammo=struct.unpack('>h',u.mem_read(addr+0x5e,2))[0]
    cases.append(dict(routine='admit_weapon',a=admission_slot,b=0,input=event,
        result=dict(ammo=ammo,allocation_called=bool(admission_allocations),slot=admission_slot if admission_success else None,sounds=admission_sounds),expected=0))
for hook in admission_hooks:u.hook_del(hook)
print('1600 original constructor admission blocks executed; allocation results supplied and sound calls captured')
# Original geometry compaction tail, with original cell lookup and no fixtures.
u.mem_write(a5+0x77a,bytes.fromhex('4ef9')+struct.pack('>I',0xe3eee));u.ctl_flush_tb()
for iteration in range(768):
    count=rng.randrange(1,25);removed=count if iteration%3==0 else rng.randrange(1,count+1)
    before=[list(rng.randbytes(128)) for _ in range(count)]
    before[-1][0x14]=iteration%256;before[-1][0x1a]=rng.choice([0,1,2,12])
    position=[rng.randint(-2**31,2**31-1) for _ in range(2)]
    packed=bytearray(before[-1]);struct.pack_into('>ii',packed,0,*position);before[-1]=list(packed)
    cell=max(0,min(15,position[0]>>22))*16+max(0,min(15,position[1]>>22))
    grid=[[] for _ in range(256)];grid[cell]=[rng.choice([count,removed,0,-1,rng.randrange(1,30)]) for _ in range(rng.randrange(21))]
    ids=[rng.randint(-32768,32767) for _ in range(12)];participants=bytearray(rng.randbytes(12*256))
    for i,id in enumerate(ids):struct.pack_into('>h',participants,i*256+0x56,id)
    u.mem_write(0x42000,bytes(participants));u.mem_write(0x44000,bytes(sum(before,[])))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1348,struct.pack('>h',count));u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    grid_bytes=bytearray(256*42)
    for i,entries in enumerate(grid):
        struct.pack_into('>h',grid_bytes,i*42,len(entries))
        for j,id in enumerate(entries):struct.pack_into('>h',grid_bytes,i*42+2+j*2,id)
    u.mem_write(0x48000,bytes(grid_bytes));u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000)
    u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D6,removed)
    u.emu_start(0xb0f4a,0xb104a,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0xb104a
    assert struct.unpack('>h',u.mem_read(a5-0x1348,2))[0]==count-1
    after=[list(u.mem_read(0x44000+i*128,128)) for i in range(count)]
    after_ids=[struct.unpack('>h',u.mem_read(0x42000+i*256+0x56,2))[0] for i in range(12)]
    after_grid=[list(struct.unpack('>'+str(len(entries))+'h',u.mem_read(0x48000+i*42+2,len(entries)*2))) if entries else [] for i,entries in enumerate(grid)]
    cases.append(dict(routine='compact_geometry_tail',a=count,b=removed,before=before,after=after,
        ids=ids,after_ids=after_ids,grid=grid,after_grid=after_grid,expected=0))
print('768 original geometry compaction tails executed with original cell lookup and no service fixtures')
# Complete geometry removal including original CODE 14 participant status write.
u.mem_write(a5+0x90a,bytes.fromhex('4ef9')+struct.pack('>I',0x112e20))
u.ctl_flush_tb()
for iteration in range(1024):
    count=rng.randrange(1,25);removed=count if iteration%3==0 else rng.randrange(1,count+1)
    before=[list(rng.randbytes(128)) for _ in range(count)]
    participant_kinds=[6,7,8,10,13,20,21,22,23,24,25,26,27,31,35,45]
    for index,kind in [(count-1,rng.randrange(256)),(removed-1,iteration%256)]:
        before[index][0x14]=kind
        before[index][0x1a]=rng.choice([1,2,12] if kind in participant_kinds else [0,1,2,12])
    grid=[[] for _ in range(256)]
    for index in set([count-1,removed-1]):
        position=[rng.randint(-2**31,2**31-1) for _ in range(2)]
        packed=bytearray(before[index]);struct.pack_into('>ii',packed,0,*position);before[index]=list(packed)
        cell=max(0,min(15,position[0]>>22))*16+max(0,min(15,position[1]>>22))
        grid[cell]=[rng.choice([count,removed,0,-1,rng.randrange(1,30)]) for _ in range(rng.randrange(21))]
    fields=dict(ids=[rng.randint(-32768,32767) for _ in range(12)],
        shots=[rng.choice([-32768,-1,0,1,5,32767]) for _ in range(12)],active=[rng.randrange(256) for _ in range(12)],status=list(rng.randbytes(12)))
    participants=bytearray(rng.randbytes(12*256))
    for i in range(12):
        struct.pack_into('>hh',participants,i*256+0x56,fields['ids'][i],fields['shots'][i])
        participants[i*256+0x88]=fields['active'][i]
    u.mem_write(0x42000,bytes(participants));u.mem_write(0x44000,bytes(sum(before,[])))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1348,struct.pack('>h',count));u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    grid_bytes=bytearray(256*42)
    for i,entries in enumerate(grid):
        struct.pack_into('>h',grid_bytes,i*42,len(entries))
        for j,id in enumerate(entries):struct.pack_into('>h',grid_bytes,i*42+2+j*2,id)
    u.mem_write(0x48000,bytes(grid_bytes));u.mem_write(a5-0x4c40,bytes(fields['status']))
    run(0xb0e62,struct.pack('>h',removed))
    assert struct.unpack('>h',u.mem_read(a5-0x1348,2))[0]==count-1
    after=[list(u.mem_read(0x44000+i*128,128)) for i in range(count)]
    after_fields=dict(ids=[],shots=[],active=[],status=list(u.mem_read(a5-0x4c40,12)))
    for i in range(12):
        actual=bytes(u.mem_read(0x42000+i*256,256))
        after_fields['ids'].append(struct.unpack_from('>h',actual,0x56)[0])
        after_fields['shots'].append(struct.unpack_from('>h',actual,0x58)[0])
        after_fields['active'].append(actual[0x88])
        expected=bytearray(participants[i*256:(i+1)*256]);expected[0x56:0x5a]=actual[0x56:0x5a];expected[0x88]=actual[0x88]
        assert actual==bytes(expected)
    after_grid=[]
    for i in range(256):
        length=struct.unpack('>h',u.mem_read(0x48000+i*42,2))[0]
        after_grid.append(list(struct.unpack('>'+str(length)+'h',u.mem_read(0x48000+i*42+2,length*2))) if length else [])
    cases.append(dict(routine='remove_geometry',a=count,b=removed,before=before,after=after,
        participants=fields,after_participants=after_fields,grid=grid,after_grid=after_grid,
        expected=0))
print('1024 complete original geometry removals executed including participant status service; no service fixtures')
# Shot-limit selector and the fresh session settings block that supplies its flag.
for flag in range(256):
    u.mem_write(a5-0x12c4,bytes([flag]));u.reg_write(UC_M68K_REG_A5,a5)
    u.emu_start(0xb2902,0xb2916,count=100)
    assert u.reg_read(UC_M68K_REG_PC)==0xb2916
    cases.append(dict(routine='concurrent_shot_limit',a=flag,b=0,
        expected=struct.unpack('>h',u.mem_read(a5-0x133c,2))[0]))
u.mem_map(0x180000,0x10000);u.mem_write(0x180000,(source/'24.bin').read_bytes())
for single in [0,1,255]:
    settings=bytearray(rng.randbytes(20));u.mem_write(0x44000,bytes(settings))
    u.reg_write(UC_M68K_REG_A6,0x57000);u.mem_write(0x57008,bytes([single,0]))
    u.mem_write(0x5700c,struct.pack('>I',0x44000));u.reg_write(UC_M68K_REG_A5,a5)
    u.emu_start(0x18140c,0x181448,count=100)
    assert u.reg_read(UC_M68K_REG_PC)==0x181448
    assert u.mem_read(0x44000,1)==bytes([0 if single else 1])
    assert u.mem_read(0x44008,2)==bytes([0,1])
    # Copy the settings flag using the original settings transfer block.
    u.reg_write(UC_M68K_REG_A4,0x44000)
    u.emu_start(0xb25fc,0xb260e,count=100)
    assert u.mem_read(a5-0x12c5,2)==bytes([0,1])
print('256 original concurrent-shot selectors and 3 fresh-settings/flag-transfer blocks executed without fixtures')
# Original weapon sound initialization and constructor request submission.
u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000)
u.emu_start(0x160bcc,0x160c28,count=1000)
assert u.reg_read(UC_M68K_REG_PC)==0x160c28
weapon_sound_calls=[]
def weapon_sound_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    priority,request=struct.unpack('>hI',cpu.mem_read(sp+4,6))
    assert priority==1
    weapon_sound_calls.append(request)
    cpu.reg_write(UC_M68K_REG_A7,sp+10);cpu.reg_write(UC_M68K_REG_PC,ret)
weapon_sound_hook=u.hook_add(UC_HOOK_CODE,weapon_sound_service,begin=a5+0x972,end=a5+0x972)
u.ctl_flush_tb()
for weapon in range(18):
    weapon_sound_calls=[]
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.mem_write(0x57000-0x2f,bytes([weapon]))
    u.emu_start(0x80836,0x80864,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x80864 and len(weapon_sound_calls)<=1
    cases.append(dict(routine='weapon_fire_sound',a=weapon,b=0,request=weapon_sound_calls[0] if weapon_sound_calls else None,expected=0))
u.hook_del(weapon_sound_hook)
print('Original weapon sound table initialized; all 18 constructor sound routes captured with priority checks')
# Type-29 motion body: supplied direction, original integer multiplication/division.
for iteration in range(1200):
    before=bytearray(rng.randbytes(128))
    values=[-2**31,-65536,-1,0,1,65536,30<<16,2**31-1]
    for offset in [0,4,0x24,0x28]:
        struct.pack_into('>i',before,offset,rng.choice(values) if iteration<800 else rng.randint(-2**31,2**31-1))
    direction=[rng.choice(values) for _ in range(2)]
    elapsed=rng.choice([-2**31,-65536,-2,-1,1,2,3,16,65536,2**31-1])
    u.mem_write(0x44000,bytes(before));u.reg_write(UC_M68K_REG_A4,0x44000)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.reg_write(UC_M68K_REG_D7,elapsed&0xffffffff)
    u.mem_write(0x57000-4,struct.pack('>i',direction[0]));u.mem_write(0x57000-8,struct.pack('>i',direction[1]))
    u.emu_start(0x81de0,0x825e8,count=2000)
    assert u.reg_read(UC_M68K_REG_PC)==0x825e8
    cases.append(dict(routine='move_bouncing_projectile',a=elapsed,b=0,before=list(before),
        direction=direction,after=list(u.mem_read(0x44000,128)),expected=0))
print('1200 original type-29 motion bodies executed with original integer arithmetic and supplied directions')
# Complete type-29 movement including original direction lookup and grid update.
u.mem_write(a5+0xbb2,bytes.fromhex('4ef9')+struct.pack('>I',0x20cd0));u.ctl_flush_tb()
bounce_table=[rng.randint(-8<<16,8<<16) for _ in range(360)]
u.mem_write(0x46000,struct.pack('>360i',*bounce_table));u.mem_write(a5-0x38ba,struct.pack('>I',0x46000))
for iteration in range(720):
    before=bytearray(rng.randbytes(128));before[0x14]=29;before[0x1a]=rng.choice([0,1,12])
    position=[rng.randint(-2**31,2**31-1) if iteration%4==0 else rng.randrange(1024<<16) for _ in range(2)]
    heading=((iteration%360)<<16)|rng.randrange(65536)
    struct.pack_into('>iii',before,0,*position,heading)
    struct.pack_into('>ii',before,0x24,rng.randint(-40<<16,40<<16),rng.randint(-2<<16,100<<16))
    elapsed=rng.choice([0,1,2,3,16,-1,-16,2**31-1]);object_id=rng.randrange(1,13)
    grid=[[] for _ in range(256)]
    for i in range(256):grid[i]=[rng.choice([object_id,1,12]) for _ in range(rng.randrange(4))]
    packed=b''.join(struct.pack('>21h',len(cell),*cell,*([0]*(20-len(cell)))) for cell in grid)
    u.mem_write(0x48000,packed);u.mem_write(a5-0x136c,struct.pack('>I',0x48000))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));record_address=0x42000+(object_id-1)*128
    u.mem_write(record_address,bytes(before))
    run(0x81d54,struct.pack('>iBBh',elapsed,0,0,object_id))
    after_grid=[]
    for cell in range(256):
        words=struct.unpack('>21h',u.mem_read(0x48000+42*cell,42));after_grid.append(list(words[1:1+words[0]]))
    cases.append(dict(routine='step_bouncing_projectile',a=elapsed,b=object_id,before=list(before),table=bounce_table,
        grid=grid,after=list(u.mem_read(record_address,128)),after_grid=after_grid,expected=0))
print('720 complete original type-29 movement calls executed with original lookup and grid routines; synthetic direction table')
# Complete single-record expiry scan, with movement/deletion/area services captured.
lifecycle_calls=[]
def lifecycle_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==0x82700:
        name='Move';size=6
    elif address==0x82aa6:
        name='Area';size=2
    else:
        name='Delete';size=2
    lifecycle_calls.append(name)
    cpu.reg_write(UC_M68K_REG_A7,sp+4+size);cpu.reg_write(UC_M68K_REG_PC,ret)
lifecycle_hooks=[u.hook_add(UC_HOOK_CODE,lifecycle_service,begin=p,end=p) for p in [0x82700,0x82aa6,a5+0xe62]]
u.ctl_flush_tb()
lifecycle_edges=[(kind,weapon,delta,owner) for kind in [9,10,28,29,32,33,34,43,44,46]
    for weapon in [-1,0,1,6,17] for delta in [-1,0,1,56,57,58,60] for owner in [0,1,12]]
for iteration in range(2048+len(lifecycle_edges)):
    before=bytearray(rng.randbytes(128));kind=iteration%256
    before[0x14]=kind;before[0x1a]=rng.choice([0,0,1,2,12])
    weapon=rng.choice([-32768,-1,0,1,6,17,18,32767]);struct.pack_into('>h',before,0x1c,weapon)
    now=rng.choice([-2**31,-100,0,100,2**31-1])
    deadline=((now+rng.choice([-1,0,1,55,56,57,58,59,60,61])+2**31)%2**32)-2**31
    if iteration>=2048:
        kind,weapon,delta,owner=lifecycle_edges[iteration-2048]
        now=100;deadline=now+delta;before[0x14]=kind;before[0x1a]=owner
        struct.pack_into('>h',before,0x1c,weapon)
    struct.pack_into('>i',before,0x16,deadline)
    counts=[rng.choice([-32768,-1,0,1,32767]) for _ in range(12)]
    u.mem_write(0x44000,bytes(before));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    for i,count in enumerate(counts):u.mem_write(0x42000+i*256+0x58,struct.pack('>h',count))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x12c8,struct.pack('>h',0));u.mem_write(a5-0x1348,struct.pack('>h',1))
    lifecycle_calls=[];run(0x82bbe,struct.pack('>i',now))
    after=bytes(u.mem_read(0x44000,128))
    if lifecycle_calls==['Area','Delete']:action='AreaEffectThenDelete'
    elif lifecycle_calls==['Move']:action='Move'
    elif lifecycle_calls==['Delete']:action='Delete'
    else:
        assert lifecycle_calls==[]
        action='Retire' if kind!=10 and after[0x14]==10 else 'None'
    after_counts=[struct.unpack('>h',u.mem_read(0x42000+i*256+0x58,2))[0] for i in range(12)]
    cases.append(dict(routine='projectile_lifecycle',a=now,b=0,before=list(before),after=list(after),counts=counts,after_counts=after_counts,action=action,expected=0))
for hook in lifecycle_hooks:u.hook_del(hook)
print('3098 complete single-record expiry scans executed; movement, deletion, and area-effect calls captured')
# Complete area-effect selector; original geometry distance/division, captured damage calls.
area_calls=[]
def area_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    damage,mode,now,attacker,victim=struct.unpack('>hhibxbx',cpu.mem_read(sp+4,12))
    area_calls.append(dict(damage=damage,mode=mode,now=now,attacker=attacker,victim=victim))
    cpu.reg_write(UC_M68K_REG_A7,sp+16);cpu.reg_write(UC_M68K_REG_PC,ret)
area_hook=u.hook_add(UC_HOOK_CODE,area_service,begin=a5+0x119a,end=a5+0x119a);u.ctl_flush_tb()
for iteration in range(1200):
    records=[bytearray(rng.randbytes(128)) for _ in range(13)]
    participants=[bytearray(rng.randbytes(256)) for _ in range(12)]
    records[0][0x1a]=rng.choice([0,1,2,12,255]);struct.pack_into('>ii',records[0],0,0,0)
    struct.pack_into('>i',records[0],0x28,rng.choice([-2**31,-65536,0,65536,2**31-1]))
    for i in range(12):
        participant=participants[i];participant[0x88]=rng.choice([0,1,2,3,128,255]);participant[5]=rng.choice([0,1,2,3,128,255])
        struct.pack_into('>h',participant,0x56,i+2)
        record=records[i+1];record[0x14]=rng.choice([6,7,10,31,45]);record[0x1d]=rng.randrange(18)
        x=rng.choice([0,(60<<16)-1,60<<16,(120<<16)-1,120<<16,(240<<16)-1,240<<16,-2**31,2**31-1])
        z=0 if iteration%2==0 else rng.randint(-300<<16,300<<16)
        struct.pack_into('>ii',record,0,x,z);struct.pack_into('>i',record,0x28,rng.choice([-2**31,-65536,0,65536,2**31-1]))
    damage=[rng.choice([-32768,-20000,-1,0,1,3,20000,32767]) for _ in range(18)]
    now=rng.randint(-2**31,2**31-1)
    raw_records=b''.join(bytes(r) for r in records);raw_participants=b''.join(bytes(p) for p in participants)
    u.mem_write(0x44000,raw_records);u.mem_write(0x42000,raw_participants)
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x45000));u.mem_write(0x45000+0x75a,struct.pack('>18h',*damage))
    u.mem_write(a5-0x134c,struct.pack('>i',now));area_calls=[]
    run(0x82aa6,struct.pack('>h',1))
    assert bytes(u.mem_read(0x44000,len(raw_records)))==raw_records
    assert bytes(u.mem_read(0x42000,len(raw_participants)))==raw_participants
    cases.append(dict(routine='projectile_area_requests',a=now,b=1,records=[list(r) for r in records],
        participants=[list(p) for p in participants],damage=damage,requests=area_calls,expected=0))
u.hook_del(area_hook)
print('1200 complete original area-effect selectors executed; original distance/division and captured damage requests')
# Damage response fields after score handling; multiplayer notification captured.
response_notifications=[]
def response_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    attacker,victim=struct.unpack('>bxbx',cpu.mem_read(sp+4,4))
    response_notifications.append([attacker,victim])
    assert cpu.mem_read(0x44014,1)==bytes([response_kind])
    cpu.reg_write(UC_M68K_REG_A7,sp+8);cpu.reg_write(UC_M68K_REG_PC,ret)
response_hook=u.hook_add(UC_HOOK_CODE,response_service,begin=a5+0x10b2,end=a5+0x10b2);u.ctl_flush_tb()
for iteration in range(1024):
    record=bytearray(rng.randbytes(128));participant=bytearray(rng.randbytes(256))
    response_kind=iteration%256;record[0x14]=response_kind
    participant[5]=rng.choice([0,0,1,2,255]);struct.pack_into('>h',participant,0x56,1)
    event=dict(now=rng.choice([-2**31,-1,0,1,2**31-1]),local=rng.choice([1,2,12]),
        victim=rng.choice([1,2,12]),attacker=rng.choice([-128,-1,0,1,2,12,127]),
        killed=rng.choice([0,0,1,2,255]),game_mode=rng.choice([0,0,1,4,255]))
    u.mem_write(0x44000,bytes(record));u.mem_write(0x42000,bytes(participant))
    u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x1342,struct.pack('>I',0x42000-(event['victim']-1)*256))
    u.mem_write(a5-0x134c,struct.pack('>i',event['now']));u.mem_write(a5-0x133d,bytes([event['local']]))
    u.mem_write(a5-0x12ca,bytes([event['game_mode']]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.reg_write(UC_M68K_REG_D5,event['attacker']&255);u.reg_write(UC_M68K_REG_D6,event['killed']);u.reg_write(UC_M68K_REG_D7,event['victim'])
    response_notifications=[];u.emu_start(0x30de2,0x30e7c,count=2000)
    assert u.reg_read(UC_M68K_REG_PC)==0x30e7c and len(response_notifications)<=1
    cases.append(dict(routine='damage_response_fields',a=0,b=0,input=event,record=list(record),participant=list(participant),
        after_record=list(u.mem_read(0x44000,128)),after_participant=list(u.mem_read(0x42000,256)),
        notification=response_notifications[0] if response_notifications else None,expected=0))
u.hook_del(response_hook)
print('1024 original damage-response blocks executed; multiplayer notification captured before kind transition')
# Complete death-burst gate and radial request producer, with downstream calls captured.
for iteration in range(1024):
    enemy=iteration%256;kind=rng.choice([0,7,8,255]);field=rng.choice([-32768,19,20,21,27,28,32767])
    participant=bytearray(rng.randbytes(256));participant[5]=enemy;participant[8]=kind
    u.mem_write(0x42000,bytes(participant));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x136e,struct.pack('>h',field))
    run(0x30a22,struct.pack('>bx',1))
    result=u.mem_read(stack+6,1)[0]
    assert bytes(u.mem_read(0x42000,256))==bytes(participant)
    cases.append(dict(routine='death_burst_gate',a=enemy,b=kind,field=field,expected=result))
burst_requests=[];burst_random_calls=0
def burst_service(cpu,address,size,data):
    global burst_random_calls
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address==0x30922:
        burst_random_calls+=1;cpu.mem_write(sp,struct.pack('>h',burst_sample));cpu.reg_write(UC_M68K_REG_PC,address+2);return
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x111a:
        height,source_kind,weapon,now,pose,owner=struct.unpack('>hBxBxiIbx',cpu.mem_read(sp+4,16))
        assert source_kind==7 and weapon==0 and now==burst_event['now']
        argsize=16;network=False
    else:
        height,owner,weapon,pose=struct.unpack('>hbxBxI',cpu.mem_read(sp+4,10))
        assert weapon==0
        source_kind=None
        argsize=10;network=True
    burst_requests.append(dict(now=burst_event['now'] if not network else None,weapon=weapon,source_kind=source_kind,network=network,pose=list(struct.unpack('>iii',cpu.mem_read(pose,12))),height=height,owner=owner))
    cpu.reg_write(UC_M68K_REG_A7,sp+4+argsize);cpu.reg_write(UC_M68K_REG_PC,ret)
burst_hooks=[u.hook_add(UC_HOOK_CODE,burst_service,begin=p,end=p) for p in [0x30922,a5+0x111a,a5+0x1012]]
u.ctl_flush_tb()
for iteration in range(1600):
    record=bytearray(rng.randbytes(128));record[0x1a]=rng.choice([0,1,2,12,255])
    struct.pack_into('>i',record,0x28,rng.choice([-2**31,-65537,-65536,-1,0,1,65536,2**31-1]))
    burst_event=dict(owner=rng.choice([0,1,2,12]),local=rng.choice([1,2,12]),field_136e=rng.choice([-32768,19,20,21,27,28,32767]),
        game_mode=rng.choice([0,0,1,4,255]),network_authority=rng.choice([0,1,2,3,128,255]),owner_enemy=rng.choice([0,1,2,3,128,255]),now=rng.randint(-2**31,2**31-1))
    burst_sample=rng.choice([-32768,-32767,-361,-360,-1,0,1,359,360,32767])
    u.mem_write(0x44000,bytes(record));u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x42000+(burst_event['owner']-1)*256+5,bytes([burst_event['owner_enemy']]))
    u.mem_write(a5-0x136e,struct.pack('>h',burst_event['field_136e']));u.mem_write(a5-0x133d,bytes([burst_event['local']]))
    u.mem_write(a5-0x12ca,bytes([burst_event['game_mode']]));u.mem_write(a5-0x4f4f,bytes([burst_event['network_authority']]))
    u.mem_write(a5-0x134c,struct.pack('>i',burst_event['now']))
    burst_requests=[];burst_random_calls=0
    run(0x308c6,struct.pack('>bxh',burst_event['owner'],1))
    assert bytes(u.mem_read(0x44000,128))==bytes(record)
    cases.append(dict(routine='radial_burst_requests',a=0,b=0,record=list(record),input=burst_event,sample=burst_sample,
        random_calls=burst_random_calls,requests=burst_requests,expected=0))
for hook in burst_hooks:u.hook_del(hook)
print('1024 original death-burst gates and 1600 complete radial request routines executed; Random supplied and constructors captured')
# Complete death fragment request producer with original angle wrapping/arithmetic.
fragment_requests=[];fragment_random_calls=0
def fragment_service(cpu,address,size,data):
    global fragment_random_calls
    sp=cpu.reg_read(UC_M68K_REG_A7)
    if address in [0x307a2,0x307b8]:
        cpu.mem_write(sp,struct.pack('>h',fragment_samples[fragment_random_calls]));fragment_random_calls+=1
        cpu.reg_write(UC_M68K_REG_PC,address+2);return
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x111a:
        height,source_kind,weapon,now,pose,owner=struct.unpack('>hBxBxiIbx',cpu.mem_read(sp+4,16))
        assert weapon==6 and source_kind==record[0x15] and now==fragment_event['now']
        argsize=16;network=False
    else:
        height,owner,weapon,pose=struct.unpack('>hbxBxI',cpu.mem_read(sp+4,10))
        assert weapon==6
        source_kind=None;argsize=10;network=True
    fragment_requests.append(dict(now=fragment_event['now'] if not network else None,weapon=weapon,source_kind=source_kind,network=network,
        pose=list(struct.unpack('>iii',cpu.mem_read(pose,12))),height=height,owner=owner))
    cpu.reg_write(UC_M68K_REG_A7,sp+4+argsize);cpu.reg_write(UC_M68K_REG_PC,ret)
fragment_hooks=[u.hook_add(UC_HOOK_CODE,fragment_service,begin=p,end=p) for p in [0x307a2,0x307b8,a5+0x111a,a5+0x1012]]
u.ctl_flush_tb()
for iteration in range(1600):
    record=bytearray(rng.randbytes(128));record[0x15]=iteration%256
    struct.pack_into('>i',record,0x28,rng.choice([-2**31,-65537,-65536,-1,0,1,65536,2**31-1]))
    fragment_event=dict(owner=rng.choice([0,1,2,12]),local=rng.choice([1,2,12]),
        game_mode=rng.choice([0,0,1,4,255]),network_authority=rng.choice([0,1,2,3,128,255]),owner_enemy=rng.choice([0,1,2,3,128,255]),now=rng.randint(-2**31,2**31-1))
    fragment_samples=[rng.choice([-32768,-32767,-361,-360,-3,-2,-1,0,1,2,359,360,32767]) for _ in range(2)]
    u.mem_write(0x44000,bytes(record));u.mem_write(a5-0x1346,struct.pack('>I',0x44000));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(0x42000+(fragment_event['owner']-1)*256+5,bytes([fragment_event['owner_enemy']]))
    u.mem_write(a5-0x133d,bytes([fragment_event['local']]))
    u.mem_write(a5-0x12ca,bytes([fragment_event['game_mode']]));u.mem_write(a5-0x4f4f,bytes([fragment_event['network_authority']]))
    u.mem_write(a5-0x134c,struct.pack('>i',fragment_event['now']))
    fragment_requests=[];fragment_random_calls=0
    run(0x30776,struct.pack('>bxh',fragment_event['owner'],1))
    assert fragment_random_calls==2 and bytes(u.mem_read(0x44000,128))==bytes(record)
    cases.append(dict(routine='death_fragment_requests',a=0,b=0,record=list(record),input=fragment_event,samples=fragment_samples,
        requests=fragment_requests,expected=0))
for hook in fragment_hooks:u.hook_del(hook)
print('1600 complete original death fragment producers executed; two Random values supplied and constructors captured')
# Original sound branches; stop before the unrelated local AI reset loop.
damage_sound_calls=[];damage_sound_flush=False
def damage_sound_service(cpu,address,size,data):
    global damage_sound_flush
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==a5+0x972:
        damage_sound_calls.append(list(struct.unpack('>hI',cpu.mem_read(sp+4,6))));argsize=6
    else:
        damage_sound_flush=True;argsize=0
    cpu.reg_write(UC_M68K_REG_A7,sp+4+argsize);cpu.reg_write(UC_M68K_REG_PC,ret)
damage_sound_hooks=[u.hook_add(UC_HOOK_CODE,damage_sound_service,begin=p,end=p) for p in [a5+0x972,a5+0x95a]]
u.ctl_flush_tb()
for iteration in range(512):
    local=iteration%2;killed=rng.choice([0,0,1,2,255]);lives=rng.choice([-32768,-1,0,1,2,3,32767])
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(0x4205c,struct.pack('>h',lives))
    u.mem_write(a5-0x12d2,bytes([0])) # AI reset path is outside this sound-only check.
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.reg_write(UC_M68K_REG_D6,killed);u.reg_write(UC_M68K_REG_D7,1)
    damage_sound_calls=[];damage_sound_flush=False
    start=0x30eee if local else 0x30f94;end=0x30f3a if local and killed else 0x30fc4
    u.emu_start(start,end,count=1000);assert u.reg_read(UC_M68K_REG_PC)==end
    cases.append(dict(routine='damage_sounds',a=local,b=killed,lives=lives,result=dict(requests=damage_sound_calls,flush=damage_sound_flush),expected=0))
for hook in damage_sound_hooks:u.hook_del(hook)
print('512 original damage sound branches executed; sound/flush services captured, AI reset excluded')
# Complete AI reset service, including participant-one global cache invalidation.
for iteration in range(768):
    participant=iteration%12+1;record=rng.randbytes(256);cache=rng.randbytes(16)
    u.mem_write(0x42000,record);u.mem_write(a5-0x1342,struct.pack('>I',0x42000-(participant-1)*256))
    u.mem_write(a5-0x3b68,cache)
    run(0x172a30,struct.pack('>bx',participant))
    cases.append(dict(routine='reset_ai_participant',a=participant,b=0,record=list(record),cache=list(cache),
        after=list(u.mem_read(0x42000,256)),after_cache=list(u.mem_read(a5-0x3b68,16)),expected=0))
reset_targets=[]
def reset_target_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    reset_targets.append(struct.unpack('>b',cpu.mem_read(sp+4,1))[0])
    cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,ret)
reset_target_hook=u.hook_add(UC_HOOK_CODE,reset_target_service,begin=a5+0xd9a,end=a5+0xd9a);u.ctl_flush_tb()
for iteration in range(512):
    active=[rng.choice([0,1,2,3,128,255]) for _ in range(12)]
    enemy=[rng.choice([0,0,1,2,3,128,255]) for _ in range(12)]
    mode=iteration%256
    records=bytearray(rng.randbytes(12*256))
    for i in range(12):records[i*256+0x88]=active[i];records[i*256+5]=enemy[i]
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x12d2,bytes([mode]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    reset_targets=[];u.emu_start(0x30f3a,0x30fc4,count=2000)
    assert u.reg_read(UC_M68K_REG_PC)==0x30fc4 and bytes(u.mem_read(0x42000,len(records)))==bytes(records)
    cases.append(dict(routine='death_ai_reset_targets',a=mode,b=0,active=active,enemy=enemy,targets=reset_targets,expected=0))
u.hook_del(reset_target_hook)
print('768 complete original AI resets and 512 local-death notification scans executed')
# Death flag selection and local transition state, including original angle wrap.
for iteration in range(512):
    health=rng.choice([-32768,-1,0,1,32767]);mode=iteration%256;supplied=rng.randrange(256)
    u.mem_write(0x4205a,struct.pack('>h',health));u.mem_write(a5-0x12ca,bytes([mode]))
    u.mem_write(0x5700a,bytes([supplied]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x42000);u.reg_write(UC_M68K_REG_A6,0x57000)
    u.emu_start(0x30b9a,0x30bae,count=100)
    assert u.reg_read(UC_M68K_REG_PC)==0x30bae
    cases.append(dict(routine='damage_killed',a=health,b=mode,supplied=supplied,expected=u.reg_read(UC_M68K_REG_D6)&255))
for iteration in range(1024):
    record=bytearray(rng.randbytes(128));state=rng.randbytes(30)
    heading=rng.choice([-2**31,2**31-1,-360<<16,-1,0,180<<16,360<<16])
    struct.pack_into('>i',record,8,heading)
    now=rng.choice([-2**31,-1,0,1,2**31-1]);local=iteration%2==0;killed=rng.choice([0,0,1,2,255])
    u.mem_write(0x44000,bytes(record));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(0x42056,struct.pack('>h',1))
    u.mem_write(a5-0x133d,bytes([1]));u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x1338,state)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.reg_write(UC_M68K_REG_D6,killed);u.reg_write(UC_M68K_REG_D7,1 if local else 2)
    u.emu_start(0x30bae,0x30c34,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x30c34
    cases.append(dict(routine='local_death_state',a=now,b=killed,local=local,record=list(record),state=list(state),
        after=list(u.mem_read(0x44000,128)),after_state=list(u.mem_read(a5-0x1338,30)),expected=0))
print('512 original death-flag selectors and 1024 local-death state blocks executed with original angle wrap')
# Complete active death-camera calculation; explicit direction lookup values.
death_camera_angles=[]
def death_camera_direction(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    first,second,angle=struct.unpack('>IIi',cpu.mem_read(sp+4,12));death_camera_angles.append(angle)
    cpu.mem_write(first,struct.pack('>i',camera_direction[0]));cpu.mem_write(second,struct.pack('>i',camera_direction[1]))
    cpu.reg_write(UC_M68K_REG_A7,sp+16);cpu.reg_write(UC_M68K_REG_PC,ret)
camera_hook=u.hook_add(UC_HOOK_CODE,death_camera_direction,begin=a5+0xbaa,end=a5+0xbaa);u.ctl_flush_tb()
for iteration in range(1024):
    state=bytearray(rng.randbytes(30));now=rng.choice([-2**31,-1,0,1,2**31-1,rng.randrange(10000)])
    struct.pack_into('>iii',state,0,rng.choice([-2**31,-1,0,1,2**31-1]),200,360)
    struct.pack_into('>hh',state,12,750,200);state[29]=1
    base=rng.choice([-32768,-1,0,4,32767]);camera_direction=[rng.randint(-65536,65536) for _ in range(2)]
    u.mem_write(a5-0x1338,bytes(state));u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x20c2,struct.pack('>h',base))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    death_camera_angles=[];u.emu_start(0x750c0,0x75196,count=3000);assert u.reg_read(UC_M68K_REG_PC)==0x75196
    result=dict(pose=list(struct.unpack('>iii',u.mem_read(0x57000-0x1a,12))),height=struct.unpack('>i',u.mem_read(a5-0x38da,4))[0])
    assert bytes(u.mem_read(a5-0x1338,30))==bytes(state)
    cases.append(dict(routine='death_camera',a=now,b=base,state=list(state),direction=camera_direction,angles=death_camera_angles,result=result,expected=0))
u.hook_del(camera_hook)
print('1024 original active death-camera calculations executed; direction lookup supplied, original integer arithmetic and angle wrapping')
# Entire participant lifecycle transition gate; no external services.
for iteration in range(2048):
    now=rng.choice([-2**31,-1,0,1,259,260,261,2_000_000_000,2**31-1])
    event=dict(now=now,deadline=rng.choice([-2**31,-1,0,260,now,2_000_000_000,2**31-1]),
        kind=rng.choice([0,6,7,10,10,31,255]),game_mode=rng.choice([0,0,1,4,255]),local=iteration%2==0,
        authority=rng.choice([0,1,2,3,128,255]),enemy=rng.choice([0,1,2,3,128,255]),
        active=rng.choice([0,1,2,3,128,255]),changed=iteration%256)
    participant=bytearray(rng.randbytes(256));record=bytearray(rng.randbytes(128))
    participant[5]=event['enemy'];participant[0x88]=event['active'];participant[0x54]=event['changed']
    struct.pack_into('>h',participant,0x56,1);record[0x14]=event['kind'];struct.pack_into('>i',record,0x16,event['deadline'])
    slot=1 if event['local'] else 2
    u.mem_write(0x42000,bytes(participant));u.mem_write(0x44000,bytes(record))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000-(slot-1)*256));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x134c,struct.pack('>i',event['now']));u.mem_write(a5-0x133d,bytes([1]))
    u.mem_write(a5-0x12ca,bytes([event['game_mode']]));u.mem_write(a5-0x4f4f,bytes([event['authority']]))
    run(0x31cfe,struct.pack('>bx',slot))
    assert bytes(u.mem_read(0x42000,256))==bytes(participant) and bytes(u.mem_read(0x44000,128))==bytes(record)
    cases.append(dict(routine='participant_transition_ready',a=0,b=0,input=event,expected=u.mem_read(stack+6,1)[0]))
print('2048 complete original participant lifecycle transition gates executed without service fixtures')
# Lifecycle entry state before health/life dispatch.
for iteration in range(512):
    participant=bytearray(rng.randbytes(256));record=rng.randbytes(128)
    participant[5]=rng.choice([0,0,1,2,255]);participant[8]=iteration%256
    struct.pack_into('>h',participant,0x56,1)
    u.mem_write(0x42000,bytes(participant));u.mem_write(0x44000,record)
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D7,1)
    u.emu_start(0x31aa0,0x31afa,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x31afa
    cases.append(dict(routine='begin_participant_transition',a=0,b=0,record=list(record),participant=list(participant),
        after_record=list(u.mem_read(0x44000,128)),after_participant=list(u.mem_read(0x42000,256)),expected=0))
print('512 original participant transition entry blocks executed without fixtures')
# Full original ground projector for one point, including culling and height-256 fast path.
ground_table=[[rng.randint(-32768,32767),rng.randint(-32768,32767)] for _ in range(1200)]
u.mem_write(0x46000,b''.join(struct.pack('>hh',*p) for p in ground_table))
for iteration in range(1024):
    depth=rng.choice([(8<<16)-1,8<<16,(300<<16)-1,300<<16,rng.randint(-10<<16,400<<16)])
    horizontal=rng.choice([0,depth,-depth,depth+1,-depth-1,rng.randint(-400<<16,400<<16)])
    height=rng.choice([-2**31,-1,0,255,256,257,32*8,107*8,482*8,2**31-1])
    center=[rng.randint(-32768,32767),rng.randint(-32768,32767)]
    u.mem_write(0x41000,bytes(12))
    args=struct.pack('>IIihhhiiiiI',0x41000,0x41004,height,center[1],center[0],1,0,0,horizontal,depth,0x46000)
    run(0xe3ce6,args)
    count=struct.unpack('>h',u.mem_read(0x41000,2))[0];assert count in [0,1]
    result=list(reversed(struct.unpack('>hh',u.mem_read(0x41004,4)))) if count else None
    cases.append(dict(routine='project_ground_point',a=height,b=0,point=[horizontal,depth],center=center,table=ground_table,result=result,expected=0))
print('1024 complete original single-point ground projections executed with synthetic reciprocal/precomputed-height table')
# Original ground table initialization, original division and bytewise word writer.
u.mem_write(a5+0x5a,bytes.fromhex('4ef9')+struct.pack('>I',0x1200b8));u.ctl_flush_tb()
for iteration in range(4):
    before=rng.randbytes(4800);u.mem_write(0x46000,before);u.mem_write(a5-0x38ca,struct.pack('>I',0x46000))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x70442,0x704c2,count=200000);assert u.reg_read(UC_M68K_REG_PC)==0x704c2
    raw=bytes(u.mem_read(0x46000,4800));assert raw[:4]==before[:4]
    original_ground_table=[list(p) for p in struct.iter_unpack('>hh',raw)]
    cases.append(dict(routine='ground_projection_table',a=0,b=0,before=[list(p) for p in struct.iter_unpack('>hh',before)],after=original_ground_table,expected=0))
for iteration in range(512):
    depth=rng.choice([(8<<16)-1,8<<16,(8<<16)+16383,(8<<16)+16384,16<<16,100<<16,(300<<16)-1,300<<16,rng.randint(0,300<<16)])
    horizontal=rng.choice([0,depth,-depth,depth+1,-depth-1,rng.randint(-300<<16,300<<16)])
    height=rng.choice([0,32*8,107*8,482*8,1003*8]);center=[160,100]
    u.mem_write(0x41000,bytes(12))
    run(0xe3ce6,struct.pack('>IIihhhiiiiI',0x41000,0x41004,height,center[1],center[0],1,0,0,horizontal,depth,0x46000))
    count=struct.unpack('>h',u.mem_read(0x41000,2))[0];assert count in [0,1]
    result=list(reversed(struct.unpack('>hh',u.mem_read(0x41004,4)))) if count else None
    cases.append(dict(routine='project_ground_point',a=height,b=0,point=[horizontal,depth],center=center,table=original_ground_table,result=result,expected=0))
print('4 original 1200-entry ground table initializations and 512 projections using the original-generated table executed without fixtures')
# Viewport extent/center/placement arithmetic, stopping before QuickDraw OffsetRect.
u.mem_map(0x190000,0x10000);u.mem_write(0x190000,(source/'33.bin').read_bytes())
u.mem_write(a5+0x452,bytes.fromhex('4ef9')+struct.pack('>I',0x7002c));u.ctl_flush_tb()
for iteration in range(1024):
    bounds=rng.choice([[0,0,480,640],[0,0,600,800],[40,80,520,720],[0,0,79,320],
        [rng.randint(-32768,32767) for _ in range(4)]])
    limits=[rng.choice([-32768,-1,0,319,320,479,480,640,800,32767]) for _ in range(2)]
    u.mem_write(a5-0x6632,struct.pack('>4h',*bounds));u.mem_write(a5-0x3ff0,struct.pack('>h',limits[0]));u.mem_write(a5-0x3ff4,struct.pack('>h',limits[1]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x190746,0x1907a6,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1907a6
    height,width=struct.unpack('>hh',u.mem_read(a5-0x39a4,4))
    center_y,center_x=struct.unpack('>hh',u.mem_read(a5-0x62fe,4))
    assert struct.unpack('>h',u.mem_read(a5-0x39a6,2))[0]==center_x
    u.emu_start(0x1907c2,0x1907e6,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1907e6
    dy,dx,rect=struct.unpack('>hhI',u.mem_read(u.reg_read(UC_M68K_REG_A7),8));assert rect==a5-0x39ae
    cases.append(dict(routine='viewport_metrics',a=0,b=0,bounds=bounds,limits=limits,
        result=dict(size=[width,height],center=[center_x,center_y],offset=[dx,dy]),expected=0))
print('1024 original viewport metric blocks executed with original min helper; stopped before QuickDraw placement')
# Original display extents and compact-mode inset arguments; no QuickDraw fixture.
u.mem_map(0x1a0000,0x10000);u.mem_write(0x1a0000,(source/'17.bin').read_bytes())
for iteration in range(512):
    bounds=rng.choice([[0,0,480,640],[0,0,342,512],[0,0,343,513],[40,80,520,720],
        [rng.randint(-32768,32767) for _ in range(4)]])
    full_size=rng.choice([0,0,1,127,255])
    u.mem_write(a5-0x3ffe,struct.pack('>4h',*bounds));u.mem_write(a5-0x7bf0,bytes([full_size]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    end=0x1a045e if full_size else 0x1a04b0
    u.emu_start(0x1a043c,end,count=10000);assert u.reg_read(UC_M68K_REG_PC)==end
    width=struct.unpack('>i',u.mem_read(a5-0x3ff2,4))[0]
    height=struct.unpack('>i',u.mem_read(a5-0x3ff6,4))[0]
    inset=None
    if not full_size:
        dy,dx,rect=struct.unpack('>hhI',u.mem_read(u.reg_read(UC_M68K_REG_A7),8));assert rect==a5-0x3ffe
        inset=[dx,dy]
    cases.append(dict(routine='display_rectangle_setup',a=0,b=0,bounds=bounds,full_size=full_size,
        result=dict(size=[width,height],inset=inset),expected=0))
print('512 original display extent/inset blocks executed with original division; stopped before QuickDraw InsetRect')
# Complete row projectors in both display modes, with existing output prefixes.
u.mem_write(0x46000,b''.join(struct.pack('>hh',*p) for p in original_ground_table))
for iteration in range(1024):
    full_display=bool(iteration%2)
    depth=rng.choice([8<<16,60<<16,100<<16,299<<16,rng.randint(0,300<<16)])
    horizontal=rng.choice([depth,-depth,depth+1,-depth-1,2*depth,-2*depth,2*depth+1,rng.randint(-300<<16,300<<16)])
    start=[horizontal,depth]
    step=[rng.randint(-16<<16,16<<16),rng.randint(-8<<16,8<<16)]
    count=rng.choice([-32768,-1,0,1,2,8,32,64]);height=rng.choice([0,255,256,257,32*8,482*8])
    center=[rng.randint(-32768,32767),rng.randint(-32768,32767)]
    prefix=rng.randbytes(rng.randrange(5)*4);u.mem_write(0x41000,struct.pack('>h',len(prefix)//4))
    u.mem_write(0x41004,prefix+bytes(512))
    run(0xe3de6 if full_display else 0xe3ce6,struct.pack('>IIihhhiiiiI',0x41000,0x41004,height,center[1],center[0],count,step[0],step[1],start[0],start[1],0x46000))
    final_count=struct.unpack('>h',u.mem_read(0x41000,2))[0]
    assert len(prefix)//4<=final_count<=len(prefix)//4+max(count,0)
    raw=bytes(u.mem_read(0x41004,final_count*4));assert raw[:len(prefix)]==prefix
    result=[list(reversed(p)) for p in struct.iter_unpack('>hh',raw[len(prefix):])]
    cases.append(dict(routine='project_ground_row',a=height,b=0,start=start,step=step,count=count,center=center,
        full_display=full_display,table=original_ground_table,result=result,expected=0))
print('1024 complete original ground-row projectors executed in both display modes; prior output prefixes preserved')
# Ground lattice defaults, spacing, and sparse-column alignment.
for flag in [0,1,127,255]:
    u.mem_write(a5-0x27bc,bytes([0xa5])*121);u.mem_write(a5-0x7b99,bytes([flag]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x704c2,0x70500,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x70500
    cases.append(dict(routine='ground_grid_defaults',a=0,b=0,mask=list(u.mem_read(a5-0x27bc,121)),
        divisions=struct.unpack('>h',u.mem_read(a5-0x27be,2))[0],expected=0))
for iteration in range(512):
    extent=rng.choice([1024,-32768,32767,rng.randint(-32768,32767)])
    divisions=rng.choice([64,-32768,-1,1,32767])
    u.mem_write(a5-0x137e,struct.pack('>h',extent));u.mem_write(a5-0x27be,struct.pack('>h',divisions))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x190348,0x19036a,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x19036a
    cases.append(dict(routine='ground_grid_spacing',a=extent,b=divisions,
        result=struct.unpack('>i',u.mem_read(a5-0x27c2,4))[0],expected=0))
for iteration in range(1024):
    first=rng.choice([0,1,7,8,9,63,64,rng.randint(-32768,32767)])
    last=rng.choice([0,64,120,rng.randint(-32768,32767)])
    u.mem_write(0x57000-0x14,struct.pack('>hh',first,last))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    for register in [UC_M68K_REG_D4,UC_M68K_REG_D5,UC_M68K_REG_D6,UC_M68K_REG_D7]:u.reg_write(register,0)
    u.emu_start(0x73d08,0x73d86,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x73d86
    skip=struct.unpack('>h',u.mem_read(0x57000-0x1c,2))[0]-1
    count=struct.unpack('>h',u.mem_read(0x57000-0x44,2))[0]
    cases.append(dict(routine='ground_sparse_row',a=first,b=last,result=[skip,count],expected=0))
print('4 original lattice initializers, 512 spacing blocks, and 1024 sparse-row alignment blocks executed')
# Complete visible-grid bounds routine, including original min/max helpers.
for iteration in range(1024):
    origin=[rng.randint(-128<<16,1152<<16) for _ in range(2)]
    edges=[[rng.randint(-500<<16,500<<16) for _ in range(2)] for _ in range(2)]
    spacing=rng.choice([8<<16,16<<16,32<<16,64<<16]);divisions=rng.choice([16,32,64,120])
    u.mem_write(a5-0x27c2,struct.pack('>i',spacing));u.mem_write(a5-0x27be,struct.pack('>h',divisions))
    u.mem_write(0x41000,bytes(8))
    args=struct.pack('>IIIIiiiiii',0x41006,0x41004,0x41002,0x41000,
        edges[1][1],edges[1][0],edges[0][1],edges[0][0],origin[1],origin[0])
    run(0x73918,args)
    result=list(struct.unpack('>4h',u.mem_read(0x41000,8)))
    cases.append(dict(routine='ground_visible_bounds',a=spacing,b=divisions,origin=origin,edges=edges,result=result,expected=0))
print('1024 complete original visible-grid bounds routines executed, including original min/max helpers')
# Camera-to-ground basis with supplied direction values and original software multiply.
u.mem_write(a5+0x75a,bytes.fromhex('4ef9')+struct.pack('>I',0xe3c5a));u.mem_write(a5-0x7b99,b'\0');u.ctl_flush_tb()
for iteration in range(1024):
    position=[rng.choice([-2**31,2**31-1,rng.randint(-128<<16,1152<<16)]) for _ in range(2)]
    direction=[rng.randint(-65536,65536) for _ in range(2)]
    spacing=rng.choice([8<<16,16<<16,32<<16,64<<16])
    u.mem_write(0x57000-0xc,struct.pack('>ii',*position))
    u.mem_write(0x57000-0x4c,struct.pack('>ii',direction[1],direction[0]))
    u.mem_write(a5-0x27c2,struct.pack('>i',spacing))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x73b48,0x73bc2,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x73bc2
    signed=lambda n:((n+2**31)%2**32)-2**31
    origin=[signed(u.reg_read(UC_M68K_REG_D5)),signed(u.reg_read(UC_M68K_REG_D4))]
    step=[signed(u.reg_read(UC_M68K_REG_D7)),signed(u.reg_read(UC_M68K_REG_D6))]
    # Execute the actual row advance too; don't infer its output from the column step.
    u.emu_start(0x73e74,0x73e84,count=100);assert u.reg_read(UC_M68K_REG_PC)==0x73e84
    row_step=[signed(u.reg_read(UC_M68K_REG_D5)-origin[0]),signed(u.reg_read(UC_M68K_REG_D4)-origin[1])]
    cases.append(dict(routine='ground_camera_basis',a=spacing,b=0,position=position,direction=direction,
        result=dict(origin=origin,column_step=step,row_step=row_step),expected=0))
print('1024 original camera-to-ground basis blocks and row advances executed; direction values supplied')
# Composed ground pipeline: original caller, bounds, arithmetic, and row projectors.
for displacement,target in [(0x76a,0xe3ce6),(0x772,0xe3de6)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.mem_write(0x46000,b''.join(struct.pack('>hh',*p) for p in original_ground_table))
u.mem_write(a5-0x38ca,struct.pack('>I',0x46000));u.mem_write(a5-0x27c2,struct.pack('>i',16<<16));u.mem_write(a5-0x27be,struct.pack('>h',64))
u.mem_write(a5-0x27bc,bytes(int(i%8==0) for i in range(121)))
def ground_direction_service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret,cos_ptr,sin_ptr,angle=struct.unpack('>IIIi',cpu.mem_read(sp,16))
    cpu.mem_write(cos_ptr,struct.pack('>i',direction[1]));cpu.mem_write(sin_ptr,struct.pack('>i',direction[0]))
    cpu.reg_write(UC_M68K_REG_A7,sp+16);cpu.reg_write(UC_M68K_REG_PC,ret)
ground_hook=u.hook_add(UC_HOOK_CODE,ground_direction_service,begin=a5+0xbaa,end=a5+0xbaa);u.ctl_flush_tb()
for iteration in range(256):
    position=[rng.randint(0,1024<<16) for _ in range(2)]
    direction=rng.choice([[0,65536],[65536,0],[0,-65536],[-65536,0],[46340,46340],[-46340,46340],[30000,-58000]])
    full_display=bool(iteration%2);height=rng.choice([0,32,107,482,1003]);center=[rng.randint(100,400),rng.randint(100,300)]
    u.mem_write(0x42000,struct.pack('>iii',*position,0))
    u.mem_write(a5-0x38da,struct.pack('>i',height));u.mem_write(a5-0x7bf0,bytes([full_display]))
    u.mem_write(a5-0x62fe,struct.pack('>hh',center[1],center[0]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>IhhhI',stop,0,0,0,0x42000))
    u.emu_start(0x73b10,0x73e96,count=200000);assert u.reg_read(UC_M68K_REG_PC)==0x73e96
    frame=u.reg_read(UC_M68K_REG_A6);count=struct.unpack('>h',u.mem_read(frame-0x104e,2))[0]
    assert 0<=count<=1024
    raw=bytes(u.mem_read(frame-0x104c,count*4));result=[list(reversed(p)) for p in struct.iter_unpack('>hh',raw)]
    cases.append(dict(routine='ground_points',a=height,b=0,position=position,direction=direction,center=center,
        full_display=full_display,table=original_ground_table,result=result,expected=0))
u.hook_del(ground_hook)
print('256 composed original ground pipelines executed through point generation; direction supplied and pixel drawing excluded')
# Resource color bindings after resource selection, before display-depth overrides.
color_resources=[(root/'.local/game-resources'/kind/'999.bin').read_bytes() for kind in ['636f6231','636f6234','636f6238']]
for data in color_resources+[rng.randbytes(128) for _ in range(512)]:
    u.mem_write(0x42000,struct.pack('>I',0x44000));u.mem_write(0x44000,data)
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x42000);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D7,1)
    u.emu_start(0x70838,0x7094a,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x7094a
    pairs=lambda first,second,n:[[u.mem_read(a5-first+i,1)[0],u.mem_read(a5-second+i,1)[0]] for i in range(n)]
    result=dict(vehicles=pairs(0x270e,0x2712,4),kinds=pairs(0x2742,0x3b30,47),effects=list(u.mem_read(a5-0x3b00,4)),
        special=[u.mem_read(a5-offset,1)[0] for offset in [0x3b32,0x3b33,0x3b31,0x3b34,0x3b35,0x3b36,0x3b37,0x3b38]])
    cases.append(dict(routine='color_bindings',a=0,b=0,data=list(data),result=result,expected=0))
print('515 original color-binding loader blocks executed, including all three disc tables; resource selection and depth overrides excluded')
# Base RGB triples copied from selected clut, before level-specific overrides.
palette_resources=[p.read_bytes() for p in sorted((root/'.local/game-resources/636c7574').glob('*.bin'))]
for data in palette_resources+[rng.randbytes(112) for _ in range(512)]:
    u.mem_write(0x42000,struct.pack('>I',0x44000));u.mem_write(0x44000,data)
    u.mem_write(0x57000-0x4a0,struct.pack('>I',0x42000))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe1184,0xe11c4,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe11c4
    result=[list(p) for p in struct.iter_unpack('>3H',bytes(u.mem_read(0x57000-0x494,78)))]
    cases.append(dict(routine='base_palette',a=0,b=0,data=list(data),result=result,expected=0))
print('516 original base-palette copies executed, including all four disc cluts; later color substitutions excluded')
# Complete level-color routines with explicit resource-handle service fixtures.
color_requests=[]
def level_color_resource(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);rid,kind=struct.unpack('>hI',cpu.mem_read(sp,6));color_requests.append((kind,rid))
    cpu.mem_write(sp+6,struct.pack('>I',0x42000 if color_data is not None else 0))
    cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,address+2)
color_hooks=[u.hook_add(UC_HOOK_CODE,level_color_resource,begin=p,end=p) for p in [0xe0dc8,0xe0e74]];u.ctl_flush_tb()
for iteration in range(1024):
    use_2001=bool(iteration%2);rid=2001 if use_2001 else 2000
    level=rng.choice([-32768,-1,0,1,2,29,30,31,60,61,32767,rng.randint(-32768,32767)])
    depth=rng.choice([1,4,8,16,32]);color_data=rng.choice([None,(root/f'.local/game-resources/636c7574/{rid}.bin').read_bytes(),rng.randbytes(248)])
    u.mem_write(a5-0x3fe4,struct.pack('>h',depth));u.mem_write(0x42000,struct.pack('>I',0x44000))
    if color_data is not None:u.mem_write(0x44000,color_data)
    u.mem_write(0x41000,bytes(6));color_requests.clear()
    run(0xe0e1a if use_2001 else 0xe0d9c,struct.pack('>hI',level,0x41000))
    assert color_requests==([] if use_2001 and depth==4 else [(0x636c7574,rid)])
    result=list(struct.unpack('>3H',u.mem_read(0x41000,6)))
    cases.append(dict(routine='level_palette_color',a=level,b=depth,use_2001=use_2001,data=None if color_data is None else list(color_data),result=result,expected=0))
for hook in color_hooks:u.hook_del(hook)
print('1024 complete original level-color routines executed; GetResource handles supplied, fallback RGB constructors execute directly')
# Base copy, both original level substitutions, and final black entry together.
composed_requests=[]
def composed_palette_resource(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);rid,kind=struct.unpack('>hI',cpu.mem_read(sp,6));assert kind==0x636c7574
    composed_requests.append(rid);handle={2000:0x42100,2001:0x42200}[rid] if level_cluts[rid] is not None else 0
    cpu.mem_write(sp+6,struct.pack('>I',handle));cpu.reg_write(UC_M68K_REG_A7,sp+6);cpu.reg_write(UC_M68K_REG_PC,address+2)
composed_hooks=[u.hook_add(UC_HOOK_CODE,composed_palette_resource,begin=p,end=p) for p in [0xe0dc8,0xe0e74]];u.ctl_flush_tb()
for iteration in range(512):
    base=rng.choice(palette_resources+[rng.randbytes(112)])
    level=rng.choice([1,2,7,10,13,14,30,31,60,-32768,32767]);depth=rng.choice([4,8,16,32]);active=rng.choice([2,4,8,13])
    level_cluts={rid:rng.choice([None,(root/f'.local/game-resources/636c7574/{rid}.bin').read_bytes(),rng.randbytes(248)]) for rid in [2000,2001]}
    for handle,address,data in [(0x42000,0x44000,base),(0x42100,0x45000,level_cluts[2000]),(0x42200,0x46000,level_cluts[2001])]:
        u.mem_write(handle,struct.pack('>I',address))
        if data is not None:u.mem_write(address,data)
    u.mem_write(0x57000-0x4a0,struct.pack('>I',0x42000));u.mem_write(0x57000+8,struct.pack('>h',level));u.mem_write(a5-0x3fe4,struct.pack('>h',depth))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D5,active)
    composed_requests.clear();u.emu_start(0xe1184,0xe1224,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe1224
    assert composed_requests==([2000] if depth==4 else [2001,2000])
    result=[list(p) for p in struct.iter_unpack('>3H',bytes(u.mem_read(0x57000-0x494,78)))]
    cases.append(dict(routine='compose_resource_palette',a=level,b=depth,base=list(base),active_colors=active,
        clut_2000=None if level_cluts[2000] is None else list(level_cluts[2000]),clut_2001=None if level_cluts[2001] is None else list(level_cluts[2001]),result=result,expected=0))
for hook in composed_hooks:u.hook_del(hook)
print('512 composed original palette blocks executed; resource handles supplied, both level routines and black override execute directly')
# Execute the complete selector wrapper; capture its face-constructor call.
face_calls=[]
def capture_face(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    face_calls.append(bytes(cpu.mem_read(sp+4,20)))
    return_pc=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+24)
    cpu.reg_write(UC_M68K_REG_PC,return_pc)
u.mem_write(a5+0x7c2,bytes.fromhex('4e75'))
face_hook=u.hook_add(UC_HOOK_CODE,capture_face,begin=a5+0x7c2,end=a5+0x7c2)
u.ctl_flush_tb()
for iteration in range(1024):
    data=rng.randbytes(47)
    # Include signed aliases whose low byte still addresses a real kind.
    selector=rng.choice([255,-1,1000,32767,rng.randrange(1000,32768),
        rng.randrange(47)+rng.choice([0,256,512,768,-256,-32768])])
    payload=rng.randbytes(18)
    u.mem_write(a5-0x2742,data)
    face_calls.clear()
    run(0xc3ea0,struct.pack('>h',selector)+payload)
    assert len(face_calls)==1 and face_calls[0][2:]==payload
    result=struct.unpack('>h',face_calls[0][:2])[0]
    cases.append(dict(routine='face_color_selector',a=selector,b=0,data=list(data),result=result,expected=0))
u.hook_del(face_hook)
print('1024 complete original face selector calls executed; downstream constructor captured, all other arguments preserved')
# Original face-light arithmetic and its bright/distance adjustment blocks.
for iteration in range(1024):
    span=1024 if iteration%2==0 else 32767
    normal=[rng.randint(-span,span) for _ in range(3)]
    light=[rng.randint(-32768,32767) for _ in range(2)]
    u.mem_write(0x43000,struct.pack('>hhih',normal[0],normal[2],0,normal[1]))
    u.mem_write(0x57000-0x5cc,struct.pack('>h',light[0]));u.mem_write(0x57000-0x5ce,struct.pack('>h',light[1]))
    u.reg_write(UC_M68K_REG_A3,0x43000);u.reg_write(UC_M68K_REG_A5,a5)
    u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x101854,0x1018dc,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1018dc
    result=struct.unpack('>h',struct.pack('>H',u.reg_read(UC_M68K_REG_D4)&65535))[0]
    cases.append(dict(routine='face_light_shade',a=0,b=0,normal=normal,light=light,expected=result))
    shade=rng.choice([result,-32768,-1,0,24,32,64,32767]);depth=rng.choice([-32768,-1,0,2400,4800,32767,rng.randint(-32768,32767)])
    bright=bool(iteration&1);fade=bool(iteration&2)
    u.mem_write(0x57000+0x10,bytes([bright]));u.mem_write(0x57000+0xe,bytes([fade]))
    u.mem_write(0x57000+0x28,struct.pack('>I',0x43000));u.mem_write(0x43006,struct.pack('>h',depth))
    u.reg_write(UC_M68K_REG_D4,shade&65535);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x101924,0x101986,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x101986
    result=struct.unpack('>h',struct.pack('>H',u.reg_read(UC_M68K_REG_D4)&65535))[0]
    cases.append(dict(routine='adjust_face_shade',a=shade,b=depth,bright=bright,fade=fade,expected=result))
print('1024 original face-light blocks and 1024 bright/distance adjustment blocks executed with supplied normal and horizontal light components')
# Execute the shade lookup and complete eight-bit pattern builder.
for iteration in range(2048):
    color_map=list(range(13)) if iteration%2==0 else [rng.randint(-32768,32767) for _ in range(13)]
    u.mem_write(a5-0x7526,struct.pack('>13h',*color_map))
    shade=rng.choice([0,1,2,3,15,16,17,-32768,32767,rng.randint(-32768,32767)])
    color=iteration%13;depth=rng.choice([1,4,8,16,32]);four_bit=bool(iteration&1)
    u.mem_write(a5-0x3fe4,struct.pack('>h',depth));u.mem_write(a5-0x6647,bytes([four_bit]))
    run(0xe21be,struct.pack('>hBB',shade,color,0))
    result=struct.unpack('>h',u.mem_read(stack+8,2))[0]
    cases.append(dict(routine='shade_pixel_index',a=shade,b=color,depth=depth,four_bit_color=four_bit,map=color_map,result=result,expected=0))
    shade=rng.choice([-32768,-1,0,1,2,3,4,23,24,31,32,63,64,65,32767,rng.randint(0,64)])
    u.mem_write(a5-0x3fe4,struct.pack('>h',8));u.mem_write(a5-0x7bf1,bytes([0]))
    run(0xe227a,struct.pack('>hBB',shade,color,0))
    result=struct.unpack('>I',u.mem_read(stack+8,4))[0]
    cases.append(dict(routine='face_pixel_pattern_8',a=shade,b=color,map=color_map,result=result,expected=0))
print('2048 complete original shade lookups and 2048 complete eight-bit pattern builders executed with supplied color remapping tables')
# Original RGB interpolation; the jump stub points to original signed min-long.
u.mem_write(a5+0x462,bytes.fromhex('4ef9')+struct.pack('>I',0x7004c));u.ctl_flush_tb()
for iteration in range(1024):
    base=[rng.randrange(65536) for _ in range(3)];target=[rng.randrange(65536) for _ in range(3)]
    fraction=rng.choice([-2**31,-65536,-1,0,1,32768,65534,65535,65536,2**31-1,rng.randint(-2**31,2**31-1)])
    u.mem_write(0x43000,struct.pack('>3H',*base));u.mem_write(0x43010,struct.pack('>3H',*target))
    run(0xe0ca0,struct.pack('>iIII',fraction,0x43010,0x43000,0x43020))
    result=list(struct.unpack('>3H',u.mem_read(0x43020,6)))
    cases.append(dict(routine='interpolate_rgb',a=fraction,b=0,base=base,target=target,result=result,expected=0))
# FixRatio fixture supplies only exact j/16 values; interpolation executes directly.
ramp_ratios=[]
def ramp_ratio(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);denominator,numerator=struct.unpack('>hh',cpu.mem_read(sp,4))
    assert denominator==16 and 0<=numerator<16
    ramp_ratios.append(numerator)
    cpu.mem_write(sp+4,struct.pack('>i',numerator<<12))
    cpu.reg_write(UC_M68K_REG_A7,sp+4);cpu.reg_write(UC_M68K_REG_PC,address+2)
ramp_hook=u.hook_add(UC_HOOK_CODE,ramp_ratio,begin=0xe13a4,end=0xe13a4);u.ctl_flush_tb()
for iteration in range(64):
    base=[[rng.randrange(65536) for _ in range(3)] for _ in range(13)]
    target=[rng.randrange(65536) for _ in range(3)] if iteration%2 else [0,0,0]
    u.mem_write(0x57000-0x494,b''.join(struct.pack('>3H',*rgb) for rgb in base))
    u.mem_write(0x57000-0x12,struct.pack('>3H',*target));u.mem_write(0x57000-0x49c,struct.pack('>h',16))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D5,13)
    ramp_ratios.clear();u.emu_start(0xe1360,0xe13ec,count=100000);assert u.reg_read(UC_M68K_REG_PC)==0xe13ec
    assert ramp_ratios==list(range(16))*12
    result=[list(rgb) for rgb in struct.iter_unpack('>3H',bytes(u.mem_read(a5-0x708a,192*6)))]
    cases.append(dict(routine='rgb_shade_ramps_8',a=0,b=0,base=base,target=target,result=result,expected=0))
u.hook_del(ramp_hook)
print('1024 complete original RGB interpolations and 64 complete 192-color ramp blocks executed; only exact j/16 FixRatio results supplied')
# Original row-word selector followed by the complete eight-bit span writer.
# The fixture supplies one changed solid span and one prior span; textured
# streams and unchanged-span optimizations are outside this comparison.
for iteration in range(1024):
    pattern=rng.getrandbits(32);row=rng.randint(-32768,32767)
    start=rng.randrange(32);count=rng.randrange(1,65)
    initial=rng.randbytes(128)
    u.mem_write(0x43000,struct.pack('>I',pattern));u.mem_write(0x57000+0x34,struct.pack('>i',row))
    u.reg_write(UC_M68K_REG_A1,0x43000);u.reg_write(UC_M68K_REG_A4,0x43010);u.reg_write(UC_M68K_REG_A6,0x57000)
    u.emu_start(0x1a0d56,0x1a0d64,count=20);assert u.reg_read(UC_M68K_REG_PC)==0x1a0d64
    selected=struct.unpack('>H',u.mem_read(0x43010,2))[0]
    assert selected==((pattern>>(16 if row&1 else 0))&65535)
    # Two 128-pixel streams, each followed by another valid span.
    u.mem_write(0x44000,struct.pack('>hHhH',128,selected,128,selected))
    u.mem_write(0x44100,struct.pack('>hHhH',128,selected^65535,128,selected^65535))
    u.mem_write(0x44200,struct.pack('>II',0x44000,0x44100));u.mem_write(0x45000,initial)
    run(0x1a0e62,struct.pack('>7I',0x44200,0x44204,0x45000,3,start,start,count))
    result=list(u.mem_read(0x45000,128))
    cases.append(dict(routine='pattern_span_8',a=row,b=0,pattern=pattern,start=start,count=count,initial=list(initial),result=result,expected=0))
print('1024 original scanline selections and complete solid-span writes executed; eight-bit changed spans, aligned framebuffer, untouched bytes compared')
# Complete original face-plane constructor, including its integer length helper.
for iteration in range(2048):
    span=100 if iteration%2==0 else 32767
    vertices=[[rng.randint(-span,span) for _ in range(3)] for _ in range(3)]
    if iteration%8==0:vertices[2]=vertices[1].copy()
    if iteration%8==1:vertices=[[0,0,0],[1,0,0],[1,1,0]]
    color=rng.randint(-32768,32767)
    u.mem_write(0x43000,bytes(40))
    for index,vertex in enumerate(vertices):u.mem_write(0x43002+index*10,struct.pack('>3h',*vertex))
    run(0x10026c,struct.pack('>hhhhII',color,1,2,3,0x43000,0x44000))
    result=list(u.mem_read(0x44000,12))
    cases.append(dict(routine='face_plane',a=color,b=0,vertices=vertices,result=result,expected=0))
print('2048 complete original face-plane constructors executed, including original cross-product scaling and integer length; no service fixtures')
for iteration in range(1024):
    direction=[rng.randint(-32768,32767) for _ in range(2)]
    u.mem_write(0x57000-0x5c8,struct.pack('>h',direction[0]));u.mem_write(0x57000-0x5c4,struct.pack('>h',direction[1]))
    u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x10145e,0x1014be,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1014be
    result=[struct.unpack('>h',u.mem_read(0x57000-offset,2))[0] for offset in [0x5cc,0x5ce]]
    cases.append(dict(routine='face_light_components',a=0,b=0,direction=direction,result=result,expected=0))
face_ticks=[];face_clock_calls=[]
def face_clock(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);index=len(face_clock_calls)
    assert index<2
    cpu.mem_write(sp,struct.pack('>i',face_ticks[index]));face_clock_calls.append(address)
    cpu.reg_write(UC_M68K_REG_PC,address+2)
face_hooks=[u.hook_add(UC_HOOK_CODE,face_clock,begin=p,end=p) for p in [0x1018ec,0x1018f8]];u.ctl_flush_tb()
for iteration in range(1024):
    deadline=rng.choice([-2**31,2**31-1,0,100]);toggle=rng.randrange(256)
    first=rng.choice([deadline,((deadline+1+2**31)%2**32)-2**31,rng.randint(-2**31,2**31-1)])
    face_ticks[:]=[first,rng.randint(-2**31,2**31-1)]
    color=rng.choice([13,0,12,1000,-1,rng.randint(-32768,32767)])
    index=rng.randint(-32768,32767);limit=rng.choice([index,index+1 if index<32767 else index,rng.randint(-32768,32767)])
    default=rng.randrange(256);flash=rng.randrange(256)
    u.mem_write(a5-0x6624,struct.pack('>i',deadline));u.mem_write(a5-0x6625,bytes([toggle]))
    u.mem_write(0x4300a,struct.pack('>h',color));u.mem_write(0x57000-0x5d6,struct.pack('>h',default if default<128 else default-256))
    u.mem_write(0x57000+0x20,struct.pack('>h',limit));u.mem_write(0x57000+0x1e,bytes([flash]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A3,0x43000);u.reg_write(UC_M68K_REG_D7,index&65535)
    u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    face_clock_calls.clear();u.emu_start(0x1018dc,0x101924,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x101924
    assert len(face_clock_calls)==(0 if color==13 else 2 if first>deadline else 1)
    result=[struct.unpack('>h',u.mem_read(0x57000-0x5d6,2))[0],struct.unpack('>i',u.mem_read(a5-0x6624,4))[0],u.mem_read(a5-0x6625,1)[0]]
    cases.append(dict(routine='rendered_face_color',a=default,b=flash,face=[color,index,limit],ticks=face_ticks.copy(),deadline=deadline,toggle=toggle,result=result,expected=0))
for hook in face_hooks:u.hook_del(hook)
print('1024 original light-component blocks and 1024 face-color/flash blocks executed; direction words and TickCount values supplied')
# Face submissions must run unchanged executable bytes.
assert bytes(u.mem_read(0xc0000,len((source/'19.bin').read_bytes()))) == (source/'19.bin').read_bytes(), 'CODE 19 changed during fixtures'
# Original tetrahedron face submissions; constructor hook captures its arguments.
tetra_faces=[]
def capture_tetra_face(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7)
    words=struct.unpack('>10h',cpu.mem_read(sp+4,20))
    assert words[9]==3 and words[1:6]==(0,0,0,0,0)
    tetra_faces.append((list(v-1 for v in [words[8],words[7],words[6]]),words[0]))
    return_pc=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+24);cpu.reg_write(UC_M68K_REG_PC,return_pc)
tetra_hook=u.hook_add(UC_HOOK_CODE,capture_tetra_face,begin=a5+0x7c2,end=a5+0x7c2);u.ctl_flush_tb()
for color in range(256):
    u.mem_write(a5-0x2741,bytes([color]));u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,0x56000)
    tetra_faces.clear();u.emu_start(0xc3212,0xc328a,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0xc328a
    assert len(tetra_faces)==4
    cases.append(dict(routine='tetrahedron_faces',a=color,b=0,faces=[v[0] for v in tetra_faces],colors=[v[1] for v in tetra_faces],expected=0))
u.hook_del(tetra_hook)
print('256 original tetrahedron face-submission blocks executed; all kind-one primary bytes, original winding and constructor arguments captured')
# Two original procedural box callers and their complete six-face builder.
box_faces=[]
def capture_box_face(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);words=struct.unpack('>10h',cpu.mem_read(sp+4,20))
    assert words[9]==4 and words[1:5]==(0,0,0,0)
    box_faces.append(([v-1 for v in [words[8],words[7],words[6],words[5]]],words[0]))
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+24);cpu.reg_write(UC_M68K_REG_PC,ret)
u.mem_write(a5+0xbfa,bytes.fromhex('4ef9')+struct.pack('>I',0x211fa))
box_hook=u.hook_add(UC_HOOK_CODE,capture_box_face,begin=a5+0x7c2,end=a5+0x7c2);u.ctl_flush_tb()
for selector,kind,start,end in [(1,0,0xc257e,0xc258c),(5,4,0xc2024,0xc2032)]:
    for color in range(256):
        u.mem_write(a5-0x2742+kind,bytes([color]))
        u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,0x56000)
        box_faces.clear();u.emu_start(start,end,count=2000);assert u.reg_read(UC_M68K_REG_PC)==end
        assert len(box_faces)==6
        cases.append(dict(routine='fixed_obstacle_faces',a=selector,b=color,kind=kind,faces=[v[0] for v in box_faces],colors=[v[1] for v in box_faces],expected=0))
u.hook_del(box_hook)
print('512 original box-caller and complete six-face builder blocks executed; all primary color bytes, original winding and constructor arguments captured')
column_faces=[]
def capture_column_face(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);words=struct.unpack('>10h',cpu.mem_read(sp+4,20))
    count=words[9];assert count in [4,8]
    column_faces.append(([words[8-i]-1 for i in range(count)],words[0]))
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+24);cpu.reg_write(UC_M68K_REG_PC,ret)
column_hook=u.hook_add(UC_HOOK_CODE,capture_column_face,begin=a5+0x7c2,end=a5+0x7c2);u.ctl_flush_tb()
for color in range(256):
    u.mem_write(a5-0x2740,bytes([color]));u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,0x56000)
    column_faces.clear();u.emu_start(0xc3472,0xc35d2,count=3000);assert u.reg_read(UC_M68K_REG_PC)==0xc35d2
    assert len(column_faces)==10
    cases.append(dict(routine='column_faces',a=color,b=0,faces=[v[0] for v in column_faces],colors=[v[1] for v in column_faces],expected=0))
u.hook_del(column_hook)
print('256 original column face-submission blocks executed; caps, eight sides, and all primary-color bytes captured')
# Original filled flag faces: both orientations use kind-twelve primary color.
flag_faces=[]
def capture_flag_face(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);words=struct.unpack('>10h',cpu.mem_read(sp+4,20))
    assert words[9]==4 and words[1:5]==(0,0,0,0)
    flag_faces.append(([words[8-i]-1 for i in range(4)],words[0]))
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    cpu.reg_write(UC_M68K_REG_A7,sp+24);cpu.reg_write(UC_M68K_REG_PC,ret)
flag_hook=u.hook_add(UC_HOOK_CODE,capture_flag_face,begin=a5+0x7c2,end=a5+0x7c2);u.ctl_flush_tb()
for color in range(256):
    u.mem_write(a5-0x2736,bytes([color]));u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,0x56000)
    flag_faces.clear();u.emu_start(0xc1072,0xc10f0,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0xc10f0
    assert len(flag_faces)==2
    cases.append(dict(routine='flag_faces',a=color,b=0,faces=[v[0] for v in flag_faces],colors=[v[1] for v in flag_faces],expected=0))
u.hook_del(flag_hook)
print('256 original filled-flag face-submission blocks executed; both orientations and all primary-color bytes captured')
# Complete edge insertion, including opposite-direction merges and first-match behavior.
u.ctl_flush_tb()
for iteration in range(1536):
    initial=[]
    for _ in range(rng.randrange(9)):
        initial.append(dict(vertices=[rng.randrange(-3,5),rng.randrange(-3,5)],visibility_refs=[rng.randint(-32768,32767),rng.randint(-32768,32767)],flag=rng.randrange(256)))
    vertices=[rng.randrange(-3,5),rng.randrange(-3,5)]
    if initial and iteration%2==0:vertices=initial[rng.randrange(len(initial))]['vertices'].copy()
    visibility_refs=[rng.randint(-32768,32767),rng.randint(-32768,32767)];flag=rng.randrange(256)
    u.mem_write(0x47000,bytes(2048));u.mem_write(a5-0x6490,struct.pack('>h',1));u.mem_write(a5-0x648e,struct.pack('>I',0x47000))
    u.mem_write(0x4725a,struct.pack('>h',len(initial)))
    for index,edge in enumerate(initial):
        u.mem_write(0x47266+index*10,struct.pack('>4hBB',*edge['vertices'],*edge['visibility_refs'],edge['flag'],0xa5))
    run(0x1005f2,bytes([flag,0])+struct.pack('>4h',*visibility_refs,*vertices))
    count=struct.unpack('>h',u.mem_read(0x4725a,2))[0]
    assert count in [len(initial),len(initial)+1]
    result=[]
    for index in range(count):
        values=struct.unpack('>4hB',u.mem_read(0x47266+index*10,9))
        result.append(dict(vertices=list(values[:2]),visibility_refs=list(values[2:4]),flag=values[4]))
    cases.append(dict(routine='insert_model_edge',a=flag,b=0,initial=initial,vertices=vertices,visibility_refs=visibility_refs,result=result,expected=0))
print('1536 complete original edge insertions executed; opposite-direction merges, duplicate order, and new records compared; padding excluded')
# Original edge selection paths, stopping after one projected record.
for iteration in range(1024):
    values=([0,1,2,255][(iteration//(4**i))%4] for i in range(4)) if iteration<256 else (rng.randrange(256) for _ in range(4))
    clipped,flag,first,second=values
    coordinates=[rng.randint(-32768,32767),rng.randint(-32768,32767)]
    if iteration%4==0:coordinates[1]=coordinates[0]
    for partitioned,start,end in [(False,0x100b60,0x100ba2),(True,0x100bde,0x100cb0)]:
        record=bytearray(16);struct.pack_into('>h',record,2,coordinates[0]);struct.pack_into('>h',record,6,coordinates[1])
        record[9]=clipped;struct.pack_into('>hh',record,10,0,1);record[14]=flag;record[15]=0xa5
        u.mem_write(0x43000,bytes(record));u.mem_write(0x44018,bytes([first]));u.mem_write(0x44028,bytes([second]))
        u.reg_write(UC_M68K_REG_A3,0x44000);u.reg_write(UC_M68K_REG_A4,0x43000);u.reg_write(UC_M68K_REG_A6,0x57000)
        u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D7,0)
        u.emu_start(start,end,count=500);assert u.reg_read(UC_M68K_REG_PC)==end
        queued=u.reg_read(UC_M68K_REG_D7);assert queued in [0,1]
        if queued:assert bytes(u.mem_read(0x57000-0x196,8))==bytes(record[:8])
        state=u.mem_read(0x4300f,1)[0]
        if not partitioned:assert state==0xa5
        cases.append(dict(routine='edge_scan',a=clipped,b=flag,partitioned=partitioned,visibility=[first,second],coordinates=coordinates,queued=bool(queued),state=state,expected=0))
print('2048 original edge scan blocks executed; clipping, visibility bytes, signed coordinate order, classifications, and queued endpoints compared')
# Complete edge dispatcher; capture only the final drawing service calls.
edge_batches=[]
def capture_edge_batch(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);color=cpu.mem_read(sp+4,1)[0]
    count,pointer=struct.unpack('>hI',cpu.mem_read(sp+6,6));assert 0<count<=50
    segments=[list(struct.unpack('>4h',cpu.mem_read(pointer+8*i,8))) for i in range(count)]
    edge_batches.append(dict(service=address-a5,color=color,segments=segments))
    ret=struct.unpack('>I',cpu.mem_read(sp,4))[0];cpu.reg_write(UC_M68K_REG_A7,sp+12);cpu.reg_write(UC_M68K_REG_PC,ret)
edge_hooks=[]
for service in [0x6d2,0x722,0x71a,0x70a,0x72a,0x712]:
    u.mem_write(a5+service,bytes.fromhex('4e75'));edge_hooks.append(u.hook_add(UC_HOOK_CODE,capture_edge_batch,begin=a5+service,end=a5+service))
u.ctl_flush_tb()
for iteration in range(1152):
    first=rng.randrange(1,13);count=rng.randrange(21);last=first+count-1
    flags=[rng.choice([0,1,2,255]) for _ in range(3)]+[rng.randrange(256)]
    if iteration>=1024:
        first=1;count=50;last=50;flags[0]=1
    edges=[];u.mem_write(0x44000,bytes(4096))
    for i in range(count):
        edge=dict(endpoints=[rng.randint(-32768,32767) for _ in range(4)],clipped=rng.choice([0,0,0,1,2,255]),flag=rng.choice([0,0,1,2,255]),visibility=[rng.choice([0,1,2,255]) for _ in range(2)],classification=rng.randrange(256))
        if iteration>=1024:
            edge['clipped']=0;edge['flag']=0;edge['visibility']=[1,0]
            pair=sorted([edge['endpoints'][1],edge['endpoints'][3]],reverse=bool(iteration%2))
            edge['endpoints'][1],edge['endpoints'][3]=pair
        if iteration%5==0:edge['endpoints']=[i,0,i,0]
        edges.append(edge)
        record=bytearray(16);struct.pack_into('>4h',record,0,*edge['endpoints']);record[9]=edge['clipped']
        struct.pack_into('>hh',record,10,60+2*i,61+2*i);record[14]=edge['flag'];record[15]=edge['classification']
        u.mem_write(0x44010+(first+i)*16,bytes(record))
        for j,value in enumerate(edge['visibility']):u.mem_write(0x44018+(60+2*i+j)*16,bytes([value]))
    edge_batches.clear()
    run(0x100b30,b''.join(bytes([value,0]) for value in flags)+struct.pack('>hhI',last,first,0x44000),instruction_limit=100000)
    classifications=[u.mem_read(0x4401f+(first+i)*16,1)[0] for i in range(count)]
    cases.append(dict(routine='edge_draw_batches',a=first,b=0,flags=flags,edges=edges,batches=edge_batches.copy(),classifications=classifications,expected=0))
for hook in edge_hooks:u.hook_del(hook)
print('1152 complete original edge dispatchers executed; final drawing services captured, all ordered endpoint batches and classifications compared')
# Original line palette setup, including four-bit color's special pattern.
u.mem_write(a5+0x1282,bytes.fromhex('4ef9')+struct.pack('>I',0x90004));u.ctl_flush_tb()
for iteration in range(512):
    depth=[4,8,16,32][iteration%4];four_bit_color=rng.choice([0,1,2,255])
    remapping=list(range(13)) if iteration<16 else [rng.randint(-32768,32767) for _ in range(13)]
    if iteration%3==0:remapping[rng.randrange(12)]=remapping[12]
    u.mem_write(a5-0x7526,struct.pack('>13h',*remapping));u.mem_write(a5-0x3fe4,struct.pack('>h',depth));u.mem_write(a5-0x6647,bytes([four_bit_color]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe149c,0xe1742,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe1742
    result=dict(high=list(struct.unpack('>13h',u.mem_read(a5-0x75be,26))),low=list(struct.unpack('>13h',u.mem_read(a5-0x75da,26))),patterns=list(struct.unpack('>13I',u.mem_read(a5-0x755e,52))))
    cases.append(dict(routine='line_palette',a=depth,b=four_bit_color,remapping=remapping,result=result,expected=0))
print('512 original line palette initializations executed; both word tables, all thirteen patterns, black aliases, and four-bit special pattern compared')
# Complete original viewport clipping and deferred raster-record construction.
u.ctl_flush_tb()
for iteration in range(2048):
    endpoints=[rng.randrange(-2048,2049) for _ in range(4)]
    if iteration%8==0:endpoints[2:]=endpoints[:2]
    viewport=[rng.randrange(1,1025),rng.randrange(1,769)];origin=[rng.randrange(-100,101),rng.randrange(-100,101)]
    stride=rng.choice([100,400,800,1024,-800]);depth=rng.choice([1,4,8]);base=rng.randrange(2**32)
    u.mem_write(a5-0x3fec,struct.pack('>h',viewport[0]));u.mem_write(a5-0x3fee,struct.pack('>h',viewport[1]))
    u.mem_write(a5-0x6bf2,struct.pack('>h',origin[0]));u.mem_write(a5-0x6bf4,struct.pack('>h',origin[1]))
    u.mem_write(a5-0x6bf0,struct.pack('>h',stride));u.mem_write(a5-0x6bee,struct.pack('>I',base));u.mem_write(a5-0x3fe4,struct.pack('>h',depth))
    initial=rng.randbytes(16);u.mem_write(0x43000,initial)
    run(0xe04ee,struct.pack('>I4h',0x43000,*endpoints[2:],*endpoints[:2]))
    accepted=u.mem_read(stack+16,1)[0];assert accepted in [0,1]
    result=list(u.mem_read(0x43000,16)) if accepted else None
    if not accepted:assert bytes(u.mem_read(0x43000,16))==initial
    cases.append(dict(routine='deferred_line_record',a=stride,b=depth,endpoints=endpoints,viewport=viewport,origin=origin,base=base,result=result,expected=0))
print('2048 complete original deferred line constructors executed; viewport clipping, rejection, and all sixteen raster-record bytes compared')
# Original clipper plus complete eight-bit line writer; compare every framebuffer byte.
u.mem_map(0x1b0000,0x10000);u.mem_write(0x1b0000,(source/'10.bin').read_bytes());u.ctl_flush_tb()
for iteration in range(512):
    endpoints=[[rng.randrange(-32,97),rng.randrange(-32,97),rng.randrange(-32,97),rng.randrange(-32,97)] for _ in range(rng.randrange(9))]
    if iteration%8==0:endpoints += [[0,0,47,63],[47,0,0,63],[24,0,24,63],[0,32,47,32],[24,32,24,32]]
    initial=rng.randbytes(64*48);pattern=rng.getrandbits(32)
    u.mem_write(0x45000,initial);u.mem_write(a5-0x3fec,struct.pack('>h',64));u.mem_write(a5-0x3fee,struct.pack('>h',48))
    u.mem_write(a5-0x6bf2,bytes(2));u.mem_write(a5-0x6bf4,bytes(2));u.mem_write(a5-0x6bf0,struct.pack('>h',64))
    u.mem_write(a5-0x6bee,struct.pack('>I',0x45000));u.mem_write(a5-0x3fe4,struct.pack('>h',8))
    records=[]
    for points in endpoints:
        destination=0x43000+16*len(records)
        run(0xe04ee,struct.pack('>I4h',destination,*points[2:],*points[:2]))
        if u.mem_read(stack+16,1)[0]:records.append(list(u.mem_read(destination,16)))
    run(0x1b0440,struct.pack('>IhI',pattern,len(records),0x43000),instruction_limit=100000)
    for index,record in enumerate(records):assert list(u.mem_read(0x43000+16*index,16))==record
    cases.append(dict(routine='deferred_lines_8',a=pattern,b=0,endpoints=endpoints,records=records,initial=list(initial),result=list(u.mem_read(0x45000,64*48)),expected=0))
print('512 complete original eight-bit line batches executed with original clipping; all framebuffer bytes and preserved records compared, no service fixtures')
# Complete deferred phase mask builders and their original thirteen-color setup loop.
u.ctl_flush_tb()
for iteration in range(1024):
    depth=[4,8][iteration%2];phase=rng.choice([0,1,2,255]);mapped=rng.randint(-32768,32767)
    u.mem_write(0x43000,rng.randbytes(4))
    run(0xe1f88 if depth==4 else 0xe1fdc,struct.pack('>IBBh',0x43000,phase,0,mapped))
    result=struct.unpack('>I',u.mem_read(0x43000,4))[0]
    cases.append(dict(routine='deferred_line_mask',a=depth,b=mapped,phase=phase,result=result,expected=0))
for iteration in range(512):
    depth=[4,8][iteration%2];remapping=list(range(13)) if iteration<4 else [rng.randint(-32768,32767) for _ in range(13)]
    u.mem_write(a5-0x7526,struct.pack('>13h',*remapping));u.mem_write(a5-0x3fe4,struct.pack('>h',depth))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe161a,0xe1742,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe1742
    result=dict(high=list(struct.unpack('>13h',u.mem_read(a5-0x75be,26))),low=list(struct.unpack('>13h',u.mem_read(a5-0x75da,26))),phase_zero=list(struct.unpack('>13I',u.mem_read(a5-0x7612,52))),phase_one=list(struct.unpack('>13I',u.mem_read(a5-0x764a,52))))
    cases.append(dict(routine='deferred_line_palette',a=depth,b=0,remapping=remapping,result=result,expected=0))
print('1024 complete original phase mask builders and 512 original deferred palette loops executed; both phases and both supporting tables compared')
# Original transposed RGB tables and actual frame-parity table selection.
# The new stop boundary was inside the preceding palette-loop translation.
u.ctl_flush_tb()
def original_deferred_rgb(base):
    count=len(base)
    u.mem_write(0x57000-0x494,b''.join(struct.pack('>3H',*rgb) for rgb in base))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000);u.reg_write(UC_M68K_REG_D5,count)
    u.emu_start(0xe1596,0xe161a,count=20000);assert u.reg_read(UC_M68K_REG_PC)==0xe161a
    return [[list(rgb) for rgb in struct.iter_unpack('>3H',bytes(u.mem_read(address,count*count*6)))] for address in [a5-0x708a,a5-0x750a]]
def original_phase_rgb(frame,count):
    u.mem_write(a5-0x6c08,struct.pack('>i',frame));u.mem_write(a5-0x6bea,struct.pack('>h',count*count))
    run(0xe1816,b'')
    pointer=struct.unpack('>I',u.mem_read(a5-0x7656,4))[0]
    assert pointer in [a5-0x708a,a5-0x750a] and u.mem_read(a5-0x7658,1)==b'\x01'
    assert struct.unpack('>h',u.mem_read(a5-0x7652,2))[0]==count*count
    return [list(rgb) for rgb in struct.iter_unpack('>3H',bytes(u.mem_read(pointer,count*count*6)))]
for iteration in range(256):
    count=[4,8][iteration%2];base=[[rng.randrange(65536) for _ in range(3)] for _ in range(count)]
    result=original_deferred_rgb(base);frame=rng.randint(-2**31,2**31-1);selected=original_phase_rgb(frame,count)
    cases.append(dict(routine='deferred_rgb_palettes',a=frame,b=count,base=base,result=result,selected=selected,expected=0))
# Compose both original phases: mask construction, clipping, pixel writing, and RGB selection.
for iteration in range(128):
    base=[[rng.randrange(65536) for _ in range(3)] for _ in range(8)];original_deferred_rgb(base)
    phases=[[[[rng.randrange(-4,21) for _ in range(4)],rng.randrange(8)] for _ in range(6)] for _ in range(2)]
    u.mem_write(0x45000,bytes([63])*(16*12));results=[]
    u.mem_write(a5-0x3fec,struct.pack('>h',16));u.mem_write(a5-0x3fee,struct.pack('>h',12));u.mem_write(a5-0x3fe4,struct.pack('>h',8))
    u.mem_write(a5-0x6bf2,bytes(2));u.mem_write(a5-0x6bf4,bytes(2));u.mem_write(a5-0x6bf0,struct.pack('>h',16));u.mem_write(a5-0x6bee,struct.pack('>I',0x45000))
    for phase,lines in enumerate(phases):
        for points,mapped in lines:
            run(0xe1fdc,struct.pack('>IBBh',0x43100,phase,0,mapped));pattern=struct.unpack('>I',u.mem_read(0x43100,4))[0]
            run(0xe04ee,struct.pack('>I4h',0x43000,*points[2:],*points[:2]))
            if u.mem_read(stack+16,1)[0]:run(0x1b0440,struct.pack('>IhI',pattern,1,0x43000))
        palette=original_phase_rgb(phase,8);pixels=list(u.mem_read(0x45000,16*12))
        results.append(dict(pixels=pixels,rgb=[palette[p] for p in pixels]))
    cases.append(dict(routine='deferred_phase_pixels',a=0,b=0,base=base,phases=phases,results=results,expected=0))
print('256 original RGB table/phase selections and 128 composed two-phase framebuffer sequences executed; indexed pixels and RGB words compared')
# Complete eight-bit color-group paint and erase, including both direct-pixel lists.
for displacement,target in [(0x642,0x1b0440),(0x64a,0x1b04b4)]:u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(256):
    phase=rng.choice([0,1,2,255]);remapping=[rng.randrange(8) for _ in range(13)]
    u.mem_write(a5-0x7526,struct.pack('>13h',*remapping));u.mem_write(a5-0x3fe4,struct.pack('>h',8))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe161a,0xe1742,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe1742
    u.mem_write(a5-0x3fec,struct.pack('>h',16));u.mem_write(a5-0x3fee,struct.pack('>h',12))
    u.mem_write(a5-0x6bf2,bytes(2));u.mem_write(a5-0x6bf4,bytes(2));u.mem_write(a5-0x6bf0,struct.pack('>h',16));u.mem_write(a5-0x6bee,struct.pack('>I',0x45000))
    queue_header=bytearray(338);batches=[]
    for color in range(13):
        lines=[]
        for _ in range(rng.randrange(4)):
            points=[rng.randrange(-4,21) for _ in range(4)];destination=0x43000+color*256+len(lines)*16
            run(0xe04ee,struct.pack('>I4h',destination,*points[2:],*points[:2]))
            if u.mem_read(stack+16,1)[0]:lines.append(list(u.mem_read(destination,16)))
        struct.pack_into('>hhI',queue_header,2+color*8,len(lines),len(lines),0x43000+color*256)
        pixels=[]
        for lane,offset in enumerate([0x72,0xe2]):
            addresses=[0x45000+rng.randrange(16*12) for _ in range(rng.randrange(5))];pointer=0x47000+lane*0x2000+color*256
            if addresses:u.mem_write(pointer,struct.pack('>'+str(len(addresses))+'I',*addresses))
            struct.pack_into('>hhI',queue_header,offset+color*8,len(addresses),len(addresses),pointer);pixels.append(addresses)
        batches.append(dict(lines=lines,pixels=pixels))
    u.mem_write(0x42000,bytes(queue_header));initial=bytes(rng.randrange(64) for _ in range(16*12));u.mem_write(0x45000,initial)
    run(0xe37a6,struct.pack('>BBI',phase,0,0x42000),instruction_limit=100000)
    drawn=list(u.mem_read(0x45000,16*12));assert bytes(u.mem_read(0x42000,338))==bytes(queue_header)
    run(0xe388c,struct.pack('>BBI',phase,0,0x42000),instruction_limit=100000)
    erased=list(u.mem_read(0x45000,16*12));assert bytes(u.mem_read(0x42000,338))==bytes(queue_header)
    cases.append(dict(routine='deferred_frame_8',a=phase,b=0,remapping=remapping,batches=batches,initial=list(initial),drawn=drawn,erased=erased,expected=0))
print('256 complete original eight-bit paint/erase sequences executed; all thirteen groups, lines, both pixel lists, and full framebuffer bytes compared')
# Complete original deferred queue appends, followed by complete painting and erasure.
u.mem_map(0x1c0000,0x10000);u.ctl_flush_tb()
for iteration in range(256):
    phase=rng.choice([0,1,2,255]);remapping=[rng.randrange(8) for _ in range(13)]
    u.mem_write(a5-0x7526,struct.pack('>13h',*remapping));u.mem_write(a5-0x3fe4,struct.pack('>h',8))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe161a,0xe1742,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe1742
    u.mem_write(a5-0x3fec,struct.pack('>h',16));u.mem_write(a5-0x3fee,struct.pack('>h',12));u.mem_write(a5-0x7be2,b'\0')
    u.mem_write(a5-0x6bf2,bytes(2));u.mem_write(a5-0x6bf4,bytes(2));u.mem_write(a5-0x6bf0,struct.pack('>h',16));u.mem_write(a5-0x6bee,struct.pack('>I',0x45000))
    queue_header=bytearray(338)
    for color in range(13):struct.pack_into('>hhI',queue_header,2+8*color,0,128,0x1c0000+color*4096)
    u.mem_write(a5-0x67ea,bytes(queue_header));requests=[];counts=[]
    for request in range(8):
        color=rng.randrange(13);lines=[[rng.randrange(-8,25) for _ in range(4)] for _ in range(rng.randrange(9))]
        if iteration%8==0 and request==0:lines=[[0,0,11,15],[-8,-8,-4,-4],[6,0,6,15]]
        requests.append([color,lines])
        if lines:u.mem_write(0x43000,b''.join(struct.pack('>4h',*points) for points in lines))
        run(0xe20a4,struct.pack('>BBhI',color,0,len(lines),0x43000),instruction_limit=100000)
        assert u.mem_read(a5-0x67ea,1)==b'\x01'
        counts.append([struct.unpack('>h',u.mem_read(a5-0x67e8+8*i,2))[0] for i in range(13)])
    colors=[]
    for color,count in enumerate(counts[-1]):
        assert u.mem_read(a5-0x67e6+8*color,2)==struct.pack('>h',128)
        colors.append(dict(lines=[list(u.mem_read(0x1c0000+color*4096+16*i,16)) for i in range(count)],pixels=[[],[]]))
    u.mem_write(0x45000,bytes([63])*(16*12))
    run(0xe37a6,struct.pack('>BBI',phase,0,a5-0x67ea),instruction_limit=100000);drawn=list(u.mem_read(0x45000,16*12))
    run(0xe388c,struct.pack('>BBI',phase,0,a5-0x67ea),instruction_limit=100000);erased=list(u.mem_read(0x45000,16*12))
    cases.append(dict(routine='deferred_queue_8',a=phase,b=0,remapping=remapping,requests=requests,counts=counts,queue=dict(dirty=True,colors=colors),drawn=drawn,erased=erased,expected=0))
print('256 original queue/paint/erase sequences executed; eight appends each, dirty state, compacted records, counts, and framebuffer bytes compared; capacity preallocated')
# Queue reset/merge and three-header rotation. Only the Macintosh BlockMove trap is modeled.
def queue_block_move(cpu,address,size,data):
    source_ptr=cpu.reg_read(UC_M68K_REG_A0);destination_ptr=cpu.reg_read(UC_M68K_REG_A1);length=cpu.reg_read(UC_M68K_REG_D0)
    assert length<=256
    cpu.mem_write(destination_ptr,bytes(cpu.mem_read(source_ptr,length)));cpu.reg_write(UC_M68K_REG_PC,address+2)
u.mem_write(a5+0x192,bytes.fromhex('4ef9')+struct.pack('>I',0x122216))
queue_move_hook=u.hook_add(UC_HOOK_CODE,queue_block_move,begin=0x12221e,end=0x12221e);u.ctl_flush_tb()
def make_queue(header_address,data_address):
    header=bytearray(338);header[0]=rng.choice([0,1,2,255]);colors=[]
    for color in range(13):
        lines=[list(rng.randbytes(16)) for _ in range(rng.randrange(4))]
        pixels=[[rng.getrandbits(32) for _ in range(rng.randrange(4))] for _ in range(2)]
        for lane,(offset,items,width) in enumerate([(2,lines,16),(0x72,pixels[0],4),(0xe2,pixels[1],4)]):
            pointer=data_address+(color*3+lane)*256
            payload=b''.join(bytes(item) if width==16 else struct.pack('>I',item) for item in items)
            u.mem_write(pointer,payload+bytes(256-len(payload)))
            struct.pack_into('>hhI',header,offset+8*color,len(items),16,pointer)
        colors.append(dict(lines=lines,pixels=pixels))
    u.mem_write(header_address,bytes(header))
    return dict(dirty=bool(header[0]),colors=colors),bytes(header)
def read_queue(header_address):
    header=bytes(u.mem_read(header_address,338));colors=[]
    for color in range(13):
        lists=[]
        for offset,width in [(2,16),(0x72,4),(0xe2,4)]:
            count,capacity,pointer=struct.unpack_from('>hhI',header,offset+8*color);assert capacity==16 and 0<=count<=6
            lists.append([list(u.mem_read(pointer+i*16,16)) if width==16 else struct.unpack('>I',u.mem_read(pointer+i*4,4))[0] for i in range(count)])
        colors.append(dict(lines=lists[0],pixels=lists[1:]))
    return dict(dirty=bool(header[0]),colors=colors)
for iteration in range(256):
    destination,destination_header=make_queue(0x42000,0x1c0000);source_queue,source_header=make_queue(0x42200,0x1c8000)
    run(0xe0896,struct.pack('>II',0x42000,0x42200),instruction_limit=100000)
    merged=read_queue(0x42000);assert bytes(u.mem_read(0x42200,338))==source_header
    run(0xe0840,struct.pack('>I',0x42000));cleared=read_queue(0x42000)
    cases.append(dict(routine='deferred_queue_merge_clear',a=0,b=0,destination=destination,source_queue=source_queue,merged=merged,cleared=cleared,expected=0))
u.hook_del(queue_move_hook)
for iteration in range(256):
    before=[list(rng.randbytes(338)) for _ in range(3)]
    for address,header in zip([a5-0x6be0,a5-0x6a8e,a5-0x67ea],before):u.mem_write(address,bytes(header))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe3ae8,0xe3b30,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe3b30
    after=[list(u.mem_read(address,338)) for address in [a5-0x6be0,a5-0x6a8e,a5-0x67ea]]
    cases.append(dict(routine='deferred_queue_rotation',a=0,b=0,before=before,after=after,expected=0))
print('256 complete queue merges/resets with modeled BlockMove and 256 original three-header rotations executed; full logical lists and header bytes compared')
# Complete deferred eight-bit frame presentation, with palette-ready and preallocated queues.
u.mem_map(0x1d0000,0x10000)
queue_move_hook=u.hook_add(UC_HOOK_CODE,queue_block_move,begin=0x12221e,end=0x12221e);u.ctl_flush_tb()
frame_queue_addresses=[a5-0x6be0,a5-0x6a8e,a5-0x67ea,a5-0x693c]
for iteration in range(128):
    remapping=[rng.randrange(8) for _ in range(13)];frame=rng.choice([0,1,-1,2**31-1,-2**31,rng.randint(-2**31,2**31-1)])
    u.mem_write(a5-0x7526,struct.pack('>13h',*remapping));u.mem_write(a5-0x3fe4,struct.pack('>h',8))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A6,0x57000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0xe161a,0xe1742,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0xe1742
    u.mem_write(a5-0x3fec,struct.pack('>h',16));u.mem_write(a5-0x3fee,struct.pack('>h',12))
    u.mem_write(a5-0x6bf2,bytes(2));u.mem_write(a5-0x6bf4,bytes(2));u.mem_write(a5-0x6bf0,struct.pack('>h',16));u.mem_write(a5-0x6bee,struct.pack('>I',0x45000))
    initial_queues=[]
    for slot,header_address in enumerate(frame_queue_addresses):
        queue_header=bytearray(338);queue_header[0]=rng.choice([0,1,2,255])
        for color in range(13):
            lines=[];pointer=0x1c0000+slot*0x8000+color*768
            for _ in range(rng.randrange(3)):
                points=[rng.randrange(-4,21) for _ in range(4)]
                run(0xe04ee,struct.pack('>I4h',pointer+len(lines)*16,*points[2:],*points[:2]))
                if u.mem_read(stack+16,1)[0]:lines.append(1)
            struct.pack_into('>hhI',queue_header,2+color*8,len(lines),16,pointer)
            for lane,offset in enumerate([0x72,0xe2]):
                addresses=[0x45000+rng.randrange(192) for _ in range(rng.randrange(3))];pixel_pointer=pointer+256+lane*256
                if addresses:u.mem_write(pixel_pointer,struct.pack('>'+str(len(addresses))+'I',*addresses))
                struct.pack_into('>hhI',queue_header,offset+color*8,len(addresses),16,pixel_pointer)
        u.mem_write(header_address,bytes(queue_header));initial_queues.append(read_queue(header_address))
    initial_pixels=bytes(rng.randrange(64) for _ in range(192));u.mem_write(0x45000,initial_pixels)
    u.mem_write(a5-0x6c08,struct.pack('>i',frame));u.mem_write(a5-0x7658,b'\0');u.mem_write(a5-0x7be2,b'\0');u.mem_write(a5-0x7b98,b'\0');u.mem_write(a5-0x6bea,struct.pack('>h',64))
    u.mem_write(a5-0x6c09,b'\xff');u.mem_write(a5-0x6be5,b'\xff')
    run(0xe3a78,b'',instruction_limit=500000)
    result_frame=struct.unpack('>i',u.mem_read(a5-0x6c08,4))[0]
    selected=struct.unpack('>I',u.mem_read(a5-0x7656,4))[0];assert selected==a5-(0x750a if result_frame&1 else 0x708a)
    assert u.mem_read(a5-0x6c09,1)==b'\0' and u.mem_read(a5-0x6be5,1)==b'\0'
    cases.append(dict(routine='deferred_frame_cycle',a=0,b=0,remapping=remapping,initial_state=dict(frame=frame,queues=initial_queues[:3],auxiliary=initial_queues[3],palette_pending=False),initial_pixels=list(initial_pixels),result_state=dict(frame=result_frame,queues=[read_queue(address) for address in frame_queue_addresses[:3]],auxiliary=read_queue(frame_queue_addresses[3]),palette_pending=bool(u.mem_read(a5-0x7658,1)[0])),result_pixels=list(u.mem_read(0x45000,192)),expected=0))
for hook in [queue_move_hook]:u.hook_del(hook)
print('128 complete original deferred frame cycles executed; erase, paint, counter wrap, palette publication, rotation, auxiliary merge/reset, and pixels compared; BlockMove modeled')
# Complete original single-vertex projector and packed matrix-transform/project pipeline.
u.ctl_flush_tb()
for iteration in range(2048):
    depth=rng.choice([0,1,9,10,11,-1,-32768,-32759,-32758,32767,rng.randint(-32768,32767)])
    initial=[rng.randint(-32768,32767),rng.randint(-32768,32767),depth,rng.randint(-32768,32767),rng.randint(-32768,32767)]
    u.mem_write(0x43000,struct.pack('>5h',*initial));run(0xe3b7e,struct.pack('>I',0x43000))
    result=list(struct.unpack('>5h',u.mem_read(0x43000,10)))
    cases.append(dict(routine='project_vertex_record',a=0,b=0,initial=initial,result=result,expected=0))
for iteration in range(512):
    count=rng.choice([0,1,2,8,60]);vertices=[[rng.randint(-32768,32767) for _ in range(3)] for _ in range(count)]
    rows=[[rng.randint(-32768,32767) for _ in range(3)] for _ in range(3)]
    if iteration%4==0:rows=[[16384,0,0],[0,16384,0],[0,0,16384]]
    translation=[rng.randint(-32768,32767) for _ in range(3)];initial=[[rng.randint(-32768,32767) for _ in range(5)] for _ in range(count)]
    packed=struct.pack('>h',count)+b''.join(struct.pack('>3h',*vertex)+rng.randbytes(4) for vertex in vertices)
    u.mem_write(0x43000,packed);u.mem_write(0x44000,struct.pack('>h',-1)+b''.join(struct.pack('>5h',*record) for record in initial))
    for address,row in zip([0x46000,0x46010,0x46020],rows):u.mem_write(address,struct.pack('>3h',*row))
    u.mem_write(0x46030,struct.pack('>3h',*translation))
    run(0xe3ba8,struct.pack('>6I',0x44000,0x43000,0x46020,0x46010,0x46000,0x46030),instruction_limit=100000)
    assert u.mem_read(0x44000,2)==struct.pack('>h',count) and bytes(u.mem_read(0x43000,len(packed)))==packed
    result=[list(struct.unpack('>5h',u.mem_read(0x44002+i*10,10))) for i in range(count)]
    cases.append(dict(routine='transform_project_vertices',a=0,b=0,vertices=vertices,rows=rows,translation=translation,initial=initial,result=result,expected=0))
print('2048 complete original vertex projectors and 512 complete matrix-transform/project pipelines executed; wrapped depth gates, DIVS overflow, preserved words, and full outputs compared')
# Original render-record matrix setup, including the complete direction lookup.
u.ctl_flush_tb()
for iteration in range(1024):
    direction_values=[rng.randint(-2**31,2**31-1) for _ in range(360)]
    position=[rng.randint(-32768,32767) for _ in range(3)]
    camera_height=rng.randint(-32768,32767)
    heading=rng.choice([-2**31,2**31-1,-1,0,90<<16,359<<16,360<<16,rng.randint(-2**31,2**31-1)])
    record=bytearray(64)
    struct.pack_into('>3h',record,6,position[2],position[0],position[1])
    struct.pack_into('>i',record,0x18,heading)
    u.mem_write(0x43000,bytes(record));u.mem_write(0x44000,struct.pack('>360i',*direction_values))
    u.mem_write(a5-0x6246,struct.pack('>I',0x44000));u.mem_write(0x57000-2,struct.pack('>h',camera_height))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    u.emu_start(0x101af6,0x101b56,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x101b56
    translation=list(struct.unpack('>3h',u.mem_read(0x57000-0x32,6)))
    rows=[list(struct.unpack('>3h',u.mem_read(0x57000-0x2c+i*6,6))) for i in range(3)]
    assert bytes(u.mem_read(0x43000,64))==record
    cases.append(dict(routine='model_view_transform',a=0,b=0,position=position,camera_height=camera_height,heading=heading,table=direction_values,rows=rows,translation=translation,expected=0))
print('1024 original render-record matrix setups executed with complete direction lookup; all translation and rotation words compared')
# Original camera-relative planar conversion before object visibility selection.
u.ctl_flush_tb()
for iteration in range(2048):
    boundaries=[-2**31,2**31-1,-1025,-1024,-1023,-1,0,1,1023,1024,1025]
    delta=[rng.choice(boundaries) if iteration%4==0 else rng.randint(-2**31,2**31-1) for _ in range(2)]
    basis=[rng.randint(-32768,32767) for _ in range(2)]
    if iteration%8==0:basis=rng.choice([[16384,0],[0,16384],[-16384,0],[0,-16384]])
    u.mem_write(0x57000-0x22,struct.pack('>2i',*delta))
    u.mem_write(0x57000+0x10,struct.pack('>h',basis[0]));u.mem_write(0x57000+0x14,struct.pack('>h',basis[1]))
    for reg,value in [(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    u.emu_start(0x71af2,0x71b6a,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x71b6a
    result=[struct.unpack('>h',struct.pack('>H',u.reg_read(reg)&0xffff))[0] for reg in [UC_M68K_REG_D5,UC_M68K_REG_D7]]
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    cases.append(dict(routine='render_record_planar',a=0,b=0,delta=delta,basis=basis,result=result,expected=0))
print('2048 original camera-relative planar transforms executed without fixtures; both render-record coordinates and balanced stack checked')
# Complete original indexed face clipper, its interpolation, and point projector.
for entry,address in [(0x742,0xe3b58),(0x74a,0xe3b7e)]:u.mem_write(a5+entry,bytes.fromhex('4ef9')+struct.pack('>I',address))
u.ctl_flush_tb()
for iteration in range(2048):
    count=rng.choice([1,2,3,4,8,20]);vertex_count=rng.randrange(count,61)
    vertices=[[rng.randint(-32768,32767),rng.randint(-32768,32767),rng.choice([-32768,-1,0,15,16,17,32767,rng.randint(-32768,32767)]),rng.randint(-32768,32767),rng.randint(-32768,32767)] for _ in range(vertex_count)]
    indices=[rng.randrange(vertex_count) for _ in range(count)]
    origin=[rng.randint(-32768,32767) for _ in range(2)];offset=[rng.randint(-32768,32767) for _ in range(2)]
    packed=struct.pack('>h',vertex_count)+b''.join(struct.pack('>5h',*v) for v in vertices)
    u.mem_write(0x43000,packed);u.mem_write(0x44000,struct.pack('>'+str(count)+'h',*[i+1 for i in indices]));u.mem_write(0x45000,struct.pack('>h',count))
    u.mem_write(0x46000,b'\xa5'*512);u.mem_write(a5-0x62fc,struct.pack('>h',origin[0]));u.mem_write(a5-0x62fe,struct.pack('>h',origin[1]))
    run(0x1012ca,struct.pack('>2h4I',offset[1],offset[0],0x46000,0x44000,0x43000,0x45000),instruction_limit=100000)
    result_count=struct.unpack('>h',u.mem_read(0x45000,2))[0];assert 0<=result_count<=count*2
    result=[list(reversed(struct.unpack('>2h',u.mem_read(0x46000+i*4,4)))) for i in range(result_count)]
    assert bytes(u.mem_read(0x43000,len(packed)))==packed
    assert bytes(u.mem_read(0x46000+result_count*4,512-result_count*4))==b'\xa5'*(512-result_count*4)
    cases.append(dict(routine='clip_project_face',a=0,b=0,vertices=vertices,indices=indices,origin=origin,offset=offset,result=result,expected=0))
print('2048 complete original indexed face clippers executed with original interpolation and projection; full screen coordinates, counts, source and output guards checked')
# Original horizontal origin gate; stop both branches at their block exits.
def origin_gate_exit(uc,address,size,data):
    uc.reg_write(UC_M68K_REG_D4,0)
    uc.reg_write(UC_M68K_REG_PC,0x7283c)
origin_gate_hook=u.hook_add(UC_HOOK_CODE,origin_gate_exit,begin=0x72a52,end=0x72a52)
u.ctl_flush_tb()
for iteration in range(1024):
    horizontal=rng.randint(-32768,32767);depth=rng.randint(-32768,32767);full_size=bool(iteration%2)
    u.mem_write(a5-0x7bf0,bytes([int(full_size)]))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_D5,horizontal&0xffff),(UC_M68K_REG_D7,depth&0xffff),(UC_M68K_REG_D4,1)]:u.reg_write(reg,value)
    u.emu_start(0x727fe,0x7283c,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x7283c
    cases.append(dict(routine='solid_origin_visible',a=horizontal,b=depth,full_size=full_size,result=bool(u.reg_read(UC_M68K_REG_D4)),expected=0))
u.hook_del(origin_gate_hook)
print('1024 original horizontal object-origin gates executed; both display modes and signed boundaries compared')
# Original model edge clipping block, retaining complete endpoint records.
for entry,address in [(0x742,0xe3b58),(0x74a,0xe3b7e)]:u.mem_write(a5+entry,bytes.fromhex('4ef9')+struct.pack('>I',address))
def edge_clip_exit(uc,address,size,data):
    uc.reg_write(UC_M68K_REG_D4,0);uc.reg_write(UC_M68K_REG_PC,0x101128)
edge_clip_hook=u.hook_add(UC_HOOK_CODE,edge_clip_exit,begin=0x101194,end=0x101194)
u.ctl_flush_tb()
for iteration in range(2048):
    edge=[[rng.randint(-32768,32767),rng.randint(-32768,32767),rng.choice([-32768,-1,0,15,16,17,32767,rng.randint(-32768,32767)]),rng.randint(-32768,32767),rng.randint(-32768,32767)] for _ in range(2)]
    u.mem_write(0x57000-0x54c,b''.join(struct.pack('>5h',*v) for v in edge));u.mem_write(0x43000,bytes([0xa5])*16)
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_D4,1)]:u.reg_write(reg,value)
    u.emu_start(0x10106c,0x101128,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x101128
    accepted=bool(u.reg_read(UC_M68K_REG_D4))
    result=[list(struct.unpack('>5h',u.mem_read(0x57000-0x54c+i*10,10))) for i in range(2)] if accepted else None
    if not accepted:assert bytes(u.mem_read(0x43008,2))==bytes([0,1])
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    cases.append(dict(routine='clip_edge_records',a=0,b=0,edge=edge,result=result,expected=0))
u.hook_del(edge_clip_hook)
print('2048 original model edge clipping blocks executed with original interpolation and projection; complete endpoint records and rejection flags checked')
# Original suppression/depth/cached-distance gates; terminal branch hooks only.
def distance_gate_exit(uc,address,size,data):
    uc.reg_write(UC_M68K_REG_D6,0);uc.reg_write(UC_M68K_REG_PC,0x72880)
distance_gate_hook=u.hook_add(UC_HOOK_CODE,distance_gate_exit,begin=0x72a52,end=0x72a52)
u.ctl_flush_tb()
for iteration in range(2048):
    horizontal=rng.choice([-32768,32767,0,2399,2400,rng.randint(-32768,32767)])
    depth=rng.choice([-32768,-1,0,2399,2400,2401,32767,rng.randint(-32768,32767)])
    cached=rng.choice([-1,-1,0,86,87,88,-32768,32767,rng.randint(-32768,32767)]);suppressed=rng.choice([0,0,0,1,2,255])
    u.mem_write(0x57000-0x1a2b,bytes([suppressed]))
    for reg,value in [(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D5,horizontal&0xffff),(UC_M68K_REG_D7,depth&0xffff),(UC_M68K_REG_D4,cached&0xffff),(UC_M68K_REG_D6,1)]:u.reg_write(reg,value)
    u.emu_start(0x7283c,0x72880,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x72880 and u.reg_read(UC_M68K_REG_A7)==0x56000
    result=[bool(u.reg_read(UC_M68K_REG_D6)),struct.unpack('>h',struct.pack('>H',u.reg_read(UC_M68K_REG_D4)&0xffff))[0]]
    cases.append(dict(routine='solid_distance_visible',a=horizontal,b=depth,suppressed=suppressed,cached=cached,result=result,expected=0))
u.hook_del(distance_gate_hook)
print('2048 original suppression/depth/cached-distance gates executed; admission, preserved/computed cache, signed boundaries, and stack checked')
# Complete original face-plane evaluation and visibility wrapper.
u.ctl_flush_tb()
for iteration in range(2048):
    plane=bytearray(rng.randbytes(12));point=[rng.randint(-32768,32767) for _ in range(3)]
    if iteration%4!=0:
        dot=sum(struct.unpack_from('>h',plane,offset)[0]*coordinate for offset,coordinate in zip([0,8,2],point))
        struct.pack_into('>I',plane,4,(-dot+[-1,0,1][iteration%4-1])&0xffffffff)
    u.mem_write(0x43000,bytes(plane));args=struct.pack('>3hI',*reversed(point),0x43000)
    run(0x100aa4,args);result=struct.unpack('>i',u.mem_read(stack+4+len(args),4))[0]
    run(0x100afe,args);visible=bool(u.mem_read(stack+4+len(args),1)[0])
    assert bytes(u.mem_read(0x43000,12))==plane
    cases.append(dict(routine='face_plane_visibility',a=0,b=0,plane=list(plane),point=point,result=result,visible=visible,expected=0))
print('2048 complete original face-plane evaluations and visibility wrappers executed; wrapped sums and negative/zero/positive boundaries compared')
# Original inverse world offset and model-heading rotation for face visibility.
u.ctl_flush_tb()
for iteration in range(1024):
    delta=[rng.randint(-2**31,2**31-1) for _ in range(2)];heading=rng.randint(-2**31,2**31-1)
    direction_values=[rng.randint(-2**31,2**31-1) for _ in range(360)]
    camera_height=rng.randint(-32768,32767);object_height=rng.randint(-32768,32767)
    record=bytearray(64);struct.pack_into('>3i',record,0xc,*delta,heading)
    u.mem_write(0x43000,bytes(record));u.mem_write(0x44000,bytes(32));u.mem_write(0x45000,struct.pack('>360i',*direction_values))
    u.mem_write(a5-0x6246,struct.pack('>I',0x45000))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_A3,0x44000)]:u.reg_write(reg,value)
    u.emu_start(0x101c30,0x101c50,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x101c50
    u.emu_start(0x101cec,0x101d56,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x101d56
    x,z=struct.unpack('>2h',u.mem_read(0x4400e,4))
    # Original caller's height subtraction at +0x17d4..+0x17dc.
    u.mem_write(0x57018,struct.pack('>h',camera_height));u.mem_write(0x5701c,struct.pack('>h',object_height))
    u.emu_start(0x1017d4,0x1017dc,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1017dc
    y=struct.unpack('>h',struct.pack('>H',u.reg_read(UC_M68K_REG_D0)&0xffff))[0]
    assert u.reg_read(UC_M68K_REG_A7)==0x56000 and bytes(u.mem_read(0x43000,64))==record
    cases.append(dict(routine='face_viewpoint',a=0,b=0,delta=delta,heading=heading,table=direction_values,camera_height=camera_height,object_height=object_height,result=[x,y,z],expected=0))
print('1024 original face-viewpoint conversions executed with complete direction lookup and original height subtraction; no service fixtures')
# Complete horizon routine; model OffsetRect and capture final rectangle drawing requests.
horizon_draws=[]
def horizon_services(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    if address in [0x74a9a,0x74af4,0x74b28]:
        dy,dx,pointer=struct.unpack('>hhI',uc.mem_read(sp,8))
        rect=struct.unpack('>4h',uc.mem_read(pointer,8))
        uc.mem_write(pointer,struct.pack('>4H',*[(v+(dy if i%2==0 else dx))&0xffff for i,v in enumerate(rect)]))
        uc.reg_write(UC_M68K_REG_A7,sp+8);uc.reg_write(UC_M68K_REG_PC,address+2)
    elif address==a5+0x6ea:
        ret,shade,color,pointer=struct.unpack('>IhhI',uc.mem_read(sp,12))
        horizon_draws.append(list(struct.unpack('>4h',uc.mem_read(pointer,8)))+[color,shade])
        uc.reg_write(UC_M68K_REG_A7,sp+12);uc.reg_write(UC_M68K_REG_PC,ret)
horizon_hooks=[u.hook_add(UC_HOOK_CODE,horizon_services,begin=address,end=address) for address in [0x74a9a,0x74af4,0x74b28,a5+0x6ea]]
u.ctl_flush_tb()
for iteration in range(1024):
    center=[rng.randint(-32768,32767) for _ in range(2)];half_width=rng.randint(-32768,32767);vertical_offset=rng.randint(-32768,32767)
    level=rng.choice([-32768,32767,-96,-88,-80,-8,0,32,rng.randint(-32768,32767)])
    depth=rng.choice([1,4,8,16,32]);four_bit=bool(iteration%2);color=rng.choice([3,3,rng.randint(-128,127)])
    solid_color=rng.choice([None,None,-1,rng.randint(-32768,32767)])
    for offset,value in [(0x62fc,center[0]),(0x62fe,center[1]),(0x39a6,half_width),(0x20c2,level),(0x3fe4,depth),(0x130c,solid_color if solid_color is not None else -1)]:u.mem_write(a5-offset,struct.pack('>h',value))
    for offset,value in [(0x6647,int(four_bit)),(0x3b37,color&255),(0x7bef,int(solid_color is not None))]:u.mem_write(a5-offset,bytes([value]))
    horizon_draws.clear();run(0x74a1a,struct.pack('>hh',vertical_offset,0),instruction_limit=100000)
    cases.append(dict(routine='horizon_bands',a=0,b=0,input=dict(center=center,half_width=half_width,vertical_offset=vertical_offset,level=level,depth=depth,four_bit=four_bit,color=color,solid_color=solid_color),result=list(horizon_draws),expected=0))
for hook in horizon_hooks:u.hook_del(hook)
print('1024 complete original horizon routines executed; all rectangles/colors/shades captured, OffsetRect modeled and final paint service captured')
# Complete aiming-mark routine; capture final line drawing calls.
crosshair_draws=[];crosshair_colors=[]
def crosshair_draw(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    color=uc.mem_read(sp+4,1)[0];y1,x1,y0,x0=struct.unpack('>4h',uc.mem_read(sp+6,8))
    crosshair_draws.append([x0,y0,x1,y1]);crosshair_colors.append(color)
    uc.reg_write(UC_M68K_REG_A7,sp+14);uc.reg_write(UC_M68K_REG_PC,ret)
crosshair_hook=u.hook_add(UC_HOOK_CODE,crosshair_draw,begin=a5+0x6ca,end=a5+0x6ca);u.ctl_flush_tb()
for iteration in range(1024):
    center=[rng.choice([-32768,32767,0,rng.randint(-32768,32767)]) for _ in range(2)];hidden=rng.choice([0,0,0,1,255]);color=rng.randrange(256)
    u.mem_write(a5-0x62fc,struct.pack('>h',center[0]));u.mem_write(a5-0x62fe,struct.pack('>h',center[1]));u.mem_write(a5-0x2d70,bytes([hidden]));u.mem_write(a5-0x3b33,bytes([color]))
    crosshair_draws.clear();crosshair_colors.clear();run(0x7415a,b'')
    assert crosshair_colors==([color]*3 if hidden==0 else [])
    cases.append(dict(routine='crosshair_lines',a=0,b=0,center=center,hidden=bool(hidden),result=list(crosshair_draws),expected=0))
u.hook_del(crosshair_hook)
print('1024 complete original aiming-mark routines executed; ordered endpoints, selected colors, hidden branch, and wrapped coordinates checked via captured line service')
# Original viewport placement, radar bounds, and instrument strip.
# Model only OffsetRect; execute original min and rectangle constructors.
def viewport_offset(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    dy,dx,pointer=struct.unpack('>hhI',uc.mem_read(sp,8))
    rect=struct.unpack('>4h',uc.mem_read(pointer,8))
    uc.mem_write(pointer,struct.pack('>4H',*[(v+(dy if i%2==0 else dx))&0xffff for i,v in enumerate(rect)]))
    uc.reg_write(UC_M68K_REG_A7,sp+8);uc.reg_write(UC_M68K_REG_PC,address+2)
viewport_hooks=[u.hook_add(UC_HOOK_CODE,viewport_offset,begin=address,end=address) for address in [0x1907e6,0x1907f0,0x190838]]
u.mem_write(a5+0x452,bytes.fromhex('4ef9')+struct.pack('>I',0x7002c))
u.mem_write(a5+0x44a,bytes.fromhex('4ef9')+struct.pack('>I',0x7001e));u.ctl_flush_tb()
for iteration in range(1024):
    bounds=rng.choice([[0,0,480,640],[0,0,342,512],[40,80,520,720],[rng.randint(-32768,32767) for _ in range(4)]])
    limits=[rng.choice([512,640,800,262,400,rng.randint(-32768,32767)]) for _ in range(2)]
    u.mem_write(a5-0x6632,struct.pack('>4h',*bounds))
    u.mem_write(a5-0x3ff0,struct.pack('>h',limits[0]));u.mem_write(a5-0x3ff4,struct.pack('>h',limits[1]))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    u.emu_start(0x190746,0x19083a,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x19083a
    u.emu_start(0x19083e,0x19087e,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x19087e
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    height,width=struct.unpack('>2h',u.mem_read(a5-0x39a4,4))
    cy,cx=struct.unpack('>2h',u.mem_read(a5-0x62fe,4))
    world=list(struct.unpack('>4h',u.mem_read(a5-0x39ae,8)))
    radar=list(struct.unpack('>4h',u.mem_read(a5-0x39be,8)))
    instruments=list(struct.unpack('>4h',u.mem_read(a5-0x39b6,8)))
    ry,rx=struct.unpack('>2h',u.mem_read(a5-0x39c2,4))
    word=lambda v: ((v+32768)%65536)-32768
    offset=[word(world[1]-bounds[1]),word(world[0]-bounds[0])]
    cases.append(dict(routine='viewport_layout',a=0,b=0,bounds=bounds,limits=limits,result=dict(metrics=dict(size=[width,height],center=[cx,cy],offset=offset),world=world,radar_center=[rx,ry],radar=radar,instruments=instruments),expected=0))
for hook in viewport_hooks:u.hook_del(hook)
print('1024 original viewport/radar/instrument placement blocks executed; OffsetRect modeled, marker initialization skipped, all rectangle words and stack checked')
# Complete display extent/inset/top adjustment block, with modeled InsetRect.
def display_inset(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    dy,dx,pointer=struct.unpack('>hhI',uc.mem_read(sp,8))
    top,left,bottom,right=struct.unpack('>4h',uc.mem_read(pointer,8))
    uc.mem_write(pointer,struct.pack('>4H',*[(v&0xffff) for v in [top+dy,left+dx,bottom-dy,right-dx]]))
    uc.reg_write(UC_M68K_REG_A7,sp+8);uc.reg_write(UC_M68K_REG_PC,address+2)
inset_hook=u.hook_add(UC_HOOK_CODE,display_inset,begin=0x1a04b0,end=0x1a04b0)
u.mem_write(a5+0x128a,bytes.fromhex('4ef9')+struct.pack('>I',0x90038));u.ctl_flush_tb()
for iteration in range(1024):
    bounds=rng.choice([[0,0,480,640],[0,0,481,641],[0,0,342,512],[0,0,343,513],[40,80,520,720],[rng.randint(-32768,32767) for _ in range(4)]])
    full_size=rng.choice([0,0,1,127,255])
    u.mem_write(a5-0x3ffe,struct.pack('>4h',*bounds));u.mem_write(a5-0x7bf0,bytes([full_size]))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    u.emu_start(0x1a043c,0x1a04d4,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x1a04d4
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    width=struct.unpack('>i',u.mem_read(a5-0x3ff2,4))[0];height=struct.unpack('>i',u.mem_read(a5-0x3ff6,4))[0]
    after=list(struct.unpack('>4h',u.mem_read(a5-0x3ffe,8)))
    cases.append(dict(routine='display_rectangle',a=0,b=0,bounds=bounds,full_size=full_size,result=dict(size=[width,height],bounds=after),expected=0))
u.hook_del(inset_hook)
print('1024 complete original display extent/inset/top-adjustment blocks executed; InsetRect modeled, original division, signed extents, final rectangles and stack checked')
# Complete drawing viewport origin setup; use the existing modeled OffsetRect.
drawing_hook=u.hook_add(UC_HOOK_CODE,viewport_offset,begin=0xe1f4a,end=0xe1f4a)
u.ctl_flush_tb()
for iteration in range(1024):
    rect=[rng.randint(-32768,32767) for _ in range(4)]
    origin=[rng.randint(-32768,32767) for _ in range(2)]
    previous=[rng.randint(-32768,32767) for _ in range(2)]
    depth=rng.choice([1,1,4,8,16,32,-1])
    u.mem_write(0x43000,struct.pack('>4h',*rect))
    u.mem_write(a5-0x6632,struct.pack('>2h',origin[1],origin[0]))
    u.mem_write(a5-0x6bf4,struct.pack('>2h',previous[1],previous[0]))
    u.mem_write(a5-0x3fe4,struct.pack('>h',depth))
    run(0xe1f22,struct.pack('>I',0x43000))
    assert u.reg_read(UC_M68K_REG_A7)==stack+8
    assert list(struct.unpack('>4h',u.mem_read(0x43000,8)))==rect
    bounds=list(struct.unpack('>4h',u.mem_read(a5-0x6bfc,8)))
    height,width=struct.unpack('>2h',u.mem_read(a5-0x3fee,4))
    oy,ox=struct.unpack('>2h',u.mem_read(a5-0x6bf4,4))
    cases.append(dict(routine='drawing_viewport',a=depth,b=0,rect=rect,screen_origin=origin,previous_origin=previous,result=dict(bounds=bounds,size=[width,height],origin=[ox,oy]),expected=0))
u.hook_del(drawing_hook)
print('1024 complete original drawing viewport setups executed; OffsetRect modeled, local bounds/extents, color versus monochrome origin, source preservation, and stack checked')
# Complete axis-aligned immediate line routine with terminal rectangle painter captured.
axis_rectangles=[]
def capture_axis_rectangle(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    axis_rectangles.append(list(struct.unpack('>4h',uc.mem_read(sp+8,8))))
    uc.reg_write(UC_M68K_REG_A7,sp+4);uc.reg_write(UC_M68K_REG_PC,ret)
axis_hook=u.hook_add(UC_HOOK_CODE,capture_axis_rectangle,begin=0x1a4004,end=0x1a4004)
u.ctl_flush_tb()
for iteration in range(1024):
    endpoints=[[rng.randint(-32768,32767) for _ in range(2)] for _ in range(2)]
    axis=iteration%2;endpoints[1][axis]=endpoints[0][axis]
    if iteration%7==0:endpoints[1]=list(endpoints[0])
    pen=[rng.choice([1,1,2,0,-1,32767,rng.randint(-32768,32767)]) for _ in range(2)]
    suppressed=iteration%11==0
    u.mem_write(0x43000,bytes(0x63)+bytes([int(suppressed)]))
    axis_rectangles.clear()
    run(0x1a39c6,struct.pack('>I4h5i',0x43000,endpoints[0][1],endpoints[0][0],endpoints[1][1],endpoints[1][0],*pen,0x12345678,-2,0))
    assert u.reg_read(UC_M68K_REG_A7)==stack+4
    assert len(axis_rectangles)<=1
    cases.append(dict(routine='axis_line_rectangle',a=0,b=0,endpoints=endpoints,pen=pen,suppressed=suppressed,result=axis_rectangles[0] if axis_rectangles else None,expected=0))
u.hook_del(axis_hook)
print('1024 complete original axis-aligned immediate line routines executed; terminal rectangle painter captured, pen sizes, reversed endpoints, suppression, degeneracy and wrapped bounds checked')
# Compose complete original aiming marks, color dispatch, immediate line wrappers,
# and axis-line conversion. Only the final rectangle painter is captured.
axis_hook=u.hook_add(UC_HOOK_CODE,capture_axis_rectangle,begin=0x1a4004,end=0x1a4004)
for displacement,target in [(0x6ca,0xe201e),(0xae2,0x1a01d8)]:u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(256):
    center=[rng.randint(-32768,32767) for _ in range(2)];hidden=iteration%9==0
    color=rng.randrange(13)
    u.mem_write(a5-0x62fc,struct.pack('>h',center[0]));u.mem_write(a5-0x62fe,struct.pack('>h',center[1]))
    u.mem_write(a5-0x2d70,bytes([int(hidden)]));u.mem_write(a5-0x3b33,bytes([color]))
    u.mem_write(a5-0x7be2,b'\x01');u.mem_write(a5-0x401c,struct.pack('>I',0x43000))
    u.mem_write(0x43000,bytes(0x64));u.mem_write(a5-0x755e+color*4,struct.pack('>I',0x11223344))
    axis_rectangles.clear();run(0x7415a,b'',instruction_limit=100000)
    cases.append(dict(routine='immediate_crosshair_rectangles',a=0,b=0,center=center,hidden=hidden,result=list(axis_rectangles),expected=0))
u.hook_del(axis_hook)
print('256 complete original immediate crosshair call chains executed through color dispatch and line wrappers; only final rectangle painter captured')
# Surface-to-framebuffer geometry with explicit modeled QuickDraw port translation.
port_translation=[0,0]
def surface_coordinates(uc,address,size,data):
    if address==0x1a37c8:
        viewport_offset(uc,address,size,data);return
    sp=uc.reg_read(UC_M68K_REG_A7);pointer=struct.unpack('>I',uc.mem_read(sp,4))[0]
    y,x=struct.unpack('>2h',uc.mem_read(pointer,4));sign=1 if address==0x1a36d8 else -1
    uc.mem_write(pointer,struct.pack('>2H',(y+sign*port_translation[1])&0xffff,(x+sign*port_translation[0])&0xffff))
    uc.reg_write(UC_M68K_REG_A7,sp+4);uc.reg_write(UC_M68K_REG_PC,address+2)
surface_hooks=[u.hook_add(UC_HOOK_CODE,surface_coordinates,begin=address,end=address) for address in [0x1a36d8,0x1a37a4,0x1a37ae,0x1a37c8]]
u.ctl_flush_tb()
for iteration in range(1024):
    surface_origin=[rng.randint(-32768,32767) for _ in range(2)]
    port_translation[:]=[rng.randint(-32768,32767) for _ in range(2)]
    pixmap_origin=[rng.randint(-32768,32767) for _ in range(2)]
    global_clip=[rng.randint(-32768,32767) for _ in range(4)]
    base=rng.randrange(2**32);stride=rng.randint(-32768,32767)
    u.mem_write(0x43000,bytes(100))
    u.mem_write(0x4300c,struct.pack('>4h',surface_origin[1],surface_origin[0],0,0))
    u.mem_write(0x43018,struct.pack('>2h',pixmap_origin[1],pixmap_origin[0]))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_A4,0x43000)]:u.reg_write(reg,value)
    u.emu_start(0x1a36c2,0x1a36da,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1a36da
    u.emu_start(0x1a3774,0x1a3784,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1a3784
    dy,dx=struct.unpack('>2h',u.mem_read(0x57000-0x14,4))
    u.mem_write(0x57000-0x10,struct.pack('>4h',*global_clip))
    u.emu_start(0x1a37a0,0x1a37ca,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1a37ca
    local_clip=list(struct.unpack('>4h',u.mem_read(0x57000-0x10,8)))
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    u.mem_write(0x57018,struct.pack('>2h',dy,dx));u.mem_write(0x5701c,struct.pack('>4h',*local_clip));u.mem_write(0x5700c,struct.pack('>I',base))
    u.reg_write(UC_M68K_REG_D7,stride&0xffff)
    u.emu_start(0x1a1c20,0x1a1c5a,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x1a1c5a
    cases.append(dict(routine='surface_blit_geometry',a=0,b=0,input=dict(global_clip=global_clip,surface_origin=surface_origin,port_origin=list(port_translation),pixmap_origin=pixmap_origin,base=base,stride=stride),result=dict(local_clip=local_clip,destination_offset=[dx,dy],row_address=u.reg_read(UC_M68K_REG_A3)),expected=0))
for hook in surface_hooks:u.hook_del(hook)
print('1024 original immediate surface presentation geometry blocks executed; explicit modeled port translations and OffsetRect, local clip, destination offset, and framebuffer row address checked')
# Complete original radar projection and edge marker initialization, no fixtures.
u.ctl_flush_tb()
for iteration in range(2048):
    position=[rng.randint(-32768,32767) for _ in range(2)];center=[rng.randint(-32768,32767) for _ in range(2)]
    distance=rng.choice([-1,-1,-32768,0,32767,rng.randint(-32768,32767)])
    threshold=rng.choice([0,1,32767,-32768,rng.randint(-2**31,2**31-1)])
    scale_base=rng.choice([32767,rng.randint(-2**31,2**31-1)]);falloff=rng.randint(-32768,32767)
    u.mem_write(a5-0x39c2,struct.pack('>2h',center[1],center[0]))
    u.mem_write(a5-0x1394,struct.pack('>i',threshold));u.mem_write(a5-0x1398,struct.pack('>i',scale_base));u.mem_write(a5-0x139e,struct.pack('>h',falloff))
    u.mem_write(0x43000,bytes(8))
    run(0x70342,struct.pack('>IIhhh',0x43002,0x43000,distance,*position))
    assert u.reg_read(UC_M68K_REG_A7)==stack+18
    result=list(struct.unpack('>2h',u.mem_read(0x43000,4)))
    cases.append(dict(routine='radar_point',a=distance,b=falloff,position=position,center=center,threshold=threshold,scale_base=scale_base,result=result,expected=0))
for iteration in range(1024):
    center=[rng.randint(-32768,32767) for _ in range(2)]
    u.mem_write(a5-0x39c2,struct.pack('>2h',center[1],center[0]));run(0x700c4,b'')
    result=[list(reversed(p)) for p in struct.iter_unpack('>2h',u.mem_read(a5-0x27f2,48))]
    assert u.reg_read(UC_M68K_REG_A7)==stack+4
    cases.append(dict(routine='radar_edge_points',a=0,b=0,center=center,result=result,expected=0))
print('2048 complete original nonlinear radar projections and 1024 radar edge-marker initializations executed without service fixtures')
# Original glyph extraction; GetPixel reads an explicitly supplied monochrome bitmap.
pict=(root/'.local/game-resources/50494354/1000.bin').read_bytes()
offset=52;atlas=[]
for y in range(5):
    n=pict[offset];offset+=1;packed=pict[offset:offset+n];offset+=n;row=[];j=0
    while j<len(packed):
        control=packed[j];j+=1
        if control<128:row.extend(packed[j:j+control+1]);j+=control+1
        elif control>128:row.extend([packed[j]]*(257-control));j+=1
    assert len(row)==22
    atlas.extend(int(bool(row[(x+6)//8]&(128>>((x+6)%8)))) for x in range(164))
assert pict[offset:]==b'\xff'
glyph_bitmap=[]
def glyph_pixel(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);y,x=struct.unpack('>2h',uc.mem_read(sp,4))
    assert 0<=x<164 and 0<=y<5
    uc.mem_write(sp+4,bytes([glyph_bitmap[y*164+x],0]));uc.reg_write(UC_M68K_REG_A7,sp+4);uc.reg_write(UC_M68K_REG_PC,address+2)
glyph_hook=u.hook_add(UC_HOOK_CODE,glyph_pixel,begin=0x702f0,end=0x702f0);u.ctl_flush_tb()
for iteration in range(257):
    if iteration==0:glyph_bitmap=list(atlas)
    else:
        glyph_bitmap=[0]*820
        for glyph in range(41):
            for cell in rng.sample(range(20),rng.randrange(16)):
                x,y=divmod(cell,5);glyph_bitmap[y*164+glyph*4+x]=1
    u.mem_write(a5-0x2d12,bytes(41*32))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    u.emu_start(0x7029c,0x70320,count=100000);assert u.reg_read(UC_M68K_REG_PC)==0x70320
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    glyphs=[]
    for glyph in range(41):
        raw=bytes(u.mem_read(a5-0x2d12+glyph*32,32));count=struct.unpack_from('>h',raw)[0]
        assert 0<=count<=15
        glyphs.append([list(p) for p in struct.iter_unpack('>2b',raw[2:2+count*2])])
    case=dict(routine='radar_glyphs',a=0,b=0,bitmap=list(glyph_bitmap),result=glyphs,expected=0)
    if iteration==0:case['pict']=list(pict)
    cases.append(case)
u.hook_del(glyph_hook)
print('257 original 41-glyph extraction loops executed with modeled GetPixel, including decoded owned PICT 1000; all ordered glyph offsets checked')
# Enemy radar branch with original type-31 frame gate and captured glyph request.
enemy_radar_kinds=[7,8,13,20,22,26,27,31,35,45]
original6=(source/'6.bin').read_bytes()
assert [kind for kind in range(2,47) if 0x1be0+struct.unpack_from('>h',original6,0x1be2+2*(kind-2))[0]==0x1e4c]==enemy_radar_kinds
radar_requests=[]
def capture_radar_request(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    radar_requests.append(list(struct.unpack('>hBxhhh',uc.mem_read(sp+4,10))))
    uc.reg_write(UC_M68K_REG_A7,sp+14);uc.reg_write(UC_M68K_REG_PC,ret)
radar_request_hook=u.hook_add(UC_HOOK_CODE,capture_radar_request,begin=0x717f6,end=0x717f6);u.ctl_flush_tb()
for iteration in range(2048):
    kind=rng.choice(enemy_radar_kinds);view=rng.choice([-32768,-1,0,1,2,3,32767])
    color=rng.choice([13,13,rng.randrange(256)])
    animation=[rng.randint(-128,127) for _ in range(3)]
    if iteration%2==0:
        animation=[rng.randrange(30),rng.randrange(30),rng.randrange(1,20)];animation[0]=animation[1]+animation[2]-1
    for offset,value in zip([0x25cf,0x25ff,0x262f],animation):u.mem_write(a5-offset,bytes([value&255]))
    final_frame=animation[0]==animation[1]+animation[2]-1
    packet=[rng.randint(-32768,32767) for _ in range(3)]
    record=bytearray(128);record[0x14]=kind;record[0x28:0x2c]=rng.randbytes(4)
    u.mem_write(0x43000,bytes(record));u.mem_write(a5-0x3b30+kind,bytes([color]));u.mem_write(0x57008,struct.pack('>h',view))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_D4,packet[0]&0xffff),(UC_M68K_REG_D5,packet[1]&0xffff),(UC_M68K_REG_D7,packet[2]&0xffff)]:u.reg_write(reg,value)
    radar_requests.clear();u.emu_start(0x71e86,0x727fe,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x727fe
    assert u.reg_read(UC_M68K_REG_A7)==0x56000 and bytes(u.mem_read(0x43000,128))==record
    assert bytes(u.mem_read(0x57000-0x1a20,4))==record[0x28:0x2c]
    assert len(radar_requests)<=1
    if radar_requests:assert radar_requests[0][1:]==[color]+packet
    cases.append(dict(routine='enemy_radar_glyph',a=kind,b=view,color=color,final_frame=final_frame,result=radar_requests[0][0] if radar_requests else None,expected=0))

# Full supported object branches; only terminal glyph drawing is captured.
object_radar_groups={0x2322:[19,36,37],0x241a:[16,17,18,23,24,39,40]}
for entry,kinds in object_radar_groups.items():
    assert [kind for kind in range(2,47) if 0x1be0+struct.unpack_from('>h',original6,0x1be2+2*(kind-2))[0]==entry]==kinds
    for kind in kinds:
        for view in [-32768,-1,0,1,2,3,32767]:
            for color in range(256):
                packet=[rng.randint(-32768,32767) for _ in range(3)]
                solid=rng.randrange(256)
                record=bytearray(rng.randbytes(128));record[0x14]=kind
                u.mem_write(0x43000,bytes(record))
                u.mem_write(a5-0x3b30+kind,bytes([color]))
                u.mem_write(a5-0x2742+kind,bytes([solid]))
                u.mem_write(0x57008,struct.pack('>h',view))
                u.mem_write(0x57000-0x190e,b'\xff')
                for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_D4,packet[0]&0xffff),(UC_M68K_REG_D5,packet[1]&0xffff),(UC_M68K_REG_D7,packet[2]&0xffff)]:u.reg_write(reg,value)
                radar_requests.clear();u.emu_start(0x70000+entry,0x727fe,count=10000)
                assert u.reg_read(UC_M68K_REG_PC)==0x727fe and u.reg_read(UC_M68K_REG_A7)==0x56000
                assert bytes(u.mem_read(0x43000,128))==record
                assert bytes(u.mem_read(0x57000-0x35,1))==bytes([solid])
                if entry==0x2322:assert bytes(u.mem_read(0x57000-0x1a20,4))==record[0x28:0x2c]
                else:assert bytes(u.mem_read(0x57000-0x190e,1))==b'\0'
                assert len(radar_requests)<=1
                if radar_requests:assert radar_requests[0][1:]==[color]+packet
                cases.append(dict(routine='object_radar_glyph',a=kind,b=view,color=color,result=radar_requests[0][0] if radar_requests else None,expected=0))
print('17920 original object radar branches executed; all colors, seven views, dispatch, glyph packets, local writes, source and stack checked')
# Complete type-5 presentation branch, sharing subtype colors for solids/radar.
assert 0x1be0+struct.unpack_from('>h',original6,0x1be2+2*(5-2))[0]==0x23ac
for iteration in range(2048):
    subtype=iteration%4;view=rng.choice([-32768,-1,0,1,2,3,32767]);colors=[rng.randrange(256) for _ in range(4)]
    if iteration%3==0:colors[subtype]=13
    packet=[rng.randint(-32768,32767) for _ in range(3)]
    record=bytearray(rng.randbytes(128));record[0x14]=5;struct.pack_into('>h',record,0x1c,subtype)
    u.mem_write(0x43000,bytes(record));u.mem_write(a5-0x3b00,bytes(colors));u.mem_write(0x57008,struct.pack('>h',view));u.mem_write(0x57000-0x190e,b'\xff')
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_D4,packet[0]&0xffff),(UC_M68K_REG_D5,packet[1]&0xffff),(UC_M68K_REG_D7,packet[2]&0xffff)]:u.reg_write(reg,value)
    radar_requests.clear();u.emu_start(0x723ac,0x727fe,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x727fe and u.reg_read(UC_M68K_REG_A7)==0x56000
    assert bytes(u.mem_read(0x43000,128))==record and bytes(u.mem_read(0x57000-0x190e,1))==b'\0'
    solid=u.mem_read(0x57000-0x35,1)[0];assert solid==colors[subtype]
    assert len(radar_requests)<=1
    if radar_requests:assert radar_requests[0][1:]==[solid]+packet
    cases.append(dict(routine='pickup_presentation',a=subtype,b=view,colors=colors,solid=solid,result=radar_requests[0][:2] if radar_requests else None,expected=0))
print('2048 complete original pickup presentation branches executed; subtype colors, view/hidden-color gates, glyph packets and preservation checked')


# Single-player flag selection; session mode zero excludes owner/participant services.
for iteration in range(2048):
    view=rng.choice([-32768,-1,0,1,2,3,32767])
    color=rng.choice([13,13,rng.randrange(256)]);flash=rng.choice([0,1,rng.randrange(256)])
    packet=[rng.randint(-32768,32767) for _ in range(3)]
    u.mem_write(a5-0x12ca,b'\0');u.mem_write(a5-0x3991,bytes([flash]))
    u.mem_write(a5-0x3b24,bytes([color]));u.mem_write(0x57008,struct.pack('>h',view))
    u.mem_write(0x57000-0xf,bytes([int(color!=13)]))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D4,packet[0]&0xffff),(UC_M68K_REG_D5,packet[1]&0xffff),(UC_M68K_REG_D7,packet[2]&0xffff)]:u.reg_write(reg,value)
    radar_requests.clear();u.emu_start(0x724be,0x725e6,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x725e6 and u.reg_read(UC_M68K_REG_A7)==0x56000
    assert len(radar_requests)<=1
    if radar_requests:assert radar_requests[0][1:]==[color]+packet
    cases.append(dict(routine='flag_radar_glyph',a=view,b=flash,color=color,result=radar_requests[0][0] if radar_requests else None,expected=0))
for iteration in range(2048):
    now=rng.randint(-2**31,2**31-1)
    deadline=rng.choice([now,now-1 if now>-2**31 else now,now+1 if now<2**31-1 else now,rng.randint(-2**31,2**31-1)])
    flash=rng.randrange(256)
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x3996,struct.pack('>i',deadline));u.mem_write(a5-0x3991,bytes([flash]))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x75082,0x750a4,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x750a4 and u.reg_read(UC_M68K_REG_A7)==0x56000
    result=[struct.unpack('>i',u.mem_read(a5-0x3996,4))[0],u.mem_read(a5-0x3991,1)[0]]
    cases.append(dict(routine='radar_flash',a=now,b=deadline,flash=flash,result=result,expected=0))
print('2048 original single-player flag radar selections and 2048 flash updates executed; terminal glyph drawing captured, timing arithmetic original')
u.hook_del(radar_request_hook)
print('2048 original enemy radar selection blocks executed with original type-31 animation gate; dispatch table, glyph/color/position requests, height copy, input preservation and stack checked')

# Complete glyph routine, including original projection, disturbance, and color queue.
glyph_random=[];glyph_random_used=[]
def supply_glyph_random(uc,address,size,data):
    value=glyph_random[len(glyph_random_used)];glyph_random_used.append(value)
    sp=uc.reg_read(UC_M68K_REG_A7)
    uc.mem_write(sp,struct.pack('>h',value));uc.reg_write(UC_M68K_REG_PC,address+2)
glyph_random_hooks=[u.hook_add(UC_HOOK_CODE,supply_glyph_random,begin=address,end=address) for address in [0x7181a,0x7183a]]
u.ctl_flush_tb()
for iteration in range(2048):
    glyph=rng.randrange(1,42);color=rng.randrange(13);count=rng.randrange(16);prefix=rng.randrange(20)
    offsets=[[rng.randint(-128,127) for _ in range(2)] for _ in range(count)]
    glyph_record=struct.pack('>h',count)+b''.join(struct.pack('>2b',*p) for p in offsets)
    u.mem_write(a5-0x2d12+(glyph-1)*32,glyph_record+bytes(32-len(glyph_record)))
    position=[rng.randint(-32768,32767) for _ in range(2)];center=[rng.randint(-32768,32767) for _ in range(2)]
    distance=rng.choice([-1,-1,rng.randint(-32768,32767)])
    stable=rng.choice([0,0,1,255]);now=rng.randint(-2**31,2**31-1)
    deadline=rng.choice([now,rng.randint(-2**31,2**31-1)])
    glyph_random[:]=[rng.choice([-32768,-32767,0,32767,rng.randint(-32768,32767)]) for _ in range(2)]
    glyph_random_used.clear()
    u.mem_write(a5-0x39c2,struct.pack('>2h',center[1],center[0]))
    u.mem_write(a5-0x1394,struct.pack('>i',156));u.mem_write(a5-0x1398,struct.pack('>i',32767));u.mem_write(a5-0x139e,struct.pack('>h',1688))
    u.mem_write(a5-0x12ef,bytes([stable]));u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x12f4,struct.pack('>i',deadline))
    queues=bytearray(rng.randbytes(13*2002))
    for c in range(13):struct.pack_into('>h',queues,c*2002,prefix)
    u.mem_write(0x44000,bytes(queues));u.mem_write(a5-0x2d16,struct.pack('>I',0x44000))
    run(0x717f6,struct.pack('>hBxhhh',glyph,color,distance,*position))
    assert u.reg_read(UC_M68K_REG_A7)==stack+14
    assert len(glyph_random_used)==(2 if stable==0 else 0)
    final=bytes(u.mem_read(0x44000,len(queues)));base=color*2002
    assert struct.unpack_from('>h',final,base)[0]==prefix+count
    points=[list(reversed(p)) for p in struct.iter_unpack('>2h',final[base+2+prefix*4:base+2+(prefix+count)*4])]
    expected_queue=bytearray(queues)
    expected_queue[base:base+2]=final[base:base+2]
    expected_queue[base+2+prefix*4:base+2+(prefix+count)*4]=final[base+2+prefix*4:base+2+(prefix+count)*4]
    assert final==expected_queue
    assert bytes(u.mem_read(a5-0x2d12+(glyph-1)*32,len(glyph_record)))==glyph_record
    result=[points,u.mem_read(a5-0x12ef,1)[0]]
    cases.append(dict(routine='radar_glyph_points',a=distance,b=stable,position=position,center=center,offsets=offsets,now=now,deadline=deadline,random=list(glyph_random),result=result,expected=0))
for hook in glyph_random_hooks:u.hook_del(hook)
print('2048 complete original radar glyph routines executed through projection, disturbance and color-queue append; supplied Random words, all other arithmetic original; queue isolation and stack checked')

# Original low-health radar disturbance trigger; real selected-difficulty getter.
u.mem_write(a5+0xba,bytes.fromhex('4ef9')+struct.pack('>I',0x120890))
disturbance_sounds=[]
def capture_disturbance_sound(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    disturbance_sounds.append(list(struct.unpack('>hI',uc.mem_read(sp+4,6))))
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
disturbance_hook=u.hook_add(UC_HOOK_CODE,capture_disturbance_sound,begin=a5+0x972,end=a5+0x972)
u.ctl_flush_tb()
for iteration in range(2048):
    victim=rng.randrange(1,13);local=rng.choice([victim,rng.randrange(1,13)])
    health=rng.choice([-32768,-1,0,1,2,3,32767,rng.randint(-32768,32767)])
    difficulty=rng.choice([0,1,2,7,546,547,-32768,32767,rng.randint(-32768,32767)])
    now=rng.randint(-2**31,2**31-1);old_deadline=rng.randint(-2**31,2**31-1);old_stable=rng.randrange(256)
    participants=rng.randbytes(12*256);u.mem_write(0x42000,participants)
    u.mem_write(0x42000+(victim-1)*256+0x5a,struct.pack('>h',health))
    participants=bytes(u.mem_read(0x42000,12*256))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x133d,bytes([local]))
    u.mem_write(a5-0x4258,struct.pack('>h',difficulty));u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.mem_write(a5-0x12ef,bytes([old_stable]));u.mem_write(a5-0x12f4,struct.pack('>i',old_deadline))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D7,victim)]:u.reg_write(reg,value)
    disturbance_sounds.clear();u.emu_start(0x30c4c,0x30caa,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x30caa and u.reg_read(UC_M68K_REG_A7)==0x56000
    assert bytes(u.mem_read(0x42000,12*256))==participants
    deadline=struct.unpack('>i',u.mem_read(a5-0x12f4,4))[0];stable=u.mem_read(a5-0x12ef,1)[0]
    result=None
    if disturbance_sounds:
        assert len(disturbance_sounds)==2
        result=dict(deadline=deadline,stable=stable,sounds=list(disturbance_sounds))
    else:assert (deadline,stable)==(old_deadline,old_stable)
    cases.append(dict(routine='radar_damage_disturbance',a=victim,b=local,health=health,difficulty=difficulty,now=now,result=result,expected=0))
u.hook_del(disturbance_hook)
print('2048 original low-health radar disturbance blocks executed with original difficulty getter; sound requests captured, deadline/stable writes, non-trigger preservation and stack checked')

# Complete AI behavior-five cooldown handler, with only sound playback captured.
cooldown_sounds=[]
def capture_cooldown_sound(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    cooldown_sounds.append(list(struct.unpack('>hI',uc.mem_read(sp+4,6))))
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
cooldown_hook=u.hook_add(UC_HOOK_CODE,capture_cooldown_sound,begin=a5+0x972,end=a5+0x972)
u.ctl_flush_tb()
for iteration in range(2048):
    participant=rng.randrange(1,13)
    now=rng.choice([-2**31,2**31-1,rng.randint(-2**31,2**31-1)])
    deadline=rng.choice([now,rng.randint(-2**31,2**31-1)])
    event=dict(kind=rng.choice([23,23,rng.randint(-128,127)]),behavior=rng.randrange(256),movement=rng.randrange(256),
        deadline=deadline,now=now,distance=rng.choice([(400<<16)-1,400<<16,(400<<16)+1,rng.randint(-2**31,2**31-1)]),
        local_dying=rng.choice([0,1,2,255]))
    records=bytearray(rng.randbytes(12*256));base=(participant-1)*256
    records[base+8]=event['kind']&255;records[base+9]=event['behavior'];records[base+16]=event['movement']
    struct.pack_into('>i',records,base+0x4e,deadline)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x131b,bytes([event['local_dying']]))
    cooldown_sounds.clear();run(0x170fb8,struct.pack('>ibx',event['distance'],participant))
    assert u.reg_read(UC_M68K_REG_A7)==stack+10
    after=bytes(u.mem_read(0x42000,len(records)));deadline_after=struct.unpack_from('>i',after,base+0x4e)[0]
    expected_record=bytearray(records)
    for offset,length in [(9,1),(16,1),(0x4e,4)]:expected_record[base+offset:base+offset+length]=after[base+offset:base+offset+length]
    assert after==expected_record
    assert len(cooldown_sounds)<=1
    if cooldown_sounds:assert cooldown_sounds[0][0]==1
    result=[after[base+9],after[base+16],deadline_after,cooldown_sounds[0][1] if cooldown_sounds else None]
    cases.append(dict(routine='ai_cooldown',a=0,b=0,input=event,result=result,expected=0))
u.hook_del(cooldown_hook)
print('2048 complete original AI behavior-five cooldown handlers executed; sound request captured, all participant bytes and stack checked')

original20=(source/'20.bin').read_bytes()
assert [k for k in range(32) if original20[0x14be-1-k//8]&(1<<(k%8))]==[23,24]
for iteration in range(1024):
    now=rng.choice([-2**31,2**31-1,rng.randint(-2**31,2**31-1)])
    deadline=rng.choice([-1,now,rng.randint(-2**31,2**31-1)])
    behavior=rng.randrange(256);movement=rng.randrange(256)
    record=bytearray(rng.randbytes(80));record[1]=behavior;record[8]=movement;struct.pack_into('>i',record,0x46,deadline)
    u.mem_write(0x43000,bytes(record));u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A4,0x43000);u.reg_write(UC_M68K_REG_A7,0x56000)
    u.emu_start(0x1712f8,0x1714ae,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x1714ae and u.reg_read(UC_M68K_REG_A7)==0x56000
    after=bytes(u.mem_read(0x43000,80));expected_record=bytearray(record)
    for offset,length in [(1,1),(8,1),(0x46,4)]:expected_record[offset:offset+length]=after[offset:offset+length]
    assert after==expected_record
    result=[after[1],after[8],struct.unpack_from('>i',after,0x46)[0]]
    cases.append(dict(routine='ai_surface_phase',a=now,b=deadline,behavior=behavior,movement=movement,result=result,expected=0))
print('1024 original kind-23/kind-24 phase blocks executed without fixtures; original kind mask, record preservation, phase deadline and stack checked')

# Complete acid contact handler, capturing terminal sound/local/network requests.
acid_calls=[]
def acid_service(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    if address==a5+0x972:
        acid_calls.append(['sound',*struct.unpack('>hI',uc.mem_read(sp+4,6))]);n=6
    elif address==a5+0x119a:
        damage,death,timestamp,attacker,victim=struct.unpack('>hBxi bxbx',uc.mem_read(sp+4,12))
        assert death==0
        acid_calls.append(['local',damage,timestamp,attacker,victim]);n=12
    else:
        damage,timestamp,attacker,victim=struct.unpack('>hi bxbx',uc.mem_read(sp+4,10))
        acid_calls.append(['network',damage,timestamp,attacker,victim]);n=10
    uc.reg_write(UC_M68K_REG_A7,sp+4+n);uc.reg_write(UC_M68K_REG_PC,ret)
acid_hooks=[u.hook_add(UC_HOOK_CODE,acid_service,begin=a5+offset,end=a5+offset) for offset in [0x972,0x119a,0x101a]]
u.ctl_flush_tb()
for iteration in range(2048):
    now=rng.choice([-2**31,2**31-1,rng.randint(-2**31,2**31-1)]);deadline=rng.choice([now,0,rng.randint(-2**31,2**31-1)])
    mode=rng.choice([0,0,1,255]);damage=rng.randint(-32768,32767)
    attacker=rng.randint(-128,127);victim=rng.randint(-128,127);geometry=rng.randrange(1,13)
    records=bytearray(rng.randbytes(12*128));records[(geometry-1)*128+0x1a]=attacker&255
    u.mem_write(0x44000,bytes(records));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x1350,struct.pack('>I',0x46000));u.mem_write(0x4614a,struct.pack('>h',damage))
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-0x3d1c,struct.pack('>i',deadline));u.mem_write(a5-0x12ca,bytes([mode]))
    acid_calls.clear();run(0x11096,struct.pack('>hbx',geometry,victim))
    assert u.reg_read(UC_M68K_REG_A7)==stack+8 and bytes(u.mem_read(0x44000,len(records)))==records
    after=struct.unpack('>i',u.mem_read(a5-0x3d1c,4))[0]
    result=None
    if acid_calls:
        assert len(acid_calls)==2 and acid_calls[0][0]=='sound'
        route,amount,timestamp,source_id,victim_id=acid_calls[1]
        result=dict(deadline=after,sound=acid_calls[0][1:],damage=amount,timestamp=timestamp,attacker=source_id,victim=victim_id,network=route=='network')
    else:assert after==deadline
    cases.append(dict(routine='acid_contact',a=now,b=deadline,mode=mode,damage=damage,attacker=attacker,victim=victim,result=result,expected=0))
for hook in acid_hooks:u.hook_del(hook)
print('2048 complete original acid contact handlers executed; terminal sound/local/network damage captured; strict deadline, owner/amount forwarding, record preservation and stack checked')

# Complete ice/mud contact handlers; only terminal sound requests are captured.
handling_sounds=[]
def handling_sound(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    handling_sounds.append(list(struct.unpack('>hI',uc.mem_read(sp+4,6))))
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
handling_hook=u.hook_add(UC_HOOK_CODE,handling_sound,begin=a5+0x972,end=a5+0x972)
u.ctl_flush_tb()
for iteration in range(2048):
    kind=rng.choice([17,18,24]);entry=0x1132a if kind==17 else 0x11360
    clock=0x3d20 if kind==17 else 0x12f8;other=0x12f8 if kind==17 else 0x3d20
    now=rng.choice([-2**31,2**31-1,rng.randint(-2**31,2**31-1)])
    deadline=rng.choice([now,rng.randint(-2**31,2**31-1)]);mud=rng.randint(-128,127)
    untouched=rng.randbytes(4);u.mem_write(a5-other,untouched)
    u.mem_write(a5-0x134c,struct.pack('>i',now));u.mem_write(a5-clock,struct.pack('>i',deadline))
    u.mem_write(a5-0x3fcb,bytes([mud&255]));u.mem_write(a5-0x3d32,rng.randbytes(1))
    handling_sounds.clear();run(entry,rng.randbytes(4))
    assert u.reg_read(UC_M68K_REG_A7)==stack+8 and bytes(u.mem_read(a5-other,4))==untouched
    assert len(handling_sounds)<=1
    result=dict(adjustment=struct.unpack('>b',u.mem_read(a5-0x3d32,1))[0],
        deadline=struct.unpack('>i',u.mem_read(a5-clock,4))[0],
        sound=handling_sounds[0] if handling_sounds else None)
    cases.append(dict(routine='handling_contact',a=now,b=deadline,kind=kind,mud=mud,result=result,expected=0))
u.hook_del(handling_hook)
print('2048 complete original ice/mud contact handlers executed; terminal sound requests captured, adjustment, independent cooldown clocks and stack checked')
# Original pickup refill blocks with original capacity and signed-min helpers.
for disp,target in [(0xe0a,0xf0684),(0xe12,0xf06fc),(0x452,0x7002c)]:
    u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(1024):
    participant=iteration%12+1;full=bool(iteration%2)
    health=rng.choice([0,6,32767,-32768,rng.randint(-32768,32767)])
    ammo=rng.choice([0,40,32760,-32768,rng.randint(-32768,32767)])
    shields=rng.randint(-128,127);ammo_setting=rng.randint(-128,127)
    records=bytearray(rng.randbytes(12*256));base=(participant-1)*256
    records[base+5]=0
    struct.pack_into('>h',records,base+0x5a,health);struct.pack_into('>h',records,base+0x5e,ammo)
    u.mem_write(0x42000,bytes(records));u.mem_write(a5-0x1342,struct.pack('>I',0x42000))
    u.mem_write(a5-0x3f8e+(participant-1)*40+0x21,struct.pack('>bb',shields,ammo_setting))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A4,0x42000+base),(UC_M68K_REG_D7,participant),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    entry,end=(0x31240,0x31258) if full else (0x311e6,0x3121c)
    u.emu_start(entry,end,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==end and u.reg_read(UC_M68K_REG_A7)==0x56000
    after=bytes(u.mem_read(0x42000,len(records)))
    result=[struct.unpack_from('>h',after,base+offset)[0] for offset in [0x5a,0x5e]]
    for offset,value in zip([0x5a,0x5e],result):struct.pack_into('>h',records,base+offset,value)
    assert after==bytes(records)
    cases.append(dict(routine='dump_refill',a=health,b=ammo,shields=shields,ammo_setting=ammo_setting,full=full,result=result,expected=0))
print('1024 original pickup refill blocks executed with original capacity/min helpers; signed overflow, participant preservation and stack checked')
# Original subtype routine and relocation loop, supplying OS Random and collision service.
dump_words=[];dump_used=[];dump_queries=[];dump_positions=[];dump_accept=0
def dump_services(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7)
    if address in [0xb022c,0x31428,0x31458]:
        value=dump_words[len(dump_used)];dump_used.append(value)
        uc.mem_write(sp,struct.pack('>h',value));uc.reg_write(UC_M68K_REG_PC,address+2);return
    ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    if address==0x310f6:
        geometry,pointer=struct.unpack('>hI',uc.mem_read(sp+4,6));assert geometry==7
        dump_queries.append(list(struct.unpack('>iii',uc.mem_read(pointer,12))))
        uc.mem_write(sp+10,bytes([int(len(dump_queries)==dump_accept)]));n=6
    else:
        pointer,geometry=struct.unpack('>Ih',uc.mem_read(sp+4,6));assert geometry==7
        dump_positions.append(list(struct.unpack('>iii',uc.mem_read(pointer,12))));n=6
    uc.reg_write(UC_M68K_REG_A7,sp+4+n);uc.reg_write(UC_M68K_REG_PC,ret)
dump_hooks=[u.hook_add(UC_HOOK_CODE,dump_services,begin=address,end=address) for address in [0xb022c,0x31428,0x31458,0x310f6,a5+0x1062]]
u.mem_write(a5+0x1282,bytes.fromhex('4ef9')+struct.pack('>I',0x90004));u.ctl_flush_tb()
for iteration in range(2048):
    word=rng.choice([-32768,0,32767,rng.randint(-32768,32767)])
    weights=[5,2,1] if iteration%2 else [rng.randint(-128,127) for _ in range(3)]
    for offset,value in zip([0x3fc7,0x3fc8,0x3fc9],weights):u.mem_write(a5-offset,struct.pack('>b',value))
    dump_words[:]=[word];dump_used.clear();run(0xb0224,b'')
    assert dump_used==[word] and u.reg_read(UC_M68K_REG_A7)==stack+4
    result=struct.unpack('>h',u.mem_read(stack+4,2))[0]
    cases.append(dict(routine='dump_subtype',a=word,b=0,weights=weights,expected=result))
for iteration in range(256):
    extent=rng.choice([256,512,1024,2048,32767,-32768,201,199])
    dump_accept=rng.choice([0,1,2,25,50,51]);pose=[rng.randint(-2**31,2**31-1) for _ in range(3)]
    dump_words[:]=[rng.choice([-32768,0,32767,rng.randint(-32768,32767)]) for _ in range(102)]
    dump_used.clear();dump_queries.clear();dump_positions.clear()
    u.mem_write(0x43000,struct.pack('>iii',*pose));u.mem_write(a5-0x137e,struct.pack('>h',extent))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A4,0x43000),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D6,7)]:u.reg_write(reg,value)
    u.emu_start(0x31416,0x314b8,count=50000)
    assert u.reg_read(UC_M68K_REG_PC)==0x314b8 and u.reg_read(UC_M68K_REG_A7)==0x56000
    assert len(dump_positions)==1 and bytes(u.mem_read(0x43000,12))==struct.pack('>iii',*pose)
    result=[dump_positions[0],len(dump_used)//2]
    cases.append(dict(routine='relocate_dump',a=extent,b=0,pose=pose,words=list(dump_used),accept=dump_accept,queries=list(dump_queries),result=result,expected=0))
for hook in dump_hooks:u.hook_del(hook)
print('2048 complete original dump subtype routines and 256 relocation loops executed; supplied OS random/collision, all candidate queries and final position/stack checked')
# All four pickup effect branches, retaining original getters and arithmetic.
effect_sounds=[];effect_requests=[]
def dump_effect_service(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    if address==a5+0x972:
        effect_sounds.append(list(struct.unpack('>hI',uc.mem_read(sp+4,6))));n=6
    elif address==a5+0x111a:
        height,kind,weapon,stamp,pointer,owner=struct.unpack('>hBxBxiIBx',uc.mem_read(sp+4,16))
        assert (kind,weapon,stamp,pointer,owner)==(32,7,12345,0x44000,local)
        effect_requests.append([False,height]);n=16
    else:
        height,owner,weapon,pointer=struct.unpack('>hBxBxI',uc.mem_read(sp+4,10))
        assert (owner,weapon,pointer)==(local,7,0x44000)
        effect_requests.append([True,height]);n=10
    uc.reg_write(UC_M68K_REG_A7,sp+4+n);uc.reg_write(UC_M68K_REG_PC,ret)
effect_hooks=[u.hook_add(UC_HOOK_CODE,dump_effect_service,begin=a5+off,end=a5+off) for off in [0x972,0x111a,0x1012]]
for disp,target in [(0xe0a,0xf0684),(0xe12,0xf06fc),(0x452,0x7002c),(0x128a,0x90038)]:u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(2048):
    participant=iteration%12+1;local=(iteration//12)%12+1;base=(participant-1)*256;local_base=(local-1)*256
    subtype=rng.choice([0,1,2,3,-1,32767]);health=rng.randint(-32768,32767);ammo=rng.randint(-32768,32767)
    shields=rng.randint(-128,127);ammo_setting=rng.randint(-128,127)
    vertical=rng.choice([-2**31,2**31-1,-65535,-1,0,65535,rng.randint(-2**31,2**31-1)])
    energy=rng.randint(-32768,32767);mode=rng.choice([0,1,255])
    records=bytearray(rng.randbytes(12*256));records[base+5]=0
    for offset,value in [(base+0x5a,health),(base+0x5e,ammo),(base+0x56,2),(local_base+0x9a,energy)]:struct.pack_into('>h',records,offset,value)
    geometry=bytearray(rng.randbytes(256));struct.pack_into('>h',geometry,0x1c,subtype);struct.pack_into('>i',geometry,128+0x28,vertical)
    u.mem_write(0x42000,bytes(records));u.mem_write(0x44000,bytes(geometry))
    for off,value in [(0x1342,0x42000),(0x1346,0x44000)]:u.mem_write(a5-off,struct.pack('>I',value))
    u.mem_write(a5-0x133d,bytes([local]));u.mem_write(a5-0x12ca,bytes([mode]));u.mem_write(a5-0x134c,struct.pack('>i',12345))
    u.mem_write(a5-0x3f8e+base//256*40+0x21,struct.pack('>bb',shields,ammo_setting))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A4,0x42000+base),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D6,1),(UC_M68K_REG_D7,participant)]:u.reg_write(reg,value)
    effect_sounds.clear();effect_requests.clear();u.emu_start(0x311d6,0x313da,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x313da and u.reg_read(UC_M68K_REG_A7)==0x56000
    after=bytes(u.mem_read(0x42000,len(records)))
    values=[struct.unpack_from('>h',after,off)[0] for off in [base+0x5a,base+0x5e,local_base+0x9a]]
    for offset,value in zip([base+0x5a,base+0x5e,local_base+0x9a],values):struct.pack_into('>h',records,offset,value)
    assert after==records and bytes(u.mem_read(0x44000,256))==geometry
    assert len(effect_sounds)<=1 and len(effect_requests)<=1
    result=dict(health=values[0],ammo=values[1],local_energy=values[2],sound=effect_sounds[0] if effect_sounds else None,effect=effect_requests[0] if effect_requests else None)
    inputs=dict(subtype=subtype,health=health,ammo=ammo,shields=shields,ammo_setting=ammo_setting,vertical=vertical,local_energy=energy,mode=mode)
    cases.append(dict(routine='dump_effect',a=0,b=0,input=inputs,result=result,expected=0))
for hook in effect_hooks:u.hook_del(hook)
print('2048 original pickup effect dispatch blocks executed; real arithmetic/getters, terminal sound/effect packets, remote/local energy ownership, record preservation and stack checked')
# Complete initial object creation, including original proximity and subtype routines.
creation_words=[];creation_used=[]
def creation_random(uc,address,size,data):
    word=creation_words[len(creation_used)];creation_used.append(word)
    uc.mem_write(uc.reg_read(UC_M68K_REG_A7),struct.pack('>h',word));uc.reg_write(UC_M68K_REG_PC,address+2)
creation_hooks=[u.hook_add(UC_HOOK_CODE,creation_random,begin=address,end=address) for address in [0xb02d4,0xb0302,0xb0374,0xb022c]]
u.mem_write(a5+0x1282,bytes.fromhex('4ef9')+struct.pack('>I',0x90004));u.ctl_flush_tb()
for iteration in range(512):
    count=rng.choice([0,1,5,20,49,50,51]);extent=rng.choice([512,1024,2048])
    kind=rng.choice([5,5,5,4,12]);now=rng.randint(-2**31,2**31-1);weights=[rng.randint(-128,127) for _ in range(3)]
    records=bytearray(rng.randbytes(53*128));existing=[]
    for index in range(count):
        point=[rng.randrange(100,extent-100)<<16,rng.randrange(100,extent-100)<<16]
        if index==0:point=[100<<16,100<<16]
        existing.append(point);struct.pack_into('>ii',records,index*128,*point)
    before=list(records[count*128:(count+1)*128]);u.mem_write(0x42000,bytes(records))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x12c8,struct.pack('>h',count));u.mem_write(a5-0x137e,struct.pack('>h',extent));u.mem_write(a5-0x134c,struct.pack('>i',now))
    for offset,value in zip([0x3fc7,0x3fc8,0x3fc9],weights):u.mem_write(a5-offset,struct.pack('>b',value))
    creation_words[:]=[0,0,0,0]+[rng.randint(-32768,32767) for _ in range(4096)];creation_used.clear()
    run(0xb029c,struct.pack('>bx',kind),instruction_limit=100000)
    assert u.reg_read(UC_M68K_REG_A7)==stack+6
    after=bytes(u.mem_read(0x42000,len(records)));created=count<50
    assert struct.unpack('>h',u.mem_read(a5-0x12c8,2))[0]==count+int(created)
    expected=bytearray(records);expected[count*128:(count+1)*128]=after[count*128:(count+1)*128];assert after==expected
    cases.append(dict(routine='create_dump_record',a=extent,b=kind,now=now,weights=weights,existing=existing,before=before,after=list(after[count*128:(count+1)*128]),created=created,words=list(creation_used),expected=0))
for hook in creation_hooks:u.hook_del(hook)
print('512 complete original initial object creations executed with only OS Random supplied; original proximity and subtype routines, every record byte, capacity, random consumption and stack checked')
# Complete original dump animation, including both real vertex stores.
for displacement,target in [(0x7ea,0x100a66),(0x1282,0x90004)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(1024):
    phase=iteration if iteration<60 else rng.randint(-32768,32767)
    slot=rng.randint(1,8)
    u.mem_write(a5-0x25e9,struct.pack('>b',slot))
    u.mem_write(a5-0x648e+(slot-1)*4,struct.pack('>I',0x47000))
    u.mem_write(a5-0x13b0,struct.pack('>I',0x42000))
    primary=rng.randbytes(64);secondary=rng.randbytes(8*0x25a)
    u.mem_write(0x47000,primary);u.mem_write(0x42000,secondary)
    run(0xc2276,struct.pack('>h',phase))
    assert u.reg_read(UC_M68K_REG_A7)==stack+6
    result=[list(struct.unpack('>hhh',u.mem_read(0x47002+i*10,6))) for i in range(4)]
    expected_primary=bytearray(primary);expected_secondary=bytearray(secondary)
    for i,vertex in enumerate(result):
        struct.pack_into('>hhh',expected_primary,2+i*10,*vertex)
        struct.pack_into('>hhh',expected_secondary,(slot-1)*0x25a+2+i*10,*vertex)
    assert bytes(u.mem_read(0x47000,64))==expected_primary
    assert bytes(u.mem_read(0x42000,len(secondary)))==expected_secondary
    cases.append(dict(routine='dump_animation',a=phase,b=0,result=result,expected=0))
print('1024 complete original dump animations: both vertex buffers, untouched bytes and stack checked')
# Original hover-height branches with actual long arithmetic and word maximum.
for displacement,target in [(0x1282,0x90004),(0x128a,0x90038),(0x45a,0x7003c)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(2048):
    height=rng.choice([0,1,200,-1,rng.randint(-2**31,2**31-1)])
    vertical=rng.randint(-2**31,2**31-1);hold=rng.choice([0,0,0,1,127,-128,-1])
    active=bool(rng.randrange(2));speed=rng.randint(-128,127)
    setting=rng.choice([1,3,5,8,9,-128,-1,127]);now=rng.randint(-2**31,2**31-1)
    elapsed=rng.choice([0,1,2,4,60,rng.randint(-2**31,2**31-1)])
    record=bytearray(rng.randbytes(128));record[0x1a]=1
    struct.pack_into('>i',record,0x28,vertical);struct.pack_into('>b',record,0x22,speed);struct.pack_into('>b',record,0x5a,hold)
    u.mem_write(0x42000,bytes(record));u.mem_write(a5-0x3f8e+0x20,struct.pack('>b',setting))
    u.mem_write(a5-0x131a,struct.pack('>i',height));u.mem_write(a5-0x1315,bytes([active]));u.mem_write(a5-0x134c,struct.pack('>i',now))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A4,0x42000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D7,elapsed&0xffffffff)]:u.reg_write(reg,value)
    u.emu_start(0x8154c,0x81668,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x81668
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    result=[struct.unpack('>i',u.mem_read(a5-0x131a,4))[0],struct.unpack('>i',u.mem_read(0x42028,4))[0],struct.unpack('>b',u.mem_read(0x4205a,1))[0]]
    expected_record=bytearray(record);struct.pack_into('>i',expected_record,0x28,result[1]);struct.pack_into('>b',expected_record,0x5a,result[2])
    assert bytes(u.mem_read(0x42000,128))==expected_record
    cases.append(dict(routine='flight_height',a=height,b=vertical,hold=hold,active=active,speed=speed,setting=setting,clocks=[now,elapsed],result=result,expected=0))
print('2048 original hover-height branches executed: active, falling and hold paths, real arithmetic, record preservation and stack checked')
# Complete local energy update block, with original minimum/maximum and division.
for displacement,target in [(0x452,0x7002c),(0x45a,0x7003c),(0x128a,0x90038)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(2048):
    flight=rng.choice([0,1,400,2000,-32768,32767,rng.randint(-32768,32767)])
    hyperspace=rng.randint(-32768,32767);paused=bool(rng.randrange(2));owner=rng.randint(1,12)
    previous=rng.randint(-2**31,2**31-1)
    elapsed=rng.choice([0,1,3,4,7,8,60,-1,262144,rng.randint(-2**31,2**31-1)])
    now=((previous+elapsed+2**31)%2**32)-2**31
    records=bytearray(rng.randbytes(12*256));offset=(owner-1)*256+0x9a
    struct.pack_into('>h',records,offset,flight);u.mem_write(0x42000,bytes(records))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x133d,bytes([owner]))
    u.mem_write(a5-0x1378,struct.pack('>h',hyperspace));u.mem_write(a5-0x134c,struct.pack('>i',previous));u.mem_write(a5-0x2d70,bytes([paused]))
    u.mem_write(a5-0x1315,b'\x7f')
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D7,now&0xffffffff)]:u.reg_write(reg,value)
    u.emu_start(0x1900da,0x190172,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x190172
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    enabled=bytes(u.mem_read(a5-0x1315,1))[0];assert enabled in [0,1]
    result=[struct.unpack('>h',u.mem_read(0x42000+offset,2))[0],struct.unpack('>h',u.mem_read(a5-0x1378,2))[0],bool(enabled)]
    struct.pack_into('>h',records,offset,result[0]);assert bytes(u.mem_read(0x42000,len(records)))==records
    assert struct.unpack('>i',u.mem_read(a5-0x134c,4))[0]==previous
    cases.append(dict(routine='energy_clock',a=flight,b=hyperspace,paused=paused,now=now,previous=previous,result=result,expected=0))
print('2048 original local energy blocks: pause, clock/word overflow, all twelve owner slots, record preservation and stack checked')
# Whole transition and hover block, preserving the original shared camera state.
for displacement,target in [(0x1282,0x90004),(0x128a,0x90038),(0x45a,0x7003c)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(2048):
    before=dict(transition=rng.choice([0,65536,-65536,rng.randint(-2**31,2**31-1)]),
        camera_scale=rng.choice([4*65536,-12*65536,rng.randint(-2**31,2**31-1)]),
        renderer_height=rng.randint(-32768,32767),hover=rng.randint(-100,400),
        hold=rng.choice([0,0,1,127,-1,-128]),vertical=rng.randint(-2**31,2**31-1),active=bool(rng.randrange(2)))
    energy=rng.choice([0,400,-1]);speed=rng.randint(-128,127);setting=rng.choice([1,5,9,-128,-1])
    now=rng.randint(-2**31,2**31-1);elapsed=rng.choice([0,1,2,8,60,-1,rng.randint(-2**31,2**31-1)])
    record=bytearray(rng.randbytes(128));record[0x1a]=1
    for off,key in [(0x24,'transition'),(0x28,'vertical')]:struct.pack_into('>i',record,off,before[key])
    struct.pack_into('>b',record,0x22,speed);struct.pack_into('>b',record,0x5a,before['hold'])
    u.mem_write(0x42000,bytes(record));u.mem_write(a5-0x1342,struct.pack('>I',0x44000));u.mem_write(0x4409a,struct.pack('>h',energy))
    u.mem_write(a5-0x3f8e+0x20,struct.pack('>b',setting))
    for offset,key,fmt in [(0x131a,'hover','i'),(0x20c0,'camera_scale','i'),(0x20c2,'renderer_height','h')]:
        u.mem_write(a5-offset,struct.pack('>'+fmt,before[key]))
    u.mem_write(a5-0x1315,bytes([before['active']]));u.mem_write(a5-0x134c,struct.pack('>i',now))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A4,0x42000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D7,elapsed&0xffffffff)]:u.reg_write(reg,value)
    u.emu_start(0x8145a,0x81668,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x81668
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    result=dict(before)
    for offset,key,fmt in [(0x131a,'hover','i'),(0x20c0,'camera_scale','i'),(0x20c2,'renderer_height','h')]:result[key]=struct.unpack('>'+fmt,u.mem_read(a5-offset,struct.calcsize(fmt)))[0]
    result['active']=bool(bytes(u.mem_read(a5-0x1315,1))[0])
    expected_record=bytearray(record)
    for off,key,fmt in [(0x24,'transition','i'),(0x28,'vertical','i'),(0x5a,'hold','b')]:
        result[key]=struct.unpack('>'+fmt,u.mem_read(0x42000+off,struct.calcsize(fmt)))[0]
        struct.pack_into('>'+fmt,expected_record,off,result[key])
    assert bytes(u.mem_read(0x42000,128))==expected_record
    cases.append(dict(routine='update_vertical',a=energy,b=speed,setting=setting,clocks=[now,elapsed],before=before,result=result,expected=0))
print('2048 original transition-plus-hover blocks: camera globals, participant height, transition clamps, hold and record preservation checked')
# Solid collision record response after the original movement rollback service.
for iteration in range(2048):
    owner=rng.randint(1,12);moving=rng.randint(1,16);other=17;boundary=bool(rng.randrange(2))
    height=rng.choice([0,-1,1,2,3,60<<16,2**31-1,rng.randint(-2**31,2**31-1)])
    other_height=rng.choice([0,height,height-1 if height>-2**31 else height,rng.randint(-2**31,2**31-1)])
    records=bytearray(rng.randbytes(17*128));offset=(moving-1)*128
    struct.pack_into('>i',records,offset+0x28,height);struct.pack_into('>i',records,(other-1)*128+0x28,other_height)
    before=list(records[offset:offset+128]);u.mem_write(0x42000,bytes(records))
    participants=rng.randbytes(12*256);u.mem_write(0x45000,participants)
    u.mem_write(0x45000+(owner-1)*256+0x56,struct.pack('>h',moving))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    u.mem_write(0x57000-8,struct.pack('>h',-999 if boundary else other))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D7,owner)]:u.reg_write(reg,value)
    u.emu_start(0x11be6,0x11d1e,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x11d1e
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    after=bytes(u.mem_read(0x42000+offset,128));expected_records=bytearray(records);expected_records[offset:offset+128]=after
    assert bytes(u.mem_read(0x42000,len(records)))==expected_records
    cases.append(dict(routine='contact_rebound',a=int(boundary),b=other_height,before=before,after=list(after),expected=0))
print('2048 original contact rebound blocks: boundary bypass, positive-height hold, signed overflow, movement reversal and record preservation checked')
# Single-player solid response gate and ordered terminal service packets.
solid_calls=[]
def solid_service(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    if address==a5+0x10aa:
        contact,owner=struct.unpack('>hb',uc.mem_read(sp+4,3));argc=4
        solid_calls.append(['notify',contact,owner])
    elif address==a5+0x972:
        channel,request=struct.unpack('>hI',uc.mem_read(sp+4,6));argc=6
        solid_calls.append(['sound',channel,request])
    else:
        elapsed=struct.unpack('>i',uc.mem_read(sp+4,4))[0]
        owner=struct.unpack('>b',uc.mem_read(sp+8,1))[0];moving=struct.unpack('>h',uc.mem_read(sp+10,2))[0];argc=8
        solid_calls.append(['rollback',moving,owner,elapsed])
    uc.reg_write(UC_M68K_REG_A7,sp+4+argc);uc.reg_write(UC_M68K_REG_PC,ret)
def solid_end(uc,address,size,data):uc.emu_stop()
solid_hooks=[u.hook_add(UC_HOOK_CODE,solid_service,begin=a5+off,end=a5+off) for off in [0x10aa,0x972,0x116a]]
solid_hooks += [u.hook_add(UC_HOOK_CODE,solid_end,begin=address,end=address) for address in [0x11be6,0x11d4c]]
u.ctl_flush_tb()
for iteration in range(2048):
    special=rng.choice([0,0,0,1,255]);hold=rng.choice([0,0,1,30,255]);owner=rng.randint(1,12);local=bool(rng.randrange(2))
    contact=rng.choice([-999,17]);elapsed=rng.choice([0,1,4,-1,-2**31,2**31-1,rng.randint(-2**31,2**31-1)])
    moving=rng.randint(1,16);records=bytearray(rng.randbytes(17*128));records[(moving-1)*128+0x5a]=hold
    u.mem_write(0x42000,bytes(records));u.mem_write(0x45000+(owner-1)*256+0x56,struct.pack('>h',moving))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    u.mem_write(a5-0x133d,bytes([owner if local else (owner%12)+1]));u.mem_write(a5-0x12ca,b'\0')
    u.mem_write(0x57000-8,struct.pack('>h',contact));u.mem_write(0x57000+8,struct.pack('>i',elapsed))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000),(UC_M68K_REG_D7,owner),(UC_M68K_REG_D5,special)]:u.reg_write(reg,value)
    solid_calls.clear();u.emu_start(0x11adc,0x40000,count=10000)
    assert u.reg_read(UC_M68K_REG_PC) in [0x11be6,0x11d4c]
    assert u.reg_read(UC_M68K_REG_A7)==0x56000, (iteration, hex(u.reg_read(UC_M68K_REG_A7)), hex(u.reg_read(UC_M68K_REG_PC)), solid_calls, special, hold, contact)
    assert bytes(u.mem_read(0x42000,len(records)))==records
    result=None
    if solid_calls:
        assert solid_calls[0]==['notify',contact,owner]
        assert solid_calls[-1][:3]==['rollback',moving,owner]
        if local:assert len(solid_calls)==3 and solid_calls[1][0]=='sound'
        else:assert len(solid_calls)==2
        result=[solid_calls[-1][3],solid_calls[1][1:] if local else None]
    cases.append(dict(routine='solid_response_request',a=special,b=contact,hold=hold,local=local,elapsed=elapsed,result=result,expected=0))
for hook in solid_hooks:u.hook_del(hook)
print('2048 original single-player solid response gates: ordered notification/sound/rollback packets, hold and boundary gates, signed elapsed and record preservation checked')
# Complete local type-6 movement dispatcher, only final spatial-index service captured.
for disp,target in [(0x1282,0x90004),(0x128a,0x90038),(0x452,0x7002c),(0x45a,0x7003c),(0x522,0x10004),(0xbba,0x20d24),(0xe02,0xf05b8)]:
    u.mem_write(a5+disp,bytes.fromhex('4ef9')+struct.pack('>I',target))
step_grid=[]
def step_grid_service(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    new,old,identity=struct.unpack('>IIh',uc.mem_read(sp+4,10))
    step_grid.append([identity,list(struct.unpack('>iii',uc.mem_read(old,12))),list(struct.unpack('>iii',uc.mem_read(new,12)))])
    uc.reg_write(UC_M68K_REG_A7,sp+14);uc.reg_write(UC_M68K_REG_PC,ret)
step_hook=u.hook_add(UC_HOOK_CODE,step_grid_service,begin=0x80f60,end=0x80f60)
u.ctl_flush_tb()
step_table=[int(65536*__import__('math').sin(i*__import__('math').pi/180)) for i in range(360)]
u.mem_write(a5-0x38be,struct.pack('>I',0x47000));u.mem_write(0x47000,struct.pack('>360i',*step_table))
for iteration in range(512):
    now=rng.randint(0,100000);elapsed=rng.choice([-8,-4,-1,0,1,4,8]);limit=rng.randint(1,9);energy=rng.choice([0,400])
    before=dict(pose=[rng.randint(100,900)<<16,rng.randint(100,900)<<16,rng.randrange(360<<16)],speed=rng.randint(-limit,limit),ramp=rng.randint(0,8),
        vertical=dict(transition=rng.choice([0,65536,-65536]),camera_scale=4*65536,renderer_height=32,hover=rng.randint(0,200),hold=rng.choice([0,1,30]),vertical=0,active=bool(rng.randrange(2))))
    commands=[rng.choice([-1,0,1]),rng.choice([-1,0,1])];rate=65536
    inputs=dict(commands=commands,limit=limit,energy=energy,turn_rate=rate,now=now,elapsed=elapsed)
    record=bytearray(128);struct.pack_into('>iii',record,0,*before['pose']);struct.pack_into('>hh',record,0xc,*commands)
    record[0x14]=6;record[0x1a]=1;struct.pack_into('>bb',record,0x22,before['speed'],before['ramp'])
    for off,key in [(0x24,'transition'),(0x28,'vertical')]:struct.pack_into('>i',record,off,before['vertical'][key])
    record[0x5a]=before['vertical']['hold'];u.mem_write(0x42000,bytes(record))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x1342,struct.pack('>I',0x45000));u.mem_write(0x45000,bytes(256))
    u.mem_write(0x4506c,struct.pack('>i',rate));u.mem_write(0x4509a,struct.pack('>h',energy));u.mem_write(a5-0x3cf8,b'\0')
    u.mem_write(a5-0x133d,b'\1');u.mem_write(a5-0x12ca,b'\0');u.mem_write(a5-0x3d32,b'\0');u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.mem_write(a5-0x3f8e+0x20,bytes([limit]));u.mem_write(a5-0x3f8e+0x23,b'\0')
    for off,key,fmt in [(0x20c0,'camera_scale','i'),(0x20c2,'renderer_height','h'),(0x131a,'hover','i')]:u.mem_write(a5-off,struct.pack('>'+fmt,before['vertical'][key]))
    u.mem_write(a5-0x1315,bytes([before['vertical']['active']]))
    step_grid.clear();run(0x81d54,struct.pack('>iBBh',elapsed,1,0,1));assert u.reg_read(UC_M68K_REG_A7)==stack+12
    after=bytes(u.mem_read(0x42000,128));vertical=dict(before['vertical'])
    for off,key,fmt in [(0x20c0,'camera_scale','i'),(0x20c2,'renderer_height','h'),(0x131a,'hover','i')]:vertical[key]=struct.unpack('>'+fmt,u.mem_read(a5-off,struct.calcsize(fmt)))[0]
    vertical['active']=bool(bytes(u.mem_read(a5-0x1315,1))[0])
    for off,key,fmt in [(0x24,'transition','i'),(0x28,'vertical','i'),(0x5a,'hold','b')]:vertical[key]=struct.unpack_from('>'+fmt,after,off)[0]
    result=dict(pose=list(struct.unpack_from('>iii',after)),speed=struct.unpack_from('>b',after,0x22)[0],ramp=struct.unpack_from('>b',after,0x23)[0],vertical=vertical)
    assert step_grid==([[1,before['pose'],result['pose']]] if elapsed else [])
    cases.append(dict(routine='type6_step',a=0,b=0,before=before,input=inputs,result=result,table=step_table,expected=0))
u.hook_del(step_hook)
print('512 complete local type-6 dispatchers: positive/negative/zero elapsed, original turn/speed/vertical/getter routines; only spatial-index service captured')
# Both complete vertical transit contact handlers; only sound playback captured.
transit_sounds=[]
def transit_sound(uc,address,size,data):
    sp=uc.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',uc.mem_read(sp,4))[0]
    transit_sounds.append(list(struct.unpack('>hI',uc.mem_read(sp+4,6))))
    uc.reg_write(UC_M68K_REG_A7,sp+10);uc.reg_write(UC_M68K_REG_PC,ret)
transit_hook=u.hook_add(UC_HOOK_CODE,transit_sound,begin=a5+0x972,end=a5+0x972)
u.ctl_flush_tb()
for iteration in range(2048):
    kind=36+iteration%2;owner=rng.randint(1,12);moving=rng.randint(1,16)
    transition=rng.choice([0,65536,-65536,rng.randint(-2**31,2**31-1)])
    vertical=rng.choice([0,-1,1,-96<<16,rng.randint(-2**31,2**31-1)]);energy=rng.randint(-32768,32767)
    records=bytearray(rng.randbytes(16*128));participants=bytearray(rng.randbytes(12*256));off=(moving-1)*128;part=(owner-1)*256
    struct.pack_into('>ii',records,off+0x24,transition,vertical)
    struct.pack_into('>h',participants,part+0x56,moving);struct.pack_into('>h',participants,part+0x9a,energy)
    u.mem_write(0x42000,bytes(records));u.mem_write(0x45000,bytes(participants))
    u.mem_write(a5-0x1346,struct.pack('>I',0x42000));u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
    transit_sounds.clear();run(0x1125c if kind==36 else 0x112b8,struct.pack('>hBx',17,owner))
    assert u.reg_read(UC_M68K_REG_A7)==stack+8
    next_transition=struct.unpack('>i',u.mem_read(0x42000+off+0x24,4))[0];next_energy=struct.unpack('>h',u.mem_read(0x45000+part+0x9a,2))[0]
    struct.pack_into('>i',records,off+0x24,next_transition);struct.pack_into('>h',participants,part+0x9a,next_energy)
    assert bytes(u.mem_read(0x42000,len(records)))==records and bytes(u.mem_read(0x45000,len(participants)))==participants
    assert len(transit_sounds)<=1
    cases.append(dict(routine='transit_contact',a=kind,b=transition,vertical=vertical,energy=energy,result=[next_transition,next_energy,transit_sounds[0] if transit_sounds else None],expected=0))
u.hook_del(transit_hook)
assert [0x1774+struct.unpack_from('>h',code7,0x1776+2*(kind-13))[0] for kind in [36,37]]==[0x181a,0x1828]
print('2048 complete original transit handlers: dispatch targets, upward height gate, downward energy clearing, sound packets, record/participant preservation and stack checked')
# Complete behavior-4/9 tracking handler; only cached target-pose retrieval is supplied.
for displacement,target in [(0xb2,0x120882),(0x4ca,0x71760),(0x1282,0x90004)]:
    u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
tracking_hook=u.hook_add(UC_HOOK_CODE,tracking_pose_service,begin=0x1702e4,end=0x1702e4)
u.ctl_flush_tb()
for iteration in range(1024):
    records=bytearray(rng.randbytes(12*256));record=records[264:392]
    mode=rng.choice([0,1,2,255]);active=rng.choice([0,1,128,255]);records[256+0x88]=active
    difficulty=rng.choice([0,1,127,-32768,32767,rng.randint(-32768,32767)])
    geometry=bytearray(rng.randbytes(128));source_pose=[rng.randint(-2**31,2**31-1) for _ in range(3)]
    struct.pack_into('>iii',geometry,0,*source_pose)
    struct.pack_into('>h',records,256+0x56,1)
    tracking_supplied=[rng.randint(-2**31,2**31-1) for _ in range(3)];tracking_calls=0
    before=full_tracking_states(records)[1]
    u.mem_write(0x42000,bytes(records));u.mem_write(0x44000,bytes(geometry))
    u.mem_write(a5-0x1342,struct.pack('>I',0x42000));u.mem_write(a5-0x1346,struct.pack('>I',0x44000))
    u.mem_write(a5-0x12ca,bytes([mode]));u.mem_write(a5-0x4256,struct.pack('>h',difficulty))
    run(0x170ef4,bytes([2,0]))
    assert u.reg_read(UC_M68K_REG_A7)==stack+6
    after_records=bytes(u.mem_read(0x42000,12*256));after=full_tracking_states(after_records)[1]
    permitted=bytearray(records)
    for lo,hi in [(265,268),(272,273),(276,288),(298,302)]:permitted[lo:hi]=after_records[lo:hi]
    assert bytes(permitted)==after_records
    assert bytes(u.mem_read(0x44000,128))==geometry
    cases.append(dict(routine='ai_attack_tracking',a=mode,b=difficulty,before=before,after=after,
                      source=source_pose,supplied=tracking_supplied,called=bool(tracking_calls),expected=0))
u.hook_del(tracking_hook)
print('1024 complete attack-tracking handlers: original distance, difficulty getter and multiply; cached pose supplied; record preservation and stack checked')
# Complete radar scale initialization arithmetic, original long multiply/divide.
for displacement,target in [(0x1282,0x90004),(0x128a,0x90038)]:u.mem_write(a5+displacement,bytes.fromhex('4ef9')+struct.pack('>I',target))
u.ctl_flush_tb()
for iteration in range(1024):
    extent=rng.choice([256,512,1024,2048,32767,-32768,rng.randint(-32768,32767)])
    packed=(((extent+80)*8+32768)%65536)-32768
    doubled=((packed*packed*2+2**31)%2**32)-2**31
    limit=abs(doubled)//65536 * (-1 if doubled<0 else 1)
    if limit==156:continue # Explicit host guard; do not execute original zero-divisor path.
    u.mem_write(a5-0x137e,struct.pack('>h',extent))
    for reg,value in [(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,0x57000),(UC_M68K_REG_A7,0x56000)]:u.reg_write(reg,value)
    u.emu_start(0x190284,0x19030c,count=10000);assert u.reg_read(UC_M68K_REG_PC)==0x19030c
    assert u.reg_read(UC_M68K_REG_A7)==0x56000
    fields=['packed_extent','distance_limit','outer_distance','threshold','scale_base','far_scale','falloff']
    result={name:struct.unpack('>i',u.mem_read(a5-offset,4))[0] for name,offset in zip(fields,[0x1388,0x138c,0x1390,0x1394,0x1398,0x139c,0x13a0])}
    cases.append(dict(routine='radar_scale',a=extent,b=0,result=result,expected=0))
print('Original radar scale initialization blocks executed with original long arithmetic; all seven globals and stack checked, zero divisors excluded')
report=dict(method='Unicorn 2.1.4 M68000, original bytes, bounded execution, synthetic index marker tables, candidate collision tables from modeled SANE, and supplied Random trap results',
            hashes={f'CODE{segment}':hashlib.sha256((source/f'{segment}.bin').read_bytes()).hexdigest()
                    for segment in [1,6,7,10,11,12,14,15,17,18,19,20,21,22,23,24,25,28,30,31,33,36]},
            movement_marker_table=table,cases=cases)
output=root/'.local/disassembly/recovered-vectors.json'
output.parent.mkdir(parents=True,exist_ok=True)
output.write_text(json.dumps(report,indent=2)+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
