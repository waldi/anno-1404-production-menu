# Anno 1404 Production Menu

An experimental in-game production overview for Anno 1404 Venice, with a standalone GUI authoring tool and reverse-engineering utilities.

| Component | Purpose | Source |
| --- | --- | --- |
| Production Menu | Native in-game panel; separate GOG x86 and History x64 packages | [tools/production-menu](tools/production-menu/README.md) |
| Anno GUI Studio | Windows x64 desktop editor and preview for game GUI definitions | [tools/gui_editor](tools/gui_editor/STANDALONE.md) |
| Research tools | Live population, production and GUI inspection, probes and debugging | [tools/research](tools/research/README.md) |

[Download releases](https://github.com/waldi/anno-1404-production-menu/releases). Production Menu supports the exact executable builds listed in its installation guide and requires DirectX 9. GUI Studio requires WebView2, your own game installation and separately supplied RDA Explorer dependencies. The editor saves working copies rather than installing edits.

Research memory readers use read-only process handles. The Production Menu DLL runs inside the game and hooks/writes its GUI. Debugging and development helpers have additional side effects; consult their documentation before using them. Current/potential production estimates are distinct from actual delivered flow.

## Development

```powershell
./tools/ci.ps1 -Product production-menu
./tools/ci.ps1 -Product gui-studio
./tools/ci.ps1 -Product research
```

Native builds require MSVC x86/x64 tools. Studio requires Node (version in .node-version), pinned Rust and MSVC. Public CI builds from source without game assets. Local fidelity checks require extracted resources and captures; see [build and release instructions](docs/releasing.md).

## Knowledge base

- [AGENTS.md](AGENTS.md): repository safety and verification rules.
- [findings.json](docs/findings.json): runtime contracts, confidence and evidence.
- [production](docs/production.md), [GUI system](docs/gui-system.md), [edition compatibility](docs/edition-compatibility.md): research context.
- Ghidra is the canonical static structure/function source; its local database and game/dependency trees are excluded from Git.

Original project code is licensed under [GNU GPL v3](LICENSE) (GPL-3.0-only). Game resources are not licensed by this project and are not included in release archives. See [third-party and fixture notes](THIRD_PARTY_NOTICES.md).
