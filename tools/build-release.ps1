# Stage edition-specific folders; never installs or starts a game.
[CmdletBinding()]
param(
    [string]$OutputDirectory,
    [switch]$Test,
    [switch]$RequireHistorySupport,
    [switch]$CreateArchives,
    [switch]$ExperimentalHistoryNative
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$bridge = Join-Path $PSScriptRoot 'production-menu'
. (Join-Path $bridge 'msvc.ps1')
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repo 'dist\production-calculator' }
$output = Assert-AnnoRepoOutput $OutputDirectory
$version = (Get-Content (Join-Path $PSScriptRoot 'release-versions.json') -Raw | ConvertFrom-Json).'production-menu'
$commit = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { (& git -C $repo rev-parse --verify HEAD 2>$null) }
if (-not $commit) { $commit = 'uncommitted' }
$profiles = Get-Content -LiteralPath (Join-Path $bridge 'edition-profiles.json') -Raw | ConvertFrom-Json
if ($Test) {
    [xml]$dialog = Get-Content -LiteralPath (Join-Path $bridge 'examples\production_dialog_v5.dlg') -Raw
    $interactiveButtons = @(@(1..19 | ForEach-Object { 'ANNORE_ISLAND_ROW{0:D2}' -f $_ }) + @(0..3 | ForEach-Object { "FILTER$_" }))
    foreach ($pathId in $interactiveButtons) {
        $button = $dialog.SelectSingleNode("/FRAME/BUTTON[PATHID='$pathId']")
        $event = if ($button) { $button.SelectSingleNode('EVENTHANDLER/EVENT') }
        if (-not $event -or $event.InnerText -ne 'OnLeftClick') {
            throw "Dialog input test failed: $pathId must dispatch OnLeftClick."
        }
    }
    Write-Host "Dialog input test passed ($($interactiveButtons.Count) settlement/filter buttons)."
}
if (($RequireHistorySupport -or $CreateArchives) -and -not $profiles.'history-edition'.nativePanelAvailable) {
    throw 'History native panel is not implemented/validated. Refusing a two-edition release.'
}
# A fresh build directory prevents stale DLLs from being packaged as new output.
$buildRoot = Assert-AnnoRepoOutput (Join-Path $repo ('.build\release-' + [guid]::NewGuid().ToString('N')))
New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null
& (Join-Path $bridge 'build.ps1') -OutputDirectory (Join-Path $buildRoot 'gog') -Test:$Test
& (Join-Path $bridge 'build-history.ps1') -OutputDirectory (Join-Path $buildRoot 'history-edition') -Test:$Test -ExperimentalNative:($ExperimentalHistoryNative -or $profiles.'history-edition'.nativePanelAvailable)
foreach ($id in @('gog', 'history-edition')) {
    $profile = $profiles.$id
    $destination = Join-Path $output $id
    # Replace only this script's managed folders. Validate immediately before removal.
    $destination = Assert-AnnoRepoOutput $destination
    if (Test-Path -LiteralPath $destination) {
        $marker = Join-Path $destination 'package.json'
        if (-not (Test-Path -LiteralPath $marker)) { throw "Refusing to replace unmanaged folder: $destination" }
        $old = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
        if ($old.packageOwner -ne 'Anno1404-RE/build-release' -or $old.editionId -ne $id) {
            throw "Refusing to replace unrelated folder: $destination"
        }
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    if ($profile.nativePanelAvailable) {
        $runtime = Join-Path $buildRoot $id
        if ($id -eq 'history-edition') { $runtime = Join-Path $runtime 'experimental-native' }
        foreach ($name in @('d3d9.dll', 'AnnoRECompanion.exe')) {
            Copy-Item -LiteralPath (Join-Path $runtime $name) -Destination $destination
        }
        if ($id -eq 'history-edition') {
            Copy-Item -LiteralPath (Join-Path $runtime 'ANNORE_HISTORY_NATIVE.flag.example') -Destination (Join-Path $destination 'ANNORE_HISTORY_NATIVE.flag')
        }
        foreach ($name in @('production_dialog_v5.dlg', 'production_button.dlg')) {
            Copy-Item -LiteralPath (Join-Path $bridge "examples\$name") -Destination $destination
        }
        $readme = @"
Anno production calculator — $($profile.edition)
Experimental native panel for $($profile.gameExecutable) $($profile.gameVersion).
Close the game, then copy the runtime DLL, companion and dialog files next to $($profile.gameExecutable).
For History, include ANNORE_HISTORY_NATIVE.flag to activate its native bridge.
Use DirectX 9 mode. The DLL starts the companion; Ctrl+Q toggles Production.
Current/Potential rates are estimates, not measured deliveries.
These $($profile.architecture) files are specific to this edition and fingerprint.
"@
    } else {
        # Engineering output only: never include a GOG DLL or an inert fake History DLL.
        Copy-Item -LiteralPath (Join-Path $buildRoot 'history-edition\history_probe.py') -Destination $destination
        Copy-Item -LiteralPath (Join-Path $buildRoot 'history-edition\AnnoREHistoryReader.exe') -Destination $destination
        if ($ExperimentalHistoryNative) {
            Copy-Item -LiteralPath (Join-Path $buildRoot 'history-edition\experimental-native') -Destination $destination -Recurse
        }
        $readme = @"
History Edition — engineering preview, NOT a playable native panel release.
The x64 native GUI bridge is still being ported. A read-only reader is available.
There is deliberately no d3d9.dll or AnnoRECompanion.exe in this folder.
Run AnnoREHistoryReader.exe --parent-pid GAME_PID --snapshot to print production JSON.
It verifies the installed executable fingerprint and checks island/global population sums.
Without --snapshot it refuses native-panel mode. It does not modify the game or write logs.
history_probe.py is a read-only diagnostic requiring 64-bit Python, PID and loaded module base.
It checks the known executable SHA256 and prints production-property candidates.
Do not copy this folder into the game install. See LIMITATIONS.md.
Release CI must use -RequireHistorySupport before publishing a two-edition release.
"@
    }
    $readme | Set-Content -LiteralPath (Join-Path $destination 'README.txt') -Encoding UTF8
    foreach ($name in @('LICENSE', 'THIRD_PARTY_NOTICES.md')) { Copy-Item -LiteralPath (Join-Path $repo $name) -Destination $destination }
    $files = @(Get-ChildItem -LiteralPath $destination -File -Recurse | Sort-Object FullName | ForEach-Object {
        @{ name = $_.FullName.Substring($destination.Length + 1).Replace('\', '/'); size = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    $manifest = [ordered]@{
        schemaVersion = 1; packageOwner = 'Anno1404-RE/build-release'; editionId = $id
        version = $version; sourceCommit = $commit
        edition = $profile.edition; architecture = $profile.architecture
        status = $profile.status; nativePanelAvailable = $profile.nativePanelAvailable
        experimentalNativeCandidate = ($id -eq 'history-edition' -and $ExperimentalHistoryNative.IsPresent)
        gameExecutable = $profile.gameExecutable; gameVersion = $profile.gameVersion
        gameSha256 = $profile.gameSha256; files = $files
    }
    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $destination 'package.json') -Encoding UTF8
    & (Join-Path $PSScriptRoot 'verify-package.ps1') -Directory $destination
    Write-Host "Staged $id ($($profile.status)): $destination"
}
# Build-only scratch is disposable after successful staging.
$buildRoot = Assert-AnnoRepoOutput $buildRoot
Remove-Item -LiteralPath $buildRoot -Recurse -Force
if ($CreateArchives) {
    foreach ($id in @('gog', 'history-edition')) {
        $archive = Assert-AnnoRepoOutput (Join-Path $output "Production-Menu-$version-$id.zip")
        Compress-Archive -Path (Join-Path $output "$id\*") -DestinationPath $archive -Force
        Write-Host "Archived $archive"
    }
    $checksums = @(Get-ChildItem -LiteralPath $output -Filter "Production-Menu-$version-*.zip" | Sort-Object Name | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name })
    $checksums | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
}
Write-Host 'Output folders are ready for archival. Both native panels remain experimental pending broader gameplay and lifecycle validation.'
