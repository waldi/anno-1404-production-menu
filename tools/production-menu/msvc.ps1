# Shared compiler discovery for local machines and GitHub Windows runners.
function Get-AnnoVcVars {
    param([ValidateSet('x86', 'x64')][string]$Architecture)
    $vcName = if ($Architecture -eq 'x86') { 'vcvars32.bat' } else { 'vcvars64.bat' }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -ne 0) { throw 'vswhere failed' }
        if ($installation) {
            $candidate = Join-Path $installation "VC\Auxiliary\Build\$vcName"
            if (Test-Path -LiteralPath $candidate) { return $candidate }
        }
    }
    foreach ($year in @('2022', '2019')) {
        foreach ($edition in @('Community', 'Professional', 'Enterprise', 'BuildTools')) {
            foreach ($prefix in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
                $candidate = Join-Path $prefix "Microsoft Visual Studio\$year\$edition\VC\Auxiliary\Build\$vcName"
                if (Test-Path -LiteralPath $candidate) { return $candidate }
            }
        }
    }
    throw "$vcName not found; install the MSVC x86/x64 build tools"
}

function Assert-AnnoRepoOutput {
    param([string]$Path)
    $repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..')).TrimEnd('\')
    $absolute = [IO.Path]::GetFullPath($Path)
    if (-not $absolute.StartsWith($repo + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Build output must stay inside the repository: $absolute"
    }
    # Reparse points can redirect a lexically repo-local output outside it.
    $cursor = $absolute
    while ($cursor -and $cursor.Length -ge $repo.Length) {
        if (Test-Path -LiteralPath $cursor) {
            if ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Build output may not traverse a reparse point: $cursor"
            }
        }
        $cursor = Split-Path -Parent $cursor
    }
    return $absolute
}

function Get-AnnoPeMachine {
    param([string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw "Not a PE file: $Path" }
    $offset = [BitConverter]::ToInt32($bytes, 60)
    if ($offset -lt 0 -or $offset + 24 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $offset) -ne 0x4550) {
        throw "Invalid PE header: $Path"
    }
    return [BitConverter]::ToUInt16($bytes, $offset + 4)
}
