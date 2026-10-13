# Production Menu

Experimental in-game production overview for Anno 1404 Venice. It displays settlement populations and production estimates using the game's own GUI.

| Package | Supported executable | Architecture |
| --- | --- | --- |
| GOG Gold / Venice | Addon.exe 2.01.5010 | x86 |
| Ubisoft History / Venice | Anno1404Addon.exe 4.0.4253.0 | x64 |

The DLL checks the known executable fingerprint before using edition-specific hooks. Unknown builds are rejected. Use DirectX 9 mode. Production rates are estimates, not measured deliveries; full warehouses and unavailable inputs can prevent delivery. Both panels remain experimental pending broader gameplay and lifecycle regression.

## Install and remove

Download the matching edition ZIP from [Releases](https://github.com/waldi/anno-1404-production-menu/releases). Close the game. Back up an existing d3d9.dll before copying files; another proxy/mod may already use that filename. Copy d3d9.dll, AnnoRECompanion.exe and the two .dlg files next to the supported executable. For History, also copy ANNORE_HISTORY_NATIVE.flag. Start the game in DirectX 9 mode; Ctrl+Q toggles Production.

To uninstall, close the game, remove only the files installed by this package, and restore any original proxy backup. Deleting the History activation flag disables custom GUI handling on the next launch.

## Build

From the repository root, with Visual Studio MSVC x86/x64 tools installed:

```powershell
./tools/build-release.ps1 -Test -RequireHistorySupport -CreateArchives
```

Outputs are under dist/production-calculator. Builds do not install files or launch the game. See [release development](../../docs/releasing.md).

## Ownership

Native C++ sources and headers implement edition-specific hooks/readers and shared presentation/transport. examples/ contains authored dialogs; only production_dialog_v5.dlg and production_button.dlg ship. dev/ contains experimental bridge commands, a legacy Python UI, capture/input helpers, palette maintenance and a restart/install helper. These helpers may write game memory, inject input or restart the repo-local game; they are not user distribution files. Shared live inspectors are in ../research/.

Historical bridge experiments and old limitations are preserved in [native bridge history](../../docs/research/native-bridge-history.md), not current installation guidance. Transport identifiers and the annobridge.cpp source filename are retained for compatibility.
