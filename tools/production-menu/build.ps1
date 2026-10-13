# Builds annobridge.cpp into a 32-bit d3d9.dll proxy.
# The game is 32-bit, so this MUST be built with the x86 toolset.

param([switch]$Test, [string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

. (Join-Path $here 'msvc.ps1')
$vcvars = Get-AnnoVcVars -Architecture x86
if (-not $OutputDirectory) { $OutputDirectory = $here }
$OutputDirectory = Assert-AnnoRepoOutput $OutputDirectory
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$source = Join-Path $here 'annobridge.cpp'
$companionSource = Join-Path $here 'AnnoRECompanion.cpp'
$testSource = Join-Path $here 'test_ui.cpp'
$identityTestSource = Join-Path $here 'test_game_identity.cpp'

Push-Location $OutputDirectory
try {
    cmd /d /c "`"$vcvars`" >nul && cl /nologo /W3 /O2 /MT /GS- /LD `"$source`" /Fe:d3d9.dll /link /SUBSYSTEM:WINDOWS user32.lib && cl /nologo /W3 /O2 /EHsc /MT /GS- `"$companionSource`" /Fe:AnnoRECompanion.exe"
    if ($LASTEXITCODE -ne 0) { throw "build failed" }
    if ($Test) {
        cmd /d /c "`"$vcvars`" >nul && cl /nologo /W3 /O2 /EHsc /MT `"$testSource`" /Fe:test_ui.exe && test_ui.exe"
        if ($LASTEXITCODE -ne 0) { throw "UI tests failed" }
        Remove-Item -LiteralPath test_ui.obj, test_ui.exe
        cmd /d /c "`"$vcvars`" >nul && cl /nologo /W3 /O2 /EHsc /MT `"$identityTestSource`" /Fe:test_game_identity.exe && test_game_identity.exe"
        if ($LASTEXITCODE -ne 0) { throw 'Game identity tests failed' }
        Remove-Item -LiteralPath test_game_identity.obj, test_game_identity.exe
    }
    Remove-Item -ErrorAction SilentlyContinue annobridge.obj, d3d9.exp, d3d9.lib, AnnoRECompanion.obj
    $dll = Join-Path $OutputDirectory 'd3d9.dll'
    $companion = Join-Path $OutputDirectory 'AnnoRECompanion.exe'
    if ((Get-AnnoPeMachine $dll) -ne 0x14C -or (Get-AnnoPeMachine $companion) -ne 0x14C) {
        throw 'GOG output must contain x86 PE binaries'
    }
    Write-Host "built $dll"
    Write-Host "built $companion"
    Write-Host "copy both next to Addon.exe, then run the game in DirectX 9 mode."
} finally {
    Pop-Location
}
