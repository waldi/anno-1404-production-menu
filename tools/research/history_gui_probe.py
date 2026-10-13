"""Read-only History GUI registry and bridge diagnostics (exact supported binary)."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import re
import struct

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--pid', type=int, required=True)
p.add_argument('--base', type=lambda s: int(s, 0), required=True)
a = p.parse_args()
k = c.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k.OpenProcess.restype = w.HANDLE
k.ReadProcessMemory.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)]
k.CloseHandle.argtypes = [w.HANDLE]
k.QueryFullProcessImageNameW.argtypes = [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)]
h = k.OpenProcess(0x410, False, a.pid)
if not h:
    raise c.WinError(c.get_last_error())
def read(addr, size):
    b = c.create_string_buffer(size)
    n = c.c_size_t()
    if not k.ReadProcessMemory(h, addr, b, size, c.byref(n)) or n.value != size:
        raise ValueError(f'Unreadable {addr:#x}')
    return b.raw
def q(addr):
    return struct.unpack('<Q', read(addr, 8))[0]
try:
    path = c.create_unicode_buffer(32768)
    length = w.DWORD(len(path))
    if not k.QueryFullProcessImageNameW(h, 0, path, c.byref(length)):
        raise c.WinError(c.get_last_error())
    with open(path.value, 'rb') as f:
        if hashlib.file_digest(f, 'sha256').hexdigest() != '53cc20b9ec1ede15c8a708d52bc2d053c25f7c6c12b59e6becc73c46e79c5142':
            raise ValueError('Unsupported binary')
    if read(a.base, 2) != b'MZ':
        raise ValueError('Invalid module base')
    head = q(a.base + 0x207b310 + 0x1d8)
    count = q(a.base + 0x207b310 + 0x1e0)
    seen = set()
    entries = []
    def walk(node, depth=0):
        if not node or node == head:
            return
        if node in seen or depth > 64 or len(seen) > 20000:
            raise ValueError('Invalid registry tree')
        seen.add(node)
        raw = read(node, 0x58)
        if raw[0x19]:
            return
        left, _, right = struct.unpack_from('<QQQ', raw)
        size, cap = struct.unpack_from('<QQ', raw, 0x30)
        if size > 4096 or cap < size:
            raise ValueError('Invalid registry key')
        key = (read(struct.unpack_from('<Q', raw, 0x20)[0], size) if cap > 15 else raw[0x20:0x20+size]).decode('ascii', 'replace')
        widget = struct.unpack_from('<Q', raw, 0x50)[0]
        if '/' not in key or 'ANNORE' in key:
            entry = {'path': key, 'widget': hex(widget), 'type': struct.unpack('<i', read(widget + 0x118, 4))[0]}
            if 'ANNORE' in key:
                try:
                    entry['rect_2f8'] = list(struct.unpack('<iiii', read(widget + 0x2f8, 16)))
                    entry['access_id'] = read(widget + 0x15d, 64).split(b'\0')[0].decode('ascii', 'replace')
                    entry['event_binding'] = read(widget + 0x42c, 64).split(b'\0')[0].decode('ascii', 'replace')
                    entry['event_map'] = hex(q(widget + 0x110))
                except ValueError:
                    pass
            entries.append(entry)
        walk(left, depth+1)
        walk(right, depth+1)
    walk(q(head+8))
    manager = q(a.base + 0x2097088)
    context = q(manager + 0x5a8) if manager else 0
    documents = []
    def native_string(addr):
        raw = read(addr, 32)
        size, cap = struct.unpack_from('<QQ', raw, 16)
        if size > 4096 or cap < size:
            return '<invalid>'
        return (read(struct.unpack_from('<Q', raw)[0], size) if cap > 15 else raw[:size]).decode('ascii', 'replace')
    def event_names(widget):
        tree = q(widget + 0x110)
        head = q(tree)
        names = []
        seen_nodes = set()
        def visit(node, depth=0):
            if not node or node == head:
                return
            if node in seen_nodes or depth > 16 or len(seen_nodes) > 64:
                raise ValueError('Invalid widget event map')
            seen_nodes.add(node)
            raw = read(node, 0x58)
            if raw[0x19]:
                return
            left, _, right = struct.unpack_from('<QQQ', raw)
            size, cap = struct.unpack_from('<QQ', raw, 0x30)
            if size > 256 or cap < size:
                raise ValueError('Invalid event name bounds')
            key = (read(struct.unpack_from('<Q', raw, 0x20)[0], size) if cap > 15 else raw[0x20:0x20+size]).decode('ascii', 'replace')
            names.append(key)
            visit(left, depth + 1)
            visit(right, depth + 1)
        visit(q(head + 8))
        return names
    if context:
        root = context + 8
        link = q(root + 0x10)
        count_links = 0
        while link and count_links < 2000:
            count_links += 1
            node = q(link)
            name = native_string(node + 0x38)
            if 'PRODUCTION_' in name.upper():
                nodes = []
                child = q(node + 0x10)
                while child and len(nodes) < 8:
                    value = q(child)
                    nodes.append({'name': native_string(value+0x38), 'comment': read(value+8,1)[0], 'children': hex(q(value+0x10))})
                    child = q(child+8)
                documents.append({'name': name, 'nodes': nodes})
            link = q(link + 8)
    k.OpenFileMappingW.argtypes = [w.DWORD, w.BOOL, w.LPCWSTR]
    k.OpenFileMappingW.restype = w.HANDLE
    k.MapViewOfFile.argtypes = [w.HANDLE, w.DWORD, w.DWORD, w.DWORD, c.c_size_t]
    k.MapViewOfFile.restype = c.c_void_p
    k.UnmapViewOfFile.argtypes = [c.c_void_p]
    mapping = k.OpenFileMappingW(4, False, 'AnnoREHistoryBridge')
    transport = {}
    if mapping:
        view = k.MapViewOfFile(mapping, 4, 0, 0, 4096)
        if view:
            raw = c.string_at(view, 4096)
            transport = {'operation': struct.unpack_from('<I', raw, 648)[0], 'status': struct.unpack_from('<I', raw, 652)[0], 'target': raw[656:720].split(b'\0')[0].decode('ascii', 'replace'), 'result': struct.unpack_from('<I', raw, 1236)[0]}
            k.UnmapViewOfFile(view)
        k.CloseHandle(mapping)
    batch = {}
    mapping = k.OpenFileMappingW(4, False, 'AnnoREHistoryUiV4')
    if mapping:
        view = k.MapViewOfFile(mapping, 4, 0, 0, 69836)
        if view:
            raw = c.string_at(view, 69836)
            state, updates, result, sequence = struct.unpack_from('<IIII', raw, 16)
            paths = {e['path']: e for e in entries}
            failures = []
            for i in range(min(updates,192)):
                at = 1484 + i * 356
                target = raw[at:at+64].split(b'\0')[0].decode('ascii','replace')
                flags, visible, bar, maximum, icon, left, top, right, bottom = struct.unpack_from('<Iiiiiiiii',raw,at+320)
                widget = paths.get(target)
                reason = 'missing' if widget is None else ''
                if widget:
                    kind = widget['type']
                    if (flags & 4 and kind != 2) or (flags & 1 and kind not in (1,2)) or (flags & 64 and kind != 1): reason = 'type'
                    if flags & 32 and (kind != 9 or maximum <= 1 or bar < 0 or bar > maximum): reason = 'bar'
                    if flags & 16 and (right <= left or bottom <= top): reason = 'rect'
                if reason: failures.append({'target':target, 'flags':flags, 'type':widget['type'] if widget else None, 'reason':reason,'bar':bar,'max':maximum})
            written = struct.unpack_from('<I', raw, 32)[0]
            clicks = []
            start = max(0, written - 10)
            for seq in range(start, written):
                at = 204 + (seq % 64) * 20
                row, ctrl, island, player, tick = struct.unpack_from('<iIiiI', raw, at)
                clicks.append({'seq':seq+1,'row':row,'ctrl':ctrl,'island':island,'player':player,'tick':tick})
            batch = {'state':state, 'result':result, 'sequence':sequence, 'viewport':[struct.unpack_from('<ii',raw,36)], 'clicks':clicks, 'failures':failures}
            k.UnmapViewOfFile(view)
        k.CloseHandle(mapping)
    controls = [e for e in entries if re.search(r'/(?:ANNORE_ISLAND_ROW(?:0[1-4]|2[0-3])|FILTER[0-3]|CLOSE)$', e['path'])]
    for entry in controls:
        try:
            entry['events'] = event_names(int(entry['widget'], 16))
        except (ValueError, OSError):
            entry['events'] = ['<unreadable>']
    print(json.dumps({'pid': a.pid, 'count': count, 'visited': len(seen), 'gui_thread': struct.unpack('<I', read(manager+0x5d0, 4))[0] if manager else None, 'transport': transport, 'batch':batch, 'documents': documents, 'roots': sum('/' not in e['path'] for e in entries), 'custom_count': sum('ANNORE' in e['path'] for e in entries), 'controls':controls}, indent=2))
finally:
    k.CloseHandle(h)
