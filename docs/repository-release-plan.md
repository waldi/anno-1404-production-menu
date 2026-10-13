# Repository cleanup and GitHub release plan

Audit date: 2026-10-13. Implementation is underway: ownership folders and workflows now exist. See docs/releasing.md for current commands. The audit below describes the starting state; paths in historical examples were migrated.

## Findings

The repository contains two products and a shared research toolkit:

| Current location | Responsibility | Proposed location |
| --- | --- | --- |
| `tools/production-menu/` native C++ sources, headers, edition profiles, builds | In-game Production Menu, including its proxy DLL and external companion | `tools/production-menu/` |
| `tools/gui_editor/` React/Tauri sources, packaging and preparation | Standalone Anno GUI Studio | Keep this path in the first cleanup |
| `annomem.py`, `annoprod.py`, `annogui.py`, `annowatch.py` | Shared live research and inspection; not shipped with either product | `tools/research/` |
| `findpop.py`, `check_production_rates.py`, `history_probe.py`, `history_gui_probe.py` | Population, recipe and History research probes | `tools/research/` |
| `tools/build-release.ps1` | Production Menu edition orchestration | Keep as public entrypoint initially |
| `tools/production-menu/dev/send.py`, `prod_ui.py`, `restart.py`, `mouse.py`, `capture.ps1` | Native bridge development, legacy Python UI and live test helpers | `tools/production-menu/dev/` after dependency fixes |
| `tools/production-menu/dev/update_meter_palette.py` | Authored dialog maintenance | `tools/production-menu/dev/` |
| GUI Studio `capture-*.ps1`, `compare-*.ps1`, archive inspection/extraction | Editor fidelity evidence and development | Keep with GUI Studio; move to its `dev/` only in a later change |

The research readers import each other, and editor capture scripts use research readers. This is a shared toolkit, not code to duplicate into both products. The Python `prod_ui.py` is a development/legacy path; current native release packaging ships `AnnoRECompanion.exe` instead.

Specific cleanup and release gaps:

- Git has no commits, tracked files or remote. This is preparation for an initial import, not cleanup of existing published history.
- No `.github/` workflows or project license were found.
- Root README describes a read-only population companion and says nothing writes to the game. That contradicts the native GUI hook and writes. AGENTS also needs its project scope and renamed paths updated, while retaining its safety rules.
- The native README combines prototype history, commands and stale limitations. Separate current user instructions from historical engineering notes.
- `next.md` is an old session handoff with obsolete operational instructions. Preserve it under dated research notes rather than presenting it as the current next step.
- `edition-profiles.json` currently enables both native panels as experimental. The later compatibility documentation records a successful History panel test; the top packaging entry in `findings.json` still describes the earlier incomplete port. Reconcile status from recorded evidence without upgrading confidence beyond it.
- Native builds already discover MSVC, enforce x86/x64 PE architecture, run transport/presentation and identity tests, use fresh staging directories, and produce per-file SHA256 manifests. Extend these foundations rather than replace them.
- GUI Studio has committed-candidate npm and Cargo lockfiles. Its package script uses an explicit EXE/document allowlist, but writes to `tools/gui_editor/release/`, which is not currently ignored.
- Generated Tauri schemas under `src-tauri/gen/schemas/` are visible as import candidates. Decide which generated outputs are reproducible, then ignore them.
- Some Rust tests load ignored `data/` resources, original textures/localization/fonts and captured fixtures. A clean public checkout cannot run the complete current test suite as-is.
- Product folders mix authored runtime dialogs with research examples. Package only the two required production dialogs; retain other examples as development assets until their callers are checked.
- Fixtures include screenshots and game-derived glyph imagery. Review provenance before public import; moving them to another folder does not resolve redistribution questions.

## Target organization

```text
tools/
  production-menu/       native product, edition profiles, build scripts
    dev/                 bridge commands and local live testing
    examples/            current dialogs plus research examples initially
  gui_editor/            standalone GUI Studio and its own fixtures
  research/              shared Python readers and probes
  build-release.ps1      native packaging entrypoint
docs/
  findings.json          runtime contract and confidence
  repository-release-plan.md
  research/              dated audits and session handoffs, after link migration
.github/workflows/
  ci.yml
  release-production-menu.yml
  release-gui-studio.yml
.build/                  ignored intermediate output
dist/                    ignored staged packages and archives
```

Keep one repository, with independent product tags and releases. GUI Studio supports authoring and inspection; Production Menu is the native runtime product. They should not force each other to release or ship each other's dependencies.

## Implementation sequence

### 1. Prepare the public import

Rewrite the root README around both products, their supported editions, experimental status, installation links and development commands. Separate read-only research from the in-process GUI component accurately. Add contribution/build documentation and a tool inventory. Move the stale handoff to a dated archive and update links.

Choose a license for original code before public distribution. Inventory authored versus copied/generated assets, fixtures, icons and third-party notices. Keep game executables, archives, extracted resources, fonts, Ghidra databases, third-party tool checkouts and local session configuration excluded. Review the prospective import with `git status`, `git check-ignore` and an explicit candidate-file inventory, including size and credential scans. No blanket `git add .` before this review.

Acceptance: prospective tracked files are source, approved documentation/fixtures and lockfiles; README describes actual products; license choice and asset provenance are resolved.

### 2. Rename and separate ownership

Rename `tools/production-menu` to `tools/production-menu`. Initially keep its internal layout and `annobridge.cpp` filename to limit unrelated changes. Update active documentation, AGENTS, ignore rules, package scripts, editor import references and evidence reproduction commands. Preserve historical narratives as history with an explicit old-path note where appropriate.

Move the shared Python scripts together to `tools/research`; repair callers in editor capture scripts, native dev helpers and History packaging. Move native dev helpers only after fixing their imports, dialog paths and repo-root discovery. In particular, `prod_ui.py` and `restart.py` derive locations from their own directories, and `build-history.ps1` copies a sibling `history_probe.py`. `msvc.ps1` currently derives the repository root by going two levels up: preserve this assumption or replace it with explicit root discovery before relocating it further.

Do not rename shared-memory objects, protocol magic, log identities or shipped binary names as part of a folder rename. Those changes need separate compatibility reasoning. Do not delete a script merely because it appears old; document active callers or archive it.

Acceptance: source/path searches show no broken active old paths; both native architectures build and test; editor builds; research CLIs resolve imports; no game files are installed or game processes launched by cleanup checks.

### 3. Make clean-checkout builds reliable

Use explicit output directories under `.build/` and `dist/`. Preserve native edition gates and architecture checks. Add release version and commit identity to package metadata and versioned archive names. Validate each edition's `requiredFiles`, payload hashes, architecture, package status and activation flag. Ensure packaging cannot pick up stale binaries or unexpected files.

For GUI Studio, use `npm ci` and Cargo's locked dependency mode; pin the Rust toolchain and supported Node version. Move ZIP output to `dist/gui-studio/`. Align npm, Cargo and Tauri versions for a release. Split deterministic tests using authored/synthetic fixtures from local fidelity tests requiring licensed game resources. Make the latter an explicit local suite with a documented prerequisite, rather than silently counting skipped game-dependent checks as passed.

Acceptance: builds and public test suites succeed from a fresh checkout with no game install, extracted `data/`, RDA Explorer checkout or local caches. Full fidelity tests remain separately reproducible on the authorized local installation.

### 4. Add GitHub Actions CI

Use a fixed Windows runner image such as `windows-2022` and SHA-pinned actions. Trigger on pull requests and pushes to the chosen default branch, with path filtering per product and shared dependencies. Use `contents: read`, timeouts and concurrency cancellation for obsolete CI runs.

Native job: invoke the renamed packaging entrypoint with `-Test -RequireHistorySupport -CreateArchives`; verify manifests and ZIP allowlists; upload GOG x86 and History x64 archives as CI artifacts. GUI Studio job: install pinned Node/Rust, restore lockfile-keyed caches, run frontend and public Rust tests, build the Tauri executable and package its EXE plus standalone instructions. Research job: syntax/import checks that do not attach to a game, plus any existing deterministic checks suitable for CI.

No hosted job installs into or launches a game. CI verifies buildability, deterministic behavior and package contents; it does not establish live gameplay compatibility.

Acceptance: all jobs pass on a GitHub checkout, including a pull request; downloadable archives contain exactly the documented files.

### 5. Add product-specific releases

Use tags `production-menu-vX.Y.Z` and `gui-studio-vX.Y.Z`. Each release workflow checks tag/version agreement and runs the corresponding tested build at that tag. Upload the exact resulting ZIPs, SHA256 checksum file and release notes; do not rebuild different binaries in the publishing job. Native releases contain separate edition archives. GUI Studio releases contain the standalone Windows x64 archive.

Give only the publishing job `contents: write`. Use the repository's `GITHUB_TOKEN`; a personal access token is unnecessary for ordinary releases. Publish experimental native versions as prereleases. Stable status requires a recorded live regression checklist, not just a green compiler run. Include supported executable versions/fingerprints, DirectX 9 requirement, WebView2/editor prerequisites, installation/uninstallation and known limitations in release notes.

Acceptance: a test prerelease tag produces complete, versioned downloadable assets and checksums; failed builds publish nothing; reruns have a documented policy and cannot silently replace an existing stable release's artifacts.

### 6. First release and repository settings

After the source/license review, create the initial commit, choose the GitHub repository and default branch, configure its remote, and push source and workflows. Protect the default branch with the proven required checks. Run local gameplay regression separately: both supported editions, open/close, settlement selection, filters/scrolling, new/load save, companion shutdown and unsupported-binary rejection. Record remaining limits honestly.

Create the first product prerelease tags only after hosted CI succeeds. Publishing a repository or release is a separate action from this audit and plan.

## Decisions needed during implementation

- License and public repository owner/name/visibility.
- Initial product versions and whether GUI Studio is ready for its own prerelease.
- Which game-derived captures may be published, and which should remain local.
- Optional executable signing: a separate certificate/service decision; basic automated ZIP releases do not depend on it.

## GitHub references

- [Hosted Windows runner choices](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Workflow token permissions; contents write permits releases](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)
- [Workflow artifacts](https://docs.github.com/en/actions/concepts/workflows-and-actions/workflow-artifacts)
