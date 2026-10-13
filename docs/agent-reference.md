# Agent reference

Detailed project guidance split out of `AGENTS.md` to keep new-session context small. Read this when you need command examples, repository layout, toolchain details, or gotchas beyond the compact startup rules.

---

## Two toolchains, and which is authoritative

|                   | Static structure                            | Live values                    |
| ----------------- | ------------------------------------------- | ------------------------------ |
| Tool              | Ghidra + `mcp__ghidra__*`                   | `tools/research/annomem.py`             |
| Authoritative for | struct layouts, function behaviour, offsets | what the numbers actually mean |
| Needs             | Ghidra running                              | the game running               |

**The Ghidra project is the source of truth for anything structural.** Query it rather than re-deriving or reading it out of markdown:

```text
mcp__ghidra__get_struct_layout(struct_name="CIsland")
mcp__ghidra__decompile_function(address="00756940")
mcp__ghidra__get_comment(address="00756940", type="plate")
```

The `CIsland` and `CGOPManager` structs are defined and applied, so decompiled output reads as `this->aPopulation[tier][slot]` rather than raw index arithmetic. Named functions and plate comments carry the derivations. **Do not duplicate struct layouts into markdown** — they will drift, and they already did once.

`docs/findings.json` is the exception: it carries offsets in machine-readable form because runtime tools need them without opening Ghidra. It is a consumer contract, not a second copy of the type database.

---

## Environment invariants

- Target binary is **`Addon.exe`** (Venice/addon). `Anno4.exe` is the base game and does **not** share these addresses — always record which binary an offset came from.
- **No ASLR** (`DllCharacteristics=0x0000`), image base `0x400000`. Ghidra addresses are valid verbatim against the live process. No translation, ever.
- 32-bit target; 64-bit Python reads it fine via `ReadProcessMemory`.
- RTTI is present (1568 classes with real names), so `CFoo::vftable` symbols exist — use them to find objects instead of chasing pointer chains.
- Game links **STLport**, not MSVC STL. Container node layouts differ. Don't assume MSVC `std::list`/`std::map` shapes.

---

## Repository layout

```text
AGENTS.md                  compact startup rules
docs/agent-reference.md    this detailed guide
docs/findings.json         machine-readable record: pointer paths, confidence, open questions
docs/methodology.md        static<->live correlation loop, and traps
docs/setup.md              how the toolchain was built; read if something is broken
docs/brainstorm-convo.md   historical; superseded, kept for context
docs/production.md         goods, recipes, productivity, warehouses, needs
docs/gui-system.md         in-game UI toolkit, and what a native custom menu costs
docs/gui-rendering.md      widget-to-pixel rendering, GUIRenderer, text pipeline, fonts, hook points
docs/gui-actions.md        613 <EVENTHANDLER> action verbs with ids and listener classes (live dump)
docs/ui-investigation-plan.md  prioritised open GUI topics + inbox; ADD new topics there, move answers to gui-rendering.md / findings.json / Ghidra
tools/research/annomem.py           read-only live memory inspector
tools/research/annoprod.py          read-only production overview per settlement
tools/research/annowatch.py         hardware data write-breakpoint: who writes this address?
tools/research/annogui.py           read-only dump of GUI tables: action verbs, templates, runtime XML documents
tools/research/findpop.py           one-off correlation harness (kept as a worked example)
tools/production-menu/          IN-PROCESS prototype: shows an external number in the game GUI
ghidra-project/            the Ghidra project — the real knowledge base
ghidra_12.1.2/             Ghidra install
ghidra-mcp/                GhidraMCP plugin + Python bridge
anno-1404-warenrechner/    third-party C# calculator; the original known-good offsets
"Anno 1404 Gold Edition"/  game install
```

---

## Commands

Live inspection (game must be running):

```bash
python tools/research/annoprod.py --stock                 # production per settlement + warehouse + demand
python tools/research/annoprod.py --measure 60            # measured nominal/delivered/consumed balance
python tools/research/annoprod.py --each -p 0             # one line per building
python tools/research/annomem.py islands -p 0 --nonzero   # per-island population, cross-checked vs global
python tools/research/annomem.py current --watch          # island under the cursor (what the infobar names)
python tools/research/annomem.py ptr 0x12d5a90            # deref a global
python tools/research/annomem.py struct 0x155f9c68 -n 0x200 -z   # labeled field dump
python tools/research/annomem.py array ADDR --stride 0x20 --count 11 --width 8
python tools/research/annomem.py scan 0x010ffd00          # find objects by vtable
python tools/research/annomem.py diff ADDR -n 0x200 --wait 10    # what changed while you played
python tools/research/annogui.py verbs                    # GUI action verbs / templates / structs / xmldocs
```

`islands` self-verifies: it prints the per-island sums and the global table and says whether they match. If it says DIVERGE, trust that over any assumption.

The one tool that is **not** read-only — it runs inside the game process and writes to it. Read `tools/production-menu/README.md` before using it:

```bash
powershell -ExecutionPolicy Bypass -File tools/production-menu/build.ps1
python tools/production-menu/dev/send.py --status
python tools/production-menu/dev/send.py HOUSE_FOOD_DEMAND_FULFILLMENT_BAR --sweep
```

Rebuilding the Ghidra plugin after editing it:

```bash
cd ghidra-mcp && GHIDRA_INSTALL_DIR=../ghidra_12.1.2 ./gradlew.bat deploy
```

---

## Conventions and gotchas

- **Record findings in Ghidra first**, `findings.json` second. The plate comment on `CIsland_Construct` (0x00756940) is the canonical narrative for island structure.
- GhidraMCP's naming layer is asymmetric: function renames only **warn**; global renames are **rejected** if the Hungarian prefix doesn't match the type. That rejection is useful — it pushes you to define the type properly. Struct fields get auto-prefixed (`population` → `aPopulation`), so names differ from `findings.json` by prefix.
- Some renames are rejected as "token-subset collisions" with existing symbols. Pick a genuinely distinguishing name rather than fighting it.
- Ghidra's Gradle build needs a **JDK 21 toolchain** even though Ghidra itself runs on JDK 25. Path is pinned in `~/.gradle/gradle.properties`.
- `.mcp.json` registers the Ghidra MCP server at project scope. New sessions need a restart (and a trust prompt) before `mcp__ghidra__*` tools appear.
- Don't dump whole decompiled functions into context repeatedly. `QuestEvaluateConditions` (0x007cf330) is ~600 lines; its plate comment already records what it proved.

---

## Static-analysis shortcut

This binary reflects its save-game fields **by name**, one function per class:

```c
FUN_00b0fbe0(ar, "m_Productivity", (int *)(this + 0x10), 100);
```

So a class's layout is usually already written down in the binary. Search strings for `^m_[A-Za-z]*$`, take the single xref, read the offsets. On the `CPropertyXxx` classes the reflect function is at **vtable + 0x34**. This is how the production system was recovered; see `docs/methodology.md`.

---

## Current state

Per-island population, island names, geometry, **and the production/goods/needs system** are solved and verified. The **current island** (what the infobar names: island under the cursor, else nearest the camera centre) is a static `int32` at `0x012D54F8` written only by `UpdateIslandUnderCursor` (0x005808F0); see `findings.json` → `current_island`. See `docs/production.md` for the object model (entity-component: game object → property slots → shared `CPropertyDataContainer`) and `docs/findings.json` → `open_questions` for what's next.

The 4 s consumption tick is **solved**: a two-phase economy tick (`CWarehouseAccumulateDemand` → `CWarehouseApplyAccumulatedDemand` → `CWarehouseConsumeGood`) driven from the game's **CSL script layer**, with the interval a data constant (`GetEconomyBalancingBlock()[+0x00]`, 4000 ms, proven by patching it live). That is why the per-capita consumption rates are absent from the C++ object model — they live in script. See `docs/production.md`.

---

## Live debugging

Allowed, and it works — `tools/research/annowatch.py` sets hardware data write-breakpoints and answers "who writes this address?" in one run, which the call graph could not. Two traps, both of which killed the game before being understood:

- Under **WOW64** a 32-bit process delivers debug exceptions as `STATUS_WX86_BREAKPOINT` (`0x4000001F`) / `STATUS_WX86_SINGLE_STEP` (`0x4000001E`), **not** the 64-bit `EXCEPTION_*` codes. Miss that and your own trap goes unhandled and Windows terminates the game.
- Use `Wow64Get/SetThreadContext`, and `DebugSetProcessKillOnExit(FALSE)`.

Prefer hardware breakpoints to INT3 (no memory write, nothing to restore), and build a `--probe` mode that attaches and detaches without writing before theorising about anti-debug.

Related standing caution: **nominal is not actual**. A producer at 100 % productivity with a full warehouse delivers nothing, so any rate computed from object state alone is a nameplate figure.
