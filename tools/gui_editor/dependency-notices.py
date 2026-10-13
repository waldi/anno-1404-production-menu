"""Collect the actual installed dependency licenses for the standalone package."""
import json
import os
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
env = dict(os.environ, CARGO_HOME=str(here / '.cargo-cache'))
metadata = json.loads(subprocess.check_output([
    'cargo', 'metadata', '--locked', '--offline', '--filter-platform',
    'x86_64-pc-windows-msvc', '--manifest-path', str(here / 'src-tauri/Cargo.toml'),
    '--format-version', '1'
], cwd=here, env=env))
sections = []
missing = []

def collect(name, version, license_name, folder):
    texts = []
    for file in sorted(folder.iterdir()):
        if file.is_file() and file.name.lower().startswith(('license', 'licence', 'copying', 'notice')):
            texts.append(f'{file.name}\n{file.read_text(encoding="utf-8", errors="replace")}')
    if not texts:
        fallback = here / 'licenses' / (('webview2-com' if name.startswith('webview2-com') else name) + '.txt')
        if fallback.is_file():
            texts.append(fallback.read_text(encoding='utf-8'))
        else:
            missing.append(f'{name}@{version}: {license_name}')
    sections.append(f'{name}@{version} — {license_name}\n' + '\n'.join(texts))

for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['source']:
        collect(package['name'], package['version'], package.get('license') or 'see package license', Path(package['manifest_path']).parent)
# Frontend runtime dependencies are embedded; test/build tools are not shipped.
lock = json.loads((here / 'package-lock.json').read_text())['packages']
for file in sorted((here / 'node_modules').rglob('package.json')):
    relative = file.parent.relative_to(here).as_posix()
    if relative not in lock or lock[relative].get('dev'):
        continue
    package = json.loads(file.read_text(encoding='utf-8'))
    if package.get('name') and package.get('version'):
        collect(package['name'], package['version'], str(package.get('license', 'see package license')), file.parent)
if missing:
    print('Dependencies without a top-level license file:\n' + '\n'.join(missing), file=sys.stderr)
    raise SystemExit(1)
Path(sys.argv[1]).write_text('Third-party dependency licenses\n\n' + '\n\n' + ('\n\n' + '=' * 72 + '\n\n').join(sections), encoding='utf-8')
print(f'Collected {len(sections)} dependency notices')
