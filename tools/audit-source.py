"""Audit public-import candidates without importing or running game-facing tools."""
import ast
import json
import re
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
files = subprocess.check_output(
    ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=root
).decode().split('\0')
errors = []
count = 0
for name in sorted(set(filter(None, files))):
    path = root / name
    if not path.is_file():
        continue
    count += 1
    if path.stat().st_size > 10 * 1024 * 1024:
        errors.append(f'{name}: exceeds 10 MiB import budget')
    if path.suffix in {'.dll', '.exe', '.rda', '.ttf', '.dds', '.obj', '.pdb', '.zip'}:
        errors.append(f'{name}: binary/game/build file in import candidates')
    if path.suffix == '.py':
        ast.parse(path.read_text(encoding='utf-8-sig'), filename=name)
    if path.suffix == '.json':
        json.loads(path.read_text(encoding='utf-8-sig'))
    if path.suffix in {'.py', '.ps1', '.json', '.yml', '.md', '.rs', '.cpp', '.h'}:
        content = path.read_text(encoding='utf-8-sig')
        if re.search(r'-----BEGIN (?:RSA |OPENSSH |EC )?PRIVATE KEY-----|\bgh[pousr]_[A-Za-z0-9]{36,}\b|\bAKIA[A-Z0-9]{16}\b', content):
            errors.append(f'{name}: possible credential; inspect locally')
print(f'Audited {count} source/import candidates; {len(errors)} issues')
for error in errors:
    print(error)
raise SystemExit(bool(errors))
