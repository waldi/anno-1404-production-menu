[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('production-menu','gui-studio','research')][string]$Product)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    if ($Product -eq 'production-menu') {
        & (Join-Path $PSScriptRoot 'build-release.ps1') -Test -RequireHistorySupport -CreateArchives
    } elseif ($Product -eq 'gui-studio') {
        & (Join-Path $PSScriptRoot 'gui_editor/build.ps1') -Test
        & (Join-Path $PSScriptRoot 'gui_editor/package.ps1') -Build
    } else {
        python (Join-Path $PSScriptRoot 'audit-source.py')
        if ($LASTEXITCODE) { throw 'Source audit failed' }
    }
} finally { Pop-Location }
