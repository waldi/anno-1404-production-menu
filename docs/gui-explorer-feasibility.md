# GUI explorer / visual editor feasibility — 2026-10-09

This records the initial investigation. The subsequent compiled Tauri editor is
under `tools/gui_editor/`; current implementation and verified limits are in
`gui-editor-fidelity-progress.md`. Original DDS backgrounds, icon crops/label
placement, layout comparisons and raw localization inspection now go beyond
the initial Python explorer described below, which has been removed after being
superseded by the compiled editor. CDF rewriting and game-accepted
archive export remain open.

A standalone explorer is feasible now and implemented in
`tools/gui_editor/`. A visual layout editor is feasible incrementally, but an
engine-faithful WYSIWYG renderer and a reliable archive export/build loop need
further verification. No released `.dlg`/`.tpl` WYSIWYG editor was established;
editor-oriented metadata and dead load/save functions are leads, not evidence
that an internal editor existed or was released.

This investigation used extracted files and the Addon.exe Ghidra project only.
No live process inspection, bridge rebuild, game restart, archive modification,
or changes to the production dialog were needed. The initial dirty worktree
included `docs/findings.json` and eight production/bridge files, plus screenshot
and goods-icon artifacts. Only an additive findings entry accompanies this tool.

## Verified corpus and round-trip boundaries

| Format | Evidence / confidence | Safe scope today |
|---|---|---|
| `.dlg`, `.tpl`, `.stp` | Confirmed extracted corpus: 85 FRAME dialogs, 881 TEMPLATE roots, one STRUCTTEMPLATE; all parse. | Read all nodes, ordered children, repeated tags, empty tags and comments. Source bytes retained unchanged. |
| `.bkg` | Confirmed: 906 BackgroundLayout XML files; 1,880 named layout records. Every record has `m_XSize * m_YSize == IconList` count. | Parse layout names, grid dimensions, client offsets, local texture paths, tile rectangles, flags and unknown fields. |
| XML serialization | Confirmed: all 1,873 source files survive ElementTree serialize/reparse with the same normalized node model. No attributes observed in this corpus. | Structural read/write is plausible; **not** proof of game acceptance after edits. Whitespace, empty-tag spelling and encoding can change. |
| `.cdf` | Confirmed file observations: addon 18,654,609 bytes; base 15,151,978 bytes; headers differ. Existing `/compilegui` experiment in gui-system.md compiled archive inputs and ignored loose overrides. | Retain/copy opaque bytes only. No decoded serializer, relocations, version model or safe field editing was implemented. |
| Icons | Confirmed: addon game/icons.xml has 2,362 GUID records with IconFileID/IconIndex/VariationID; base gui/iconfilemap.xml has 124 entries mapping file IDs to nominal PNG paths and cell dimensions. | Parse mappings/XML; original DDS/ABM bytes remain opaque to this prototype. Pixel index ordering, variation precedence and archive overrides need verification. |

For a future editor, retain original bytes and implement minimal patches in a
**separate working tree** using a lossless XML representation. Preserve unknown
nodes/attributes, comments, duplicate tags, order, encoding/BOM and unchanged
byte regions. Keep flattened preview values separate from authoring XML so
template inheritance is not baked into exports accidentally. Structural
round-trip tests alone do not establish lexical preservation or engine acceptance.

## Templates and widget tree

Confirmed corpus: 8,497 non-empty TEMPLATE_REF_ID occurrences, zero missing or
ambiguous targets, zero duplicate template ACCESSIDs and no inheritance cycles.
Template IDs are indexed by ACCESSID. Inspection of all widget chains passes.

Ghidra plates on `GuiTemplate_Apply` (0x00DB6740) and
`GuiElement_ReadDefinitionNode` (0x00D6E1A0), plus the latter's decompilation,
confirm template application precedes local definition properties and dispatches
through the widget's type-specific reader. It does not create children. The
factory plate at 0x00DA14F0 establishes the recognized widget classes; flags
such as FRAME_MODAL, FRAME_POPUP and TOGGLEBUTTON are not containers.

New static verification, recorded in Ghidra at 0x00DC2090 first: EXTENT saves
existing width/height, reads each provided coordinate/size, and reconstructs
right/bottom. The long reader at 0x00DC1DE0 leaves an absent field untouched and
sets a present empty field to zero. Partial EXTENT inheritance is thus confirmed.
Local PATHID/ACCESSID are read after template application, including empty names.

The explorer shows local widget children and all ordered property layers with
provenance. Its last-explicit-leaf property table is labeled **candidate**:
generic merging of TEXTSETTING, TOOLTIP, flags and repeated EVENTHANDLERs has
not been proven. Constructor defaults and properties inapplicable to a particular
widget are not filtered. This is an understandable source tree, not a runtime dump.

## Backgrounds, states and preview fidelity

906 `.bkg.png` files exist, but only **905 match a `.bkg`**: mainmenu_anno_logo_button
has no preview; general_buttons/12x12_circle.bkg.png has no corresponding source.
All matched PNGs can be displayed directly without reconstructing the renderer.
The objectmenuinside preview is 94×94; its EmptyLayout has a 7×5 grid with 35 tile
records, rather than a universal 3×3 nine-slice description. Its tile records
include flag 4. This does not establish flag semantics or how the game stretches it.

States include EmptyLayout, Normal, mouseover/pressed/disabled, Checked variants,
PB_Bar/PB_Background/PB_Overlay and slider Thumb states. Twelve tile flag values
occur (0,1,2,3,4,8,32,36,64,65,68,96). Their full rendering semantics, state fallback,
stretch versus repeat, masks and color modes remain **unverified** here.
Selecting a state in the prototype shows metadata, not a re-rendered state.
Scaling a PNG would distort borders and cannot establish game fidelity.

`m_localIconFileMap` points at textures, but those nominal PNGs can actually be
stored as DDS plus ABM: ingame_gui_map_01_0.dds has DDS magic and a 1024×1024 header;
ingame_gui_map_01.abm begins `ABM\x01`. The ABM body is not decoded. Decode the DDS,
verify tile coordinates against crops, then correlate several flags and state
variants with existing screenshots before implementing a background compositor.

## How much preview needs the binary?

Assets alone provide enough for source browsing, template dependency inspection,
explicit extents, background images, state inventories and icon mapping discovery.
Existing Ghidra findings provide BOXLAYOUT stacking, alignment/layout modes,
clipping, z-order and scrollbar behavior. Those support an approximate layout
preview with explicitly stated limits, without new binary work for every widget.

Faithful preview still needs a verified background compositor, constructor defaults
and per-type merge rules, font metrics/localization and text substitutions,
runtime visibility/card selection, generated scrollbars, animation, and menu
controller updates. RENDERTARGET/VIDEOPANEL and context-dependent labels need
placeholders or captured examples. Extracted archive precedence is not fully
established; this addon corpus cannot be advertised as the final live asset set.

## Export and staged recommendation

1. Keep the read-only explorer as the first deliverable. Add icon crops and a
   background grid/state compositor after verifying textures and flag meanings.
2. Add approximate explicit-extent/BOXLAYOUT preview with unsupported cases shown.
   Only then add dragging and property changes in separate working copies, with
   source diffs and unknown-content preservation tests.
3. Export source `.dlg`/`.tpl`/`.bkg` at their original logical paths into a separate
   mod staging directory. Verify archive precedence and RDA writer integrity on a
   disposable archive before packing edited sources. Existing RDAExplorer binaries
   are present, but this investigation did not execute or validate their writer.
4. Establish an isolated compiler acceptance test: changed archive source with a
   unique PATHID, `/compilegui`, confirm marker in output and load the result in a
   controlled game session. Compiler/template source consistency must be checked;
   never treat XML parse success as proof the game loads the change. This task did
   not run a compiler or touch an install/archive.

The existing bridge's XML subtree route proves an alternative runtime experiment
path in earlier work, but it writes inside the game and is outside this read-only
prototype. It is not evidence of distributable data-only export.

Public discovery checked ProjectAnno's Anno 1404 downloads and the AnnoDesigner
project. These identify map/tool workflows and a building-layout designer; no
matching `.dlg`/`.tpl` editor was found in this focused search. This is a bounded
negative result, not proof of historical nonexistence:
https://projectanno.de/Downloads/Anno%201404.html and
https://github.com/AnnoDesigner/anno-designer.

Validation: three unittest checks pass, including full corpus normalized
round-trips/source hashes and HTTP GET-only boundaries; JavaScript passes
`node --check`. Localhost tests needed elevated execution because sandbox network
isolation rejected the loopback connection. No elevated game access was needed.
Browser verification also passed: searching/selecting infobar.dlg produced the
widget hierarchy; selecting objectmenuinside.bkg displayed its PNG and EmptyLayout
metadata. These checks describe the retired prototype; its files and screenshot
were removed during cleanup. Current checks are documented in the editor README.
