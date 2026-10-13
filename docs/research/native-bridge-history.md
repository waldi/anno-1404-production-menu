# annobridge

## Edition build packaging (2026-10-12)

From the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-release.ps1 -Test
```

This creates `dist/production-calculator/gog/` and
`dist/production-calculator/history-edition/`, with per-folder `package.json`
manifests, hashes, architecture, known game fingerprint and support status.
GOG contains the x86 DLL, companion and two required dialog files. History is
currently an **engineering preview** with a working x64 read-only production
reader and probe. `-ExperimentalHistoryNative` also stages the x64 native panel
candidate under `experimental-native/`; its activation flag is deliberately an
example file. Whole-panel behavior is still under live validation, so the candidate
must not be advertised as supported. The build script does not install anything.

The build discovers Visual Studio through vswhere (including Windows CI runners),
uses isolated fresh compiler output, validates GOG PE architecture, and replaces
only folders carrying its own package marker. `-Test` runs the existing transport
and presentation tests plus executable-identity rejection tests under both x86
and x64. History's reader additionally passed live population-sum and production
snapshot checks; native panel behavior requires separate live validation.
The Gold bridge and companion now reject unknown executable fingerprints before
using game offsets; recompiling their sources as a History build is also rejected.

For future release CI, use:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-release.ps1 -Test -RequireHistorySupport -CreateArchives
```

That command currently fails deliberately before staging or archiving: the
History native port has not completed live validation. Once that port is
registered in the build, it will produce `gog.zip` and `history-edition.zip`.
Changing the manifest flag alone cannot create a working History build.
The low-level GOG `build.ps1 -Test` command keeps its existing output locations;
`-OutputDirectory` allows repository-local isolated builds.

Detailed verification and remaining work: `docs/edition-compatibility.md`.

## Production panel update architecture (2026-10-07)

The production panel is centered and capped at 1280x1024 GUI units, shrinking
to the viewport minus 10-unit margins on smaller screens. Its enclosing frame
fills the viewport, with a transparent-to-input fullscreenblurbackground.bkg
LABEL behind the capped beige panel. Both layers hide with the dialog. The
Close button declares BUTTON_CHECKESCAPE=1, using native Escape handling;
real Escape input was verified live to close the dialog without opening Pause.

The resource grid now lives in RESOURCE_SCROLL, a native LABEL viewport with
four checked icon filter buttons above it: All (Resources icon), Consumer goods
(Fish), Construction materials (Wood), and Other (Iron). The selected filter is
stored in AnnoRECompanion.ini beside the companion executable and survives
menu openings, settlement changes and companion restarts. Consumer goods match
the 21 final goods; construction is Wood/Tools/Stone/Glass/Mosaic; other goods
are the remaining rows. Reserved click rows 20..23 use the existing bridge's
button event transport, leaving settlement rows 1..19 unchanged.
SCROLLBAR_VERT=2 and SCROLLBAR_VERT_TEMPLATE. It contains 68 slots in two
columns; unused slots are hidden and collapsed to 1x1 rectangles. The headers
and settlement selector stay outside the viewport. Previous/Next are hidden.
The native scrollbar appears only when content overflows; wheel, arrows and
thumb dragging use the game's existing scrolling behavior. Large grids drain
in acknowledged batches of at most 192 widget updates. A whole grid refresh
can span several frames, while each individual batch remains preflighted and
applied together. Selection buttons sort before resource widgets in delivery.

Ctrl+Q toggles the Production menu while the game is foreground. The companion
detects one press per hold and dispatches the existing Production button click,
preserving its selection/open/close behavior. No global shortcut is registered.

Each resource row shows combined Current/Potential meters for demand and supply.
Resident demand is identical in both estimates. Factory input demand uses live
productivity for Current and 100% capacity for Potential; paused factories count
as zero in both. Meter tracks use #b29d72, current demand #71a4b5, potential
demand #445f38, current supply #648d56, and potential supply #486339.
The darker
green Current fill overlays the lighter Potential bar, with both values in one
caption. Current sums each unpaused producer's 100% capacity multiplied by its own live
productivity percentage. All three bars share a scale including boosted output
above 100%. Current is a productivity-based rate, not measured deliveries.

Demand update (2026-10-09): the native panel now reads every resident tier and
estimates demand for all 21 resident goods using the repository community
calculator's divisor table. Canonical production rates were read from loaded
game recipes (`docs/live-production-rates.json`). Unlock thresholds use the
player-wide population, independently of selected settlements. Factory inputs
are included at current productivity, with paused factories contributing zero.
Needed goods appear even without a local producer. Potential output is the
summed capacity of unpaused producers
at 100% productivity, independent of their current productivity; factory input
demand still uses current productivity. Zero means no resident or
factory demand; construction, trade and ship spending are excluded. These are
estimates, not measured consumption: need bans, item/scenario consumption
modifiers and retained unlocks after population loss are not modelled. Only
peasant fish/cider consumption has been empirically verified. The older
peasant-only panel descriptions below are historical.

The native companion separates read-only sampling, selection, and presentation:

* A background worker discovers buildings **once for all settlements**, publishes
  a value-only snapshot, then waits one second before sampling again. No game
  pointers survive in the published snapshot. World/player/island-map changes
  detected during a scan invalidate its results. This is a best-effort live read,
  not an atomic snapshot of the game's simulation.
* The foreground loop checks clicks every 10 ms and formats the selected
  settlement(s) from the latest completed snapshot. Switching settlements does
  not scan memory. New building/population data arrives on the next completed
  sample; sampling latency does not block selection.
* `ui_protocol.h` defines the independent `AnnoREUiV4` mapping. Up to 192 widget
  updates are submitted together, preflighted, and applied on the game GUI thread
  in one dispatch. Unchanged text/visibility is suppressed. New desired state
  coalesces while a batch is outstanding; its payload remains immutable until
  acknowledged. `AnnoREUiV4Done` wakes the companion on acknowledgement.
* A 64-entry click ring preserves rapid Ctrl-clicks. Each click carries the
  displayed island ID and player, so list reordering cannot redirect a click.
  Overflow is logged and retains the newest 64 clicks. The DLL updates the
  displayed row mapping only after a successful batch.
* GUI health is checked every two seconds without rewriting widgets. Path maps
  are rebuilt only if a panel/trigger lookup fails. Missing dialog and trigger
  are installed independently after a probe; failed updates invalidate the
  applied cache and retry after a delay. Widget pointers are resolved anew for
  every batch and retained only during that GUI dispatch.

Settlement buttons now use the template's native **Checked** artwork. `UI_CHECKED`
requests a selected-state update and `UI_CHECKED_ON` carries its boolean value.
The companion's selection model drives both multi- and single-selection; automatic
`TOGGLEBUTTON` behavior stays disabled so clicking an already selected settlement
cannot independently uncheck it. Ghidra confirms the checked byte and native visual
refresh routine; the custom panel's checked artwork was verified live on
2026-10-07. Row captions now show the
settlement name without the former `[ ]`/`[*]` prefix.

The dialog fills its full-screen `SYSTEMMESSAGES` parent with a 10-unit outer
margin and 30-unit inner padding. Layout coordinates use the game's GUI space;
`dialog_layout.h` computes rectangles for the viewport and visible settlement
count. Geometry travels in the same changed-only batch via `UI_RECT`, and is
applied using the native extent setter so alignment and hit testing refresh.
The frame's four `ALIGNMENT_*` edge tags are explicitly 1, disabling the default
screen-center offsets. Live bounds at 1440x1080 are (10,10)-(1430,1070).
A Close button sits inside the frame. Buttons use the skin's full 48px height
and explicitly centered captions;
settlements wrap into columns in short viewports rather than shrinking the skin.
Visibility transitions share the input
queue: row 0 means open (with the hovered island/player captured at that instant),
row -1 means close. Opening clears the previous manual/multiple selection and
pins the captured owned settlement; water or another player's island starts with
an empty selection. Repeated Ctrl-clicks still work within that opening.

After bridge fixes, build/test and run `python tools/production-menu/dev/restart.py` to
install the matching files and reload the newest autosave (Waldi's standing
instruction, 2026-10-07).

The original 4096-byte `AnnoREBridge` protocol is unchanged for existing scripts.
The new transport checks magic, version, byte size, and owning game PID; install
the DLL and companion together. Never overwrite a submitted command after a
timeout: the game may still execute it later. Legacy commands are now reserved
for installation and manual diagnostics, not periodic row updates.

Build and run the deterministic tests (no game required):

```powershell
powershell -ExecutionPolicy Bypass -File tools\production-menu\build.ps1 -Test
```

Tests cover aggregation, changed-only updates, coalescing, failure replay,
metadata-only batches, queue ordering/overflow/wrap, and capacity rejection.
Runtime log records include `scan_ms`, batch `ack_ms`, and `click_to_ack_ms`.
Basic settlement switching was confirmed working by Waldi on 2026-10-07.
Measured latency and save-load recovery still require a live test. Expected
settlement response is a 10 ms polling interval plus GUI dispatch/frame timing,
possibly one outstanding batch; it no longer includes per-widget sleeps or a
building scan.

Proof of concept: **an externally supplied number, displayed by Anno 1404's own
GUI.** An external process writes a value into shared memory; a DLL inside the
game writes it into a native widget.

```
send.py  ──write──▶  shared section "AnnoREBridge"  ──read──▶  annobridge (d3d9.dll)
                                                                     │
                                                          hooks the GUI dispatcher
                                                                     ▼
                                                        widget matched by ACCESSID
```

## This breaks the read-only rule, deliberately

`AGENTS.md` rule one is *never write to the game process*, and `annomem.py`
enforces it structurally by not requesting `PROCESS_VM_WRITE`. This tool runs
**inside** the game and writes to its memory. That guarantee does not apply here.

What it does write is deliberately minimal: two dwords on one widget — its value
and its range — and nothing else. It refuses to patch anything if the host is not
`Addon.exe`, if the image base is not `0x400000`, or if the dispatcher's prologue
bytes do not match what was reverse-engineered. `annomem.py` is untouched and
stays read-only.

Test in single player first. In lockstep multiplayer, any write to game state
risks a desync; a widget's display value should be outside the simulation, but
that is an assumption, not a verified fact.

## Build

Needs the MSVC **x86** toolset (the game is 32-bit). VS 2022 Community is enough.

```powershell
powershell -ExecutionPolicy Bypass -File tools\production-menu\build.ps1
```

Produces `tools/production-menu/d3d9.dll` and `tools/production-menu/AnnoRECompanion.exe`.
The companion also needs `examples/production_dialog_v5.dlg` and
`examples/production_button.dlg` beside the two binaries.
The companion is a native external process. It owns panel startup and reads the
production state through a read-only `OpenProcess` handle; the process boundary is
retained for safety.

**Close the game before copying.** A running Anno holds `d3d9.dll` open and the
copy will fail with a sharing violation. The shared section also changed size
between v1 and v2, so an old running instance must be restarted anyway.

## Run

1. Copy `d3d9.dll`, `AnnoRECompanion.exe`, and both dialog files
   (`production_dialog_v5.dlg` and `production_button.dlg`) next to `Addon.exe` (the
   game's install root, *not* `bin/`). The game does
   `LoadLibrary("d3d9.dll")` and the exe's directory is searched first, so it loads
   ours; all exports are forwarded to the real system DLL. The DLL starts the
   companion automatically after initialization.

   **The game must be running in DirectX 9 mode.** In DX10 mode `d3d9.dll` is
   never loaded and nothing happens.

2. Start the game and load a save.

3. The companion automatically installs the production panel once the in-game GUI
   is ready, and reinstalls it if save loading replaces the runtime GUI tree. Its
   log entries are written to `%TEMP%\\annobridge.log`. The companion also updates
   the production rows automatically; `prod_ui.py` is no longer required for the
   normal production panel.

4. Check the DLL came up:

   ```bash
   python tools/production-menu/dev/send.py --status
   ```

   `magic ok` means the DLL is loaded and the shared block exists. The log is at
   `%TEMP%\annobridge.log`.

   If `magic` is zero the DLL is not in. Note the shared section lives only as
   long as someone holds it open: with the game running the DLL holds it, so
   values persist between `send.py` runs; without the game, every invocation
   gets a fresh, empty one.

4. Pick a widget and drive it. A residence's demand bars are a good first target
   — click any house and its object menu shows them:

   ```bash
   python tools/production-menu/dev/send.py HOUSE_FOOD_DEMAND_FULFILLMENT_BAR --sweep
   ```

   The food-demand bar should ramp 0→100 and repeat. `hits` in `--status` counts
   the updates the DLL actually applied; if it stays at 0, the element was never
   on screen or the ACCESSID is wrong.

   `--clock` sends seconds-past-the-minute instead, which is the "show a
   timestamp" version of the same test.

## Building our own panel (v2)

The DLL can ask the game to build a native widget for us, using its own runtime
factory `GuiRuntime_CreateElement` (`0x00DB72C0`). No `.dlg`, no `/compilegui`,
no RDA repacking.

```bash
# be in-game (not in a menu) so INFOBAR exists, then:
python tools/production-menu/dev/send.py --create --rect 620 150 220 24
python tools/production-menu/dev/send.py ANNORE_BAR --sweep
```

`--create` asks for a `PROGRESSBAR` named `ANNORE_BAR` parented under `INFOBAR`
(the always-visible top bar). The DLL performs the call on the next GUI event, so
it happens on the right thread. Status comes back through `--status`:

| status | meaning |
|---|---|
| `done` | the factory returned 0; the widget exists |
| `parent not found` | the parent name did not resolve (factory returned `0xC`) |
| `bad type` | unknown element type (returned `3`) |
| `failed` | something else, or an exception — check the log |

Other element types: `--type 1` LABEL, `2` BUTTON, `3` SLIDER, `6` BOXLAYOUT,
`13` ALPHAPROGRESSBAR. Parent can be any element name, e.g. `--parent ACTION_BAR`.

### Geometry and skin: the two things a fresh widget lacks

A runtime-created widget comes up **zero-sized and unskinned**, so it renders
nothing. Both are fixable live:

```bash
python tools/production-menu/dev/send.py --inspect HOUSE_FOOD_DEMAND_FULFILLMENT_BAR --at 0x14C --count 1
python tools/production-menu/dev/send.py ANNORE_BAR --skin-index 91
python tools/production-menu/dev/send.py ANNORE_BAR --rect 620 150 820 174 --sweep
```

`--rect` writes a **`CRect` {left, top, right, bottom}** to element`+0x2A4` every
frame. It is *not* x/y/width/height: `620 150 820 174` is a 200x24 bar at
(620,150). Confirmed against `HOUSE_FOOD_DEMAND_FULFILLMENT_BAR`, which reads
(54, 10, 133, 48) while visibly on screen.

`--skin-index N` applies registry template N via `GuiTemplate_Apply`
(`0x00DB6740`). Read the index off a widget that already looks right - its
`+0x14C`. On this build, 91 is the demand-bar template. Applying a template does
**not** set the extent; do both.

`--no-rect` stops forcing geometry, so the widget keeps whatever it has.

### Matching

A runtime-created element gets a PATHID but no ACCESSID, so the DLL matches the
target name against **either** — the same command drives both our widgets and the
game's existing ones.

## v6: real art and real text (2026-09-18)

Three more game calls, all on an element named by ACCESSID, PATHID or **full path**
(`MAIN_MENU/ANNORE_BAR`), which reaches elements that are hidden or zero-sized and
therefore never dispatch events:

```bash
python tools/production-menu/dev/send.py --create --type 1 --name ANNORE_LABEL --parent MAIN_MENU
python tools/production-menu/dev/send.py MAIN_MENU/ANNORE_LABEL --rect 480 340 760 372
python tools/production-menu/dev/send.py --background MAIN_MENU/ANNORE_LABEL @MAIN_MENU_PROFILE_BUTTON
python tools/production-menu/dev/send.py --text MAIN_MENU/ANNORE_LABEL "Hello from annobridge"
python tools/production-menu/dev/send.py --font MAIN_MENU/ANNORE_LABEL "Mainmenu Subheader"
python tools/production-menu/dev/send.py --align MAIN_MENU/ANNORE_LABEL 1,4
```

| flag | game call | notes |
|---|---|---|
| `--text NAME TEXT` | `GuiWndLabel_SetTextW` (LABEL) / `TextSetting_SetTextW` (BUTTON `+0xA70`, EDITFIELD `+0x86C`) | arbitrary text, no GUID. **Confirmed on screen.** |
| `--background NAME BG` | vtable `+0xBC` `GuiWnd_SetBackground` | `BG` is a `.bkg` path, or `@OTHER` to copy OTHER's `+0x199`. **Confirmed**: gives a runtime widget the real menu-button art. |
| `--oncreate NAME` | vtable `+0x58` | resolves BACKGROUND from `+0x199` and fires `OnCreate`. Prefer `--background`. |
| `--font NAME FONT` | writes the font name string at TextData`+0xC8` | font names as in the `.dlg` `<FONT>` tags. **Simpler and confirmed:** put `[FONT Tooltip]` at the start of the text; `[CENTER]`, `[COLOR AARRGGBB]`, `[ICONGUID n]` work the same way. |
| `--align NAME H,V` | `TextSetting_SetHAlign/SetVAlign` | H 0/1/2, V 0/4/8 |
| `--visible NAME 0|1` | vtable `+0xAC` | |
| `--children NAME` | walks the child list at `+0x80` | PATHID, ACCESSID, type, visible flag |

`--rect` now belongs to the target named on the same command line; before v6.1 it followed
whatever the current target was and once dragged the label onto the button.

### Two crashes worth knowing about

Both happened on 2026-09-18 and both are fixed; the minidumps in `%LOCALAPPDATA%\CrashDumps`
were what found them.

1. **Re-entrancy.** The game dispatches GUI events from inside `GuiRuntime_CreateElement`
   and from OnCreate. The hook saw the still-pending request and ran it again: ~200
   copies of `ANNORE_BAR`, then OnCreate recursing until the stack overflowed. The
   request is now cleared before the call and a busy flag makes nested events observe
   only. Earlier "it created the bar" successes probably created many.
2. **`GuiRegistry_FindElementByName` takes two stack arguments** (`RET 8`). Called with
   one, the game died inside this DLL's epilogue with ESP = 0.

### The 64-byte trap

Until v6.1 the layout name travelled in a 64-byte field while every `.bkg` path is
86 to 90 characters. The game got `...housem`, saw no `.bkg`, and drew its grey
placeholder. `send.py` now warns on truncation and the field is 260 bytes. If a
background ever shows as a grey box with a black border, check `+0x199` first.

### In-game results (2026-09-18, second session)

With the 260-byte field the progress bar finally shows the game's own bar art, and the
rest of the original plan went through in one sitting:

```
python tools/production-menu/dev/send.py --create --type 6 --name ANNORE_PANEL --parent SYSTEMMESSAGES
python tools/production-menu/dev/send.py SYSTEMMESSAGES/ANNORE_PANEL --rect 400 300 800 480
python tools/production-menu/dev/send.py --background SYSTEMMESSAGES/ANNORE_PANEL addondata/config/gui/backgrounds/final/general_frames/bigmessagebackgroundgold.bkg
python tools/production-menu/dev/send.py --create --type 1 --name ANNORE_P_LBL --parent SYSTEMMESSAGES/ANNORE_PANEL
python tools/production-menu/dev/send.py SYSTEMMESSAGES/ANNORE_PANEL/ANNORE_P_LBL --rect 20 20 380 60
python tools/production-menu/dev/send.py --text SYSTEMMESSAGES/ANNORE_PANEL/ANNORE_P_LBL "[FONT Tooltip]Panel child: [COLOR FFFFAA00]42[DEFAULTCOLOR] ok"
python tools/production-menu/dev/send.py --create --type 9 --name ANNORE_BAR --parent INFOBAR
python tools/production-menu/dev/send.py INFOBAR/ANNORE_BAR --rect 620 150 820 174 --value 60
python tools/production-menu/dev/send.py --background INFOBAR/ANNORE_BAR addondata/config/gui/backgrounds/final/general_progressbars/tiny_progressbar.bkg
```

That gives a gold message frame over the map with a white-on-shadow label inside it, and a
framed silver progress bar under the HUD. Rules learned while getting there:

- **Extents are relative to the parent, and children are clipped to the parent.**
  `INFOBAR` is only 195 px tall; anything below that vanishes. `SYSTEMMESSAGES` is a
  live 1280x1024 frame in-game and makes a good root for free-floating panels.
- **`--parent` takes a full path.** The factory resolves the parent through the same
  full-path map as everything else, so runtime panels can hold runtime children.
- **`--rect` follows one element at a time** (`rectTarget`). Earlier elements keep the
  extent they were last given.
- **`--font NAME` works** (writes `TextData+0xC8`), as does `--align H,V` with
  H = 0 left / 1 right / 2 centre and V = 0 top / 4 / 8.
- **The house-menu bar layouts have no frame.** `PB_Background` is an empty icon; the
  frame you see in the game is the menu behind it. `tiny_progressbar.bkg` carries its
  own `PB_Overlay` frame if you want a self-contained bar.
- **Screenshots without alt-tabbing:** `tools/production-menu/dev/capture.ps1` uses
  `PrintWindow(PW_RENDERFULLCONTENT)` and works while the game is covered by other
  windows. It must be DPI-aware or you get a top-left crop of the window.
## v7: clicks, dialogs, and a hands-free restart (2026-09-18)

The hook now reads the event *name* too, which turns the dispatcher hook into a click
handler. Three new commands, one script:

| command | what it does |
|---|---|
| `--toggle BUTTON TARGET` | when BUTTON receives `OnLeftClick`, flip TARGET's visibility through the game's own SetVisible (vtable `+0xAC`). TARGET may be a full path. `--toggle-off` clears it. |
| `--click NAME [--event EV]` | dispatch `OnLeftClick` (or EV) to NAME through `GuiElement_DispatchNamedEvent`, exactly as the game's own `FUN_004555E0` does: name string, 16-byte arg block `{0, 0, element, result}`, default result 1. Runs the element's handlers, e.g. the loading screen's `START_MISSION` ("Set sail!"). |
| `--load SWW` | post a savegame to `MainControl_LoadSavegameFile` (`0x004B0320`, `__stdcall`, RET 8 verified from the bytes). Bare names resolve in `Documents/ANNO 1404 Venice/Savegames`. Works from the main menu. |
| `--trace 1/0` | log every non-render GUI event with its element to `%TEMP%\annobridge.log`. Invaluable: it showed that our clicks were arriving on an element with a different name. |

`tools/production-menu/dev/restart.py` chains it all: close the game, install the DLL, start it, wait
for the intro movie, `--load`, wait for `LOADING_SCREEN/START_MISSION`, `--click` it, wait
for `INFOBAR`. No mouse or keyboard involved.

Verified live: a runtime BUTTON under `SYSTEMMESSAGES` with the diplomacy button's copied
`.bkg` receives real mouse clicks and shows/hides a runtime BOXLAYOUT dialog (gold message
frame with a label) on each click.

### Things that cost an hour each

- **`GuiTemplate_Apply` renames the element.** Applying a template copies the template's
  PATHID and ACCESSID into the target. Our skinned `ANNORE_BTN` was receiving every click
  as `INGAME_MENUBUTTONS_TEMPLATE`, so the name match never fired. `--children` listings
  showing `HOUSE_DEMAND_FULFILMENT_TEMPLATE` under INFOBAR were the renamed `ANNORE_BAR`.
  The DLL now saves and restores both names around the call (build after 18:36).
- **Short STLport strings live inline.** The event name is an `stlp_string*`; names of
  15 characters or fewer are stored *at the object* (`+0x00..+0x0F`), with `+0x14` equal
  to `obj+0x10` as the marker. Reading `+0` as a pointer gives garbage for `OnLeftClick`.
- **The intro movie.** The main menu tree is live while the intro plays, so "MAIN_MENU
  answers" is not a readiness signal. Loading during the movie leaves the game with no
  cursor over the world and no map dragging. Wait ~120 s after launch (`--intro-wait`).
- **Runtime elements die with their screen.** Going back to the main menu destroys the
  in-game screens and everything we hung under them.

- **Pokes stay armed across target changes.** `--poke` slots persist and are applied to
  whatever the *current* target is. Setting `--poke 0x2B8 12` on a box and then running
  `ROW1 --rect ...` for each row gave every row a hidden scrollbar (2026-09-20). Run
  `--clear-pokes` before switching targets.
- **`--skin-index` calls the raw `GuiTemplate_Apply`.** The game's own wrapper
  `GuiWnd_ApplyTemplateByIndex` (`0x00D6DE00`) does the name save/restore and rebuilds the
  background; switching to it would remove two workarounds. Until then follow `--skin-index`
  with `--oncreate` if the element should show the template's art.

### How the hit test works (so you know where a button can live)

`GuiWnd_HandleInputEvent` (`0x00D61D80`, vtable `+0xEC`) walks the tree from the root,
children first to last, and the first element that contains the point and returns non-zero
swallows the event. Containment uses the absolute rect at `+0x8C` (the game fills it from
the relative extent at `+0x2A4`). Elements with flag `0x20000000` or alpha < 0.001 are
input-transparent (`GuiWnd_IsInputTransparent`); `+0xA0` is the hover state the input code
sets itself. Nothing has to be registered: a fresh runtime button under `SYSTEMMESSAGES`
or `INFOBAR` is clickable as soon as it is visible.
## v8: dialogs from XML (2026-09-26)

The game keeps its `.dlg`/`.tpl` grammar alive at runtime (`docs/gui-rendering.md`, "The XML
layer at runtime"). v8 uses that instead of pokes:

```bash
python tools/production-menu/dev/send.py --xml tools/production-menu/examples/scrollbox.dlg            # under SYSTEMMESSAGES
python tools/production-menu/dev/send.py --xml my_panel.dlg --parent INFOBAR
python tools/production-menu/dev/send.py --rebuild-paths
```

| flag | game calls | notes |
|---|---|---|
| `--xml FILE` | `XmlNode_Construct`/`XmlNode_InitWithName`/`XmlNode_AppendChild` per tag, `XmlDoc_ConstructEmpty` on the GUI's XML context (`g_pGuiManager+0x42C`), then `GuiElement_InstantiateXmlSubtree(parent, doc, -1)` (`0x00DBA940`) and `GuiRegistry_RebuildPathMap` | The DLL parses the file itself (tags, text, comments; no attributes exist in the format) and builds the game's own 0x5C-byte `CXMLExtNode`s. Every tag `ReadDefinitionNode` knows is honoured: `EXTENT`, `BACKGROUND`, `TEXTSETTING`, `TEMPLATE_REF_ID`, `SCROLLBAR_VERT`, `EVENTHANDLER`, nested children. `AddChild` fires OnCreate, so art appears without `--oncreate`. Result = number of new direct children of the parent. |
| `--rebuild-paths` | `GuiRegistry_RebuildPathMap` | Makes game-created children (scrollbars) reachable by full path. |
| `--skin-index N` | now `GuiWnd_ApplyTemplateByIndex` (`0x00D6DE00`) | The game's wrapper saves/restores PATHID/ACCESSID and rebuilds the background; the v6 name save/restore and the follow-up `--oncreate` are gone. |

v8.1 also calls `GuiWnd_RegisterInRegistry` (`0x00D6DB10`) on every new element: the XML
constructors skip it, and without it the elements render but full-path lookups and
ACCESSID-based actions (`SETVISIBLE ...`) cannot find them. Verified 2026-09-26 with the example:
the declared `SETVISIBLE ANNORE_X_ROW1; toggle` handler hides and shows the row on `--click`.

Rules: ACCESSIDs in the fragment must be unique for the game run (a clash pops the game's
"Duplicate access ID" message box); PATHIDs only need to be unique among siblings. The
parent may be any element, full paths allowed. Nodes are allocated by the DLL and never
freed (a few KB per call). The fragment's root element may repeat: top-level siblings all
become children of the parent.

## v8.3 / v8.4: modal frames and tooltips (2026-09-26)

- `--xml` now wraps `<FRAME_MODAL>1` / `<FRAME_POPUP>1` frames in the game's
  `CGUIWndModalBackground` after the registration walk (as the .cdf loader does) and re-applies
  the frame's visibility through `GuiWndFrame_SetVisible`, so the wrapper follows the frame.
  Wrapping *during* the walk hung the game once; the wrap is deferred for that reason.
- `--tooltip NAME TEXT` (v8.4): once per tooltip show of NAME, re-runs `ToolTipManager_SetText`
  with TEXT. `--tooltip-mode 2` also re-runs `ToolTipManager_LayoutStandard`; that mode crashed the
  game once (no dump) and is off by default. **Mode 1 also preceded a game exit in the
  2026-09-26 16:16 live test**: the log confirms SetText returned before the exit.
  Both modes are unsafe pending diagnosis; exact cause is UNVERIFIED. See the plan's follow-up.
- Shared block grew to 4096 bytes (`SIZE` in send.py); old DLL + new send.py cannot map it.

## v8.2: real mouse tests and the SetVisible fix (2026-09-26)

`tools/production-menu/dev/mouse.py click|wheel|drag|move X Y ...` drives the real mouse in GUI client
coordinates. The game reads the OS cursor only while it is the foreground window, so the
script taps ALT and calls `SetForegroundWindow` first; without that every event lands on the
in-game root. With it, a runtime scrollbar behaves like the game's own: wheel over the rows,
arrow buttons, track paging and knob drag all work (topic 19).

`--visible` and `--toggle` used to pass `visible` as a `char`; `GuiWnd_SetVisible` stores the
whole dword at `+0x9C` and everything compares it with `== 1`, so garbage upper bytes made the
element vanish while still being hit-tested. v8.2 passes an int. If an element ever reads a
strange `+0x9C`, `--poke 0x9C 1` repairs it.

## Production dialog helper

`prod_ui.py` is the first real companion UI: it installs a small `Production` button and a native dialog from `examples/production_dialog.dlg`, then updates label text from read-only live memory.

```bash
python tools/production-menu/dev/prod_ui.py --install --show
python tools/production-menu/dev/prod_ui.py --watch
```

The dialog follows the current/hovered island (`0x012D54F8`, same source as the infobar). Rows are shown as `Good  Need: N  Produced: M`, where `Produced` is production-building count (`active/total active` if some are paused). `Need` is an equivalent building count only for verified consumption rates currently in the repo (`SaltFish`, `Cider` for peasants); unknown goods intentionally show `?`.

## Choosing a target widget

Any `PROGRESSBAR` or `SLIDER` with an `ACCESSID`. To list candidates from the
extracted definitions:

```bash
grep -B20 '</PROGRESSBAR>' data/addon/data1.rda/addondata/config/gui/menudefinitions/*.dlg | grep ACCESSID
```

Known-good ones in `objectmenu_house.dlg`:
`HOUSE_FOOD_DEMAND_FULFILLMENT_BAR`, `HOUSE_DRINK_DEMAND_FULFILLMENT_BAR`,
`HOUSE_FAITH_DEMAND_FULFILLMENT_BAR`, … and in `objectmenu_manufactory.dlg`:
`MANUFACTORY_PRODUCT_AMOUNT_BAR`.

Hijacking an existing widget is deliberate: it proves the whole pipeline without
depending on `Addon.exe /compilegui`, which has **not** been verified yet. Once a
custom `.dlg` compiles, point `--sweep` at your own `ACCESSID` and nothing else
changes.

## How it works

| Piece | Address | Derivation |
|---|---|---|
| `GuiElement_DispatchNamedEvent` | `0x00D6C0D0` | every GUI event passes through, element in ECX |
| `ACCESSID` | element `+0x109` | `GuiElement_ReadDefinitionNode` (`0x00D6E1A0`) |
| element type enum | element `+0x0C4` | `GuiDefinition_BeginElementNodeForType` (`0x00DB8B20`) |
| progress bar value / max | `+0x6F0` / `+0x6F4` | `GuiWndProgressBar_ReadDefinitionNode` (`0x00DB9A40`) |
| slider value / resolution | `+0x6F8` / `+0x6FC` | `GuiWndSlider_ReadDefinitionNode` (`0x00DA2950`) |

The hook is a 5-byte relative jump over the dispatcher's first 8 bytes, with a
trampoline holding the stolen bytes. Those 8 bytes are position-independent
(`PUSH -1; MOV EAX, FS:[0]`), so no instruction rewriting is needed.

The event name is ignored on purpose. `OnRender` fires every frame for a visible
element, so matching on `ACCESSID` alone is enough — and it avoids decoding an
STLport `std::string` inside the hook.

Full background in `docs/gui-system.md`.

## Bounds, and a crash worth knowing about

Every read and write is clamped to the element’s real allocation size, taken from
`GuiRuntime_CreateElement`’s own malloc table:

| Type | Size | | Type | Size |
|---|---|---|---|---|
| PROGRESSBAR 9 | `0x6FC` | | LABEL 1 | `0x924` |
| BOXLAYOUT 6 | `0x704` | | RENDERTARGET 12 | `0x920` |
| ALPHAPROGRESSBAR 13 | `0x714` | | VIDEOPANEL 15 | `0x938` |
| CARDLAYOUT 10 | `0x738` | | BUTTON 2 | `0xAAC` |
| SLIDER 3 | `0x7BC` | | COMBOBOX 7 | `0xBF0` |
| LISTBOX 8 | `0x84C` | | EDITFIELD 5 | `0x8A8` |

This matters: v4 bounded pokes at `0xF00`, which is **larger than every widget**.
A poke at `0xA38` (the BUTTON’s ICONGUID) aimed at a LABEL wrote 276 bytes past
the end of a 0x924 allocation — straight into heap metadata. The symptom was an
`ACCESS_VIOLATION in ntdll.dll` some time later, which looks nothing like the
cause. If you see a crash in ntdll, suspect an out-of-bounds write here first.

A rejected poke is now disabled and logged rather than retried every frame.

**BUTTON (type 2) creation crashed the game once** and has not been retried. The
ntdll signature points at the heap bug above rather than the button itself, but
that is unproven. A LABEL can show an icon too (`ICONGUID` at `+0x878`, plus the
`ENABLE_PICTURE` bit `0x40` at `+0x2A0`), and the game uses labels as icons
itself — `INFOBAR_RESOURCE_ICON1`. Prefer that over a button.

## Limits

- Numbers only. A label's text is a localisation GUID, not a string; arbitrary
  text needs the tag-substitution path, which is unfinished.
- The slider's digit rendering (`SLIDER_PERCENTAGE_TEXT`) is **unverified** — it
  is probably `value/resolution` as a percentage. Progress bars are the safe bet.
- Rebuilding means restarting the game.

## Paired production bars (2026-10-08)

The V4 companion snapshot retains nominal potential rates per ware (t/min).
Production rows pair the native blue demand bar and green potential bar on a
per-good shared scale, with an explicit numeric t/min caption and capacity
status. Product ICONGUIDs are generated from exact ware dataset atlas tuples
matched to icons.xml GUIDs. Peasant demand is known for SaltFish and Cider; other
goods explicitly display Unknown demand. Potential excludes paused buildings
and assumes inputs and warehouse room; it does not represent measured delivery.

Native colored bars use their full 36px three-tile height. A solid light-grey,
full native progress bar sits behind each colored fill, so the unfilled area is
opaque and does not show scenery through it. The 64-entry click ring pages the product
list so all goods remain reachable. The versioned batch capacity is 192 for all
visible row widgets plus settlement selector updates.
