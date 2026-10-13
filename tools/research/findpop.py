#!/usr/bin/env python3
"""
Correlate CIsland candidate arrays against the known global population table.

For each candidate offset, read the per-island int32[11][8] table from every
live CIsland, sum it across islands, and report any (candidate_row -> global_row)
pair whose 8 player columns match exactly. The global table is the ground truth
the community calculator already reads, so an exact 8-wide match on a non-zero
row identifies the per-island array.
"""
import struct
import sys

from annomem import Mem, looks_ptr

CIsland_VFTABLE = 0x010FFD00
GOP_GLOBAL = 0x012D5A90
GLOBAL_POP = 0x7C78          # int32[12][8]
MODULE_END = 0x01400000

# int32[11][8] arrays zero-filled by CIsland::CIsland (0x00756940)
CANDIDATES = [0x24D8, 0x2660, 0x2AA0, 0x2CE0, 0x2EB8, 0x3018,
              0x3178, 0x32D8, 0x3438, 0x3598, 0x36F8, 0x3858]

ROWS, COLS, STRIDE = 11, 8, 0x20


def table(m, base, rows, cols, stride):
    out = []
    for r in range(rows):
        d = m.read(base + r * stride, 4 * cols)
        if d is None:
            return None
        out.append(list(struct.unpack("<" + "i" * cols, d)))
    return out


def main():
    m = Mem()

    gop = m.u32(GOP_GLOBAL)
    glob = table(m, gop + GLOBAL_POP, 12, COLS, STRIDE)
    print(f"CGOPManager @ 0x{gop:08x}")
    print("global population table (+0x7c78):")
    for i, row in enumerate(glob):
        if any(row):
            print(f"  global row {i:2d} (+0x{GLOBAL_POP + i*STRIDE:04x}): {row}")

    # enumerate islands
    islands = []
    target = struct.pack("<I", CIsland_VFTABLE)
    for base, size, _ in m.regions():
        data = m.read(base, size)
        if data is None:
            continue
        i = 0
        while True:
            i = data.find(target, i)
            if i < 0:
                break
            a = base + i
            # Validate: CIsland::CIsland writes a self-referential list
            # sentinel at +8 / +0xC. Rejects stray dwords that merely happen
            # to equal the vtable address.
            if (i % 4 == 0 and a >= MODULE_END
                    and m.u32(a + 8) == a + 8 and m.u32(a + 0xC) == a + 8):
                islands.append(a)
            i += 1
    ids = [m.i32(a + 0x3FF0) for a in islands]
    print(f"\n{len(islands)} CIsland instances; ids at +0x3FF0 = "
          f"{sorted(v for v in ids if v is not None)}")

    # sum each candidate across islands
    print("\ncandidate arrays, summed over all islands "
          "(non-zero rows only):")
    sums = {}
    for off in CANDIDATES:
        acc = [[0] * COLS for _ in range(ROWS)]
        ok = 0
        for isl in islands:
            t = table(m, isl + off, ROWS, COLS, STRIDE)
            if t is None:
                continue
            ok += 1
            for r in range(ROWS):
                for c in range(COLS):
                    acc[r][c] += t[r][c]
        sums[off] = acc
        nz = [(r, acc[r]) for r in range(ROWS) if any(acc[r])]
        print(f"\n  +0x{off:04x}  ({ok}/{len(islands)} islands readable)")
        if not nz:
            print("     all zero")
        for r, row in nz:
            print(f"     row {r:2d}: {row}")

    # exact match hunt
    print("\n=== exact 8-column matches against global rows ===")
    found = False
    for off, acc in sums.items():
        for r in range(ROWS):
            if not any(acc[r]):
                continue
            for g in range(12):
                if acc[r] == glob[g]:
                    found = True
                    print(f"  island +0x{off:04x} row {r:2d}  ==  "
                          f"global row {g:2d} (+0x{GLOBAL_POP + g*STRIDE:04x})"
                          f"  {acc[r]}")
    if not found:
        print("  none — per-island array may not be a plain sum "
              "(check scaling or row alignment)")


if __name__ == "__main__":
    main()
