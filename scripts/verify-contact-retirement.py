#!/usr/bin/env python3
"""Original contact-retirement flag and complete response, with movement captured."""
import hashlib,json,random,struct,subprocess,sys
from pathlib import Path
from unicorn import Uc,UC_ARCH_M68K,UC_MODE_BIG_ENDIAN,UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000,UC_M68K_REG_A5,UC_M68K_REG_A7,UC_M68K_REG_PC
root=Path(__file__).resolve().parent.parent;resources=root/'.local/game-resources/434f4445'
bouncing = '--bouncing' in sys.argv
flight = '--flight' in sys.argv or bouncing
# Static provenance checks: startup passes SysEnvRec at A5-7b96 to jt_028a.
# Its machineType word at +2 is the flag input A5-7b94.
startup=(resources/'3.bin').read_bytes()
wrapper=(resources/'1.bin').read_bytes()
assert startup[0x818:0x826].hex()=='2f3c00010000486d846a4ead028a'
assert wrapper[0x29ac:0x29b6].hex()=='225f205f301f2f09a090'
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000);u.mem_map(0x10000,0xf0000)
segments={30:0x10000,25:0x20000,6:0x30000,18:0x40000,36:0x50000}
for segment,address in segments.items():u.mem_write(address,(resources/f'{segment}.bin').read_bytes())
a5,stack,stop=0xa0000,0x90000,0x80000
u.mem_write(a5+0x452,bytes.fromhex('4ef9')+struct.pack('>I',0x3002c))
u.mem_write(a5-0x1346,struct.pack('>I',0xb0000));u.mem_write(a5-0x1342,struct.pack('>I',0xc0000))
for offset,address in [(0xbb2,0x40cd0),(0x1282,0x50004),(0x128a,0x50038)]:
    u.mem_write(a5+offset,bytes.fromhex('4ef9')+struct.pack('>I',address))
direction_table = [(i * 7919) - 1400000 for i in range(360)]
u.mem_write(0xd0000,struct.pack('>360i',*direction_table))
u.mem_write(a5-0x38ba,struct.pack('>I',0xd0000))
calls=[];deleted=[];grid_calls=[]
def service(cpu,address,size,data):
    sp=cpu.reg_read(UC_M68K_REG_A7);ret=struct.unpack('>I',cpu.mem_read(sp,4))[0]
    if address==0x11d54:
        elapsed,owner,obj=struct.unpack('>iBxh',cpu.mem_read(sp+4,8));assert obj==1
        record=bytearray(cpu.mem_read(0xb0000,128));assert owner==record[0x1a]
        calls.append([elapsed,record[0x14],owner])
        if flight: return
        x=struct.unpack_from('>i',record,0)[0];struct.pack_into('>I',record,0,(x+elapsed)&0xffffffff)
        cpu.mem_write(0xb0000,bytes(record));size=8
    elif address==0x10f60:
        current,previous,obj=struct.unpack('>IIh',cpu.mem_read(sp+4,10))
        grid_calls.append([list(struct.unpack('>iii',cpu.mem_read(previous,12))),list(struct.unpack('>iii',cpu.mem_read(current,12))),obj])
        size=10
    else:
        deleted.append(struct.unpack('>h',cpu.mem_read(sp+4,2))[0]);size=2
    cpu.reg_write(UC_M68K_REG_A7,sp+4+size);cpu.reg_write(UC_M68K_REG_PC,ret)
for address in [0x11d54,0x10f60,a5+0xe62]:u.hook_add(UC_HOOK_CODE,service,begin=address,end=address)
rng=random.Random(0x302676);cases=[]
for iteration in range(1024):
    mode=rng.choice(list(range(-1,18))+[-32768,32767]);elapsed=rng.choice([-2147483648,-32768,-1,0,1,2,3,63,65535,65536,2147483647])
    now=rng.choice([-2147483648,0,100,2147483647]);record=bytearray(rng.randbytes(128));record[0x1a]=rng.randrange(13)
    if flight:
        record[0x14]=29 if bouncing else 9
        struct.pack_into('>h',record,0x1c,6 if bouncing else rng.choice([0,1,3,4,5,17]))
        if bouncing:
            for offset in [0x24,0x28]:
                struct.pack_into('>i',record,offset,rng.choice([0,1,-1,65536,-65536,-2147483648,2147483647,rng.randint(-10000000,10000000)]))
        heading=rng.randrange(360)
        struct.pack_into('>i',record,8,heading<<16)
        direction=[direction_table[(90-heading)%360],direction_table[heading]]
    counts=[rng.choice([-32768,-1,0,1,32767]) for _ in range(12)]
    participants=bytearray(rng.randbytes(12*256))
    for i,count in enumerate(counts):struct.pack_into('>h',participants,i*256+0x58,count)
    u.mem_write(0xb0000,bytes(record));u.mem_write(0xc0000,bytes(participants));u.mem_write(a5-0x134c,struct.pack('>i',now))
    u.mem_write(a5-0x7b94,struct.pack('>h',mode));u.reg_write(UC_M68K_REG_A5,a5);u.reg_write(UC_M68K_REG_A7,stack)
    u.emu_start(0x22d04,0x22d2a,count=1000);assert u.reg_read(UC_M68K_REG_PC)==0x22d2a
    flag=u.mem_read(a5-0x3cf7,1)[0]
    u.mem_write(stack,struct.pack('>Iih',stop,elapsed,1));calls.clear();deleted.clear();grid_calls.clear()
    u.emu_start(0x12676,stop,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==stop and u.reg_read(UC_M68K_REG_A7)==stack+10
    after_counts=[struct.unpack('>h',u.mem_read(0xc0000+i*256+0x58,2))[0] for i in range(12)]
    expected_participants=participants.copy()
    for i,count in enumerate(after_counts):struct.pack_into('>h',expected_participants,i*256+0x58,count)
    assert bytes(u.mem_read(0xc0000,12*256))==expected_participants
    assert deleted==([] if flag else [1])
    if flight:
        moved = bool(flag and (elapsed & 65535))
        assert len(grid_calls)==int(moved)
        if moved:
            assert grid_calls[0]==[list(struct.unpack_from('>iii',record)),list(struct.unpack('>iii',u.mem_read(0xb0000,12))),1]
    cases.append(dict(routine='projectile_contact_retirement',a=elapsed,b=now,mode=mode,record=list(record),counts=counts,
                      after=list(u.mem_read(0xb0000,128)),after_counts=after_counts,calls=calls.copy(),action='Retire' if flag else 'Delete',expected=0))
    if flight: cases[-1]['direction']=direction
output=root/('.local/disassembly/contact-retirement-bouncing-vectors.json' if bouncing else '.local/disassembly/contact-retirement-flight-vectors.json' if flight else '.local/disassembly/contact-retirement-vectors.json')
output.write_text(json.dumps(dict(method=('Original CODE25 hardware flag and full CODE30 contact response/movement; kind29 weapon6' if bouncing else 'Original CODE25 hardware flag and full CODE30 contact response/movement; kind9 simple weapons') + '; original direction lookup, arithmetic, and minimum; supplied table; terminal grid relocation and deletion captured' if flight else 'Original CODE25 hardware flag and complete CODE30 contact response; signed minimum; movement callback supplies a coordinate mutation; deletion captured',hashes={str(k):hashlib.sha256((resources/f'{k}.bin').read_bytes()).hexdigest() for k in segments},movement_marker_table=[0]*360,cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
print('1024 original flag/contact-response cases match Rust; rewind arguments and ordering, record fields, owner counts and stack verified')
