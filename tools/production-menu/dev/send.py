"""Feed a number to annobridge, which displays it in Anno 1404's own GUI.

The DLL creates a named shared-memory block inside the game; this writes to it.
Nothing here opens a handle on the game process.

    python tools/production-menu/dev/send.py --status
    python tools/production-menu/dev/send.py --create                 # build our own panel
    python tools/production-menu/dev/send.py --create --rect 620 150 220 24
    python tools/production-menu/dev/send.py ANNORE_BAR --sweep       # drive it
    python tools/production-menu/dev/send.py HOUSE_FOOD_DEMAND_FULFILLMENT_BAR --sweep
"""

import os
import argparse
import mmap
import struct
import sys
import time

NAME = "AnnoREBridge"
SIZE = 4096
MAGIC = 0x4F4E4E41  # 'ANNO'

# layout must match struct Bridge in annobridge.cpp (version 2)
O_MAGIC, O_VERSION, O_VALUE, O_MAXVALUE = 0, 4, 8, 12
O_WRITES, O_HITS, O_LASTTYPE, O_TARGET = 16, 20, 24, 28
O_CREATEREQ = O_TARGET + 64          # 92
O_CREATESTATUS = O_CREATEREQ + 4     # 96
O_CREATETYPE = O_CREATESTATUS + 4    # 100
O_PARENT = O_CREATETYPE + 4          # 104
O_NEWNAME = O_PARENT + 64            # 168
O_RECT = O_NEWNAME + 64              # 232
O_APPLYRECT = O_RECT + 16            # 248
O_INSPECTNAME = O_APPLYRECT + 4      # 252
O_DUMPOFF = O_INSPECTNAME + 64       # 316
O_DUMPCOUNT = O_DUMPOFF + 4          # 320
O_INSPECTPTR = O_DUMPCOUNT + 4       # 324
O_INSPECTHITS = O_INSPECTPTR + 4     # 328
O_DUMPOUT = O_INSPECTHITS + 4        # 332
O_SKININDEX = O_DUMPOUT + 256        # 588
O_SKINREQ = O_SKININDEX + 4          # 592
O_SKINSTATUS = O_SKINREQ + 4         # 596
O_POKES = O_SKINSTATUS + 4           # 600  (4 slots of {enabled, offset, value})
# v6 element operations
O_OPREQ = O_POKES + 48               # 648
O_OPSTATUS = O_OPREQ + 4             # 652
O_OPTARGET = O_OPSTATUS + 4          # 656
O_OPARG = O_OPTARGET + 64            # 720  (260 bytes: .bkg paths are ~90 chars)
O_OPTEXT = O_OPARG + 260             # 980  (128 UTF-16 code units)
O_OPRESULT = O_OPTEXT + 256          # 1236
O_LISTOUT = O_OPRESULT + 4           # 1240 (512 bytes)
O_RECTTARGET = O_LISTOUT + 512       # 1752
# v7 click -> show/hide
O_TOGGLEBTN = O_RECTTARGET + 64      # 1816
O_TOGGLETGT = O_TOGGLEBTN + 64       # 1880
O_TOGGLECOUNT = O_TOGGLETGT + 64     # 1944
O_LASTEVENT = O_TOGGLECOUNT + 4      # 1948 (32 bytes)
O_TRACE = O_LASTEVENT + 32           # 1980
O_TIPTARGET = O_TRACE + 4            # 1984 (v8.4)
O_TIPTEXT = O_TIPTARGET + 64         # 2048 .. 2304
O_TIPCOUNT = O_TIPTEXT + 256         # 2304
O_TIPMODE = O_TIPCOUNT + 4           # 2308
OP_NAMES = {1: "oncreate", 2: "settext", 3: "setbackground", 4: "setvisible", 5: "listchildren", 6: "setfont", 7: "setalign",
            8: "loadsavegame", 9: "click", 10: "xml", 11: "rebuildpaths"}
OP_STATUS = {0: "idle", 1: "pending", 2: "done", 3: "failed", 4: "no TextSetting for this type",
             5: "waiting for @source element to appear"}
NAME_LEN = 64

TYPE_NAMES = {0: "none", 1: "LABEL", 2: "BUTTON", 3: "SLIDER", 4: "FRAME", 5: "EDITFIELD", 6: "BOXLAYOUT",
              7: "COMBOBOX", 8: "LISTBOX", 9: "PROGRESSBAR", 10: "CARDLAYOUT", 12: "RENDERTARGET",
              13: "ALPHAPROGRESSBAR", 15: "VIDEOPANEL", 20: "STRUCTTEMPLATEINSTANCE"}
CREATE_STATUS = {0: "idle", 1: "requested", 2: "done", 3: "parent not found",
                 4: "bad type", 5: "failed"}


def attach():
    return mmap.mmap(-1, SIZE, tagname=NAME)


def ru(m, off):
    return struct.unpack_from("<I", m, off)[0]


def ri(m, off):
    return struct.unpack_from("<i", m, off)[0]


def rs(m, off):
    return bytes(m[off:off + NAME_LEN]).split(b"\0")[0].decode(errors="replace")


def ws(m, off, text, n=NAME_LEN):
    raw = text.encode("ascii", "strict")
    if len(raw) > n - 1:
        print(f"warning: {text!r} truncated to {n-1} bytes")
        raw = raw[:n - 1]
    m[off:off + n] = raw + bytes(n - len(raw))


def run_op(m, op, target, arg="", text=""):
    """Queue an element operation and wait for the DLL to report on it."""
    ws(m, O_OPTARGET, target)
    ws(m, O_OPARG, arg, 260)
    raw = text.encode("utf-16-le")[:254]
    m[O_OPTEXT:O_OPTEXT + 256] = raw + b"\x00" * (256 - len(raw))
    struct.pack_into("<I", m, O_OPRESULT, 0)
    m[O_LISTOUT:O_LISTOUT + 512] = bytes(512)  # no stale listing if the op never runs
    struct.pack_into("<I", m, O_OPSTATUS, 1)
    struct.pack_into("<I", m, O_OPREQ, op)
    print(f"{OP_NAMES[op]} on {target!r}" + (f" arg={arg!r}" if arg else "") + (f" text={text!r}" if text else ""))
    for _ in range(50):
        time.sleep(0.1)
        st = ru(m, O_OPSTATUS)
        if st not in (1, 5):
            break
    st = ru(m, O_OPSTATUS)
    res = ru(m, O_OPRESULT)
    print(f"-> {st} {OP_STATUS.get(st, '?')}" + (f"   bg object 0x{res:08X}" if op in (1, 3) else ""))
    if st == 1:
        print("   (element never appeared - is it on screen?)")
        struct.pack_into("<I", m, O_OPREQ, 0)
    return st == 2


def status(m):
    magic = ru(m, O_MAGIC)
    cs = ru(m, O_CREATESTATUS)
    t = ru(m, O_LASTTYPE)
    rect = [ri(m, O_RECT + 4 * i) for i in range(4)]
    return (
        f"magic     {'ok (v%d)' % ru(m, O_VERSION) if magic == MAGIC else '0x%08X (DLL not loaded?)' % magic}\n"
        f"target    {rs(m, O_TARGET) or '(unset)'}\n"
        f"value     {ri(m, O_VALUE)}   max {ri(m, O_MAXVALUE)}\n"
        f"writes    {ru(m, O_WRITES)}   hits {ru(m, O_HITS)}\n"
        f"lastType  {t} {TYPE_NAMES.get(t, '?')}\n"
        f"create    {cs} {CREATE_STATUS.get(cs, '?')}   "
        f"parent={rs(m, O_PARENT) or '-'} new={rs(m, O_NEWNAME) or '-'} type={ri(m, O_CREATETYPE)}\n"
        f"rect      {rect}  applied={ru(m, O_APPLYRECT)}"
    )


def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("target", nargs="?", help="ACCESSID or PATHID of the widget to drive")
    p.add_argument("--value", type=int)
    p.add_argument("--max", type=int, default=100)
    p.add_argument("--sweep", action="store_true", help="ramp 0..max forever")
    p.add_argument("--clock", action="store_true", help="seconds past the minute, forever")
    p.add_argument("--status", action="store_true")
    # creation
    p.add_argument("--create", action="store_true",
                   help="ask the DLL to build a widget via the game's own factory")
    p.add_argument("--parent", default="INFOBAR", help="existing element to parent under")
    p.add_argument("--name", default="ANNORE_BAR", help="PATHID for the new widget")
    p.add_argument("--type", type=int, default=9, help="element type enum (9 = PROGRESSBAR)")
    p.add_argument("--rect", nargs=4, type=int,
                   metavar=("LEFT", "TOP", "RIGHT", "BOTTOM"),
                   help="CRect written to element+0x2A4 every frame. NOT x/y/w/h: "
                        "right and bottom are absolute, so 620 150 820 174 is a "
                        "200x24 bar at (620,150)")
    # introspection / skinning
    p.add_argument("--no-rect", action="store_true",
                   help="stop forcing geometry, so the widget keeps its own")
    p.add_argument("--inspect", metavar="NAME",
                   help="dump raw dwords from an element (ACCESSID or PATHID)")
    p.add_argument("--at", default="0x2A4", help="byte offset to dump from (default 0x2A4)")
    p.add_argument("--count", type=int, default=8, help="dwords to dump, max 64")
    p.add_argument("--skin-index", type=int, metavar="N",
                   help="apply registry template N to the target element")
    p.add_argument("--poke", nargs=2, action="append", metavar=("OFFSET", "VALUE"),
                   help="write VALUE to target+OFFSET every frame; repeatable, max 4 "
                        "(e.g. --poke 0x878 51000 to set a BUTTON's ICONGUID)")
    p.add_argument("--clear-pokes", action="store_true")
    # v6 element operations
    p.add_argument("--oncreate", metavar="NAME",
                   help="call the element's OnCreate (vtable +0x58) so its BACKGROUND object is built")
    p.add_argument("--text", nargs=2, metavar=("NAME", "TEXT"),
                   help="set arbitrary text on a LABEL / BUTTON / EDITFIELD")
    p.add_argument("--background", nargs=2, metavar=("NAME", "BG"),
                   help="SetBackground(BG) on NAME; BG may be @OTHER to copy OTHER's BACKGROUND")
    p.add_argument("--font", nargs=2, metavar=("NAME", "FONT"),
                   help="set the text font by localisation font name, e.g. 'Tooltip'")
    p.add_argument("--align", nargs=2, metavar=("NAME", "H,V"),
                   help="text alignment: H 0 left/1 centre/2 right, V 0 top/4 middle/8 bottom")
    p.add_argument("--children", metavar="NAME",
                   help="list the direct children of NAME (PATHID, ACCESSID, type, visible flag)")
    p.add_argument("--visible", nargs=2, metavar=("NAME", "0|1"),
                   help="SetVisible on NAME. NAME may be a full path like INFOBAR/ANNORE_BAR, "
                        "which also reaches hidden elements")
    p.add_argument("--toggle", nargs=2, metavar=("BUTTON", "TARGET"),
                   help="v7: when BUTTON receives OnLeftClick, flip TARGET's visibility. "
                        "TARGET may be a full path; BUTTON is matched by ACCESSID/PATHID")
    p.add_argument("--toggle-off", action="store_true", help="clear the click binding")
    p.add_argument("--load", metavar="SWW",
                   help="v7: load a savegame (path or bare name in Documents/ANNO 1404 Venice/Savegames) "
                        "via MainControl_LoadSavegameFile; works from the main menu and in-game")
    p.add_argument("--click", metavar="NAME",
                   help="v7: dispatch OnLeftClick (or --event) to NAME through the game's event system")
    p.add_argument("--event", default="", help="event name for --click (default OnLeftClick)")
    p.add_argument("--xml", metavar="FILE",
                   help="v8: build a dialog subtree from a .dlg fragment under --parent, through the "
                        "game's own XML node objects and GuiElement_InstantiateXmlSubtree. "
                        "PATHID/ACCESSID inside must be unique (a duplicate ACCESSID pops a game message box)")
    p.add_argument("--rebuild-paths", action="store_true",
                   help="v8: GuiRegistry_RebuildPathMap, so game-created children (scrollbars) get full paths")
    p.add_argument("--tooltip", nargs=2, metavar=("NAME", "TEXT"),
                   help="v8.4: replace the tooltip text of NAME (the element needs a <TOOLTIP><TEXT>guid so a tooltip shows at all); empty TEXT clears")
    p.add_argument("--tooltip-mode", type=int, choices=[1, 2],
                   help="v8.4: 1 = replace text only (default), 2 = also re-run the tooltip layout (crashed once)")
    p.add_argument("--trace", choices=["0", "1"],
                   help="v7.1: 1 logs every non-render GUI event with its element to %%TEMP%%\\annobridge.log")
    a = p.parse_args()

    m = attach()

    if a.tooltip_mode is not None:
        struct.pack_into("<I", m, O_TIPMODE, a.tooltip_mode)
        print(f"tooltip mode {a.tooltip_mode}")
        return 0
    if a.tooltip:
        raw = a.tooltip[1].encode("utf-16-le")[:254]
        m[O_TIPTEXT:O_TIPTEXT + 256] = raw + b"\x00" * (256 - len(raw))
        ws(m, O_TIPTARGET, a.tooltip[0] if a.tooltip[1] else "")
        print(f"tooltip override on {a.tooltip[0]!r}: {a.tooltip[1]!r} (applied {ru(m, O_TIPCOUNT)} times so far)")
        return 0
    if a.trace is not None:
        struct.pack_into("<I", m, O_TRACE, int(a.trace))
        print(f"event trace {'on' if a.trace == '1' else 'off'}")
        return 0

    if a.status:
        print(status(m))
        btn, tgt = rs(m, O_TOGGLEBTN), rs(m, O_TOGGLETGT)
        if btn:
            ev = bytes(m[O_LASTEVENT:O_LASTEVENT + 32]).split(b"\x00")[0].decode("ascii", errors="backslashreplace")
            print(f"toggle    {btn} -> {tgt}   clicks {ru(m, O_TOGGLECOUNT)}   last event on button: {ev or '-'}")
        return 0

    if a.toggle_off:
        ws(m, O_TOGGLEBTN, "")
        ws(m, O_TOGGLETGT, "")
        print("toggle binding cleared")
        return 0
    if a.toggle:
        ws(m, O_TOGGLETGT, a.toggle[1])
        struct.pack_into("<I", m, O_TOGGLECOUNT, 0)
        m[O_LASTEVENT:O_LASTEVENT + 32] = bytes(32)
        ws(m, O_TOGGLEBTN, a.toggle[0])      # last: arms the binding
        print(f"clicks on {a.toggle[0]} now toggle {a.toggle[1]}")
        return 0

    if a.no_rect:
        struct.pack_into("<I", m, O_APPLYRECT, 0)
        print("geometry override off")

    if a.rect:
        for i, v in enumerate(a.rect):
            struct.pack_into("<i", m, O_RECT + 4 * i, v)
        struct.pack_into("<I", m, O_APPLYRECT, 1)
        ws(m, O_RECTTARGET, a.target or "")
        print(f"rect set to {a.rect} for {a.target or 'the current target'} (applied every frame)")

    if a.clear_pokes:
        for i in range(4):
            struct.pack_into("<I", m, O_POKES + 12 * i, 0)
        print("pokes cleared")

    if a.poke:
        if not a.target:
            p.error("--poke needs a target element")
        ws(m, O_TARGET, a.target)
        for i, (off, val) in enumerate(a.poke[:4]):
            o, v = int(off, 0), int(val, 0)
            struct.pack_into("<III", m, O_POKES + 12 * i, 1, o, v & 0xFFFFFFFF)
            print(f"poke +0x{o:X} = {v} (0x{v & 0xFFFFFFFF:08X})")
        if len(a.poke) > 4:
            print(f"warning: only the first 4 pokes are used ({len(a.poke)} given)")
        if not (a.sweep or a.clock or a.value is not None):
            return 0

    if a.rebuild_paths:
        return 0 if run_op(m, 11, "") else 1
    if a.xml:
        path = os.path.abspath(a.xml)
        if not os.path.isfile(path):
            print(f"no such file: {path}")
            return 1
        if len(path) > 259:
            print("path too long for the 260-byte arg field")
            return 1
        parent = a.parent if a.parent != "INFOBAR" or "--parent" in sys.argv else "SYSTEMMESSAGES"
        ok = run_op(m, 10, parent, arg=path)
        n = ri(m, O_OPRESULT)
        print(f"   {n} new direct children under {parent}" if ok else "   see %TEMP%\\annobridge.log")
        return 0 if ok else 1
    if a.load:
        path = a.load
        if not os.path.isabs(path):
            base = os.path.join(os.path.expanduser("~"), "Documents", "ANNO 1404 Venice", "Savegames")
            if not path.lower().endswith(".sww"):
                path += ".sww"
            path = os.path.join(base, path)
        if not os.path.isfile(path):
            print(f"no such savegame: {path}")
            return 1
        if len(path) > 127:
            print("path too long for the 128-char text field")
            return 1
        return 0 if run_op(m, 8, "", text=path) else 1
    if a.click:
        return 0 if run_op(m, 9, a.click, arg=a.event) else 1
    if a.oncreate:
        return 0 if run_op(m, 1, a.oncreate) else 1
    if a.text:
        return 0 if run_op(m, 2, a.text[0], text=a.text[1]) else 1
    if a.background:
        return 0 if run_op(m, 3, a.background[0], arg=a.background[1]) else 1
    if a.visible:
        return 0 if run_op(m, 4, a.visible[0], arg=a.visible[1]) else 1
    if a.font:
        return 0 if run_op(m, 6, a.font[0], arg=a.font[1]) else 1
    if a.align:
        return 0 if run_op(m, 7, a.align[0], arg=a.align[1]) else 1
    if a.children:
        ok = run_op(m, 5, a.children)
        raw = bytes(m[O_LISTOUT:O_LISTOUT + 512]).split(b"\x00")[0].decode(errors="replace")
        for entry in filter(None, raw.split(";")):
            path, access, typ, vis = (entry.split("|") + ["", "", "", ""])[:4]
            print(f"  {path:<40} {access:<40} type {typ:>2} {TYPE_NAMES.get(int(typ or 0), '?'):<12} visible={vis}")
        return 0 if ok else 1

    if a.inspect:
        ws(m, O_INSPECTNAME, a.inspect)
        off = int(a.at, 0)
        n = max(1, min(64, a.count))
        struct.pack_into("<I", m, O_DUMPOFF, off)
        struct.pack_into("<I", m, O_DUMPCOUNT, n)
        before = ru(m, O_INSPECTHITS)
        for _ in range(50):
            time.sleep(0.1)
            if ru(m, O_INSPECTHITS) != before:
                break
        else:
            print(f"element {a.inspect!r} never appeared (is it on screen?)")
            return 1
        ptr = ru(m, O_INSPECTPTR)
        print(f"{a.inspect}  @ 0x{ptr:08X}   dump from +0x{off:X}")
        for i in range(n):
            v = ri(m, O_DUMPOUT + 4 * i)
            print(f"  +0x{off + 4*i:04X}  {v:12d}  0x{v & 0xFFFFFFFF:08X}")
        return 0

    if a.skin_index is not None:
        if not a.target:
            p.error("--skin-index needs a target element")
        ws(m, O_TARGET, a.target)
        struct.pack_into("<i", m, O_SKININDEX, a.skin_index)
        struct.pack_into("<I", m, O_SKINSTATUS, 0)
        struct.pack_into("<I", m, O_SKINREQ, 1)
        print(f"applying template {a.skin_index} to {a.target!r}")
        for _ in range(50):
            time.sleep(0.1)
            if ru(m, O_SKINSTATUS) != 0:
                break
        st = ru(m, O_SKINSTATUS)
        print("->", {0: "no response", 2: "done", 3: "bad index", 4: "failed"}.get(st, st))
        return 0 if st == 2 else 1

    if a.create:
        ws(m, O_PARENT, a.parent)
        ws(m, O_NEWNAME, a.name)
        struct.pack_into("<i", m, O_CREATETYPE, a.type)
        struct.pack_into("<I", m, O_CREATESTATUS, 0)
        struct.pack_into("<I", m, O_CREATEREQ, 1)
        print(f"requested: type {a.type} named {a.name!r} under {a.parent!r}")
        print("(the DLL acts on the next GUI event - be in-game, not in a menu)")
        for _ in range(50):
            time.sleep(0.1)
            if ru(m, O_CREATESTATUS) != 0:
                break
        cs = ru(m, O_CREATESTATUS)
        print(f"-> {cs} {CREATE_STATUS.get(cs, '?')}")
        if cs != 2:
            return 1
        if not a.target:
            a.target = a.name

    if not a.target:
        if a.rect or a.no_rect or a.clear_pokes:
            return 0
        p.error("a target is required unless --status or --create is given")

    ws(m, O_TARGET, a.target)
    print(f"target set to {a.target!r}")

    if a.sweep or a.clock:
        print("streaming - Ctrl+C to stop")
        try:
            i = 0
            while True:
                v = (time.localtime().tm_sec * a.max // 60) if a.clock else (i % (a.max + 1))
                struct.pack_into("<i", m, O_VALUE, v)
                struct.pack_into("<i", m, O_MAXVALUE, a.max)
                struct.pack_into("<I", m, O_WRITES, (ru(m, O_WRITES) + 1) & 0xFFFFFFFF)
                print(f"\rvalue={v:<6} hits={ru(m, O_HITS):<10}", end="", flush=True)
                i += 1
                time.sleep(0.1)
        except KeyboardInterrupt:
            print("\nstopped")
        return 0

    struct.pack_into("<i", m, O_VALUE, a.value if a.value is not None else 50)
    struct.pack_into("<i", m, O_MAXVALUE, a.max)
    struct.pack_into("<I", m, O_WRITES, (ru(m, O_WRITES) + 1) & 0xFFFFFFFF)
    print(status(m))
    return 0


if __name__ == "__main__":
    sys.exit(main())
