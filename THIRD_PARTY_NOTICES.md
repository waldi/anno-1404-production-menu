# Licensing and resource provenance

Original project code is GPL-3.0-only; see LICENSE. Anno 1404 executables, archives, textures, fonts and other game assets remain the property of their respective owners. This license does not grant rights to those resources.

Release packages contain project binaries, authored dialog definitions, documentation and manifests. They do not include game archives, extracted assets, reference screenshots, fonts or RDA Explorer binaries. GUI Studio asks users to supply their own game installation and RDA Explorer dependencies.

Research JSON fixtures contain observed metadata, counts, geometry and extracted text used as technical evidence. Reference PNG captures remain local and are excluded from the public import. The test that embeds a captured PNG is compiled only with the local-fidelity feature. App icons under src-tauri/icons are project UI branding assets; review their provenance before publishing the first public release.

GUI Studio uses Tauri, React, Vite and Rust crates listed in package-lock.json and Cargo.lock. Those dependencies retain their own licenses; GPL does not replace them. Studio packages include THIRD_PARTY_LICENSES.txt, generated from installed runtime npm dependencies and Windows Cargo dependency packages. Missing license texts for alloc-stdlib, defmt-parser and webview2-com are vendored from the source revisions recorded in their published crates; selectors uses the official MPL-2.0 text. See tools/gui_editor/licenses/README.md.

Corresponding project source is available at https://github.com/waldi/anno-1404-production-menu. Each release is built from its product version tag, and package manifests identify the source commit. The tag archive includes source and build instructions. Local uncommitted packages are development output and should not be distributed as official releases.
