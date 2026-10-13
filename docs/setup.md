# Setup

Everything here is already installed and working. This is a repair/rebuild
reference, not a to-do list.

## Installed state

| Component | Location / version |
|---|---|
| Ghidra | `ghidra_12.1.2/` — 12.1.2, min JDK 21, **no max** |
| GhidraMCP | `ghidra-mcp/` — v7.0.0, pins Ghidra 12.1.2 |
| Ghidra project | `ghidra-project/` — `Addon.exe` imported and analyzed |
| JDK 25 (Temurin) | runs Ghidra |
| JDK 21 (Temurin) | `C:\Program Files\Eclipse Adoptium\jdk-21.0.12.101-hotspot` — **build only** |
| uv | 0.12.15, plus the bridge venv in `ghidra-mcp/.venv` |
| MCP registration | `.mcp.json` at project root |

## The JDK split

Ghidra 12.1.2 *runs* on JDK 25 (`application.java.max` is empty). But
`ghidra-mcp/build.gradle` hard-pins `JavaLanguageVersion.of(21)` for compilation, so
the Gradle build needs a real JDK 21 present. The path is pinned in
`~/.gradle/gradle.properties`:

```
org.gradle.java.installations.paths=C:\Program Files\Eclipse Adoptium\jdk-21.0.12.101-hotspot
```

Maven is **not** required despite the README saying so — `build.gradle` states Gradle
is now canonical, and the repo ships a wrapper that self-downloads Gradle 9.7.1.

## Rebuild / redeploy the plugin

```bash
cd ghidra-mcp && GHIDRA_INSTALL_DIR=../ghidra_12.1.2 ./gradlew.bat deploy
```

`deploy` stops Ghidra, builds, installs to both the Ghidra tree and the user profile,
and patches `FrontEndTool.xml` to activate the plugin — so there is no "File →
Configure → enable plugin" step.

## Server startup

The MCP HTTP server starts automatically in the plugin's `init()` (UDS and TCP both
on by default, TCP on `127.0.0.1:8089`). The README's "Tools → GhidraMCP → Start MCP
Server" step is stale for v7 — there is nothing to click.

Health check:

```bash
curl -s http://127.0.0.1:8089/mcp/instance_info
```

Returns pid, project, open programs, tcp_port.

## Connecting a new session

`.mcp.json` registers the bridge at project scope. A new Claude Code session needs a
restart and a trust prompt before `mcp__ghidra__*` tools appear. Ghidra itself must
be running with `Addon.exe` open — the bridge is a proxy, it holds no state.

## One manual setting

**Edit → Tool Options → GhidraMCP HTTP Server → Strict Naming Enforcement.**

Turning this off is what allows non-conventional names. It is currently *partially*
off in effect: function renames warn but succeed, global renames still hard-reject on
prefix/type mismatch. If renames start failing outright, check this option.

## Analyzer configuration used

Defaults, plus:

- **Windows x86 PE RTTI** — essential; 1568 classes with real names, and it creates
  the `CFoo::vftable` symbols the whole approach depends on
- **Demangler Microsoft** — turns `.?AVCIsland@@` into readable names
- **Windows x86 PE Exception Handling** — function boundaries on a 16 MB binary
- **Decompiler Parameter ID** — enabled; expensive but improves every signature
- **Aggressive Instruction Finder** — deliberately left **off**; it invents junk
  functions and pollutes xref lists

Re-running analysis is not needed and would not improve anything; the custom types
and comments are what make the project valuable now.

## Version control

This directory is now a git repo on `master` with **no commits yet**.
`.gitignore` already excludes the large third-party trees (`ghidra_12.1.2/`,
`ghidra-mcp/`, `Anno 1404 Gold Edition/`, `anno-1404-warenrechner/`) and
`ghidra-project/`, leaving `AGENTS.md`, `docs/`, `tools/` and `.mcp.json` to
commit.

Keep `ghidra-project/` out of git: it is a binary database, so back it up by
copying the directory, not by diffing. It is also the project's real knowledge
base, which makes it the one thing here that is genuinely expensive to lose —
the markdown can be rewritten from it, not the other way round.
