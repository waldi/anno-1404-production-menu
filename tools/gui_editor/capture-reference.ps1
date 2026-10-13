# Read-only Anno snapshot. Run elevated when the interactive process is hidden
# from the normal workspace shell. No bridge, input, suspension or game calls.
param([string]$Name='current')
$ErrorActionPreference='Stop'
if ($Name -notmatch '^[a-zA-Z0-9_-]+$') { throw 'Use a simple reference name' }
$destination=Join-Path $PSScriptRoot '.tmp/references'
New-Item -ItemType Directory -Force $destination | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AnnoReference {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadProcessMemory(IntPtr process,IntPtr address,byte[] bytes,int count,out IntPtr read);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window,IntPtr dc,uint flags);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr window,ref Point point);
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [StructLayout(LayoutKind.Sequential)] public struct Point {public int X,Y;}
 public class Widget {public uint Address;public uint Parent;public int Type;public string PathId;public string AccessId;public int[] Relative;public int[] Absolute;public float[] ScaledRelative;public int[] Scroll;public float Alpha;public int[] FrameAlignment;public int Visible;public uint Flags;public bool UsesBackgroundClient;public uint Background;public string BackgroundName;public uint[] Children;}
 public class Screen {public int[] ReferenceSize;public int[] PhysicalSize;public int[] GuiSize;public int Mode;public float[] Scale;public int[] AlignmentOffsets;}
 static byte[] Read(IntPtr p,uint a,int count){var b=new byte[count];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,count,out got)||got.ToInt64()!=count)throw new Exception("Read failed at "+a.ToString("X8"));return b;}
 static uint U(IntPtr p,uint a){return BitConverter.ToUInt32(Read(p,a,4),0);}
 static string Name(byte[] b,int o,int limit=0x41){int n=0;while(n<limit&&b[o+n]!=0)n++;return Encoding.UTF8.GetString(b,o,n);}
 static int[] R(byte[] b,int o){return new[]{BitConverter.ToInt32(b,o),BitConverter.ToInt32(b,o+4),BitConverter.ToInt32(b,o+8),BitConverter.ToInt32(b,o+12)};}
 static float[] F(byte[] b,int o,int count){var v=new float[count];for(int i=0;i<count;i++)v[i]=BitConverter.ToSingle(b,o+4*i);return v;}
 public static Screen ScreenSnapshot(int pid){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("OpenProcess read-only failed");try{var b=Read(p,U(p,0x0130430C),0x40);return new Screen{ReferenceSize=new[]{BitConverter.ToInt32(b,4),BitConverter.ToInt32(b,8)},PhysicalSize=new[]{BitConverter.ToInt32(b,12),BitConverter.ToInt32(b,16)},GuiSize=new[]{BitConverter.ToInt32(b,20),BitConverter.ToInt32(b,24)},Mode=BitConverter.ToInt32(b,32),Scale=F(b,40,2),AlignmentOffsets=R(b,48)};}finally{CloseHandle(p);}}
 public static Widget[] Snapshot(int pid){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("OpenProcess read-only failed");try{
  uint manager=U(p,0x012D334C),root=U(p,manager+0x420);var queue=new Queue<uint>();queue.Enqueue(root);var seen=new HashSet<uint>();var result=new List<Widget>();
  while(queue.Count>0){uint a=queue.Dequeue();if(a==0||!seen.Add(a))continue;if(seen.Count>20000)throw new Exception("Widget traversal limit exceeded");
   var b=Read(p,a,0x6d0);uint sentinel=a+0x80,node=BitConverter.ToUInt32(b,0x80);var children=new List<uint>();var links=new HashSet<uint>();
   while(node!=sentinel){if(node==0||!links.Add(node)||links.Count>20000)throw new Exception("Invalid child list");var link=Read(p,node,12);uint child=BitConverter.ToUInt32(link,8);children.Add(child);queue.Enqueue(child);node=BitConverter.ToUInt32(link,0);}
   int type=BitConverter.ToInt32(b,0xc4);
   result.Add(new Widget{Address=a,Parent=BitConverter.ToUInt32(b,0x88),Type=type,PathId=Name(b,0xc8),AccessId=Name(b,0x109),Relative=R(b,0x2a4),Absolute=R(b,0x8c),ScaledRelative=F(b,8,4),Scroll=new[]{BitConverter.ToInt32(b,0xa4),BitConverter.ToInt32(b,0xa8)},Alpha=BitConverter.ToSingle(b,0x20),FrameAlignment=type==11?R(Read(p,a+0x6f0,16),0):null,Visible=BitConverter.ToInt32(b,0x9c),Flags=BitConverter.ToUInt32(b,0x2a0),UsesBackgroundClient=b[0x588]!=0,Background=BitConverter.ToUInt32(b,0x580),BackgroundName=Name(b,0x199,0x104),Children=children.ToArray()});
  }return result.ToArray();
 }finally{CloseHandle(p);}}
}
'@
[AnnoReference]::SetProcessDPIAware() | Out-Null
$game=Get-Process Addon | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1
if (-not $game) {throw 'Interactive Addon window unavailable'}
$window=New-Object AnnoReference+Rect; $client=New-Object AnnoReference+Rect; $origin=New-Object AnnoReference+Point
if (-not [AnnoReference]::GetWindowRect($game.MainWindowHandle,[ref]$window)) {throw 'GetWindowRect failed'}
if (-not [AnnoReference]::GetClientRect($game.MainWindowHandle,[ref]$client)) {throw 'GetClientRect failed'}
[AnnoReference]::ClientToScreen($game.MainWindowHandle,[ref]$origin) | Out-Null
$widgets=[AnnoReference]::Snapshot($game.Id)
$bitmap=New-Object System.Drawing.Bitmap ($window.Right-$window.Left),($window.Bottom-$window.Top)
$graphics=[System.Drawing.Graphics]::FromImage($bitmap)
try {$dc=$graphics.GetHdc(); try {if(-not [AnnoReference]::PrintWindow($game.MainWindowHandle,$dc,2)){throw 'PrintWindow failed'}} finally {$graphics.ReleaseHdc($dc)}; $bitmap.Save((Join-Path $destination "$Name.png"))} finally {$graphics.Dispose();$bitmap.Dispose()}
$reference=@{schema=2;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path -Algorithm SHA256).Hash.ToLower();process_id=$game.Id;client_size=@($client.Right,$client.Bottom);gui_screen=[AnnoReference]::ScreenSnapshot($game.Id);client_origin_in_capture=@(($origin.X-$window.Left),($origin.Y-$window.Top));language=$null;scenario='Current screen; no input sent';confidence='Non-atomic live read. Invisible/off-clip absolute rectangles may be stale. Pair relative rectangles with visibility and validate stable captures.';widgets=$widgets}
[IO.File]::WriteAllText((Join-Path $destination "$Name.json"),($reference | ConvertTo-Json -Depth 10))
"Captured $($widgets.Count) widgets and $($client.Right)x$($client.Bottom) client pixels"
