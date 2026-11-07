#!/usr/bin/env python3
"""Compare integrated Game movement with composed original instructions.
Conditional on the candidate direction table; original SANE is not verified.
The arena is empty, with balanced Coast-0 settings and no special effects.
"""
import importlib.util, json, struct, subprocess
from pathlib import Path
root=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('oracle',root/'scripts/verify-recovered.py')
o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
u,a5=o.u,o.a5
candidate=json.loads((root/'.local/disassembly/direction-table-candidate.json').read_text())['movement']
u.mem_write(0x43000,struct.pack('>360i',*candidate))
u.mem_write(a5-0x38be,struct.pack('>I',0x43000))
u.mem_write(a5-0x1342,struct.pack('>I',0x45000))
u.mem_write(0x45000,bytes(256))
u.mem_write(a5-0x133d,b'\x01')
u.mem_write(a5-0x12ca,b'\0')
u.mem_write(a5-0x3f8e+0x20,b'\x05')
u.mem_write(a5-0x3f8e+0x23,b'\0')
u.mem_write(a5-0x20c2,struct.pack('>h',32))
u.mem_write(a5-0x20c0,bytes(4))
u.mem_write(a5-0x131a,bytes(4))
u.mem_write(a5-0x3d32,b'\0')
record=bytearray(128)
struct.pack_into('>iii',record,0,496<<16,496<<16,90<<16)
record[0x14]=6;record[0x1a]=1
u.mem_write(0x42000,bytes(record))
frames=[]
# Acceleration, turning while moving, coasting stop, reversal, turning in place.
for throttle,steer,count in [(1,0,30),(1,1,60),(0,0,15),(-1,-1,60),(0,1,30),(0,0,15)]:
    for _ in range(count):
        turn=-steer
        u.mem_write(0x4200c,struct.pack('>hh',turn,throttle))
        o.run(0x810b8,struct.pack('>iih',1,turn*65536,1))
        o.run(0x81370,struct.pack('>hhhihh',0,0,0,1,0,1))
        x,z,heading=struct.unpack('>iii',u.mem_read(0x42000,12))
        speed,ramp=struct.unpack('>bb',u.mem_read(0x42022,2))
        frames.append(dict(throttle=throttle,steer=steer,expected=[[x,z],heading,speed,ramp]))
p=root/'.local/disassembly/handling-replay.json';p.write_text(json.dumps(frames)+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-handling',str(p)],cwd=root,check=True)
