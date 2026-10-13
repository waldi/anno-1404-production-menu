# Reproducible exploratory comparison of the archived production row's top strip.
# Foreground content is outside the selected 20px strip. This is not a full-row gate.
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$reference=Join-Path $repo 'production-objectmenuinside_full.png'
$render=Join-Path $PSScriptRoot '.tmp/objectmenuinside-render.png'
$game=[Drawing.Bitmap]::new($reference);$art=[Drawing.Bitmap]::new($render)
$composite=[Drawing.Bitmap]::new(1080,114);$graphics=[Drawing.Graphics]::FromImage($composite)
try {
 if($art.Width -ne 1080 -or $art.Height -ne 114){throw 'Run native smoke check to produce the 1080x114 render'}
 $graphics.Clear($game.GetPixel(100,600));$graphics.DrawImageUnscaled($art,0,0)
 $composite.Save((Join-Path $PSScriptRoot '.tmp/objectmenuinside-composite.png'))
 $crop=$game.Clone([Drawing.Rectangle]::new(333,211,1080,114),$game.PixelFormat)
 try {$crop.Save((Join-Path $PSScriptRoot '.tmp/objectmenuinside-reference-crop.png'))} finally {$crop.Dispose()}
 $total=0;$maximum=0;$changed=0
 for($y=0;$y -lt 20;$y++){for($x=0;$x -lt 1080;$x++){
  $a=$composite.GetPixel($x,$y);$b=$game.GetPixel($x+333,$y+211)
  $d=@([Math]::Abs([int]$a.R-$b.R),[Math]::Abs([int]$a.G-$b.G),[Math]::Abs([int]$a.B-$b.B))
  $total+=($d|Measure-Object -Sum).Sum;$m=($d|Measure-Object -Maximum).Maximum;$maximum=[Math]::Max($maximum,$m);if($m -gt 3){$changed++}
 }}
 $result=@{schema=1;reference_sha256=(Get-FileHash $reference -Algorithm SHA256).Hash.ToLower();render_sha256=(Get-FileHash $render -Algorithm SHA256).Hash.ToLower();crop=@(333,211,1080,114);region=@(0,0,1080,20);sampled_background=@(100,600);mean_rgb_error=$total/(1080*20*3);maximum_rgb_error=$maximum;pixels_over_3=$changed;total_pixels=21600;confidence='Archived reference plus stale live row geometry; background-strip evidence only. No text, icons, other states or fresh full-row parity claim.'}
 $json=$result|ConvertTo-Json -Depth 5;[IO.File]::WriteAllText((Join-Path $PSScriptRoot '.tmp/objectmenuinside-comparison.json'),$json);$json
} finally {$graphics.Dispose();$composite.Dispose();$art.Dispose();$game.Dispose()}
