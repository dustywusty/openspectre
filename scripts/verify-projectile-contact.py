#!/usr/bin/env python3
"""Compare original post-query collision filtering for cannon and Smart Missiles."""
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A4, UC_M68K_REG_A5, UC_M68K_REG_A7, UC_M68K_REG_D7, UC_M68K_REG_PC
root = Path(__file__).resolve().parent.parent
binary = (root / '.local/game-resources/434f4445/7.bin').read_bytes()
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0x50000)
u.mem_write(0x10000, binary)
u.mem_write(0x30000-0x1346, struct.pack('>I', 0x40000))
rng = random.Random(0x70b76)
cases = []
for iteration in range(2048):
    shot = bytearray(rng.randbytes(128))
    target = bytearray(rng.randbytes(128))
    shot[0x14] = [9, 34][iteration % 2]
    target[0x14] = (iteration // 2) % 48
    shot[0x1a] = rng.randrange(13)
    target[0x1a] = shot[0x1a] if iteration % 3 == 0 else rng.randrange(13)
    height = rng.choice([-2147483648, 2147483647, 0, 60<<16, -(60<<16), rng.randint(-10000000, 10000000)])
    delta = rng.choice([0, 1, -1, (60<<16)-1, 60<<16, (60<<16)+1, -(60<<16), -2147483648])
    target_height = (height+delta+2**31) % 2**32-2**31
    struct.pack_into('>i', shot, 0x28, height)
    struct.pack_into('>i', target, 0x28, target_height)
    records = bytes(shot+target)
    u.mem_write(0x40000, records)
    for reg, value in [(UC_M68K_REG_A4, 0x40000), (UC_M68K_REG_A5, 0x30000), (UC_M68K_REG_A7, 0x50000), (UC_M68K_REG_D7, 2)]:
        u.reg_write(reg, value)
    u.emu_start(0x10b76, 0x10d72, count=10000)
    assert u.reg_read(UC_M68K_REG_PC) == 0x10d72
    assert u.reg_read(UC_M68K_REG_A7) == 0x50000
    assert bytes(u.mem_read(0x40000, 256)) == records
    hit = u.reg_read(UC_M68K_REG_D7) & 65535
    assert hit in [2, 65535]
    cases.append(dict(routine='direct_projectile_contact', a=0, b=0, shot=list(shot), target=list(target), expected=int(hit == 2)))
output = root / '.local/disassembly/projectile-contact-vectors.json'
output.write_text(json.dumps(dict(method='Original CODE 7 +0b76..+0d72 with selected hit supplied; no service stubs', hashes={'CODE7':hashlib.sha256(binary).hexdigest()}, movement_marker_table=[0]*360, cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)],cwd=root,check=True)
print('2048 original cannon/missile post-query filters match Rust; all target kinds, signed height boundaries, owner equality, records and stack checked')
