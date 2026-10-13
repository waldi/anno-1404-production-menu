"""Read-only dump of GUI-system tables from the running Addon.exe.

    python tools/research/annogui.py verbs        # event-handler action verbs: id, queue flag, handler class
    python tools/research/annogui.py templates    # CGUITemplate vector + name map (index = element +0x14C)
    python tools/research/annogui.py structs      # CGUIWndStructTemplate vector + name map (STRUCTTEMPLATEID)
    python tools/research/annogui.py xmldocs      # documents held in the runtime XML context (templates by name)
    python tools/research/annogui.py fonts        # CFontContainer map of the text engine (font names usable in <FONT>)

Everything here is ReadProcessMemory only (annomem.Mem); nothing is written.

Sources (Addon.exe, Ghidra):
  g_pGuiManager 0x012D334C -> CGUIManager
     +0x8C  map<stlp_string,int>  verb -> action id  (GuiManager_GetActionId 0x00D75560)
     +0xA4  map<stlp_string,u8>   verb -> queued flag (GuiManager_GetActionQueueFlag 0x00D75590)
     +0xBC/+0xC0 vector<listener*>, index = id >> 20, vtable+0x10 = Execute(record, args)
     +0x42C XML document context (CConfigDocContext, ctor 0x00428670)
  GuiRegistry 0x012D8F78 (GuiRegistry_Get returns the address itself)
     +0x1DC/+0x1E0 vector<CGUITemplate*>          +0x1E8 map<name,int> (GuiElement_ReadDefinitionNode)
     +0x218/+0x21C vector<CGUIWndStructTemplate*>  +0x224 map<name,int> (0x00DBADD0)
  STLport map node: {color, parent, left, right} + key stlp_string at +0x10, value at +0x30.
"""
import struct
import sys

sys.path.insert(0, __file__.rsplit("\\", 1)[0] if "\\" in __file__ else ".")
import annomem  # noqa: E402

G_GUIMGR = 0x012D334C
G_REGISTRY = 0x012D8F78


def stlp_string(m, addr):
    end = m.u32(addr + 0x14)
    fin = m.u32(addr + 0x10)
    if end is None or fin is None:
        return "<unreadable>"
    start = addr if end == addr + 0x10 else m.u32(addr)
    n = fin - start
    if n < 0 or n > 512:
        return f"<bad str @{addr:08X}>"
    b = m.read(start, n)
    return b.decode("latin1") if b else ""


def cstr(m, addr, n=0x104):
    b = m.read(addr, n) or b""
    return b.split(b"\0")[0].decode("latin1")


def walk_map(m, hdr, valfmt):
    """In-order walk of an STLport _Rb_tree; hdr = address of the map object."""
    root = m.u32(hdr + 4)
    out = []

    def rec(n, depth=0):
        if not n or depth > 64:
            return
        rec(m.u32(n + 8), depth + 1)
        raw = m.read(n + 0x30, 4) or b"\0\0\0\0"
        out.append((stlp_string(m, n + 0x10), struct.unpack(valfmt, raw[:struct.calcsize(valfmt)])[0]))
        rec(m.u32(n + 0xC), depth + 1)

    rec(root)
    return out


def rtti_name(m, obj):
    vt = m.u32(obj)
    col = m.u32(vt - 4) if vt else None
    td = m.u32(col + 0xC) if col else None
    name = cstr(m, td + 8, 96) if td else "?"
    return name, vt or 0


def vector_ptrs(m, base):
    beg, end = m.u32(base), m.u32(base + 4)
    return [m.u32(beg + 4 * i) for i in range((end - beg) // 4)]


def cmd_verbs(m):
    mgr = m.u32(G_GUIMGR)
    ids = dict(walk_map(m, mgr + 0x8C, "<i"))
    flags = dict(walk_map(m, mgr + 0xA4, "<B"))
    groups = []
    for i, p in enumerate(vector_ptrs(m, mgr + 0xBC)):
        groups.append((i, p) + (rtti_name(m, p) if p else ("-", 0)))
    print(f"g_pGuiManager = {mgr:08X}   {len(ids)} verbs")
    print("\nhandler groups (index = id >> 20):")
    for i, p, name, vt in groups:
        if p:
            print(f"  [{i:2}] obj {p:08X} vtable {vt:08X} {name}")
    print("\nverbs:  id        grp sub   queued  verb                                     handler")
    for k, v in sorted(ids.items(), key=lambda kv: kv[1]):
        g = v >> 20
        gname = groups[g][2] if 0 <= g < len(groups) else "?"
        print(f"  {v:08X}  {g:3} {v & 0xFFFFF:5}   q={flags.get(k, 0)}   {k:40s} {gname}")


def cmd_templates(m, vec_off, map_off, label):
    reg = G_REGISTRY
    ptrs = vector_ptrs(m, reg + vec_off)
    names = dict(walk_map(m, reg + map_off, "<i"))
    byidx = {v: k for k, v in names.items()}
    print(f"{label}: {len(ptrs)} entries in vector, {len(names)} names in map")
    for i, p in enumerate(ptrs):
        objname = cstr(m, p) if p else "-"
        mapname = byidx.get(i, "")
        flag = "" if objname == mapname or not mapname else f"   (map says {mapname})"
        extra = ""
        if p and vec_off == 0x1DC:
            extra = f"  accessid={cstr(m, p + 0x104, 0x41)!r} ref={cstr(m, p + 0x145, 0x41)!r}"
        print(f"  [{i:3}] {p:08X} {objname}{extra}{flag}")


def cmd_xmldocs(m):
    mgr = m.u32(G_GUIMGR)
    ctx = m.u32(mgr + 0x42C)
    root = ctx + 4                    # embedded root CXMLExtNode, children list at +8
    print(f"XML context {ctx:08X}, root node {root:08X} name={stlp_string(m, root + 0x1C)!r}")
    n = m.u32(root + 8)
    count = 0
    while n and count < 20000:
        node = m.u32(n)
        first = m.u32(node + 8)
        child = m.u32(first) if first else 0
        tag = stlp_string(m, child + 0x1C) if child else ""
        print(f"  {node:08X} {stlp_string(m, node + 0x1C):48s} -> <{tag}>")
        n = m.u32(n + 4)
        count += 1
    print(f"{count} documents")


G_TEXTENGINE = 0x012D3038


def cmd_fonts(m):
    te = m.u32(G_TEXTENGINE)
    print(f"g_pTextEngine = {te:08X}; font map at +0x10 (name -> CFontContainer*)")
    for name, ptr in walk_map(m, te + 0x10, "<I"):
        print(f"  {ptr:08X} {name}")


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ("verbs", "templates", "structs", "xmldocs", "fonts"):
        print(__doc__)
        sys.exit(2)
    m = annomem.Mem()
    cmd = sys.argv[1]
    if cmd == "verbs":
        cmd_verbs(m)
    elif cmd == "templates":
        cmd_templates(m, 0x1DC, 0x1E8, "CGUITemplate (element +0x14C / TEMPLATE_REF_ID)")
    elif cmd == "structs":
        cmd_templates(m, 0x218, 0x224, "CGUIWndStructTemplate (STRUCTTEMPLATEID)")
    elif cmd == "fonts":
        cmd_fonts(m)
    else:
        cmd_xmldocs(m)


if __name__ == "__main__":
    main()
