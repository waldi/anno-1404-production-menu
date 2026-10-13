[CmdletBinding()]
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'production-menu/msvc.ps1')
$Directory = Assert-AnnoRepoOutput $Directory
$manifest = Get-Content -LiteralPath (Join-Path $Directory 'package.json') -Raw | ConvertFrom-Json
if ($manifest.schemaVersion -ne 1 -or -not $manifest.files.Count) { throw 'Invalid package manifest' }
$expected = @('package.json')
foreach ($file in $manifest.files) {
    if ($file.name -match '(^/|\\|:|(^|/)\.\.(/|$))') { throw "Unsafe manifest path: $($file.name)" }
    $path = Join-Path $Directory $file.name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing $path" }
    if ((Get-Item -LiteralPath $path).Length -ne $file.size -or
        (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256) { throw "Hash/size mismatch: $path" }
    $expected += $file.name
}
$actual = @(Get-ChildItem -LiteralPath $Directory -File -Recurse | ForEach-Object { $_.FullName.Substring($Directory.Length + 1).Replace('\', '/') })
if (@(Compare-Object ($expected | Sort-Object) ($actual | Sort-Object)).Count) { throw 'Package contains unexpected or missing files' }
if ($manifest.editionId) {
    $profiles = Get-Content (Join-Path $PSScriptRoot 'production-menu/edition-profiles.json') -Raw | ConvertFrom-Json
    $profile = $profiles.($manifest.editionId)
    if (-not $profile -or $manifest.gameSha256 -ne $profile.gameSha256 -or $manifest.architecture -ne $profile.architecture) { throw 'Edition identity mismatch' }
    if ($manifest.nativePanelAvailable) {
        $allowed = @($profile.requiredFiles) + @('README.txt', 'package.json', 'LICENSE', 'THIRD_PARTY_NOTICES.md')
        if (@(Compare-Object ($allowed | Sort-Object) ($actual | Sort-Object)).Count) { throw 'Native package allowlist mismatch' }
        $machine = if ($profile.architecture -eq 'x86') { 0x14C } else { 0x8664 }
        foreach ($name in @('d3d9.dll', 'AnnoRECompanion.exe')) {
            if ((Get-AnnoPeMachine (Join-Path $Directory $name)) -ne $machine) { throw "Wrong architecture: $name" }
        }
    }
} else {
    $allowed = @('anno-gui-studio.exe', 'STANDALONE.md', 'package.json', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_LICENSES.txt')
    if (@(Compare-Object ($allowed | Sort-Object) ($actual | Sort-Object)).Count) { throw 'Studio package allowlist mismatch' }
    if ((Get-AnnoPeMachine (Join-Path $Directory 'anno-gui-studio.exe')) -ne 0x8664) { throw 'Studio must be x64' }
}
Write-Host "Verified package: $Directory"
