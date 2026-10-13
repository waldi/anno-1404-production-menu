"""Update the production dialog and emit a live GUI fragment."""
from pathlib import Path
import re

base = Path(__file__).resolve().parent.parent
cpp = base / 'AnnoRECompanion.cpp'
dialog = base / 'examples/production_dialog_v5.dlg'
aliases = {'DEMAND':'DEMPOT', 'CURTRACK':'DEMCUR', 'DTEXT':'DEMLABEL',
           'OUTPUT':'SUPPOT', 'CURBAR':'SUPCUR', 'CURTEXT':'SUPLABEL',
           'DTRACK':'DEMBASE', 'PTRACK':'SUPBASE'}
text = cpp.read_text()
xml = dialog.read_text()
for old, new in aliases.items():
    text = text.replace('"'+old+'"', '"'+new+'"')
    xml = re.sub(old+r'(\d+)', new+r'\1', xml)
colors = {'DEMBASE':'b29d72','SUPBASE':'b29d72','DEMCUR':'71a4b5',
          'DEMPOT':'445f38','SUPCUR':'648d56','SUPPOT':'486339'}
nodes = []
def update(match):
    node = match.group(0)
    prefix = re.search(r'<PATHID>([A-Z]+)\d+</PATHID>',node).group(1)
    if prefix not in colors and prefix not in ('DEMLABEL','SUPLABEL'):
        return node
    order = 199 if prefix.endswith('BASE') else 200 if prefix.endswith('POT') else 201
    if prefix.endswith('LABEL'): order = 205
    node = re.sub(r'<ZORDER>.*?</ZORDER>',f'<ZORDER>{order}</ZORDER>',node)
    if prefix in colors:
        node = re.sub(r'<BACKGROUND>.*?</BACKGROUND>','',node)
        node = re.sub(r'<BARCOLOR>.*?</BARCOLOR>','',node)
        node = node.replace('</PROGRESSBAR>',f'<BARCOLOR>{int(colors[prefix],16)}</BARCOLOR></PROGRESSBAR>')
    nodes.append(node)
    return node
xml = re.sub(r'<(PROGRESSBAR|LABEL)><PATHID>[A-Z]+\d+</PATHID>.*?</\1>',update,xml)
cpp.write_text(text)
dialog.write_text(xml)
(base / 'examples/demand_supply_meters.dlg').write_text('\n'.join(nodes))
print(f'Updated {len(nodes)} meter widgets')
