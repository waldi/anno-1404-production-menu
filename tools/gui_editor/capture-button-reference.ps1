# Existing cached button states only: no process writes, game calls or UI input.
param([string]$Name='buttons-current')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'capture-reference.ps1') -Name $Name
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class AnnoButtonReference {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadProcessMemory(IntPtr process,IntPtr address,byte[] bytes,int count,out IntPtr read);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 public class Input {public bool hovered,pressed,@checked,suppress_hover,force_disabled,force_checked_hover;}
 public class Button {public uint widget;public Input input;public bool enabled_chain;public List<string> layouts;public int actual_index;public int icon_guid;public bool text_enabled;public uint effective_color,effective_color2,background_color,background_color2;public float background_fade;}
 public class Capture {public List<Button> buttons=new List<Button>();public List<string> diagnostics=new List<string>();}
 static byte[] Read(IntPtr p,uint a,int count){if(a<0x10000||count<0||count>65536)throw new Exception("Invalid bounded read");var b=new byte[count];IntPtr got;if(!ReadProcessMemory(p,new IntPtr((long)a),b,count,out got)||got.ToInt64()!=count)throw new Exception("Read failed at "+a.ToString("X8"));return b;}
 static uint U(IntPtr p,uint a){return BitConverter.ToUInt32(Read(p,a,4),0);}
 static int I(IntPtr p,uint a){return BitConverter.ToInt32(Read(p,a,4),0);}
 static bool B(IntPtr p,uint a){return Read(p,a,1)[0]!=0;}
 public static Capture Snapshot(int pid,uint[] addresses){var p=OpenProcess(0x410,false,pid);if(p==IntPtr.Zero)throw new Exception("Read-only OpenProcess failed");try{
  var result=new Capture();uint hover=U(p,0x012D32CC);
  foreach(uint widget in addresses){try{if(I(p,widget+0xc4)!=2)continue;uint bg=U(p,widget+0x580);if(bg==0)continue;
   bool visible=true,enabled=true;uint ancestor=widget;var seen=new HashSet<uint>();while(ancestor!=0){if(!seen.Add(ancestor)||seen.Count>20000)throw new Exception("Invalid ancestor chain");if(I(p,ancestor+0x9c)!=1)visible=false;if((U(p,ancestor+0x2a0)&0x10000000)!=0)enabled=false;ancestor=U(p,ancestor+0x88);}if(!visible)continue;
   int count=I(p,bg+0x14),start=I(p,bg+0x10);if(count<1)continue;if(count>256||start<0||start>65536)throw new Exception("Invalid layout subset");uint vector=U(p,bg+0x40),begin=U(p,vector),end=U(p,vector+4);if(end<begin||(long)(start+count)*0x60>end-begin)throw new Exception("Invalid layout vector");
   var names=new List<string>();for(int i=0;i<count;i++){var bytes=Read(p,begin+(uint)((start+i)*0x60+0x28),50);int n=Array.IndexOf(bytes,(byte)0);names.Add(Encoding.ASCII.GetString(bytes,0,n<0?50:n));}
   result.buttons.Add(new Button{widget=widget,enabled_chain=enabled,layouts=names,actual_index=I(p,bg+0xc),icon_guid=I(p,widget+0xa38),text_enabled=(U(p,widget+0x2a0)&1)!=0,effective_color=U(p,widget+0x24),effective_color2=U(p,widget+0x28),background_color=U(p,bg+0x24),background_color2=U(p,bg+0x28),background_fade=BitConverter.ToSingle(Read(p,widget+0x58c,4),0),input=new Input{hovered=hover==widget||B(p,widget+0xa61),pressed=B(p,widget+0xbc),@checked=B(p,widget+0xa60),suppress_hover=(U(p,widget+0x2a0)&0x02000000)!=0,force_disabled=B(p,widget+0xaa8),force_checked_hover=B(p,widget+0xaa9)}});
  }catch(Exception e){result.diagnostics.Add(widget.ToString("X8")+": "+e.Message);}}
  return result;
 }finally{CloseHandle(p);}}
}
'@
$states=[AnnoButtonReference]::Snapshot($game.Id,[uint[]]@($widgets | ForEach-Object Address))
$record=@{schema=1;captured_utc=[DateTime]::UtcNow.ToString('o');binary_sha256=(Get-FileHash -LiteralPath $game.Path).Hash.ToLowerInvariant();buttons=$states.buttons;diagnostics=$states.diagnostics;scope='Visible ancestor-chain BUTTON cached layout indices. Non-atomic reads; no input or game calls. Hidden buttons excluded, glyph and pixel parity not implied.'}
[IO.File]::WriteAllText((Join-Path $destination "$Name-buttons.json"),($record | ConvertTo-Json -Depth 10))
"Captured $($states.buttons.Count) visible-chain button states; diagnostics=$($states.diagnostics.Count)"
