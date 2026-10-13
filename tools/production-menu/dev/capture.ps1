# capture.ps1 - screenshot the running Anno window even when other windows cover it.
# Uses PrintWindow(PW_RENDERFULLCONTENT), which reads the DWM-composited surface, so
# the game does not have to be in the foreground. SetProcessDPIAware is required:
# without it GetWindowRect returns the DPI-virtualised size and the bitmap is a
# top-left crop of the real window.
#   powershell -ExecutionPolicy Bypass -File tools/production-menu/dev/capture.ps1 -out shot.png
#   ... -x 400 -y 300 -w 430 -h 90 -zoom 3 -out zoom.png   (also writes zoom_full.png)
# Crop coordinates are window pixels (title bar included). The GUI's own coordinate
# space starts at the client origin, so add the title-bar height (~37px) to GUI y.
param([int]$x,[int]$y,[int]$w,[int]$h,[string]$out,[int]$zoom=4,[ValidateSet('Addon','Anno1404Addon')][string]$ProcessName='Addon')
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class W3 { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
 [StructLayout(LayoutKind.Sequential)] public struct R { public int L,T,Rt,B; } }
'@
[W3]::SetProcessDPIAware() | Out-Null
$hw = (Get-Process -Name $ProcessName | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1).MainWindowHandle
if (-not $hw) { throw "No visible $ProcessName game window" }
$r = New-Object W3+R; [W3]::GetWindowRect($hw, [ref]$r) | Out-Null
$bmp = New-Object System.Drawing.Bitmap ($r.Rt-$r.L), ($r.B-$r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp); $hdc = $g.GetHdc(); [W3]::PrintWindow($hw, $hdc, 2) | Out-Null; $g.ReleaseHdc($hdc)
$bmp.Save(($out -replace '\.png$','_full.png'))
if ($w -gt 0) { if ($x+$w -gt $bmp.Width) { $w = $bmp.Width-$x }; if ($y+$h -gt $bmp.Height) { $h = $bmp.Height-$y }
  $rc = New-Object System.Drawing.Rectangle $x,$y,$w,$h; $crop = $bmp.Clone($rc,$bmp.PixelFormat)
  $big = New-Object System.Drawing.Bitmap ($w*$zoom),($h*$zoom); $g2=[System.Drawing.Graphics]::FromImage($big); $g2.InterpolationMode='NearestNeighbor'; $g2.DrawImage($crop,0,0,$w*$zoom,$h*$zoom); $big.Save($out)
}
"ok $($bmp.Width)x$($bmp.Height)"
