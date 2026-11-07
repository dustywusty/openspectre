#!/usr/bin/env python3
"""Compare original CODE 12 render dispatch order with Rust using owned disc bytes.
Terminal drawing is intercepted. Preparation blocks execute original transforms and lookup services; rasterization remains outside this test.
"""
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import (UC_CPU_M68K_M68000, UC_M68K_REG_A5,
    UC_M68K_REG_A6, UC_M68K_REG_A7, UC_M68K_REG_A3, UC_M68K_REG_A4, UC_M68K_REG_D6, UC_M68K_REG_D5, UC_M68K_REG_D7, UC_M68K_REG_PC)

root = Path(__file__).resolve().parent.parent
code = (root / '.local/game-resources/434f4445/12.bin').read_bytes()
u = Uc(UC_ARCH_M68K, UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000, 0x70000)
base, a5, a6, stack = 0x10000, 0x60000, 0x50000, 0x40000
u.mem_write(base, code)
observed = []

def drawing(machine, address, size, data):
    if address not in (base + 0x1428, base + 0xfda):
        return
    sp = machine.reg_read(UC_M68K_REG_A7)
    argc = 36 if address == base + 0x1428 else 28
    index = machine.reg_read(UC_M68K_REG_D7) & 0xffff
    pointer = struct.unpack('>I', machine.mem_read(sp + argc, 4))[0]
    assert pointer == 0x70000 + index * 16, (index, hex(pointer))
    observed.append(index - 1)
    ret = struct.unpack('>I', machine.mem_read(sp, 4))[0]
    machine.reg_write(UC_M68K_REG_A7, sp + 4 + argc)
    machine.reg_write(UC_M68K_REG_PC, ret)

u.hook_add(UC_HOOK_CODE, drawing)
u.ctl_flush_tb()
rng = random.Random(12001991)
fixtures = [[], [-1], [0], [1], [2], [-128, 127],
            [1, -1, 2, 0, 1, -1], [0] * 100, [127] * 100, [-128] * 100]
fixtures += [[value] for value in range(-128, 128)]
fixtures += [[rng.randint(-128, 127) for _ in range(rng.randrange(101))] for _ in range(512)]
cases = []
for classes in fixtures:
    for filled in (False, True):
        records = bytearray(rng.randbytes(100 * 32))
        for index, layer in enumerate(classes):
            struct.pack_into('>h', records, index * 32 + 6, 1)
            struct.pack_into('>b', records, index * 32 + 0x1b, layer)
            u.mem_write(a5 - 0x6620 + index * 4, struct.pack('>I', 0x70000 + (index + 1) * 16))
        u.mem_write(a5 - 0x648e, struct.pack('>I', 0x78000))
        # Prepared records begin at local -0xc94 (one-based index times 32).
        u.mem_write(a6 - 0xc94, bytes(records))
        u.mem_write(a6 - 0xc9d, struct.pack('>b', min([0] + classes)))
        u.mem_write(a6 - 0xc9e, struct.pack('>b', max([1] + classes)))
        u.mem_write(a5 - 0x7be2, bytes([filled]))
        for reg, value in [(UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, a6),
                           (UC_M68K_REG_A7, stack), (UC_M68K_REG_D5, len(classes))]:
            u.reg_write(reg, value)
        observed.clear()
        u.emu_start(base + 0x1dd4, base + 0x1efe, count=1000000)
        assert u.reg_read(UC_M68K_REG_PC) == base + 0x1efe
        assert u.reg_read(UC_M68K_REG_A7) == stack
        assert bytes(u.mem_read(a6 - 0xc94, len(records))) == records
        assert sorted(observed) == list(range(len(classes)))
        cases.append(dict(routine='render_dispatch_order', a=0, b=0, expected=0,
                          classes=classes, filled=filled, result=observed.copy()))
dispatch_count = len(cases)
# Original insertion block. Supply the later per-record initialization after each insertion.
# Every moved byte and pointer association is checked, not only the resulting keys.
key_fixtures = [[], [0], [32767, -32768, 0, -1, 1], [5] * 100,
                list(range(100)), list(reversed(range(100)))]
key_fixtures += [[rng.choice([-32768, 32767, 0, -1, 1, rng.randint(-32768, 32767)])
                  for _ in range(rng.randrange(101))] for _ in range(256)]
for keys in key_fixtures:
    original = []
    pointer_base = a5 - 0x6620
    for index, key in enumerate(keys):
        u.mem_write(pointer_base + index * 4, struct.pack('>I', 0x70000 + index * 16))
        u.mem_write(0x30006, struct.pack('>h', key))
        for reg, value in [(UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, a6),
                           (UC_M68K_REG_A7, stack), (UC_M68K_REG_A4, 0x30000),
                           (UC_M68K_REG_D5, index)]:
            u.reg_write(reg, value)
        u.emu_start(base + 0x1a5e, base + 0x1af6, count=100000)
        assert u.reg_read(UC_M68K_REG_PC) == base + 0x1af6
        assert u.reg_read(UC_M68K_REG_A7) == stack
        assert u.reg_read(UC_M68K_REG_D5) == index + 1
        insertion = (u.reg_read(UC_M68K_REG_D6) & 0xffff) - 1
        record = bytearray(rng.randbytes(32))
        struct.pack_into('>h', record, 0, key)
        original.append(bytes(record))
        u.mem_write(a6 - 0xc94 + insertion * 32, bytes(record))
        pointers = struct.unpack('>' + 'I' * (index + 1), u.mem_read(pointer_base, (index + 1) * 4))
        order = [(pointer - 0x70000) // 16 for pointer in pointers]
        assert sorted(order) == list(range(index + 1))
        actual = bytes(u.mem_read(a6 - 0xc94, (index + 1) * 32))
        assert actual == b''.join(original[i] for i in order)
    if not keys:
        order = []
    cases.append(dict(routine='render_preparation_order', a=0, b=0, expected=0,
                      keys=keys, result=order))
# Complete overlap-marking block, including its bytewise detail/filled gate.
for iteration in range(1024):
    count = rng.randrange(1, 101)
    inserted = rng.randrange(count)
    intervals = [[rng.randint(-32768, 32767), rng.randint(-32768, 32767)] for _ in range(count)]
    if iteration % 2 == 0:
        intervals = [sorted(pair) for pair in intervals]
    if count > 1 and iteration % 3 == 0:
        intervals[(inserted + 1) % count] = intervals[inserted].copy()
    flags = [rng.choice([0, 1, 2, 255]) for _ in range(count)]
    detail, filled = [rng.choice([0, 1, 2, 127, 128, 255]) for _ in range(2)]
    records = bytearray(rng.randbytes(count * 32))
    for index, (low, high) in enumerate(intervals):
        records[index * 32 + 9] = flags[index]
        struct.pack_into('>hh', records, index * 32 + 10, low, high)
    u.mem_write(a6 - 0xc94, bytes(records))
    u.mem_write(a6 + 8, bytes([detail]))
    u.mem_write(a5 - 0x7be2, bytes([filled]))
    low, high = intervals[inserted]
    u.mem_write(a6 - 0x10, struct.pack('>hh', high, low))
    for reg, value in [(UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, a6),
                       (UC_M68K_REG_A7, stack), (UC_M68K_REG_D5, count),
                       (UC_M68K_REG_D6, inserted + 1)]:
        u.reg_write(reg, value)
    u.emu_start(base + 0x1d56, base + 0x1dc2, count=100000)
    assert u.reg_read(UC_M68K_REG_PC) == base + 0x1dc2
    assert u.reg_read(UC_M68K_REG_A7) == stack
    after = bytearray(u.mem_read(a6 - 0xc94, count * 32))
    result = [after[index * 32 + 9] for index in range(count)]
    for index, flag in enumerate(flags):
        after[index * 32 + 9] = flag
    assert after == records
    cases.append(dict(routine='render_overlap_flags', a=inserted, b=detail, expected=0,
                      intervals=intervals, flags=flags, filled=filled, result=result))
# Complete prepared-record construction, with the original +0140 direction helper.
u.ctl_flush_tb()  # The new stop address was an entry point in the preceding test.
for iteration in range(1024):
    input_record = bytearray(rng.randbytes(64))
    model_id = rng.randint(1, 100)
    struct.pack_into('>h', input_record, 0, model_id)
    u.mem_write(0x30000, bytes(input_record))
    model = dict(face_count=rng.choice([-32768, -1, 0, 1, 32767]),
                 edge_split=rng.choice([-32768, -1, 0, 1, 32767]), mode=rng.randrange(256))
    u.mem_write(a5 - 0x648e + (model_id - 1) * 4, struct.pack('>I', 0x78000))
    u.mem_write(0x7825c, struct.pack('>hh', model['face_count'], model['edge_split']))
    u.mem_write(0x78265, bytes([model['mode']]))
    direction_table = [rng.randint(-2**31, 2**31 - 1) for _ in range(360)]
    u.mem_write(0x74000, struct.pack('>360i', *direction_table))
    u.mem_write(a5 - 0x6246, struct.pack('>I', 0x74000))
    interval = [rng.randint(-32768, 32767) for _ in range(2)]
    u.mem_write(a6 - 0x10, struct.pack('>hh', interval[1], interval[0]))
    classes = [rng.randint(-128, 127) for _ in range(2)]
    u.mem_write(a6 - 0xc9d, struct.pack('>b', classes[0]))
    u.mem_write(a6 - 0xc9e, struct.pack('>b', classes[1]))
    slot = rng.randint(1, 100)
    address = a6 - 0xcb4 + slot * 32
    before = rng.randbytes(32)
    u.mem_write(address - 4, b'HEAD' + before + b'TAIL')
    for reg, value in [(UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, a6),
                       (UC_M68K_REG_A7, stack), (UC_M68K_REG_A4, 0x30000),
                       (UC_M68K_REG_D6, slot)]:
        u.reg_write(reg, value)
    u.emu_start(base + 0x1c20, base + 0x1d56, count=100000)
    assert u.reg_read(UC_M68K_REG_PC) == base + 0x1d56
    assert u.reg_read(UC_M68K_REG_A7) == stack
    assert bytes(u.mem_read(0x30000, 64)) == input_record
    assert bytes(u.mem_read(address - 4, 4)) == b'HEAD'
    assert bytes(u.mem_read(address + 32, 4)) == b'TAIL'
    output_record = list(u.mem_read(address, 32))
    after_classes = [struct.unpack('>b', u.mem_read(a6 - offset, 1))[0] for offset in [0xc9d, 0xc9e]]
    cases.append(dict(routine='prepare_render_record', a=0, b=0, expected=0,
                      input=list(input_record), before=list(before), model=model, interval=interval,
                      classes=classes, table=direction_table, result=[output_record, after_classes]))
# Integrated transform/setup, interval scan, and record construction. No services stubbed.
code11 = (root / '.local/game-resources/434f4445/11.bin').read_bytes()
u.mem_write(0x20000, code11)
u.mem_write(a5 + 0x752, bytes.fromhex('4ef9') + struct.pack('>I', 0x23ba8))
u.ctl_flush_tb()
for iteration in range(512):
    vertex_count = rng.randrange(0, 25)
    vertices = [[rng.randint(-32768, 32767) for _ in range(3)] for _ in range(vertex_count)]
    capacity = vertex_count + rng.randrange(0, 6)
    initial_vertices = [[rng.randint(-32768, 32767) for _ in range(5)] for _ in range(capacity)]
    input_record = bytearray(rng.randbytes(64))
    override = iteration % 2
    struct.pack_into('>hh', input_record, 0, 1, override)
    model = dict(face_count=rng.choice([-1, 0, 1, 5]), edge_split=rng.choice([-1, 0, 1, 5]), mode=rng.randrange(256))
    scan_count = rng.randint(0, capacity) if override else vertex_count
    config = dict(model=model, camera_height=rng.randint(-32768, 32767), scan_count=scan_count,
                  detail=rng.choice([0, 1, 2, 255]), filled=rng.choice([0, 1, 2, 255]))
    state = dict(vertices=initial_vertices, record=list(rng.randbytes(32)),
                 interval=[rng.randint(-32768, 32767) for _ in range(2)],
                 classes=[rng.randint(-128, 127) for _ in range(2)])
    direction_table = [rng.randint(-2**31, 2**31 - 1) for _ in range(360)]
    u.mem_write(0x30000, bytes(input_record))
    u.mem_write(0x74000, struct.pack('>360i', *direction_table))
    u.mem_write(a5 - 0x6246, struct.pack('>I', 0x74000))
    u.mem_write(a5 - 0x648e, struct.pack('>I', 0x78000))
    u.mem_write(a5 - 0x62fa, struct.pack('>I', 0x79000))
    u.mem_write(a5 - 0x6620, struct.pack('>I', 0x70000))
    geometry = struct.pack('>h', vertex_count) + b''.join(struct.pack('>5h', *v, 0, 0) for v in vertices)
    u.mem_write(0x79000 if override else 0x78000, geometry)
    u.mem_write(0x78000, struct.pack('>h', scan_count))
    u.mem_write(0x7825c, struct.pack('>hh', model['face_count'], model['edge_split']))
    u.mem_write(0x78265, bytes([model['mode']]))
    initial_output = struct.pack('>h', 123) + b''.join(struct.pack('>5h', *v) for v in initial_vertices)
    u.mem_write(0x70000, initial_output + b'TAIL')
    u.mem_write(a6 - 2, struct.pack('>h', config['camera_height']))
    u.mem_write(a6 + 8, bytes([config['detail']]))
    u.mem_write(a5 - 0x7be2, bytes([config['filled']]))
    u.mem_write(a6 - 0x10, struct.pack('>hh', state['interval'][1], state['interval'][0]))
    u.mem_write(a6 - 0xc9d, struct.pack('>b', state['classes'][0]))
    u.mem_write(a6 - 0xc9e, struct.pack('>b', state['classes'][1]))
    u.mem_write(a6 - 0xc94, bytes(state['record']))
    for reg, value in [(UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, a6), (UC_M68K_REG_A7, stack),
                       (UC_M68K_REG_A4, 0x30000), (UC_M68K_REG_D6, 1)]:
        u.reg_write(reg, value)
    u.emu_start(base + 0x1af6, base + 0x1d56, count=100000)
    assert u.reg_read(UC_M68K_REG_PC) == base + 0x1d56
    assert u.reg_read(UC_M68K_REG_A7) == stack
    assert bytes(u.mem_read(0x30000, 64)) == input_record
    assert bytes(u.mem_read(0x70000 + len(initial_output), 4)) == b'TAIL'
    assert struct.unpack('>h', u.mem_read(0x70000, 2))[0] == vertex_count
    result = dict(vertices=[list(struct.unpack('>5h', u.mem_read(0x70002 + index * 10, 10))) for index in range(capacity)],
                  record=list(u.mem_read(a6 - 0xc94, 32)),
                  interval=[struct.unpack('>h', u.mem_read(a6 - offset, 2))[0] for offset in [0xe, 0x10]],
                  classes=[struct.unpack('>b', u.mem_read(a6 - offset, 1))[0] for offset in [0xc9d, 0xc9e]])
    cases.append(dict(routine='prepare_render_geometry', a=0, b=0, expected=0, input=list(input_record),
                      vertices=vertices, table=direction_table, config=config, state=state, result=result))
# Complete metadata setters, including selected-model lookup and Pascal stack cleanup.
u.ctl_flush_tb()
for iteration in range(1024):
    before = rng.randbytes(0x804)
    slot = rng.randint(1, 100)
    u.mem_write(0x78000, before)
    u.mem_write(a5 - 0x6490, struct.pack('>h', slot))
    u.mem_write(a5 - 0x648e + (slot - 1) * 4, struct.pack('>I', 0x78000))
    split = rng.randint(-32768, 32767)
    references = [rng.randint(-32768, 32767) for _ in range(2)]
    mode = rng.randrange(256)
    configure, enable = iteration % 3 != 0, iteration % 3 != 1
    for target, arguments in ([(0x8ee, bytes([mode, 0]) + struct.pack('>hhh', references[1], references[0], split))] if configure else []) + ([(0x926, b'')] if enable else []):
        u.mem_write(stack, struct.pack('>I', 0x35000) + arguments)
        u.reg_write(UC_M68K_REG_A7, stack)
        u.reg_write(UC_M68K_REG_A5, a5)
        u.emu_start(base + target, 0x35000, count=10000)
        assert u.reg_read(UC_M68K_REG_PC) == 0x35000
        assert u.reg_read(UC_M68K_REG_A7) == stack + 4 + len(arguments)
    after = bytes(u.mem_read(0x78000, 0x804))
    fields = dict(face_count=struct.unpack_from('>h', after, 0x25c)[0],
                  edge_split=struct.unpack_from('>h', after, 0x25e)[0], mode=after[0x265])
    cases.append(dict(routine='model_render_metadata', a=split, b=mode, expected=0,
                      configure=configure, enable=enable, references=references,
                      before=list(before), result=list(after), fields=fields))
# Resource top-view plane selection, including the original CODE 19 slot setter.
code1 = (root / '.local/game-resources/434f4445/1.bin').read_bytes()
code19 = (root / '.local/game-resources/434f4445/19.bin').read_bytes()
u.mem_write(0x30000, code1)
u.mem_map(0x80000, 0x10000)
u.mem_write(0x80000, code19)
u.mem_write(a5 + 0xca2, bytes.fromhex('4ef9') + struct.pack('>I', 0x83e1e))
u.ctl_flush_tb()
for iteration in range(1024):
    stored = rng.choice([-32768, -2, -1, 0, 1, 8, 9, 32767, rng.randint(-32768, 32767)])
    last_line = rng.choice([-32768, -1, 0, 7, 8, 32767, rng.randint(-32768, 32767)])
    format_byte = rng.choice([0, 1, 2, 3, 255])
    version = rng.choice([0, 15, 16, 255, rng.randrange(256)])
    slot = rng.randint(1, 100)
    u.mem_write(0x70000, struct.pack('>h', stored))
    u.mem_write(a6 + 8, struct.pack('>h', slot))
    u.mem_write(a6 - 0x112, struct.pack('>h', last_line))
    u.mem_write(a5 - 0x414c, bytes([format_byte, version]))
    slots_before = rng.randbytes(200)
    u.mem_write(a5 - 0x218a, slots_before)
    for reg, value in [(UC_M68K_REG_A5, a5), (UC_M68K_REG_A6, a6),
                       (UC_M68K_REG_A7, stack), (UC_M68K_REG_A3, 0x70000)]:
        u.reg_write(reg, value)
    u.emu_start(0x3192a, 0x3197c, count=10000)
    assert u.reg_read(UC_M68K_REG_PC) == 0x3197c
    assert u.reg_read(UC_M68K_REG_A7) == stack
    count = struct.unpack('>h', u.mem_read(a5 - 0x218a + (slot - 1) * 2, 2))[0]
    after = bytearray(u.mem_read(a5 - 0x218a, 200))
    after[(slot - 1) * 2:slot * 2] = slots_before[(slot - 1) * 2:slot * 2]
    assert after == slots_before
    consumed = u.reg_read(UC_M68K_REG_A3) - 0x70000
    assert consumed in [0, 2]
    cases.append(dict(routine='resource_plane_sides', a=stored, b=last_line, expected=0,
                      format=format_byte, version=version, result=[count, consumed == 2]))
report = dict(method='Original CODE 12 insertion, record construction, overlap marking, and dispatch blocks; terminal draws captured',



              hashes={'CODE12': hashlib.sha256(code).hexdigest(), 'CODE11': hashlib.sha256(code11).hexdigest(), 'CODE1': hashlib.sha256(code1).hexdigest(), 'CODE19': hashlib.sha256(code19).hexdigest()}, cases=cases)
output = root / '.local/disassembly/render-dispatch-vectors.json'
output.write_text(json.dumps(report, indent=2) + '\n')
subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'spectre-tools', '--',
                'verify-recovered', str(output)], cwd=root, check=True)
print(f'{dispatch_count} original dispatch vectors and {len(key_fixtures)} insertion sequences plus 1024 overlap, 1024 record-construction, 512 integrated geometry blocks, 1024 metadata setter cases, and 1024 plane-selection cases: order, record/pointer association, and stack checked')
