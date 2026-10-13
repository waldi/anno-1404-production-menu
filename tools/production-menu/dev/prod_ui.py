#!/usr/bin/env python3
"""Install and update the first in-game production dialog.

This is a thin orchestrator around two existing pieces:
- annomem/annoprod read live game state with read-only process handles.
- annobridge's in-process DLL owns the native GUI writes.

Typical use, while Addon.exe is running with tools/production-menu/d3d9.dll loaded:

    python tools/production-menu/dev/prod_ui.py --install
    python tools/production-menu/dev/prod_ui.py --watch

The button toggles the dialog. The dialog follows the current/hovered island
(the same island id the infobar uses). "Need" is expressed in equivalent
production-building counts. Only the rates currently verified in this repo are
filled in; unknown goods show "?" rather than inventing a balance table.
"""

from __future__ import annotations

import argparse
import mmap
import os
import struct
import sys
import time
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools" / "research"
sys.path.insert(0, str(TOOLS))

from annomem import (  # noqa: E402
    CURRENT_ISLAND_PLAYER,
    ISLAND_POP,
    Mem,
    current_island,
    settlement_name,
)
from annoprod import (  # noqa: E402
    WARE_NAMES,
    cstr,
    describe,
    find_buildings,
    name_table,
)

BRIDGE_NAME = "AnnoREBridge"
BRIDGE_SIZE = 4096
BRIDGE_MAGIC = 0x4F4E4E41

O_MAGIC = 0
O_OPREQ = 648
O_OPSTATUS = 652
O_OPTARGET = 656
O_OPARG = 720
O_OPTEXT = 980
O_OPRESULT = 1236

OP_STATUS_PENDING = 1
OP_STATUS_WAIT_SRC = 5
OP_SETTEXT = 2
OP_SETVISIBLE = 4
OP_XML = 10

ROW_COUNT = 12
ROOT_PATH = "SYSTEMMESSAGES"
BUTTON = f"{ROOT_PATH}/ANNORE_PROD5_BUTTON"
DIALOG = f"{ROOT_PATH}/ANNORE_PROD5_DIALOG"
TITLE = f"{DIALOG}/TITLE"
HELP = f"{DIALOG}/HELP"

# island population row -> label. Need rates below are intentionally tiny:
# these are the only consumption-building equivalents verified by live tests in
# docs/production.md / docs/findings.json. Add more rows only when verified.
TIER_NAMES = {
    2: "Beggars",
    4: "Nomads",
    5: "Envoys",
    7: "Peasants",
    8: "Citizens",
    9: "Patricians",
    10: "Noblemen",
}
NEED_BUILDING_DIVISORS = {
    "SaltFish": {7: 200.0},  # one FishermanHut per 200 peasants
    "Cider": {7: 340.0},     # one CiderFarm per 340 peasants
}


def _ws(m: mmap.mmap, off: int, text: str, n: int) -> None:
    raw = text.encode("ascii", "strict")
    if len(raw) > n - 1:
        raw = raw[: n - 1]
    m[off : off + n] = raw + bytes(n - len(raw))


def _run_op(m: mmap.mmap, op: int, target: str = "", arg: str = "", text: str = "", timeout: float = 5.0) -> bool:
    _ws(m, O_OPTARGET, target, 64)
    _ws(m, O_OPARG, arg, 260)
    raw = text.encode("utf-16-le")[:254]
    m[O_OPTEXT : O_OPTEXT + 256] = raw + b"\x00" * (256 - len(raw))
    struct.pack_into("<I", m, O_OPRESULT, 0)
    struct.pack_into("<I", m, O_OPSTATUS, OP_STATUS_PENDING)
    struct.pack_into("<I", m, O_OPREQ, op)

    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(0.05)
        st = struct.unpack_from("<I", m, O_OPSTATUS)[0]
        if st not in (OP_STATUS_PENDING, OP_STATUS_WAIT_SRC):
            return st == 2
    struct.pack_into("<I", m, O_OPREQ, 0)
    return False


def attach_bridge() -> mmap.mmap:
    m = mmap.mmap(-1, BRIDGE_SIZE, tagname=BRIDGE_NAME)
    magic = struct.unpack_from("<I", m, O_MAGIC)[0]
    if magic != BRIDGE_MAGIC:
        raise RuntimeError("annobridge shared block exists but DLL magic is not set; is d3d9.dll loaded in Addon.exe?")
    return m


def install_dialog(m: mmap.mmap, parent: str) -> None:
    xml = Path(__file__).resolve().parent.parent / "examples" / "production_dialog_v5.dlg"
    if not _run_op(m, OP_XML, parent, arg=str(xml.resolve())):
        raise RuntimeError("failed to instantiate production_dialog.dlg; see %TEMP%\\annobridge.log")
    # Use full paths for children inside the initially hidden dialog. Hidden
    # elements do not dispatch events, while full-path operations run on any GUI
    # event and can therefore reach them immediately.
    _run_op(m, OP_SETTEXT, BUTTON, text="[FONT Tooltip][COLOR FFFFFFFF][CENTER]Production")
    _run_op(m, OP_SETTEXT, TITLE, text="[FONT Tooltip][COLOR FFFFFFFF]Production")
    _run_op(m, OP_SETTEXT, HELP, text="[FONT Tooltip][COLOR FFFFFFFF]Hover an island; click Production to toggle this dialog.")


def _fmt(v: float | None) -> str:
    if v is None:
        return "?"
    if abs(v - round(v)) < 0.005:
        return str(int(round(v)))
    return f"{v:.2f}".rstrip("0").rstrip(".")


def _pop_by_tier(mem: Mem, island: int, player: int) -> dict[int, int]:
    return {tier: mem.i32(island + ISLAND_POP + tier * 0x20 + player * 4) or 0 for tier in TIER_NAMES}


def _need_for_good(good: str, pop: dict[int, int]) -> float | None:
    divisors = NEED_BUILDING_DIVISORS.get(good)
    if not divisors:
        return None
    return sum((pop.get(tier, 0) / divisor) for tier, divisor in divisors.items())


def snapshot_lines(mem: Mem) -> tuple[str, list[str]]:
    iid, island = current_island(mem)
    player = mem.i32(CURRENT_ISLAND_PLAYER) or 0
    if iid <= 0 or not island:
        return "[FONT Tooltip][COLOR FFFFFFFF]Production: no island", ["[FONT Tooltip][COLOR FFFFFFFF]Hover an island or move the camera over one."]

    wares = name_table(mem, WARE_NAMES, 68)
    name = settlement_name(mem, island, player) or f"island {iid}"
    title = f"[FONT Tooltip][COLOR FFFFFFFF]Production: {name} (island {iid}, player {player})"

    islands_by_addr = {island: iid}
    cache: dict[int, str | None] = {}
    buildings = find_buildings(mem, islands_by_addr).get((iid, player), [])
    rows = [d for d in (describe(mem, b, wares, cache) for b in buildings) if d]

    produced: dict[str, dict[str, float | int]] = defaultdict(lambda: {"total": 0, "active": 0, "rate": 0.0})
    for d in rows:
        p = produced[d["product"]]
        p["total"] = int(p["total"]) + 1
        if not d["paused"]:
            p["active"] = int(p["active"]) + 1
        p["rate"] = float(p["rate"]) + float(d["rate"])

    pop = _pop_by_tier(mem, island, player)
    goods = sorted(set(produced) | {g for g in NEED_BUILDING_DIVISORS if (_need_for_good(g, pop) or 0) > 0})
    if not goods:
        return title, ["[FONT Tooltip][COLOR FFFFFFFF]No production buildings found for this settlement."]

    out: list[str] = []
    for good in goods[:ROW_COUNT]:
        p = produced.get(good, {"total": 0, "active": 0, "rate": 0.0})
        total, active = int(p["total"]), int(p["active"])
        produced_text = str(total) if active == total else f"{active}/{total} active"
        need = _need_for_good(good, pop)
        colour = "FFAAFFAA" if need is not None and active >= need else "FFFFAA00" if need is not None else "FFCCCCCC"
        out.append(
            f"[FONT IngameMessages][COLOR {colour}]{good}[DEFAULTCOLOR]  "
            f"[COLOR FFFFFFFF]Need: {_fmt(need)}  Produced: {produced_text}[DEFAULTCOLOR]"
        )
    if len(goods) > ROW_COUNT:
        out[-1] = f"[FONT Tooltip][COLOR FFFFFFFF]... {len(goods) - ROW_COUNT + 1} more goods"
    return title, out


def update_dialog(m: mmap.mmap, mem: Mem) -> None:
    title, lines = snapshot_lines(mem)
    _run_op(m, OP_SETTEXT, TITLE, text=title)
    for i in range(ROW_COUNT):
        text = lines[i] if i < len(lines) else ""
        _run_op(m, OP_SETTEXT, f"{DIALOG}/ROW{i + 1:02d}", text=text)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--install", action="store_true", help="instantiate the button/dialog XML under --parent")
    ap.add_argument("--parent", default="SYSTEMMESSAGES", help="GUI parent for --install")
    ap.add_argument("--watch", action="store_true", help="keep updating until Ctrl+C")
    ap.add_argument("--interval", type=float, default=2.0, help="seconds between --watch updates")
    ap.add_argument("--show", action="store_true", help="show the dialog after updating")
    ap.add_argument("--hide", action="store_true", help="hide the dialog after updating")
    args = ap.parse_args()

    bridge = attach_bridge()
    if args.install:
        install_dialog(bridge, args.parent)
        print("installed production button/dialog")

    mem = Mem()
    try:
        while True:
            update_dialog(bridge, mem)
            if args.show:
                _run_op(bridge, OP_SETVISIBLE, DIALOG, arg="1")
            if args.hide:
                _run_op(bridge, OP_SETVISIBLE, DIALOG, arg="0")
            if not args.watch:
                break
            time.sleep(max(0.25, args.interval))
    except KeyboardInterrupt:
        print("stopped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
