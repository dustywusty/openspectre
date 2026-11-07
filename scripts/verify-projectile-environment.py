#!/usr/bin/env python3
"""Original post-query environment gate, including the complete boundary routine."""
import hashlib, json, random, struct, subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A4, UC_M68K_REG_A5, UC_M68K_REG_A7, UC_M68K_REG_D5, UC_M68K_REG_D7, UC_M68K_REG_PC
root = Path(__file__).resolve().parent.parent
binary = (root / '.local/game-resources/434f4445/30.bin').read_bytes()
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0x60000)
u.mem_write(0x10000, binary)
a5, record_address, stack = 0x30000, 0x40000, 0x60000
u.mem_write(a5-0x1346, struct.pack('>I', record_address))
def endpoint(cpu, address, size, data):
    cpu.emu_stop()
for address in [0x12808, 0x12814]:
    u.hook_add(UC_HOOK_CODE, endpoint, begin=address, end=address)
rng = random.Random(0x302788)
cases = []
for iteration in range(2048):
    record = bytearray(rng.randbytes(128))
    record[0x14] = [9, 28, 29, 32, 33, 34, 43, 44, 46][iteration % 9]
    extent = rng.choice([0, 1, 65536, 800<<16, -1, -2147483648, 2147483647])
    positions = [0, -1, 1, extent, max(-2147483648, extent-1), min(2147483647, extent+1)]
    for offset in [0, 4]: struct.pack_into('>i', record, offset, rng.choice(positions))
    for offset in [0x24, 0x28]: struct.pack_into('>i', record, offset, rng.choice([0, -1, 1, -2147483648, 2147483647]))
    hit = rng.choice([-32768, -1, 0, 1, 2, 32767])
    u.mem_write(record_address, bytes(record))
    u.mem_write(a5-0x13ac, struct.pack('>i', extent))
    for reg, value in [(UC_M68K_REG_A4, record_address), (UC_M68K_REG_A5, a5), (UC_M68K_REG_A7, stack), (UC_M68K_REG_D5, 1), (UC_M68K_REG_D7, hit & 65535)]:
        u.reg_write(reg, value)
    u.emu_start(0x12788, 0x12816, count=10000)
    pc = u.reg_read(UC_M68K_REG_PC)
    assert pc in [0x12808, 0x12814]
    assert u.reg_read(UC_M68K_REG_A7) == stack
    assert bytes(u.mem_read(record_address, 128)) == record
    cases.append(dict(routine='projectile_environment_contact', a=extent, b=hit, record=list(record), expected=int(pc == 0x12808)))
output = root / '.local/disassembly/projectile-environment-vectors.json'
output.write_text(json.dumps(dict(method='Original CODE30 +2788..+2814 and complete +261e boundary routine; post-movement record and selected collision supplied; no service stubs', hashes={'CODE30':hashlib.sha256(binary).hexdigest()}, movement_marker_table=[0]*360, cases=cases))+'\n')
subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'spectre-tools', '--', 'verify-recovered', str(output)], cwd=root, check=True)
print('2048 original environment gates match Rust; boundary/height limits, hit priority, record preservation and stack verified')
