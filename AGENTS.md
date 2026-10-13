# AGENTS.md

Reverse-engineering Anno 1404 to build an experimental in-game Production Menu, a standalone GUI Studio, and shared research tools. Research readers are read-only; the native Production Menu hooks and writes the game GUI.

## Startup order

1. Read this file.
2. Check `docs/findings.json` for known offsets, confidence, evidence, and open questions.
3. Read task-specific docs only as needed:
   - `docs/agent-reference.md` — detailed commands, layout, toolchain gotchas, environment invariants.
   - `docs/methodology.md` — static/live correlation workflow.
   - `docs/production.md` — goods, recipes, productivity, warehouses, needs.
   - `docs/gui-system.md`, `docs/gui-rendering.md`, `docs/gui-actions.md`, `docs/ui-investigation-plan.md` — GUI work.
   - `docs/setup.md` — broken toolchain/setup issues.

## Hard rules

- **Verify, don't infer.** Mark confidence explicitly in `docs/findings.json`; write `UNVERIFIED` in Ghidra comments when appropriate.
- **Stay inside this repo.** Do not modify outside files/folders/registry/other game installs without asking Waldi first.
- The only game install tools may touch is repo-local `Anno 1404 Gold Edition/`; install `tools/production-menu/d3d9.dll` there only while the game is closed.
- Target binary is **`Addon.exe`** (Venice/addon), no ASLR, image base `0x400000`; Ghidra addresses are live addresses verbatim.
- The target is 32-bit, RTTI is present, and the game uses **STLport** containers, not MSVC STL.

## Source of truth

- **Static structure/function behaviour:** Ghidra project via `mcp__ghidra__*`. Query it; do not re-derive or duplicate struct layouts in markdown.
- **Live values/meaning:** `tools/research/annomem.py` and related runtime tools.
- **Runtime consumer contract:** `docs/findings.json` may contain offsets because tools need them without opening Ghidra.
- Record structural findings in Ghidra first, then `docs/findings.json`.

Useful Ghidra examples:

```text
mcp__ghidra__get_struct_layout(struct_name="CIsland")
mcp__ghidra__decompile_function(address="00756940")
mcp__ghidra__get_comment(address="00756940", type="plate")
```

## High-value commands

```bash
python tools/research/annomem.py islands -p 0 --nonzero   # self-verifying island population
python tools/research/annomem.py current --watch          # current island under cursor/camera
python tools/research/annoprod.py --stock                 # production + warehouse + demand
python tools/research/annoprod.py --measure 60            # measured actual flow
python tools/research/annogui.py verbs                    # GUI verbs/templates/runtime XML
```

If `annomem.py islands` says `DIVERGE`, trust that over assumptions. For more commands, see `docs/agent-reference.md`.

## Current known state

Solved and verified: per-island population, island names, geometry, production/goods/needs, and the 4 s CSL-driven consumption tick. Current island is static `int32` at `0x012D54F8`, written by `UpdateIslandUnderCursor` (`0x005808F0`). See `docs/findings.json` and `docs/production.md`.

## Standing cautions

- **Sandbox process visibility:** sandboxed commands can hide an already-running `Addon.exe`, including from `Get-Process` and Toolhelp enumeration in `annomem.py`. A sandboxed "no running process" result is inconclusive. Retry the read-only inspection with `sandbox_permissions="require_escalated"` before claiming the game is closed or troubleshooting offsets. Confirmed 2026-10-09: the same commands found PID 29188 and read settlements correctly outside the sandbox. This does not authorize writes outside this repo.
- `tools/production-menu/` is in-process and writes to the game; read `tools/production-menu/README.md` before using it.
- Prefer Ghidra plate comments over dumping huge decompilations into context.
- Save-game field reflection often reveals class layouts: search `m_*` strings, inspect the single xref; on `CPropertyXxx`, reflect is usually vtable `+0x34`.
- Live debugging with `tools/research/annowatch.py` is allowed, but under WOW64 use `STATUS_WX86_BREAKPOINT`/`STATUS_WX86_SINGLE_STEP`, `Wow64Get/SetThreadContext`, and `DebugSetProcessKillOnExit(FALSE)`.
- **Nominal is not actual:** producers at 100% with a full warehouse deliver nothing; measure actual flow when it matters.
