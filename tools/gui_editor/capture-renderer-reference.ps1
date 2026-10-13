# Bounded read-only renderer selection. Structures/selection recorded in Ghidra
# before capture; no game calls, suspension, injection or renderer handle use.
param([string]$Name='renderer')
$ErrorActionPreference='Stop'
if($Name -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Use a simple reference name'}
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$game=Get-Process -Name Addon -ErrorAction Stop | Where-Object {$_.Path -eq (Join-Path $repo 'Anno 1404 Gold Edition/Addon.exe')} | Select-Object -First 1
if(-not $game){throw 'Repo-local Addon.exe not visible; use elevated read-only inspection'}
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class AnnoRendererReference {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll")] static extern bool ReadProcessMemory(IntPtr p,IntPtr address,byte[] bytes,int count,out IntPtr got);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr p);
 public class Capture {public uint Facade,Backend,BackendVtable,Direct3D,Direct3DVtable;public int Mode;}
 static uint U(IntPtr p,uint a){var b=new byte[4];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,4,out got)||got.ToInt64()!=4)throw new Exception("Read failed at "+a.ToString("X8"));return BitConverter.ToUInt32(b,0);}
 public static Capture Read(int pid){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("OpenProcess read-only failed");try{var c=new Capture();c.Facade=U(p,0x013049E0);c.Mode=(int)U(p,c.Facade+0x68);c.Backend=U(p,c.Facade+0x87c);c.BackendVtable=U(p,c.Backend);c.Direct3D=U(p,c.Backend+0x234);c.Direct3DVtable=U(p,c.Direct3D);return c;}finally{CloseHandle(p);}}
}
'@
$capture=[AnnoRendererReference]::Read($game.Id)
$record=@{schema=1;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path).Hash.ToLowerInvariant();renderer=$capture;scope='Bounded query/read access to selected renderer pointers; no renderer methods or handles used.'}
$destination=Join-Path $PSScriptRoot '.tmp/references'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
[IO.File]::WriteAllText((Join-Path $destination "$Name-renderer.json"),($record|ConvertTo-Json -Depth 6))
$record|ConvertTo-Json -Depth 6
