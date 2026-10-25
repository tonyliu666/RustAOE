import sys, struct, os
EXE, OUTDIR = sys.argv[1], sys.argv[2]
sys.argv = ['list7z.py', EXE]
import io, contextlib
with contextlib.redirect_stdout(io.StringIO()):
    exec(compile(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'list7z.py')).read(), 'list7z', 'exec'))
MASK = 0xFFFFFFFF

def bcj2_decode(main, call, jump, rc, outsize):
    probs = [1024] * 258
    rng = MASK; code = 0; rcp = 0
    for _ in range(5):
        code = ((code << 8) | rc[rcp]) & MASK; rcp += 1
    out = bytearray(); inp = 0; cp = 0; jp = 0; prev = 0
    def is_j(b0, b1):
        return (b1 & 0xFE) == 0xE8 or (b0 == 0x0F and (b1 & 0xF0) == 0x80)
    while len(out) < outsize and inp < len(main):
        b = main[inp]; inp += 1; out.append(b)
        if not is_j(prev, b):
            prev = b; continue
        if len(out) >= outsize: break
        idx = prev if b == 0xE8 else (256 if b == 0xE9 else 257)
        p = probs[idx]
        bound = (rng >> 11) * p
        if code < bound:
            rng = bound; probs[idx] = p + ((2048 - p) >> 5); bit = 0
        else:
            rng -= bound; code -= bound; probs[idx] = p - (p >> 5); bit = 1
        if rng < (1 << 24):
            rng = (rng << 8) & MASK; code = ((code << 8) | rc[rcp]) & MASK; rcp += 1
        if bit == 0:
            prev = b; continue
        if b == 0xE8:
            src = struct.unpack_from('>I', call, cp)[0]; cp += 4
        else:
            src = struct.unpack_from('>I', jump, jp)[0]; jp += 4
        dest = (src - (len(out) + 4)) & MASK
        out += struct.pack('<I', dest)
        prev = (dest >> 24) & 0xFF
    return bytes(out[:outsize])

def npacks(fidx):
    fo = main['folders'][fidx]
    nins = [4 if c == '0303011b' else 1 for c, _ in fo['coders']]
    return len(fo['packed'])

def decode_folder_full(fidx, pack_base):
    fo = main['folders'][fidx]
    base_out = sum(f['tout'] for f in main['folders'][:fidx])
    coders = fo['coders']; binds = dict(fo['binds'])  # in-index -> out-index
    nins = [4 if c == '0303011b' else 1 for c, _ in coders]
    in_start = []; ni = 0
    for n in nins: in_start.append(ni); ni += n
    bound_ins = set(binds.keys())
    packed_ins = fo['packed']
    off = 32 + main['packPos'] + sum(main['packSizes'][:pack_base])
    packs = {}
    for j, ins in enumerate(packed_ins):
        sz = main['packSizes'][pack_base + j]
        packs[ins] = arc[off:off + sz]; off += sz
    def get_in(ins):
        return decode_out(binds[ins]) if ins in binds else packs[ins]
    def decode_out(c):
        cid, props = coders[c]
        size = main['unpackSizes'][base_out + c]
        if cid == '0303011b':
            ins = [get_in(in_start[c] + k) for k in range(4)]
            return bcj2_decode(ins[0], ins[1], ins[2], ins[3], size)
        return apply(cid, props, get_in(in_start[c]), size)
    bound_outs = set(binds.values())
    final = [o for o in range(len(coders)) if o not in bound_outs][0]
    return decode_out(final)

# Map every non-empty file to (folder index, offset in folder) using sub-stream sizes
os.makedirs(OUTDIR, exist_ok=True)
pack_base = 0
folder_data = {}
sub_i = 0
subsizes = main['subSizes']
files_nonempty = []
# names with a stream: skip directories (names with no data stream) by counting sub-streams
for fidx, fo in enumerate(main['folders']):
    n_sub = main['numUnpack'][fidx]
    data = decode_folder_full(fidx, pack_base)
    pack_base += npacks(fidx)
    folder_data[fidx] = data
    print('folder', fidx, 'decoded', len(data), 'bytes; substreams', n_sub, flush=True)


def safe_path(root, name):
    """The output path for an archive entry, or None if it would escape `root`.

    Entry names come from a downloaded archive, so `../x`, `/etc/x`, or
    `C:\\x` must not be able to write outside the output directory.
    """
    parts = name.replace('\\', '/').split('/')
    if name.startswith(('/', '\\')) or any(p in ('', '.', '..') or ':' in p for p in parts):
        return None
    path = os.path.realpath(os.path.join(root, *parts))
    if not path.startswith(os.path.realpath(root) + os.sep):
        return None
    return path

# Every entry with data takes the next sub-stream, in folder order.
streams = []
for fidx in range(len(main['folders'])):
    data, at = folder_data[fidx], 0
    count = main['numUnpack'][fidx]
    first = sum(main['numUnpack'][:fidx])
    for size in subsizes[first:first + count]:
        streams.append(data[at:at + size])
        at += size
assert len(streams) == len(names) - sum(empty_stream), (len(streams), len(names), sum(empty_stream))

written = dirs = empties = refused = 0
next_stream = 0
next_empty = 0
for i, name in enumerate(names):
    path = safe_path(OUTDIR, name)
    if empty_stream[i]:
        is_file = empty_file[next_empty] if next_empty < len(empty_file) else False
        next_empty += 1
    if path is None:
        refused += 1
        print('refused unsafe name:', repr(name))
        if not empty_stream[i]:
            next_stream += 1
        continue
    if empty_stream[i]:
        if is_file:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, 'wb').close()
            empties += 1
        else:
            os.makedirs(path, exist_ok=True)
            dirs += 1
        continue
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'wb') as out:
        out.write(streams[next_stream])
    next_stream += 1
    written += 1
print('wrote', written, 'files,', empties, 'empty files,', dirs, 'directories; refused', refused)
