"""Read-only dump of loaded static production recipes, including unbuilt types."""
import json
from annomem import Mem
from annoprod import scan_value, find_buildings, name_table, cstr, rtti_class

m = Mem()
from annomem import find_islands
islands = dict(find_islands(m))
# Obtain the static container vtable from a validated live building.
groups = find_buildings(m, {addr: iid for iid, addr in islands.items()})
building = next(b for buildings in groups.values() for b in buildings)
vt = m.u32(m.u32(building + 0xC4))
wares = name_table(m, 0x0127B820, 68)
rows = []
for dc in scan_value(m, vt):
    wp, standard = m.u32(dc + 0x34), m.u32(dc + 0x74)
    if not wp or not standard:
        continue
    if rtti_class(m, m.u32(wp)) != '.?AVCPropertyDataWareProduction@@':
        continue
    ware, cycle, amount = m.u32(wp + 12), m.u32(wp + 8), m.u32(wp + 20)
    if ware is None or ware >= len(wares) or not cycle or not amount:
        continue
    name = cstr(m, standard + 8)
    if not name.isascii() or not name.isprintable():
        name = '(name not decoded)'
    rows.append(dict(name=name, good=wares[ware],
                     cycle_ms=cycle, amount_kg=amount,
                     rate=amount / 1000 * 60000 / cycle))
print(json.dumps(sorted(rows, key=lambda r: (r['good'], r['name'])), indent=2))
