# Native UI: investigation plan and topic tracker

Living document. It lists what we still want to learn about Anno 1404 Venice's own GUI
toolkit so that an injected DLL can build in-game UI that looks and behaves like the
game's. **Add a topic here the moment you meet one during an investigation**, even
half-formed; move it up or down later. Everything that gets *answered* goes into
`docs/gui-rendering.md` (mechanics), `docs/findings.json` (facts, with confidence) and
Ghidra plate comments (derivations); this file then only keeps a one-line "done" entry.

Status: `open` `in progress` `done` `parked`. Value/effort are gut calls, 1 (low) to 3 (high).

## Follow-up (2026-09-26, 16:17) — supersedes live-test instructions below

- Newer DLL installed with `restart.py`; HUD reached and `tier2.dlg` loaded once (17 registered elements, one modal wrapper). Probe screenshot: `docs/img/tier2-check_full.png`.
- Two-target FADE_OUT real mouse click (817,617): FADER effective alpha was 1 before, 0 at 0.1 s, then 1 at 0.3/0.6/1.0 s after mouse-up. Inspection hits advanced throughout. Sustained fade remains unverified; investigate sequence duration/reset before closing topic 11.
- **Mode 1 tooltip is unsafe too.** Armed text-only override `ANNORE custom tooltip test`, hovered coin (806,276). Log at 16:16:57 reports replacement in mode 1; Addon exited before screenshot. Exact cause UNVERIFIED. Do not retry either mode blindly. Ghidra plate on `0x00D8A870` records the warning.
- Game is now closed. Topics 6 and 11 remain in progress. Next: diagnose tooltip hook timing/map lifetime and inspect fade sequence behavior. The post-exit shared-memory count of zero is not evidence that the override never ran (mapping was recreated after process exit).

## HANDOFF (2026-09-26, 15:30) -- historical; see follow-up above

State when the previous session stopped mid-Tier-2:

- **Game**: `Addon.exe` is running, in the HUD (autosave loaded), with annobridge **v8.4** installed
  (`Anno 1404 Gold Edition/d3d9.dll`, copied 14:25). That build's tooltip override runs in
  "text + layout" mode, which **crashed the game once** (14:24:46, no dump written). **Do not run
  `--tooltip` on it.** `tools/production-menu/d3d9.dll` is a newer build (tooltip mode switch with a
  text-only default, plus background-visibility sync right after the modal wrap); install it with
  `python tools/production-menu/dev/restart.py` (closes the game; about 3.5 minutes).
- **Probe fragment**: `tools/production-menu/examples/tier2.dlg` (icons, tooltip, z-order, vertical text
  alignment, alpha, sequences, modal frame). `--xml` it once per game run (ACCESSIDs `ANNORE_T2_*`),
  then set texts with `--text SYSTEMMESSAGES/ANNORE_T2/<PATHID> "..."`. The modal frame ends up under
  the wrapper background, whose PATHID is empty, so its full path is
  `SYSTEMMESSAGES/ANNORE_T2//MODAL` (double slash).
- **Mouse tests** need `tools/production-menu/dev/mouse.py` (taps ALT so the game becomes foreground). Clicks
  on empty spots hit the world; make sure the probe is on screen before clicking its buttons
  (FADE_OUT 817,617; FADE_IN 887,617; SHOW_MODAL 957,617; MODAL_CLOSE 1017,807; A/B overlap 910,395;
  coin icon 806,276, all in client coordinates).
- **Two open live checks**; everything else in Tier 2 is done:
  1. START_SEQUENCE fade: `tier2.dlg` now passes two targets (`ANNORE_T2_ICON_TAG, ANNORE_T2_FADER`)
     because tracks map to targets positionally and `buildmenufading.xml` fades in its second track.
     Click FADE_OUT and sample `FADER +0x20` at 0.3 s and 0.6 s; expect a value below 1.0.
  2. Tooltip override: after installing the current build, `--tooltip ANNORE_T2_ICON_COINS "text"`,
     hover the coin icon, screenshot. Mode 1 (text only, default) may keep the old layout size;
     `--tooltip-mode 2` also re-runs `ToolTipManager_LayoutStandard` and is what crashed. If mode 1
     does not change the visible text, the fallbacks are: call the layout from a later event (the
     TOOLTIP_FRAME's OnRender), or register a private text GUID in the text engine (block list at
     `g_pTextEngine+4`, guid ranges at block+0x1C/+0x20, per-block lookup `FUN_00DC7340`).
- **Pitfalls learned today** (all fixed in the current source): `GuiWnd_SetVisible` takes an int
  (v8.2); XML-built elements must be registered (v8.1); modal frames must be wrapped **after** the
  registration walk, wrapping during the walk frees the list node under the iterator and hangs the
  GUI thread; the fresh wrapper is visible by default and blocks its siblings until the frame is
  shown or hidden through `GuiWndFrame_SetVisible` (+0xAC); a log watcher must not key on the
  clock minute (that is the script that "ran for an hour").

## Where we stand (2026-09-26)

Tier 1 was investigated in one session (static in Ghidra, live with `tools/production-menu` v7 and
the new read-only `tools/research/annogui.py`). Details in `docs/gui-rendering.md` sections "The XML
layer at runtime", "Layout and scrolling", "Event handlers and action verbs".

- **XML is a runtime format.** Templates are parsed from the RDA lazily into a DOM at
  `g_pGuiManager+0x42C`; `GuiElement_CreateFromXmlNode` / `GuiElement_InstantiateXmlSubtree`
  build live elements from nodes. A whole dialog can be declared as a node tree and
  instantiated under any parent. **Verified live 2026-09-26** with annobridge v8 `--xml`:
  a scroll box with rows, scrollbar, skinned button and a working `SETVISIBLE` handler from one
  `.dlg` fragment (`docs/img/xml-scrollbox-2026-09-26.png`). XML-built elements must be
  registered with `GuiWnd_RegisterInRegistry` (`0x00D6DB10`); v8.1 does it.
- **Templates** copy every tag of the element type via `ReadDefinitionNode(doc, bLive=0)`;
  `GuiWnd_ApplyTemplateByIndex` (`0x00D6DE00`) is the safe wrapper. `AddChild` fires OnCreate.
- **Scrolling works**: write the target offset `+0xB0` (y) / `+0xAC` (x); the relayout clamps it
  and drives the slider. With `+0x154` = template 573 the game's own arrowed scrollbar renders,
  and knob, arrows, track and wheel all work with the real mouse (topic 19).
- **Layout pass** = `GuiWnd_RelayoutTree` every frame; BOXLAYOUT stacks children by size only;
  `+0x8C` is a render-time by-product and goes stale off-screen.
- **Handlers**: 0x54 record layout known, 613 verbs dumped with ids and listener classes
  (`docs/gui-actions.md`); `GuiEventHandlerList_AddRecord` adds one at runtime.

## Prioritised topics

### Tier 1: unlocks whole categories of UI

| # | Topic | Why it matters | Where to start | V | E | Status |
|---|---|---|---|---|---|---|
| 1 | **Build elements from our own XML** — done. `XmlDoc_ConstructEmpty` + `XmlNode_*` + `GuiElement_InstantiateXmlSubtree(parent, doc, -1)` (`0x00DBA940`) + `GuiWnd_RegisterInRegistry` per element + `GuiRegistry_RebuildPathMap`; shipped as annobridge v8 `--xml FILE` with `tools/production-menu/examples/scrollbox.dlg`. The game's parser only takes file paths through `CEngineFile`, so the DLL parses the fragment itself. | Replaces the poke-by-offset approach for everything. | `docs/gui-rendering.md` "Element factories from XML" | 3 | 1 | done |
| 2 | **Scroll box** — done. Target offset `+0xB0`, flags `+0x2B8`, template index `+0x154`, slider fields, wheel not native. Left: knob dragging with the real mouse (needs a human or SendInput), wheel translation in the hook. | | `gui_scrolling` in findings.json | 3 | 1 | done (drag/wheel follow-ups in Tier 2) |
| 3 | **Templates** — done. What they copy and why they rename; `GuiWnd_ApplyTemplateByIndex`; struct templates instantiate via the same XML subtree factory (only `EMPTYSTRUCTTEMPLATE` ships, so cloning game rows means cloning their `.tpl`/`.dlg` XML instead). | | `gui_xml_runtime` in findings.json | 3 | 1 | done |
| 4 | **Data-driven event handlers** — done. Record layout, verb table, dispatch; an `<EVENTHANDLER><SETVISIBLE>ANNORE_X_ROW1; toggle</SETVISIBLE><EVENT>OnLeftClick</EVENT>` declared in XML toggled the row live on 2026-09-26 (target resolved by ACCESSID, so the element must be registered). Left: `GuiEventHandlerList_AddRecord` on an existing element and switching `--toggle` to it. | Buttons that drive the game without our hook. | `docs/gui-actions.md`; `0x00496890` | 3 | 1 | done |
| 5 | **Layout pass** — done. `GuiWnd_RelayoutTree`, BOXLAYOUT placement, `FITSIZETOCHILDREN` = vtable `+0xD0`, `LAYOUTMODEH/V`, alignment stamp, stale `+0x8C`. | | `gui_layout` in findings.json | 2 | 1 | done |

### Tier 2: fidelity and polish

| # | Topic | Why | Where to start | V | E | Status |
|---|---|---|---|---|---|---|
| 6 | **Tooltips** -- native part done: `<TOOLTIP><TEXT>guid</TEXT>[<ALIGNMENT>][<REFWINDOW>]</TOOLTIP>` on an XML label showed the game's honour tooltip (2026-09-26). Pipeline: element `+0x2BC` block -> `RenderTree` -> `ToolTipManager_ShowForElement` -> per-frame `ToolTipManager_Update` -> `ApplyRequest` (guid -> text -> `ToolTipManager_SetText` + `LayoutStandard`), show/hide delays `DAT_0128BE10/0C`. **Left**: arbitrary text; v8.4 `--tooltip NAME TEXT` re-calls SetText (optionally LayoutStandard, which crashed once); see HANDOFF. | Native tooltips on our buttons. | plates on `0x00D894E0`, `0x00D8AC30`, `0x00D8A870` | 2 | 1 | in progress |
| 7 | **Modal frames** -- done 2026-09-26. `<FRAME_MODAL>1` = flag 0x200 (`FRAME_POPUP` 0x800) on a FRAME; the loader attaches a `CGUIWndModalBackground` wrapper (type 14, ZORDER INT_MAX-1, fills the parent) with the frame inside; `GuiWndFrame_SetVisible` (+0xAC) shows/hides both. annobridge v8.3+ wraps XML frames after registration. Verified live: show, clicks behind swallowed, close. The screen stack (which root survives menu changes) moved to topic 20. | A dialog that eats clicks. | plates on `0x00D8C340`, `0x00D8C7D0` | 2 | 1 | done |
| 8 | **Text engine leftovers** -- partly done: fonts registered (21 names, `tools/research/annogui.py fonts`); `<ALIGNMENT_VERTICAL>` 0/1/2 = top/middle/bottom verified live, `<ALIGNMENT_HORIZONTAL>` 0/1/2 maps to TextSetting 0/4/8; `AUTOUPDATE` is TextSetting definition field [3] (consumer not traced). Still open: DrawText `scale/offset`, GUIRenderer slots 19-26, `[CONTEXT_STRING]` supplier (tag table `0x0127D884`), dirty-text consumer at `g_pGuiManager+0x190`. | Exact text metrics. | `docs/gui-rendering.md` Open list | 1 | 1 | in progress |
| 9 | **Icons** -- done 2026-09-26: `<ICONGUID>1160</ICONGUID><ICONSTRETCH>1</ICONSTRETCH>` on an XML LABEL draws the coin icon, 34020 the cross; the inline `[ICONGUID 1156]` tag draws the laurel inside text. `[ICONGUID n w h]` did not render (syntax unverified). | Goods icons in rows. | `docs/img/tier2-2026-09-26.png` | 2 | 1 | done |
| 10 | **Z-order** -- done 2026-09-26: `GuiWnd_SetZOrder` sorts the parent's child list ascending (`GuiWnd_SortChildrenByZOrder`); render = list order, the hit test walks the list backwards, so the higher ZORDER draws on top and gets the click (verified with two overlapping labels). | Overlays. | plate on `0x00D6D4B0` | 2 | 1 | done |
| 11 | **Alpha and sequences** -- mostly done: `<ALPHA>0.4</ALPHA>` -> `+0x3BC` -> effective `+0x20`, rendered translucent. An `EVENT_REACTION_SEQUENCE` file on an XML label scaled it to 1.1 on hover. `START_SEQUENCE path; marker; targets` goes to `SequencePlayer_Start` (`0x00D9C400`); tracks map to targets positionally; the `FADEIN`/`FADEOUT` verbs are stubs. **Left**: the two-target fade test (HANDOFF). | Fades and highlights. | plate on `0x00D9C400` (sequence grammar) | 1 | 1 | in progress |
| 12 | **Drag and drop**: `CDragAndDropManager`, `CGUIWndGhostButton`, `OnDrop`. | Reorderable lists. | RTTI | 1 | 3 | parked |
| 20 | **Screen stack**: `CGUIScreen`, `GuiScreen_*`, which root survives menu changes (runtime elements under `SYSTEMMESSAGES` die on return to the main menu), `GuiManager+0x420` root and the modal counter at `+0xD0` (`FUN_00D84CB0`). | A panel that survives menus. | RTTI vftables | 2 | 2 | open |
| 19 | **Scrollbar input** — done 2026-09-26. Knob drag, arrows, track paging and the mouse wheel are all native on a runtime scrollbar; the wheel is handled in `GuiWnd_OnMouseWheelUp/Down` before any named event (the earlier "wheel not native" conclusion was wrong). `tools/production-menu/dev/mouse.py` drives the real mouse for tests. | Lists that feel native. | `gui_scrolling` in findings.json | 2 | 1 | done |

### Tier 3: tooling and hygiene

| # | Topic | Why | Where to start | V | E | Status |
|---|---|---|---|---|---|---|
| 13 | **Path-map rebuild on demand** — `--rebuild-paths` shipped in v8. Left: an ACCESSID lookup op (`registry+0x1AC`) so hidden elements can be reached by ACCESSID too (hidden elements dispatch no events, so `--inspect ACCESSID` reports "never appeared"). | Stops the "never appeared" dead ends. | annobridge `FindByFullPath` | 1 | 1 | in progress |
| 14 | **`--xml FILE` in annobridge** — done (v8/v8.1, 2026-09-26): parses the fragment, builds nodes, instantiates, registers, rebuilds paths; `--skin-index` now uses `GuiWnd_ApplyTemplateByIndex`; `--rebuild-paths` added. Left: `--toggle` via `GuiEventHandlerList_AddRecord`. | Repeatable experiments, no more pokes. | `tools/production-menu/README.md` v8 | 3 | 1 | done |
| 15 | **Intro-movie detection** for `restart.py`. | Deterministic restarts. | `CGUIWndVideoPanel` vtable, `+0x9C` of the video panel | 1 | 1 | open |
| 16 | **Loose-file override**: does `XmlDoc_LoadFile` (via `CEngineFile` `0x00B28B60`, exists-check `FUN_00B27EE0`) open a loose file whose path is *not* in the RDA? The runtime document names are the normalised absolute install paths, so the layer is path-based; test with `XmlDoc_ConstructByName` on an absolute path from the DLL. `/compilegui` and `anno.cdf` (`0x010A60FC`) are a separate question. | A `.dlg` on disk as the delivery format. | `0x0042ACF0`, `CFileChangeManager` | 2 | 1 | open |
| 17 | **Element type coverage**: EDITFIELD, COMBOBOX, LISTBOX, CARDLAYOUT, SLIDER as input, `CGUIWndTab`, `CGUIWndDotlineButton`, RENDERTARGET. All have XML constructors listed in `GuiElement_CreateFromXmlNode`, so topic 14 covers creation; what remains is what each needs to be useful. | Knowing the toolbox. | `0x00DA14F0` | 1 | 2 | open |
| 18 | **Crash safety**: catalogue calling conventions (RET n) of every function annobridge calls; debug build that logs each call. New candidates: `GuiElement_InstantiateXmlSubtree` (3 stack args, `__cdecl`), `GuiElement_CreateFromXmlNode` (2 stack args), `GuiWnd_ApplyTemplateByIndex` (`__thiscall`, 1 arg), `GuiEventHandlerList_AddRecord` (`__thiscall`, 2 args). | Fewer restarts lost to stack corruption. | `tools/production-menu/README.md` crash notes | 1 | 1 | open |

## Done (one line each, details live elsewhere)

- 2026-09-18 Render stack, GUIRenderer slots, text pipeline, fonts, inline tags — `docs/gui-rendering.md`.
- 2026-09-18 Runtime creation, nesting, relative extents, clipping, backgrounds, progress-bar art — `docs/gui-rendering.md` live results.
- 2026-09-18 Hit test (`GuiWnd_HandleInputEvent`), click → toggle, template renaming pitfall, STLport short strings — Ghidra plates + `findings.json`.
- 2026-09-18 Savegame load + Set sail by dispatched click; `restart.py`.
- 2026-09-20 Scrollbar creation via `+0x2B8` bits + OnCreate; BOXLAYOUT layout fields (`+0x6F0..+0x702`).
- 2026-09-20 Scrolling (topic 2): target offset `+0xB0`, slider range/page, template 573 renders the native bar, wheel not native — `gui_scrolling`, screenshot `docs/img/scrollbox-2026-09-20.png` (skinned bar without OnCreate, scrolled to 80, bar after OnCreate).
- 2026-09-20 Templates (topic 3): `GuiTemplate_Apply` = `ReadDefinitionNode(doc, 0)`; `GuiWnd_ApplyTemplateByIndex`; `AddChild` fires OnCreate — `gui_xml_runtime`, plates on `0x00DB6740`, `0x00D6DE00`, `0x00D6E000`.
- 2026-09-20 Layout pass (topic 5): `GuiWnd_RelayoutTree`, BOXLAYOUT placement, `FITSIZETOCHILDREN`, `LAYOUTMODE*`, stale `+0x8C` — `gui_layout`, plate on `0x00D6FCC0`.
- 2026-09-20 Handler records and 613 verbs (topic 4, static part) — `gui_actions`, `docs/gui-actions.md`, plate on `0x00D78C90`.
- 2026-09-20 XML runtime layer (topic 1, static part): `CXMLExtNode`, `CConfigDocTemplate`, factories — `gui_xml_runtime`, plates on `0x00DA14F0`, `0x00DBA940`, `0x00429E50`, `0x0042CD80`, `0x00428670`. Tool: `tools/research/annogui.py`.

- 2026-09-26 Tier 2 (7, 9, 10 done; 6, 8, 11 in progress): tooltips, modal wrapper, z-order, icons, alpha, sequences -- plates on `0x00D894E0`, `0x00D8AC30`, `0x00D8C340`, `0x00D8C7D0`, `0x00D6D4B0`, `0x00D9C400`, `0x005FCB50`; `docs/img/tier2-2026-09-26.png`.
- 2026-09-26 Scrollbar input (topic 19): all native; `GuiWnd_OnMouseWheelUp/Down`; SetVisible int pitfall; `mouse.py` — `gui_scrolling`, plates on `0x00D6C910`, `0x00D627E0`, `0x00D61D80`.
- 2026-09-26 XML route live (topics 1, 4, 14): annobridge v8 `--xml`, `GuiWnd_RegisterInRegistry` requirement, `docs/img/xml-scrollbox-2026-09-26.png`.

## Inbox: raised during investigations, not yet triaged

- `+0x2B4..+0x2C0`: now `+0x2B4` ZORDER, `+0x2B8` scrollbar flags, `+0x2BC` TOOLTIP (topic 6); `+0x2C0` still unknown.
- `INFOBAR` reports width 1365 while `SYSTEMMESSAGES` is 1280x1024: is the GUI coordinate space per-screen, and how does it map to the window? (`GuiWnd_ApplyLayoutMode` with LAYOUTMODE 1 fills the parent; INFOBAR may simply be wider than the screen.)
- `GuiWnd_IsHovered` reads `+0xA0 == 1`, but the real diplomacy button also reads 0 at rest: confirm it is transient hover state.
- Type-15 VIDEOPANEL with empty PATHID receives mouse events over the world: is that the 3D view's input proxy, and can our panels be parented to it?
- Element `+0x578` is a per-element list of action listeners (`vtable+0x10(record,args)`, −1 = decline). Which element types populate it (CARDLAYOUT? LISTBOX?) and can we register our own object there to receive verbs?
- `GuiActionRecord.queued` verbs (`SETVISIBLE`, `FITSIZETOCHILDREN`, `START_SEQUENCE`, …) run deferred from the list at `g_pGuiManager+0x188`; when exactly, and does that matter for ordering with our hook?
- Three dead functions `FUN_00DB87D0` / `FUN_00DB89B0` / `FUN_00DEE1A0` build `.dlg` paths from a dialog name and load/save XML at runtime (`FUN_00D545F0` looks like an XML *writer*). Editor leftovers with no callers; a writer would let us dump a live element back to XML.
- A hidden element dispatches no events, so annobridge's event-based matching (`--inspect NAME`, pokes by bare name) cannot see it; full paths (registry map) still work. An ACCESSID-map lookup op would close the gap (topic 13).
- annobridge pitfalls found: pokes stay armed across target changes; `--skin-index` needs a following `--oncreate` for art. Noted in the README.
- Another session was driving annobridge in the same game (22:46, `ANNORE_PANEL`/`ANNORE_BTN`); the bridge is single-target. Coordinate before live tests.
