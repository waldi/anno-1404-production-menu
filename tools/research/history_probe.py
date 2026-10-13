"""Read-only exploratory History Edition production probe; not a calculator backend."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
import hashlib
import struct
import time

k = c.WinDLL('kernel32', use_last_error=True)
k.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k.OpenProcess.restype = w.HANDLE
k.ReadProcessMemory.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)]
k.ReadProcessMemory.restype = w.BOOL
k.VirtualQueryEx.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t]
k.VirtualQueryEx.restype = c.c_size_t
k.CloseHandle.argtypes = [w.HANDLE]
k.QueryFullProcessImageNameW.argtypes = [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)]

class MBI(c.Structure):
    _fields_ = [('base', c.c_uint64), ('allocation', c.c_uint64),
                ('allocation_protect', w.DWORD), ('partition', w.WORD),
                ('size', c.c_uint64), ('state', w.DWORD),
                ('protect', w.DWORD), ('type', w.DWORD)]

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--pid', type=int, required=True)
    p.add_argument('--base', type=lambda x: int(x, 0), required=True)
    p.add_argument('--seconds', type=int, default=25)
    p.add_argument('--world-only', action='store_true', help='Inspect island/population paths without scanning the heap')
    args = p.parse_args()
    if c.sizeof(c.c_void_p) != 8:
        p.error('64-bit Python required')
    h = k.OpenProcess(0x410, False, args.pid)  # READ + QUERY only
    if not h:
        raise c.WinError(c.get_last_error())
    def read(addr, size):
        buf = c.create_string_buffer(size)
        got = c.c_size_t()
        if k.ReadProcessMemory(h, addr, buf, size, c.byref(got)) and got.value == size:
            return buf.raw
        return None
    try:
        path = c.create_unicode_buffer(32768)
        path_size = w.DWORD(len(path))
        if not k.QueryFullProcessImageNameW(h, 0, path, c.byref(path_size)):
            raise c.WinError(c.get_last_error())
        with open(path.value, 'rb') as executable:
            digest = hashlib.file_digest(executable, 'sha256').hexdigest()
        if digest != '53cc20b9ec1ede15c8a708d52bc2d053c25f7c6c12b59e6becc73c46e79c5142':
            raise ValueError('Unsupported executable fingerprint')
        # Reject a mismatched executable before using any version-specific RVAs.
        hdr = read(args.base, 4096)
        if not hdr or hdr[:2] != b'MZ':
            raise ValueError('No target PE at supplied module base')
        pe = struct.unpack_from('<I', hdr, 60)[0]
        if struct.unpack_from('<H', hdr, pe + 4)[0] != 0x8664:
            raise ValueError('Target is not AMD64')
        expected = b'm_Productivity\0'
        if read(args.base + 0x1903900, len(expected)) != expected:
            raise ValueError('Unsupported target: reflection string mismatch')
        def value(addr, fmt):
            raw = read(addr, struct.calcsize(fmt))
            if raw is None:
                raise ValueError(f'Unreadable value at {addr:#x}')
            return struct.unpack(fmt, raw)[0]
        def wide_name(addr):
            raw = read(addr, 32)
            if raw is None:
                raise ValueError('Unreadable city string header')
            length, capacity = struct.unpack_from('<QQ', raw, 16)
            if length > 1024 or capacity < length:
                raise ValueError('Invalid city string bounds')
            data = read(struct.unpack_from('<Q', raw)[0], length * 2) if capacity > 7 else raw[:length * 2]
            if data is None:
                raise ValueError('Unreadable city string payload')
            return data.decode('utf-16-le')
        world_checks = []
        engine = value(args.base + 0x207bab0, '<Q')
        engine_state = {'pointer': hex(engine), 'current_island': value(engine + 0x12c, '<i'),
                        'sticky_island': value(engine + 0x130, '<i'),
                        'local_player': value(engine + 0x134, '<i'),
                        'world_pointer': hex(value(engine + 0xb8, '<Q'))}
        for global_rva in (0x2097040, 0x207bab0):
            record = {'global_rva': hex(global_rva)}
            try:
                world = value(args.base + global_rva, '<Q')
                record['pointer'] = hex(world)
                record['vtable_rva'] = hex(value(world, '<Q') - args.base)
                sentinel = value(world + 0xecc0, '<Q')
                count = value(world + 0xecc8, '<Q')
                if not 0 < count < 4096:
                    raise ValueError(f'Invalid island tree count {count}')
                seen, islands = set(), []
                def walk(node, depth=0):
                    if node == sentinel:
                        return
                    if not node or node in seen or depth > 64 or len(seen) >= count:
                        raise ValueError('Invalid island tree traversal')
                    seen.add(node)
                    raw = read(node, 48)
                    if raw is None or raw[0x19]:
                        raise ValueError('Unreadable or unexpected nil tree node')
                    left, _, right = struct.unpack_from('<QQQ', raw)
                    key = struct.unpack_from('<i', raw, 0x20)[0]
                    island = struct.unpack_from('<Q', raw, 0x28)[0]
                    if value(island, '<Q') != args.base + 0x1901d68:
                        raise ValueError('Island vtable mismatch')
                    if value(island + 0x67f8, '<i') != key:
                        raise ValueError('Island map key does not match reflected island ID')
                    population = read(island + 0x5470, 11 * 32)
                    if population is None:
                        raise ValueError('Unreadable island population')
                    rows = [list(struct.unpack_from('<8i', population, tier * 32)) for tier in range(11)]
                    if any(v < 0 for row in rows for v in row):
                        raise ValueError('Negative population value')
                    walk(left, depth + 1)
                    islands.append({'id': key, 'address': hex(island),
                                    'names': [wide_name(island + 0x6148 + slot * 0x30) for slot in range(8)],
                                    'population_by_tier_player': rows})
                    walk(right, depth + 1)
                walk(value(sentinel + 8, '<Q'))
                if len(islands) != count:
                    raise ValueError('Island tree count mismatch')
                global_raw = read(world + 0xea04, 11 * 32)
                if global_raw is None:
                    raise ValueError('Unreadable global population')
                global_rows = [list(struct.unpack_from('<8i', global_raw, tier * 32)) for tier in range(11)]
                sums = [[sum(i['population_by_tier_player'][tier][slot] for i in islands)
                         for slot in range(8)] for tier in range(11)]
                record.update({'islands': islands, 'global_population_by_tier_player': global_rows,
                               'island_population_sums': sums, 'population_matches': sums == global_rows})
                # Reject common save/load changes; reads remain non-atomic.
                if value(args.base + global_rva, '<Q') != world or value(world + 0xecc0, '<Q') != sentinel or value(world + 0xecc8, '<Q') != count:
                    raise ValueError('World changed during read')
            except (ValueError, UnicodeError) as error:
                record['error'] = str(error)
            world_checks.append(record)
        needles = {struct.pack('<Q', args.base + rva): label for rva, label in
                   [(0x1902440, 'farm'), (0x1902358, 'factory'), (0x19039d8, 'production_base')]}
        hits = []
        scanned = 0
        deadline = time.monotonic() + args.seconds
        addr = 0
        mbi = MBI()
        timed_out = False
        while not args.world_only and k.VirtualQueryEx(h, addr, c.byref(mbi), c.sizeof(mbi)):
            end = mbi.base + mbi.size
            if end <= addr:
                break
            if time.monotonic() >= deadline:
                timed_out = True
                break
            if mbi.state == 0x1000 and mbi.type == 0x20000 and not mbi.protect & 0x100 and (mbi.protect & 0xff) in (2, 4, 8, 0x20, 0x40, 0x80):
                for start in range(mbi.base, end, 1024 * 1024):
                    if time.monotonic() >= deadline:
                        timed_out = True
                        break
                    data = read(start, min(1024 * 1024, end - start))
                    if not data:
                        continue
                    scanned += len(data)
                    for needle, label in needles.items():
                        pos = data.find(needle)
                        while pos >= 0:
                            obj = start + pos
                            if obj % 8 == 0:
                                raw = read(obj, 0x30)
                                if raw:
                                    owner = struct.unpack_from('<Q', raw, 8)[0]
                                    owner_data = read(owner, 0x230) if owner else None
                                    owner_slots = [hex(i) for i in range(8, 0x230, 8)
                                                   if owner_data and struct.unpack_from('<Q', owner_data, i)[0] == obj]
                                    dc = struct.unpack_from('<Q', owner_data, 0x150)[0] if owner_data else 0
                                    wp_raw = read(dc + 0x68, 8) if dc else None
                                    wp = struct.unpack('<Q', wp_raw)[0] if wp_raw else 0
                                    recipe_raw = read(wp, 48) if wp else None
                                    hits.append({'address': hex(obj), 'kind': label,
                                                 'owner': hex(owner), 'owner_readable': bool(owner_data),
                                                 'owner_vtable': hex(struct.unpack_from('<Q', owner_data)[0]) if owner_data else None,
                                                 'owner_backreference_slots': owner_slots,
                                                 'player': struct.unpack_from('<i', owner_data, 0x148)[0] if owner_data else None,
                                                 'island_pointer': hex(struct.unpack_from('<Q', owner_data, 0x170)[0]) if owner_data else None,
                                                 'data_container': hex(dc), 'recipe_pointer': hex(wp),
                                                 'recipe_raw_hex': recipe_raw.hex() if recipe_raw else None,
                                                 'production_time': struct.unpack_from('<i', raw, 0x10)[0],
                                                 'goods': struct.unpack_from('<i', raw, 0x14)[0],
                                                 'productivity': struct.unpack_from('<i', raw, 0x18)[0],
                                                 'stopped': raw[0x1c]})
                            pos = data.find(needle, pos + 1)
            addr = end
        print(json.dumps({'pid': args.pid, 'module_base': hex(args.base), 'sha256': digest,
                          'scanned_bytes': scanned, 'time_limited': timed_out,
                          'world_checks': world_checks,
                          'engine_state': engine_state,
                          'confidence': 'vtable candidates; includes stale pooled objects; UI semantics unverified',
                          'objects': hits}, indent=2))
    finally:
        k.CloseHandle(h)

if __name__ == '__main__':
    main()
