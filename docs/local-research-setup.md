# Local research prerequisites

The public checkout contains product source, build scripts, research utilities and documented findings. The development machine also has local tools and game resources that are deliberately excluded from Git:

| Excluded local path | What it contains |
| --- | --- |
| `ghidra_12.1.2/` | A local Ghidra installation; not a bundled project dependency |
| `ghidra-mcp/` | The locally configured Ghidra MCP integration |
| `ghidra-project/` | Binary Ghidra databases with analyzed programs, types, names and comments |
| `Anno 1404 Gold Edition/` | The local licensed game installation, including `Addon.exe` and archives |
| `data/` | Extracted game resources used by local GUI fidelity checks |
| `rda-explorer/` | Separately supplied archive reader and its dependencies |
| `anno-1404-warenrechner/` | A third-party research reference checkout |
| `.mcp.json` | Machine-specific MCP configuration |

None of these are downloaded automatically when you clone the repository. Game binaries and assets must come from your own licensed installation. Ghidra, its MCP integration and RDA Explorer must be installed/configured separately if your investigation needs them. The public checkout does not reconstruct the annotated Ghidra databases from findings.json.

## Which prerequisites do you need?

- **Install Production Menu:** only your supported game installation and the matching release ZIP. No research tools are needed.
- **Build release packages:** the compiler/toolchains described in [releasing.md](releasing.md). Ghidra, MCP and a game installation are not required for public CI tests or compilation.
- **Use GUI Studio:** your game installation, WebView2 and separately supplied RDA Explorer dependencies, as described in [standalone setup](../tools/gui_editor/STANDALONE.md).
- **Inspect live memory:** Windows, Python and a running supported executable. Some probes require a PID/module base; read the script's usage before running it.
- **Perform static research:** a separately installed Ghidra, the correct executable imported/analyzed, and a configured Ghidra MCP connection if using the documented `mcp__ghidra__*` commands. Those commands are not available merely by cloning this repository.
- **Run local fidelity tests:** extracted resources plus local captures described in the editor documentation. These checks are separate from the public test suite.

The canonical structural evidence lives in the local annotated Ghidra projects. Published findings.json records runtime consumer offsets, confidence and evidence references; it is not a substitute for the missing database. If that database is unavailable, do not describe a finding as freshly verified in Ghidra. Import and analyze the matching binary and record new evidence before changing structural claims.

Follow [AGENTS.md](../AGENTS.md) for repository boundaries and game safety, [methodology.md](methodology.md) for verification, and [agent-reference.md](agent-reference.md) for the original development environment. Paths and installed tool versions in older research notes describe that local environment and may need adapting on another machine.
