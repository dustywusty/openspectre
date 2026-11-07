#!/usr/bin/env python3
"""Original render-queue height conversion, including software division."""
import hashlib,json,random,struct,subprocess
from pathlib import Path
from unicorn import Uc,UC_ARCH_M68K,UC_MODE_BIG_ENDIAN
from unicorn.m68k_const import UC_CPU_M68K_M68000,UC_M68K_REG_A3,UC_M68K_REG_A5,UC_M68K_REG_A6,UC_M68K_REG_A7,UC_M68K_REG_PC
root=Path(__file__).resolve().parent.parent
resources=root/'.local/game-resources/434f4445'
code=(resources/'6.bin').read_bytes();division=(resources/'36.bin').read_bytes()
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000);u.mem_map(0x10000,0x70000)
u.mem_write(0x10000,code);u.mem_write(0x20000,division)
a5,frame,stack,record=0x60000,0x50000,0x40000,0x70000
u.mem_write(a5+0x128a,bytes.fromhex('4ef9')+struct.pack('>I',0x20038))
# The kind-29 dispatch uses +2400, copying raw height +28 into local -1a20.
assert 0x1be0+struct.unpack_from('>h',code,0x1be2+2*(29-2))[0]==0x2400
assert code[0x2400:0x2406].hex()=='2d6c0028e5e0'
rng=random.Random(0x62a10);cases=[]
for i in range(2048):
    height=([-2147483648,-65537,-65536,-65535,-1,0,1,65535,65536,65537,2147483647][i%11] if i<110 else rng.randint(-2147483648,2147483647))
    original=rng.randbytes(64);u.mem_write(record,original);u.mem_write(frame-0x1a20,struct.pack('>i',height))
    for reg,value in [(UC_M68K_REG_A3,record),(UC_M68K_REG_A5,a5),(UC_M68K_REG_A6,frame),(UC_M68K_REG_A7,stack)]:u.reg_write(reg,value)
    u.emu_start(0x12a10,0x12a24,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==0x12a24 and u.reg_read(UC_M68K_REG_A7)==stack
    result=bytes(u.mem_read(record,64));assert result[:10]==original[:10] and result[12:]==original[12:]
    cases.append(dict(routine='render_height_word',a=height,b=0,expected=struct.unpack_from('>h',result,10)[0]))
p=root/'.local/disassembly/render-height-vectors.json'
p.write_text(json.dumps(dict(method='Original CODE6 render queue height conversion and CODE36 division; kind29 dispatch provenance checked; no service stubs',hashes={'CODE6':hashlib.sha256(code).hexdigest(),'CODE36':hashlib.sha256(division).hexdigest()},movement_marker_table=[0]*360,cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(p)],cwd=root,check=True)
print('2048 original height conversions match Rust; signed fractional boundaries, record preservation and stack checked')
