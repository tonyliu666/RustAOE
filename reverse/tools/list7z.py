import struct, lzma, sys

EXE = sys.argv[1]
OUT = sys.argv[2] if len(sys.argv) > 2 else None
f = open(EXE, 'rb').read()
BASE = 0x27c00  # PE raw data ends here; 7z overlay starts here
assert f[BASE:BASE+6] == b"7z\xbc\xaf'\x1c", "no 7z signature at overlay"
arc = f[BASE:]

def num(b, p):
    first = b[p]; p += 1
    mask = 0x80; val = 0
    for i in range(8):
        if first & mask == 0:
            return val | ((first & (mask - 1)) << (8 * i)), p
        val |= b[p] << (8 * i); p += 1
        mask >>= 1
    return val, p

def read_bits(b, p, n):
    out = []; cur = 0; mask = 0
    for _ in range(n):
        if mask == 0:
            cur = b[p]; p += 1; mask = 0x80
        out.append(bool(cur & mask)); mask >>= 1
    return out, p

def read_bits_all(b, p, n):
    if b[p]:
        return [True]*n, p + 1
    return read_bits(b, p + 1, n)

def skip_digests(b, p, n):
    d, p = read_bits_all(b, p, n)
    for x in d:
        if x: p += 4
    return p

def parse_streams(b, p):
    info = {'packPos': 0, 'packSizes': [], 'folders': [], 'unpackSizes': [], 'numUnpack': [], 'subSizes': []}
    while True:
        t, p = num(b, p)
        if t == 0x00: break
        if t == 0x06:
            info['packPos'], p = num(b, p)
            n, p = num(b, p)
            while True:
                t2, p = num(b, p)
                if t2 == 0x00: break
                if t2 == 0x09:
                    for _ in range(n):
                        s, p = num(b, p); info['packSizes'].append(s)
                elif t2 == 0x0A:
                    p = skip_digests(b, p, n)
                else: raise SystemExit('pack prop %x' % t2)
        elif t == 0x07:
            t2, p = num(b, p); assert t2 == 0x0B
            nf, p = num(b, p)
            assert b[p] == 0; p += 1
            for _ in range(nf):
                nc, p = num(b, p)
                coders = []; tin = 0; tout = 0
                for _ in range(nc):
                    flag = b[p]; p += 1
                    idsz = flag & 0x0F
                    cid = b[p:p+idsz].hex(); p += idsz
                    nin, nout = 1, 1
                    if flag & 0x10:
                        nin, p = num(b, p); nout, p = num(b, p)
                    props = b''
                    if flag & 0x20:
                        ps, p = num(b, p); props = b[p:p+ps]; p += ps
                    coders.append((cid, props)); tin += nin; tout += nout
                binds = []
                for _ in range(tout - 1):
                    i, p = num(b, p); o, p = num(b, p); binds.append((i, o))
                npk = tin - len(binds)
                packed = []
                if npk > 1:
                    for _ in range(npk):
                        i, p = num(b, p); packed.append(i)
                else:
                    packed = [i for i in range(tin) if i not in {x for x, _ in binds}]
                info['folders'].append({'coders': coders, 'binds': binds, 'tout': tout, 'packed': packed})
            t3, p = num(b, p); assert t3 == 0x0C, hex(t3)
            for fo in info['folders']:
                for _ in range(fo['tout']):
                    s, p = num(b, p); info['unpackSizes'].append(s)
            while True:
                t4, p = num(b, p)
                if t4 == 0x00: break
                if t4 == 0x0A: p = skip_digests(b, p, len(info['folders']))
                else: raise SystemExit('unpack prop %x' % t4)
        elif t == 0x08:
            info['numUnpack'] = [1]*len(info['folders'])
            while True:
                t2, p = num(b, p)
                if t2 == 0x00: break
                if t2 == 0x0D:
                    info['numUnpack'] = []
                    for _ in info['folders']:
                        n, p = num(b, p); info['numUnpack'].append(n)
                elif t2 == 0x09:
                    for i, n in enumerate(info['numUnpack']):
                        tot = 0
                        for _ in range(n - 1):
                            s, p = num(b, p); info['subSizes'].append(s); tot += s
                        if n: info['subSizes'].append(folder_size(info, i) - tot)
                elif t2 == 0x0A:
                    cnt = sum(n for n in info['numUnpack'] if n != 1)
                    p = skip_digests(b, p, cnt)
                else: raise SystemExit('sub prop %x' % t2)
        else:
            raise SystemExit('streams tag %x' % t)
    return info, p

def folder_size(info, i):
    fo = info['folders'][i]
    base = sum(f['tout'] for f in info['folders'][:i])
    bound = {o for _, o in fo['binds']}
    final = [o for o in range(fo['tout']) if o not in bound][0]
    return info['unpackSizes'][base + final]

def lzma1_props(pb_byte, dict_size):
    pb = pb_byte // 45; lp = (pb_byte % 45) // 9; lc = pb_byte % 9
    return {'id': lzma.FILTER_LZMA1, 'dict_size': dict_size, 'lc': lc, 'lp': lp, 'pb': pb}

def apply(cid, props, data, size):
    if cid == '030101':
        return lzma.LZMADecompressor(lzma.FORMAT_RAW, filters=[lzma1_props(props[0], struct.unpack('<I', props[1:5])[0])]).decompress(data)
    if cid == '21':
        d = props[0]
        dict_size = (2 | (d & 1)) << (d // 2 + 11) if d < 40 else 0xFFFFFFFF
        return lzma.LZMADecompressor(lzma.FORMAT_RAW, filters=[{'id': lzma.FILTER_LZMA2, 'dict_size': dict_size}]).decompress(data)
    if cid == '03030103': return lzma.LZMADecompressor(lzma.FORMAT_RAW, filters=[{'id': lzma.FILTER_X86}]).decompress(data) if False else bcj_x86(data)
    raise SystemExit('unsupported coder %s' % cid)

def bcj_x86(data):
    # placeholder: BCJ filter not implemented; flagged below
    raise SystemExit('BCJ x86 coder needs filter (not implemented)')

def decode_folder(info, idx):
    off = 32 + info['packPos'] + sum(info['packSizes'][:idx])
    data = arc[off: off + info['packSizes'][idx]]
    fo = info['folders'][idx]
    # linear chain assumption: coder i output feeds coder i-1; decode from last coder
    sizes = info['unpackSizes'][sum(f['tout'] for f in info['folders'][:idx]):]
    for i in reversed(range(len(fo['coders']))):
        cid, props = fo['coders'][i]
        data = apply(cid, props, data, sizes[i] if i < len(sizes) else None)
    return data

p = 0
hdr = arc[32 + struct.unpack_from('<Q', arc, 12)[0]: 32 + struct.unpack_from('<Q', arc, 12)[0] + struct.unpack_from('<Q', arc, 20)[0]]
if hdr[0] == 0x17:
    si, _ = parse_streams(hdr, 1)
    hdr = decode_folder(si, 0)
    print('encoded header decoded:', len(hdr), 'bytes; coders', [c for c, _ in si['folders'][0]['coders']])
assert hdr[0] == 0x01
p = 1
main = None
t, p = num(hdr, p)
if t == 0x04:
    main, p = parse_streams(hdr, p)
    t, p = num(hdr, p)
names = []
# kEmptyStream: entries with no data of their own (directories and empty
# files). kEmptyFile: which of those are empty files rather than directories.
empty_stream = []
empty_file = []
if t == 0x05:
    nfiles, p = num(hdr, p)
    empty_stream = [False] * nfiles
    while True:
        t, p = num(hdr, p)
        if t == 0x00: break
        size, p = num(hdr, p)
        body = hdr[p:p+size]; p += size
        if t == 0x11:
            names = body[1:].decode('utf-16-le').split('\x00')[:nfiles]
        elif t == 0x0E:
            empty_stream, _ = read_bits(body, 0, nfiles)
        elif t == 0x0F:
            empty_file, _ = read_bits(body, 0, sum(empty_stream))
print('folders:', len(main['folders']) if main else 0, 'files:', len(names))
if main:
    print('coders per folder:', [[c for c, _ in fo['coders']] for fo in main['folders']][:6])
    print('unpack total MB:', round(sum(main['unpackSizes'])/1e6, 1), 'packed total MB:', round(sum(main['packSizes'])/1e6, 1))
    print('sub-streams:', len(main['subSizes']))

# file -> size mapping (non-empty files only, in order)
sizes = main['subSizes'] if main else []
real = [n for n in names]
print('first 80 names:')
for n in names[:80]: print('  ', n)
print('names with extensions histogram:')
import collections
ext = collections.Counter(n.rsplit('.',1)[-1].lower() if '.' in n else '(none)' for n in names)
print(ext.most_common(30))
print('folder0 sample decode test:')
try:
    d = decode_folder(main, 0)
    print('  folder0 decoded bytes:', len(d), 'head', d[:40])
except SystemExit as e:
    print('  ', e)

# map file -> (folder, size) using numUnpack and subSizes
import itertools
names_all = names
# empty stream flags: files with kEmptyStream are directories or empty files
