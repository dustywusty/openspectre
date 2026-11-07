#!/usr/bin/env python3
"""Execute complete original optional-weapon cycling and hint unlock routines."""
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A5, UC_M68K_REG_A7, UC_M68K_REG_PC
root = Path(__file__).resolve().parent.parent
source = root / '.local/game-resources/434f4445/6.bin'
code = source.read_bytes()
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0x60000);u.mem_write(0x10000, code)
a5, stack, stop, parameters = 0x60000, 0x50000, 0x40000, 0x30000
u.mem_write(a5-0x1350, struct.pack('>I', parameters))
rng = random.Random(0x6121e)
cases = []
def execute(address, args):
    u.mem_write(stack, struct.pack('>I', stop)+args)
    u.reg_write(UC_M68K_REG_A5, a5);u.reg_write(UC_M68K_REG_A7, stack)
    u.emu_start(address, stop, count=10000)
    assert u.reg_read(UC_M68K_REG_PC) == stop
    assert u.reg_read(UC_M68K_REG_A7) == stack+4+len(args)
for iteration in range(2048):
    current = iteration % 18
    available = bytearray(rng.randbytes(18))
    if iteration % 4 == 0:available = bytearray([2]*18) # nonzero is not sufficient
    if iteration % 4 == 1:
        available = bytearray(18);available[(iteration//4)%18] = 1
    record = bytearray(rng.randbytes(0x820));record[0x7ea:0x7fc] = available
    u.mem_write(parameters, bytes(record));u.mem_write(a5-0x12fe, bytes([173, current, 219]))
    execute(0x1121e, b'')
    selected = u.mem_read(a5-0x12fd, 1)[0]
    assert bytes(u.mem_read(parameters, len(record))) == record
    assert u.mem_read(a5-0x12fe, 1) == b'\xad' and u.mem_read(a5-0x12fc, 1) == b'\xdb'
    cases.append(dict(routine='cycle_optional_weapon', a=current, b=0, available=list(available), expected=selected))
for iteration in range(2048):
    hint = list(range(8, 21))[iteration%13] if iteration < 1024 else rng.randint(-32768, 32767)
    current = rng.randrange(256);available = bytearray(rng.choice([0, 0, 1, 2, 255]) for _ in range(18))
    record = bytearray(rng.randbytes(0x820));record[0x7ea:0x7fc] = available
    u.mem_write(parameters, bytes(record));u.mem_write(a5-0x12fe, bytes([173, current, 219]))
    execute(0x154ec, struct.pack('>h', hint))
    after = bytes(u.mem_read(parameters, len(record)))
    selected = u.mem_read(a5-0x12fd, 1)[0]
    record[0x7ea:0x7fc] = after[0x7ea:0x7fc];assert bytes(record) == after
    assert u.mem_read(a5-0x12fe, 1) == b'\xad' and u.mem_read(a5-0x12fc, 1) == b'\xdb'
    cases.append(dict(routine='unlock_optional_weapon', a=hint, b=current, available=list(available), after=list(after[0x7ea:0x7fc]), expected=selected))
code31 = (source.parent/'31.bin').read_bytes()
u.mem_write(0x20000, code31)
u.mem_write(a5+0x4b2, bytes.fromhex('4ef9')+struct.pack('>I', 0x1121e))
sounds = []
def sound_service(cpu, address, size, data):
    sp = cpu.reg_read(UC_M68K_REG_A7)
    ret = struct.unpack('>I', cpu.mem_read(sp, 4))[0]
    sounds.append(list(struct.unpack('>hI', cpu.mem_read(sp+4, 6))))
    cpu.reg_write(UC_M68K_REG_A7, sp+10);cpu.reg_write(UC_M68K_REG_PC, ret)
u.hook_add(UC_HOOK_CODE, sound_service, begin=a5+0x972, end=a5+0x972)
u.ctl_flush_tb()
for iteration in range(2048):
    now = rng.choice([-2**31, 2**31-1, rng.randint(-2**31, 2**31-1)])
    elapsed = rng.choice([-2**31, -1, 0, 9, 10, 11, 12, 2**31-1])
    last = ((now-elapsed+2**31) % 2**32)-2**31
    control = rng.choice([-32768, -1, 0, 1, 1, 2, 32767]);selected = iteration % 18
    available = list(rng.randbytes(18)) if iteration % 3 else [0]*18
    u.mem_write(parameters+0x7ea, bytes(available))
    u.mem_write(a5-0x134c, struct.pack('>i', now));u.mem_write(a5-0x3d2c, struct.pack('>i', last))
    u.mem_write(a5-0x6688, struct.pack('>h', control));u.mem_write(a5-0x12fd, bytes([selected]))
    globals_before = bytearray(u.mem_read(a5-0x7000, 0x7000));parameters_before = bytes(u.mem_read(parameters, 0x820))
    sounds.clear();u.reg_write(UC_M68K_REG_A5, a5);u.reg_write(UC_M68K_REG_A7, stack)
    u.emu_start(0x218c2, 0x21902, count=10000)
    assert u.reg_read(UC_M68K_REG_PC) == 0x21902 and u.reg_read(UC_M68K_REG_A7) == stack
    after = bytes(u.mem_read(a5-0x7000, 0x7000))
    for offset, count in [(0x12fd, 1), (0x3d2c, 4)]:
        start = 0x7000-offset;globals_before[start:start+count] = after[start:start+count]
    assert bytes(globals_before) == after
    assert bytes(u.mem_read(parameters, 0x820)) == parameters_before
    assert sounds in [[], [[1, 0x10022]]]
    result = [u.mem_read(a5-0x12fd, 1)[0], struct.unpack('>i', u.mem_read(a5-0x3d2c, 4))[0], bool(sounds)]
    cases.append(dict(routine='optional_weapon_selection', a=now, b=last, control=control, selected=selected,
                      available=available, result=result, expected=0))
shots = []
def shot_service(cpu, address, size, data):
    sp = cpu.reg_read(UC_M68K_REG_A7)
    ret = struct.unpack('>I', cpu.mem_read(sp, 4))[0]
    shots.append(struct.unpack('>b', cpu.mem_read(sp+4, 1))[0])
    cpu.reg_write(UC_M68K_REG_A7, sp+6);cpu.reg_write(UC_M68K_REG_PC, ret)
u.hook_add(UC_HOOK_CODE, shot_service, begin=a5+0x1122, end=a5+0x1122)
u.ctl_flush_tb()
for iteration in range(2048):
    now = rng.choice([-2**31, 2**31-1, rng.randint(-2**31, 2**31-1)])
    elapsed = rng.choice([-2**31, -1, 0, 14, 15, 16, 17, 2**31-1])
    last = ((now-elapsed+2**31) % 2**32)-2**31
    control = rng.choice([-32768, -1, 0, 1, 1, 2, 32767]);selected = 5 if iteration % 2 else iteration % 256
    u.mem_write(a5-0x134c, struct.pack('>i', now));u.mem_write(a5-0x1376, struct.pack('>i', last))
    u.mem_write(a5-0x668e, struct.pack('>h', control));u.mem_write(a5-0x12fd, bytes([selected]))
    before = bytes(u.mem_read(a5-0x7000, 0x7000));shots.clear()
    u.reg_write(UC_M68K_REG_A5, a5);u.reg_write(UC_M68K_REG_A7, stack)
    u.emu_start(0x2187a, 0x218c2, count=1000)
    assert u.reg_read(UC_M68K_REG_PC) == 0x218c2 and u.reg_read(UC_M68K_REG_A7) == stack
    assert bytes(u.mem_read(a5-0x7000, 0x7000)) == before
    cases.append(dict(routine='optional_weapon_fire', a=now, b=last, control=control, selected=selected,
                      result=shots.copy(), expected=0))
report = dict(method='Complete original CODE 6 cycling and hint unlock routines, plus original selection input branch; terminal sound and shot requests captured',
              hashes={'CODE6': hashlib.sha256(code).hexdigest(), 'CODE31': hashlib.sha256(code31).hexdigest()}, movement_marker_table=[0]*360, cases=cases)
output = root / '.local/disassembly/weapon-selection-vectors.json'
output.write_text(json.dumps(report)+'\n')
subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'spectre-tools', '--', 'verify-recovered', str(output)], cwd=root, check=True)
print('4096 complete original cycling/unlock calls; all parameter bytes, adjacent globals and stack checked')

print('2048 original selection input blocks with original cycling; sound requests captured, globals and parameters preserved, stack checked')

print('2048 original optional-fire input blocks; ordered projectile requests captured, all globals and stack checked')
