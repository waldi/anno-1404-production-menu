# GUI editor fidelity plan

2026-10-09. This is a proposed implementation and verification plan, not a record
of newly confirmed engine behavior. Continue with Tauri, Rust and TypeScript;
no Python dependency or engine calls in the editor. Preserve extracted sources,
existing production-dialog work and the running game.

## Goal and definition of fidelity

Replace guessed preview behavior with verified behavior for ordinary static GUI:
FRAME, BOXLAYOUT, CARDLAYOUT, LABEL, BUTTON, PROGRESSBAR and scrolling containers.
At a specified GUI viewport, language and preview state, the editor should produce
the same widget rectangles, client rectangles, artwork, text layout and state
selection as Addon.exe. The first target is the production-style panel, followed
by representative shipped dialogs. This does not promise that every shipped
dialog or world-dependent widget can be reproduced immediately.

Two levels of acceptance are separate: geometry/state agreement and pixel
agreement. Matching rectangles does not establish correct font rasterization,
alpha blending or texture filtering. State each supported feature's evidence and
confidence; unsupported features must remain visible in the inspector. Only label
a preview verified when all visible features have passed the relevant checks.

Runtime content is a separate input: language, text substitutions, selected card,
scroll offset, hovered/pressed/disabled/checked state and progress value. Store
these in a preview scenario, not in authored XML. Live controllers, videos and 3D
render targets require explicit captured or supplied content; XML alone cannot
determine their output.

## Starting evidence and gaps

- Confirmed: 1,873 extracted GUI sources parse; 8,497 template references resolve.
  Templates apply before local properties, dispatch through per-type readers and
  do not create children. Partial EXTENT fields preserve existing values; present
  empty numeric fields become zero. This is not proof of generic nested merging.
- Confirmed in existing findings: layout pass, basic box placement, layout modes,
  scrollbar creation and z-order behavior. The TypeScript preview implements only
  a subset and clips every child to the parent rectangle, which requires replacing
  with the actual render/client clipping rules.
- Confirmed assets: 906 background definitions, 1,880 layout records, 905 matched
  preview PNGs; objectmenuinside is a 7-by-5 grid. Tile flags, resizing, state
  fallback, masks and the state represented by each preview PNG remain unverified.
- Confirmed mappings: addon icon GUID records and base icon-file map exist.
  Atlas cell order, variations, texture selection and ABM meaning remain gaps.
- Text pipeline and named fonts have existing static/live evidence, but actual
  font resources, resolution-specific selection, metrics, wrapping and complete
  tag/substitution behavior are not implemented in the editor.
- Older prose has superseded statements (font enumeration and hit-test order,
  for example). Resolve discrepancies against current Ghidra plates and findings,
  not whichever document was read first.

## 1. Build a reference harness and coverage inventory

Before rendering changes, create a feature inventory from the corpus: widget
types, property combinations, layouts, tile flags, states, font names and inline
tags. Rank both by corpus frequency and production-panel relevance.

Create small immutable fixtures and a repo-local reference manifest: source and
asset hashes, Addon.exe build identity, scenario inputs, game GUI viewport,
window capture dimensions, language, scaling, widget paths and evidence source.
Use existing screenshots first. Add screenshots and read-only runtime rectangle/
state snapshots where an existing game screen provides the needed case. The
normal shell may not see the interactive process; use the elevated read-only
process/capture route if needed and explain that requirement. Do not rebuild the
bridge or restart the game simply to collect assets.

Implement side-by-side, overlay and difference views, plus numeric comparisons
for geometry. Run comparisons at native GUI resolution before editor zoom or
Windows display scaling. Mask only genuinely dynamic content with documented
reasons; a missing renderer feature is not an acceptable masked region.

Pass: reproducible baselines for a simple frame, objectmenuinside panel, stacked
rows, overlapping widgets, button states, progress bar and text label. Geometry
can be compared automatically and image differences can be inspected. Existing
screenshots without viewport/state metadata are qualitative evidence only.

## 2. Make property evaluation and layout engine-faithful

Replace the generic candidate-leaf merge with per-widget property evaluation:
constructor defaults, template application order, local fields, empty values,
duplicate nodes, nested text settings and type-inapplicable fields. Investigate
only readers/defaults needed by the selected fixtures first. Record structural
behavior in Ghidra before mirroring confidence/evidence in findings.json.

Introduce an authoritative Rust preview model containing effective values and
their provenance, relative/absolute/client/content rectangles, generated children,
clip regions and draw order. The frontend should display these results rather
than independently deriving geometry. Share this model with drag/resize rules.

Implement the verified layout sequence, including parent client offsets,
LAYOUTMODEH/V, alignment, multi-row/column BOXLAYOUT, direction/spacing/behavior,
fit-to-children, CARDLAYOUT, hidden-child participation, scroll offsets and
generated scrollbars. Verify update ordering and convergence before assuming a
single recursive pass. Keep modal wrappers as generated preview nodes.

Separate editor selection from simulated game hit testing. Verify current
render/hit ordering, CLIPPINGDISABLED, alpha/visibility and background-dependent
clip behavior; do not preserve earlier conflicting hit-test descriptions.

Pass: integer relative, absolute and client rectangles agree exactly with known
runtime snapshots across several viewport sizes, including asymmetric backgrounds,
multi-column boxes, hidden children and scrollbar appearance. Tests cover boundary
conditions and pass ordering, not only the editor's implementation.

## 3. Reconstruct backgrounds from original textures

Inventory actual DDS formats and decode only the needed formats initially in
Rust. Resolve base/addon/patch logical paths with recorded asset provenance.
Verify archive precedence before calling that resolver the game's effective set.
Inspect ABM consumers to establish whether ABM supplies masks, metadata or other
information; do not assign semantics from its extension/header alone.

Trace background loading/rendering and turn each of the twelve observed tile
flag values into a documented rule, including combinations. Verify source
rectangles, destination grid sizing, fixed/stretch/repeat behavior, client offsets,
undersized frames, texture filtering, color/alpha and state fallback. Do not
convert every background into a generic nine-slice.

Produce draw commands for tile rectangles, UVs, texture identity, clipping,
colors and blend modes. Render actual backgrounds on the canvas from those
commands. PNG thumbnails remain useful references but cease to be render inputs.

Pass: objectmenuinside matches at its reference size and several larger/smaller
sizes; corners and borders remain correct. Additional fixtures exercise every
implemented flag, button states and PB_Background/PB_Bar/PB_Overlay layering.
Report unsupported flags/states rather than silently substituting a scaled PNG.

## 4. Resolve and render icons faithfully

Use the same texture resolver/decoder for icon maps. Verify GUID -> file/index/
variation selection, row/column ordering, transparent margins, overlays, disabled
effects, tint, ICONSPACE, ICONSTRETCH and ICONSCALE separately for LABEL and BUTTON.
Use existing confirmed button/client-rectangle behavior as a starting fixture.

Pass: a spread of goods, currency and control icons matches source crops and game
captures, including first/last atlas entries and variations. A button icon and a
child label icon reproduce their distinct geometry. Unknown variations get a
diagnostic rather than a guessed icon.

## 5. Reproduce text and state rendering

Locate/extract the required language archives into the repo without altering the
install. Resolve actual localization GUID tables, font definitions/resources and
resolution-specific font selection. Implement verified alignment, wrapping,
line height, clipping, shadows/outlines and supported inline tags. Context tags
receive named scenario inputs; missing inputs remain explicit diagnostics.

Browser fonts are not assumed equivalent. First compare advances, baselines,
line breaks and glyph pixels against the game. If Canvas/WebView2 text differs,
investigate the game's font creation/rasterization path and choose a Rust/native
text backend that can match it. Evaluate that decision with a small fixture
before moving all text rendering. Likewise, replace Canvas texture composition
only if filtering/blending comparisons show it cannot meet the acceptance target.

Implement verified state selection for hover, press, disable, check, focus,
progress and scrolling. Simulate these through scenario controls; do not execute
game verbs. Add declarative state transitions only where their semantics are
verified. Arbitrary controllers remain externally supplied scenario content.

Pass: several fonts and languages reproduce measured advances and line breaks;
short, wrapped, multiline and tagged labels agree with captures. Report glyph
raster differences independently. All ordinary button/progress/scroll states
select the verified layouts and layers.

## 6. Validate complete dialogs and edited output

Compose the verified features and compare full representative screens with held-out
fixtures that were not used to tune the implementation. Include the production
panel as a copied fixture; never replace its ongoing source. Every remaining
mismatch gets classified as assets, geometry, state, text, rasterization or runtime
content, with a concrete reproduction.

Require exact integer geometry and state agreement. For images, establish a
measured raster tolerance from repeat captures and isolate text/edge/flat regions;
do not pick a broad percentage that can hide missing borders or incorrect icons.
Track coverage by exercised behavior, not merely number of parsed files.

Game acceptance is a separate final workstream: export to mod staging, validate a
disposable RDA writer/precedence path, prove archived edits reach /compilegui and
then test in a controlled session. Loose override acceptance is not established.
Keep CDF opaque unless independent decoding becomes necessary. An exact preview
does not by itself prove that exported files load. Any install changes or runtime
write experiments require a separately scoped operation after preparation.

Pass: a defined set of complete dialogs and all their supported scenario states
meet recorded comparison thresholds; saved edits retain unrelated bytes and a
controlled archive/compiler test demonstrates game-readable output.

## Recommended order and first deliverable

Reference harness -> typed property/layout engine -> background compositor ->
icons -> text/state fidelity -> complete-dialog and export validation.
Background client offsets couple phases 2 and 3: implement those early, before
declaring layout verified. Asset inventory can proceed alongside fixture setup.

The next concrete deliverable is an objectmenuinside production-row fixture:
verified client geometry plus original-texture tile rendering at several sizes,
with an overlay/difference view and explicit coverage diagnostics. This removes a
major visible approximation while establishing the reusable validation loop.

Release gates should say exactly what is verified (for example, static frame and
box geometry plus selected backgrounds). Fully faithful arbitrary dialogs remain
unverified until their feature combinations and runtime inputs are covered. Do
not promise pixel-exact text before testing the rasterization path, or a timeline
for undecoded flags/ABM/controller behavior before the initial investigations.
