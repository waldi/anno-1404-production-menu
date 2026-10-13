# Read-only cached text/font capture; no game function calls or cross-process GDI.
param([string]$Name='text-current')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'capture-reference.ps1') -Name $Name
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AnnoTextReference {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadProcessMemory(IntPtr process,IntPtr address,byte[] bytes,int count,out IntPtr read);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 public class Font {public uint Address;public string Name;public string Face;public uint Flags;public int Height;public int Width;public int Weight;public uint DefaultColor;public int[] ShadowOffset;public uint ShadowColor;public uint OutlineColor;}
 public class Run {public uint Widget;public uint Part;public int ProcessedId;public string Text;public int[] Size;public int[] Offset;public int[] LineRect;public uint Color;public uint Font;public int IconGuid,IconVariation;public byte IconClip;public uint Texture,RenderTarget;}
 public class Block {public uint Widget;public int ProcessedId,Mode,Guid,LayoutWidth,ParentMaxWidth;public string OriginalText,FontName;public uint DataFlags,LayoutFlags,LayoutTextColor;public int[] ClientSize,ProcessedSize;}
 public class Capture {public List<Font> Fonts=new List<Font>();public List<Run> Runs=new List<Run>();public List<Run> Parts=new List<Run>();public List<Block> Blocks=new List<Block>();public List<string> Diagnostics=new List<string>();}
 static byte[] Read(IntPtr p,uint a,int count){if(a<0x10000||count<0||count>65536)throw new Exception("Invalid bounded read");var b=new byte[count];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,count,out got)||got.ToInt64()!=count)throw new Exception("Read failed at "+a.ToString("X8"));return b;}
 static uint U(IntPtr p,uint a){return BitConverter.ToUInt32(Read(p,a,4),0);}
 static int I(IntPtr p,uint a){return BitConverter.ToInt32(Read(p,a,4),0);}
 static string Narrow(IntPtr p,uint a){uint end=U(p,a+0x10),begin=U(p,a+0x14)==a+0x10?a:U(p,a);if(end<begin||end-begin>4096)throw new Exception("Invalid narrow string");return Encoding.UTF8.GetString(Read(p,begin,(int)(end-begin)));}
 static string Wide(IntPtr p,uint a){uint end=U(p,a+0x20),begin=U(p,a+0x24)==a+0x20?a:U(p,a);if(end<begin||end-begin>32768||(end-begin)%2!=0)throw new Exception("Invalid UTF16 string");return Encoding.Unicode.GetString(Read(p,begin,(int)(end-begin)));}
 static string WidePointer(IntPtr p,uint a){var bytes=new List<byte>();for(int i=0;i<16384;i++){var b=Read(p,a+(uint)(i*2),2);if(b[0]==0&&b[1]==0)return Encoding.Unicode.GetString(bytes.ToArray());bytes.AddRange(b);}throw new Exception("Unterminated bounded text pointer");}
 static int[] R(IntPtr p,uint a,int count){var b=Read(p,a,count*4);var v=new int[count];for(int i=0;i<count;i++)v[i]=BitConverter.ToInt32(b,i*4);return v;}
 static uint FindProcessed(IntPtr p,uint engine,int id){uint node=U(p,engine+0x2c);var seen=new HashSet<uint>();while(node!=0){if(!seen.Add(node)||seen.Count>20000)throw new Exception("Invalid processed-text tree");int key=I(p,node+0x10);if(id==key)return U(p,node+0x14);node=U(p,node+(id<key?8u:12u));}return 0;}
 static List<uint> Items(IntPtr p,uint sentinel){var values=new List<uint>();var seen=new HashSet<uint>();uint node=U(p,sentinel);while(node!=sentinel){if(node==0||!seen.Add(node)||seen.Count>20000)throw new Exception("Invalid intrusive list");values.Add(U(p,node+8));node=U(p,node);}return values;}
 static Font ReadFont(IntPtr p,uint a){var b=Read(p,a,0xa0);string face=Encoding.Unicode.GetString(b,0x60,64).Split('\0')[0];return new Font{Address=a,Name=Narrow(p,a+4),Face=face,Flags=BitConverter.ToUInt32(b,0x3c),Height=BitConverter.ToInt32(b,0x40),Width=BitConverter.ToInt32(b,0x44),Weight=BitConverter.ToInt32(b,0x48),DefaultColor=BitConverter.ToUInt32(b,0x4c),ShadowOffset=new[]{BitConverter.ToInt32(b,0x58),BitConverter.ToInt32(b,0x5c)},ShadowColor=BitConverter.ToUInt32(b,0x50),OutlineColor=BitConverter.ToUInt32(b,0x54)};}
 public static Capture Snapshot(int pid,uint[] widgets){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("Read-only OpenProcess failed");try{var result=new Capture();uint manager=U(p,0x012D334C),resources=U(p,manager+0x430),engine=U(p,0x012D3038);var fonts=new HashSet<uint>();foreach(uint f in Items(p,resources+0x1c)){fonts.Add(f);result.Fonts.Add(ReadFont(p,f));}
 foreach(uint widget in widgets){try{if(I(p,widget+0xc4)!=1)continue;uint td=U(p,widget+0x8cc);if(td==0)continue;int id=I(p,td+0xc);uint processed=FindProcessed(p,engine,id);if(processed==0)continue;
  int mode=I(p,td);string original=mode==4?Wide(p,td+0x18):(mode==2||mode==3?WidePointer(p,U(p,td+0x10)):null);
  result.Blocks.Add(new Block{Widget=widget,ProcessedId=id,Mode=mode,Guid=I(p,td+0x14),OriginalText=original,FontName=Narrow(p,td+0xc8),DataFlags=U(p,td+4),LayoutFlags=U(p,td+0x88),LayoutTextColor=U(p,td+0xe8),ClientSize=R(p,td+0x48,2),ProcessedSize=R(p,processed+0x20,2),LayoutWidth=I(p,processed+0x28),ParentMaxWidth=I(p,td+8)});
  foreach(uint line in Items(p,processed+0x18)){foreach(uint part in Items(p,line)){uint font=U(p,part+0x44);if(font!=0&&fonts.Add(font))result.Fonts.Add(ReadFont(p,font));var run=new Run{Widget=widget,Part=part,ProcessedId=id,Text=Wide(p,part+4),Size=R(p,part+0x34,2),Offset=R(p,part+0x3c,2),LineRect=R(p,line+8,4),Color=U(p,part),Font=font,IconGuid=I(p,part+0x50),IconVariation=I(p,part+0x54),IconClip=Read(p,part+0x5d,1)[0],Texture=U(p,part+0x48),RenderTarget=U(p,part+0x58)};result.Parts.Add(run);if(font!=0)result.Runs.Add(run);}}
 }catch(Exception e){result.Diagnostics.Add(widget.ToString("X8")+": "+e.Message);}}
 return result;}finally{CloseHandle(p);}}
}
'@
$text=[AnnoTextReference]::Snapshot($game.Id,[uint[]]@($widgets | ForEach-Object Address))
$record=@{schema=1;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path).Hash.ToLowerInvariant();gui_screen=[AnnoReference]::ScreenSnapshot($game.Id);fonts=$text.Fonts;runs=$text.Runs;parts=$text.Parts;blocks=$text.Blocks;diagnostics=$text.Diagnostics;scope='Cached LABEL text/inline parts, independent TextData source/parameters and font settings; non-atomic read. No renderer/GDI handles are used and no game calls are made. Cached part sizes/offsets/line rectangles are comparison outputs, not layout inputs.'}
$target=Join-Path $destination "$Name-text.json"
[IO.File]::WriteAllText($target,($record|ConvertTo-Json -Depth 10))
Write-Output "Captured $($text.Fonts.Count) fonts and $($text.Runs.Count) text runs; diagnostics=$($text.Diagnostics.Count)"
