# Edition compatibility audit — 2026-10-12

The original calculator does not support Ubisoft History Edition out of the box.
One release can support both games using separate x86/x64 bridge artifacts and
memory backends. The x64 reader is implemented and live verified; the native
History panel candidate is being tested in the approved Ubisoft installation.

| Verified property | Repository GOG install | Ubisoft install |
| --- | --- | --- |
| Edition | Gold Edition / Venice | History Edition / Venice |
| Executable | Addon.exe | Anno1404Addon.exe |
| File version | 2.01.5010 | 4.0.4253.0 |
| PE architecture | x86 / PE32 | AMD64 / PE32+ |
| Pointer width | 4 bytes | 8 bytes |
| Preferred image base | 0x400000 | 0x140000000 |
| ASLR enabled in PE header | No | Yes, including high entropy |

These properties were read from the installed files, with architecture/base independently checked through Ghidra. Exact fingerprints and confidence are in `findings.json`. The older GOG version in this repository is Gold Edition, despite the initial description as History GOG.

## Why the current code cannot work

`annomem.py` discovers Addon.exe and dereferences pointers using 32-bit reads. `annoprod.py` calls that default reader and hardcodes GOG globals, vtables and container/field offsets. Choosing another executable name alone would still read the wrong addresses and pointer widths.

The original companion read uint32 pointers and GOG layouts. The shared companion
now selects `gold_reader.h` or `history_reader.h` at build time. History uses
64-bit pointers, its own verified layouts, and loaded module base plus RVAs.

The in-game bridge adds another port: `build.ps1` builds an x86 d3d9 proxy DLL, and `annobridge.cpp` rejects other executable names/bases and uses fixed GOG function addresses, x86 calling conventions and a binary-specific hook. An x86 DLL cannot load into the x64 History process. Keep the existing guards until a separately verified x64 bridge exists.

## Recommended distribution

Ship one product and installer/archive. A 64-bit external companion can read either game using distinct, verified Gold32 and History64 backends. Each backend must specify target pointer width, binary fingerprint, module-relative addresses and its own object/container/string/RTTI interpretation. Read target pointers explicitly; host pointer width must not dictate target data width. Enumerate the loaded module base for ASLR and use base + RVA.

Share the value-only settlement/building snapshots, aggregation, selection and presentation. The current UI transport contains fixed-width values and no game pointers, so it is a candidate for sharing after cross-architecture size/alignment and atomic-access checks. Use PID-scoped transport names if simultaneous games are to be supported; current names are global to the session. Recipes and resident-demand constants require edition comparison before being shared as verified data.

For an external-only UI, no game DLL is required. To retain the current native in-game panel, distribute separate x86 and x64 bridge DLLs selected by the installer/launcher. These are two architecture-specific artifacts in one release, not necessarily two separate product releases. History's GUI port still requires ABI, hook, widget and loading verification.

## Remaining verification

1. After Ghidra analysis completes, recover History production, population, warehouse and current-island paths and record structures/functions in Ghidra before consumer offsets.
2. Implement binary identification and fail-closed reader selection; never fall back to GOG offsets for an unknown History build.
3. Validate History in a running loaded save: island sums versus global population, settlement names/selection, building recipes/productivity/paused state and warehouse counts. Confirm semantics against the game UI and measure flows where needed.
4. Compare demand, unlocks and production rates; preserve the existing distinction between productivity estimates and actual deliveries.
5. If retaining native UI, port the bridge separately and test each architecture's hooks and transport. Run the existing GOG checks to establish no regression.

The initial audit had only GOG running. After Waldi started History, elevated read-only inspection found PID39848 with module base 0x7FF68B610000, confirming relocation from the preferred base. Ghidra cross-references are now available and the annotated program saved successfully.

## Partial live verification

The new exploratory `tools/research/history_probe.py` opens the process with READ and QUERY permissions only, verifies the executable SHA256, resolves candidate vtables using the loaded module base, and scans committed readable private memory. Its final capture is `docs/history-live-probe.json`. It does not call into or write to the game, and is not a complete calculator backend.

Ghidra production reflection confirms changed productivity/stopped offsets; the detailed consumer contract is in `findings.json`, with canonical comments in the History Ghidra program. Constructor/destructor and reflection functions identify farm and factory properties despite obfuscated RTTI names BZS/BZR. GOG's readable-class-name matching cannot carry over unchanged.

The first scan found54 farm and16 factory candidates, including12 stopped flags. A second scan found55 farm and16 factory candidates; all71 had readable building owners and reciprocal pointers from their owners. Productivity values included0,50,77,87,90,92,93 and100. All second-scan stopped flags were0. Counts/flags changed between scans without an attributed cause; each scan is non-atomic.

These observations establish usable live production-property reads and reciprocal ownership. They do not establish active-world membership, settlement/player assignment, goods identity, population or warehouse totals, recipe/balancing parity, UI correspondence or actual delivered flow. No Ubisoft installation files or game memory were modified.

## Reader and packaging progress

Further Ghidra-guided live checks recovered the world island tree, island ID,
city-name strings and population arrays. The world-only probe captured19 entries;
every tree key matched the island's own reflected ID and all11 tier rows for all8
players summed exactly to the global population arrays. Player0 city names were
Pfaffenhausen, Munich and Aschgabad. These checks supersede the earlier lack of
population/name-path verification; UI correspondence and atomic reads remain open.
Canonical structural evidence is in Ghidra, runtime offsets/confidence in
`findings.json`, and the read-only capture in `history-world-probe.json`.

The native GUI dispatcher, XML definition reader, registry and progress-bar field
changes have been identified statically in Ghidra. The compiled x64 candidate loaded in
History, hooked the dispatcher, launched its companion and acknowledged GUI-thread
requests. Native registry traversal matched all7825 initial tree entries. The
native file parser incorrectly nested widgets under the leading XML comment.
Comment-free native tree construction then created725 custom widgets and both
dialog/button text requests succeeded (8550 registry entries). A shared asset
typo named the first scroll card CARD2 instead of CARD1, preventing the whole
update batch. The asset is corrected. The panel rendered in a live save; Close
worked, and after the click-capture fix settlement selection was consumed by the
companion and the panel updated. The user confirmed the settlement buttons now
work. Full gameplay coverage remains experimental, particularly measured delivery
flow and long-session lifecycle behavior.

`tools/build-release.ps1 -Test` builds two separate staging folders in
`dist/production-calculator/`. Both editions have experimental native panels,
using separate x86 and x64 bridges and their own executable fingerprints.
Hashes cover all staged payload files. Visual Studio discovery works
with vswhere; compilation uses fresh repo-local scratch directories. Shared
transport/presentation and fingerprint tests run under both x86 and x64.
The GOG DLL/companion reject unknown binaries using the known SHA256 before
accessing edition-specific state. The staged GOG reader was tested against the
running History process and rejected it before GUI transport or offset reads.

Release CI can call `tools/build-release.ps1 -Test -RequireHistorySupport
-CreateArchives` to validate both edition packages and create separate archives.
Both packages remain marked experimental until broader gameplay and lifecycle
validation is complete. The profile gate records the completed live native-panel
test; it does not claim measured production delivery or stable long-session
support.

The History output folder includes a separate AMD64 native bridge using the same
dialog assets and shared presentation code. Its DLL remains inert without the
activation flag. After explicit user approval, the DLL, companion, dialogs and
activation flag were installed in Ubisoft History while the game was closed.
The live test confirmed the native panel opens, renders and responds to settlement
selection. Earlier statements that no Ubisoft files were modified describe the
initial read-only audit only.

The compiled History reader published four settlements with tier populations,
building counts, paused state, recipe cycle/amount and productivity-based current
and potential rates. Evidence is `history-native-reader-snapshot.json`. These
rates estimate production; warehouse availability and measured delivered flow
are not established by the snapshot. Demand constants need edition comparison.
