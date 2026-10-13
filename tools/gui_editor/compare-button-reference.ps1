# Reproduce the deliberately unresolved checked-button pixel gate.
# Reference crop uses cached absolute rectangle plus captured client origin.
# Coordinates remain subject to independent render-path verification.
param([ValidateSet('base','patch0','patch1')][string]$Candidate='base',[switch]$HeldOut,[switch]$VisibleHeldOut)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
if($HeldOut -and $VisibleHeldOut){throw 'Choose one held-out capture'}
$reference=Join-Path $PSScriptRoot $(if($VisibleHeldOut){'fixtures/force-north-checked-visible-heldout-live.png'}elseif($HeldOut){'fixtures/force-north-checked-heldout-live.png'}else{'fixtures/force-north-checked-live.png'})
$render=Join-Path $PSScriptRoot $(if($Candidate -eq 'base'){'.tmp/force-north-checked-render.png'}else{".tmp/force-north-$Candidate-render.png"})
$record=Get-Content (Join-Path $PSScriptRoot $(if($VisibleHeldOut){'fixtures/force-north-checked-visible-heldout-comparison.json'}elseif($HeldOut){'fixtures/force-north-checked-heldout-comparison.json'}else{'fixtures/force-north-checked-comparison.json'})) -Raw | ConvertFrom-Json
if($record.visible_chain -eq $false){@{schema=1;candidate=$Candidate;captured_utc=$record.captured_utc;status='excluded_hidden_ancestor';passed=$null;scope='Cached absolute rectangle is stale because the widget ancestor chain is hidden; no pixel comparison is eligible.'}|ConvertTo-Json;return}
$a=[Drawing.Bitmap]::new($render);$b=[Drawing.Bitmap]::new($reference)
try {
 if($a.Width -ne $b.Width -or $a.Height -ne $b.Height){throw 'Pixel gate requires matching dimensions; no resizing'}
 $count=0;$sum=0L;$maximum=0;$over=0
 for($y=0;$y -lt $a.Height;$y++){for($x=0;$x -lt $a.Width;$x++){
  $p=$a.GetPixel($x,$y);if($p.A -ne 255){continue};$q=$b.GetPixel($x,$y)
  $channels=@([Math]::Abs([int]$p.R-$q.R),[Math]::Abs([int]$p.G-$q.G),[Math]::Abs([int]$p.B-$q.B))
  $sum+=($channels|Measure-Object -Sum).Sum;$errorMax=($channels|Measure-Object -Maximum).Maximum
  $maximum=[Math]::Max($maximum,$errorMax);if($errorMax -gt 3){$over++};$count++
 }}
 if($count -eq 0){throw 'No opaque pixels; gate cannot run'}
 $result=@{schema=1;candidate=$Candidate;reference_sha256=(Get-FileHash -LiteralPath $reference).Hash.ToLowerInvariant();render_sha256=(Get-FileHash -LiteralPath $render).Hash.ToLowerInvariant();captured_utc=$record.captured_utc;binary_sha256=$record.binary_sha256;crop=$record.crop;opaque_pixels=$count;mean_rgb_error=$sum/($count*3);maximum_rgb_error=$maximum;pixels_over_3=$over;passed=($over -eq 0);scope='Opaque rendered pixels only; underlying artwork excluded. Cached capture crop coordinates still need independent verification. A mismatch must not be corrected by fitting an offset.'}
 $json=$result|ConvertTo-Json -Depth 5
 $outputName=if($VisibleHeldOut){".tmp/force-north-visible-heldout-$Candidate-comparison.json"}elseif($HeldOut){".tmp/force-north-heldout-$Candidate-comparison.json"}elseif($Candidate -eq 'base'){'.tmp/force-north-checked-comparison.json'}else{".tmp/force-north-$Candidate-comparison.json"}
 [IO.File]::WriteAllText((Join-Path $PSScriptRoot $outputName),$json)
 $json
}finally{$a.Dispose();$b.Dispose()}
