> **HISTORICAL — superseded.** This is the original planning conversation, kept for
> context on why the approach was chosen. Much of it is now answered or outdated:
> the per-island population problem it describes is solved, the tooling questions it
> leaves open were resolved (GhidraMCP for static analysis, a plain CLI rather than
> an MCP server for live memory), and its speculation about layout was partly wrong.
>
> For current state see `README.md`, `AGENTS.md`, `docs/findings.json`, and
> `docs/methodology.md`. Do not take offsets or conclusions from this file.

---

Handoff Summary
User's goal

Build a read-only companion tool for Anno 1404 (2009 Ubisoft city-builder, 32-bit DX9, no anti-cheat, local multiplayer only) that displays live game state in an external UI.

The specific blocker: per-island population counts. Global population is easy to find and already documented in online resources. Per-island is not findable by plain Cheat Engine scanning. The user proposed using Ghidra to understand the game's internal logic, speculating there might be a way to iterate all houses and determine which island each sits on.

The user is brainstorming an agent setup to do this research — not asking for a one-off answer.

Key technical reframe established

The user's "iterate all houses" idea was redirected. Reasoning: Anno needs per-island population constantly (supply satisfaction, tax income, ascension thresholds, the island info panel), so a 2009-era engine almost certainly caches an incrementally-maintained counter on the island object rather than recomputing by walking buildings. Counting houses is the fallback, not the primary approach.

Corollary reframe: the target is not an address but a pointer path — game → world → island list → island[i] → population block. Per-island values live inside dynamically allocated island objects, so any address Cheat Engine finds is session-garbage. Once the path is recovered, per-island population falls out for free, along with island names, coordinates, and ownership.

Recommended approach

Entry point: the known global population address. In Cheat Engine, "find out what writes to this address" → yields an instruction like mov [esi+0x2C4], eax. If that write fires repeatedly in a burst, it's an accumulation loop iterating islands — the summed operand is the per-island value. Capture the register value; that's a live pointer into a real structure.

Then Ghidra: decompile the containing function, read the loop shape, harvest offsets. Address translation: CE address minus module base, plus Ghidra image base (usually 0x400000 for 32-bit PE if ASLR didn't relocate).

Patterns flagged for the decompiler work:

Loop stride identifies the container — 4 = array of pointers, ~0x180 = inline structs, node = node->next = linked list (no index, changes how the reader walks it)
Population tiers are almost certainly contiguous — Beggars/Peasants/Citizens/Patricians/Noblemen plus Oriental tiers. Find one at [island+0x2C4], check +0x2C8, +0x2CC, +0x2D0. This was called out as the single highest-value thing to check
Vtable pointer at offset 0 means C++ object; vtable lives at a static .rdata address, so scanning memory for pointers-to-that-vtable enumerates every island object without finding the list at all
Name structs in Ghidra as you go — output improves compoundingly

Verifying the island list: count matches actual island count and increments on discovery; per-entry name/ID fields give distinct sensible values; plausible x/y floats matching the minimap; re-check against a savegame with a different island count.

MCP server design (user needs to write this; nothing public covers it)

Python on Windows, ctypes over ReadProcessMemory, VirtualQueryEx for region enumeration (filter MEM_COMMIT + MEM_PRIVATE + readable, skip PAGE_GUARD), numpy for comparison speed. 64-bit Python reading a 32-bit process is fine.

Enforce read-only at the server: open the handle with PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, never request VM_WRITE.

Critical design rule for scanning tools: the candidate set lives server-side; tools return counts, not address lists. A first int32 scan returns hundreds of thousands of hits and will destroy the context window. Only surface addresses below ~50 candidates.

Tools prioritized for this problem (more useful here than the scan engine — diff_struct alone might find the population block without opening Ghidra):

read_struct(address, layout) — labeled field dump, ~0x200 bytes as simultaneous int32/float/pointer interpretations, flagging values that look like valid heap pointers
diff_struct(address, size) — snapshot, wait, snapshot, report changed offsets. Build a house, see which field increments
walk_array(base, stride, count, field_offsets) — table across all islands at once; immediately reveals whether the container is right
find_vtable_instances(vtable_addr) — the enumeration shortcut

Plus the scanning set (attach, scan_new, scan_refine, scan_sample, read, read_many, watch) for general work.

Deliverable format: a findings.json accumulating pointer paths, types, confidence, and which verification steps each passed. The UI reads this file.

Explicit hand-offs (do not have the agent do these)
Pointer scanning → use Cheat Engine's optimized scanner. Better: "find what writes," read the base pointer from the register, get a 2-hop path instead of a fragile 7-hop one
Static game data (building production rates, etc.) → extract from the game's XML inside .rda archives via RDAExplorer; properties.xml has building/production definitions. Do not scrape this from memory. Community tooling exists (anno-mods on GitHub, FileDBReader) but which versions cover 1404 specifically was not verified
Check the modding community first — Anno 1404 has had savegame editors for ~15 years; the island structure may already be documented. An hour of searching could save a week. Unverified lead, not a fact
UI recommendation

Skip DirectX hooking — fragile for DX9, small payoff. Run a local HTTP/WebSocket server polling verified pointer paths at 2–4 Hz, serve a web UI (second monitor, tablet, or borderless always-on-top window). Frontend iterates without restarting the game.

Game-specific gotchas
Version divergence: original 2009 release, Venice addon, and 2020 History Edition have different binaries. Record which build every offset came from
Gold is signed int32 — it goes negative
HUD numbers may be interpolated/animated, so exact-value search fails; fall back to changed/unchanged scanning
Contiguous arrays (population tiers, warehouse stock) are where addresses come cheap
Open item — unresolved

The user shared four URLs and asked whether they'd be useful:

github.com/akiselev/ghidra-cli (README)
github.com/a5c-ai/babysitter — library/specializations/security-research/skills/ghidra-ida-re/SKILL.md
github.com/a5c-ai/babysitter (repo root)
github.com/mitsuhiko/agent-stuff — skills/ghidra/SKILL.md

These were never read — no web access in that session. The user was asked to either enable the Chrome extension or paste the contents. The conversation ended here; no answer was given. This is the immediate next thing to resolve.

Evaluation criteria offered in the meantime:

ghidra-cli: does it drive headless Ghidra or a running GUI instance? GUI matters — the user will be renaming structs and eyeballing the decompiler, and wants the agent's renames in the same project. Headless-only means a detached copy and loses most of the value
babysitter's ghidra-ida-re: filed under security-research/, so likely oriented to vuln hunting and malware triage. Adjacent but not the same — this is structure recovery on a cooperative, non-obfuscated binary. Workflow patterns may transfer; unpacking/anti-analysis/exploit material is dead weight. Also check whether it assumes IDA is present
mitsuhiko/agent-stuff: inferred likely most directly usable based on the author's track record for small pragmatic tooling — this is reputation-based inference, not knowledge of contents
General check for all: does the skill constrain output size? Anything that dumps whole decompiled functions or full xref lists will exhaust context on a binary this large. Good version fetches one function, summarizes, writes findings to file, moves on

The gap none of them fill: all three are static-analysis tools. The actual problem is correlation — Ghidra says _(int _)(param_1 + 0x2c4) exists; only live memory says that field is Peasants on the island currently displayed. Expected final shape: adopt whichever Ghidra skill fits + write the memory MCP server + write a custom skill describing the correlation loop between them. That third piece is unpublished because it's specific to this use case.

Notes for the receiving agent
The user is technically strong — comfortable with Cheat Engine, proposing Ghidra unprompted, thinking in terms of agent architecture. Pitch accordingly; don't re-explain basics
Unverified claims in this summary are marked inline. Anno-specific offsets, community tool names, and the MCP-server ecosystem for Ghidra were all flagged as needing verification rather than stated as fact
A minor non-technical note was raised: a read-only companion still confers an information advantage in local multiplayer, so worth mentioning to opponents. Table manners, not a technical issue
Offers made but not yet taken up: writing the MCP server skeleton (attach → scan_new → scan_refine loop), and writing the struct-diffing tools. The struct-diffing tools were recommended as the better starting point for this specific problem
Tool context differs between sessions. The previous session had no web access. If the receiving agent has search or browsing, reading those four URLs is the fastest way to unblock
