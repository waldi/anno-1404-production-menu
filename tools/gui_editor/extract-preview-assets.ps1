param([string[]]$Archives = @('addon/Data2.rda', 'addon/Data3.rda', 'addon/Data4.rda', 'addon/patch0.rda'), [switch]$Localisation,[switch]$StageCandidates)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$outputRoot = [IO.Path]::GetFullPath((Join-Path $repo $(if($StageCandidates){'tools/gui_editor/fixtures/archive-candidates'}else{'data'})))
Add-Type -Path (Join-Path $repo 'rda-explorer/RDAExplorer.dll')
$wanted = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$gui = Join-Path $repo 'data/addon/data1.rda/addondata/config/gui'
Get-ChildItem -LiteralPath $gui -Recurse -Filter *.bkg -File | ForEach-Object {
    [xml]$definition = Get-Content -LiteralPath $_.FullName -Raw
    foreach ($texture in $definition.BackgroundLayout.m_localIconFileMap.i) {
        $name = ([string]$texture).Replace('\', '/')
        if ($name -match '^(data|addondata)/[a-zA-Z0-9_./-]+\.png$' -and $name -notmatch '(^|/)\.\.?(/|$)') {
            $stem = $name.Substring(0, $name.Length - 4)
            foreach ($suffix in @('.dds', '_0.dds', '.abm')) { [void]$wanted.Add($stem + $suffix) }
        }
    }
}
$manifest = [Collections.Generic.List[object]]::new()
foreach($mapPath in @('data/maindata/data1.rda/data/config/gui/iconfilemap.xml','data/addon/data1.rda/addondata/config/gui/iconfilemap.xml')){
    $mapFile=Join-Path $repo $mapPath;if(-not (Test-Path -LiteralPath $mapFile)){continue}
    [xml]$fileMap=Get-Content -LiteralPath $mapFile -Raw
    foreach($record in $fileMap.DocumentElement.ChildNodes){
        $name=([string]$record.IconFilename).Replace('\','/')
        if($name -match '^(data|addondata)/[a-zA-Z0-9_./-]+\.png$' -and $name -notmatch '(^|/)\.\.?(/|$)'){
            $stem=$name.Substring(0,$name.Length-4);foreach($suffix in @('.dds','_0.dds','.abm')){[void]$wanted.Add($stem+$suffix)}
        }
    }
}
$archiveReports=[Collections.Generic.List[object]]::new()
function Visit-Folder($folder, $archiveKey) {
    foreach ($file in $folder.Files) {
        $logical = $file.FileName.Replace('\', '/').TrimStart('/')
        $localeAsset = $Localisation -and ($logical -match '^(data|addondata)/loca/(eng|ger)/[a-zA-Z0-9_./-]+\.(xml|txt|ttf|otf)$') -and ($logical -notmatch '(^|/)\.\.?(/|$)')
        if (-not $wanted.Contains($logical) -and -not $localeAsset) { continue }
        $stream = $file.BinaryFile.BaseStream
        if ($stream -is [IO.FileStream] -and $stream.Name -ne $script:archiveCopy) { throw 'Library accessed a file outside its temporary archive copy' }
        $target = [IO.Path]::GetFullPath((Join-Path (Join-Path $outputRoot $archiveKey) $logical))
        if (-not $target.StartsWith($outputRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Extraction outside repo data root' }
        $bytes = $file.GetData()
        $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
        if (Test-Path -LiteralPath $target) {
            if ((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw "Existing extracted asset differs: $target" }
        } else {
            [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target))
            [IO.File]::WriteAllBytes($target, $bytes)
        }
        $manifest.Add([pscustomobject]@{archive=$archiveKey; logical=$logical; bytes=$bytes.Length; sha256=$hash})
    }
    foreach ($child in $folder.Folders) { Visit-Folder $child $archiveKey }
}
foreach ($archive in $Archives) {
    if ($archive -notmatch '^(addon|maindata)/(Data[0-9]+|patch[0-9]+|eng[0-9]+|ger[0-9]+)\.rda$') { throw 'Unsupported repo-local archive path' }
    $reader = [RDAExplorer.RDAReader]::new()
    $copyRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '.tmp/archive-read-copies'))
    [void][IO.Directory]::CreateDirectory($copyRoot)
    $script:archiveCopy = Join-Path $copyRoot ([Guid]::NewGuid().ToString() + '.rda')
    try {
        # The compiled reader opens files read/write. Give it only a disposable copy.
        $sourcePath = Join-Path (Join-Path $repo 'Anno 1404 Gold Edition') $archive
        $source = [IO.File]::Open($sourcePath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        try {
            $destination = [IO.File]::Open($script:archiveCopy, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            try { $source.CopyTo($destination) } finally { $destination.Dispose() }
        } finally { $source.Dispose() }
        $reader.FileName = $script:archiveCopy
        $savedConsoleOut=[Console]::Out;$readerLog=[IO.StringWriter]::new()
        try{[Console]::SetOut($readerLog);$reader.ReadRDAFile()}finally{[Console]::SetOut($savedConsoleOut)}
        $archiveReports.Add([pscustomobject]@{archive=$archive.ToLowerInvariant();reader_log=$readerLog.ToString()});$readerLog.Dispose()
        Visit-Folder $reader.rdaFolder $archive.ToLowerInvariant()
    } finally { $reader.Dispose(); if ($script:archiveCopy.StartsWith($copyRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $script:archiveCopy)) { Remove-Item -LiteralPath $script:archiveCopy -Force } }
}
$manifestPath = Join-Path $PSScriptRoot $(if($StageCandidates){'.tmp/candidate-asset-extraction.json'}elseif ($Localisation) {'.tmp/localisation-asset-extraction.json'} else {'.tmp/preview-asset-extraction.json'})
[void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($manifestPath))
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding utf8
if($StageCandidates){[IO.File]::WriteAllText((Join-Path $PSScriptRoot '.tmp/candidate-archive-census.json'),(@{schema=1;checked_utc=[DateTime]::UtcNow.ToString('o');archives=$Archives;reports=@($archiveReports.ToArray());assets=@($manifest.ToArray());scope='Whitelisted GUI background textures; archived metadata read through compiled RDAExplorer on disposable copies. Reader logs retained; this is not a census of all game assets or loose-file/custom mount precedence.'}|ConvertTo-Json -Depth 9))}
Write-Output "Verified/extracted $($manifest.Count) whitelisted assets; manifest: $manifestPath"
