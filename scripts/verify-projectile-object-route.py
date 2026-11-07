#!/usr/bin/env python3
"""Execute original selected-object routing with all byte-valued target kinds."""
import hashlib, json, struct, subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import UC_CPU_M68K_M68000, UC_M68K_REG_A4, UC_M68K_REG_A5, UC_M68K_REG_A6, UC_M68K_REG_A7, UC_M68K_REG_D7, UC_M68K_REG_PC
root = Path(__file__).resolve().parent.parent
binary = (root / '.local/game-resources/434f4445/30.bin').read_bytes()
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0x70000)
u.mem_write(0x10000, binary)
response = (root / '.local/game-resources/434f4445/31.bin').read_bytes()
u.mem_write(0x70000, response)
a5, records_address, participants_address, stack, frame = 0x30000, 0x40000, 0x50000, 0x60000, 0x61000
u.mem_write(a5-0x1346, struct.pack('>I', records_address))
u.mem_write(a5-0x1342, struct.pack('>I', participants_address))
endpoints = {0x12a72:'Ignore', 0x128ee:'DamageParticipant', 0x12a26:'Delete', 0x12a12:'RespondToObject'}
def endpoint(cpu, address, size, data): cpu.emu_stop()
for address in endpoints: u.hook_add(UC_HOOK_CODE, endpoint, begin=address, end=address)
cases = []
response_calls = 0
for projectile_kind in [9, 28, 29, 32, 33, 34, 43, 44, 46]:
    for target_kind in range(256):
        for hit, health in [(2, 1), (2, 0), (2, -32768), (0, 32767), (-1, 1)]:
            records = bytearray(256)
            records[0x14] = projectile_kind
            records[128+0x14] = target_kind
            records[128+0x1a] = 1
            participants = bytearray(256)
            struct.pack_into('>h', participants, 0x5a, health)
            u.mem_write(records_address, bytes(records))
            u.mem_write(participants_address, bytes(participants))
            for reg, value in [(UC_M68K_REG_A4, records_address), (UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, frame), (UC_M68K_REG_A7, stack), (UC_M68K_REG_D7, hit & 65535)]:
                u.reg_write(reg, value)
            u.emu_start(0x12814, 0x12a74, count=10000)
            pc = u.reg_read(UC_M68K_REG_PC)
            assert pc in endpoints and u.reg_read(UC_M68K_REG_A7) == stack
            assert bytes(u.mem_read(records_address, 256)) == records
            assert bytes(u.mem_read(participants_address, 256)) == participants
            if endpoints[pc] == 'RespondToObject':
                # The shipped entry jumps directly to its epilogue. Execute it,
                # rather than assuming the unreachable counter/damage body runs.
                u.mem_write(stack, struct.pack('>IBxih', 0x68000, 1, 12345, 2))
                u.emu_start(0x710b0, 0x68000, count=1000)
                assert u.reg_read(UC_M68K_REG_PC) == 0x68000
                assert u.reg_read(UC_M68K_REG_A7) == stack+12
                assert bytes(u.mem_read(records_address, 256)) == records
                assert bytes(u.mem_read(participants_address, 256)) == participants
                response_calls += 1
            cases.append(dict(routine='projectile_object_contact', a=projectile_kind, b=hit, target_kind=target_kind, health=health, action=endpoints[pc], expected=0))
output = root / '.local/disassembly/projectile-object-route-vectors.json'
output.write_text(json.dumps(dict(method='Original CODE30 selected-object routing through health gate; no service stubs; damage and object response stop before execution', hashes={'CODE30':hashlib.sha256(binary).hexdigest(), 'CODE31':hashlib.sha256(response).hexdigest()}, object_response_calls=response_calls, movement_marker_table=[0]*360, cases=cases))+'\n')
subprocess.run(['cargo','run','--quiet','--locked','-p','spectre-tools','--','verify-recovered',str(output)], cwd=root, check=True)
print(f'{len(cases)} original object routes match Rust; all target bytes, nine projectile kinds, health gates, record preservation and stack checked')
print(f'{response_calls} complete original object-response calls leave geometry and participant records unchanged')
