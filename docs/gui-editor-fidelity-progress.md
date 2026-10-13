# Fidelity implementation progress — 2026-10-09

The sessions through 18:30 follow `gui-editor-fidelity-plan.md`. No milestone is
declared fully complete merely because its implementation tests pass.

| Milestone | Implemented / evidence | Gate still open |
|---|---|---|
| 1. Reference harness | Compiled read-only `gui-audit` inventory with source hashes and feature frequencies; checked-in corpus summary; hashed existing screenshot references; Compare panel with native-size overlay/difference and numeric metrics; read-only capture script with viewport/origin, binary hash and widget snapshot. | Complete scenario/language metadata and stable visible reference cases for all listed fixtures. |
| 2. Semantics/layout | Rust is authoritative. Recovered frame screen-edge offsets, pre/child/post/mode order, fixed/automatic wrapping and reverse asymmetry, pure fit-to-children, minimum inherited alpha and first-handler-wins. Live infobar comparison matches 25 visible source widgets at unit scale. | Complete per-type defaults/merges, generated scrollbars, scaling/animations, modal/type overrides and held-out multi-dialog comparisons. |
| 3. Background compositor | Original DDS/grid compositor, non-drawing cells, both asset namespaces, BC1/BC2/BC3 and 32-bit raw decode; 23 original assets extracted through temporary archive copies. 1,853/1,880 explicit states pass parser/bounds/decode audit. | 27 state failures, ABM, archive precedence, state fallback, colors/effects and broad pixel validation. |
| 4. Icons | Explicit GUID+variation crop inspector and verified unit-scale normal LABEL icon placement/client/inset/clip rules. | Other widget types, runtime variants, overlays/disabled/tint effects, inherited scaling and live pixel comparisons. |
| 5. Text/states | Explicit native font probes; 121 captured run metrics exact. Plain token placement and ordinary word wrapping: 22 blocks exact in two captures with explicit English GUID sources. Button-state selection and progress layer scenarios implemented. | Glyph pixels, rich tags/substitutions, oversized words, owner alignment, full widget state logic and live progress evidence. Dialog captions remain placeholders. |
| 6. Whole dialogs/export | Source preservation tests retained; game/install and original production changes preserved. | Held-out complete-screen comparisons and isolated RDA/compiler acceptance. No archive exporter shipped. |

## Background evidence

The packaged native WebView renders objectmenuinside at 1080x114 from its original
atlas. Compared the top 20px strip against the existing production screenshot,
after compositing transparent artwork over the panel color sampled at [100,600].
Across 21,600 pixels, mean RGB channel error was 0.0174074 and maximum error was 1.
The selected strip contains no foreground labels/icons. This is background-strip
evidence, not a full-row comparison or proof of all flag/state behavior.

Crop [333,211,1080,114] corresponds to CARD1's last rendered absolute rectangle
and capture client origin. The currently running production dialog is hidden, so
the captured absolute rectangle is stale. An older screenshot plus that geometry
does not substitute for a fresh visible-dialog reference. No UI input was sent.

Reproduce with the built `anno-gui-studio.exe --smoke-test`, then run
`tools/gui_editor/compare-background-reference.ps1`. Generated PNGs and metric JSON
are under `.tmp/`; the script captures reference/render hashes in the result.

## Inspection and testing

Structural behavior was queried and annotated in the Addon.exe Ghidra project
before adding findings.json entries. Relevant plates: background allocation
0x00DBF3C0, draw path 0x00DBFE30, client rect 0x00D5AD90, box constructor
0x00D8D9C0, atlas draw 0x00DECE40 and atlas grid initialization 0x00DEB210.

Checks include source preservation/conflicts/undo, complete source corpus,
background allocation, DDS masks/truncation, BC3 alpha branches/colors/partial
blocks, explicit icon crop/variant rejection, client/mode/box geometry,
frontend edit constraints, native-size pixel metrics and stretch/repeat commands.
Packaged smoke uses actual Tauri IPC, WebView image decoding and Canvas rendering;
it does not establish user-driven visual interaction or game parity.

Latest checks: 22 Rust tests and 8 frontend tests passed; TypeScript/Vite and the
release build passed; packaged native smoke exited successfully. Release hash and
the smoke result are recorded in `tools/gui_editor/fixtures/session-verification.json`.
Findings and fixture JSON parse successfully, and `git diff --check` passes.

An editor executable was locked during compilation. The user subsequently
authorized closing editor builds and deleting obsolete builds. The current build
uses `anno-gui-studio.exe`; run.ps1 points to it. Obsolete editor/scrollfix artifacts
were removed; `gui-audit.exe` remains the separate read-only verification utility.
Smoke uses a separate profile inside `.webview/` and writes only test artifacts.

The new decoder's BC3 format reference is Microsoft's
[Direct3D block-compression documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression).
Numerical GPU/browser raster agreement still needs its own comparison evidence.

## Follow-up evidence through 18:30

Read-only captures at 1440x1080, unit effective scale, agree on INFOBAR geometry.
The latest fresh capture at 16:15:59 UTC has 9,879 total runtime widgets. With
captured visibility as the only runtime layout input, all 25 visible source
widgets match raw and absolute rectangles exactly. The report separately retains
51 hidden widgets, one offscreen widget and two extra runtime widgets. Captured
rectangles are never fed into the preview. A smaller immutable infobar fixture
plus source hash is checked in; the regression checks source bytes are unchanged.

Compare inspector accepts schema-2 snapshot JSON and can apply/reset captured
visibility as a separate scenario. Hidden absolute rectangles can be stale and
are not accepted as geometry parity. This is one static scenario, not a proof of
text, artwork, clipping, state or full-dialog parity.

The background audit covers all 1,880 explicit layout records at 1000x500.
It checks allocation, drawable texture resolution, DDS decode and source bounds.
After staging missing original addon assets and preserving non-drawing grid cells,
1,853 pass and 27 remain unsupported: 20 layout flags, five missing textures,
one drawable invalid texture ID and one source-bounds failure. No state fallback
or archive precedence is guessed. Source archives are opened read-only to make
disposable copies; the RDA library only accesses those copies because it opens
read/write. Extracted conflicting files cause an error instead of replacement.

Label icon rendering now uses recovered unit-scale client geometry and normal
variation0. ICONSPACE WIDTH/HEIGHT are far-edge insets under stretch, not explicit
destination dimensions. ICONCLIP=false escapes to viewport clip. Constructor
defaults are verified for LABEL only. Normal crop placement does not verify
runtime GUID/variation, disabled effects, tint, overlays or pixels.

English tables use UTF-16 BOM encoding; the inspector decodes this strictly and
preserves raw inline tags in displayed candidates. Base/addon named-font XML is
shown independently. Georgia TTF is staged, but font aliasing, resolution-specific
metrics, markup and rendering are still open. Raw resource inspection is useful
for the next font investigation without adding a guessed canvas text renderer.

Next: font registration/selection and native metrics, button/progress state
evaluation, generated scrolling children, and held-out screenshot comparisons.
All six broad milestone gates remain open until their listed evidence exists.

The final packaged smoke also checks strict UTF-16 GUID lookup, named-font
inspection, label-icon placement and the 25-widget geometry fixture through
actual Tauri IPC. Source-derived HIDDEN values no longer dim a widget made
visible by a scenario override. Final hashes and scoped evidence are in
`tools/gui_editor/fixtures/session-verification.json`; the native test saves no
working copies. Temporary archive copies are removed after extraction.

## Native text metrics, continued session

Recovered FontContainer measurement and locale font parsing now drive an
own-process GDI probe. The inspector requires an explicit staged font candidate;
it does not guess archive precedence. Compare imports read-only cached LABEL run
captures. The immutable 1440x1080 unit-scale fixture contains 121 runs across eight
font/style groups, all with exact width/height agreement. Two later captures each
contain 179 exact runs and zero mismatches. Captured sizes are never measurement
inputs; deliberately changing a recorded width produces a regression mismatch.

Ghidra partial structures and plates were saved before findings.json was updated.
The helper searches existing processed text only, using bounded read rights; it
does not create game text objects, invoke game functions or access game GDI handles.
Captures are non-atomic and include hidden cached runs. Font color/shadow field
metadata was corrected during investigation; the immutable metric fixture omits
those fields because this gate concerns dimensions only.

The game creates a separate D3DX render font. Exact GDI run metrics therefore do
not establish glyph pixels, wrapping, alignment or full text layout. Current
canvas captions remain editor placeholders. The next text gate is reconstructing
line/part placement from supplied text, then comparing it with live cached runs.

The continued-session test suite currently passes 25 Rust and 8 frontend tests.
The packaged smoke additionally exercises the 121-run comparison and explicit
XML-font probe through actual Tauri IPC; final build evidence is recorded in the
session manifest after smoke passes.

## Button states and progress content, continued session

The editor now exposes explicit button hover/press/check/enabled scenarios. State
selection follows the saved game code, including case-insensitive names, fallback
to the first layout, disabled ancestors and the pressed-outside-hover branch.
Selected state client offsets feed the same Rust layout used for geometry and
icon previews. The immutable capture has 53 exact visible-button indices: 48
Normal, three Checked and two Normal+Disabled. Hover/pressed transitions have
static and synthetic evidence; no game input was sent to exercise them.

Progress scenarios use the recovered float32 ratio, integer truncation, direction,
maximum guard and PB_Bar/PB_Overlay destination rules. They modify no XML. The
preview assumes PB_Background steady state; missing state/client history is
reported. Tests verify allocation and texture composition, but there is no live
progress fill or pixel acceptance gate yet.

A fresh Checked MINIMAP_FORCE_NORTH background test failed on all 87 rendered
opaque pixels (mean RGB error 19.4176245; maximum 166). This is evidence against
claiming full fidelity. The selected index matched; icon was absent and text
was disabled. A later stable capture reports white effective/background tint,
secondary color zero and fade one. Asset precedence and texture sampling remain
hypotheses, not established causes. No fitted artwork correction was applied.

Next steps: trace texture load/version selection and D3D sampling, reproduce the
checked-button pixel test with independently verified render coordinates, then
capture a live progress fixture. Separately reconstruct text line/part placement;
exact GDI run dimensions do not establish wrapping or glyph pixels. Broad
milestone gates remain open.

Current checks: 32 Rust and eight frontend tests. The release manifest records the
packaged IPC smoke and build hashes after validation. The Python explorer is
retired; release has anno-gui-studio.exe (app) and gui-audit.exe (CLI validator).

A further bounded live capture verifies all four Checked source/destination tile
rectangles, flags and logical texture filename against the standalone 24x24 grid.
The game remaps authored FileID 17 to runtime ID 2; the actual loaded layout color
is white. These independently captured outputs are now a regression fixture.
This excludes a different tile grid or logical filename in that capture; GPU
texture bytes, archive selection, filtering and final draw origin remain open.
The helper capture-texture-reference.ps1 records existing CPU metadata only.

## Archived artwork variants: pixel discrepancy narrowed

The minimap atlas occurs with three different hashes in base Data2, base patch0
and base patch1. Original extracted files remain unchanged; patch candidates are
staged separately under tools/gui_editor/fixtures/archive-candidates. The read-only
archive inventory used disposable copies because RDAExplorer opens its input for
read/write. Copies were disposed and removed; selected file hashes were verified.

With unchanged geometry, crop and renderer, explicit patch0 and patch1 artwork
both pass the original checked-button opaque-pixel gate: 122 pixels, maximum RGB
channel error 1 and mean 0.01639344. The base candidate fails. Changing artwork
alone is sufficient to resolve this scoped discrepancy. Both patches produce
identical pixels here, so this cannot establish which archive the game loaded.
The inspector now exposes explicit candidate selection; no patch priority is
silently guessed and no selection is written into XML.

A later capture is excluded because the game is showing its pause menu and the
MINIMAP parent is hidden. Its absolute cache rectangle is stale. The comparison
helper reports excluded_hidden_ancestor, with no eligible pixel result. A second
independent visible capture and coverage of other states remain open. No game
input was sent to force the scene back.

Current checks pass 33 Rust and eight frontend tests; the packaged IPC smoke also
renders both staged candidates. Next: verify effective archive ordering in the
file loader, apply an explicit artwork profile consistently to dialogs and icons,
then gather independently visible held-out states. Full blended button pixels,
text and full-dialog parity are not established by the opaque-pixel check.

Static startup registration is now traced and saved in Ghidra: InitFileSystem
mounts base data0..5, base language0, base patch0..1, addon data0..5, addon language0
and addon patch0. Path insertion retrieves an existing terminal filename and
replaces its metadata, so later registered duplicates win in this startup map.
This provides an evidence-based next implementation step for an artwork profile.
It does not cover loose-file settings, later custom mounts or completeness of the
extracted asset census. Existing per-background candidate choice remains explicit.

The minimap became visible again without input from this tool. A second independent
visible capture at 19:25:58 UTC also passes the patch1 opaque-artwork check: 122
pixels, maximum channel error 1, mean 0.01639344. Recorded visible ancestry and
Checked index make this capture eligible. Rust regression now checks both visible
captures against both patch candidates. The hidden pause-menu capture is retained
as explicitly excluded evidence. Reproduce with -Candidate patch1 -VisibleHeldOut.

## Consistent artwork profiles and independent text placement

The default English startup sequence has now been inspected across all 17 archives.
The hashed whitelist census contains 92 staged assets; original archives and extracted
assets remain unchanged. The editor exposes a global artwork profile shared by
backgrounds, LABEL icons and progress layers, showing the actual selected source.
Known startup registration order chooses the latest available registered duplicate;
loose-file settings, later custom mounts and actual live file origin remain open.
The packaged IPC smoke asserts patch1 minimap selection, icon source identity and
progress composition through this profile.

The new plain-text placement gate reconstructs direct-string tokens using original
strings, registered font-name lookup and the retained layout width. Current client
width may already have changed through fitting, so it is not a substitute input.
The independent fixture has 13 exact labels, zero mismatches and 60 unsupported
blocks. It checks token sizes, local offsets and pre-alignment line bounds; a changed
captured offset must fail. The Compare inspector and compiled audit CLI expose it.
Rich tags, GUID resolution, wrapping, owner alignment, cache lifecycle and glyph
pixels still require implementation and separate evidence. All broad gates remain open.

Explicit extracted English GUID resolution extends this gate to 21 exact placements,
zero mismatches and 52 unsupported blocks. Each resolved GUID reports source paths
and hashes; missing/ambiguous candidates are rejected. The language selection is
independent of the running game. No cached token concatenation becomes source text.

Current checks: 36 Rust tests and eight frontend tests pass. Release build and
packaged smoke hashes/results are recorded in the session verification manifest.

Ordinary word wrapping is now implemented and verified for GUID 160014, using
independently extracted English text and retained width 280. Ghidra DB0140 calls
DC3480 -> DC4A70 -> DC4940 to trim and remeasure one trailing space from the
previous line's last token before wrap alignment. Its pre-trim line bounds remain.
This produces exact tokens, offsets and bounds on both captured lines. The final
text gate is 22 exact, zero mismatches and 51 unsupported blocks in the first
capture; held-out capture is 22 exact, zero mismatches and 52 unsupported blocks.
Oversized-word splitting, rich tags, runtime substitutions, owner alignment and
glyph pixels remain open. This extends the earlier single-line result above.

## Next session

1. Extend the recorded tokenizer/layout path for inline AUTOSIZE_ICONGUID,
   font/color transitions and oversized-word splitting. Keep raw source strings
   and explicit scenario substitutions as inputs; cached parts are outputs only.
2. Recover owner text origin, vertical alignment and clipping, then compare D3DX
   glyph rasterization independently. Dialog captions remain placeholders.
3. Collect eligible visible progress and remaining button-state fixtures. Check
   blended pixels, tint, focus and filtering without fitting screenshot offsets.
4. Validate complete dialogs and isolated archive/compiler export after component
   gates pass. Current saves remain separate XML working copies; game-readable
   archive/CDF output has not been demonstrated.

The 22:00 cutoff ends this session, not these fidelity gates. Production-dialog
work, original extracted assets and game files remain preserved.

## Continued session: inline text parts

The resumed session extends the local layout engine to FONT/COLOR/DEFAULTCOLOR,
alignment changes, ordinary CR and inline icon parts. New bounded read-only captures
include fontless parts and original layout color. The first capture matches 34
blocks and the independent second capture matches 33; both report zero mismatches
and 39 unsupported blocks. One quest has four lines and two inline icons. Comparison
checks local dimensions, offsets, pre-alignment bounds, font names, color, icon GUID,
variation and clipping flags. Mutating a captured icon GUID, color or offset must fail.

CR uses the last part advance rather than empty-line union bounds. AUTOSIZE_ICONGUID
uses the current font's measured capital-W height. Ghidra structures and plates were
recorded before extending capture fields. These are narrow cached-part checks, not
visible owner-origin or glyph-pixel parity. Numeric CR, oversized splitting,
ellipsis, runtime callback tags and remaining branches are explicitly unsupported.
The packaged smoke now exercises both all-part fixtures through actual Tauri IPC.

## Native glyph atlas and first pixel witness

The read-only renderer capture selects mode 3 (D3D10), rather than identifying the
backend from loaded DLLs. The GUI font command follows the custom GDI atlas path
through A463B0/A470E0, not the alternative direct D3DX font wrapper. Ghidra partial
structures and plates precede the bounded font-resource captures. The standalone
Rust builder reproduces all 5,632 page-zero glyph metadata records across 22 fonts
in each of two independent captures, including dimensions, advance, bearing and UV.
This metadata comparison does not establish texture or screenshot parity.

The software probe implements the recovered integer glyph quads, font shader color
equation, outline masks and per-glyph shadow order. A stable island-name screenshot
matches all 370 predicted fully opaque pixels exactly at [729,52], without searching
or adjusting the origin. The original string is supplied independently from the
current island settlement name; cached text parts are not raster inputs. This is a
single Headlines font witness, not a complete text-rendering gate. Translucent
edges, fractional filtering, clipping and additional visible witnesses remain open.

A later capture is excluded because the production dialog covers the HUD. Ancestor
visibility alone cannot establish that screenshot pixels belong to the widget.
The earlier floating-message probe differed at 28 of 78 opaque pixels; its animated
position and non-atomic capture leave the cause unverified. Neither failed/excluded
case is treated as evidence of broad parity. No game input or bridge changes were
used to expose a more convenient fixture.

The font inspector now calls the native single-run raster probe instead of drawing
only an ink mask in JavaScript. Source XML supplies verified shadow enable/offset/
color and ordered outline border color; sample ARGB is explicit. Eight named font
definitions reproduce captured command inputs, including active style fields.
Missing active-shadow color and oversized decimal style colors are unsupported.
Tests preserve the Outline-reset ordering. DefaultColor overflow is still unresolved
and is not used to choose a sample color or claim complete caption support.

## Source caption canvas integration

Supported GUID LABEL captions now use extracted English strings and font XML,
recovered text-part layout, integer owner alignment and native glyph rasters.
TEXTCOLOR forces alpha FF as the XML reader does. Horizontal alignment makes the
client width constrained; ordinary unaligned text uses the unbounded layout path.
Preview diagnostics expose unsupported substitutions, owner resizing, inline icons,
button captions, missing fonts and source ambiguity. This does not establish full
caption parity. The source Save game header reproduces an independent cached
capture's tokens, sizes, centered offsets and color; owner geometry and clipped
screenshot pixels remain separate gates. No cached text is used as preview input.

The selected localization document now supplies verified retained shadow color.
Font settings initialization zeros offsets, but does not reset scalar colors in the
temporary loader storage. Mainmenu Subheader Start therefore retains 15522472 from
the preceding font and resets offsets to 0/0. All 21 named fonts now reproduce source
command settings and 5,376 glyph records in each of two snapshots. A missing first
color remains unknown; the parser does not invent an initial stack value. Raw font
XML and source hashes remain intact, and font candidate context differences fail
explicitly rather than selecting a convenient result.

## Unicode pages

Static tracing confirms that codeunit high bytes select complete 256-glyph pages
through the same GDI builder. Pages 1 through FE are created lazily; page FF aborts
the draw. The standalone renderer now builds bounded BMP pages itself, including
curly punctuation and ellipses. The save dialog renders three source LABEL captions
and nine text sprites, compared with one caption and two sprites before this change.
Tests check mixed-page advance, page indexing, bounds and previous regressions.

The elevated capture attempt found no running Addon.exe. No game restart, input or
renderer calls were made. The read-only script can now traverse already-loaded
page metadata, but higher-page live metadata and screenshot parity remain pending.
Preview diagnostics state this distinction; source coverage is not a pixel gate.

## Button source captions (2026-10-10)

Inspected D3D650/D3D8D0 constructor defaults, D40140 XML loader, D3E2C0 visual-state refresh, D3FBA0 renderer and DC1E90/DC1A90 color conversion; recorded Ghidra plate first. Explicitly enabled source BUTTON captions now share the recovered native LABEL text path and receive preview state inputs. Normal, hover, checked/pressed and disabled choose their source colors. TEXTCOLOR_ENABLE=0 clears the explicit color even if TEXTSETTING contains one. Forced checked-hover retains previous color in the game and produces a history diagnostic when that color matters. Missing active source color and unresolved enable defaults are rejected.

The original exit_game_menu definition exercises source caption rendering and state-color selection. All 46 Rust and eight frontend tests pass. This extends implemented static behavior; it does not prove button caption geometry or pixels against a live capture. Owner resize, inline icons, overlays, runtime content and whole-dialog comparisons remain open.

## CURRENT_CITY_NAME callback source (2026-10-10)

RTTI identifies CTextTagProcessor callback at005154A0. Its full decompile fails; inspected the specific dispatch branch from Ghidra disassembly. Initializer005153D0 registers378 tags from0127D4E8; CURRENT_CITY_NAME is index0x50, branch005183D0. The callback looks up the game engine's last nonzero island (+E8), not cursor mirror012D54F8. It resolves local-player (+EC) name via0075C7B0, with additional empty-name fallback rules. Resolved UTF16 is inserted into the tokenizer's current scan position and rescanned.

Recorded plates first and filled the previously empty CGameEngine placeholder with verified partial fields90/E4/E8/EC. Corrected capture-text-scenario.ps1 to use this callback path, retain cursor mirrors as diagnostics, reject empty local names until fallback capture is implemented, and reread pointer/island/player for changes. PowerShell AST and embedded C# compile pass. No game writes/calls or restart. No fresh corrected live scenario witness exists yet; historical First Island pixel regression proves its supplied glyph string, not arbitrary callback resolution.

Next: independently capture the full fallback inputs, model name selection, and feed named scenario data through the actual streaming tokenizer before exposing runtime caption controls. Prefix/suffix and supplied markup must be preserved; whole-string replacement without tokenizer semantics is insufficient.

## Streaming city-name scenarios and full fallback inputs (2026-10-10)

Implemented bounded CURRENT_CITY_NAME insertion at the tokenizer cursor, with retained plain prefixes and rescanned value/suffix markup. Semicolon splitting rewrites/rescans the next tag. Tests exercise adjoining text, escapes, supplied FONT markup, empty values, ignored callback arguments, missing scenario and recursive insertion bounds. OriginalGUID198142 and original font/color plus the explicit independent First Island string reproduce the captured local parts, sizes, offsets, line rectangles and colors exactly. This does not supply or verify the resized owner rectangle.

city_name.rs follows0075C7B0 local/primary/secondary/localization fallback order with strictly greater scores, ties and invalid-player behavior. Schema3 capture now collects30 independent names,8 primary scores and30 secondary vector counts. Recorded partial CIsland array fields and vector-range contract; vector+8 identity remains unknown and is not consumed. Capture checks strings and pointer/island/player/GOP again, but remains non-atomic. PowerShell/C# compile passes; elevated attempt confirms Addon is closed and supplies no live witness.

Added explicit scenario control and JSON import to the Tauri preview, plus gui-audit --city-name-scenario and optional scenario file for --captions. Scenario data is separate from XML and never saved to source. Older cursor-mirror captures are rejected. INFOBAR name has RESIZEPARENTWINDOW=1 and remains unsupported until its owner-resize/layout lifecycle is modeled; the preview reports that limitation. All50 Rust and eight frontend tests pass. Next substantive fidelity step is that text-driven layout path, including anchor flags, background client border and parent fit effects.

Release and packaged smoke passed: manual city input and synthetic schema3 primary-score/tie selection through actual IPC. Release audit independently resolves original English fallbackGUID140038 as 'Uninhabited Island', with source hash provenance. Final schema3 PowerShell/C# compile passes. Regenerated duplicate executables removed; current editor and audit retained.

## Source-driven label resizing before widget layout (2026-10-10)

Recovered the lifecycle: GuiManager dirty-text drain D76180 runs before D77820 and GuiWnd_RelayoutTree. D8B680 obtains processed bounds via D43E80/DC4F20; D5D770 grows client size by active background borders and preserves both near edges or both far edges according to TextData bit1. Recorded Ghidra plates before implementation. DC69B0 accumulates processed bounds independently of glyph parts; CR appends prior lines, while final drain skips zero-width lines. The two independent text fixtures still give34 and33 exact supported blocks, now including cached processed sizes as comparison outputs. Mutating those sizes produces a mismatch.

Implemented unbounded source LABEL intrinsic measurement before layout. Original GUID198142, original Headlines/font/color and explicit First Island string resize the source width96 to90x22; raw relative [11,9,101,31] matches the historical snapshot exactly. The tracked compact fixture records its parent-capture hash and scope. No captured extents feed the layout engine. Border expansion, minimum client size, both-axis far anchoring and box/parent-fit ordering have synthetic regression coverage. These checks do not verify complete runtime ancestor positioning or generated content.

Geometry, captions, icons, progress and geometry comparison now share scenario/language inputs through Tauri. The CLI caption path also computes geometry after scenario import. Missing runtime values, unresolved background offsets, constrained resize history and non-LABEL lifecycles report diagnostics instead of guessed geometry. Whole-dialog parity, archive/compiler acceptance and broader fidelity gates remain open.

Final verification: all53 Rust tests and eight frontend tests pass; release build and hidden packaged WebView smoke pass. Original source city label measurement/relative rectangle and caption PNGs are exercised through actual IPC. Release CLI produces First Island with two native sprites. Removed five regenerated duplicate executables; current editor/audit retained. Manifest hashes updated; broad gates remain open.

## Explicit constrained text refresh scenarios (2026-10-10)

Recovered D8B680 width retention and its three callers: D76180 dirty drain passes0, fixed tooltip D89DB0 passes0, standard tooltip D89810 first assigns an extent then passes1. With wrap, mode0 keeps unclamped pre-refresh client width; mode1 uses measured width. Text layout itself clamps to PARENTWINDOWMAXWIDTH. Verified LABEL constructor default INT_MAX via D7BB90, reader DC2390, copy DC1AA0 and historical SetFont D8B1F0 (actually width clamp). Plates recorded first.

Implemented text_refresh inputs per widget with client_width and fit_wrapped_width. Source near position/height plus supplied client width define the explicit initial state; no automatic runtime history is inferred. The properties panel provides those controls; JSON import and CLI accept the same data. Shared runtime reaches every geometry-consuming preview. Critically, captions retain pre-refresh wrapping/alignment after owner resize instead of recalculating at the new width. Unknown IDs, invalid ranges and unknown input fields are rejected. Inputs are never saved into original XML or copies.

Original load_text_frame source296 width minus original8+8 background borders independently gives280. Explicit mode0 predicts processed259x34 and raw[36,30,332,80], matching the historical cached parts/bounds and offscreen raw rectangle. The compact tracked fixture records capture hash and narrow scope. Mode1 predicts259x34 client /275x50 outer with unchanged cached parts; that is static/synthetic branch coverage, not a live mode1 witness. Parent clamp test preserves original width280 even when layout is clamped250. Unbounded First Island and existing25-widget INFOBAR regression remain passing.

Read-only text capture now additionally records ParentMaxWidth and ProcessedSize. The verifier compares both cached TextData dimensions and processed bounds when available; altered processed outputs cause a mismatch. PowerShell AST and embedded C# compile pass. No fresh capture, game write, bridge change or restart. One initial parallel test run showed isolated text discrepancies; isolated checks and subsequent parallel runs passed without altered reference expectations or tolerances. Root cause is unverified; retain that concern if it recurs.

Final constrained verification: all55 Rust tests and eight frontend tests pass, release build passes, packaged smoke exits0 with keep/fit owner widths and identical cached caption parts through actual IPC. Release CLI renders12 loading caption parts/sprites from original source and separate scenario JSON. Current release/audit hashes updated; five duplicate executables removed. Whole-dialog/export gates remain open.

## Island-info controller positioning (2026-10-10)

Static inspection of0048C2D0 confirms horizontal centering in the parent client rectangle, followed by a rightward clamp against INFOBAR_WARES_PANEL's absolute right edge. The vertical centralinterfacetop sequence does not implement this controller. Recorded the Ghidra plate before findings/consumer changes. The historical width232 at viewport1440 predicts X604; the captured wares right497 leaves the clamp inactive. This is a narrow arithmetic witness, not a whole-panel parity claim.

The source OnRender executes UPDATE_INFOBAR_ISLAND_INFO before horizontal FITSIZETOCHILDREN. A controller implementation must retain native pass ordering and prior-frame geometry; recentering a newly fitted root would silently replace the first-frame behavior. Fertility and resource updates fill existing numbered source slots, rather than creating missing widget structures. Slot selection, icon GUIDs and formatted count strings require independent runtime scenario inputs.

Extracted the historical 57-widget subtree into fixtures/island-panel-geometry-live.json with original capture hash/timestamp and explicit output-only scope. Extended gui-audit --geometry with optional --text-scenario scenario.json (English) so the existing independent city-name/refresh inputs can accompany captured visibility. No captured extent is supplied as an input. This exposes the remaining controller/lifecycle differences before any implementation claims.

The independent audit currently reports12 visible mismatches and45 hidden nodes. Nine visible descendant raw rectangles already agree; all visible absolute positions still differ. The source one-pass root width is341 instead of captured232 because the centered fertility panel retains a109px offset while its parent fits. FERTILITIES_LIST also predicts height26 instead of captured41, exposing a separate inherited handler/lifecycle issue. These differences are explicit evidence against whole-panel accuracy. Release build and all55 Rust tests pass; the new CLI scenario path completes successfully against the historical fixture. Next: recover list height/handler behavior and model repeated native relayout/controller ordering without using captured extents.

## Mixed-case FITSIZETOCHILDREN correction (2026-10-10)

The separate list-height mismatch had a concrete cause:005FEB30 calls00B08440, whose byte comparator folds ASCII case (plus three CP1252 umlaut pairs). Earlier documentation incorrectly described exact keyword comparison. Original FERTILITIES_LIST spells its argument Horizontal, so the preview selected both-axis fit and shrank height41 to26. Corrected Ghidra plates first, then case-insensitive ASCII keyword/RIGHT matching in Rust. Original-source regression now reproduces raw [2,0,86,41] exactly; absolute comparison intentionally remains a mismatch while ancestor positioning is unsupported. A separate mixed-case right-anchor regression preserves height and the far edge. All57 Rust tests pass.

## Explicit retained layout passes (2026-10-11)

Added a scenario with explicit1..32 layout passes. Each pass starts with the previous pass's geometry, executes source handlers in source order, and reports the root rectangle and whether any raw widget rectangle changed. The scoped island-info controller runs before root FITSIZETOCHILDREN; its input requires an explicitly matching full-viewport parent and external wares right edge. It rejects another frame, changed viewport, border, variable edge or unsupported background offsets. It does not infer an iteration count or treat unchanged geometry as evidence about native show timing.

The Tauri properties inspector exposes pass count and the scoped controller inputs and renders the pass trace. JSON scenarios preserve the layout block on import/export. Synthetic tests verify first pass center from width489, second pass center from fitted width232, stabilization, a binding right clamp and invalid pass bounds. All14 layout/text-layout tests pass, as do Rust library/audit binary check and TypeScript noEmit. The archive-backed historical controller test was added but could not run under the current sandbox: the test process received Windows access-denied on extracted game data. A prior elevated review request hit an account execution quota and was not retried. Thus historical full-tree12-widget parity and the latest packaged Tauri build remain unverified. Existing pre-change packaged smoke is not evidence for this version.
