# Visual GUI editor plan — Tauri desktop application

Decision, 2026-10-09: build a compiled Windows desktop application in
`tools/gui_editor/` using **Tauri 2 + Rust + React/TypeScript + Vite**. No Python
runtime, Python sidecar or local HTTP service in the packaged editor. Tauri's
window uses WebView2 for the frontend; the filesystem and document logic live in
Rust. The existing Python explorer remains an investigation reference, with its
corpus expectations and test cases ported to Rust rather than called at runtime.

This plan is not an implementation or a toolchain installation request.
The feasibility evidence remains in `docs/gui-explorer-feasibility.md` and
`docs/findings.json`; proposed behavior below is not a confirmed engine finding.

## First useful release

Open an extracted dialog, inspect its template inheritance, see a layout preview,
select a widget, move/resize it or change a property, undo the change, then save
and reopen a separate working copy. Export a source diff with clear preview
limitations. Game installation and archive packaging are separate later work.

Window layout:

- Left: searchable definition browser and widget hierarchy.
- Center: zoomable preview, selection handles, rulers/grid and viewport dimensions.
- Right: typed properties, local/inherited provenance and template navigation.
- Bottom: source diff, validation messages and unsupported-preview details.
- Asset picker: existing backgrounds with PNG previews/state metadata; icons as
  GUID/mapping entries until their pixel lookup is verified.

## Architecture and boundaries

Rust owns the catalog, source documents, template lookup, project workspaces,
edit commands, undo/redo, validation and disk writes. Separate these in a core
crate so they can be tested without a Tauri window.

Use two document views: original XML bytes with node/token spans for editing,
and a semantic widget/property model for inspection and preview. Preserve
comments, attributes, unknown elements, duplicate nodes, element order, encoding,
BOM and unchanged byte ranges. Evaluate candidate XML libraries by these needs;
do not assume a normal DOM serializer is lossless. Begin with scalar replacement
and insertion of a local property; subtree operations come after preservation tests.

Keep stable node IDs separate from PATHID/ACCESSID: names can be empty or repeated.
Track document revisions so stale frontend edit requests cannot modify the wrong
node. Frontend sends structured edit commands, not arbitrary filesystem paths.
Rust returns an updated model/diff and owns the authoritative undo history.

Preview uses an explicit game-layout model and a Canvas 2D renderer with an
HTML property/tree UI. CSS layout does not determine the game's widget geometry.
Screen-to-game coordinate conversion accounts for pan, zoom and display scaling.
Share layout calculations with edit constraints through typed Rust results.

Editing an inherited property creates a local override by default. Editing its
template requires selecting the template document explicitly, with affected
definition references visible. Reset-to-inherited removes only the local override.
Do not flatten inherited values into saved XML.

Sources are read-only. User projects live under repo-local
`tools/gui_editor/workspaces/<project>/`, excluded from Git; output preserves logical
game paths. Copy definitions/templates on first edit, with a manifest containing
source roots, baseline hashes and changed files. Reject output paths inside the
extracted assets or game install, including paths redirected by links/junctions.
Use atomic saves and detect externally changed working files before overwriting.

Grant the Tauri window only the commands/dialog access it needs. Enforce path
boundaries in Rust as well as capabilities. Bundle the frontend locally; no remote
content, shell execution or game-process commands are needed for the MVP.

## Milestones and acceptance criteria

### 0. Buildable desktop shell

Verify MSVC/Windows SDK and WebView2; install Rust MSVC toolchain if authorized.
Pin Rust/frontend dependencies and lockfiles. Create the separate Tauri project,
with build/cache outputs kept inside the repository when practical.

Pass: development window and release executable both launch; packaged operation
does not need Python, Node, a browser tab or a development server. WebView2 remains
a runtime prerequisite. Start with the executable; installer packaging comes later.

### 1. Safe document core and desktop explorer

Port catalog/template resolution and asset-preview access. Add a project working
directory, original-byte retention, node IDs, typed edit commands and source diff.
Support changing a known scalar plus adding/removing a local override. Add undo/redo.

Pass: all known 1,873 GUI sources parse; 8,497 non-empty template references resolve;
no-op save is byte-identical; targeted edits leave unrelated bytes unchanged;
unknown-content fixtures survive; reopening a saved working copy preserves edits;
source hashes remain unchanged. Compare against the existing explorer's evidence.

### 2. Visual layout editing — MVP completion

Render FRAME/LABEL/BUTTON and explicit EXTENT geometry first, including parent
coordinates, z-order, clipping and hidden-element inspection. Add supported
BOXLAYOUT stacking and a CARDLAYOUT card selector. Show placeholders for runtime
text/GUIDs, unsupported widgets and unknown layout behavior. Start with background
PNG thumbnails; resizing those is illustrative and must be labeled approximate.

Add selection, drag, resize, numeric geometry editing, grid snapping, keyboard
nudges and undo/redo. BOXLAYOUT controls child position: disable free positioning
there and expose size, spacing and order as the appropriate edits. Implement
reordering only once subtree preservation is covered by tests. Alignment-managed
positions need equivalent constraints. Keep preview visibility/card overrides
separate from authored HIDDEN/current-card values.

Pass: select/move/resize a sample dialog, inspect inherited versus local values,
save, close and reopen with matching authored geometry and XML diff. Verify this
for one freely positioned dialog and one stacked layout. Production-dialog examples
may be opened read-only or copied as fixtures; ongoing originals remain untouched.

### 3. Improve preview fidelity

Decode DDS texture atlases in Rust. Verify source rectangles, icon index order and
variation mapping. Correlate `.bkg` tile flags, client offsets, stretch/repeat,
states, overlays and masks against Ghidra and existing screenshots. Implement only
verified behavior and retain explicit placeholders for the rest.

Resolve per-type property/default semantics and add localized text/font support.
Use temporary sample text for runtime-only labels without saving it into the source.
Add other widget types based on real needs; videos/3D render targets can stay placeholders.

Pass: compare a representative fixed-size background, resized frame, button states
and progress-bar layers with reference images. Record confidence in findings.json;
put new binary behavior in Ghidra first. Accurate rendering work does not block MVP.

### 4. Source export and game acceptance

Export changed XML at original logical paths into a separate mod staging folder,
with dependency manifest and diff. Verify archive precedence, RDA writer integrity
and `/compilegui` input selection using a disposable archive and unique marker.
Plan any required game session separately. Installation is an explicit operation,
never a consequence of Save or Preview.

Pass: the changed archived source reaches compiler output and loads as intended
in a controlled test. Keep `.cdf` opaque until its format is independently decoded;
the planned editor does not require a `.cdf` writer. `.bkg` authoring can follow once
its renderer and preservation contract are verified.

## Validation and priorities

Rust tests: corpus parsing, template cycles/ambiguity, edit/undo/redo, byte preservation,
empty-versus-absent properties, safe output paths and external-change conflicts.
Layout tests: parent-relative geometry, inherited EXTENT, stacking, clipping,
z-order and zoomed coordinate conversion. Frontend tests: selection/inspector sync,
local overrides, keyboard edits and dirty-state handling. Finish each milestone
with a release build and a desktop interaction check.

Work order: **build shell → safe saves → visual layout editing → rendering fidelity
→ archive delivery**. Do not make exact fonts/backgrounds or archive recompilation
prerequisites for a useful first editor. Event handlers remain inspectable XML in
the MVP; an action editor and game execution are later features.

## Environment observations and references

### Implementation update — 2026-10-09

The first compiled prototype is implemented in `tools/gui_editor/`. Rust is now
installed (1.99.0, x86_64 MSVC). Release build and hidden native WebView/IPC smoke
test passed. The prototype includes catalog/hierarchy/template inspection,
approximate Canvas layout with move/resize, scalar/local-override edits, undo/redo,
separate working-copy saves, project hash manifest and saved template resolution.
Sources/game/production-dialog files are not written by the editor.

The Rust core is a testable library in the Tauri package; preview layout currently
lives in a separately tested TypeScript module. UTF-8 byte-span edits preserve
unchanged content; other XML encodings are not implemented. Source comparison is
an original/current view rather than a full unified diff. One default working
project is supported. Accurate assets/defaults, structural edits, richer diffs,
project selection and game/archive export remain later milestones. The plan above
describes the intended fuller tool, not a claim those features are complete.

Read-only checks on 2026-10-09 found Node 22.23.3, npm 10.9.9, Visual Studio 2022
Community's vcvars64.bat and a WebView2 application directory. Rust/cargo were not
on PATH or in the usual user `.cargo/bin` directory; absence elsewhere is not proven.
MSVC workload completeness and an actual Tauri build are still unverified.
Installing outside the repo requires Waldi's authorization under AGENTS.md.
Nothing was installed or modified outside the repo while making this plan.

Official references:

- [Tauri](https://tauri.app/) — Rust application logic and independent frontend.
- [Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows) —
  MSVC build tools, Rust and WebView2.
- [Capabilities](https://v2.tauri.app/security/capabilities/) — scoped window permissions.
