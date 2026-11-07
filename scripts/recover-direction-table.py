#!/usr/bin/env python3
"""Run original table initializer with modeled SANE services.
This is NOT an original-SANE differential oracle. Trigonometry uses Decimal
Taylor evaluation; conversions use explicit binary rounding. Generated own-disc
constants, instruction listings, and results remain local.
"""
from pathlib import Path
from fractions import Fraction
from decimal import Decimal, localcontext
import struct, json, hashlib
from unicorn import Uc, UC_ARCH_M68K, UC_MODE_BIG_ENDIAN, UC_HOOK_CODE
from unicorn.m68k_const import *
root=Path(__file__).resolve().parent.parent
source=root/'.local/game-resources/434f4445'
u=Uc(UC_ARCH_M68K,UC_MODE_BIG_ENDIAN)
u.ctl_set_cpu_model(UC_CPU_M68K_M68000)
u.mem_map(0x10000,0x90000)
for segment,address in [(12,0x10000),(35,0x20000),(36,0x30000)]:
    u.mem_write(address,(source/f'{segment}.bin').read_bytes())
a5,stack,frame=0x60000,0x50000,0x48000
u.reg_write(UC_M68K_REG_A5,a5)
u.reg_write(UC_M68K_REG_A7,stack)
u.reg_write(UC_M68K_REG_A6,frame)
u.mem_write(a5-0x6242,struct.pack('>I',0x42000))
for offset,target in [(0x1272,0x20084),(0x126a,0x20004)]:
    u.mem_write(a5+offset,b'\x4e\xf9'+struct.pack('>I',target))
def word(p): return struct.unpack('>H',u.mem_read(p,2))[0]
def ptr(p): return struct.unpack('>I',u.mem_read(p,4))[0]
def extended(p):
    signexp,mantissa=struct.unpack('>HQ',u.mem_read(p,10))
    if not mantissa: return Fraction(0)
    assert 0 < (signexp&0x7fff) < 0x7fff
    exponent=(signexp&0x7fff)-16383-63
    return (-1 if signexp&0x8000 else 1)*Fraction(mantissa)*Fraction(2)**exponent
def store_extended(p,value):
    value=Fraction(value)
    if not value: u.mem_write(p,bytes(10)); return
    sign=0x8000 if value<0 else 0
    value=abs(value)
    exponent=value.numerator.bit_length()-value.denominator.bit_length()
    if value < Fraction(2)**exponent: exponent-=1
    mantissa=round(value/Fraction(2)**(exponent-63))
    if mantissa==1<<64: mantissa>>=1; exponent+=1
    u.mem_write(p,struct.pack('>HQ',sign|(exponent+16383),mantissa))
def sine(value):
    # Domain is 0..2*pi; 80 decimal digits give ample guard precision for f32.
    with localcontext() as ctx:
        ctx.prec=80
        x=Decimal(value.numerator)/Decimal(value.denominator)
        term=total=x
        for n in range(1,200):
            term=-term*x*x/Decimal((2*n)*(2*n+1))
            total+=term
            if abs(term)<Decimal('1e-75'): break
        return Fraction(total)
def cosine(value):
    with localcontext() as ctx:
        ctx.prec=80
        x=Decimal(value.numerator)/Decimal(value.denominator)
        term=total=Decimal(1)
        for n in range(1,200):
            term=-term*x*x/Decimal((2*n-1)*(2*n))
            total+=term
            if abs(term)<Decimal('1e-75'): break
        return Fraction(total)
counts={}; environment=0

def trap(cpu,address,size,user):
    global environment
    opcode=word(address)
    if opcode not in [0xa9eb,0xa9ec]: return
    sp=cpu.reg_read(UC_M68K_REG_A7); op=word(sp)
    counts[f'{opcode:04x}:{op:04x}']=counts.get(f'{opcode:04x}:{op:04x}',0)+1
    dst=ptr(sp+2)
    consumed=6
    if opcode==0xa9ec:
        assert op in [0x18,0x1a]
        store_extended(dst,(sine if op==0x18 else cosine)(extended(dst)))
    elif op in [0x1010,0x100e,0x200e,0,4,0x2810]:
        src=ptr(sp+6); consumed=10
        if op==0x1010:
            u.mem_write(dst,struct.pack('>f',float(extended(src))))
        elif op==0x100e:
            store_extended(dst,Fraction(struct.unpack('>f',u.mem_read(src,4))[0]))
        elif op==0x200e:
            store_extended(dst,struct.unpack('>h',u.mem_read(src,2))[0])
        elif op==0: store_extended(dst,extended(dst)+extended(src))
        elif op==4: store_extended(dst,extended(dst)*extended(src))
        else:
            assert environment & 0x6000 == 0x6000
            u.mem_write(dst,struct.pack('>i',int(extended(src))))
    elif op==0x17: u.mem_write(dst,struct.pack('>H',environment))
    elif op in [1,0x19]: environment=word(dst)
    elif op==0x1b:
        # Conversion stays within i32 range; requested overflow flag is clear.
        assert word(dst)==4
        u.mem_write(dst,b'\0')
    else: raise RuntimeError(f'Unmodeled SANE opcode {op:x}')
    cpu.reg_write(UC_M68K_REG_A7,sp+consumed)
    cpu.reg_write(UC_M68K_REG_PC,address+2)

u.hook_add(UC_HOOK_CODE,trap)
u.emu_start(0x11fbe,0x12066,count=200000)
assert u.reg_read(UC_M68K_REG_PC)==0x12066
assert counts['a9ec:0018']==360
assert counts['a9eb:1010']==721 and counts['a9eb:100e']==1080
assert counts['a9eb:2810']==360 and environment==0
base=list(struct.unpack('>360i',u.mem_read(0x42000,1440)))
# Scale with the original software fixed-point multiply, CODE 11 +0x3c5a.
u.mem_write(0x70000,(source/'11.bin').read_bytes())
u.mem_write(a5-0x7b99,b'\0')
scaled=[]
for value in base:
    u.reg_write(UC_M68K_REG_A7,stack)
    u.mem_write(stack,struct.pack('>Iiii',0x40000,0x4000,value,0))
    u.emu_start(0x73c5a,0x40000,count=1000)
    assert u.reg_read(UC_M68K_REG_PC)==0x40000
    scaled.append(struct.unpack('>i',u.mem_read(stack+12,4))[0])
# Independent collision tables: original CODE 6 direct-angle loop.
# It uses extended 0.017453 per degree, no f32 accumulation or f32 trig output.
movement_counts=counts.copy();counts.clear()
u.mem_write(0x80000,(source/'6.bin').read_bytes())
for offset,target in [(0x127a,0x20088),(0x128a,0x30038)]:
    u.mem_write(a5+offset,b'\x4e\xf9'+struct.pack('>I',target))
u.reg_write(UC_M68K_REG_A7,stack);u.reg_write(UC_M68K_REG_A6,frame)
u.emu_start(0x80550,0x8061a,count=200000)
assert u.reg_read(UC_M68K_REG_PC)==0x8061a
assert counts['a9ec:0018']==360 and counts['a9ec:001a']==360
assert counts['a9eb:200e']==720 and counts['a9eb:2810']==720
assert 'a9eb:1010' not in counts and environment==0
collision=list(map(list,zip(struct.unpack('>360i',u.mem_read(a5-0x38b2,1440)),
                            struct.unpack('>360i',u.mem_read(a5-0x3312,1440)))))
report={'method':'Original CODE12 and CODE6 initializers; modeled SANE services; original CODE11 scaling and CODE36 division',
        'limitation':'Not verified against original SANE transcendental implementation or runtime floating environment',
        'hashes':{str(i):hashlib.sha256((source/f'{i}.bin').read_bytes()).hexdigest() for i in [6,11,12,35,36]},
        'traps':movement_counts,'collision_traps':counts,'base':base,'movement':scaled,'collision':collision}
p=root/'.local/disassembly/direction-table-candidate.json'
p.write_text(json.dumps(report,indent=2)+'\n')
print('360 candidate direction values generated through original initializer and fixed-point scaling.')
print('SANE operations are modeled, not original-code verified.')
print({i:scaled[i] for i in [0,45,90,180,270,359]})

print('360 collision cosine/sine pairs generated through original CODE6 with modeled SANE.')
print({i:collision[i] for i in [0,45,90,180,270,359]})
