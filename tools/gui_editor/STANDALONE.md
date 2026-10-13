# Anno GUI Studio

Extract the ZIP and run `anno-gui-studio.exe` on Windows with Microsoft Edge
WebView2 installed. No repository, Python or Node installation is required.

On first launch:

1. Select your Anno 1404 Venice or History Edition game root (the folder containing
   `Addon.exe` or `Anno1404Addon.exe`).
2. Select your downloaded, unpacked RDA Explorer folder containing `RDAExplorer.dll`
   and its accompanying dependencies. Use a copy from a source you trust: the
   editor loads this reader to unpack your game resources.
3. Choose **Prepare and open editor**. Progress shows the archive being read.

Preparation reads original archives without writing to them. Temporary archive
copies, prepared resources, settings, logs and saved edits are stored under
`%LOCALAPPDATA%\AnnoGUIStudio`. Allow free space for extracted UI resources and a
temporary copy of the largest archive. Preparation currently scans the supported
base/addon data, patch, English and German archives.

The header shows the current game directory. Choose **Change…** to inspect or
change either selected folder, or prepare the installation again. Switching asks
you to save or discard unsaved edits. Each installation has separate working
copies, preserved when its resources are prepared again. A failed or cancelled
preparation keeps the previous installation selected.

The standalone setup accepts both Gold/Venice and History Edition executable names.
Both editions use the `addon/` and `maindata/` archive layout expected by resource
preparation. This confirms setup and archive discovery compatibility; runtime
rendering parity against History Edition has not been validated. Studio reads these
archives and does not launch or attach to the selected executable. Later launches
reuse prepared resources. Missing installations or changes to archive sizes/timestamps
return to setup. The editor's verified GUI behavior is based on Gold/Venice assets;
History-specific asset and rendering parity remains unverified. English and German
localization are supported where present in the selected archives. Existing preview
coverage and archive-precedence limitations still apply. Save copy does not install
edits into the game.

This distribution contains the editor only. It contains no game resources,
historical game captures or RDA Explorer binaries.
