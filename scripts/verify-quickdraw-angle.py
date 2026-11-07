#!/usr/bin/env python3
"""Research verification against CHM's historical QuickDraw source.
Source and assembled reference stay in ignored .local; no ROM is distributed.
Requires the CHM source checkout, local vasm, and the existing Unicorn environment.
"""
import hashlib
import json
import random
import re
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN
from unicorn.m68k_const import UC_M68K_REG_A7, UC_M68K_REG_PC, UC_CPU_M68K_M68000

root = Path(__file__).resolve().parent.parent
source = root / '.local/quickdraw-reference/102658076_quickdraw_acc/QuickDraw Source/Angles.a'
original = source.read_bytes()
body = source.read_text().split('        .PROC')[0]
lines = ['        org $180000', 'AngleFromSlope:']
for line in body.splitlines():
    if re.match(r'\s*\.(INCLUDE|FUNC|DEF)', line):
        continue
    line = line.replace('.BYTE', 'dc.b').replace('.WORD', 'dc.w')
    line = line.replace('SLOPE-91(D0)', 'SLOPE-91(pc,d0.w)').replace('SLOPE(D0)', 'SLOPE(pc,d0.w)')
    if re.match(r'^[A-Za-z]', line):
        line = re.sub(r'^(\w+)', r'\1:', line)
    lines.append(line)
assembly = root / '.local/quickdraw-angle.s'
binary = root / '.local/quickdraw-angle.bin'
assembly.write_text('\n'.join(lines) + '\n')
subprocess.run([str(root/'.local/vasm/vasmm68k_mot'), '-m68000', '-Fbin', '-o', str(binary), str(assembly)], check=True)
low = [int(v,16) for v in re.findall(r'^\s*\.WORD\s+\$([0-9A-F]+)', body, re.M)]
high = [int(v,16) for v in re.findall(r'^\s*\.BYTE\s+\$([0-9A-F]+)', body, re.M)]
assert len(low)==91 and len(high)==28
slopes=[]
for angle,word in enumerate(low):
    integer=0 if angle<45 else 1 if angle<64 else high[angle-63]
    if integer&128:integer |= 0x7f00
    slopes.append((integer<<16)|word)
assert slopes==sorted(slopes) and slopes[-1]==2**31-1
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN);u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000,0x190000);u.mem_write(0x180000,binary.read_bytes())
stack,stop=0x50000,0x40000
rng=random.Random(1984)
values={-2**31,2**31-1,-1,0,1}
for threshold in slopes:
    for offset in [-1,0,1]:
        value=((threshold+500+offset+2**31)%2**32)-2**31
        values.add(value);values.add(((-value+2**31)%2**32)-2**31)
values.update(rng.randint(-2**31,2**31-1) for _ in range(1000))
cases=[]
for value in sorted(values):
    u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>Iih',stop,value,-12345))
    u.emu_start(0x180000,stop,count=10000)
    assert u.reg_read(UC_M68K_REG_PC)==stop
    result=struct.unpack('>h',u.mem_read(stack+8,2))[0]
    cases.append(dict(routine='historical_angle_from_slope',a=value,b=0,expected=result))
report=dict(method='Historical QuickDraw source reassembled with vasm; executed with Unicorn M68000. Not System 7 ROM verification.',
    source='https://github.com/computerhistory/Historical-Source-Code-Quick-Draw-Repository',
    source_sha256=hashlib.sha256(original).hexdigest(),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
    positive_slopes=slopes,cases=cases)
output=root/'.local/disassembly/quickdraw-angle-vectors.json'
output.write_text(json.dumps(report,indent=2)+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
print(f'{len(cases)} historical QuickDraw angle cases verified, including both sides of every table boundary')
