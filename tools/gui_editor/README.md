# Build/release entrypoints

Use `../ci.ps1 -Product gui-studio` for the public test/build/package sequence. ZIPs are now in `../../dist/gui-studio/`. Local resource fidelity tests require `./build.ps1 -Test -LocalFidelity`; default tests exclude them explicitly. See [release instructions](../../docs/releasing.md).

# Anno GUI Studio — initial Tauri desktop editor

Rust backend + React/TypeScript frontend. The release executable launches without
repository assets. First launch asks for a Venice game directory and an unpacked
RDA Explorer folder, then prepares its own resources. Windows PowerShell and
WebView2 are required; Python, Node and a development server are not.

## Launch and distribution

Run `./run.ps1`, or launch `src-tauri/target/release/anno-gui-studio.exe` directly
after building. Use `./package.ps1` to produce `release/Anno-GUI-Studio.zip` with
only the executable and [standalone instructions](STANDALONE.md).

Normal runs store settings, prepared assets and per-installation working copies
under `%LOCALAPPDATA%/AnnoGUIStudio`. The header's **Game directory · Change…**
control opens settings for both selected folders. Preparation runs in a worker,
reports archive progress, and supports cancellation. It reads original archives
read-only and gives the downloaded reader disposable archive copies. Loading
local reader byte copies avoids downloaded-file blocking without changing the
selected RDA Explorer folder.
Extraction selects 32-bit or 64-bit Windows PowerShell from the native
`zlib.dll` PE architecture and pins native DLL lookup to the reader copies.
Successful preparation activates a new asset generation; failed/cancelled
preparation retains the previous active editor.
Archive size/timestamp changes invalidate the saved generation on restart.

Developer `--smoke-test` runs continue using repository fixtures and storage.
`--data-dir <absolute-folder>` selects an alternate application storage root.
`--setup-smoke-test --data-dir <repo-local-test-folder>` runs the packaged setup
UI, invalid-selection, real preparation, cancellation, and cached-startup checks
in a hidden window; its game/reader inputs come from the repository only in this
explicit test mode. Results are written to `setup-smoke.json` in the test folder.
The extraction helper is embedded in the release binary. Historical comparison
fixtures are not required or shipped; users can import their own captures.

## Current workflow

The background gallery renders thumbnails from `.bkg` definitions and extracted
DDS textures using the selected artwork profile. It shows the first defined
state at 240 × 120, loads visible cards lazily, and limits concurrent atlas
decodes. Pre-generated `.bkg.png` files are not required. Unsupported backgrounds
show their actual rendering error. Thumbnail caches are scoped to the current
gallery/installation and artwork selection.

1. Browse `.dlg` dialogs, `.tpl` templates or `.bkg` backgrounds; search filenames/paths.
   To inspect a definition outside the prepared game assets, choose a `.dlg`/`.tpl`
   file, or enter its repo-relative path when running from the source repository.
   The studio copies it into a private source snapshot and edits only the workspace
   copy, so repository and extracted sources stay unchanged.
2. Select a widget in the hierarchy or layout canvas. The inspector shows local and
   candidate inherited properties, template source links and ordered property layers.
3. Drag/resize freely positioned widgets, edit scalar/geometry properties, or use
   arrow keys to nudge (Shift = 10 px). Set snap spacing, zoom and preview viewport.
4. Undo/redo, reset individual local overrides, inspect source and compare original
   XML against the working version. Ctrl+Z / Ctrl+Shift+Z and Ctrl+S are supported.
5. Save copy writes only under the installation’s application-data workspace with the same relative asset
   paths and a source/working hash manifest. Reopening a definition loads its saved
   copy. Saved template copies participate in dependent property resolution.

Unsaved work prompts before switching definitions. Original extracted sources are
never overwritten. Unknown nodes, attributes, comments, line endings and UTF-8 BOM
remain byte-identical outside the edited spans. Repeated/complex scalar nodes are
inspection-only; there is no generic XML rewriting. UTF-16/other encodings are not
supported in this initial editor (the tested corpus is UTF-8). Disk changes to a
working copy or original source are detected before Save. Saves replace files
atomically; the document and project manifest are separate atomic writes, not a
multi-file transaction. Node IDs and revision checks prevent stale edit requests.

The working project is currently a single `default` project. This is a repository
tool, not a game installer: Save does not write RDA/CDF files or affect the running
game. Developer dependency/build and smoke-test cache directories remain inside
`tools/gui_editor/`. Normal WebView caches and working copies use application data.

## Preview limits

The native font inspector can build process-local GDI atlas masks. Two independent
live snapshots match all 5,632 page-zero glyph metadata records across 22 fonts.
The CLI glyph raster probe also has one screenshot regression: the Headlines island
name matches 370 fully opaque pixels exactly at its independently predicted origin.
These checks do not establish full caption rendering: translucent edges, fractional
filtering, complete source-to-caption resolution and whole-dialog parity remain open.
See `fixtures/glyph-headlines-witness.json` for that narrow witness and its exclusions.
The font inspector offers single-run native samples with outline and shadow from
the selected font XML and an explicit ARGB sample color. Eight source definitions
match captured native settings. Selected-document context now resolves retained
shadow color after a verified earlier assignment: all 21 named fonts reproduce
source-derived command settings and glyph metadata in two captures. Unknown initial
shadow color remains unsupported; oversized source default colors are not guessed.
Native samples also support bounded BMP glyph pages through FE, including curly
punctuation and ellipses. Higher-page live metadata and pixel verification remain
pending; page FF and non-BMP input are rejected. The source save-dialog preview now
renders three LABEL captions and nine glyph-run sprites.

Source BUTTON captions use the recovered normal, hover, checked/pressed and disabled
text colors and explicit caption/color enables. Missing active colors and retained
forced-state history produce diagnostics. Button caption pixels remain unverified.
The preview's city-name scenario accepts an explicit resolved name or imports
`capture-text-scenario.ps1` schema3 inputs. The standalone resolver follows the
game's local-name/primary-score/secondary-count/localization fallback order; inserted
names are rescanned with surrounding source text and tags. Unbounded LABEL text
resizes its owner before widget layout, including active background client borders.
INFOBAR's First Island label matches the historical raw relative rectangle90x22
from original GUID/font inputs. Constrained resizing accepts an explicit pre-refresh
client width and keep/fit wrapped-width scenario in the selected LABEL's properties.
Automatic refresh history, runtime ancestor positioning, live fallback and full
pixel parity remain pending. Older cursor-mirror
schema1/2 captures are rejected.

The preview scenario import also accepts JSON such as the following for
`menudefinitions/load_text_frame.dlg` (widget `0/0`). Width280 comes from its original
outer width296 minus its background's two eight-pixel borders. The initial owner
uses authored position/height and the supplied client width. These inputs are
separate from XML and are not written to working copies.

```json
{"text_refresh":{"0/0":{"client_width":280,"fit_wrapped_width":false}}}
```

Normal wrapped refresh keeps the supplied width; fit selects the measured width.
Both preserve the text's original wrapping/alignment width after owner resize.
Unknown widget IDs and out-of-range widths are rejected. A historical relative
rectangle/part comparison passes for the loading label; full dialog pixels and the
provenance of a live refresh remain unverified.

- Rust supplies authoritative preview geometry, including recovered layout modes,
  client offsets, clipping escape, z-order and directional/multi-column box placement.
  Constructor defaults, frame screen alignment, pre/child/post ordering,
  fixed/automatic box wrapping and pure fit-to-children are supplied where verified.
  Captured visibility can be imported as a separate preview scenario. Complete engine parity remains
  unverified; the inspector reports unsupported behavior and asset gaps.
- Parent box/alignment-controlled positions cannot be freely dragged in the UI.
  Generated scrollbars, scaling, animation and type-specific/runtime layout overrides
  still need implementation and comparison evidence.
- Supported backgrounds now render from original DDS textures using recovered grid
  and tile rules. The dialog canvas uses explicit EmptyLayout; the background browser
  provides state/size controls and PNG export. It rejects unsupported layout flags,
  old-format icons and ambiguous texture candidates. ABM, all state-selection rules,
  effects and broad pixel parity remain unverified. DDS support currently covers
  uncompressed 32-bit RGB/alpha and DXT1/BC1, DXT3/BC2, DXT5/BC3.
- The icon inspector resolves an explicit addon GUID/variation and displays its atlas
  crop. Normal variation0 LABEL icons also draw using recovered unit-scale
  client/ICONSPACE/stretch/clip rules. Other widget renderers, runtime variation,
  tint/disabled/overlay effects and pixel parity remain open.
- Supported source GUID LABEL captions now draw extracted English text using native
  glyphs at unit scale, with recovered local layout and owner alignment. Guide names
  remain inspection overlays. Missing runtime substitutions, constrained text resize
  history, inline icon pixels and unsupported fonts/tags report diagnostics.
  The source-defined Save game header matches captured tokens, sizes, centered
  offsets and color; full source geometry/clipping/pixels remain unverified.
  Animation, videos, render targets and runtime controller updates need later work.
  The extracted text/font inspector shows raw English/German candidates and
  named-font XML; it preserves inline tags without interpreting them. English
  resources are staged here; other languages need their own extraction.
  Each named-font candidate has an explicit native GDI metric probe. Compare can
  import cached text-run captures from `capture-text-reference.ps1`; the checked-in
  121-run fixture matches recovered GDI measurements at unit scale. This verifies
  run dimensions. A separate placement comparator also verifies plain tokens and
  ordinary word wrapping on 22 captured blocks; glyph pixels and complete text
  layout remain open. New all-part captures also check supported inline icon/tag
  placement, font names and colors: 34 and 33 exact blocks in independent snapshots,
  zero mismatches, 39 unsupported in each. These use original text and retained
  parameters as inputs; captured parts are comparison outputs. Runtime substitutions,
  oversized splitting and owner origins remain open. Dialog captions remain placeholders.
- Inherited values remain candidates where per-type parser/default behavior is
  unverified. Templates contribute properties, not cloned widget children.

## Build and test

The Compare inspector imports schema-2 captures from `capture-reference.ps1` and
reports exact/mismatching geometry, hidden/offscreen nodes and extra runtime nodes.
Use captured visibility to supply controller visibility without copying captured
rectangles into the preview. The checked-in 1440x1080 infobar fixture matches 25
visible source widgets; this does not verify pixels or other dialogs.

`extract-preview-assets.ps1` extracts only referenced background textures from
repo-local archive copies, with source hashes and conflict checks. The compiled
RDA library opens read/write, so originals are copied through read-only streams
before the library sees them. `-Localisation -Archives addon/eng0.rda,maindata/eng0.rda`
also stages English localization resources. Generated manifests are under `.tmp/`.
Run `gui-audit.exe --background-coverage` for explicit-state decode/bounds coverage;
its count does not imply pixel or color/state fidelity.
Run `gui-audit.exe --text-metrics <capture.json>` for cached run-size comparisons.
Run `gui-audit.exe --text-flow <capture.json>` for independently predicted plain
single-line token sizes, offsets and line bounds. The Compare inspector accepts the
same capture. The 1440x1080 fixture has 13 exact direct-string labels and 60 explicitly
unsupported blocks; this does not verify rich tags, oversized-word splitting or glyph pixels.
Explicit English GUID resolution extends this to 22 exact placements and 51
unsupported blocks, with zero mismatches. Choose a language in the inspector or use
`gui-audit.exe --text-flow <capture.json> --language eng`; selected source paths and
hashes are reported. The capture's cached tokens are never used as the original text.

The global artwork selector defaults to the explicit known English startup archive
profile across locally available assets. Backgrounds, LABEL icons and progress layers
use the same selection and expose actual source paths. The 17-archive whitelist
census is recorded in `fixtures/known-startup-artwork-census.json`; large candidate
assets are staged separately and ignored by Git. Loose files, custom later mounts
and the running game's actual file origin remain unverified.
The capture helper reads existing LABEL text/font caches without calling game
functions. Use the elevated read-only path when sandbox process enumeration hides
the interactive game; captures are non-atomic and may include stale hidden runs.

Requirements: Node/npm, Rust MSVC toolchain, Visual Studio C++ build tools and
WebView2. This machine's toolchain was verified by a native release build.

```powershell
./build.ps1         # release executable
./build.ps1 -Dev    # native development window with Vite hot reload
./build.ps1 -Test   # frontend + Rust tests
```

On a fresh checkout, run `npm ci --cache .npm-cache` first if dependencies are absent;
the build script also does this automatically. It uses repo-local Cargo/npm/temp
caches. Lockfiles pin dependency resolution. The Rust shim is found through PATH
or the user's standard `.cargo/bin` directory.

Checks cover corpus/reference resolution, byte preservation and unknown XML,
validation, revision conflicts, undo/redo, atomic save/reopen, external disk edits,
parent-relative geometry, clipping, hidden children, box stacking, cards and zoom.

Native release smoke check:

```powershell
./src-tauri/target/release/anno-gui-studio.exe --smoke-test
```

This opens a hidden WebView, loads the actual bundled frontend, exercises real
Tauri IPC and checks rendered canvas/inspector controls, then exits. Result:
`.tmp/smoke.json`; startup log: `.tmp/startup.log`. It edits only in memory and
does not save extracted sources or user working copies. No game process is touched.

The Compare tab accepts matching native-resolution PNGs and shows overlay/difference
and error metrics. It never silently resizes images. Images remain in memory.

Read-only investigation helpers:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin gui-audit > .tmp/corpus-inventory.json
./capture-reference.ps1 -Name live-hud
./compare-background-reference.ps1
```

The capture helper requires access to the interactive game process and writes only
repo-local reference files. It uses OpenProcess read/query permissions, PrintWindow
and ReadProcessMemory; no bridge, input, game calls or suspension. Captures are
non-atomic and hidden/off-clip widget rectangles may be stale. The comparison helper
uses the existing production screenshot and smoke-render artifact; its selected
background strip is not a whole-dialog acceptance gate.

Next milestones and exact remaining gates are recorded in
`docs/gui-editor-fidelity-progress.md` and `docs/gui-editor-fidelity-plan.md`.

## Background gallery

Select .bkg or use Background gallery to browse searchable thumbnails. Previews load as you scroll. Click a card to open the existing background state renderer and source inspector.


## Button and progress scenarios

Select a BUTTON to choose hover, pressed, checked and enabled inputs. The selected
background variant also supplies client offsets for child layout. Compare accepts
read-only captures from capture-button-reference.ps1; gui-audit --button-states
runs the same comparison without the app. Captured indices never drive selection.

Select a PROGRESSBAR to override current/maximum for preview only. Recovered fill
and layer order are implemented at unit scale. Steady-state background history,
runtime animation and full pixel parity remain unverified. No scenario saves XML.

The background inspector has an Artwork candidate selector for staged base-game
patch0/patch1 textures. Candidates affect this background preview only; they do
not imply a global game archive-order rule or change XML. compare-button-reference.ps1
-Candidate patch1 reproduces the scoped opaque checked-button check; -HeldOut
excludes the later hidden minimap capture. inspect-texture-archives.ps1 inventories
selected archived atlas versions without modifying archives.

Geometry audits can also supply independent English text inputs:

```powershell
.\src-tauri\target\release\gui-audit.exe --geometry menudefinitions/infobar_island_info.dlg 1440 1080 fixtures/island-panel-geometry-live.json --captured-visibility --text-scenario scenario.json
```

For example, `scenario.json` can contain `{"current_city_name":"First Island","layout":{"passes":8,"island_info":{"parent_viewport":[1440,1080],"wares_right":430}}}`. Pass count is explicit and retained geometry is fed into the next pass. The optional island controller is accepted only for its borderless, fixed-edge source frame with an explicitly matching full-screen parent. `wares_right` is an independent scene input; do not copy the island panel's captured rectangle into it. Results report each pass and whether geometry stopped changing. Reaching an unchanged pass does not establish native show timing or slot contents. Visibility is separate; native rectangles remain comparison outputs.
