[CmdletBinding()]
param([switch]$Build, [string]$ExecutablePath)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $PSScriptRoot '../production-menu/msvc.ps1')
$version = (Get-Content (Join-Path $PSScriptRoot '../release-versions.json') -Raw | ConvertFrom-Json).'gui-studio'
foreach ($path in @('package.json', 'src-tauri/tauri.conf.json')) {
    if ((Get-Content (Join-Path $PSScriptRoot $path) -Raw | ConvertFrom-Json).version -ne $version) { throw "Version mismatch: $path" }
}
$cargo = Get-Content (Join-Path $PSScriptRoot 'src-tauri/Cargo.toml') -Raw
if ($cargo -notmatch ('(?m)^version = "' + [regex]::Escape($version) + '"\r?$')) { throw 'Cargo version mismatch' }
if ($Build) { & (Join-Path $PSScriptRoot 'build.ps1') }
$exe = if ($ExecutablePath) { $ExecutablePath } else { Join-Path $PSScriptRoot 'src-tauri/target/release/anno-gui-studio.exe' }
if (-not (Test-Path -LiteralPath $exe)) { throw 'Build first or use -Build' }
$output = Assert-AnnoRepoOutput (Join-Path $repo 'dist/gui-studio')
$stage = Assert-AnnoRepoOutput (Join-Path $repo ('.build/studio-' + [guid]::NewGuid().ToString('N')))
New-Item -ItemType Directory -Force -Path $stage, $output | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'anno-gui-studio.exe')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'STANDALONE.md') -Destination $stage
foreach ($name in @('LICENSE', 'THIRD_PARTY_NOTICES.md')) { Copy-Item -LiteralPath (Join-Path $repo $name) -Destination $stage }
python (Join-Path $PSScriptRoot 'dependency-notices.py') (Join-Path $stage 'THIRD_PARTY_LICENSES.txt')
if ($LASTEXITCODE) { throw 'Dependency notice collection failed' }
$commit = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { (& git -C $repo rev-parse --verify HEAD 2>$null) }
if (-not $commit) { $commit = 'uncommitted' }
$files = @(Get-ChildItem -LiteralPath $stage -File | Sort-Object Name | ForEach-Object {
    @{ name = $_.Name; size = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
})
@{ schemaVersion = 1; product = 'gui-studio'; version = $version; sourceCommit = $commit; architecture = 'x64'; files = $files } |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $stage 'package.json') -Encoding UTF8
& (Join-Path $PSScriptRoot '../verify-package.ps1') -Directory $stage
$zip = Assert-AnnoRepoOutput (Join-Path $output "Anno-GUI-Studio-$version-windows-x64.zip")
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
('{0}  {1}' -f (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant(), (Split-Path -Leaf $zip)) |
    Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
$stage = Assert-AnnoRepoOutput $stage
Remove-Item -LiteralPath $stage -Recurse -Force
Write-Output "Standalone package: $zip"
