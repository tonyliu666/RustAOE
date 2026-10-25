#!/usr/bin/env python3
"""Lists the DRS archives the original opens, in order, read from aoc.exe.

    python3 -I reverse/tools/archive_order.py aoe2-install/aoc.exe

Evidence for the archive search order (Phase 1.6 spec, step 1). Reads the
executable's bytes only; it never runs it. Inside `FUN_0043ae90` (from
0x43ae90 to 0x43b200), every `push imm32` (opcode 0x68) whose address lands
on a zero-terminated string ending in `.drs` is printed with the address of
the push, in file order. Each archive name is pushed just before one call
to `FUN_00543370`, which appends that archive to the search list; the
first call after each push is printed so this can be checked.
"""

import struct
import sys

FUNCTION_START = 0x43AE90
FUNCTION_END = 0x43B200


def sections(pe):
    """(virtual address, virtual size, raw offset, raw size) per section."""
    pe_at = struct.unpack_from("<I", pe, 0x3C)[0]
    count = struct.unpack_from("<H", pe, pe_at + 6)[0]
    optional_size = struct.unpack_from("<H", pe, pe_at + 20)[0]
    image_base = struct.unpack_from("<I", pe, pe_at + 24 + 28)[0]
    table = pe_at + 24 + optional_size
    out = []
    for i in range(count):
        at = table + 40 * i
        vsize, vaddr, rsize, roff = struct.unpack_from("<IIII", pe, at + 8)
        out.append((image_base + vaddr, vsize, roff, rsize))
    return out


def file_offset(secs, va):
    for vaddr, vsize, roff, rsize in secs:
        if vaddr <= va < vaddr + max(vsize, rsize):
            off = roff + (va - vaddr)
            if off < roff + rsize:
                return off
    return None


def string_at(pe, secs, va):
    off = file_offset(secs, va)
    if off is None:
        return None
    end = pe.find(b"\0", off, off + 64)
    if end < 0:
        return None
    try:
        return pe[off:end].decode("ascii")
    except UnicodeDecodeError:
        return None


def next_call(pe, off, va, within=40):
    """The target of the first `call rel32` (opcode 0xE8) in the next
    `within` bytes, if any."""
    for i in range(within):
        if pe[off + i] == 0xE8:
            rel = struct.unpack_from("<i", pe, off + i + 1)[0]
            return (va + i + 5 + rel) & 0xFFFFFFFF
    return None


def main():
    pe = open(sys.argv[1], "rb").read()
    secs = sections(pe)
    start = file_offset(secs, FUNCTION_START)
    end = file_offset(secs, FUNCTION_END)
    order = 0
    for off in range(start, end - 5):
        if pe[off] != 0x68:
            continue
        target = struct.unpack_from("<I", pe, off + 1)[0]
        name = string_at(pe, secs, target)
        if name and name.lower().endswith(".drs"):
            order += 1
            push_va = FUNCTION_START + (off - start)
            called = next_call(pe, off + 5, FUNCTION_START + (off + 5 - start))
            into = f"then calls {called:#x}" if called else "no call follows"
            print(f"{order}. push at {push_va:#x}: {target:#x} {name}; {into}")


if __name__ == "__main__":
    main()
