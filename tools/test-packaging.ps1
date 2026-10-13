[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'production-menu/msvc.ps1')
$repo = Split-Path -Parent $PSScriptRoot
$scratch = Assert-AnnoRepoOutput (Join-Path $repo ('.build/package-check-' + [guid]::NewGuid().ToString('N')))
New-Item -ItemType Directory -Path $scratch | Out-Null
try {
    $archives = @(Get-ChildItem (Join-Path $repo 'dist/production-calculator') -Filter *.zip) + @(Get-ChildItem (Join-Path $repo 'dist/gui-studio') -Filter *.zip)
    if ($archives.Count -ne 3) { throw 'Expected three current release archives; remove obsolete versions deliberately before checking' }
    foreach ($archive in $archives) {
        $destination = Assert-AnnoRepoOutput (Join-Path $scratch $archive.BaseName)
        Expand-Archive -LiteralPath $archive.FullName -DestinationPath $destination
        & (Join-Path $PSScriptRoot 'verify-package.ps1') -Directory $destination
        $manifest = Get-Content (Join-Path $destination 'package.json') -Raw | ConvertFrom-Json
        $payload = Join-Path $destination $manifest.files[0].name
        $original = [IO.File]::ReadAllBytes($payload)
        [IO.File]::WriteAllText($payload, 'tampered')
        $rejected = $false
        try { & (Join-Path $PSScriptRoot 'verify-package.ps1') -Directory $destination } catch { $rejected = $true }
        if (-not $rejected) { throw 'Verifier accepted tampered payload' }
        [IO.File]::WriteAllBytes($payload, $original)
        'unexpected' | Set-Content (Join-Path $destination 'unexpected.txt')
        $rejected = $false
        try { & (Join-Path $PSScriptRoot 'verify-package.ps1') -Directory $destination } catch { $rejected = $true }
        if (-not $rejected) { throw 'Verifier accepted extra payload' }
    }
    Write-Host 'All three ZIPs verified; tampered and unexpected payloads rejected.'
} finally {
    $scratch = Assert-AnnoRepoOutput $scratch
    Remove-Item -LiteralPath $scratch -Recurse -Force
}
