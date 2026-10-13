#!/usr/bin/env python3
"""Anno 1404 (Addon.exe) production overview -- READ ONLY.

Prints, per island and player, an Anno-1800-style production screen: every
production building, what it makes, its productivity, whether the player has
paused it, its input goods, plus the settlement's warehouse stock and the
game's own per-good demand coverage.

Reuses annomem.Mem, which opens the process WITHOUT PROCESS_VM_WRITE, so
writing to the game is impossible with that handle. Keep it that way.

Structure lives in the Ghidra project; docs/findings.json carries the offsets
in machine-readable form, docs/production.md the derivation.
"""
import argparse
import os
import struct
import sys
import time
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from annomem import Mem, find_islands, settlement_name   # noqa: E402

# ---- static tables in the image (no ASLR, so these are absolute) -----------
WARE_NAMES = 0x0127B820       # char *[68], index == ware id
LEVEL_NAMES = 0x0127B930      # char *[11], index == population tier
CATEGORY_NAMES = 0x0127B060   # char *[13], need categories

# ---- CIsland --------------------------------------------------------------
ISLAND_WAREHOUSES = 0x3F78    # CWarehouse *[], indexed by player slot

# ---- CWarehouse (one per island per player; the Kontor) -------------------
WH_STORAGE = 0x33C            # int m_storage; <= 999 means "no kontor here"
WH_PRODUCTS_MAP = 0x35C       # std::map<wareId,int> stock, 1000 == 1 t
WH_DEMAND_HISTORY = 0x3B0     # list[68], 40 rolling samples of demand coverage
WH_DEMANDS_PER_LEVEL = 0x768  # list[11] of build-material demands
WH_PLAYER_ID = 0x7C0

# ---- game object (building) ----------------------------------------------
BLD_POS_X, BLD_POS_Y = 0x04, 0x08
BLD_PLAYER_SLOT = 0xC0
BLD_DATA_CONTAINER = 0xC4     # CPropertyDataContainer *, shared per type
BLD_ISLAND = 0xD4             # CIsland *

# ---- CPropertyDataContainer (0x12C bytes, one fixed slot per property) ----
DC_FACTORY = 0x08             # CPropertyDataFactory        (input goods)
DC_WARE_PRODUCTION = 0x34     # CPropertyDataWareProduction (output, rate)
DC_STANDARD = 0x74            # CPropertyDataStandard       (type name + guid)

DS_NAME = 0x08                # inline NUL-terminated char[] type name
DS_GUID = 0x28

DWP_CYCLE_MS = 0x08           # ms per production cycle at 100 %
DWP_PRODUCT = 0x0C            # ware id produced
DWP_CAPACITY = 0x10
DWP_AMOUNT = 0x14             # units produced per cycle (1000 == 1 t)

DF_IN1_WARE, DF_IN1_CAP, DF_IN1_AMOUNT = 0x08, 0x0C, 0x10
DF_IN2_WARE, DF_IN2_CAP, DF_IN2_AMOUNT = 0x14, 0x18, 0x1C

# ---- CPropertyProduction and subclasses (runtime state) ------------------
PP_OWNER = 0x04               # back-pointer to the building object
PP_GOODS = 0x0C               # units currently held in the building
PP_PRODUCTIVITY = 0x10        # percent
PP_STOPPED = 0x14             # byte, 1 == player paused this building
PP_IS_PRODUCING = 0x1D        # byte

VT_PROPERTY_BUILDING = 0x0110018C   # every building carries one
PRODUCTION_CLASSES = (
    ".?AVCPropertyProduction@@",
    ".?AVCPropertyFactory@@",
    ".?AVCPropertyFarm@@",
    ".?AVCPropertyShipProduction@@",
)


def cstr(m, addr, n=64):
    b = m.read(addr, n)
    return b.split(b"\0")[0].decode("ascii", "replace") if b else ""


def name_table(m, base, count):
    return [cstr(m, m.u32(base + i * 4)) for i in range(count)]


def rtti_class(m, vtable):
    """vtable[-1] -> Complete Object Locator -> +12 TypeDescriptor -> +8 name."""
    if not vtable or not (0x400000 <= vtable < 0x1400000):
        return None
    col = m.u32(vtable - 4)
    if not col or not (0x400000 <= col < 0x1400000):
        return None
    td = m.u32(col + 12)
    if not td or not (0x400000 <= td < 0x1400000):
        return None
    name = cstr(m, td + 8)
    return name if name.startswith(".?AV") else None


def scan_value(m, value, max_region=64 * 1024 * 1024):
    """Every 4-byte-aligned address holding `value`."""
    pat = struct.pack("<I", value)
    out = []
    for base, size, _prot in m.regions():
        if size > max_region:
            continue
        data = m.read(base, size)
        if not data:
            continue
        i = data.find(pat)
        while i >= 0:
            if i % 4 == 0:
                out.append(base + i)
            i = data.find(pat, i + 1)
    return out


def walk_map(m, header):
    """STLport rb-tree std::map<int,int>: +4 root, node +8/+0xC kids,
    +0x10 key, +0x14 value."""
    out = {}

    def rec(node, depth=0):
        if not node or depth > 48:
            return
        rec(m.u32(node + 8), depth + 1)
        out[m.u32(node + 0x10)] = m.u32(node + 0x14)
        rec(m.u32(node + 0xC), depth + 1)

    rec(m.u32(header + 4))
    return out


def list_values(m, head, field=8, limit=64):
    """STLport list: node+0 == next, the header itself is the sentinel."""
    out, p = [], m.u32(head)
    while p and p != head and len(out) < limit:
        out.append(m.u32(p + field))
        p = m.u32(p)
    return out


def find_buildings(m, islands_by_addr):
    """Every game object carrying a CPropertyBuilding, grouped by settlement.

    A raw vtable scan produces false positives (see docs/methodology.md), so
    each hit must survive three checks: the owner is a heap pointer, the
    owner's island is one we already enumerated, and its player slot is < 8.
    """
    out = defaultdict(list)
    for hit in scan_value(m, VT_PROPERTY_BUILDING):
        owner = m.u32(hit + PP_OWNER)
        if not owner or owner < 0x01400000:
            continue
        island = m.u32(owner + BLD_ISLAND)
        slot = m.u32(owner + BLD_PLAYER_SLOT)
        if island in islands_by_addr and slot is not None and slot < 8:
            out[(islands_by_addr[island], slot)].append(owner)
    return out


def production_property(m, building, cache):
    """The runtime CPropertyProduction subclass hanging off this building."""
    for off in range(0x40, 0x84, 4):
        p = m.u32(building + off)
        if not p or p < 0x01400000:
            continue
        vt = m.u32(p)
        if vt not in cache:
            cache[vt] = rtti_class(m, vt)
        if cache[vt] in PRODUCTION_CLASSES:
            return p, cache[vt]
    return None, None


def describe(m, building, wares, cache):
    dc = m.u32(building + BLD_DATA_CONTAINER)
    if not dc or dc < 0x01400000:
        return None
    wp = m.u32(dc + DC_WARE_PRODUCTION)
    if not wp or wp < 0x01400000:
        return None
    prop, cls = production_property(m, building, cache)
    if prop is None:
        return None
    std = m.u32(dc + DC_STANDARD)

    inputs = []
    fac = m.u32(dc + DC_FACTORY)
    if (fac and fac >= 0x01400000
            and rtti_class(m, m.u32(fac)) == ".?AVCPropertyDataFactory@@"):
        for w_off, a_off in ((DF_IN1_WARE, DF_IN1_AMOUNT),
                             (DF_IN2_WARE, DF_IN2_AMOUNT)):
            w = m.u32(fac + w_off)
            if w:
                inputs.append((wares[w] if w < len(wares) else str(w),
                               m.u32(fac + a_off)))

    product = m.u32(wp + DWP_PRODUCT)
    cycle = m.u32(wp + DWP_CYCLE_MS)
    amount = m.u32(wp + DWP_AMOUNT)
    prod = m.u32(prop + PP_PRODUCTIVITY)
    stopped = (m.read(prop + PP_STOPPED, 1) or b"\0")[0]

    return {
        "type": cstr(m, std + DS_NAME) if std else "?",
        "guid": m.u32(std + DS_GUID) if std else 0,
        "class": cls[4:-2] if cls else "?",
        "pos": (m.u32(building + BLD_POS_X), m.u32(building + BLD_POS_Y)),
        "product": wares[product] if product < len(wares) else str(product),
        "cycle_ms": cycle,
        "per_cycle": amount,
        "productivity": prod,
        "paused": bool(stopped),
        "stock": m.u32(prop + PP_GOODS),
        "inputs": inputs,
        # tons per game-minute at the current productivity; a paused
        # building contributes nothing, so it must not be counted
        "rate": (0.0 if stopped or not cycle else
                 (amount / 1000.0) * (60000.0 / cycle) * (prod / 100.0)),
    }


def measure_consumption(m, sites, seconds, sample=0.25):
    """Measure real consumption from warehouse drawdown.

    Consumption is deducted on a strict 4.00 s tick as a single negative step;
    producer deliveries arrive as positive jumps at unrelated moments. Sampling
    well under the tick therefore separates the two without having to pause
    anything: sum only the negative steps.

    The positive steps are the mirror image: goods actually DELIVERED into the
    warehouse. That is not the same as nominal output -- a producer whose
    warehouse is full keeps reporting 100 % productivity while delivering
    nothing -- so both are reported.

    Returns {siteKey: {"out": {wareId: t/min consumed},
                       "in":  {wareId: t/min delivered}}}.
    """
    acc = {k: {"out": defaultdict(float), "in": defaultdict(float)}
           for k in sites}
    prev = {k: walk_map(m, wh + WH_PRODUCTS_MAP) for k, wh in sites.items()}
    t0 = time.time()
    while time.time() - t0 < seconds:
        time.sleep(sample)
        for k, wh in sites.items():
            cur = walk_map(m, wh + WH_PRODUCTS_MAP)
            for ware, v in cur.items():
                d = v - prev[k].get(ware, v)
                if d < 0:
                    acc[k]["out"][ware] += -d / 1000.0
                elif d > 0:
                    acc[k]["in"][ware] += d / 1000.0
            prev[k] = cur
    elapsed = (time.time() - t0) / 60.0
    return {k: {side: {w: v / elapsed for w, v in d.items()}
                for side, d in sides.items()} for k, sides in acc.items()}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-p", "--player", type=int, default=None,
                    help="only this player slot")
    ap.add_argument("--stock", action="store_true",
                    help="also print warehouse stock and demand coverage")
    ap.add_argument("--each", action="store_true",
                    help="one line per building instead of grouping by type")
    ap.add_argument("--measure", type=float, metavar="SECONDS",
                    help="sample this long to measure real consumption and "
                         "print the production/consumption balance "
                         "(60 is usually enough; the game must be unpaused)")
    a = ap.parse_args()

    m = Mem()
    wares = name_table(m, WARE_NAMES, 68)
    islands = dict(find_islands(m))
    by_addr = {addr: iid for iid, addr in islands.items()}

    cache = {}
    buildings = find_buildings(m, by_addr)

    settlements = []
    for (iid, slot) in sorted(buildings):
        if a.player is not None and slot != a.player:
            continue
        island = islands[iid]
        wh = m.u32(island + ISLAND_WAREHOUSES + slot * 4)
        if not wh or wh < 0x01400000 or (m.u32(wh + WH_STORAGE) or 0) <= 999:
            continue
        if not any(describe(m, b, wares, cache) for b in buildings[(iid, slot)]):
            continue
        settlements.append({
            "iid": iid, "slot": slot, "wh": wh,
            "name": settlement_name(m, island, slot) or f"island {iid}"})

    consumption = {}
    if a.measure:
        print(f"sampling {a.measure:.0f}s to measure consumption "
              f"(game must be running)...", flush=True)
        consumption = measure_consumption(
            m, {s["iid"]: s["wh"] for s in settlements}, a.measure)

    # Read building state AFTER the sampling window, so a productivity or
    # pause flag that changed mid-window matches the flows reported beside it.
    for s in settlements:
        s["rows"] = [d for d in (describe(m, b, wares, cache)
                                 for b in buildings[(s["iid"], s["slot"])]) if d]

    for s in settlements:
        iid, slot, wh, rows, name = (s["iid"], s["slot"], s["wh"],
                                     s["rows"], s["name"])
        print(f"\n=== {name}  (island {iid}, player {slot})  "
              f"warehouse capacity {m.u32(wh + WH_STORAGE)}")

        if a.each:
            print(f"  {'building':22s} {'makes':14s} {'pos':>9s} {'prod%':>5s} "
                  f"{'paused':>6s} {'held':>6s}  inputs")
            for d in sorted(rows, key=lambda r: (r["product"], r["type"])):
                ins = ", ".join(f"{w} {amt/1000:g}t" for w, amt in d["inputs"]) or "-"
                print(f"  {d['type']:22.22s} {d['product']:14.14s} "
                      f"{d['pos'][0]:4d},{d['pos'][1]:4d} {d['productivity']:5d} "
                      f"{'yes' if d['paused'] else '':>6s} "
                      f"{d['stock']/1000:6.1f}  {ins}")
        else:
            by_type = defaultdict(list)
            for d in rows:
                by_type[(d["type"], d["product"])].append(d)
            print(f"  {'building':22s} {'makes':14s} {'n':>2s} {'avg %':>6s} "
                  f"{'paused':>6s} {'t/min':>7s}  inputs")
            for (typ, product), group in sorted(by_type.items()):
                n = len(group)
                avg = sum(g["productivity"] for g in group) / n
                paused = sum(1 for g in group if g["paused"])
                rate = sum(g["rate"] for g in group)
                ins = ", ".join(f"{w} {amt/1000:g}t"
                                for w, amt in group[0]["inputs"]) or "-"
                print(f"  {typ:22.22s} {product:14.14s} {n:2d} {avg:6.1f} "
                      f"{paused:6d} {rate:7.2f}  {ins}")

        if a.stock:
            stock = walk_map(m, wh + WH_PRODUCTS_MAP)
            print("  warehouse:", ", ".join(
                f"{wares[k]}={v/1000:g}t" for k, v in sorted(stock.items())))
            cover = []
            for w in range(68):
                hist = list_values(m, wh + WH_DEMAND_HISTORY + w * 8)
                if any(hist):
                    cover.append(f"{wares[w]}={hist[0]}%")
            if cover:
                print("  demand coverage:", ", ".join(cover))

        if a.measure:
            made = defaultdict(float)
            for d in rows:
                made[d["product"]] += d["rate"]
            meas = consumption.get(iid, {"out": {}, "in": {}})
            used = {wares[w]: v for w, v in meas["out"].items()}
            got = {wares[w]: v for w, v in meas["in"].items()}
            goods = sorted(set(made) | set(used) | set(got))
            print(f"  {'good':14s} {'nominal':>8s} {'delivered':>10s} "
                  f"{'consumed':>9s} {'net':>8s}   (t/min)")
            for g in goods:
                p, i, c = made.get(g, 0.0), got.get(g, 0.0), used.get(g, 0.0)
                if p == 0.0 and i == 0.0 and c == 0.0:
                    continue
                print(f"  {g:14s} {p:8.3f} {i:10.3f} {c:9.3f} {i - c:+8.3f}")


if __name__ == "__main__":
    main()
