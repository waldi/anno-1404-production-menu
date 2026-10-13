# Bounded query/read capture of GUI atlas metadata. Ghidra partial types are
# authoritative; never use the game's DC/texture handles or call its renderer.
param([string]$Name='glyphs')
$ErrorActionPreference='Stop'
if($Name -notmatch '^[a-zA-Z0-9_-]+$'){throw 'Use a simple reference name'}
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$game=Get-Process -Name Addon -ErrorAction Stop | Where-Object {$_.Path -eq (Join-Path $repo 'Anno 1404 Gold Edition/Addon.exe')} | Select-Object -First 1
if(-not $game){throw 'Repo-local Addon.exe not visible; use elevated read-only inspection'}
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AnnoGlyphReference {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll")] static extern bool ReadProcessMemory(IntPtr p,IntPtr address,byte[] bytes,int count,out IntPtr got);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr p);
 public class Glyph {public int Code,Width,Height,Advance,Bearing;public float[] UV;}
 public class Page {public int Index;public List<Glyph> Glyphs=new List<Glyph>();}
 public class Font {public int Handle;public string Name,Face;public int Width,Height,Weight,Italic,Quality,Outline,Underline,Shadow,ShadowX,ShadowY,CachedHeight;public uint OutlineColor,ShadowColor,Resource;public float[] Scale;public List<Glyph> Glyphs=new List<Glyph>();public List<Page> Pages=new List<Page>();}
 public class Capture {public List<Font> Fonts=new List<Font>();public List<string> Diagnostics=new List<string>();}
 static byte[] B(IntPtr p,uint a,int n){if(n<0||n>65536)throw new Exception("Read bound");var b=new byte[n];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,n,out got)||got.ToInt64()!=n)throw new Exception("Read failed at "+a.ToString("X8"));return b;}
 static uint U(IntPtr p,uint a){return BitConverter.ToUInt32(B(p,a,4),0);}
 static int I(byte[] b,int o){return BitConverter.ToInt32(b,o);}
 static string Narrow(IntPtr p,uint a){uint end=U(p,a+0x10),begin=U(p,a+0x14)==a+0x10?a:U(p,a);if(end<begin||end-begin>4096)throw new Exception("Invalid font name");return Encoding.UTF8.GetString(B(p,begin,(int)(end-begin)));}
 static Page ReadPage(IntPtr p,uint address,int index){if(index<0||index>254)throw new Exception("Glyph page bound");var bytes=B(p,address+8,256*32);var page=new Page{Index=index};for(int ch=0;ch<256;ch++){int o=ch*32;page.Glyphs.Add(new Glyph{Code=index*256+ch,Width=I(bytes,o),Height=I(bytes,o+4),Advance=I(bytes,o+8),Bearing=I(bytes,o+12),UV=new[]{BitConverter.ToSingle(bytes,o+16),BitConverter.ToSingle(bytes,o+20),BitConverter.ToSingle(bytes,o+24),BitConverter.ToSingle(bytes,o+28)}});}return page;}
 public static Capture Read(int pid){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("Read-only OpenProcess failed");try{
  var c=new Capture();uint facade=U(p,0x013049E0),begin=U(p,facade+0x974),end=U(p,facade+0x978),types=U(p,facade+0x980);if(end<begin||(end-begin)%4!=0||(end-begin)/4>131072)throw new Exception("Resource vector bound: facade="+facade.ToString("X8")+" begin="+begin.ToString("X8")+" end="+end.ToString("X8")+" types="+types.ToString("X8"));int count=(int)((end-begin)/4);
  var names=new Dictionary<int,string>();uint manager=U(p,0x012D334C),resources=U(p,manager+0x430),sentinel=resources+0x1c,node=U(p,sentinel);var seen=new HashSet<uint>();while(node!=sentinel){if(node==0||!seen.Add(node)||seen.Count>512)throw new Exception("Font list bound");uint f=U(p,node+8);int handle=(int)U(p,f+0x38);if(handle>=0)names[handle]=Narrow(p,f+4);node=U(p,node);}
  foreach(int handle in names.Keys){if(handle>=count||B(p,types+(uint)handle,1)[0]!=3)continue;try{uint a=U(p,begin+(uint)handle*4);if(a==0)continue;if(c.Fonts.Count>=512)throw new Exception("Font resource bound");var b=B(p,a,0x124);uint drawManager=BitConverter.ToUInt32(b,0x120);var scale=B(p,drawManager+0x88,8);uint page=BitConverter.ToUInt32(b,0x110);if(page==0)throw new Exception("Page0 absent");string name;names.TryGetValue(handle,out name);
   var font=new Font{Handle=handle,Name=name,Resource=a,Scale=new[]{BitConverter.ToSingle(scale,0),BitConverter.ToSingle(scale,4)},Face=Encoding.Unicode.GetString(b,0x5c,64).Split('\0')[0],Width=I(b,0x9c),Height=I(b,0xa0),Weight=I(b,0xa4),Italic=I(b,0xa8),Quality=I(b,0xac),Outline=I(b,0xb0),OutlineColor=BitConverter.ToUInt32(b,0xb4),Shadow=I(b,0xb8),ShadowX=I(b,0xbc),ShadowY=I(b,0xc0),ShadowColor=BitConverter.ToUInt32(b,0xc4),Underline=I(b,0xcc),CachedHeight=I(b,0xd4)};
   font.Glyphs=ReadPage(p,page,0).Glyphs;var pending=new Queue<uint>();pending.Enqueue(BitConverter.ToUInt32(b,0xfc));var pageNodes=new HashSet<uint>();while(pending.Count!=0){uint n=pending.Dequeue();if(n==0)continue;if(!pageNodes.Add(n)||pageNodes.Count>255)throw new Exception("Glyph page tree bound/cycle");var entry=B(p,n,24);int index=I(entry,16);if(index!=0)font.Pages.Add(ReadPage(p,BitConverter.ToUInt32(entry,20),index));pending.Enqueue(BitConverter.ToUInt32(entry,8));pending.Enqueue(BitConverter.ToUInt32(entry,12));}c.Fonts.Add(font);
  }catch(Exception e){c.Diagnostics.Add(handle+": "+e.Message);}}
  return c;
 }finally{CloseHandle(p);}}
}
'@
$capture=[AnnoGlyphReference]::Read($game.Id)
$record=@{schema=1;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path).Hash.ToLowerInvariant();fonts=$capture.Fonts;diagnostics=$capture.Diagnostics;scope='GUI type3 resources: already-loaded glyph page metadata and original font-command inputs only. Cached widths/UV are outputs for independent comparison. No game DC or texture handles used; missing pages are not created in the game.'}
$destination=Join-Path $PSScriptRoot '.tmp/references'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
[IO.File]::WriteAllText((Join-Path $destination "$Name-glyphs.json"),($record|ConvertTo-Json -Depth 8))
$extraPages=[int](($capture.Fonts | ForEach-Object {$_.Pages.Count} | Measure-Object -Sum).Sum)
Write-Output "Captured $($capture.Fonts.Count) font resources, $(($capture.Fonts.Count+$extraPages)*256) glyphs ($extraPages higher pages); diagnostics=$($capture.Diagnostics.Count)"
