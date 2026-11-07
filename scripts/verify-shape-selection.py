#!/usr/bin/env python3
"""Execute each original shape-table column-selection branch on local CD data."""
from pathlib import Path
import json
import struct
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN
from unicorn.m68k_const import *
root=Path(__file__).resolve().parent.parent
code=(root/'.local/game-resources/434f4445/1.bin').read_bytes()
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000,0x50000)
u.mem_write(0x10000,code)
u.mem_write(0x41000,struct.pack('>I',0x42000))
reports={}
animations={}
frame_count=0
count=0
for campaign in ['central','eastern','south']:
    # Read exported resources. Export paths differ from early exploratory files.
    from subprocess import run
    out=root/f'.local/{campaign}-shape-resources'
    run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','export',str(root/f'.local/extracted/{campaign}.bin'),str(out)],cwd=root,check=True,capture_output=True)
    data=(out/'73687073/10000.bin').read_bytes()
    n=struct.unpack_from('>H',data)[0]
    assert len(data)==2+n*6
    u.mem_write(0x42000,data)
    rows=[]
    for kind in range(n):
        row=[]
        for column,entry in enumerate([0x1262,0x124c,0x1278]):
            u.reg_write(UC_M68K_REG_A3,0x41000)
            u.reg_write(UC_M68K_REG_D6,kind)
            u.emu_start(0x10000+entry,0x1128c,count=100)
            assert u.reg_read(UC_M68K_REG_PC)==0x1128c
            actual=u.reg_read(UC_M68K_REG_D7)&0xffff
            if actual>=32768: actual-=65536
            assert actual==struct.unpack_from('>h',data,2+kind*6+column*2)[0]
            row.append(actual);count+=1
        rows.append(row)
    reports[campaign]=rows
    animations[campaign]={}
    for path in sorted((out/'61736870').glob('*.bin')):
        data=path.read_bytes()
        u.mem_write(0x42000,data)
        u.reg_write(UC_M68K_REG_A4,0x41000)
        u.reg_write(UC_M68K_REG_A6,0x50000)
        u.emu_start(0x115de,0x115e4,count=100)
        n=struct.unpack('>h',u.mem_read(0x4fffe,2))[0]
        assert len(data)==2+n*2
        frames=[]
        for i in range(n):
            u.reg_write(UC_M68K_REG_D6,i)
            u.emu_start(0x115e8,0x115f4,count=100)
            assert u.reg_read(UC_M68K_REG_PC)==0x115f4
            value=u.reg_read(UC_M68K_REG_D7)&0xffff
            if value>=32768: value-=65536
            assert value==struct.unpack_from('>h',data,2+i*2)[0]
            frames.append(value)
            frame_count+=1
        animations[campaign][path.stem]=frames
(root/'.local/disassembly/shape-selection.json').write_text(json.dumps(reports,indent=2)+'\n')
(root/'.local/disassembly/animation-selection.json').write_text(json.dumps(animations,indent=2)+'\n')
print(f'{count} original-instruction shape selections match the three-column decoder.')
print(f'{frame_count} original-instruction animation frame selections match the ordered resource decoder.')
