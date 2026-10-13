# Shared research tools

These Python tools belong to the research toolkit, not either product's release package. Run from the repository root; Windows is required for process inspection.

| Script | Purpose |
| --- | --- |
| annomem.py | Read-only live memory, island/population/name inspection |
| annoprod.py | Read-only production, warehouse, demand and measured flow |
| annogui.py | Read-only GUI registry, XML, templates, verbs and fonts |
| annowatch.py | Live debugger/watchpoints; changes process execution |
| findpop.py | Population offset correlation research |
| check_production_rates.py | Loaded recipe data probe |
| history_probe.py | Fingerprint-guarded History production research |
| history_gui_probe.py | History GUI inspection research |

```powershell
python tools/research/annomem.py islands -p 0 --nonzero
python tools/research/annoprod.py --stock
python tools/research/annogui.py verbs
```

Keep these scripts together: several import annomem/annoprod. Native dev/prod_ui.py adds this directory to its import path. Syntax auditing never imports these scripts because some perform live inspection at module scope. See ../../AGENTS.md for target restrictions and sandbox process visibility cautions.
