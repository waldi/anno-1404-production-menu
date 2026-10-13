# Existing loaded background metadata only. No GPU readback or game functions.
param([string]$Name='texture-current',[string]$Widget='MINIMAP_FORCE_NORTH')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'capture-reference.ps1') -Name $Name
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AnnoTextureReference {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadProcessMemory(IntPtr p,IntPtr a,byte[] b,int n,out IntPtr got);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr p);
 public class Tile {public int file_id;public uint texture_container,renderer_handle;public string filename;public int[] source,destination,cell;public int flags;}
 public class Capture {public uint widget,background,layout_color;public int index;public List<Tile> tiles=new List<Tile>();}
 static byte[] Read(IntPtr p,uint a,int n){if(a<0x10000||n<0||n>65536||(ulong)a+(ulong)n>uint.MaxValue)throw new Exception("Invalid bounded read");var b=new byte[n];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,n,out got)||got.ToInt64()!=n)throw new Exception("Read failed "+a.ToString("X8"));return b;}
 static uint U(IntPtr p,uint a){return BitConverter.ToUInt32(Read(p,a,4),0);}
 static int I(IntPtr p,uint a){return BitConverter.ToInt32(Read(p,a,4),0);}
 static string Narrow(IntPtr p,uint a){uint end=U(p,a+16),tag=U(p,a+20),begin=tag==a+16?a:U(p,a);if(end<begin||end-begin>4096)throw new Exception("Invalid STLport filename");return Encoding.UTF8.GetString(Read(p,begin,(int)(end-begin)));}
 public static Capture Snapshot(int pid,uint widget){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("Read-only OpenProcess failed");try{
  uint bg=U(p,widget+0x580);int index=I(p,bg+12),start=I(p,bg+16),count=I(p,bg+20);
  if(index<0||index>=count||count>256||start<0||start>65536)throw new Exception("Invalid active layout");
  uint layouts=U(p,bg+0x40),begin=U(p,layouts),end=U(p,layouts+4);if(end<begin||(long)(start+index+1)*0x60>end-begin)throw new Exception("Invalid layout vector");
  uint layout=begin+(uint)((start+index)*0x60);int tileStart=I(p,layout+4),tileCount=I(p,layout+8);if(tileStart<0||tileStart>65536||tileCount<0||tileCount>4096)throw new Exception("Invalid tile range");
  uint tiles=U(p,bg+0x44),tileBegin=U(p,tiles),tileEnd=U(p,tiles+4);if(tileEnd<tileBegin||(long)(tileStart+tileCount)*20>tileEnd-tileBegin)throw new Exception("Invalid tile vector");
  uint rects=U(p,bg+0x18),manager=U(p,0x012D334C),resources=U(p,manager+0x430),textures=U(p,resources+0x24),textureEnd=U(p,resources+0x28);if(textureEnd<textures||textureEnd-textures>65536)throw new Exception("Invalid texture vector");
  var result=new Capture{widget=widget,background=bg,index=index,layout_color=U(p,layout+0x5c)};
  for(int j=0;j<tileCount;j++){var b=Read(p,tileBegin+(uint)((tileStart+j)*20),20);uint w0=BitConverter.ToUInt32(b,0),w1=BitConverter.ToUInt32(b,4),w2=BitConverter.ToUInt32(b,8),w3=BitConverter.ToUInt32(b,12);if(b[16]!=0)throw new Exception("Old-format tile unsupported by metadata helper");int id=(int)(w0&0x3ff);if((long)(id+1)*4>textureEnd-textures)throw new Exception("Invalid texture FileID");uint texture=U(p,textures+(uint)(id*4));var r=Read(p,rects+(uint)(j*16),16);
   result.tiles.Add(new Tile{file_id=id,texture_container=texture,renderer_handle=U(p,texture+0x38),filename=Narrow(p,texture+0x50),source=new[]{(int)((w1>>15)&0x7fff),(int)(w2&0x7fff),(int)((w2>>15)&0x7fff),(int)(w3&0x7fff)},destination=new[]{BitConverter.ToInt32(r,0),BitConverter.ToInt32(r,4),BitConverter.ToInt32(r,8),BitConverter.ToInt32(r,12)},cell=new[]{I(p,texture+0x40),I(p,texture+0x44)},flags=(int)((w3>>15)&0x7f)});
  }return result;
 }finally{CloseHandle(p);}}
}
'@
$widgetMatches=@($widgets|Where-Object PathId -eq $Widget)
if($widgetMatches.Count -ne 1){throw 'Choose a uniquely matching widget PathId'}
$metadata=[AnnoTextureReference]::Snapshot($game.Id,$widgetMatches[0].Address)
$byAddress=@{};foreach($item in $widgets){$byAddress[[string]$item.Address]=$item}
$visibleChain=$true;$ancestor=$widgetMatches[0];$seenAncestors=[Collections.Generic.HashSet[uint]]::new()
while($ancestor){if(-not $seenAncestors.Add($ancestor.Address)){throw 'Invalid ancestor cycle'};if($ancestor.Visible -ne 1){$visibleChain=$false};if($ancestor.Parent -eq 0){break};$ancestor=$byAddress[[string]$ancestor.Parent];if(-not $ancestor){throw 'Missing captured ancestor'}}
$record=@{schema=1;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path).Hash.ToLowerInvariant();visible_chain=$visibleChain;metadata=$metadata;scope='Non-atomic cached background tiles and texture identity; no GPU pixels, game calls or process writes. Hidden-chain geometry is stale and must be excluded from pixel gates. Logical filename does not establish archive precedence.'}
[IO.File]::WriteAllText((Join-Path $destination "$Name-texture.json"),($record|ConvertTo-Json -Depth 8))
$record|ConvertTo-Json -Depth 8
