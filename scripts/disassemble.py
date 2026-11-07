#!/usr/bin/env python3
"""Index classic 68K CODE resources. Listings are linear sweeps, not proven CFGs.
Requires capstone==5.0.7. Generated original-code listings stay under .local.
Offsets in reports are resource-relative (include the 4-byte segment header).
"""
import argparse
import hashlib
import json
import re
import struct
from pathlib import Path
from collections import Counter
import capstone

parser = argparse.ArgumentParser()
parser.add_argument('--input', type=Path, default=Path('.local/game-resources/434f4445'))
parser.add_argument('--output', type=Path, default=Path('.local/disassembly'))
args = parser.parse_args()
segments = {int(p.stem): p.read_bytes() for p in args.input.glob('*.bin')}
zero = segments[0]
above, below, size, base = struct.unpack_from('>4I', zero)
assert len(zero) == 16 + size and size % 8 == 0
entries = []
for n in range(size // 8):
    offset, push, segment, trap = struct.unpack_from('>4H', zero, 16+n*8)
    assert push == 0x3f3c and trap == 0xa9f0, (n, 'unsupported jump entry')
    assert segment in segments and offset+4 < len(segments[segment])
    entries.append(dict(index=n, a5=base+n*8+2, segment=segment, offset=offset+4))
jumps = {e['a5']: e for e in entries}
labels = {(e['segment'],e['offset']): f"jt_{e['a5']:04x}" for e in entries}
engine = capstone.Cs(capstone.CS_ARCH_M68K, capstone.CS_MODE_BIG_ENDIAN | capstone.CS_MODE_M68K_000)
args.output.mkdir(parents=True, exist_ok=True)
calls = []
summary = []
for segment, data in sorted(segments.items()):
    if segment == 0: continue
    table_offset, count = struct.unpack_from('>2H', data)
    assert all(e['segment'] == segment for e in entries[table_offset//8:table_offset//8+count])
    assert sum(e['segment']==segment for e in entries) == count
    lines = ['; Linear sweep: embedded data may decode as instructions.',
             '; CODE-relative offsets include four-byte header; calls via A5 annotated.',
             f'; CODE {segment}, sha256 {hashlib.sha256(data).hexdigest()}']
    # Never let a data-decoded instruction swallow a known jump-table entry.
    anchors = sorted({e['offset'] for e in entries if e['segment'] == segment} | {len(data)})
    pos = 4
    globals_used = Counter()
    while pos < len(data):
        boundary = next(a for a in anchors if a > pos)
        ins = next(engine.disasm(data[pos:min(pos+24,boundary)],pos,count=1),None)
        if ins is None or not ins.size or pos+ins.size > boundary:
            lines.append(f'{pos:06x}: {data[pos:pos+2].hex():24} dc.w ; undecoded')
            pos += 2
            continue
        if (segment,pos) in labels:
            lines.append(f"\n{labels[segment,pos]}:")
        note = ''
        match = re.fullmatch(r'\$([0-9a-f]+)\(a5\)',ins.op_str)
        if ins.mnemonic in ('jsr','jmp') and match:
            target = jumps.get(int(match[1],16))
            if target:
                note = f" ; -> CODE {target['segment']} +0x{target['offset']:04x}"
                calls.append(dict(segment=segment,offset=pos,target=target))
        for g in re.findall(r'-\$([0-9a-f]+)\(a5\)',ins.op_str):
            globals_used['-0x'+g] += 1
        lines.append(f'{pos:06x}: {bytes(ins.bytes).hex():24} {ins.mnemonic:10} {ins.op_str}{note}')
        pos += ins.size
    (args.output/f'CODE-{segment:02}.asm').write_text('\n'.join(lines)+'\n')
    summary.append(dict(segment=segment,bytes=len(data),jump_entries=count,sha256=hashlib.sha256(data).hexdigest(),globals=globals_used))
report = dict(capstone=capstone.__version__,address_convention='CODE resource offset; routine offset in jump entry plus 4',
              caveat='Linear sweep can misinterpret tables and literals. Validate each recovered routine against bytes and branch targets.',
              code0=dict(above_a5=above,below_a5=below,table_size=size,table_a5=base),
              entries=entries,calls=calls,segments=summary)
(args.output/'index.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"{len(summary)} executable segments; {len(entries)} validated jump entries; {len(calls)} candidate cross-segment call sites")
