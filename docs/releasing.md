# Building and releasing

Both products start at 0.1.0 and remain experimental. CI tests/packages source without a game installation. It does not establish live gameplay compatibility.

## Requirements and local commands

Production Menu: Windows and Visual Studio MSVC x86/x64 build tools. GUI Studio: MSVC, Node from .node-version, Rust from tools/gui_editor/rust-toolchain.toml, and WebView2 to run the editor. Install these prerequisites before building. Keep build output repo-local.

```powershell
./tools/ci.ps1 -Product research
./tools/ci.ps1 -Product production-menu
./tools/ci.ps1 -Product gui-studio
```

Native packages land in dist/production-calculator; Studio packages land in dist/gui-studio. Each versioned ZIP contains an explicit payload, license/notices and a SHA256 manifest. SHA256SUMS.txt covers the ZIPs. No build command installs into or starts a game.

Studio public tests are the default. Corpus, original-asset and captured-image fidelity checks are explicitly ignored in public CI. To run them on an authorized local installation with extracted data and captures:

```powershell
./tools/gui_editor/build.ps1 -Test -LocalFidelity
```

The local-fidelity feature includes a reference PNG that is intentionally excluded from Git. Keep local captures or regenerate them using the editor capture tools before running that suite. Ignored fidelity tests are not counted as passing public tests.

## Versions and tags

Update tools/release-versions.json. For Studio also update package.json/package-lock.json, src-tauri/Cargo.toml/Cargo.lock and tauri.conf.json. Keep versions equal. Review docs/releases/<product>.md and supported edition profiles. Commit the version and notes, then use the matching tag:

```powershell
git tag production-menu-v0.1.0
git push origin production-menu-v0.1.0
# Or independently:
git tag gui-studio-v0.1.0
git push origin gui-studio-v0.1.0
```

Tag workflows build and test the tagged source, upload exact ZIPs to workflow artifacts, then download those same assets for publishing. Only the publishing job gets contents:write. It verifies tag/version and ZIP checksums, creates a draft with all assets, then publishes it as a prerelease. A failure before publishing leaves no partial public release. Existing releases are never overwritten; delete an incomplete draft deliberately before retrying. Do not move a published tag.

Before the first public release, review application icon provenance and bundled dependency notices (THIRD_PARTY_NOTICES.md). Record both-edition live checks: open/close, settlement selection, filters/scrolling, new/load save, companion shutdown and unsupported-binary rejection. Stable releases require a separate status decision after broader gameplay/lifecycle checks.

## GitHub setup

Destination: https://github.com/waldi/anno-1404-production-menu (public). Use main as the default branch. Push the reviewed initial import, enable Actions, and run CI. Configure branch protection with Production Menu, GUI Studio and Source audit checks after those checks have appeared successfully. Release jobs use GITHUB_TOKEN; no personal access token is needed in workflow secrets. Signing is not configured.

CI currently runs all three jobs on pushes to main/master and pull requests. This deliberate first-import baseline avoids skipped required checks; add path-based job selection later if CI time warrants it. Dependabot maintains npm, Cargo and SHA-pinned action updates.
