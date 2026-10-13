#!/usr/bin/env python3
"""
annomem — read-only live memory inspector for Anno 1404 (Addon.exe).

READ-ONLY BY CONSTRUCTION: the process handle is opened with
PROCESS_VM_READ | PROCESS_QUERY_INFORMATION only. PROCESS_VM_WRITE is never
requested, so a write is not merely unused, it is impossible with this handle.

Target is 32-bit, no ASLR, ImageBase 0x400000 -> static addresses from Ghidra
are usable verbatim.
"""

import argparse
import ctypes as C
import ctypes.wintypes as W
import struct
import sys
import time

PROCESS_VM_READ = 0x0010
PROCESS_QUERY_INFORMATION = 0x0400

MEM_COMMIT = 0x1000
PAGE_GUARD = 0x100
PAGE_NOACCESS = 0x01
READABLE = (0x02 | 0x04 | 0x20 | 0x40)  # R, RW, XR, XRW

TH32CS_SNAPPROCESS = 0x00000002

k32 = C.WinDLL("kernel32", use_last_error=True)


class PROCESSENTRY32(C.Structure):
    _fields_ = [
        ("dwSize", W.DWORD), ("cntUsage", W.DWORD), ("th32ProcessID", W.DWORD),
        ("th32DefaultHeapID", C.POINTER(C.c_ulong)), ("th32ModuleID", W.DWORD),
        ("cntThreads", W.DWORD), ("th32ParentProcessID", W.DWORD),
        ("pcPriClassBase", C.c_long), ("dwFlags", W.DWORD),
        ("szExeFile", C.c_char * 260),
    ]


class MEMORY_BASIC_INFORMATION64(C.Structure):
    _fields_ = [
        ("BaseAddress", C.c_ulonglong), ("AllocationBase", C.c_ulonglong),
        ("AllocationProtect", W.DWORD), ("__alignment1", W.DWORD),
        ("RegionSize", C.c_ulonglong), ("State", W.DWORD),
        ("Protect", W.DWORD), ("Type", W.DWORD), ("__alignment2", W.DWORD),
    ]


def find_pid(name):
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if snap == -1:
        raise OSError("CreateToolhelp32Snapshot failed")
    e = PROCESSENTRY32()
    e.dwSize = C.sizeof(PROCESSENTRY32)
    hits = []
    if k32.Process32First(snap, C.byref(e)):
        while True:
            if e.szExeFile.decode("latin1").lower() == name.lower():
                hits.append(e.th32ProcessID)
            if not k32.Process32Next(snap, C.byref(e)):
                break
    k32.CloseHandle(snap)
    return hits


class Mem:
    def __init__(self, exe="Addon.exe"):
        pids = find_pid(exe)
        if not pids:
            sys.exit(f"error: no running process named {exe}")
        if len(pids) > 1:
            print(f"warning: {len(pids)} processes named {exe}, using {pids[0]}",
                  file=sys.stderr)
        self.pid = pids[0]
        self.h = k32.OpenProcess(
            PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, False, self.pid)
        if not self.h:
            sys.exit(f"error: OpenProcess failed on pid {self.pid} "
                     f"(err {C.get_last_error()}) — try running as the same user")

    def read(self, addr, size):
        """Return bytes, or None if the read fails/partially fails."""
        buf = (C.c_ubyte * size)()
        got = C.c_size_t(0)
        ok = k32.ReadProcessMemory(self.h, C.c_void_p(addr), buf, size,
                                   C.byref(got))
        if not ok or got.value != size:
            return None
        return bytes(buf)

    def u32(self, addr):
        b = self.read(addr, 4)
        return None if b is None else struct.unpack("<I", b)[0]

    def i32(self, addr):
        b = self.read(addr, 4)
        return None if b is None else struct.unpack("<i", b)[0]

    def regions(self):
        """Yield (base, size, protect) for committed readable non-guard regions."""
        mbi = MEMORY_BASIC_INFORMATION64()
        addr = 0
        while addr < 0x7FFF0000:
            r = k32.VirtualQueryEx(self.h, C.c_void_p(addr), C.byref(mbi),
                                   C.sizeof(mbi))
            if not r:
                break
            base, size = mbi.BaseAddress, mbi.RegionSize
            if size == 0:
                break
            if (mbi.State == MEM_COMMIT
                    and not (mbi.Protect & PAGE_GUARD)
                    and mbi.Protect != PAGE_NOACCESS
                    and (mbi.Protect & READABLE)):
                yield base, size, mbi.Protect
            addr = base + size


def looks_ptr(v):
    return 0x00400000 <= v < 0x7FFF0000


def fmt_f32(b):
    f = struct.unpack("<f", b)[0]
    if f != f or abs(f) == float("inf"):
        return ""
    if f == 0 or 1e-4 < abs(f) < 1e9:
        return f"{f:.4g}"
    return ""


# ---------------------------------------------------------------- commands

def cmd_info(m, a):
    print(f"pid              {m.pid}")
    hdr = m.read(0x400000, 2)
    print(f"image base 0x400000 readable: {hdr == b'MZ'}")
    tot = n = 0
    for _, size, _ in m.regions():
        n += 1
        tot += size
    print(f"committed readable regions: {n}  ({tot/1024/1024:.1f} MB)")


def cmd_read(m, a):
    data = m.read(a.address, a.count)
    if data is None:
        sys.exit(f"read failed at 0x{a.address:08x}")
    for off in range(0, len(data), 16):
        chunk = data[off:off + 16]
        hexs = " ".join(f"{b:02x}" for b in chunk)
        txt = "".join(chr(b) if 32 <= b < 127 else "." for b in chunk)
        print(f"0x{a.address+off:08x}  {hexs:<47}  {txt}")


def cmd_ptr(m, a):
    for addr in a.address:
        v = m.u32(addr)
        if v is None:
            print(f"[0x{addr:08x}] = <unreadable>")
            continue
        note = ""
        if looks_ptr(v):
            probe = m.u32(v)
            note = (f"  -> readable, first dword 0x{probe:08x}"
                    if probe is not None else "  -> NOT readable")
        print(f"[0x{addr:08x}] = 0x{v:08x}{note}")


def cmd_struct(m, a):
    data = m.read(a.address, a.count)
    if data is None:
        sys.exit(f"read failed at 0x{a.address:08x}")
    print(f"{'offset':>8}  {'hex':>10}  {'int32':>12}  {'float':>12}  note")
    for off in range(0, len(data) - 3, 4):
        raw = data[off:off + 4]
        u = struct.unpack("<I", raw)[0]
        i = struct.unpack("<i", raw)[0]
        if a.skip_zero and u == 0:
            continue
        note = ""
        if looks_ptr(u):
            note = "ptr?" if m.read(u, 4) is not None else "ptr-ish (unreadable)"
        print(f"  +0x{off:04x}  0x{u:08x}  {i:12d}  {fmt_f32(raw):>12}  {note}")


def cmd_array(m, a):
    """Walk a [count][width] int32 table with the given byte stride per row."""
    print(f"base 0x{a.address:08x}  stride 0x{a.stride:x}  rows {a.count}  "
          f"cols {a.width}")
    head = "  row   offset  " + "".join(f"{c:>9}" for c in range(a.width))
    print(head)
    print("  " + "-" * (len(head) - 2))
    for r in range(a.count):
        rowaddr = a.address + r * a.stride
        data = m.read(rowaddr, 4 * a.width)
        if data is None:
            print(f"  {r:3d}  +0x{r*a.stride:05x}  <unreadable>")
            continue
        vals = struct.unpack("<" + "i" * a.width, data)
        cells = "".join(f"{v:>9}" for v in vals)
        print(f"  {r:3d}  +0x{r*a.stride:05x}  {cells}")


def cmd_scan(m, a):
    """Find every 4-byte-aligned address holding `value` (e.g. a vtable ptr)."""
    target = struct.pack("<I", a.value)
    hits = []
    scanned = 0
    for base, size, prot in m.regions():
        if a.max_region and size > a.max_region:
            continue
        data = m.read(base, size)
        if data is None:
            continue
        scanned += size
        start = 0
        while True:
            idx = data.find(target, start)
            if idx < 0:
                break
            if idx % 4 == 0:
                hits.append(base + idx)
                if len(hits) > a.limit:
                    break
            start = idx + 1
        if len(hits) > a.limit:
            break
    print(f"scanned {scanned/1024/1024:.1f} MB, {len(hits)} hit(s) for "
          f"0x{a.value:08x}")
    if len(hits) > a.limit:
        print(f"  >{a.limit} hits — refine before listing")
        return
    for h in hits:
        print(f"  0x{h:08x}")


CISLAND_VFTABLE = 0x010FFD00
ISLAND_ID = 0x3FF0
ISLAND_POP = 0x2CE0          # int32[11 tiers][8 players]
GOP_GLOBAL = 0x012D5A90
GLOBAL_POP = 0x7C78          # int32[12][8]; row 0 = total, row r+1 == island row r

# island row -> name (global row = island row + 1)
TIERS = {2: "Beggars", 4: "Nomads", 5: "Envoys", 7: "Peasants",
         8: "Citizens", 9: "Patricians", 10: "Noblemen"}


ISLAND_MAP = 0x7F5C          # STLport rb-tree: std::map<int islandId, CIsland*>


def find_islands(m):
    """Enumerate live CIsland objects via CGOPManager's island map.

    Walks the rb-tree at CGOPManager + 0x7F5C rather than scanning memory for
    the vtable: it is instant, it yields the id alongside the pointer, and it
    has no false positives to filter. Returns [(islandId, address), ...].
    Falls back to a validated vtable scan if the map looks wrong.
    """
    gop = m.u32(GOP_GLOBAL)
    if gop:
        hdr = gop + ISLAND_MAP
        count = m.u32(hdr + 0x10)
        root = m.u32(hdr + 4)
        if root and count and count < 4096:
            out = []
            seen = set()

            def walk(n, depth=0):
                if not n or n == hdr or depth > 64 or n in seen:
                    return
                seen.add(n)
                walk(m.u32(n + 8), depth + 1)
                out.append((m.i32(n + 0x10), m.u32(n + 0x14)))
                walk(m.u32(n + 0xC), depth + 1)

            walk(root)
            if len(out) == count:
                return sorted(out)
    return [(m.i32(a + ISLAND_ID), a) for a in scan_islands_by_vtable(m)]


def scan_islands_by_vtable(m):
    """Fallback enumeration: scan for CIsland::vftable, validated by the ctor's
    self-referential list sentinel at +8/+0xC. The validation is mandatory --
    without it a stray matching dword corrupts every cross-island aggregate."""
    out = []
    target = struct.pack("<I", CISLAND_VFTABLE)
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
            if (i % 4 == 0 and a >= 0x01400000
                    and m.u32(a + 8) == a + 8 and m.u32(a + 0xC) == a + 8):
                out.append(a)
            i += 1
    return out


ISLAND_POS_X, ISLAND_POS_Y = 0x3FFC, 0x4000
ISLAND_W, ISLAND_H = 0x3FF4, 0x3FF8
SETTLEMENTS, SETTLEMENT_STRIDE = 0x39B8, 0x30   # CIslandSettlement[30]


def settlement_name(m, island, slot):
    """Per-player settlement name: wchar_t[16] UTF-16LE, NUL-terminated.

    The buffer is not zeroed when unused, only NUL-terminated at index 0, so
    stop at the first NUL and never decode the full 32 bytes.
    """
    b = m.read(island + SETTLEMENTS + slot * SETTLEMENT_STRIDE, 32)
    if b is None:
        return ""
    out = []
    for i in range(0, 32, 2):
        c = b[i] | (b[i + 1] << 8)
        if c == 0:
            break
        if c < 32 or c > 0x2000:
            return ""
        out.append(chr(c))
    return "".join(out)


def cmd_islands(m, a):
    islands = find_islands(m)
    rows = sorted(TIERS) if not a.all_tiers else range(11)
    hdr = (f"{'id':>3} {'settlement':<16} {'pos':>11} {'size':>9}  "
           + "".join(f"{TIERS.get(r, 'tier%d' % r):>11}" for r in rows))
    print(hdr)
    print("-" * len(hdr))
    totals = {r: 0 for r in rows}
    shown = 0
    for iid, isl in islands:
        pos = f"{m.i32(isl + ISLAND_POS_X)},{m.i32(isl + ISLAND_POS_Y)}"
        size = f"{m.i32(isl + ISLAND_W)}x{m.i32(isl + ISLAND_H)}"
        vals = {}
        for r in rows:
            v = m.i32(isl + ISLAND_POP + r * 0x20 + a.player * 4)
            vals[r] = v or 0
            totals[r] += vals[r]
        if a.nonzero and not any(vals.values()):
            continue
        shown += 1
        name = settlement_name(m, isl, a.player) or "-"
        print(f"{iid:>3} {name:<16} {pos:>11} {size:>9}  "
              + "".join(f"{vals[r]:>11}" for r in rows))
    print("-" * len(hdr))
    print(f"{'SUM':>3} {shown:<16} {'':>11} {'':>9}  "
          + "".join(f"{totals[r]:>11}" for r in rows))

    gop = m.u32(GOP_GLOBAL)
    gvals = {r: m.i32(gop + GLOBAL_POP + (r + 1) * 0x20 + a.player * 4) or 0
             for r in rows}
    print(f"{'GLB':>3} {'':<16} {'':>11} {'':>9}  "
          + "".join(f"{gvals[r]:>11}" for r in rows))
    bad = [TIERS.get(r, r) for r in rows if gvals[r] != totals[r]]
    print(f"\nplayer {a.player}: island sums "
          + ("MATCH the global table" if not bad
             else f"DIVERGE from global for {bad}"))


# "Current island" = the island the INFOBAR island panel shows: the island under
# the mouse cursor, falling back to the island nearest the camera centre, 0 when
# none (e.g. strategy map open). Sole writer is UpdateIslandUnderCursor
# (0x005808F0), which mirrors the value into a static global, so no pointer
# chase is needed. Same value lives at [0x012D92E8] + 0xE4.
CURRENT_ISLAND_ID = 0x012D54F8        # int32, static
CURRENT_ISLAND_PLAYER = 0x012D54F4    # int32, local player index, static
ISLAND_LAST_SEEN_OFF = 0xE8           # [0x012D92E8] + 0xE8: last non-zero island (sticky)
HOLDER_GLOBAL = 0x012D92E8


def current_island(m):
    """Return (islandId, CIsland* or 0). islandId 0 means 'none'."""
    iid = m.i32(CURRENT_ISLAND_ID) or 0
    if iid <= 0:
        return 0, 0
    return iid, dict(find_islands(m)).get(iid, 0)


def cmd_current(m, a):
    holder = m.u32(HOLDER_GLOBAL)
    isl = dict(find_islands(m))
    player = m.i32(CURRENT_ISLAND_PLAYER)

    def line(prefix, iid):
        addr = isl.get(iid, 0)
        name = settlement_name(m, addr, player) if addr else ""
        print(f"{prefix} id={iid:<3} addr=0x{addr:08x} name={name or '-'}")

    last = None
    t0 = time.time()
    while True:
        cur = m.i32(CURRENT_ISLAND_ID)
        sticky = m.i32(holder + ISLAND_LAST_SEEN_OFF) if holder else -1
        alt = m.i32(holder + 0xE4) if holder else -1
        key = (cur, sticky, alt)
        if key != last:
            if a.watch:
                print(f"[{time.time() - t0:6.1f}s]", end=" ")
            line("current", cur)
            if alt != cur:
                print(f"  NOTE: [holder]+0xE4 = {alt} differs from the global")
            if a.verbose:
                line("  last-seen", sticky)
                print(f"  local player {player}")
            last = key
        if not a.watch:
            break
        time.sleep(0.05)


def cmd_diff(m, a):
    before = m.read(a.address, a.count)
    if before is None:
        sys.exit(f"read failed at 0x{a.address:08x}")
    print(f"snapshot taken; waiting {a.wait}s …", file=sys.stderr)
    time.sleep(a.wait)
    after = m.read(a.address, a.count)
    if after is None:
        sys.exit("second read failed")
    changed = 0
    for off in range(0, len(before) - 3, 4):
        b = struct.unpack("<i", before[off:off + 4])[0]
        n = struct.unpack("<i", after[off:off + 4])[0]
        if b != n:
            changed += 1
            print(f"  +0x{off:04x}  {b:12d} -> {n:12d}   (delta {n-b:+d})")
    print(f"{changed} changed dword(s) in 0x{a.count:x} bytes")


def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--exe", default="Addon.exe")
    sub = p.add_subparsers(dest="cmd", required=True)

    def addr(x):
        return int(x, 0)

    s = sub.add_parser("info", help="process / region summary")
    s.set_defaults(fn=cmd_info)

    s = sub.add_parser("read", help="hexdump")
    s.add_argument("address", type=addr)
    s.add_argument("-n", "--count", type=addr, default=64)
    s.set_defaults(fn=cmd_read)

    s = sub.add_parser("ptr", help="dereference one or more pointers")
    s.add_argument("address", type=addr, nargs="+")
    s.set_defaults(fn=cmd_ptr)

    s = sub.add_parser("struct", help="labeled field dump")
    s.add_argument("address", type=addr)
    s.add_argument("-n", "--count", type=addr, default=0x100)
    s.add_argument("-z", "--skip-zero", action="store_true")
    s.set_defaults(fn=cmd_struct)

    s = sub.add_parser("array", help="walk a [rows][cols] int32 table")
    s.add_argument("address", type=addr)
    s.add_argument("--stride", type=addr, default=0x20)
    s.add_argument("--count", type=int, default=11)
    s.add_argument("--width", type=int, default=8)
    s.set_defaults(fn=cmd_array)

    s = sub.add_parser("scan", help="find addresses holding a 4-byte value")
    s.add_argument("value", type=addr)
    s.add_argument("--limit", type=int, default=64)
    s.add_argument("--max-region", type=addr, default=0)
    s.set_defaults(fn=cmd_scan)

    s = sub.add_parser("islands", help="per-island population table")
    s.add_argument("-p", "--player", type=int, default=0)
    s.add_argument("--all-tiers", action="store_true")
    s.add_argument("--nonzero", action="store_true",
                   help="hide islands with no population for this player")
    s.set_defaults(fn=cmd_islands)

    s = sub.add_parser("current", help="island under the cursor (what the infobar shows)")
    s.add_argument("-w", "--watch", action="store_true",
                   help="keep polling and print every change")
    s.add_argument("-v", "--verbose", action="store_true")
    s.set_defaults(fn=cmd_current)

    s = sub.add_parser("diff", help="snapshot, wait, report changed dwords")
    s.add_argument("address", type=addr)
    s.add_argument("-n", "--count", type=addr, default=0x200)
    s.add_argument("--wait", type=float, default=10.0)
    s.set_defaults(fn=cmd_diff)

    a = p.parse_args()
    a.fn(Mem(a.exe), a)


if __name__ == "__main__":
    main()
