param([Parameter(Mandatory)][string]$Game, [Parameter(Mandatory)][string]$Explorer, [Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
# This helper runs in Windows PowerShell so the downloaded .NET Framework reader can load.
$root = [IO.Path]::GetFullPath($Output)
$readerRoot = Join-Path $root 'reader'
[IO.Directory]::CreateDirectory($readerRoot) | Out-Null
# Load app-local byte copies: downloaded assemblies can retain a blocked-file
# marker which .NET Framework refuses. Do not unblock or modify the user's files.
foreach ($library in Get-ChildItem -LiteralPath $Explorer -Filter '*.dll' -File) {
    [IO.File]::WriteAllBytes((Join-Path $readerRoot $library.Name), [IO.File]::ReadAllBytes($library.FullName))
}
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ReaderNativeSearch {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    public static extern bool SetDllDirectory(string directory);
}
'@
if (-not [ReaderNativeSearch]::SetDllDirectory($readerRoot)) { throw 'Could not select the RDA Explorer native library directory' }
Add-Type -Path (Join-Path $readerRoot 'RDAExplorer.dll')
$manifest = [Collections.Generic.List[object]]::new()
$scratch = Join-Path $root 'archive-copy.rda'
function Visit($folder, $archive) {
    foreach ($file in $folder.Files) {
        $logical = $file.FileName.Replace('\','/').TrimStart('/')
        # Only editor resources; no full-game extraction.
        if ($logical -notmatch '^(data|addondata)/(config/gui/.+\.(dlg|tpl|stp|bkg|xml)|config/game/icons\.xml|graphics/ui/.+\.(dds|abm)|loca/(eng|ger)/.+\.(xml|txt|ttf|otf|dds|abm))$') { continue }
        if ($logical -match '(^|/)\.\.?(/|$)' -or $logical.Contains(':')) { throw 'Unsafe archive entry' }
        $target = [IO.Path]::GetFullPath((Join-Path (Join-Path $root ('data/' + $archive)) $logical))
        if (-not $target.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Archive entry escapes preparation folder' }
        $bytes = $file.GetData()
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target)) | Out-Null
        [IO.File]::WriteAllBytes($target, $bytes)
        $manifest.Add([pscustomobject]@{archive=$archive;logical=$logical;bytes=$bytes.Length})
    }
    foreach ($child in $folder.Folders) { Visit $child $archive }
}
try {
    foreach ($group in @('maindata','addon')) {
        foreach ($source in Get-ChildItem -LiteralPath (Join-Path $Game $group) -Filter '*.rda' -File | Sort-Object Name) {
            if ($source.Name -notmatch '^(data[0-9]+|patch[0-9]+|eng[0-9]+|ger[0-9]+)\.rda$') { continue }
            $archive = ($group + '/' + $source.Name).ToLowerInvariant()
            [IO.File]::WriteAllText((Join-Path $root 'progress.txt'), "Reading $archive")
            # The reader requests write access. Never give it the original installation file.
            $inputStream = [IO.File]::Open($source.FullName, 'Open', 'Read', 'ReadWrite')
            try {
                $copy = [IO.File]::Open($scratch, 'Create', 'Write', 'None')
                try { $inputStream.CopyTo($copy) } finally { $copy.Dispose() }
            } finally { $inputStream.Dispose() }
            $reader = [RDAExplorer.RDAReader]::new()
            try { $reader.FileName=$scratch; $reader.ReadRDAFile(); Visit $reader.rdaFolder $archive }
            finally { $reader.Dispose() }
            Remove-Item -LiteralPath $scratch -Force
        }
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'assets.json') -Encoding UTF8
    [IO.Directory]::CreateDirectory((Join-Path $root 'tools/gui_editor/fixtures/archive-candidates')) | Out-Null
} finally {
    if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Force }
}
