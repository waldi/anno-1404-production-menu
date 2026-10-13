# History engineering tools and x64 protocol verification. No game installation.
[CmdletBinding()]
param([string]$OutputDirectory, [switch]$Test, [switch]$ExperimentalNative)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msvc.ps1')
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $PSScriptRoot '..\..\.build\history-engineering' }
$output = Assert-AnnoRepoOutput $OutputDirectory
New-Item -ItemType Directory -Force -Path $output | Out-Null
$vcvars = Get-AnnoVcVars -Architecture x64
$companionSource = Join-Path $PSScriptRoot 'AnnoRECompanion.cpp'
Push-Location $output
try {
    cmd /d /c "`"$vcvars`" >nul && cl /nologo /W3 /O2 /EHsc /MT /DANNORE_HISTORY64 `"$companionSource`" /Fe:AnnoREHistoryReader.exe"
    if ($LASTEXITCODE -ne 0) { throw 'History x64 reader build failed' }
    if ((Get-AnnoPeMachine (Join-Path $output 'AnnoREHistoryReader.exe')) -ne 0x8664) { throw 'History reader did not build as AMD64' }
    Remove-Item -LiteralPath AnnoRECompanion.obj
} finally { Pop-Location }
if ($Test) {
    $testSource = Join-Path $PSScriptRoot 'test_ui.cpp'
    $identityTestSource = Join-Path $PSScriptRoot 'test_game_identity.cpp'
    Push-Location $output
    try {
        cmd /d /c "`"$vcvars`" >nul && cl /nologo /W3 /O2 /EHsc /MT /DANNORE_HISTORY64 `"$testSource`" /Fe:test_ui.exe && test_ui.exe && cl /nologo /W3 /O2 /EHsc /MT `"$identityTestSource`" /Fe:test_game_identity.exe && test_game_identity.exe"
        if ($LASTEXITCODE -ne 0) { throw 'History x64 transport/identity tests failed' }
        if ((Get-AnnoPeMachine (Join-Path $output 'test_ui.exe')) -ne 0x8664) { throw 'History tests did not build as AMD64' }
        Remove-Item -LiteralPath test_ui.exe, test_ui.obj, test_game_identity.exe, test_game_identity.obj
    } finally { Pop-Location }
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '..\research\history_probe.py') -Destination $output
if ($ExperimentalNative) {
    $candidate = Assert-AnnoRepoOutput (Join-Path $output 'experimental-native')
    New-Item -ItemType Directory -Path $candidate -Force | Out-Null
    $nativeSource = Join-Path $PSScriptRoot 'history_bridge.cpp'
    Push-Location $candidate
    try {
        cmd /d /c "`"$vcvars`" >nul && cl /nologo /W3 /O2 /EHsc /MT /LD `"$nativeSource`" /Fe:d3d9.dll /link user32.lib && cl /nologo /W3 /O2 /EHsc /MT /DANNORE_HISTORY64 /DANNORE_HISTORY_NATIVE_EXPERIMENT `"$companionSource`" /Fe:AnnoRECompanion.exe"
        if ($LASTEXITCODE -ne 0) { throw 'Experimental History native build failed' }
        foreach ($name in @('d3d9.dll', 'AnnoRECompanion.exe')) {
            if ((Get-AnnoPeMachine (Join-Path $candidate $name)) -ne 0x8664) { throw "Experimental $name is not AMD64" }
        }
        Remove-Item -LiteralPath history_bridge.obj, d3d9.lib, d3d9.exp, AnnoRECompanion.obj
    } finally { Pop-Location }
    foreach ($name in @('production_dialog_v5.dlg', 'production_button.dlg')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot "examples\$name") -Destination $candidate
    }
    'Experimental native activation requires deliberate installation and live validation. Rename this file to ANNORE_HISTORY_NATIVE.flag only for an authorized test.' |
        Set-Content -LiteralPath (Join-Path $candidate 'ANNORE_HISTORY_NATIVE.flag.example') -Encoding UTF8
    @'
Experimental History64 native panel, live-tested in History Edition.
This folder contains the native-panel assets and a separate AMD64 bridge.
The DLL remains inert until ANNORE_HISTORY_NATIVE.flag is created beside it.
The hook checks the game SHA256, exact prologue and suspended thread contexts.
Panel creation, layout and settlement selection were verified in a live save.
Install only while the game is closed. Delete the activation flag to disable
custom GUI handling on the next launch. Rates remain estimates, not measured flow.
'@ | Set-Content -LiteralPath (Join-Path $candidate 'README.txt') -Encoding UTF8
}
if ($ExperimentalNative) {
    Write-Host 'History x64 reader and experimental native panel staged; live interaction verified.'
} else {
    Write-Host 'History x64 engineering reader staged.'
}
