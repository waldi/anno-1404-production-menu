# Read-only inventory of variants of the checked-button atlas. No extraction or precedence guess.
param([string[]]$Archives=@('maindata/Data2.rda','maindata/patch0.rda','maindata/patch1.rda','maindata/patch2.rda','addon/patch0.rda'))
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Add-Type -Path (Join-Path $repo 'rda-explorer/RDAExplorer.dll')
$results=[Collections.Generic.List[object]]::new()
$copyRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '.tmp/archive-read-copies'))
[void][IO.Directory]::CreateDirectory($copyRoot)
function Visit-Atlas($folder,$archive){
 foreach($file in $folder.Files){
  $logical=$file.FileName.Replace('\','/').TrimStart('/')
  if($logical -notmatch '^data/graphics/ui/textures/ingame_gui_map_03(?:_[0-9]+)?\.(dds|png|abm)$'){continue}
  $stream=$file.BinaryFile.BaseStream
  if($stream -is [IO.FileStream] -and $stream.Name -ne $script:archiveCopy){throw 'Reader escaped disposable copy'}
  $bytes=$file.GetData();$dimensions=$null;$format=$null
  if($bytes.Length -ge 128 -and [Text.Encoding]::ASCII.GetString($bytes,0,4) -eq 'DDS '){$dimensions=@([BitConverter]::ToInt32($bytes,16),[BitConverter]::ToInt32($bytes,12));$format=[Text.Encoding]::ASCII.GetString($bytes,84,4)}
  $results.Add([pscustomobject]@{archive=$archive.ToLowerInvariant();logical=$logical;bytes=$bytes.Length;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant();dimensions=$dimensions;format=$format})
 }
 foreach($child in $folder.Folders){Visit-Atlas $child $archive}
}
foreach($archive in $Archives){
 if($archive -notmatch '^(addon|maindata)/(Data[0-9]+|patch[0-9]+)\.rda$'){throw 'Use a repo-local game archive'}
 $reader=[RDAExplorer.RDAReader]::new();$script:archiveCopy=Join-Path $copyRoot ([Guid]::NewGuid().ToString()+'.rda')
 try {
  $source=[IO.File]::Open((Join-Path (Join-Path $repo 'Anno 1404 Gold Edition') $archive),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::ReadWrite)
  try{$destination=[IO.File]::Open($script:archiveCopy,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$source.CopyTo($destination)}finally{$destination.Dispose()}}finally{$source.Dispose()}
  $reader.FileName=$script:archiveCopy;$reader.ReadRDAFile();Visit-Atlas $reader.rdaFolder $archive
 }finally{$reader.Dispose();$resolvedCopy=[IO.Path]::GetFullPath($script:archiveCopy);if(-not $resolvedCopy.StartsWith($copyRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Cleanup escaped temporary copy directory'};if(Test-Path -LiteralPath $resolvedCopy){Remove-Item -LiteralPath $resolvedCopy -Force}}
}
$record=@{schema=1;checked_utc=[DateTime]::UtcNow.ToString('o');archives=$Archives;variants=@($results.ToArray());scope='Selected archive inventory only; matching hashes prove bytes in those archives, not effective archive precedence or GPU content'}
$json=$record|ConvertTo-Json -Depth 8
[IO.File]::WriteAllText((Join-Path $PSScriptRoot '.tmp/checked-atlas-archive-inventory.json'),$json)
$json
