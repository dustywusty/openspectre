#!/usr/bin/env python3
"""Execute original weapon-8 targeting and heading; supply only angle conversion."""
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A7, UC_M68K_REG_A5, UC_M68K_REG_A4, UC_M68K_REG_A6, UC_M68K_REG_D5, UC_M68K_REG_D6, UC_M68K_REG_PC
root = Path(__file__).resolve().parent.parent
resources = root / '.local/game-resources/434f4445'
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0xf0000)
segments = {20: 0x10000, 30: 0x60000, 6: 0x70000, 7: 0xf0000}
for segment, address in segments.items():
    u.mem_write(address, (resources / f'{segment}.bin').read_bytes())
a5, stack, frame = 0xa0000, 0x90000, 0x98000
for offset, address in [(0xd5a, 0x10754), (0x4ca, 0x71760), (0x522, 0xf0004)]:
    u.mem_write(a5+offset, bytes.fromhex('4ef9')+struct.pack('>I', address))
u.mem_write(a5-0x1342, struct.pack('>I', 0xc0000))
u.mem_write(a5-0x1346, struct.pack('>I', 0xb0000))
rng = random.Random(0x300382)
cases = []
calls = []
def angle_service(cpu, address, size, data):
    sp = cpu.reg_read(UC_M68K_REG_A7)
    ret = struct.unpack('>I', cpu.mem_read(sp, 4))[0]
    pointers = struct.unpack('>II', cpu.mem_read(sp+4, 8))
    poses = [list(struct.unpack('>iii', cpu.mem_read(p, 12))) for p in pointers]
    calls.append(poses)
    index = next(i for i, p in enumerate(participants) if p['pose'] == poses[0])
    value = final_angles[index] if poses[1] == projectile else angles[index]
    cpu.mem_write(sp+12, struct.pack('>h', value))
    cpu.reg_write(UC_M68K_REG_A7, sp+12);cpu.reg_write(UC_M68K_REG_PC, ret)
u.hook_add(UC_HOOK_CODE, angle_service, begin=a5+0x4c2, end=a5+0x4c2)
for iteration in range(1024):
    owner = rng.randrange(1, 13);game_mode = [0, 1, 4, 12, 255][iteration % 5]
    participants = []
    records = bytearray(rng.randbytes(12*256));geometry = bytearray(rng.randbytes(12*128))
    for i in range(12):
        pose = [rng.choice([0, 150, 200, 201, 299, 300, 301, -300]) << 16, rng.randrange(-80, 81) << 16, i << 16]
        participant = dict(team=rng.randrange(3), owner=rng.choice([0, 1, 2, 255]), active=rng.choice([0, 1, 2, 255]),
                           health=rng.choice([-1, 0, 1, 100]), kind=rng.choice([6, 7, 8, 21]), pose=pose)
        participants.append(participant);off = i*256
        for key, offset in [('team', 4), ('owner', 5), ('kind', 8), ('active', 0x88)]:records[off+offset] = participant[key]
        struct.pack_into('>h', records, off+0x56, i+1);struct.pack_into('>h', records, off+0x5a, participant['health'])
        struct.pack_into('>iii', geometry, i*128, *pose)
    projectile = [rng.randrange(-500, 501) << 16, rng.randrange(-500, 501) << 16, rng.randrange(20, 360) << 16]
    angles = [rng.choice([-32768, -180, -46, -45, 0, 45, 46, 180, 32767]) for _ in range(12)]
    final_angles = [rng.randint(-32768, 32767) for _ in range(12)]
    record = bytearray(rng.randbytes(128));struct.pack_into('>iii', record, 0, *projectile)
    u.mem_write(0xb0000, bytes(geometry));u.mem_write(0xc0000, bytes(records));u.mem_write(0xe0000, bytes(record))
    u.mem_write(a5-0x12ca, bytes([game_mode]));calls.clear()
    for reg, value in [(UC_M68K_REG_A7, stack), (UC_M68K_REG_A6, frame), (UC_M68K_REG_A5, a5),
                       (UC_M68K_REG_A4, 0xe0000), (UC_M68K_REG_D5, owner), (UC_M68K_REG_D6, 8)]:u.reg_write(reg, value)
    u.emu_start(0x6037c, 0x603ee, count=100000)
    assert u.reg_read(UC_M68K_REG_PC) == 0x603ee and u.reg_read(UC_M68K_REG_A7) == stack
    after = bytes(u.mem_read(0xe0000, 128));heading = struct.unpack_from('>i', after, 8)[0]
    record[8:12] = after[8:12];assert bytes(record) == after
    assert bytes(u.mem_read(0xb0000, len(geometry))) == geometry
    assert bytes(u.mem_read(0xc0000, len(records))) == records
    result = [u.reg_read(UC_M68K_REG_D6) & 255, heading]
    cases.append(dict(routine='weapon_target_heading', a=owner, b=game_mode, participants=participants,
                      projectile=projectile, angles=angles, final_angles=final_angles, calls=calls.copy(), result=result, expected=0))
report = dict(method='Original weapon-8 constructor heading branch with complete original target selector, distance and wrapping; only angle results supplied',
              hashes={f'CODE{segment}': hashlib.sha256((resources / f'{segment}.bin').read_bytes()).hexdigest() for segment in segments},
              movement_marker_table=[0]*360, cases=cases)
output = root / '.local/disassembly/weapon-targeting-vectors.json'
output.write_text(json.dumps(report)+'\n')
subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'spectre-tools', '--', 'verify-recovered', str(output)], cwd=root, check=True)
print('1024 composed original weapon targeting/heading blocks; ordered angle arguments, source records, projectile writes and stack checked')
