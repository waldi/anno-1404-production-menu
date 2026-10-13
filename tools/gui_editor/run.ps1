$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'src-tauri/target/release/anno-gui-studio.exe'
if (-not (Test-Path $exe)) { throw 'Build the editor first with ./build.ps1' }
Push-Location $PSScriptRoot
try { & $exe } finally { Pop-Location }
