#!/usr/bin/env python3
"""Compare original prediction and firing-angle routing with Rust.
Prediction has no service stubs; routing supplies terminal angle services.
"""
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A7, UC_M68K_REG_A5, UC_M68K_REG_PC, UC_M68K_REG_A4, UC_M68K_REG_D4, UC_M68K_REG_D5, UC_M68K_REG_D7

root = Path(__file__).resolve().parent.parent
resources = root / '.local/game-resources/434f4445'
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0xf0000)
segments = {20: 0x10000, 18: 0x20000, 36: 0x30000, 11: 0x40000, 12: 0x50000}
for segment, address in segments.items():
    u.mem_write(address, (resources / f'{segment}.bin').read_bytes())
a5, stack, stop = 0xa0000, 0x90000, 0x80000
for offset, address in [(0xbba, 0x20d24), (0x1282, 0x30004), (0x128a, 0x30038), (0x75a, 0x43c5a), (0x78a, 0x50020)]:
    u.mem_write(a5 + offset, bytes.fromhex('4ef9') + struct.pack('>I', address))
u.mem_write(a5 - 0x1342, struct.pack('>I', 0xc0000))
u.mem_write(a5 - 0x1346, struct.pack('>I', 0xb0000))
u.mem_write(a5 - 0x38be, struct.pack('>I', 0xd0000))
u.mem_write(a5 - 0x7b99, b'\0')
rng = random.Random(0x200040)
# Explicit synthetic inputs: no claim about original SANE table generation.
direction = [rng.randint(-65536, 65536) for _ in range(360)]
u.mem_write(0xd0000, struct.pack('>360i', *direction))
cases = []
for iteration in range(2048):
    source = [rng.randint(-2**31, 2**31-1) for _ in range(3)]
    target = [rng.randint(-2**31, 2**31-1), rng.randint(-2**31, 2**31-1), rng.randrange(360 << 16)]
    speed = rng.choice([v for v in range(-128, 128) if v not in [-24, 24]])
    mode = [0, 1, 2, 255][iteration % 4]
    records = bytearray(rng.randbytes(12*256))
    geometry = bytearray(rng.randbytes(12*128))
    source_id, target_id = rng.sample(range(1, 13), 2)
    for participant in range(12):
        struct.pack_into('>h', records, participant*256+0x56, participant+1)
    struct.pack_into('>iii', geometry, (source_id-1)*128, *source)
    struct.pack_into('>iii', geometry, (target_id-1)*128, *target)
    geometry[(target_id-1)*128+0x22] = speed & 255
    u.mem_write(0xb0000, bytes(geometry));u.mem_write(0xc0000, bytes(records))
    sentinel = rng.randbytes(28);u.mem_write(0xe0000, sentinel)
    args = struct.pack('>BxBxBxI', mode, target_id, source_id, 0xe0008)
    u.mem_write(stack, struct.pack('>I', stop) + args)
    u.reg_write(UC_M68K_REG_A7, stack);u.reg_write(UC_M68K_REG_A5, a5)
    u.emu_start(0x10040, stop, count=10000)
    assert u.reg_read(UC_M68K_REG_PC) == stop
    # Pascal routine consumes the three byte arguments, retaining output pointer.
    assert u.reg_read(UC_M68K_REG_A7) == stack+10
    output = bytes(u.mem_read(0xe0000, 28))
    assert output[:8] == sentinel[:8] and output[20:] == sentinel[20:]
    assert bytes(u.mem_read(0xb0000, len(geometry))) == geometry
    assert bytes(u.mem_read(0xc0000, len(records))) == records
    cases.append(dict(routine='ai_predict_pose', a=speed, b=mode, source_pose=source, target_pose=target,
                      table=direction, after=list(struct.unpack('>iii', output[8:20])), expected=0))
# Original firing angle routing, with terminal cached/direct angle services supplied.
angle_calls = []
def angle_service(cpu, address, size, data):
    sp = cpu.reg_read(UC_M68K_REG_A7)
    ret = struct.unpack('>I', cpu.mem_read(sp, 4))[0]
    cached = address == 0x1031c
    if cached:
        assert cpu.mem_read(sp+4, 1)[0] == source_id
        result_slot = sp+6
    else:
        pointers = struct.unpack('>II', cpu.mem_read(sp+4, 8))
        assert pointers == (0xb0000+(target_id-1)*128, 0xb0000+(source_id-1)*128)
        result_slot = sp+12
    angle_calls.append(cached)
    cpu.mem_write(result_slot, struct.pack('>h', cached_angle if cached else direct_angle))
    cpu.reg_write(UC_M68K_REG_A7, result_slot)
    cpu.reg_write(UC_M68K_REG_PC, ret)
hooks = [u.hook_add(UC_HOOK_CODE, angle_service, begin=address, end=address)
         for address in [0x1031c, a5+0x4c2]]
u.ctl_flush_tb()
for source_id in range(1, 13):
    for target_id in range(1, 13):
        for movement in [0, 1, 2, 3, 255]:
            records = bytearray(rng.randbytes(12*256))
            for participant in range(12):
                struct.pack_into('>h', records, participant*256+0x56, participant+1)
            records[(source_id-1)*256+0x10] = movement
            u.mem_write(0xc0000, bytes(records))
            cached_angle, direct_angle = rng.sample(range(-32768, 32768), 2)
            angle_calls.clear()
            for register, value in [(UC_M68K_REG_A7, stack), (UC_M68K_REG_A5, a5),
                                    (UC_M68K_REG_A4, 0xc0000+(source_id-1)*256+8),
                                    (UC_M68K_REG_D4, target_id), (UC_M68K_REG_D7, source_id)]:
                u.reg_write(register, value)
            u.emu_start(0x1212a, 0x1219a, count=1000)
            assert u.reg_read(UC_M68K_REG_PC) == 0x1219a
            assert u.reg_read(UC_M68K_REG_A7) == stack
            assert len(angle_calls) == 1
            expected_angle = cached_angle if angle_calls[0] else direct_angle
            assert u.reg_read(UC_M68K_REG_D5) & 65535 == expected_angle & 65535
            assert bytes(u.mem_read(0xc0000, len(records))) == records
            assert bytes(u.mem_read(0xb0000, len(geometry))) == geometry
            cases.append(dict(routine='ai_fire_uses_cached_angle', a=target_id, b=source_id,
                              movement=movement, expected=int(angle_calls[0])))
for hook in hooks:
    u.hook_del(hook)
report = dict(method='Complete original prediction with original integer math and synthetic unit table; zero-denominator speeds excluded. Firing routing supplies terminal cached/direct angle services.',
              hashes={f'CODE{segment}': hashlib.sha256((resources / f'{segment}.bin').read_bytes()).hexdigest() for segment in segments},
              movement_marker_table=direction, cases=cases)
output = root / '.local/disassembly/ai-prediction-vectors.json'
output.write_text(json.dumps(report) + '\n')
subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'spectre-tools', '--', 'verify-recovered', str(output)], cwd=root, check=True)
print('2048 complete original prediction calls; all arithmetic and direction lookup original; geometry, participants, output bounds and stack checked')

print('720 original firing-angle routing blocks; cached/direct services supplied, argument pointers, result angle, records and stack checked')
