# License text provenance

These fallback texts fill omissions in published crates; dependency-notices.py uses installed package license files first.

- alloc-stdlib.txt: dropbox/rust-alloc-no-stdlib commit 0a81fd6928ea3b33c8cd484aa4575d50ffb98012, LICENSE.
- defmt-parser.txt: knurling-rs/defmt commit 4a8cdb44891ed57b8ff5a023b6bec7137c48708f, LICENSE-MIT (chosen from MIT OR Apache-2.0).
- webview2-com.txt: wravery/webview2-rs commit edc2caf886175ccaebe86078c9cfe1ae2a187328, LICENSE; applies to webview2-com, its macros and sys crates.
- selectors.txt: https://www.mozilla.org/media/MPL/2.0/index.txt, MPL-2.0. Corresponding source is the selectors crate identified in Cargo.lock, from servo/stylo commit 572ecba2d1600e7c3d490586692a209faf703baa.

The generated package notice enumerates the exact dependency versions and includes their license texts. Project GPL licensing does not replace those notices.
